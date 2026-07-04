use nte_capture::capture_doctor;
use nte_core::{MapLocaleList, available_locales, available_ui_locales};
use nte_store::StoreDefaults;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{ApiError, api_error, api_error_message};
use crate::state::AppState;

const ABOUT_GITHUB_URL: &str = "https://github.com/Anong0u0/nte_gacha_exporter";
const ABOUT_DISCORD_URL: &str = "https://discord.gg/4qm2HPtPZq";

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

#[tauri::command]
pub(crate) fn open_about_link(target: String) -> Result<(), ApiError> {
    platform::open_external_url(about_link_url(&target)?)
}

fn about_link_url(target: &str) -> Result<&'static str, ApiError> {
    match target {
        "github" => Ok(ABOUT_GITHUB_URL),
        "discord" => Ok(ABOUT_DISCORD_URL),
        _ => Err(api_error_message(
            "invalid_about_link",
            format!("unknown about link target: {target}"),
        )),
    }
}

pub(crate) fn store_defaults() -> StoreDefaults {
    let system_locale = user_system_locale();
    let data_locales = available_locales();
    let ui_locales = available_ui_locales();
    StoreDefaults {
        locale: resolve_data_locale(system_locale.as_deref(), &data_locales),
        ui_locale: resolve_ui_locale(system_locale.as_deref(), &ui_locales),
    }
}

fn resolve_ui_locale(locale: Option<&str>, available: &[String]) -> String {
    resolve_available_locale(locale, available)
}

fn resolve_data_locale(locale: Option<&str>, available: &[String]) -> String {
    resolve_available_locale(locale, available)
}

fn resolve_available_locale(locale: Option<&str>, available: &[String]) -> String {
    let candidates = locale_candidates(locale);
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

fn locale_candidates(locale: Option<&str>) -> Vec<String> {
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

#[cfg(windows)]
mod platform {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    use super::*;

    const SW_SHOWNORMAL: i32 = 1;

    #[link(name = "shell32")]
    unsafe extern "system" {
        fn ShellExecuteW(
            hwnd: *mut std::ffi::c_void,
            lp_operation: *const u16,
            lp_file: *const u16,
            lp_parameters: *const u16,
            lp_directory: *const u16,
            n_show_cmd: i32,
        ) -> isize;
    }

    pub(super) fn open_external_url(url: &str) -> Result<(), ApiError> {
        let operation = wide("open");
        let file = wide(url);
        let result = unsafe {
            ShellExecuteW(
                ptr::null_mut(),
                operation.as_ptr(),
                file.as_ptr(),
                ptr::null(),
                ptr::null(),
                SW_SHOWNORMAL,
            )
        };
        if result <= 32 {
            return Err(api_error_message(
                "open_about_link_failed",
                format!("open about link failed: ShellExecuteW={result}"),
            ));
        }
        Ok(())
    }

    fn wide(value: &str) -> Vec<u16> {
        OsStr::new(value)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }
}

#[cfg(not(windows))]
mod platform {
    use super::*;

    pub(super) fn open_external_url(_url: &str) -> Result<(), ApiError> {
        Ok(())
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
    lines.push(format!(
        "PPPoE detected: {}",
        report.pppoe_detection.detected
    ));
    lines.extend(report.notes);
    let capture_ready =
        report.pid.is_some() && (!report.ports.is_empty() || report.pppoe_detection.detected);
    Ok(DoctorReport {
        ok: report.windows && report.admin && capture_ready,
        exit_code: if report.windows && report.admin { 0 } else { 3 },
        lines,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_locale_matches_available_locale_candidates() {
        let available = locales(&["en", "zh-CN", "zh-Hant"]);

        assert_eq!(resolve_ui_locale(Some("zh-TW"), &available), "zh-Hant");
        assert_eq!(resolve_ui_locale(Some("zh_HK"), &available), "zh-Hant");
        assert_eq!(resolve_ui_locale(Some("zh-Hant"), &available), "zh-Hant");
        assert_eq!(resolve_ui_locale(Some("zh-SG"), &available), "zh-CN");
        assert_eq!(resolve_ui_locale(Some("ja-JP"), &available), "en");
        assert_eq!(resolve_ui_locale(None, &available), "en");
    }

    #[test]
    fn ui_locale_uses_language_code_when_dictionary_exists() {
        let available = locales(&["en", "ja", "zh-CN", "zh-Hant"]);

        assert_eq!(resolve_ui_locale(Some("ja-JP"), &available), "ja");
        assert_eq!(resolve_ui_locale(Some("fr-CA"), &available), "en");
    }

    #[test]
    fn data_locale_prefers_available_match_then_english() {
        let available = locales(&["en", "ja", "zh-Hans", "zh-Hant"]);
        assert_eq!(resolve_data_locale(Some("zh-TW"), &available), "zh-Hant");
        assert_eq!(resolve_data_locale(Some("ja-JP"), &available), "ja");
        assert_eq!(resolve_data_locale(Some("fr-CA"), &available), "en");
    }

    #[test]
    fn about_link_targets_are_fixed() {
        assert_eq!(about_link_url("github").unwrap(), ABOUT_GITHUB_URL);
        assert_eq!(about_link_url("discord").unwrap(), ABOUT_DISCORD_URL);
    }

    #[test]
    fn about_link_rejects_unknown_target() {
        assert!(about_link_url("https://example.com").is_err());
    }

    fn locales(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }
}
