use tauri::State;

use crate::error::{ApiError, api_error, api_error_message};
use crate::state::{AppState, portable_root};

#[tauri::command]
pub(crate) fn windivert_status(
    _state: State<'_, AppState>,
    check_load: Option<bool>,
) -> Result<nte_capture::windivert::WinDivertInstallStatus, ApiError> {
    let root = portable_root().map_err(api_error)?;
    Ok(nte_capture::windivert::windivert_status(
        &root,
        check_load.unwrap_or(false),
    ))
}

#[tauri::command]
pub(crate) fn windivert_install(
    _state: State<'_, AppState>,
) -> Result<nte_capture::windivert::WinDivertInstallReport, ApiError> {
    let root = portable_root().map_err(api_error)?;
    nte_capture::windivert::install_windivert(&root)
        .map_err(|message| api_error_message("windivert_unavailable", message))
}
