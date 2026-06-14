use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;

use nte_gui_core::{
    AppDatabase, DashboardSummary, GuiError, ImportReport, ItemAlias, ItemMeta, PoolRule, Profile,
    RecordFilter, RecordList,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{Manager, State};

struct AppState {
    db: Mutex<AppDatabase>,
    sidecar: Mutex<Option<SidecarClient>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SidecarResponse {
    result: Option<Value>,
    error: Option<SidecarError>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SidecarError {
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

#[derive(Debug, Serialize, Deserialize)]
struct RawReplayResult {
    document: Value,
    records_count: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct RulesBuildResult {
    pool_rules: Vec<PoolRule>,
    item_meta: Vec<ItemMeta>,
    item_aliases: Vec<ItemAlias>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DoctorReport {
    ok: bool,
    exit_code: i64,
    lines: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CaptureCounters {
    packets_seen: u64,
    decoded_packets: u64,
    dropped_packets: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct CaptureStatus {
    session_id: String,
    state: String,
    records_count: u64,
    latest_records: Vec<Value>,
    counters: CaptureCounters,
    started_at: f64,
    updated_at: f64,
    target: Option<Value>,
    error: Option<SidecarError>,
    document: Option<Value>,
}

#[tauri::command]
fn list_profiles(state: State<'_, AppState>) -> Result<Vec<Profile>, String> {
    with_db(&state, |db| db.list_profiles())
}

#[tauri::command]
fn create_profile(state: State<'_, AppState>, name: String) -> Result<Profile, String> {
    with_db(&state, |db| db.create_profile(&name))
}

#[tauri::command]
fn import_public_json(
    state: State<'_, AppState>,
    profile_id: i64,
    path: String,
) -> Result<ImportReport, String> {
    let text = fs::read_to_string(&path).map_err(to_string)?;
    with_db(&state, |db| {
        db.import_public_document(profile_id, &text, "json", Some(&path))
    })
}

#[tauri::command]
fn import_raw_jsonl(
    state: State<'_, AppState>,
    profile_id: i64,
    path: String,
    locale: Option<String>,
) -> Result<ImportReport, String> {
    let locale = locale.unwrap_or_else(|| "zh-Hant".to_string());
    let value = sidecar_call(
        &state,
        "raw.replay",
        json!({ "path": path, "locale": locale }),
    )?;
    let replay: RawReplayResult = serde_json::from_value(value).map_err(to_string)?;
    let document_text = serde_json::to_string(&replay.document).map_err(to_string)?;
    with_db(&state, |db| {
        db.import_public_document(
            profile_id,
            &document_text,
            "raw_jsonl",
            replay_source(&replay.document).as_deref(),
        )
    })
}

#[tauri::command]
fn refresh_rules(
    state: State<'_, AppState>,
    locale: Option<String>,
) -> Result<RulesBuildResult, String> {
    let locale = locale.unwrap_or_else(|| "zh-Hant".to_string());
    let value = sidecar_call(&state, "rules.build", json!({ "locale": locale }))?;
    let result: RulesBuildResult = serde_json::from_value(value).map_err(to_string)?;
    with_db(&state, |db| {
        db.upsert_rules(&result.pool_rules, &result.item_meta, &result.item_aliases)
    })?;
    Ok(result)
}

#[tauri::command]
fn dashboard_summary(
    state: State<'_, AppState>,
    profile_id: i64,
) -> Result<DashboardSummary, String> {
    with_db(&state, |db| db.dashboard_summary(profile_id))
}

#[tauri::command]
fn list_records(
    state: State<'_, AppState>,
    profile_id: i64,
    filter: RecordFilter,
) -> Result<RecordList, String> {
    with_db(&state, |db| db.list_records(profile_id, &filter))
}

#[tauri::command]
fn export_profile_json(
    state: State<'_, AppState>,
    profile_id: i64,
    path: String,
) -> Result<(), String> {
    let text = with_db(&state, |db| db.export_json(profile_id))?;
    fs::write(path, text).map_err(to_string)
}

#[tauri::command]
fn export_profile_csv(
    state: State<'_, AppState>,
    profile_id: i64,
    path: String,
) -> Result<(), String> {
    let text = with_db(&state, |db| db.export_csv(profile_id))?;
    fs::write(path, text).map_err(to_string)
}

#[tauri::command]
fn doctor_run(state: State<'_, AppState>) -> Result<DoctorReport, String> {
    let value = sidecar_call(&state, "doctor.run", json!({}))?;
    serde_json::from_value(value).map_err(to_string)
}

#[tauri::command]
fn start_live_capture(
    state: State<'_, AppState>,
    locale: Option<String>,
    pid: Option<String>,
    iface: Option<String>,
    output_raw: Option<String>,
) -> Result<CaptureStatus, String> {
    let locale = locale.unwrap_or_else(|| "zh-Hant".to_string());
    let value = sidecar_call(
        &state,
        "capture.start",
        json!({ "locale": locale, "pid": pid, "iface": iface, "output_raw": output_raw }),
    )?;
    serde_json::from_value(value).map_err(to_string)
}

#[tauri::command]
fn live_capture_status(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<CaptureStatus, String> {
    let value = sidecar_call(
        &state,
        "capture.status",
        json!({ "session_id": session_id, "include_document": false }),
    )?;
    serde_json::from_value(value).map_err(to_string)
}

#[tauri::command]
fn stop_live_capture(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<CaptureStatus, String> {
    let value = sidecar_call(&state, "capture.stop", json!({ "session_id": session_id }))?;
    serde_json::from_value(value).map_err(to_string)
}

#[tauri::command]
fn finalize_live_capture(
    state: State<'_, AppState>,
    profile_id: i64,
    session_id: String,
) -> Result<ImportReport, String> {
    let value = sidecar_call(
        &state,
        "capture.status",
        json!({ "session_id": session_id, "include_document": true }),
    )?;
    let status: CaptureStatus = serde_json::from_value(value).map_err(to_string)?;
    if status.state != "completed" {
        return Err(format!(
            "capture session is not completed: {}",
            status.state
        ));
    }
    let document = status
        .document
        .ok_or_else(|| "capture session has no document".to_string())?;
    let document_text = serde_json::to_string(&document).map_err(to_string)?;
    with_db(&state, |db| {
        db.import_public_document(
            profile_id,
            &document_text,
            "live_capture",
            Some(&format!("session:{}", status.session_id)),
        )
    })
}

#[tauri::command]
fn sidecar_ping(state: State<'_, AppState>) -> Result<Value, String> {
    sidecar_call(&state, "app.ping", json!({}))
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .or_else(|_| std::env::current_dir().map(|path| path.join("data")))
                .map_err(|err| format!("failed to resolve app data dir: {err}"))?;
            let db_path = data_dir.join("nte-gacha.db");
            let db = AppDatabase::open(db_path)
                .map_err(|err| format!("failed to open database: {err}"))?;
            app.manage(AppState {
                db: Mutex::new(db),
                sidecar: Mutex::new(None),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_profiles,
            create_profile,
            import_public_json,
            import_raw_jsonl,
            refresh_rules,
            dashboard_summary,
            list_records,
            export_profile_json,
            export_profile_csv,
            doctor_run,
            start_live_capture,
            live_capture_status,
            stop_live_capture,
            finalize_live_capture,
            sidecar_ping
        ])
        .run(tauri::generate_context!())
        .expect("error while running Tauri application");
}

fn with_db<T>(
    state: &State<'_, AppState>,
    f: impl FnOnce(&mut AppDatabase) -> Result<T, GuiError>,
) -> Result<T, String> {
    let mut db = state
        .db
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    f(&mut db).map_err(to_string)
}

fn sidecar_call(state: &State<'_, AppState>, method: &str, params: Value) -> Result<Value, String> {
    let mut sidecar = state
        .sidecar
        .lock()
        .map_err(|_| "sidecar lock poisoned".to_string())?;
    if sidecar.is_none() {
        *sidecar = Some(SidecarClient::connect_candidates()?);
    }
    sidecar
        .as_mut()
        .ok_or_else(|| "sidecar unavailable".to_string())?
        .call(method, params)
}

impl SidecarClient {
    fn connect_candidates() -> Result<Self, String> {
        let mut spawn_errors = Vec::new();
        for sidecar in sidecar_candidates() {
            match Self::connect(sidecar.clone()) {
                Ok(client) => return Ok(client),
                Err(error) => spawn_errors.push(format!("{}: {error}", sidecar.label())),
            }
        }
        Err(format!(
            "failed to start sidecar: {}",
            spawn_errors.join("; ")
        ))
    }

    fn connect(sidecar: SidecarCommand) -> Result<Self, String> {
        let mut command = Command::new(&sidecar.program);
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(to_string)?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "sidecar stdin unavailable".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "sidecar stdout unavailable".to_string())?;
        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
        })
    }

    fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let request_id = self.next_id;
        self.next_id += 1;
        let request = json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": method,
            "params": params
        });
        writeln!(self.stdin, "{request}").map_err(to_string)?;
        self.stdin.flush().map_err(to_string)?;

        let mut line = String::new();
        let read = self.stdout.read_line(&mut line).map_err(to_string)?;
        if read == 0 {
            return Err(format!("sidecar exited while handling {method}"));
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

fn parse_sidecar_response(line: &str) -> Result<Value, String> {
    if line.trim().is_empty() {
        return Err("sidecar returned no response".to_string());
    }
    let response: SidecarResponse = serde_json::from_str(line).map_err(to_string)?;
    if let Some(error) = response.error {
        return Err(format!("{}: {}", error.code, error.message));
    }
    response
        .result
        .ok_or_else(|| "sidecar response missing result".to_string())
}

fn sidecar_candidates() -> Vec<SidecarCommand> {
    if let Ok(program) = std::env::var("NTE_GACHA_SIDECAR") {
        return vec![SidecarCommand { program }];
    }

    vec![SidecarCommand {
        program: "nte-gacha-sidecar".to_string(),
    }]
}

impl SidecarCommand {
    fn label(&self) -> String {
        self.program.clone()
    }
}

fn replay_source(document: &Value) -> Option<String> {
    document
        .get("_debug")
        .and_then(|debug| debug.get("source"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn to_string(error: impl std::fmt::Display) -> String {
    error.to_string()
}
