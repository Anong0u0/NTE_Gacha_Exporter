use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::capture::{CaptureMode, CaptureStartOptions};
use crate::error::{ApiError, api_error, api_error_message};
use crate::state::{AppState, with_store};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PendingAdminCapture {
    pub(crate) profile_name: String,
    pub(crate) locale: String,
    pub(crate) mode: CaptureMode,
    #[serde(default)]
    pub(crate) options: CaptureStartOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PendingAdminDiagnostic {
    pub(crate) duration_seconds: u64,
}

#[tauri::command]
pub(crate) fn request_admin_capture_start(
    state: State<'_, AppState>,
    profile_name: String,
    locale: Option<String>,
    mode: Option<CaptureMode>,
    options: Option<CaptureStartOptions>,
) -> Result<bool, ApiError> {
    let mode = mode.unwrap_or(CaptureMode::LiveOnly);
    if !admin_relaunch_required()? {
        return Ok(false);
    }
    let locale = with_store(&state, |store| {
        let locale = nte_store::load_locale_or_settings(store, locale)?;
        store.dashboard_overview(&profile_name, &locale)?;
        Ok(locale)
    })?;
    let payload = PendingAdminCapture {
        profile_name,
        locale,
        mode,
        options: options.unwrap_or_default(),
    };
    let path = write_admin_capture_payload(&payload)?;
    relaunch_admin_with_capture_payload(&path)?;
    schedule_process_exit();
    Ok(true)
}

#[tauri::command]
pub(crate) fn request_admin_diagnostic_start(
    duration_seconds: Option<u64>,
) -> Result<bool, ApiError> {
    if !admin_relaunch_required()? {
        return Ok(false);
    }
    let payload = PendingAdminDiagnostic {
        duration_seconds: duration_seconds.unwrap_or(30).clamp(5, 120),
    };
    let path = write_admin_diagnostic_payload(&payload)?;
    relaunch_admin_with_diagnostic_payload(&path)?;
    schedule_process_exit();
    Ok(true)
}

#[tauri::command]
pub(crate) fn take_pending_admin_capture(
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
pub(crate) fn take_pending_admin_diagnostic(
    state: State<'_, AppState>,
) -> Result<Option<PendingAdminDiagnostic>, ApiError> {
    state
        .pending_admin_diagnostic
        .lock()
        .map_err(|_| {
            api_error_message(
                "admin_diagnostic_lock_poisoned",
                "admin diagnostic lock poisoned",
            )
        })
        .map(|mut pending| pending.take())
}

pub(crate) fn pending_admin_capture_from_args() -> Result<Option<PendingAdminCapture>, ApiError> {
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

pub(crate) fn pending_admin_diagnostic_from_args()
-> Result<Option<PendingAdminDiagnostic>, ApiError> {
    let mut args = env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--admin-diagnostic-json" {
            let path = args.next().ok_or_else(|| {
                api_error_message(
                    "admin_diagnostic_arg_missing",
                    "--admin-diagnostic-json requires a path",
                )
            })?;
            let text = fs::read_to_string(path).map_err(api_error)?;
            return serde_json::from_str(&text).map(Some).map_err(api_error);
        }
    }
    Ok(None)
}

pub(crate) fn admin_relaunch_required() -> Result<bool, ApiError> {
    platform::admin_relaunch_required()
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

fn write_admin_diagnostic_payload(payload: &PendingAdminDiagnostic) -> Result<PathBuf, ApiError> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(api_error)?
        .as_millis();
    let path = env::temp_dir().join(format!(
        "nte-gacha-admin-diagnostic-{}-{stamp}.json",
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

#[cfg(not(windows))]
mod platform {
    use super::*;

    pub(super) fn admin_relaunch_required() -> Result<bool, ApiError> {
        Ok(false)
    }

    pub(super) fn relaunch_admin_with_capture_payload(_path: &Path) -> Result<(), ApiError> {
        Err(api_error_message(
            "admin_relaunch_unsupported",
            "administrator relaunch requires Windows",
        ))
    }

    pub(super) fn relaunch_admin_with_diagnostic_payload(_path: &Path) -> Result<(), ApiError> {
        Err(api_error_message(
            "admin_relaunch_unsupported",
            "administrator relaunch requires Windows",
        ))
    }
}

#[cfg(windows)]
mod platform {
    use std::ffi::{OsStr, OsString};
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    use super::*;

    const SW_SHOWNORMAL: i32 = 1;

    #[link(name = "shell32")]
    unsafe extern "system" {
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

    pub(super) fn admin_relaunch_required() -> Result<bool, ApiError> {
        Ok(unsafe { IsUserAnAdmin() == 0 })
    }

    pub(super) fn relaunch_admin_with_capture_payload(path: &Path) -> Result<(), ApiError> {
        relaunch_admin_with_payload("--admin-capture-json", path)
    }

    pub(super) fn relaunch_admin_with_diagnostic_payload(path: &Path) -> Result<(), ApiError> {
        relaunch_admin_with_payload("--admin-diagnostic-json", path)
    }

    fn relaunch_admin_with_payload(flag: &str, path: &Path) -> Result<(), ApiError> {
        let executable = env::var_os("NTE_GACHA_EXPORTER_LAUNCHER")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| env::current_exe().ok())
            .ok_or_else(|| {
                api_error_message("admin_relaunch_failed", "cannot resolve launcher path")
            })?;
        let working_dir = env::var_os("NTE_GACHA_EXPORTER_PORTABLE_ROOT")
            .or_else(|| env::var_os("NTE_GACHA_EXPORTER_ROOT"))
            .map(PathBuf::from)
            .or_else(|| env::current_dir().ok());
        runas(
            &executable,
            &[flag.into(), path.as_os_str().to_os_string()],
            working_dir.as_deref(),
        )
    }

    fn runas(
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

fn relaunch_admin_with_capture_payload(path: &Path) -> Result<(), ApiError> {
    platform::relaunch_admin_with_capture_payload(path)
}

fn relaunch_admin_with_diagnostic_payload(path: &Path) -> Result<(), ApiError> {
    platform::relaunch_admin_with_diagnostic_payload(path)
}
