use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use nte_automation::{
    AUTO_PAGE_INCREMENTAL_DUPLICATE_RECORD_THRESHOLD, AutoPageControlContext,
    AutoPageControlDecision, AutoPageDiagnostics, AutoPageOptions as AutomationOptions,
    AutoPageResult as AutoPageRunResult, AutoPageStatus as AutomationStatus,
    RecordSnapshot as AutomationRecordSnapshot, run_auto_page,
};
use nte_capture::{
    CaptureAttemptSummary, CaptureBackend, CaptureOptions, CaptureRecordBuilder, CaptureStrategy,
    CaptureStrategyKind, CaptureStrategyReason, CaptureTarget, build_capture_document,
    candidate_ports, capture_live, detect_pppoe, find_process_pid,
};
use nte_core::{ImportReport, SettingsPatch};
use nte_store::load_locale_or_settings;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{AppHandle, Manager, State, Wry};

use crate::admin::admin_relaunch_required;
use crate::error::{ApiError, RuntimeError, api_error, api_error_message};
use crate::state::{AppState, new_session_id, now_seconds, portable_root, with_store};
use crate::window_commands::wake_main_window;

const CAPTURE_DRAIN_TIMEOUT: Duration = Duration::from_secs(20);
const CAPTURE_DRAIN_POLL_INTERVAL: Duration = Duration::from_millis(100);
const CAPTURE_SESSION_RETENTION_SECONDS: f64 = 30.0 * 60.0;
const CAPTURE_TERMINAL_SESSION_LIMIT: usize = 20;
const AUTO_PAGE_CAPTURE_WINDOW_PAGES: u32 = 6;
const AUTO_PAGE_PAGE_RECORD_MIN_WAIT_MS: u64 = 300;
const AUTO_PAGE_MAX_PAGE_RECORD_MIN_WAIT_MS: u64 = 1500;
const AUTO_PAGE_CAPTURE_WINDOW_STALLED_CODE: &str = "auto_page_capture_window_stalled";
const AUTO_PAGE_FAILED_CODE: &str = "auto_page_failed";

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
    attempt_stop: Mutex<Option<Arc<AtomicBool>>>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CaptureMode {
    LiveOnly,
    AutoPageIncremental,
    AutoPageFull,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct CaptureStartOptions {
    #[serde(default)]
    page_record_min_wait_ms: Option<u64>,
    #[serde(default)]
    capture_backend: Option<CaptureBackendOverride>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) enum CaptureBackendOverride {
    #[serde(rename = "pktmon")]
    Pktmon,
    #[serde(rename = "windivert")]
    WinDivert,
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
    material_key: String,
}

#[derive(Debug, Default)]
struct CapturePageTracker {
    pages_by_pool: BTreeMap<String, BTreeSet<CapturePageKey>>,
    last_decoded_at: Option<f64>,
}

impl CapturePageTracker {
    fn add_progress(&mut self, progress: &nte_capture::CaptureProgress) {
        if !progress.rows_snapshot.is_empty() || progress.row_count == 0 {
            self.replace_rows(&progress.rows_snapshot);
            return;
        }
        self.add_rows(&progress.new_rows);
    }

    fn replace_rows(&mut self, rows: &[nte_capture::ParsedRow]) {
        self.pages_by_pool.clear();
        self.add_rows(rows);
    }

    fn add_rows(&mut self, rows: &[nte_capture::ParsedRow]) {
        let mut changed = false;
        for row in rows {
            let Some((pool, key)) = capture_page_key(row) else {
                continue;
            };
            changed |= self
                .pages_by_pool
                .entry(pool)
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

struct AutoPageCoordinator {
    state: Mutex<AutoPageCoordinatorState>,
    changed: Condvar,
    full_update: bool,
    known_counts: BTreeMap<String, u64>,
    max_window_pages: u32,
}

#[derive(Debug, Default)]
struct AutoPageCoordinatorState {
    page_tracker: CapturePageTracker,
    records_by_pool: BTreeMap<String, Vec<AutomationRecordSnapshot>>,
    duplicate_counts_by_pool: BTreeMap<String, usize>,
    skipped_pools: BTreeSet<String>,
}

impl AutoPageCoordinator {
    fn new(full_update: bool, known_record_keys: &[String]) -> Self {
        Self {
            state: Mutex::new(AutoPageCoordinatorState::default()),
            changed: Condvar::new(),
            full_update,
            known_counts: record_key_counts(known_record_keys),
            max_window_pages: AUTO_PAGE_CAPTURE_WINDOW_PAGES,
        }
    }

    fn add_progress(
        &self,
        progress: &nte_capture::CaptureProgress,
        records_snapshot: Option<&[AutomationRecordSnapshot]>,
    ) {
        if let Ok(mut state) = self.state.lock() {
            state.page_tracker.add_progress(progress);
            if let Some(records) = records_snapshot {
                state.records_by_pool = records_by_pool(records);
                state.duplicate_counts_by_pool = state
                    .records_by_pool
                    .iter()
                    .map(|(pool, records)| {
                        (
                            pool.clone(),
                            consecutive_known_record_count(records, &self.known_counts),
                        )
                    })
                    .collect();
            }
            self.changed.notify_all();
        }
    }

    fn decision(&self, context: AutoPageControlContext) -> AutoPageControlDecision {
        let Ok(mut state) = self.state.lock() else {
            return AutoPageControlDecision::Continue;
        };
        let decision = self.decide_locked(&mut state, &context);
        if !matches!(decision, AutoPageControlDecision::WaitCapture { .. }) {
            return decision;
        }
        let Ok((mut state, _)) = self.changed.wait_timeout(state, Duration::from_millis(50)) else {
            return decision;
        };
        self.decide_locked(&mut state, &context)
    }

    fn decide_locked(
        &self,
        state: &mut AutoPageCoordinatorState,
        context: &AutoPageControlContext,
    ) -> AutoPageControlDecision {
        let duplicate_records = state
            .duplicate_counts_by_pool
            .get(&context.pool)
            .copied()
            .unwrap_or_default();
        if !self.full_update
            && duplicate_records >= AUTO_PAGE_INCREMENTAL_DUPLICATE_RECORD_THRESHOLD
        {
            state.skipped_pools.insert(context.pool.clone());
            return AutoPageControlDecision::SkipPool { duplicate_records };
        }

        let decoded_pages = state.page_tracker.count(&context.pool);
        let max_visited_pages = decoded_pages as u32 + self.max_window_pages;
        if context.visited_pages > max_visited_pages {
            return AutoPageControlDecision::WaitCapture {
                decoded_pages,
                max_visited_pages,
            };
        }
        AutoPageControlDecision::Continue
    }

    fn counts(&self) -> BTreeMap<String, usize> {
        self.state
            .lock()
            .map(|state| state.page_tracker.counts())
            .unwrap_or_default()
    }

    fn last_decoded_at(&self) -> Option<f64> {
        self.state
            .lock()
            .map(|state| state.page_tracker.last_decoded_at)
            .unwrap_or_default()
    }
}

fn capture_page_key(row: &nte_capture::ParsedRow) -> Option<(String, CapturePageKey)> {
    let pool = capture_pool(row.record_type.as_str(), row.pool_id.as_deref())?;
    let stream_key = row.source.stream_key.clone().unwrap_or_else(|| {
        format!(
            "{}:{}",
            row.record_type.as_str(),
            row.pool_id.as_deref().unwrap_or_default()
        )
    });
    let material_key = if let Some(index) = row.source.segment_index {
        format!("segment:{index}")
    } else if let Some(index) = row.source.page_index {
        format!("page:{index}")
    } else {
        format!("packet:{}:{}", row.source.packet_index, row.source.view)
    };
    Some((
        pool.to_string(),
        CapturePageKey {
            stream_key,
            generation_index: row.source.generation_index.unwrap_or_default(),
            material_key,
        },
    ))
}

fn records_by_pool(
    records: &[AutomationRecordSnapshot],
) -> BTreeMap<String, Vec<AutomationRecordSnapshot>> {
    let mut by_pool = BTreeMap::<String, Vec<AutomationRecordSnapshot>>::new();
    for record in records {
        if let Some(pool) = capture_pool(record.record_type.as_str(), Some(record.pool_id.as_str()))
        {
            by_pool
                .entry(pool.to_string())
                .or_default()
                .push(record.clone());
        }
    }
    by_pool
}

fn consecutive_known_record_count(
    records: &[AutomationRecordSnapshot],
    known_counts: &BTreeMap<String, u64>,
) -> usize {
    let mut remaining = known_counts.clone();
    records
        .iter()
        .rev()
        .take_while(|record| {
            let Some(count) = remaining.get_mut(record.record_key.as_str()) else {
                return false;
            };
            if *count == 0 {
                return false;
            }
            *count -= 1;
            true
        })
        .count()
}

fn record_key_counts(keys: &[String]) -> BTreeMap<String, u64> {
    let mut counts = BTreeMap::new();
    for key in keys {
        *counts.entry(key.clone()).or_default() += 1;
    }
    counts
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CaptureStatus {
    session_id: String,
    state: String,
    mode: String,
    records_count: u64,
    latest_records: Vec<Value>,
    counters: CaptureCounters,
    #[serde(default)]
    attempts: Vec<CaptureAttemptSummary>,
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

fn runtime_error(code: impl Into<String>, message: impl Into<String>) -> RuntimeError {
    RuntimeError {
        code: code.into(),
        message: message.into(),
        support_path: None,
        support_image_path: None,
    }
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
