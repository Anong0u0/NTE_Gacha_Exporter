use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use nte_core::{
    UpdateChannel, UpdateCheckReport, UpdateManifest, UpdatePackage, UpdateStageReport,
    UpdateStatus,
};
use nte_update::{
    check_update_manifest, prepare_update_install, stage_update_archive, update_status,
};
use serde::Deserialize;
use tauri::State;

use crate::error::{ApiError, api_error, api_error_message};
use crate::state::{AppState, with_store};

const GITHUB_RELEASES_API: &str =
    "https://api.github.com/repos/Anong0u0/nte_gacha_exporter/releases";
const UPDATE_MANIFEST_ASSET: &str = "nte-gacha-exporter-update.json";
const USER_AGENT: &str = "nte-gacha-exporter-updater";

#[tauri::command]
pub(crate) fn updater_status(state: State<'_, AppState>) -> Result<UpdateStatus, ApiError> {
    with_store(&state, |store| update_status(store.root(), app_version()))
}

#[tauri::command]
pub(crate) fn updater_check(
    state: State<'_, AppState>,
    channel: Option<String>,
) -> Result<UpdateCheckReport, ApiError> {
    let requested_channel = update_channel_or_settings(&state, channel)?;
    let manifest = fetch_update_manifest(requested_channel)?;
    check_update_manifest(manifest, app_version(), requested_channel).map_err(api_error)
}

#[tauri::command]
pub(crate) fn updater_download_and_stage(
    state: State<'_, AppState>,
    package: UpdatePackage,
) -> Result<UpdateStageReport, ApiError> {
    let root = with_store(&state, |store| Ok(store.root().to_path_buf()))?;
    let archive_path = download_update_archive(&root, &package)?;
    stage_update_archive(&root, &package, archive_path).map_err(api_error)
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
    if let Ok(source) = std::env::var("NTE_GACHA_EXPORTER_UPDATE_MANIFEST") {
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
