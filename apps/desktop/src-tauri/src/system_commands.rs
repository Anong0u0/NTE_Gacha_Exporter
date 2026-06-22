use nte_capture::capture_doctor;
use nte_core::{MapLocaleList, available_locales, available_ui_locales};
use nte_store::StoreDefaults;
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
pub(crate) fn ui_locale_list() -> MapLocaleList {
    MapLocaleList {
        locales: available_ui_locales(),
    }
}

#[tauri::command]
pub(crate) fn system_locale() -> Option<String> {
    user_system_locale()
}

pub(crate) fn store_defaults() -> StoreDefaults {
    let system_locale = user_system_locale();
    let locales = available_locales();
    StoreDefaults {
        locale: resolve_data_locale(system_locale.as_deref(), &locales),
        ui_locale: resolve_ui_locale(system_locale.as_deref()),
    }
}

fn resolve_ui_locale(locale: Option<&str>) -> String {
    if is_traditional_chinese(locale) {
        "zh-Hant".to_string()
    } else {
        "en".to_string()
    }
}

fn resolve_data_locale(locale: Option<&str>, available: &[String]) -> String {
    let candidates = data_locale_candidates(locale);
    for candidate in candidates {
        if available.iter().any(|item| item == &candidate) {
            return candidate;
        }
    }
    if available.iter().any(|item| item == "en") {
        "en".to_string()
    } else {
        available
            .first()
            .cloned()
            .unwrap_or_else(|| "en".to_string())
    }
}

fn data_locale_candidates(locale: Option<&str>) -> Vec<String> {
    let Some(locale) = locale.map(str::trim).filter(|value| !value.is_empty()) else {
        return vec!["en".to_string()];
    };
    let normalized = locale.replace('_', "-");
    let lower = normalized.to_ascii_lowercase();
    let mut candidates = vec![normalized.clone()];
    if is_traditional_chinese(Some(&normalized)) {
        candidates.push("zh-Hant".to_string());
    } else if lower.starts_with("zh") {
        candidates.push("zh-Hans".to_string());
        candidates.push("zh-CN".to_string());
    }
    if let Some(language) = lower.split('-').next().filter(|value| !value.is_empty()) {
        candidates.push(language.to_string());
    }
    candidates.push("en".to_string());
    candidates
}

fn is_traditional_chinese(locale: Option<&str>) -> bool {
    let Some(locale) = locale else {
        return false;
    };
    let lower = locale.replace('_', "-").to_ascii_lowercase();
    lower == "zh-hant"
        || lower.starts_with("zh-hant-")
        || matches!(lower.as_str(), "zh-tw" | "zh-hk" | "zh-mo" | "zh-tw-posix")
}

#[cfg(windows)]
fn user_system_locale() -> Option<String> {
    let mut buffer = [0_u16; 85];
    let length = unsafe {
        windows_sys::Win32::Globalization::GetUserDefaultLocaleName(
            buffer.as_mut_ptr(),
            buffer.len() as i32,
        )
    };
    if length <= 1 {
        return None;
    }
    let end = buffer
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(length as usize);
    String::from_utf16(&buffer[..end]).ok()
}

#[cfg(not(windows))]
fn user_system_locale() -> Option<String> {
    None
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_locale_uses_traditional_chinese_or_english() {
        assert_eq!(resolve_ui_locale(Some("zh-TW")), "zh-Hant");
        assert_eq!(resolve_ui_locale(Some("zh-HK")), "zh-Hant");
        assert_eq!(resolve_ui_locale(Some("zh-Hant")), "zh-Hant");
        assert_eq!(resolve_ui_locale(Some("ja-JP")), "en");
        assert_eq!(resolve_ui_locale(None), "en");
    }

    #[test]
    fn data_locale_prefers_available_match_then_english() {
        let available = ["en", "ja", "zh-Hans", "zh-Hant"]
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>();
        assert_eq!(resolve_data_locale(Some("zh-TW"), &available), "zh-Hant");
        assert_eq!(resolve_data_locale(Some("ja-JP"), &available), "ja");
        assert_eq!(resolve_data_locale(Some("fr-CA"), &available), "en");
    }
}
