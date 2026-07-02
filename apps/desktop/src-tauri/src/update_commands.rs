use std::process::{Command, Stdio};

use nte_core::{UpdateChannel, UpdateCheckReport, UpdatePackage, UpdateStageReport, UpdateStatus};
use nte_update::{prepare_update_install, update_status};
use tauri::State;

use crate::error::ApiError;
use crate::state::{AppState, with_store};
use crate::{error::api_error, update_service};

#[tauri::command]
pub(crate) fn updater_status(state: State<'_, AppState>) -> Result<UpdateStatus, ApiError> {
    with_store(&state, |store| update_status(store.root(), app_version()))
}

#[tauri::command]
pub(crate) async fn updater_check(
    state: State<'_, AppState>,
    channel: Option<String>,
) -> Result<UpdateCheckReport, ApiError> {
    let requested_channel = update_channel_or_settings(&state, channel)?;
    run_blocking_update_task(move || {
        update_service::check_for_update(requested_channel, app_version())
    })
    .await
}

#[tauri::command]
pub(crate) async fn updater_download_and_stage(
    state: State<'_, AppState>,
    package: UpdatePackage,
) -> Result<UpdateStageReport, ApiError> {
    let root = with_store(&state, |store| Ok(store.root().to_path_buf()))?;
    run_blocking_update_task(move || update_service::download_and_stage_update(&root, package))
        .await
}

#[tauri::command]
pub(crate) fn updater_install_staged(
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

fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

async fn run_blocking_update_task<T>(
    task: impl FnOnce() -> Result<T, ApiError> + Send + 'static,
) -> Result<T, ApiError>
where
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(api_error)?
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
