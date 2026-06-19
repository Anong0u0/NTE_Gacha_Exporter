use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use nte_automation::{
    AutoPageOptions as AutomationOptions, AutoPageResult as AutoPageRunResult,
    AutoPageStatus as AutomationStatus, RecordSnapshot as AutomationRecordSnapshot, run_auto_page,
};
use nte_capture::{
    CaptureOptions, CaptureRecordBuilder, CaptureTarget, build_capture_document, candidate_ports,
    capture_live, find_process_pid,
};
use nte_core::ImportReport;
use nte_store::load_locale_or_settings;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::State;

use crate::admin::admin_relaunch_required;
use crate::error::{ApiError, RuntimeError, api_error, api_error_message};
use crate::state::{AppState, new_session_id, now_seconds, with_store};

const CAPTURE_DRAIN_TIMEOUT: Duration = Duration::from_secs(20);
const CAPTURE_DRAIN_POLL_INTERVAL: Duration = Duration::from_millis(100);
const CAPTURE_SESSION_RETENTION_SECONDS: f64 = 30.0 * 60.0;
const CAPTURE_TERMINAL_SESSION_LIMIT: usize = 20;

#[derive(Debug, Clone)]
pub(crate) struct CaptureSessionMeta {
    profile_name: String,
    source_kind: String,
    source_path: Option<String>,
    full_update: bool,
    import_report: Option<ImportReport>,
}

pub(crate) struct CaptureRuntimeSession {
    status: Mutex<CaptureStatus>,
    stop: Arc<AtomicBool>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CaptureMode {
    LiveOnly,
    AutoPageIncremental,
    AutoPageFull,
}

impl CaptureMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::LiveOnly => "live_only",
            Self::AutoPageIncremental => "auto_page_incremental",
            Self::AutoPageFull => "auto_page_full",
        }
    }

    fn auto_page(self) -> bool {
        matches!(self, Self::AutoPageIncremental | Self::AutoPageFull)
    }

    fn full_update(self) -> bool {
        matches!(self, Self::AutoPageFull)
    }

    fn source_kind(self) -> &'static str {
        match self {
            Self::LiveOnly => "live_capture",
            Self::AutoPageIncremental => "auto_page_capture",
            Self::AutoPageFull => "auto_page_full",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct CaptureCounters {
    packets_seen: u64,
    decoded_packets: u64,
    dropped_packets: u64,
    #[serde(default)]
    duplicate_packets: u64,
    #[serde(default)]
    filter_restarts: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct CapturePageKey {
    stream_key: String,
    generation_index: u32,
    page_index: u32,
}

#[derive(Debug, Default)]
struct CapturePageTracker {
    pages_by_pool: BTreeMap<String, BTreeSet<CapturePageKey>>,
    last_decoded_at: Option<f64>,
}

impl CapturePageTracker {
    fn add_progress(&mut self, progress: &nte_capture::CaptureProgress) {
        let mut changed = false;
        for row in &progress.new_rows {
            let Some(pool) = capture_pool(row.record_type.as_str(), row.pool_id.as_deref()) else {
                continue;
            };
            let Some(page_index) = row.source.segment_index.or(row.source.page_index) else {
                continue;
            };
            let stream_key = row.source.stream_key.clone().unwrap_or_else(|| {
                format!(
                    "{}:{}",
                    row.record_type.as_str(),
                    row.pool_id.as_deref().unwrap_or_default()
                )
            });
            let key = CapturePageKey {
                stream_key,
                generation_index: row.source.generation_index.unwrap_or_default(),
                page_index,
            };
            changed |= self
                .pages_by_pool
                .entry(pool.to_string())
                .or_default()
                .insert(key);
        }
        if changed {
            self.last_decoded_at = Some(now_seconds());
        }
    }

    fn count(&self, pool: &str) -> usize {
        self.pages_by_pool
            .get(pool)
            .map(BTreeSet::len)
            .unwrap_or_default()
    }

    fn counts(&self) -> BTreeMap<String, usize> {
        self.pages_by_pool
            .iter()
            .map(|(pool, pages)| (pool.clone(), pages.len()))
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CaptureStatus {
    session_id: String,
    state: String,
    mode: String,
    records_count: u64,
    latest_records: Vec<Value>,
    counters: CaptureCounters,
    started_at: f64,
    updated_at: f64,
    target: Option<Value>,
    auto_page: Option<Value>,
    raw_path: Option<String>,
    error: Option<RuntimeError>,
    #[serde(default, skip_serializing)]
    document: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    import_report: Option<ImportReport>,
}

impl From<nte_capture::CaptureCounters> for CaptureCounters {
    fn from(value: nte_capture::CaptureCounters) -> Self {
        Self {
            packets_seen: value.packets_seen,
            decoded_packets: value.decoded_packets,
            dropped_packets: value.dropped_packets,
            duplicate_packets: value.duplicate_packets,
            filter_restarts: value.filter_restarts,
        }
    }
}

#[tauri::command]
pub(crate) fn capture_start(
    state: State<'_, AppState>,
    profile_name: String,
    locale: Option<String>,
    mode: Option<CaptureMode>,
) -> Result<CaptureStatus, ApiError> {
    let mode = mode.unwrap_or(CaptureMode::LiveOnly);
    if admin_relaunch_required()? {
        return Err(api_error_message(
            "admin_required",
            "pktmon capture requires administrator permission",
        ));
    }
    let locale = with_store(&state, |store| {
        let locale = load_locale_or_settings(store, locale)?;
        store.dashboard_overview(&profile_name, &locale)?;
        Ok(locale)
    })?;
    let output_raw = with_store(&state, |store| {
        let raw_path = if mode.auto_page() {
            Some(store.default_run_raw_path().to_string_lossy().to_string())
        } else {
            None
        };
        Ok(raw_path)
    })?;
    let known_record_ids = if mode == CaptureMode::AutoPageIncremental {
        with_store(&state, |store| store.profile_record_ids(&profile_name))?
    } else {
        Vec::new()
    };
    let mut status =
        start_rust_capture_session(&state, &locale, mode, output_raw.clone(), known_record_ids)?;
    status.import_report = None;
    let source_path = status.raw_path.clone().or(output_raw);
    state
        .captures
        .lock()
        .map_err(|_| api_error_message("capture_lock_poisoned", "capture lock poisoned"))?
        .insert(
            status.session_id.clone(),
            CaptureSessionMeta {
                profile_name,
                source_kind: mode.source_kind().to_string(),
                source_path,
                full_update: mode.full_update(),
                import_report: None,
            },
        );
    if status.state == "completed" {
        return capture_status_with_merge(&state, &status.session_id);
    }
    Ok(status)
}

#[tauri::command]
pub(crate) fn capture_status(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<CaptureStatus, ApiError> {
    capture_status_with_merge(&state, &session_id)
}

#[tauri::command]
pub(crate) fn capture_stop(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<CaptureStatus, ApiError> {
    let session = capture_runtime_session(&state, &session_id)?;
    session.stop.store(true, Ordering::SeqCst);
    {
        let mut status = session
            .status
            .lock()
            .map_err(|_| api_error_message("capture_lock_poisoned", "capture lock poisoned"))?;
        if matches!(status.state.as_str(), "starting" | "running") {
            status.state = "stopping".to_string();
            status.updated_at = now_seconds();
        }
    }
    capture_status_with_merge(&state, &session_id)
}

fn start_rust_capture_session(
    state: &State<'_, AppState>,
    locale: &str,
    mode: CaptureMode,
    output_raw: Option<String>,
    known_record_ids: Vec<String>,
) -> Result<CaptureStatus, ApiError> {
    let pid = find_process_pid("HTGame.exe")
        .map_err(api_error)?
        .ok_or_else(|| api_error_message("capture_environment", "HTGame.exe not found"))?;
    let ports = candidate_ports(pid).map_err(api_error)?;
    if ports.is_empty() {
        return Err(api_error_message(
            "capture_environment",
            "no HTGame.exe candidate ports",
        ));
    }
    let session_id = new_session_id();
    let now = now_seconds();
    let target = CaptureTarget {
        pid,
        exe: "HTGame.exe".to_string(),
        interface: "pktmon".to_string(),
        ports: ports.clone(),
        bpf: ports
            .iter()
            .map(|port| format!("port {port}"))
            .collect::<Vec<_>>()
            .join(" or "),
    };
    let initial_status = CaptureStatus {
        session_id: session_id.clone(),
        state: "starting".to_string(),
        mode: mode.as_str().to_string(),
        records_count: 0,
        latest_records: Vec::new(),
        counters: CaptureCounters::default(),
        started_at: now,
        updated_at: now,
        target: Some(serde_json::to_value(&target).map_err(api_error)?),
        auto_page: None,
        raw_path: output_raw.clone(),
        error: None,
        document: None,
        import_report: None,
    };
    let stop = Arc::new(AtomicBool::new(false));
    let runtime = Arc::new(CaptureRuntimeSession {
        status: Mutex::new(initial_status.clone()),
        stop: Arc::clone(&stop),
        handle: Mutex::new(None),
    });
    state
        .capture_sessions
        .lock()
        .map_err(|_| api_error_message("capture_lock_poisoned", "capture lock poisoned"))?
        .insert(session_id.clone(), Arc::clone(&runtime));

    let raw_out = output_raw.map(PathBuf::from);
    let locale_for_thread = locale.to_string();
    let snapshots = Arc::new(Mutex::new(Vec::<AutomationRecordSnapshot>::new()));
    let progress_snapshots = mode.auto_page().then(|| Arc::clone(&snapshots));
    let page_tracker = Arc::new(Mutex::new(CapturePageTracker::default()));
    let progress_page_tracker = mode.auto_page().then(|| Arc::clone(&page_tracker));
    let callback = capture_progress_callback(
        Arc::clone(&runtime),
        locale.to_string(),
        progress_snapshots,
        progress_page_tracker,
    );
    let runtime_for_thread = Arc::clone(&runtime);
    let stop_for_thread = Arc::clone(&stop);
    let handle = std::thread::spawn(move || {
        if mode.auto_page() {
            run_auto_page_capture_thread(AutoPageCaptureThread {
                runtime: runtime_for_thread,
                pid,
                ports,
                raw_out,
                locale: locale_for_thread,
                stop: stop_for_thread,
                callback,
                mode,
                known_record_ids,
                snapshots,
                page_tracker,
            });
        } else {
            let result = capture_live(
                CaptureOptions {
                    pid,
                    exe: "HTGame.exe".to_string(),
                    ports,
                    raw_out,
                    max_packets: 0,
                    max_decoded: 0,
                    on_progress: Some(callback),
                },
                stop_for_thread,
            );
            finish_capture_result(
                &runtime_for_thread,
                result.map_err(|error| error.to_string()),
                &locale_for_thread,
                "pktmon-live-capture",
                None,
                None,
            );
        }
    });
    *runtime
        .handle
        .lock()
        .map_err(|_| api_error_message("capture_lock_poisoned", "capture lock poisoned"))? =
        Some(handle);
    Ok(initial_status)
}

struct AutoPageCaptureThread {
    runtime: Arc<CaptureRuntimeSession>,
    pid: u32,
    ports: Vec<u16>,
    raw_out: Option<PathBuf>,
    locale: String,
    stop: Arc<AtomicBool>,
    callback: Arc<dyn Fn(nte_capture::CaptureProgress) + Send + Sync + 'static>,
    mode: CaptureMode,
    known_record_ids: Vec<String>,
    snapshots: Arc<Mutex<Vec<AutomationRecordSnapshot>>>,
    page_tracker: Arc<Mutex<CapturePageTracker>>,
}

fn run_auto_page_capture_thread(context: AutoPageCaptureThread) {
    let AutoPageCaptureThread {
        runtime,
        pid,
        ports,
        raw_out,
        locale,
        stop,
        callback,
        mode,
        known_record_ids,
        snapshots,
        page_tracker,
    } = context;
    let capture_stop = Arc::clone(&stop);
    let capture_handle = std::thread::spawn(move || {
        capture_live(
            CaptureOptions {
                pid,
                exe: "HTGame.exe".to_string(),
                ports,
                raw_out,
                max_packets: 0,
                max_decoded: 0,
                on_progress: Some(callback),
            },
            capture_stop,
        )
        .map_err(|error| error.to_string())
    });

    let auto_runtime = Arc::clone(&runtime);
    let auto_status_callback = Arc::new(move |status: AutomationStatus| {
        if let Ok(mut capture_status) = auto_runtime.status.lock() {
            capture_status.auto_page = Some(auto_page_status_value(&status, "running"));
            if capture_status.state != "stopping" {
                capture_status.state = "running".to_string();
            }
            capture_status.updated_at = now_seconds();
        }
    });
    let snapshot_callback = {
        let snapshots = Arc::clone(&snapshots);
        Arc::new(move || {
            snapshots
                .lock()
                .map(|records| records.clone())
                .unwrap_or_default()
        })
    };
    let decoded_page_count = {
        let page_tracker = Arc::clone(&page_tracker);
        Arc::new(move |pool: &str| {
            page_tracker
                .lock()
                .map(|tracker| tracker.count(pool))
                .unwrap_or_default()
        })
    };
    let mut options = AutomationOptions::new(pid, Arc::clone(&stop));
    options.full_update = mode.full_update();
    options.non_interactive = true;
    options.known_record_ids = known_record_ids;
    options.record_snapshot = Some(snapshot_callback);
    options.decoded_page_count = Some(decoded_page_count);
    options.on_status = Some(auto_status_callback);
    let auto_result = run_auto_page(options);
    let drain_error = auto_result
        .succeeded()
        .then(|| wait_for_capture_drain(&runtime, &page_tracker, &auto_result, &stop))
        .flatten();
    stop.store(true, Ordering::SeqCst);

    let capture_result = capture_handle
        .join()
        .map_err(|_| "capture worker panicked".to_string())
        .and_then(|result| result);
    let auto_page = Some(auto_page_result_value(&auto_result));
    let error = if auto_result.succeeded() {
        drain_error
    } else {
        Some(RuntimeError {
            code: "auto_page_failed".to_string(),
            message: auto_result.message.clone(),
        })
    };
    finish_capture_result(
        &runtime,
        capture_result,
        &locale,
        "pktmon-auto-page-capture",
        auto_page,
        error,
    );
}

fn wait_for_capture_drain(
    runtime: &Arc<CaptureRuntimeSession>,
    page_tracker: &Arc<Mutex<CapturePageTracker>>,
    auto_result: &AutoPageRunResult,
    stop: &Arc<AtomicBool>,
) -> Option<RuntimeError> {
    let required = auto_result.visited_pages_by_pool.clone();
    if required.is_empty() {
        return None;
    }

    let started = Instant::now();
    loop {
        if stop.load(Ordering::SeqCst) {
            return Some(RuntimeError {
                code: "capture_stopped".to_string(),
                message: "capture stopped before packet drain completed".to_string(),
            });
        }

        let (decoded, last_decoded_at) = page_tracker
            .lock()
            .map(|tracker| (tracker.counts(), tracker.last_decoded_at))
            .unwrap_or_default();
        let missing = missing_capture_pages(&required, &decoded);
        if missing.is_empty() {
            return None;
        }

        if started.elapsed() >= CAPTURE_DRAIN_TIMEOUT {
            return Some(RuntimeError {
                code: "capture_incomplete".to_string(),
                message: format!(
                    "capture drain timed out: required={required:?} decoded={decoded:?}"
                ),
            });
        }

        if let Ok(mut status) = runtime.status.lock() {
            if status.state != "stopping" {
                status.state = "running".to_string();
            }
            status.auto_page = Some(json!({
                "state": "draining",
                "message": "waiting for capture drain",
                "kind": "draining",
                "required_pages_by_pool": required.clone(),
                "decoded_pages_by_pool": decoded.clone(),
                "missing_pages_by_pool": missing.clone(),
                "last_decoded_at": last_decoded_at,
            }));
            status.updated_at = now_seconds();
        }
        std::thread::sleep(CAPTURE_DRAIN_POLL_INTERVAL);
    }
}

fn missing_capture_pages(
    required: &BTreeMap<String, u32>,
    decoded: &BTreeMap<String, usize>,
) -> BTreeMap<String, u32> {
    required
        .iter()
        .filter_map(|(pool, required_count)| {
            let decoded_count = decoded.get(pool).copied().unwrap_or_default() as u32;
            (decoded_count < *required_count)
                .then(|| (pool.clone(), required_count - decoded_count))
        })
        .collect()
}

fn capture_progress_callback(
    runtime: Arc<CaptureRuntimeSession>,
    locale: String,
    snapshots: Option<Arc<Mutex<Vec<AutomationRecordSnapshot>>>>,
    page_tracker: Option<Arc<Mutex<CapturePageTracker>>>,
) -> Arc<dyn Fn(nte_capture::CaptureProgress) + Send + Sync + 'static> {
    let progress_state = Arc::new(Mutex::new(LiveProgressState::new(&locale)));
    Arc::new(move |progress: nte_capture::CaptureProgress| {
        if let Some(page_tracker) = &page_tracker {
            if let Ok(mut tracker) = page_tracker.lock() {
                tracker.add_progress(&progress);
            }
        }
        let (records_count, latest, snapshot_delta) = progress_state
            .lock()
            .map(|mut state| {
                let snapshot_delta = state.apply(&progress);
                let records_count = if state.records.is_empty() {
                    progress.row_count as u64
                } else {
                    state.records.len() as u64
                };
                (
                    records_count,
                    latest_records(&state.records),
                    snapshot_delta,
                )
            })
            .unwrap_or_else(|_| (progress.row_count as u64, Vec::new(), None));
        if let Some(snapshot_delta) = snapshot_delta {
            if let Some(snapshots) = &snapshots {
                if let Ok(mut snapshot_records) = snapshots.lock() {
                    snapshot_records.extend(snapshot_delta);
                }
            }
        }
        if let Ok(mut status) = runtime.status.lock() {
            status.state = if status.state == "stopping" {
                "stopping".to_string()
            } else {
                "running".to_string()
            };
            status.records_count = records_count;
            status.latest_records = latest;
            status.counters = CaptureCounters::from(progress.counters);
            status.target = serde_json::to_value(progress.target).ok();
            status.updated_at = now_seconds();
        }
    })
}

struct LiveProgressState {
    builder: Option<CaptureRecordBuilder>,
    records: Vec<Value>,
}

impl LiveProgressState {
    fn new(locale: &str) -> Self {
        Self {
            builder: CaptureRecordBuilder::new(locale).ok(),
            records: Vec::new(),
        }
    }

    fn apply(
        &mut self,
        progress: &nte_capture::CaptureProgress,
    ) -> Option<Vec<AutomationRecordSnapshot>> {
        let builder = self.builder.as_mut()?;
        let delta = progress
            .new_rows
            .iter()
            .map(|row| builder.build_record(row))
            .collect::<Vec<_>>();
        if delta.is_empty() {
            return None;
        }
        self.records
            .extend(delta.iter().map(|record| record.value.clone()));
        Some(delta.iter().filter_map(automation_snapshot).collect())
    }
}

fn finish_capture_result(
    runtime: &Arc<CaptureRuntimeSession>,
    result: Result<nte_capture::CaptureResult, String>,
    locale: &str,
    source_kind: &str,
    auto_page: Option<Value>,
    auto_error: Option<RuntimeError>,
) {
    let mut final_status = runtime.status.lock().expect("capture status lock");
    let now = now_seconds();
    match result {
        Ok(result) => match build_capture_document(&result.rows, locale) {
            Ok(document) => {
                let latest = latest_records_from_capture_document(&document);
                final_status.records_count = result.rows.len() as u64;
                final_status.latest_records = latest;
                final_status.counters = CaptureCounters::from(result.counters);
                final_status.target = serde_json::to_value(result.target).ok();
                final_status.state = if auto_error.is_some() {
                    "failed".to_string()
                } else {
                    "completed".to_string()
                };
                final_status.auto_page = auto_page;
                final_status.error = auto_error;
                final_status.document = Some(document);
                final_status.import_report = None;
            }
            Err(error) => {
                final_status.state = "failed".to_string();
                final_status.error = Some(RuntimeError {
                    code: "capture_document_failed".to_string(),
                    message: error.to_string(),
                });
            }
        },
        Err(message) => {
            final_status.state = "failed".to_string();
            final_status.error = Some(RuntimeError {
                code: source_kind.to_string(),
                message,
            });
            final_status.auto_page = auto_page;
        }
    }
    final_status.updated_at = now;
}

fn automation_snapshot(
    record: &nte_capture::CapturePublicRecord,
) -> Option<AutomationRecordSnapshot> {
    Some(AutomationRecordSnapshot {
        record_id: record.record_id.clone(),
        record_type: record.record_type.clone(),
        pool_id: record.pool_id.clone()?,
    })
}

fn latest_records(records: &[Value]) -> Vec<Value> {
    records.iter().rev().take(10).cloned().collect::<Vec<_>>()
}

fn latest_records_from_capture_document(document: &Value) -> Vec<Value> {
    document
        .get("nte")
        .and_then(|value| value.get("list"))
        .and_then(Value::as_array)
        .map(|records| latest_records(records))
        .unwrap_or_default()
}

fn capture_pool(record_type: &str, pool_id: Option<&str>) -> Option<&'static str> {
    match record_type {
        "monopoly" if pool_id == Some("CardPool_Weapon") => Some("weapon"),
        "monopoly" => Some("character"),
        "fork" => Some("fork"),
        _ => None,
    }
}

fn auto_page_status_value(status: &AutomationStatus, state: &str) -> Value {
    json!({
        "state": state,
        "message": status.message,
        "kind": status.kind,
        "step": status.step,
        "pool": status.pool,
        "current_page": status.current_page,
        "total_pages": status.total_pages,
        "technical_detail": status.technical_detail,
        "elapsed_seconds": status.elapsed_seconds,
    })
}

fn auto_page_result_value(result: &AutoPageRunResult) -> Value {
    json!({
        "state": if result.succeeded() { "completed" } else { "failed" },
        "message": result.message,
        "completed_pools": result.completed_pools,
        "skipped_pools": result.skipped_pools,
        "visited_pages_by_pool": result.visited_pages_by_pool,
        "last_page_by_pool": result.last_page_by_pool,
    })
}

fn capture_status_with_merge(
    state: &State<'_, AppState>,
    session_id: &str,
) -> Result<CaptureStatus, ApiError> {
    let session = capture_runtime_session(state, session_id)?;
    let status = session
        .status
        .lock()
        .map_err(|_| api_error_message("capture_lock_poisoned", "capture lock poisoned"))?
        .clone();
    if status.state != "completed" {
        cleanup_terminal_capture_session(state, session_id, &session, &status)?;
        return Ok(status);
    }
    let mut status = status;
    {
        let mut captures = state
            .captures
            .lock()
            .map_err(|_| api_error_message("capture_lock_poisoned", "capture lock poisoned"))?;
        if let Some(meta) = captures.get_mut(session_id) {
            merge_completed_capture(state, &mut status, meta)?;
        }
    }
    cleanup_terminal_capture_session(state, session_id, &session, &status)?;
    Ok(status)
}

fn capture_runtime_session(
    state: &State<'_, AppState>,
    session_id: &str,
) -> Result<Arc<CaptureRuntimeSession>, ApiError> {
    state
        .capture_sessions
        .lock()
        .map_err(|_| api_error_message("capture_lock_poisoned", "capture lock poisoned"))?
        .get(session_id)
        .cloned()
        .ok_or_else(|| api_error_message("capture_not_found", "capture session not found"))
}

fn merge_completed_capture(
    state: &State<'_, AppState>,
    status: &mut CaptureStatus,
    meta: &mut CaptureSessionMeta,
) -> Result<(), ApiError> {
    if let Some(report) = &meta.import_report {
        status.import_report = Some(report.clone());
        return Ok(());
    }
    let Some(document) = status.document.as_ref() else {
        return Ok(());
    };
    let document_text = serde_json::to_string(document).map_err(api_error)?;
    let report = if meta.full_update {
        with_store(state, |store| {
            store.import_public_document_with_backup(
                &meta.profile_name,
                &document_text,
                &meta.source_kind,
                meta.source_path.as_deref(),
            )
        })?
    } else {
        with_store(state, |store| {
            store.import_public_document(
                &meta.profile_name,
                &document_text,
                &meta.source_kind,
                meta.source_path.as_deref(),
            )
        })?
    };
    meta.import_report = Some(report.clone());
    status.import_report = Some(report);
    Ok(())
}

fn cleanup_terminal_capture_session(
    state: &State<'_, AppState>,
    session_id: &str,
    session: &CaptureRuntimeSession,
    status: &CaptureStatus,
) -> Result<(), ApiError> {
    if !capture_status_is_terminal(status) {
        return Ok(());
    }
    let _ = try_join_finished_capture_thread(session);
    let mut sessions = state
        .capture_sessions
        .lock()
        .map_err(|_| api_error_message("capture_lock_poisoned", "capture lock poisoned"))?;
    let mut captures = state
        .captures
        .lock()
        .map_err(|_| api_error_message("capture_lock_poisoned", "capture lock poisoned"))?;
    prune_capture_session_maps(&mut sessions, &mut captures, session_id, now_seconds());
    Ok(())
}

fn prune_capture_session_maps(
    sessions: &mut HashMap<String, Arc<CaptureRuntimeSession>>,
    captures: &mut HashMap<String, CaptureSessionMeta>,
    preserve_session_id: &str,
    now: f64,
) {
    for session in sessions.values() {
        let is_terminal = session
            .status
            .lock()
            .map(|status| capture_status_is_terminal(&status))
            .unwrap_or(false);
        if is_terminal {
            let _ = try_join_finished_capture_thread(session);
        }
    }

    let removable = sessions
        .iter()
        .filter_map(|(session_id, session)| {
            if session_id == preserve_session_id || !capture_handle_joined(session) {
                return None;
            }
            let status = session.status.lock().ok()?;
            capture_status_is_terminal(&status).then(|| (session_id.clone(), status.updated_at))
        })
        .collect::<Vec<_>>();

    let mut to_remove = BTreeSet::new();
    for (session_id, updated_at) in &removable {
        if now - *updated_at >= CAPTURE_SESSION_RETENTION_SECONDS {
            to_remove.insert(session_id.clone());
        }
    }

    let recent = removable
        .into_iter()
        .filter(|(session_id, _)| !to_remove.contains(session_id))
        .collect::<Vec<_>>();
    if recent.len() > CAPTURE_TERMINAL_SESSION_LIMIT {
        let extra = recent.len() - CAPTURE_TERMINAL_SESSION_LIMIT;
        let mut recent = recent;
        recent.sort_by(|left, right| {
            left.1
                .total_cmp(&right.1)
                .then_with(|| left.0.cmp(&right.0))
        });
        for (session_id, _) in recent.into_iter().take(extra) {
            to_remove.insert(session_id);
        }
    }

    for session_id in to_remove {
        sessions.remove(&session_id);
        captures.remove(&session_id);
    }
}

fn capture_status_is_terminal(status: &CaptureStatus) -> bool {
    matches!(status.state.as_str(), "completed" | "failed")
}

fn capture_handle_joined(session: &CaptureRuntimeSession) -> bool {
    session
        .handle
        .lock()
        .map(|handle| handle.is_none())
        .unwrap_or(false)
}

fn try_join_finished_capture_thread(session: &CaptureRuntimeSession) -> bool {
    let handle = session.handle.lock().ok().and_then(|mut guard| {
        guard
            .as_ref()
            .is_some_and(std::thread::JoinHandle::is_finished)
            .then(|| guard.take())
            .flatten()
    });
    let Some(handle) = handle else {
        return false;
    };
    handle.join().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_status(session_id: &str, state: &str, updated_at: f64) -> CaptureStatus {
        CaptureStatus {
            session_id: session_id.to_string(),
            state: state.to_string(),
            mode: "live_only".to_string(),
            records_count: 0,
            latest_records: Vec::new(),
            counters: CaptureCounters::default(),
            started_at: updated_at,
            updated_at,
            target: None,
            auto_page: None,
            raw_path: None,
            error: None,
            document: None,
            import_report: None,
        }
    }

    fn test_session(status: CaptureStatus) -> Arc<CaptureRuntimeSession> {
        Arc::new(CaptureRuntimeSession {
            status: Mutex::new(status),
            stop: Arc::new(AtomicBool::new(false)),
            handle: Mutex::new(None),
        })
    }

    fn test_meta() -> CaptureSessionMeta {
        CaptureSessionMeta {
            profile_name: "default".to_string(),
            source_kind: "test".to_string(),
            source_path: None,
            full_update: false,
            import_report: None,
        }
    }

    #[test]
    fn latest_records_read_capture_document_nte_list() {
        let records = (0..12)
            .map(|index| json!({ "record_id": format!("r{index}") }))
            .collect::<Vec<_>>();
        let document = json!({ "nte": { "list": records } });

        let latest = latest_records_from_capture_document(&document);

        assert_eq!(latest.len(), 10);
        assert_eq!(latest[0]["record_id"], "r11");
        assert_eq!(latest[9]["record_id"], "r2");
    }

    #[test]
    fn latest_records_missing_capture_document_list_returns_empty() {
        assert!(latest_records_from_capture_document(&json!({ "records": [] })).is_empty());
    }

    #[test]
    fn prune_capture_session_maps_keeps_active_and_preserved_sessions() {
        let mut sessions = HashMap::from([
            (
                "active".to_string(),
                test_session(test_status("active", "running", 1.0)),
            ),
            (
                "preserve".to_string(),
                test_session(test_status("preserve", "completed", 1.0)),
            ),
            (
                "old".to_string(),
                test_session(test_status("old", "failed", 1.0)),
            ),
        ]);
        let mut captures = HashMap::from([
            ("active".to_string(), test_meta()),
            ("preserve".to_string(), test_meta()),
            ("old".to_string(), test_meta()),
        ]);

        prune_capture_session_maps(&mut sessions, &mut captures, "preserve", 2_000.0);

        assert!(sessions.contains_key("active"));
        assert!(sessions.contains_key("preserve"));
        assert!(!sessions.contains_key("old"));
        assert!(!captures.contains_key("old"));
    }

    #[test]
    fn prune_capture_session_maps_retains_latest_terminal_limit() {
        let mut sessions = HashMap::new();
        let mut captures = HashMap::new();
        for index in 0..25 {
            let session_id = format!("s{index:02}");
            sessions.insert(
                session_id.clone(),
                test_session(test_status(&session_id, "completed", f64::from(index))),
            );
            captures.insert(session_id, test_meta());
        }

        prune_capture_session_maps(&mut sessions, &mut captures, "s24", 100.0);

        assert_eq!(sessions.len(), 21);
        assert!(sessions.contains_key("s24"));
        assert!(!sessions.contains_key("s00"));
        assert!(!sessions.contains_key("s03"));
        assert!(sessions.contains_key("s04"));
    }
}
