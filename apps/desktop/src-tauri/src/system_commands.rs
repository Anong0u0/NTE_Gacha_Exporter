use nte_capture::capture_doctor;
use nte_core::{MapLocaleList, available_locales};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::State;

use crate::error::{ApiError, api_error};
use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct DoctorReport {
    ok: bool,
    exit_code: i64,
    lines: Vec<String>,
}

#[tauri::command]
pub(crate) fn maps_list() -> MapLocaleList {
    MapLocaleList {
        locales: available_locales(),
    }
}

#[tauri::command]
pub(crate) fn doctor_run(_state: State<'_, AppState>) -> Result<DoctorReport, ApiError> {
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
pub(crate) fn runtime_ping(_state: State<'_, AppState>) -> Result<Value, ApiError> {
    Ok(json!({ "ok": true, "runtime": "rust" }))
}
