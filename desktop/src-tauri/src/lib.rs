use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;

use nte_gui_core::{
    available_locales, check_update_manifest, load_locale_or_settings, prepare_update_install,
    stage_update_archive, update_status, BackupReport, DashboardOverview, GuiError, ImportReport,
    JsonStore, MapLocaleList, PoolKind, PoolKindDetail, Profile, RecordFilter, RecordFilterOptions,
    RecordList, RestoreReport, Settings, SettingsPatch, UpdateChannel, UpdateCheckReport,
    UpdateManifest, UpdatePackage, UpdateStageReport, UpdateStatus,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{Manager, State};

struct AppState {
    store: Mutex<JsonStore>,
    sidecar: Mutex<Option<SidecarClient>>,
    captures: Mutex<HashMap<String, CaptureSessionMeta>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SidecarResponse {
    result: Option<Value>,
    error: Option<SidecarError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SidecarError {
    code: String,
    message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApiError {
    code: String,
    message: String,
}

#[derive(Debug, Clone)]
struct SidecarCommand {
    program: String,
}

struct SidecarClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

#[derive(Debug, Clone)]
struct CaptureSessionMeta {
    profile_name: String,
    source_kind: String,
    source_path: Option<String>,
    full_update: bool,
    import_report: Option<ImportReport>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum CaptureMode {
    LiveOnly,
    AutoPageIncremental,
    AutoPageFull,
}

impl CaptureMode {
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
struct RawReplayResult {
    document: Value,
    records_count: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct DoctorReport {
    ok: bool,
    exit_code: i64,
    lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CaptureCounters {
    packets_seen: u64,
    decoded_packets: u64,
    dropped_packets: u64,
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
    error: Option<SidecarError>,
    #[serde(default, skip_serializing)]
    document: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    import_report: Option<ImportReport>,
}

const GITHUB_RELEASES_API: &str =
    "https://api.github.com/repos/Anong0u0/nte_gacha_exporter/releases";
const UPDATE_MANIFEST_ASSET: &str = "nte-gacha-update.json";
const USER_AGENT: &str = "nte-gacha-exporter-updater";

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
    let value = sidecar_call(
        &state,
        "raw.replay",
        json!({ "path": path, "locale": locale }),
    )?;
    let replay: RawReplayResult = serde_json::from_value(value).map_err(api_error)?;
    let document_text = serde_json::to_string(&replay.document).map_err(api_error)?;
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
fn doctor_run(state: State<'_, AppState>) -> Result<DoctorReport, ApiError> {
    let value = sidecar_call(&state, "doctor.run", json!({}))?;
    serde_json::from_value(value).map_err(api_error)
}

#[tauri::command]
fn sidecar_ping(state: State<'_, AppState>) -> Result<Value, ApiError> {
    sidecar_call(&state, "app.ping", json!({}))
}

#[tauri::command]
fn capture_start(
    state: State<'_, AppState>,
    profile_name: String,
    locale: Option<String>,
    mode: Option<CaptureMode>,
) -> Result<CaptureStatus, ApiError> {
    let mode = mode.unwrap_or(CaptureMode::AutoPageIncremental);
    let locale = with_store(&state, |store| {
        let locale = load_locale_or_settings(store, locale)?;
        store.dashboard_overview(&profile_name, &locale)?;
        Ok(locale)
    })?;
    let (known_record_ids, output_raw) = with_store(&state, |store| {
        let ids = if mode == CaptureMode::AutoPageIncremental {
            store.profile_record_ids(&profile_name)?
        } else {
            Vec::new()
        };
        let raw_path = if mode.auto_page() {
            Some(store.default_run_raw_path().to_string_lossy().to_string())
        } else {
            None
        };
        Ok((ids, raw_path))
    })?;
    let value = sidecar_call(
        &state,
        "capture.start",
        json!({
            "locale": locale,
            "auto_page": mode.auto_page(),
            "full_update": mode.full_update(),
            "known_record_ids": known_record_ids,
            "output_raw": output_raw,
        }),
    )?;
    let mut status = parse_capture_status(value)?;
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
    let _ = sidecar_call(&state, "capture.stop", json!({ "session_id": session_id }))?;
    capture_status_with_merge(&state, &session_id)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let root =
                portable_root().map_err(|err| format!("failed to resolve portable root: {err}"))?;
            let store =
                JsonStore::open(root).map_err(|err| format!("failed to open JSON store: {err}"))?;
            app.manage(AppState {
                store: Mutex::new(store),
                sidecar: Mutex::new(None),
                captures: Mutex::new(HashMap::new()),
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
    if let Ok(root) = std::env::var("NTE_GACHA_PORTABLE_ROOT") {
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

fn sidecar_call(
    state: &State<'_, AppState>,
    method: &str,
    params: Value,
) -> Result<Value, ApiError> {
    let mut sidecar = state
        .sidecar
        .lock()
        .map_err(|_| api_error_message("sidecar_lock_poisoned", "sidecar lock poisoned"))?;
    if sidecar.is_none() {
        *sidecar = Some(SidecarClient::connect_candidates()?);
    }
    sidecar
        .as_mut()
        .ok_or_else(|| api_error_message("sidecar_unavailable", "sidecar unavailable"))?
        .call(method, params)
}

fn capture_status_with_merge(
    state: &State<'_, AppState>,
    session_id: &str,
) -> Result<CaptureStatus, ApiError> {
    let value = sidecar_call(
        state,
        "capture.status",
        json!({ "session_id": session_id, "include_document": true }),
    )?;
    let mut status = parse_capture_status(value)?;
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

fn parse_capture_status(value: Value) -> Result<CaptureStatus, ApiError> {
    serde_json::from_value(value).map_err(api_error)
}

impl SidecarClient {
    fn connect_candidates() -> Result<Self, ApiError> {
        let mut spawn_errors = Vec::new();
        for sidecar in sidecar_candidates() {
            match Self::connect(sidecar.clone()) {
                Ok(client) => return Ok(client),
                Err(error) => spawn_errors.push(format!(
                    "{}: {}: {}",
                    sidecar.label(),
                    error.code,
                    error.message
                )),
            }
        }
        Err(api_error_message(
            "sidecar_start_failed",
            format!("failed to start sidecar: {}", spawn_errors.join("; ")),
        ))
    }

    fn connect(sidecar: SidecarCommand) -> Result<Self, ApiError> {
        let mut child = Command::new(&sidecar.program)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(api_error)?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| api_error_message("sidecar_io", "sidecar stdin unavailable"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| api_error_message("sidecar_io", "sidecar stdout unavailable"))?;
        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
        })
    }

    fn call(&mut self, method: &str, params: Value) -> Result<Value, ApiError> {
        let request_id = self.next_id;
        self.next_id += 1;
        let request = json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": method,
            "params": params
        });
        writeln!(self.stdin, "{request}").map_err(api_error)?;
        self.stdin.flush().map_err(api_error)?;

        let mut line = String::new();
        let read = self.stdout.read_line(&mut line).map_err(api_error)?;
        if read == 0 {
            return Err(api_error_message(
                "sidecar_exited",
                format!("sidecar exited while handling {method}"),
            ));
        }
        parse_sidecar_response(&line)
    }
}

impl Drop for SidecarClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn parse_sidecar_response(line: &str) -> Result<Value, ApiError> {
    if line.trim().is_empty() {
        return Err(api_error_message(
            "sidecar_empty_response",
            "sidecar returned no response",
        ));
    }
    let response: SidecarResponse = serde_json::from_str(line).map_err(api_error)?;
    if let Some(error) = response.error {
        return Err(ApiError {
            code: error.code,
            message: error.message,
        });
    }
    response
        .result
        .ok_or_else(|| api_error_message("sidecar_bad_response", "sidecar response missing result"))
}

fn sidecar_candidates() -> Vec<SidecarCommand> {
    if let Ok(program) = std::env::var("NTE_GACHA_SIDECAR") {
        return vec![SidecarCommand { program }];
    }

    let mut candidates = Vec::new();
    if let Ok(root) =
        std::env::var("NTE_GACHA_PORTABLE_ROOT").or_else(|_| std::env::var("NTE_GACHA_ROOT"))
    {
        let sidecars = PathBuf::from(root).join("sidecars");
        push_existing_sidecar(&mut candidates, sidecars.join(sidecar_exe_name()));
        push_existing_sidecar(&mut candidates, sidecars.join(sidecar_cmd_name()));
    }
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            push_existing_sidecar(
                &mut candidates,
                exe_dir.join("sidecars").join(sidecar_exe_name()),
            );
            push_existing_sidecar(
                &mut candidates,
                exe_dir.join("sidecars").join(sidecar_cmd_name()),
            );
        }
    }
    if let Ok(current_dir) = std::env::current_dir() {
        push_existing_sidecar(
            &mut candidates,
            current_dir.join("sidecars").join(sidecar_exe_name()),
        );
        push_existing_sidecar(
            &mut candidates,
            current_dir.join("sidecars").join(sidecar_cmd_name()),
        );
    }
    candidates.push(SidecarCommand {
        program: "nte-gacha-sidecar".to_string(),
    });
    candidates
}

fn push_existing_sidecar(candidates: &mut Vec<SidecarCommand>, path: PathBuf) {
    if path.is_file() {
        candidates.push(SidecarCommand {
            program: path.to_string_lossy().to_string(),
        });
    }
}

fn sidecar_exe_name() -> &'static str {
    if cfg!(windows) {
        "nte-gacha-python-core.exe"
    } else {
        "nte-gacha-python-core"
    }
}

fn sidecar_cmd_name() -> &'static str {
    "nte-gacha-python-core.cmd"
}

impl SidecarCommand {
    fn label(&self) -> String {
        self.program.clone()
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
