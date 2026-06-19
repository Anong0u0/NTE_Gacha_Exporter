use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use nte_assets::{read_zip_manifest, validate_manifest_shape};
use nte_core::{
    AssetsPackCheckReport, AssetsPackInstallReport, AssetsPackManifest, AssetsPackPackage,
    AssetsPackStatus, GuiError, UpdateChannel, bundled_maps_hash,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::State;

use crate::error::{ApiError, api_error, api_error_message};
use crate::state::{AppState, with_store};

const GITHUB_RELEASES_API: &str =
    "https://api.github.com/repos/Anong0u0/nte_gacha_exporter/releases";
const ASSETS_PACK_MANIFEST_ASSET: &str = "nte-assets-pack-manifest.json";
const USER_AGENT: &str = "nte-gacha-exporter-assets-pack";
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const HTTP_READ_TIMEOUT: Duration = Duration::from_secs(60);
const HTTP_WRITE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RELEASE_JSON_BYTES: u64 = 1024 * 1024;
const READ_BUFFER_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct AssetResolveRequest {
    pub(crate) asset_ref: String,
    pub(crate) kind: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AssetResolveResult {
    pub(crate) asset_ref: String,
    pub(crate) kind: Option<String>,
    pub(crate) url: Option<String>,
}

#[tauri::command]
pub(crate) fn assets_pack_status(state: State<'_, AppState>) -> Result<AssetsPackStatus, ApiError> {
    let root = root_from_state(&state)?;
    Ok(status_for_root(&root))
}

#[tauri::command]
pub(crate) fn assets_pack_check(
    state: State<'_, AppState>,
    channel: Option<String>,
) -> Result<AssetsPackCheckReport, ApiError> {
    let root = root_from_state(&state)?;
    let status = status_for_root(&root);
    let requested_channel = update_channel_or_settings(&state, channel)?;
    let package = match fetch_assets_pack_package(requested_channel) {
        Ok(package) if package_matches_current(&package) => Some(package),
        Ok(_) => None,
        Err(error) => return Err(error),
    };
    Ok(AssetsPackCheckReport {
        current_app_version: app_version().to_string(),
        expected_map_hash: bundled_maps_hash(),
        channel: requested_channel,
        installed: status.installed,
        compatible: status.compatible,
        package,
    })
}

#[tauri::command]
pub(crate) fn assets_pack_download_and_install(
    state: State<'_, AppState>,
    package: AssetsPackPackage,
) -> Result<AssetsPackInstallReport, ApiError> {
    if !package_matches_current(&package) {
        return Err(api_error_message(
            "assets_pack_incompatible",
            "assets pack does not match current app version and bundled maps",
        ));
    }
    let root = root_from_state(&state)?;
    let archive_path = download_assets_pack(&root, &package)?;
    install_assets_pack(&root, &package, &archive_path)
}

#[tauri::command]
pub(crate) fn assets_pack_remove(state: State<'_, AppState>) -> Result<AssetsPackStatus, ApiError> {
    let root = root_from_state(&state)?;
    let current = current_dir(&root);
    if current.exists() {
        let disabled = assets_pack_root(&root).join(format!("disabled-{}", timestamp_millis()));
        fs::rename(&current, disabled).map_err(api_error)?;
    }
    Ok(status_for_root(&root))
}

#[tauri::command]
pub(crate) fn assets_resolve_refs(
    state: State<'_, AppState>,
    refs: Vec<AssetResolveRequest>,
) -> Result<Vec<AssetResolveResult>, ApiError> {
    let root = root_from_state(&state)?;
    let Some(manifest) = compatible_manifest(&root) else {
        return Ok(refs
            .into_iter()
            .map(|item| AssetResolveResult {
                asset_ref: item.asset_ref,
                kind: item.kind,
                url: None,
            })
            .collect());
    };

    let mut exact = BTreeMap::new();
    let mut by_ref = BTreeMap::new();
    for asset in manifest.assets {
        let url = asset_url(&asset.pack_path);
        exact.insert((asset.asset_ref.clone(), asset.kind.clone()), url.clone());
        by_ref.entry(asset.asset_ref).or_insert(url);
    }

    Ok(refs
        .into_iter()
        .map(|item| {
            let url = item
                .kind
                .as_ref()
                .and_then(|kind| exact.get(&(item.asset_ref.clone(), kind.clone())).cloned())
                .or_else(|| by_ref.get(&item.asset_ref).cloned());
            AssetResolveResult {
                asset_ref: item.asset_ref,
                kind: item.kind,
                url,
            }
        })
        .collect())
}

pub(crate) fn assets_protocol_response(
    root: &Path,
    request: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    match resolve_protocol_path(root, request.uri().path()) {
        Ok(path) => match fs::read(path) {
            Ok(bytes) => response(200, "image/webp", bytes),
            Err(_) => response(
                404,
                "text/plain; charset=utf-8",
                b"asset not found".to_vec(),
            ),
        },
        Err(message) => response(400, "text/plain; charset=utf-8", message.into_bytes()),
    }
}

fn root_from_state(state: &State<'_, AppState>) -> Result<PathBuf, ApiError> {
    with_store(state, |store| Ok(store.root().to_path_buf()))
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

fn status_for_root(root: &Path) -> AssetsPackStatus {
    let manifest = installed_manifest(root).ok();
    let compatible = manifest.as_ref().is_some_and(manifest_matches_current);
    AssetsPackStatus {
        installed: manifest.is_some(),
        compatible,
        current_app_version: app_version().to_string(),
        expected_map_hash: bundled_maps_hash(),
        installed_app_version: manifest.as_ref().map(|item| item.app_version.clone()),
        installed_map_hash: manifest.as_ref().map(|item| item.map_hash.clone()),
        source_commit: manifest.as_ref().map(|item| item.source_commit.clone()),
        file_count: manifest.as_ref().map_or(0, |item| item.file_count),
        install_path: current_dir(root).display().to_string(),
    }
}

fn compatible_manifest(root: &Path) -> Option<AssetsPackManifest> {
    installed_manifest(root)
        .ok()
        .filter(manifest_matches_current)
}

fn installed_manifest(root: &Path) -> Result<AssetsPackManifest, ApiError> {
    let file = fs::File::open(current_dir(root).join("manifest.json")).map_err(api_error)?;
    let manifest: AssetsPackManifest = serde_json::from_reader(file).map_err(api_error)?;
    validate_manifest_shape(&manifest).map_err(api_error)?;
    Ok(manifest)
}

fn manifest_matches_current(manifest: &AssetsPackManifest) -> bool {
    manifest.app_version == app_version() && manifest.map_hash == bundled_maps_hash()
}

fn package_matches_current(package: &AssetsPackPackage) -> bool {
    package.app_version == app_version() && package.map_hash == bundled_maps_hash()
}

fn fetch_assets_pack_package(channel: UpdateChannel) -> Result<AssetsPackPackage, ApiError> {
    if let Ok(source) = std::env::var("NTE_GACHA_EXPORTER_ASSETS_PACK_MANIFEST") {
        if !source.trim().is_empty() {
            if source.starts_with("http://") || source.starts_with("https://") {
                return http_get_json(&source);
            }
            let file = fs::File::open(source).map_err(api_error)?;
            let text = read_text_limited(file, MAX_RELEASE_JSON_BYTES, "assets pack manifest")?;
            return serde_json::from_str(&text).map_err(api_error);
        }
    }
    let release = select_release(channel)?;
    let manifest_url = release
        .assets
        .into_iter()
        .find(|asset| asset.name == ASSETS_PACK_MANIFEST_ASSET)
        .map(|asset| asset.browser_download_url)
        .ok_or_else(|| {
            api_error_message(
                "assets_pack_manifest_missing",
                "release assets pack manifest missing",
            )
        })?;
    http_get_json(&manifest_url)
}

fn select_release(channel: UpdateChannel) -> Result<GithubRelease, ApiError> {
    let releases: Vec<GithubRelease> = http_get_json(GITHUB_RELEASES_API)?;
    releases
        .into_iter()
        .find(|release| !release.draft && (channel == UpdateChannel::Beta || !release.prerelease))
        .ok_or_else(|| {
            api_error_message(
                "assets_pack_release_missing",
                "no matching GitHub release found",
            )
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
        MAX_RELEASE_JSON_BYTES,
        "assets pack JSON response",
    )?;
    serde_json::from_str(&text).map_err(api_error)
}

fn download_assets_pack(root: &Path, package: &AssetsPackPackage) -> Result<PathBuf, ApiError> {
    let downloads = assets_pack_root(root).join("downloads").join(format!(
        "{}-{}",
        package.app_version,
        short_hash(&package.map_hash)
    ));
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
                "assets_pack_size_mismatch",
                format!(
                    "assets pack content length mismatch: expected {}, got {size}",
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
    if path.exists() {
        fs::remove_file(&path).map_err(api_error)?;
    }
    if let Err(error) = fs::rename(&tmp_path, &path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(api_error(error));
    }
    Ok(path)
}

fn install_assets_pack(
    root: &Path,
    package: &AssetsPackPackage,
    archive_path: &Path,
) -> Result<AssetsPackInstallReport, ApiError> {
    let actual_hash = sha256_file(archive_path).map_err(api_error)?;
    if !actual_hash.eq_ignore_ascii_case(&package.sha256) {
        return Err(api_error_message(
            "assets_pack_sha256_mismatch",
            format!(
                "assets pack sha256 mismatch: expected {}, got {actual_hash}",
                package.sha256
            ),
        ));
    }

    let file = fs::File::open(archive_path).map_err(api_error)?;
    let mut zip = zip::ZipArchive::new(file).map_err(api_error)?;
    let manifest = read_zip_manifest(&mut zip).map_err(api_error)?;
    validate_pack_manifest(&manifest, package)?;

    let expected = manifest
        .assets
        .iter()
        .map(|asset| (asset.pack_path.clone(), asset.sha256.clone()))
        .collect::<BTreeMap<_, _>>();
    validate_zip_entries(&mut zip, &expected)?;

    let pack_root = assets_pack_root(root);
    fs::create_dir_all(&pack_root).map_err(api_error)?;
    let staging = pack_root
        .join("staging")
        .join(timestamp_millis().to_string());
    fs::create_dir_all(staging.join("assets")).map_err(api_error)?;
    fs::write(
        staging.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).map_err(api_error)?,
    )
    .map_err(api_error)?;

    for (pack_path, expected_hash) in &expected {
        let bytes = read_zip_entry_limited(&mut zip, pack_path, package.size)?;
        let actual_hash = sha256_bytes(&bytes);
        if !actual_hash.eq_ignore_ascii_case(expected_hash) {
            return Err(api_error_message(
                "assets_pack_asset_sha256_mismatch",
                format!("assets pack asset sha256 mismatch: {pack_path}"),
            ));
        }
        let output = staging.join(pack_path);
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(api_error)?;
        }
        fs::write(output, bytes).map_err(api_error)?;
    }

    let current = current_dir(root);
    if current.exists() {
        let previous = pack_root.join(format!("previous-{}", timestamp_millis()));
        fs::rename(&current, previous).map_err(api_error)?;
    }
    fs::rename(&staging, &current).map_err(api_error)?;

    Ok(AssetsPackInstallReport {
        app_version: manifest.app_version,
        map_hash: manifest.map_hash,
        source_commit: manifest.source_commit,
        file_count: manifest.file_count,
        install_path: current.display().to_string(),
    })
}

fn validate_pack_manifest(
    manifest: &AssetsPackManifest,
    package: &AssetsPackPackage,
) -> Result<(), ApiError> {
    if !manifest_matches_current(manifest) {
        return Err(api_error_message(
            "assets_pack_incompatible",
            "assets pack manifest does not match current app version and bundled maps",
        ));
    }
    if manifest.file_count != package.file_count
        || manifest.source_commit != package.source_commit
        || manifest.map_hash != package.map_hash
        || manifest.app_version != package.app_version
    {
        return Err(api_error_message(
            "assets_pack_manifest_mismatch",
            "assets pack manifest does not match release metadata",
        ));
    }
    Ok(())
}

fn validate_zip_entries(
    zip: &mut zip::ZipArchive<fs::File>,
    expected: &BTreeMap<String, String>,
) -> Result<(), ApiError> {
    let mut seen = BTreeSet::new();
    for index in 0..zip.len() {
        let entry = zip.by_index(index).map_err(api_error)?;
        let name = entry.name().to_string();
        if name.ends_with('/') {
            continue;
        }
        if name != "manifest.json" && !expected.contains_key(&name) {
            return Err(api_error_message(
                "assets_pack_unexpected_entry",
                format!("assets pack contains unexpected entry: {name}"),
            ));
        }
        seen.insert(name);
    }
    for pack_path in expected.keys() {
        if !seen.contains(pack_path) {
            return Err(api_error_message(
                "assets_pack_missing_entry",
                format!("assets pack missing entry: {pack_path}"),
            ));
        }
    }
    Ok(())
}

fn read_zip_entry_limited(
    zip: &mut zip::ZipArchive<fs::File>,
    pack_path: &str,
    max_bytes: u64,
) -> Result<Vec<u8>, ApiError> {
    let mut entry = zip.by_name(pack_path).map_err(api_error)?;
    let mut bytes = Vec::new();
    copy_limited(&mut entry, &mut bytes, max_bytes)?;
    Ok(bytes)
}

fn resolve_protocol_path(root: &Path, uri_path: &str) -> Result<PathBuf, String> {
    let path = uri_path.trim_start_matches('/');
    if path.contains("..")
        || path.contains('\\')
        || path.contains(':')
        || !path.starts_with("assets/")
        || !path.ends_with(".webp")
    {
        return Err("invalid asset path".to_string());
    }
    Ok(current_dir(root).join(path))
}

fn asset_url(pack_path: &str) -> String {
    if cfg!(windows) {
        format!("http://nteasset.localhost/{pack_path}")
    } else {
        format!("nteasset://localhost/{pack_path}")
    }
}

fn assets_pack_root(root: &Path) -> PathBuf {
    root.join("data").join("assets-pack")
}

fn current_dir(root: &Path) -> PathBuf {
    assets_pack_root(root).join("current")
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
    _label: &str,
) -> Result<String, ApiError> {
    let mut bytes = Vec::new();
    copy_limited(&mut reader, &mut bytes, max_bytes)?;
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
                "assets_pack_too_large",
                format!("assets pack exceeds {max_bytes} bytes"),
            ));
        }
        writer.write_all(&buffer[..read]).map_err(api_error)?;
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, GuiError> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; READ_BUFFER_BYTES];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or_default()
}

fn short_hash(value: &str) -> &str {
    value.get(..12).unwrap_or(value)
}

fn response(status: u16, content_type: &str, body: Vec<u8>) -> tauri::http::Response<Vec<u8>> {
    tauri::http::Response::builder()
        .status(status)
        .header(tauri::http::header::CONTENT_TYPE, content_type)
        .header(
            tauri::http::header::CACHE_CONTROL,
            "public, max-age=31536000",
        )
        .body(body)
        .expect("asset protocol response must build")
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
