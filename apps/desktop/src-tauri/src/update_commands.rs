use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

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
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const HTTP_READ_TIMEOUT: Duration = Duration::from_secs(30);
const HTTP_WRITE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_UPDATE_JSON_BYTES: u64 = 1024 * 1024;
const READ_BUFFER_BYTES: usize = 64 * 1024;

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
            let file = fs::File::open(source).map_err(api_error)?;
            let text = read_text_limited(
                file,
                MAX_UPDATE_JSON_BYTES,
                "update_response_too_large",
                "update JSON file",
            )?;
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
    let response = http_agent()
        .get(url)
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(api_error)?;
    let text = read_text_limited(
        response.into_reader(),
        MAX_UPDATE_JSON_BYTES,
        "update_response_too_large",
        "update JSON response",
    )?;
    serde_json::from_str(&text).map_err(api_error)
}

fn download_update_archive(root: &Path, package: &UpdatePackage) -> Result<PathBuf, ApiError> {
    let downloads = root.join("update").join("downloads").join(&package.version);
    fs::create_dir_all(&downloads).map_err(api_error)?;
    let path = downloads.join(&package.asset_name);
    let tmp_path = downloads.join(format!("{}.tmp", package.asset_name));
    let response = http_agent()
        .get(&package.download_url)
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(api_error)?;
    if let Some(content_length) = response.header("Content-Length") {
        let size = content_length.trim().parse::<u64>().map_err(api_error)?;
        if size != package.size {
            return Err(api_error_message(
                "update_archive_size_mismatch",
                format!(
                    "update archive content length mismatch: expected {}, got {size}",
                    package.size
                ),
            ));
        }
    }
    let mut reader = response.into_reader();
    let mut file = fs::File::create(&tmp_path).map_err(api_error)?;
    let result = copy_limited(&mut reader, &mut file, package.size);
    if let Err(error) = result {
        let _ = fs::remove_file(&tmp_path);
        return Err(error);
    }
    if let Err(error) = file.flush() {
        let _ = fs::remove_file(&tmp_path);
        return Err(api_error(error));
    }
    if let Err(error) = fs::rename(&tmp_path, &path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(api_error(error));
    }
    Ok(path)
}

fn http_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(HTTP_CONNECT_TIMEOUT)
        .timeout_read(HTTP_READ_TIMEOUT)
        .timeout_write(HTTP_WRITE_TIMEOUT)
        .build()
}

fn read_text_limited(
    mut reader: impl Read,
    max_bytes: u64,
    error_code: &str,
    label: &str,
) -> Result<String, ApiError> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; READ_BUFFER_BYTES];
    loop {
        let read = reader.read(&mut buffer).map_err(api_error)?;
        if read == 0 {
            break;
        }
        let next_len = bytes.len() as u64 + read as u64;
        if next_len > max_bytes {
            return Err(api_error_message(
                error_code,
                format!("{label} exceeds {max_bytes} bytes"),
            ));
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    String::from_utf8(bytes).map_err(api_error)
}

fn copy_limited(
    reader: &mut impl Read,
    writer: &mut impl Write,
    max_bytes: u64,
) -> Result<(), ApiError> {
    let mut written = 0_u64;
    let mut buffer = [0_u8; READ_BUFFER_BYTES];
    loop {
        let read = reader.read(&mut buffer).map_err(api_error)?;
        if read == 0 {
            break;
        }
        written += read as u64;
        if written > max_bytes {
            return Err(api_error_message(
                "update_archive_too_large",
                format!("update archive exceeds {max_bytes} bytes"),
            ));
        }
        writer.write_all(&buffer[..read]).map_err(api_error)?;
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn read_text_limited_rejects_large_json_response() {
        let bytes = vec![b'a'; 8];
        let error = read_text_limited(
            Cursor::new(bytes),
            7,
            "update_response_too_large",
            "update JSON response",
        )
        .expect_err("oversized response should fail");

        assert_eq!(
            api_error_code(error).as_deref(),
            Some("update_response_too_large")
        );
    }

    #[test]
    fn copy_limited_rejects_archive_larger_than_expected() {
        let mut reader = Cursor::new(vec![1_u8; 9]);
        let mut writer = Vec::new();

        let error =
            copy_limited(&mut reader, &mut writer, 8).expect_err("oversized archive should fail");

        assert_eq!(
            api_error_code(error).as_deref(),
            Some("update_archive_too_large")
        );
    }

    #[test]
    fn copy_limited_accepts_exact_expected_archive_size() {
        let bytes = vec![1_u8, 2, 3, 4];
        let mut reader = Cursor::new(bytes.clone());
        let mut writer = Vec::new();

        copy_limited(&mut reader, &mut writer, bytes.len() as u64)
            .expect("exact archive size should copy");

        assert_eq!(writer, bytes);
    }

    #[test]
    fn copy_limited_allows_short_archive_for_stage_validation() {
        let bytes = vec![1_u8, 2];
        let mut reader = Cursor::new(bytes.clone());
        let mut writer = Vec::new();

        copy_limited(&mut reader, &mut writer, 4).expect("short archive should copy");

        assert_eq!(writer, bytes);
    }

    fn api_error_code(error: ApiError) -> Option<String> {
        serde_json::to_value(error)
            .ok()?
            .get("code")?
            .as_str()
            .map(str::to_string)
    }
}
