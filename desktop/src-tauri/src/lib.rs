use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::Mutex;

use nte_gui_core::{
    AppDatabase, DashboardSummary, GuiError, ImportReport, ItemMeta, PoolRule, Profile,
    RecordFilter, RecordList,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{Manager, State};

struct AppState {
    db: Mutex<AppDatabase>,
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

#[derive(Debug, Serialize, Deserialize)]
struct RawReplayResult {
    document: Value,
    records_count: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct RulesBuildResult {
    pool_rules: Vec<PoolRule>,
    item_meta: Vec<ItemMeta>,
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
    let value = sidecar_call("raw.replay", json!({ "path": path, "locale": locale }))?;
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
    let value = sidecar_call("rules.build", json!({ "locale": locale }))?;
    let result: RulesBuildResult = serde_json::from_value(value).map_err(to_string)?;
    with_db(&state, |db| {
        db.upsert_rules(&result.pool_rules, &result.item_meta)
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
fn sidecar_ping() -> Result<Value, String> {
    sidecar_call("app.ping", json!({}))
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
            app.manage(AppState { db: Mutex::new(db) });
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

fn sidecar_call(method: &str, params: Value) -> Result<Value, String> {
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params
    });
    let mut spawn_errors = Vec::new();
    for sidecar in sidecar_candidates() {
        match run_sidecar(&sidecar, &request) {
            Ok(stdout) => return parse_sidecar_response(&stdout),
            Err(error) => spawn_errors.push(format!("{}: {error}", sidecar.label())),
        }
    }
    Err(format!(
        "failed to start sidecar: {}",
        spawn_errors.join("; ")
    ))
}

fn run_sidecar(sidecar: &SidecarCommand, request: &Value) -> Result<String, String> {
    let mut command = Command::new(&sidecar.program);
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(to_string)?;
    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| "sidecar stdin unavailable".to_string())?;
        writeln!(stdin, "{request}").map_err(to_string)?;
    }
    let output = child.wait_with_output().map_err(to_string)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "sidecar exited with {}: {}",
            output.status,
            stderr.trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn parse_sidecar_response(stdout: &str) -> Result<Value, String> {
    let line = stdout
        .lines()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| "sidecar returned no response".to_string())?;
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
