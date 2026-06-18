use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use nte_automation::{
    run_auto_page, AutoPageOptions as AutomationOptions, AutoPageResult as AutoPageRunResult,
    AutoPageStatus as AutomationStatus, RecordSnapshot as AutomationRecordSnapshot,
};
use nte_gui_core::{
    available_locales, build_capture_document, candidate_ports, capture_doctor, capture_live,
    check_update_manifest, find_process_pid, load_locale_or_settings, prepare_update_install,
    read_raw_capture, stage_update_archive, update_status, BackupReport, CaptureOptions,
    CaptureRecordBuilder, CaptureTarget, DashboardOverview, GuiError, ImportReport, JsonStore,
    MapLocaleList, PoolKind, PoolKindDetail, Profile, RecordFilter, RecordFilterOptions,
    RecordList, RestoreReport, Settings, SettingsPatch, UpdateChannel, UpdateCheckReport,
    UpdateManifest, UpdatePackage, UpdateStageReport, UpdateStatus,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{Manager, State};

struct AppState {
    store: Mutex<JsonStore>,
    capture_sessions: Mutex<HashMap<String, Arc<CaptureRuntimeSession>>>,
    captures: Mutex<HashMap<String, CaptureSessionMeta>>,
    pending_admin_capture: Mutex<Option<PendingAdminCapture>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RuntimeError {
    code: String,
    message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApiError {
    code: String,
    message: String,
}

#[derive(Debug, Clone)]
struct CaptureSessionMeta {
    profile_name: String,
    source_kind: String,
    source_path: Option<String>,
    full_update: bool,
    import_report: Option<ImportReport>,
}

struct CaptureRuntimeSession {
    status: Mutex<CaptureStatus>,
    stop: Arc<AtomicBool>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingAdminCapture {
    profile_name: String,
    locale: String,
    mode: CaptureMode,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum CaptureMode {
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

#[derive(Debug, Serialize, Deserialize)]
struct DoctorReport {
    ok: bool,
    exit_code: i64,
    lines: Vec<String>,
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
    fn add_progress(&mut self, progress: &nte_gui_core::CaptureProgress) {
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
struct CaptureStatus {
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

const GITHUB_RELEASES_API: &str =
    "https://api.github.com/repos/Anong0u0/nte_gacha_exporter/releases";
const UPDATE_MANIFEST_ASSET: &str = "nte-gacha-update.json";
const USER_AGENT: &str = "nte-gacha-exporter-updater";
const CAPTURE_DRAIN_TIMEOUT: Duration = Duration::from_secs(20);
const CAPTURE_DRAIN_POLL_INTERVAL: Duration = Duration::from_millis(100);

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Result<Settings, ApiError> {
    with_store(&state, |store| store.settings())
}

#[tauri::command]
fn update_settings(state: State<'_, AppState>, patch: SettingsPatch) -> Result<Settings, ApiError> {
    with_store(&state, |store| store.update_settings(patch))
}

#[tauri::command]
fn list_profiles(state: State<'_, AppState>) -> Result<Vec<Profile>, ApiError> {
    with_store(&state, |store| store.list_profiles())
}

#[tauri::command]
fn create_profile(state: State<'_, AppState>, name: String) -> Result<Profile, ApiError> {
    with_store(&state, |store| store.create_profile(&name))
}

#[tauri::command]
fn set_active_profile(
    state: State<'_, AppState>,
    profile_name: String,
) -> Result<Settings, ApiError> {
    with_store(&state, |store| store.set_active_profile(&profile_name))
}

#[tauri::command]
fn import_public_json(
    state: State<'_, AppState>,
    profile_name: String,
    path: String,
) -> Result<ImportReport, ApiError> {
    let text = fs::read_to_string(&path).map_err(api_error)?;
    with_store(&state, |store| {
        store.import_public_document(&profile_name, &text, "public_json", Some(&path))
    })
}

#[tauri::command]
fn import_raw_jsonl(
    state: State<'_, AppState>,
    profile_name: String,
    path: String,
    locale: Option<String>,
) -> Result<ImportReport, ApiError> {
    let locale = with_store(&state, |store| load_locale_or_settings(store, locale))?;
    let rows = read_raw_capture(Path::new(&path)).map_err(api_error)?;
    let document = build_capture_document(&rows.rows, &rows.warnings, &locale, "raw-replay")
        .map_err(api_error)?;
    let document_text = serde_json::to_string(&document).map_err(api_error)?;
    with_store(&state, |store| {
        store.import_public_document(&profile_name, &document_text, "raw_jsonl", Some(&path))
    })
}

#[tauri::command]
fn dashboard_overview(
    state: State<'_, AppState>,
    profile_name: String,
    locale: Option<String>,
) -> Result<DashboardOverview, ApiError> {
    with_store(&state, |store| {
        let locale = load_locale_or_settings(store, locale)?;
        store.dashboard_overview(&profile_name, &locale)
    })
}

#[tauri::command]
fn pool_kind_detail(
    state: State<'_, AppState>,
    profile_name: String,
    pool_kind: PoolKind,
    locale: Option<String>,
) -> Result<PoolKindDetail, ApiError> {
    with_store(&state, |store| {
        let locale = load_locale_or_settings(store, locale)?;
        store.pool_kind_detail(&profile_name, &locale, pool_kind)
    })
}

#[tauri::command]
fn list_records(
    state: State<'_, AppState>,
    profile_name: String,
    filter: RecordFilter,
    locale: Option<String>,
) -> Result<RecordList, ApiError> {
    with_store(&state, |store| {
        let locale = load_locale_or_settings(store, locale)?;
        store.list_records(&profile_name, &locale, &filter)
    })
}

#[tauri::command]
fn record_filter_options(
    state: State<'_, AppState>,
    profile_name: String,
    locale: Option<String>,
) -> Result<RecordFilterOptions, ApiError> {
    with_store(&state, |store| {
        let locale = load_locale_or_settings(store, locale)?;
        store.record_filter_options(&profile_name, &locale)
    })
}

#[tauri::command]
fn export_public_json(
    state: State<'_, AppState>,
    profile_name: String,
    path: String,
    locale: Option<String>,
) -> Result<(), ApiError> {
    with_store(&state, |store| {
        let locale = load_locale_or_settings(store, locale)?;
        store.export_public_json(&profile_name, &locale, path)
    })
}

#[tauri::command]
fn export_csv(
    state: State<'_, AppState>,
    profile_name: String,
    path: String,
    locale: Option<String>,
) -> Result<(), ApiError> {
    with_store(&state, |store| {
        let locale = load_locale_or_settings(store, locale)?;
        store.export_csv(&profile_name, &locale, path)
    })
}

#[tauri::command]
fn create_backup(
    state: State<'_, AppState>,
    path: Option<String>,
) -> Result<BackupReport, ApiError> {
    with_store(&state, |store| {
        store.create_data_backup_report(path.as_deref())
    })
}

#[tauri::command]
fn restore_backup(state: State<'_, AppState>, path: String) -> Result<RestoreReport, ApiError> {
    with_store(&state, |store| store.restore_data_backup_report(path))
}

#[tauri::command]
fn updater_status(state: State<'_, AppState>) -> Result<UpdateStatus, ApiError> {
    with_store(&state, |store| update_status(store.root(), app_version()))
}

#[tauri::command]
fn updater_check(
    state: State<'_, AppState>,
    channel: Option<String>,
) -> Result<UpdateCheckReport, ApiError> {
    let requested_channel = update_channel_or_settings(&state, channel)?;
    let manifest = fetch_update_manifest(requested_channel)?;
    check_update_manifest(manifest, app_version(), requested_channel).map_err(api_error)
}

#[tauri::command]
fn updater_download_and_stage(
    state: State<'_, AppState>,
    package: UpdatePackage,
) -> Result<UpdateStageReport, ApiError> {
    let root = with_store(&state, |store| Ok(store.root().to_path_buf()))?;
    let archive_path = download_update_archive(&root, &package)?;
    stage_update_archive(&root, &package, archive_path).map_err(api_error)
}

#[tauri::command]
fn updater_install_staged(
    state: State<'_, AppState>,
    version: String,
    relaunch: Option<bool>,
) -> Result<(), ApiError> {
    let root = with_store(&state, |store| Ok(store.root().to_path_buf()))?;
    let plan = prepare_update_install(&root, &version).map_err(api_error)?;
    Command::new(&plan.helper_path)
        .arg("--root")
        .arg(&plan.root)
        .arg("--version")
        .arg(&plan.version)
        .arg("--app-pid")
        .arg(std::process::id().to_string())
        .args(relaunch.unwrap_or(true).then_some("--relaunch"))
        .current_dir(&plan.root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(api_error)?;
    std::process::exit(0);
}

#[tauri::command]
fn maps_list() -> MapLocaleList {
    MapLocaleList {
        locales: available_locales(),
    }
}

#[tauri::command]
fn doctor_run(_state: State<'_, AppState>) -> Result<DoctorReport, ApiError> {
    let report = capture_doctor("HTGame.exe").map_err(api_error)?;
    let mut lines = Vec::new();
    lines.push(format!("Windows: {}", report.windows));
    lines.push(format!("Administrator: {}", report.admin));
    lines.push(format!(
        "HTGame.exe: {}",
        report
            .pid
            .map(|pid| format!("pid {pid}"))
            .unwrap_or_else(|| "not found".to_string())
    ));
    lines.push(format!("Ports: {:?}", report.ports));
    lines.extend(report.notes);
    Ok(DoctorReport {
        ok: report.windows && report.admin && report.pid.is_some() && !report.ports.is_empty(),
        exit_code: if report.windows && report.admin { 0 } else { 3 },
        lines,
    })
}

#[tauri::command]
fn sidecar_ping(_state: State<'_, AppState>) -> Result<Value, ApiError> {
    Ok(json!({ "ok": true, "runtime": "rust" }))
}

#[tauri::command]
fn request_admin_capture_start(
    state: State<'_, AppState>,
    profile_name: String,
    locale: Option<String>,
    mode: Option<CaptureMode>,
) -> Result<bool, ApiError> {
    let mode = mode.unwrap_or(CaptureMode::LiveOnly);
    if !admin_relaunch_required()? {
        return Ok(false);
    }
    let locale = with_store(&state, |store| {
        let locale = load_locale_or_settings(store, locale)?;
        store.dashboard_overview(&profile_name, &locale)?;
        Ok(locale)
    })?;
    let payload = PendingAdminCapture {
        profile_name,
        locale,
        mode,
    };
    let path = write_admin_capture_payload(&payload)?;
    relaunch_admin_with_capture_payload(&path)?;
    schedule_process_exit();
    Ok(true)
}

#[tauri::command]
fn take_pending_admin_capture(
    state: State<'_, AppState>,
) -> Result<Option<PendingAdminCapture>, ApiError> {
    state
        .pending_admin_capture
        .lock()
        .map_err(|_| {
            api_error_message("admin_capture_lock_poisoned", "admin capture lock poisoned")
        })
        .map(|mut pending| pending.take())
}

#[tauri::command]
fn capture_start(
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
fn capture_status(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<CaptureStatus, ApiError> {
    capture_status_with_merge(&state, &session_id)
}

#[tauri::command]
fn capture_stop(state: State<'_, AppState>, session_id: String) -> Result<CaptureStatus, ApiError> {
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

pub fn run() {
    let pending_admin_capture = pending_admin_capture_from_args()
        .unwrap_or_else(|error| panic!("failed to read pending admin capture: {error:?}"));
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let root =
                portable_root().map_err(|err| format!("failed to resolve portable root: {err}"))?;
            let store =
                JsonStore::open(root).map_err(|err| format!("failed to open JSON store: {err}"))?;
            app.manage(AppState {
                store: Mutex::new(store),
                capture_sessions: Mutex::new(HashMap::new()),
                captures: Mutex::new(HashMap::new()),
                pending_admin_capture: Mutex::new(pending_admin_capture.clone()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            update_settings,
            list_profiles,
            create_profile,
            set_active_profile,
            import_public_json,
            import_raw_jsonl,
            dashboard_overview,
            pool_kind_detail,
            list_records,
            record_filter_options,
            export_public_json,
            export_csv,
            create_backup,
            restore_backup,
            updater_status,
            updater_check,
            updater_download_and_stage,
            updater_install_staged,
            maps_list,
            doctor_run,
            sidecar_ping,
            request_admin_capture_start,
            take_pending_admin_capture,
            capture_start,
            capture_status,
            capture_stop
        ])
        .run(tauri::generate_context!())
        .expect("error while running Tauri application");
}

fn with_store<T>(
    state: &State<'_, AppState>,
    f: impl FnOnce(&JsonStore) -> Result<T, GuiError>,
) -> Result<T, ApiError> {
    let store = state
        .store
        .lock()
        .map_err(|_| api_error_message("store_lock_poisoned", "store lock poisoned"))?;
    f(&store).map_err(api_error)
}

fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn update_channel_or_settings(
    state: &State<'_, AppState>,
    channel: Option<String>,
) -> Result<UpdateChannel, ApiError> {
    if let Some(channel) = channel.filter(|value| !value.trim().is_empty()) {
        return Ok(UpdateChannel::from_settings(&channel));
    }
    with_store(state, |store| {
        Ok(UpdateChannel::from_settings(
            &store.settings()?.update_channel,
        ))
    })
}

fn fetch_update_manifest(channel: UpdateChannel) -> Result<UpdateManifest, ApiError> {
    if let Ok(source) = std::env::var("NTE_GACHA_UPDATE_MANIFEST") {
        if !source.trim().is_empty() {
            if source.starts_with("http://") || source.starts_with("https://") {
                return http_get_json(&source);
            }
            let text = fs::read_to_string(source).map_err(api_error)?;
            return serde_json::from_str(&text).map_err(api_error);
        }
    }
    let release = select_release(channel)?;
    let manifest_url = release
        .assets
        .into_iter()
        .find(|asset| asset.name == UPDATE_MANIFEST_ASSET)
        .map(|asset| asset.browser_download_url)
        .ok_or_else(|| {
            api_error_message("update_manifest_missing", "release update manifest missing")
        })?;
    http_get_json(&manifest_url)
}

fn select_release(channel: UpdateChannel) -> Result<GithubRelease, ApiError> {
    let releases: Vec<GithubRelease> = http_get_json(GITHUB_RELEASES_API)?;
    releases
        .into_iter()
        .find(|release| !release.draft && (channel == UpdateChannel::Beta || !release.prerelease))
        .ok_or_else(|| {
            api_error_message("update_release_missing", "no matching GitHub release found")
        })
}

fn http_get_json<T: for<'de> Deserialize<'de>>(url: &str) -> Result<T, ApiError> {
    let response = ureq::get(url)
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(api_error)?;
    let mut text = String::new();
    response
        .into_reader()
        .read_to_string(&mut text)
        .map_err(api_error)?;
    serde_json::from_str(&text).map_err(api_error)
}

fn download_update_archive(root: &Path, package: &UpdatePackage) -> Result<PathBuf, ApiError> {
    let downloads = root.join("update").join("downloads").join(&package.version);
    fs::create_dir_all(&downloads).map_err(api_error)?;
    let path = downloads.join(&package.asset_name);
    let tmp_path = downloads.join(format!("{}.tmp", package.asset_name));
    let response = ureq::get(&package.download_url)
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(api_error)?;
    let mut reader = response.into_reader();
    let mut file = fs::File::create(&tmp_path).map_err(api_error)?;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer).map_err(api_error)?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read]).map_err(api_error)?;
    }
    file.flush().map_err(api_error)?;
    fs::rename(&tmp_path, &path).map_err(api_error)?;
    Ok(path)
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    draft: bool,
    prerelease: bool,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

fn portable_root() -> Result<PathBuf, std::io::Error> {
    if let Ok(root) = env::var("NTE_GACHA_PORTABLE_ROOT") {
        if !root.trim().is_empty() {
            return Ok(PathBuf::from(root));
        }
    }
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(PathBuf::from))
        .or_else(|| std::env::current_dir().ok())
        .ok_or_else(|| {
            std::io::Error::other("cannot resolve current executable or current directory")
        })
}

fn pending_admin_capture_from_args() -> Result<Option<PendingAdminCapture>, ApiError> {
    let mut args = env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--admin-capture-json" {
            let path = args.next().ok_or_else(|| {
                api_error_message(
                    "admin_capture_arg_missing",
                    "--admin-capture-json requires a path",
                )
            })?;
            let text = fs::read_to_string(path).map_err(api_error)?;
            return serde_json::from_str(&text).map(Some).map_err(api_error);
        }
    }
    Ok(None)
}

fn write_admin_capture_payload(payload: &PendingAdminCapture) -> Result<PathBuf, ApiError> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(api_error)?
        .as_millis();
    let path = env::temp_dir().join(format!(
        "nte-gacha-admin-capture-{}-{stamp}.json",
        std::process::id()
    ));
    let text = serde_json::to_string(payload).map_err(api_error)?;
    fs::write(&path, text).map_err(api_error)?;
    Ok(path)
}

fn schedule_process_exit() {
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_millis(750));
        std::process::exit(0);
    });
}

fn now_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs_f64())
        .unwrap_or_default()
}

fn new_session_id() -> String {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or_default();
    format!("rust-capture-{}-{stamp}", std::process::id())
}

impl From<nte_gui_core::CaptureCounters> for CaptureCounters {
    fn from(value: nte_gui_core::CaptureCounters) -> Self {
        Self {
            packets_seen: value.packets_seen,
            decoded_packets: value.decoded_packets,
            dropped_packets: value.dropped_packets,
            duplicate_packets: value.duplicate_packets,
            filter_restarts: value.filter_restarts,
        }
    }
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
            run_auto_page_capture_thread(
                runtime_for_thread,
                pid,
                ports,
                raw_out,
                locale_for_thread,
                stop_for_thread,
                callback,
                mode,
                known_record_ids,
                snapshots,
                page_tracker,
            );
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

#[allow(clippy::too_many_arguments)]
fn run_auto_page_capture_thread(
    runtime: Arc<CaptureRuntimeSession>,
    pid: u32,
    ports: Vec<u16>,
    raw_out: Option<PathBuf>,
    locale: String,
    stop: Arc<AtomicBool>,
    callback: Arc<dyn Fn(nte_gui_core::CaptureProgress) + Send + Sync + 'static>,
    mode: CaptureMode,
    known_record_ids: Vec<String>,
    snapshots: Arc<Mutex<Vec<AutomationRecordSnapshot>>>,
    page_tracker: Arc<Mutex<CapturePageTracker>>,
) {
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
) -> Arc<dyn Fn(nte_gui_core::CaptureProgress) + Send + Sync + 'static> {
    let progress_state = Arc::new(Mutex::new(LiveProgressState::new(&locale)));
    Arc::new(move |progress: nte_gui_core::CaptureProgress| {
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
        progress: &nte_gui_core::CaptureProgress,
    ) -> Option<Vec<AutomationRecordSnapshot>> {
        let builder = self.builder.as_mut()?;
        let records = builder.build_records(&progress.new_rows);
        if records.is_empty() {
            return None;
        }
        let mut snapshot_delta = Vec::new();
        for record in records {
            if let Some(pool_id) = record.pool_id.clone() {
                snapshot_delta.push(AutomationRecordSnapshot {
                    record_id: record.record_id,
                    pool_id,
                    record_type: record.record_type,
                });
            }
            self.records.push(record.value);
        }
        Some(snapshot_delta)
    }
}

fn finish_capture_result(
    runtime: &Arc<CaptureRuntimeSession>,
    result: Result<nte_gui_core::CaptureResult, String>,
    locale: &str,
    source_kind: &str,
    auto_page: Option<Value>,
    final_error: Option<RuntimeError>,
) {
    match result {
        Ok(result) => {
            let document =
                build_capture_document(&result.rows, &result.warnings, locale, source_kind);
            if let Ok(mut status) = runtime.status.lock() {
                status.auto_page = auto_page;
                match document {
                    Ok(document) => {
                        let records = document
                            .get("nte")
                            .and_then(|nte| nte.get("list"))
                            .and_then(Value::as_array)
                            .cloned()
                            .unwrap_or_default();
                        status.records_count = records.len() as u64;
                        status.latest_records = latest_records(&records);
                        status.counters = CaptureCounters::from(result.counters);
                        status.target = serde_json::to_value(result.target).ok();
                        status.document = Some(document);
                        status.error = final_error;
                        status.state = if status.error.is_some() {
                            "failed".to_string()
                        } else {
                            "completed".to_string()
                        };
                    }
                    Err(error) => {
                        status.error = Some(RuntimeError {
                            code: "capture_document_failed".to_string(),
                            message: error.to_string(),
                        });
                        status.state = "failed".to_string();
                    }
                }
                status.updated_at = now_seconds();
            }
        }
        Err(error) => {
            if let Ok(mut status) = runtime.status.lock() {
                status.auto_page = auto_page;
                status.error = Some(RuntimeError {
                    code: "capture_failed".to_string(),
                    message: error,
                });
                status.state = "failed".to_string();
                status.updated_at = now_seconds();
            }
        }
    }
}

fn latest_records(records: &[Value]) -> Vec<Value> {
    records
        .iter()
        .rev()
        .take(12)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn capture_pool(record_type: &str, pool_id: Option<&str>) -> Option<&'static str> {
    match pool_id {
        Some("CardPool_Character") => Some("limited"),
        Some("CardPool_NewRole") => Some("standard"),
        Some(pool_id) if pool_id.starts_with("ForkLottery_") => Some("fork"),
        _ if record_type == "fork" => Some("fork"),
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
        "replaceable": status.replaceable,
    })
}

fn auto_page_result_value(result: &AutoPageRunResult) -> Value {
    json!({
        "state": result.status,
        "message": result.message,
        "kind": result.status,
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
    let mut status = session
        .status
        .lock()
        .map_err(|_| api_error_message("capture_lock_poisoned", "capture lock poisoned"))?
        .clone();
    if status.state == "completed" {
        merge_completed_capture(state, &mut status)?;
    } else if let Some(report) = state
        .captures
        .lock()
        .map_err(|_| api_error_message("capture_lock_poisoned", "capture lock poisoned"))?
        .get(&status.session_id)
        .and_then(|meta| meta.import_report.clone())
    {
        status.import_report = Some(report);
    }
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
        .ok_or_else(|| {
            api_error_message(
                "capture_session_unknown",
                format!("capture session not found: {session_id}"),
            )
        })
}

fn merge_completed_capture(
    state: &State<'_, AppState>,
    status: &mut CaptureStatus,
) -> Result<(), ApiError> {
    let mut captures = state
        .captures
        .lock()
        .map_err(|_| api_error_message("capture_lock_poisoned", "capture lock poisoned"))?;
    let meta = captures.get_mut(&status.session_id).ok_or_else(|| {
        api_error_message(
            "capture_session_unknown",
            "capture session is not registered",
        )
    })?;
    if let Some(report) = meta.import_report.clone() {
        status.import_report = Some(report);
        return Ok(());
    }

    let document = status.document.as_ref().ok_or_else(|| {
        api_error_message(
            "capture_document_missing",
            "capture completed without a public document",
        )
    })?;
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

#[cfg(not(windows))]
fn admin_relaunch_required() -> Result<bool, ApiError> {
    Ok(false)
}

#[cfg(windows)]
fn admin_relaunch_required() -> Result<bool, ApiError> {
    windows_admin::is_elevated().map(|is_elevated| !is_elevated)
}

#[cfg(not(windows))]
fn relaunch_admin_with_capture_payload(_path: &Path) -> Result<(), ApiError> {
    Err(api_error_message(
        "admin_relaunch_unsupported",
        "administrator relaunch requires Windows",
    ))
}

#[cfg(windows)]
fn relaunch_admin_with_capture_payload(path: &Path) -> Result<(), ApiError> {
    let executable = env::var_os("NTE_GACHA_LAUNCHER")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| env::current_exe().ok())
        .ok_or_else(|| {
            api_error_message("admin_relaunch_failed", "cannot resolve launcher path")
        })?;
    let working_dir = env::var_os("NTE_GACHA_PORTABLE_ROOT")
        .or_else(|| env::var_os("NTE_GACHA_ROOT"))
        .map(PathBuf::from)
        .or_else(|| env::current_dir().ok());
    windows_admin::runas(
        &executable,
        &[
            "--admin-capture-json".into(),
            path.as_os_str().to_os_string(),
        ],
        working_dir.as_deref(),
    )
}

#[cfg(windows)]
mod windows_admin {
    use std::ffi::{OsStr, OsString};
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use std::ptr;

    use super::{api_error_message, ApiError};

    const SW_SHOWNORMAL: i32 = 1;

    #[link(name = "shell32")]
    extern "system" {
        fn IsUserAnAdmin() -> i32;
        fn ShellExecuteW(
            hwnd: *mut std::ffi::c_void,
            lp_operation: *const u16,
            lp_file: *const u16,
            lp_parameters: *const u16,
            lp_directory: *const u16,
            n_show_cmd: i32,
        ) -> isize;
    }

    pub fn is_elevated() -> Result<bool, ApiError> {
        Ok(unsafe { IsUserAnAdmin() != 0 })
    }

    pub fn runas(
        executable: &Path,
        arguments: &[OsString],
        working_dir: Option<&Path>,
    ) -> Result<(), ApiError> {
        let operation = wide("runas");
        let file = wide(executable.as_os_str());
        let parameters = wide(command_line(arguments));
        let directory = working_dir.map(|path| wide(path.as_os_str()));
        let directory_ptr = directory
            .as_ref()
            .map_or(ptr::null(), |value| value.as_ptr());
        let result = unsafe {
            ShellExecuteW(
                ptr::null_mut(),
                operation.as_ptr(),
                file.as_ptr(),
                parameters.as_ptr(),
                directory_ptr,
                SW_SHOWNORMAL,
            )
        };
        if result <= 32 {
            return Err(api_error_message(
                "admin_relaunch_failed",
                format!("administrator relaunch failed: ShellExecuteW={result}"),
            ));
        }
        Ok(())
    }

    fn command_line(args: &[OsString]) -> OsString {
        OsString::from(
            args.iter()
                .map(|arg| quote_arg(&arg.to_string_lossy()))
                .collect::<Vec<_>>()
                .join(" "),
        )
    }

    fn quote_arg(arg: &str) -> String {
        if arg.is_empty() {
            return "\"\"".to_string();
        }
        if !arg
            .bytes()
            .any(|byte| matches!(byte, b' ' | b'\t' | b'"' | b'\\'))
        {
            return arg.to_string();
        }
        let mut quoted = String::from("\"");
        let mut backslashes = 0;
        for ch in arg.chars() {
            if ch == '\\' {
                backslashes += 1;
                continue;
            }
            if ch == '"' {
                quoted.push_str(&"\\".repeat(backslashes * 2 + 1));
                quoted.push('"');
                backslashes = 0;
                continue;
            }
            if backslashes > 0 {
                quoted.push_str(&"\\".repeat(backslashes));
                backslashes = 0;
            }
            quoted.push(ch);
        }
        if backslashes > 0 {
            quoted.push_str(&"\\".repeat(backslashes * 2));
        }
        quoted.push('"');
        quoted
    }

    fn wide(value: impl AsRef<OsStr>) -> Vec<u16> {
        value
            .as_ref()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }
}

fn api_error(error: impl std::fmt::Display) -> ApiError {
    ApiError {
        code: "internal_error".to_string(),
        message: error.to_string(),
    }
}

fn api_error_message(code: impl Into<String>, message: impl std::fmt::Display) -> ApiError {
    ApiError {
        code: code.into(),
        message: message.to_string(),
    }
}
