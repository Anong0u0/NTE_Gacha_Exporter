use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use semver::Version;
use sha2::{Digest, Sha256};
use zip::ZipArchive;

use crate::model::{
    GuiError, UpdateChannel, UpdateCheckReport, UpdateInstallPlan, UpdateManifest, UpdatePackage,
    UpdateStageReport, UpdateStatus,
};

const MANIFEST_SCHEMA: &str = "nte-gacha-update";
const MANIFEST_SCHEMA_VERSION: u32 = 1;
const ROOT_LAUNCHER: &str = "nte-gacha.exe";
const ROOT_CLI: &str = "nte-gacha-cli.exe";
const APP_DIR: &str = "app";
const LEGACY_SIDECAR_DIR: &str = "sidecars";
const DATA_DIR: &str = "data";
const UPDATE_DIR: &str = "update";
const APP_EXE: &str = "nte-gacha-desktop.exe";
const UPDATER_EXE: &str = "nte-gacha-updater.exe";
const RELEASE_JSON: &str = "release.json";
const RELEASE_SCHEMA: &str = "nte-gacha-release";
const RELEASE_SCHEMA_VERSION: u32 = 1;

pub fn check_update_manifest(
    manifest: UpdateManifest,
    current_version: &str,
    requested_channel: UpdateChannel,
) -> Result<UpdateCheckReport, GuiError> {
    validate_manifest(&manifest)?;
    let current = parse_version(current_version)?;
    let next = parse_version(&manifest.version)?;
    let available = is_manifest_allowed(&manifest, requested_channel) && next > current;
    Ok(UpdateCheckReport {
        current_version: current_version.to_string(),
        channel: requested_channel,
        available,
        package: available.then(|| package_from_manifest(manifest)),
    })
}

pub fn update_status(
    root: impl AsRef<Path>,
    current_version: &str,
) -> Result<UpdateStatus, GuiError> {
    let root = root.as_ref();
    Ok(UpdateStatus {
        portable_root: root.to_string_lossy().to_string(),
        current_version: current_version.to_string(),
        supported_layout: is_supported_portable_layout(root),
        staged_version: latest_child_name(&root.join(UPDATE_DIR).join("staging"))?,
        rollback_version: latest_child_name(&root.join(UPDATE_DIR).join("rollback"))?,
    })
}

pub fn stage_update_archive(
    root: impl AsRef<Path>,
    package: &UpdatePackage,
    archive_path: impl AsRef<Path>,
) -> Result<UpdateStageReport, GuiError> {
    validate_package(package)?;
    let root = root.as_ref();
    let archive_path = archive_path.as_ref();
    if !archive_path.is_file() {
        return Err(GuiError::InvalidUpdate(format!(
            "update archive not found: {}",
            archive_path.to_string_lossy()
        )));
    }
    let actual_size = fs::metadata(archive_path)?.len();
    if actual_size != package.size {
        return Err(GuiError::InvalidUpdate(format!(
            "update archive size mismatch: expected {}, got {}",
            package.size, actual_size
        )));
    }
    let actual_hash = sha256_file(archive_path)?;
    if !actual_hash.eq_ignore_ascii_case(&package.sha256) {
        return Err(GuiError::InvalidUpdate(format!(
            "update archive sha256 mismatch: expected {}, got {}",
            package.sha256, actual_hash
        )));
    }

    let staging = root.join(UPDATE_DIR).join("staging").join(&package.version);
    clear_scoped_dir(&staging)?;
    let extract_root = staging.join("extract");
    let payload = staging.join("payload");
    fs::create_dir_all(&extract_root)?;
    extract_zip_checked(archive_path, &extract_root)?;
    let source_root = normalized_payload_root(&extract_root)?;
    validate_payload_layout(&source_root)?;
    validate_payload_release(&source_root, &package.version)?;
    validate_no_payload_data_files(&source_root)?;
    fs::rename(&source_root, &payload)?;

    let staged_helper = staging.join(UPDATER_EXE);
    fs::copy(payload.join(APP_DIR).join(UPDATER_EXE), staged_helper)?;
    let _ = fs::remove_dir(&extract_root);

    Ok(UpdateStageReport {
        package: package.clone(),
        archive_path: archive_path.to_string_lossy().to_string(),
        staging_path: staging.to_string_lossy().to_string(),
    })
}

pub fn prepare_update_install(
    root: impl AsRef<Path>,
    version: &str,
) -> Result<UpdateInstallPlan, GuiError> {
    let root = root.as_ref();
    let version = validate_version_string(version)?;
    let staging = root.join(UPDATE_DIR).join("staging").join(&version);
    let payload = staging.join("payload");
    validate_payload_layout(&payload)?;
    validate_payload_release(&payload, &version)?;
    let helper = staging.join(UPDATER_EXE);
    if !helper.is_file() {
        return Err(GuiError::InvalidUpdate(format!(
            "staged updater helper missing: {}",
            helper.to_string_lossy()
        )));
    }
    Ok(UpdateInstallPlan {
        root: root.to_string_lossy().to_string(),
        version,
        staging_path: staging.to_string_lossy().to_string(),
        helper_path: helper.to_string_lossy().to_string(),
    })
}

pub fn apply_staged_update(root: impl AsRef<Path>, version: &str) -> Result<(), GuiError> {
    let root = root.as_ref();
    let version = validate_version_string(version)?;
    let staging = root.join(UPDATE_DIR).join("staging").join(&version);
    let payload = staging.join("payload");
    validate_payload_layout(&payload)?;
    validate_payload_release(&payload, &version)?;
    validate_existing_data_preserved(root)?;

    let rollback = root.join(UPDATE_DIR).join("rollback").join(format!(
        "{}-{}",
        current_release_label(root),
        now_unique_stamp()
    ));
    fs::create_dir_all(&rollback)?;

    let apply_result =
        backup_current_release(root, &rollback).and_then(|()| install_payload(root, &payload));
    match apply_result {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = restore_rollback(root, &rollback);
            Err(error)
        }
    }
}

fn validate_manifest(manifest: &UpdateManifest) -> Result<(), GuiError> {
    if manifest.schema != MANIFEST_SCHEMA || manifest.schema_version != MANIFEST_SCHEMA_VERSION {
        return Err(GuiError::InvalidUpdate(
            "update manifest schema must be nte-gacha-update v1".to_string(),
        ));
    }
    validate_package(&package_from_manifest(manifest.clone()))?;
    if manifest.release_url.trim().is_empty() || manifest.download_url.trim().is_empty() {
        return Err(GuiError::InvalidUpdate(
            "update manifest URLs must be non-empty".to_string(),
        ));
    }
    Ok(())
}

fn validate_package(package: &UpdatePackage) -> Result<(), GuiError> {
    validate_version_string(&package.version)?;
    if package.asset_name.trim().is_empty() {
        return Err(GuiError::InvalidUpdate(
            "update asset_name must be non-empty".to_string(),
        ));
    }
    if package.size == 0 {
        return Err(GuiError::InvalidUpdate(
            "update archive size must be greater than zero".to_string(),
        ));
    }
    if package.sha256.len() != 64 || !package.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(GuiError::InvalidUpdate(
            "update sha256 must be 64 hex characters".to_string(),
        ));
    }
    Ok(())
}

fn package_from_manifest(manifest: UpdateManifest) -> UpdatePackage {
    UpdatePackage {
        version: manifest.version,
        channel: manifest.channel,
        release_url: manifest.release_url,
        asset_name: manifest.asset_name,
        download_url: manifest.download_url,
        sha256: manifest.sha256,
        size: manifest.size,
    }
}

fn is_manifest_allowed(manifest: &UpdateManifest, requested_channel: UpdateChannel) -> bool {
    match requested_channel {
        UpdateChannel::Stable => {
            manifest.channel == UpdateChannel::Stable
                && parse_version(&manifest.version).is_ok_and(|version| version.pre.is_empty())
        }
        UpdateChannel::Beta => true,
    }
}

fn parse_version(value: &str) -> Result<Version, GuiError> {
    Version::parse(value)
        .map_err(|_| GuiError::InvalidUpdate(format!("invalid update version: {value}")))
}

fn validate_version_string(value: &str) -> Result<String, GuiError> {
    let value = value.trim();
    parse_version(value)?;
    Ok(value.to_string())
}

fn sha256_file(path: &Path) -> Result<String, GuiError> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn extract_zip_checked(archive_path: &Path, target: &Path) -> Result<(), GuiError> {
    let file = fs::File::open(archive_path)?;
    let mut zip = ZipArchive::new(file)?;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index)?;
        let Some(safe_name) = safe_zip_path(entry.name())? else {
            continue;
        };
        if safe_name.components().next().is_some_and(
            |component| matches!(component, Component::Normal(name) if name == DATA_DIR),
        ) && entry.is_file()
        {
            return Err(GuiError::InvalidUpdate(format!(
                "update package must not contain data files: {}",
                entry.name()
            )));
        }
        let output = target.join(safe_name);
        if entry.is_dir() {
            fs::create_dir_all(output)?;
        } else {
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut output_file = fs::File::create(output)?;
            std::io::copy(&mut entry, &mut output_file)?;
            output_file.flush()?;
        }
    }
    Ok(())
}

fn safe_zip_path(name: &str) -> Result<Option<PathBuf>, GuiError> {
    if name.trim().is_empty() {
        return Ok(None);
    }
    if name.contains('\\') || name.starts_with('/') || name.contains(':') {
        return Err(GuiError::InvalidUpdate(format!(
            "invalid update zip path: {name}"
        )));
    }
    let path = PathBuf::from(name);
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            _ => {
                return Err(GuiError::InvalidUpdate(format!(
                    "invalid update zip path: {name}"
                )));
            }
        }
    }
    Ok((!clean.as_os_str().is_empty()).then_some(clean))
}

fn normalized_payload_root(extract_root: &Path) -> Result<PathBuf, GuiError> {
    if validate_payload_layout(extract_root).is_ok() {
        return Ok(extract_root.to_path_buf());
    }
    let entries = fs::read_dir(extract_root)?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    if entries.len() == 1 && validate_payload_layout(&entries[0]).is_ok() {
        return Ok(entries[0].clone());
    }
    Err(GuiError::InvalidUpdate(
        "update package payload layout is invalid".to_string(),
    ))
}

fn validate_payload_layout(payload: &Path) -> Result<(), GuiError> {
    for path in [
        payload.join(ROOT_LAUNCHER),
        payload.join(ROOT_CLI),
        payload.join(APP_DIR),
        payload.join(APP_DIR).join(APP_EXE),
        payload.join(APP_DIR).join(UPDATER_EXE),
        payload.join(APP_DIR).join(RELEASE_JSON),
    ] {
        if !path.exists() {
            return Err(GuiError::InvalidUpdate(format!(
                "update package missing required path: {}",
                path.to_string_lossy()
            )));
        }
    }
    validate_no_dev_sidecar_artifacts(payload)?;
    if payload.join(DATA_DIR).is_file() {
        return Err(GuiError::InvalidUpdate(
            "update package data path must not be a file".to_string(),
        ));
    }
    Ok(())
}

fn validate_no_dev_sidecar_artifacts(payload: &Path) -> Result<(), GuiError> {
    let sidecars = payload.join(LEGACY_SIDECAR_DIR);
    if !sidecars.exists() {
        return Ok(());
    }
    if sidecars.join("nte-gacha-python-core.cmd").exists() {
        return Err(GuiError::InvalidUpdate(
            "update package must not contain development sidecar command files".to_string(),
        ));
    }
    for path in walk_files(&sidecars)? {
        if is_text_launcher_or_script(&path) && file_contains_dev_magic(&path)? {
            return Err(GuiError::InvalidUpdate(format!(
                "update package must not contain .local development paths: {}",
                path.to_string_lossy()
            )));
        }
    }
    Ok(())
}

fn is_text_launcher_or_script(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "bat" | "cmd" | "ps1" | "sh" | "txt"
            )
        })
}

fn file_contains_dev_magic(path: &Path) -> Result<bool, GuiError> {
    let bytes = fs::read(path)?;
    Ok(bytes
        .windows(b".local".len())
        .any(|window| window == b".local"))
}

fn validate_payload_release(payload: &Path, expected_version: &str) -> Result<(), GuiError> {
    let path = payload.join(APP_DIR).join(RELEASE_JSON);
    let text = fs::read_to_string(&path)?;
    let value: serde_json::Value = serde_json::from_str(&text)?;
    if value.get("schema").and_then(serde_json::Value::as_str) != Some(RELEASE_SCHEMA)
        || value
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            != Some(u64::from(RELEASE_SCHEMA_VERSION))
    {
        return Err(GuiError::InvalidUpdate(
            "update package release schema must be nte-gacha-release v1".to_string(),
        ));
    }
    let actual_version = value
        .get("version")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            GuiError::InvalidUpdate("update package release version must be a string".to_string())
        })?;
    if actual_version != expected_version {
        return Err(GuiError::InvalidUpdate(format!(
            "update package release version mismatch: expected {expected_version}, got {actual_version}"
        )));
    }
    Ok(())
}

fn validate_no_payload_data_files(payload: &Path) -> Result<(), GuiError> {
    let data = payload.join(DATA_DIR);
    if !data.exists() {
        return Ok(());
    }
    if data.is_file() {
        return Err(GuiError::InvalidUpdate(
            "update package data path must not be a file".to_string(),
        ));
    }
    if let Some(entry) = walk_files(&data)?.into_iter().next() {
        return Err(GuiError::InvalidUpdate(format!(
            "update package must not contain data files: {}",
            entry.to_string_lossy()
        )));
    }
    Ok(())
}

fn walk_files(path: &Path) -> Result<Vec<PathBuf>, GuiError> {
    let mut files = Vec::new();
    if !path.exists() {
        return Ok(files);
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            files.extend(walk_files(&entry.path())?);
        } else if file_type.is_file() {
            files.push(entry.path());
        }
    }
    Ok(files)
}

fn is_supported_portable_layout(root: &Path) -> bool {
    root.join(ROOT_LAUNCHER).is_file() && root.join(APP_DIR).join(APP_EXE).is_file()
}

fn latest_child_name(path: &Path) -> Result<Option<String>, GuiError> {
    if !path.exists() {
        return Ok(None);
    }
    let mut names = fs::read_dir(path)?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    names.sort();
    Ok(names.pop())
}

fn clear_scoped_dir(path: &Path) -> Result<(), GuiError> {
    if path.exists() {
        if !path
            .components()
            .any(|component| matches!(component, Component::Normal(part) if part == "staging"))
        {
            return Err(GuiError::InvalidUpdate(format!(
                "refusing to clear non-staging path: {}",
                path.to_string_lossy()
            )));
        }
        fs::remove_dir_all(path)?;
    }
    fs::create_dir_all(path)?;
    Ok(())
}

fn validate_existing_data_preserved(root: &Path) -> Result<(), GuiError> {
    if root.join(DATA_DIR).is_file() {
        return Err(GuiError::InvalidUpdate(
            "portable data path must not be a file".to_string(),
        ));
    }
    Ok(())
}

fn backup_current_release(root: &Path, rollback: &Path) -> Result<(), GuiError> {
    for name in [ROOT_LAUNCHER, ROOT_CLI, APP_DIR, LEGACY_SIDECAR_DIR] {
        let source = root.join(name);
        if source.exists() {
            fs::rename(source, rollback.join(name))?;
        }
    }
    Ok(())
}

fn install_payload(root: &Path, payload: &Path) -> Result<(), GuiError> {
    for name in [ROOT_LAUNCHER, ROOT_CLI, APP_DIR] {
        fs::rename(payload.join(name), root.join(name))?;
    }
    Ok(())
}

fn restore_rollback(root: &Path, rollback: &Path) -> Result<(), GuiError> {
    for name in [ROOT_LAUNCHER, ROOT_CLI, APP_DIR, LEGACY_SIDECAR_DIR] {
        let target = root.join(name);
        if target.exists() {
            remove_path(&target)?;
        }
        let source = rollback.join(name);
        if source.exists() {
            fs::rename(source, target)?;
        }
    }
    Ok(())
}

fn remove_path(path: &Path) -> Result<(), GuiError> {
    if path.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn current_release_label(root: &Path) -> String {
    let path = root.join(APP_DIR).join(RELEASE_JSON);
    let Ok(text) = fs::read_to_string(path) else {
        return "unknown".to_string();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return "unknown".to_string();
    };
    value
        .get("version")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("unknown")
        .to_string()
}

fn now_unique_stamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| format!("{}-{:09}", value.as_secs(), value.subsec_nanos()))
        .unwrap_or_else(|_| "0-000000000".to_string())
}
