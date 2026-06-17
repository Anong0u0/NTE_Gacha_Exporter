use std::io::{Read, Write};
use std::path::Path;

use serde_json::json;
use sha2::Digest;

use crate::{
    apply_staged_update, check_update_manifest, prepare_update_install, stage_update_archive,
    update_status, UpdateChannel, UpdateManifest, UpdatePackage,
};

fn manifest(version: &str, channel: UpdateChannel) -> UpdateManifest {
    UpdateManifest {
        schema: "nte-gacha-update".to_string(),
        schema_version: 1,
        version: version.to_string(),
        channel,
        release_url: "https://example.invalid/release".to_string(),
        asset_name: format!("nte-gacha-desktop-{version}.zip"),
        download_url: "https://example.invalid/download.zip".to_string(),
        sha256: "0".repeat(64),
        size: 1,
    }
}

fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let file = std::fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::FileOptions::default();
    for (name, bytes) in entries {
        zip.start_file(*name, options).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
}

fn sha256_file(path: &Path) -> String {
    let mut file = std::fs::File::open(path).unwrap();
    let mut hasher = sha2::Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = file.read(&mut buffer).unwrap();
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    format!("{:x}", hasher.finalize())
}

fn package_for_archive(path: &Path, version: &str) -> UpdatePackage {
    UpdatePackage {
        version: version.to_string(),
        channel: UpdateChannel::Stable,
        release_url: "https://example.invalid/release".to_string(),
        asset_name: path.file_name().unwrap().to_string_lossy().to_string(),
        download_url: "https://example.invalid/download.zip".to_string(),
        sha256: sha256_file(path),
        size: std::fs::metadata(path).unwrap().len(),
    }
}

fn portable_entries(prefix: &str, version: &str) -> Vec<(String, Vec<u8>)> {
    vec![
        (format!("{prefix}nte-gacha.exe"), b"launcher".to_vec()),
        (
            format!("{prefix}app/nte-gacha-desktop.exe"),
            b"desktop".to_vec(),
        ),
        (
            format!("{prefix}app/nte-gacha-updater.exe"),
            b"updater".to_vec(),
        ),
        (
            format!("{prefix}app/release.json"),
            json!({
                "schema": "nte-gacha-release",
                "schema_version": 1,
                "version": version
            })
            .to_string()
            .into_bytes(),
        ),
        (
            format!("{prefix}sidecars/nte-gacha-python-core.exe"),
            b"sidecar".to_vec(),
        ),
        (
            format!("{prefix}sidecars/bin/nte-gacha-core.exe"),
            b"python core".to_vec(),
        ),
        (
            format!("{prefix}sidecars/resources/maps/zh-Hant.json"),
            b"{}".to_vec(),
        ),
        (
            format!("{prefix}sidecars/resources/automation/default.json"),
            b"{}".to_vec(),
        ),
    ]
}

fn portable_entries_with_release(
    prefix: &str,
    release: serde_json::Value,
) -> Vec<(String, Vec<u8>)> {
    let mut entries = portable_entries(prefix, "0.2.0");
    let release_path = format!("{prefix}app/release.json");
    let release_bytes = release.to_string().into_bytes();
    for (name, bytes) in &mut entries {
        if name == &release_path {
            *bytes = release_bytes;
            break;
        }
    }
    entries
}

fn write_portable_zip(path: &Path, prefix: &str, version: &str) {
    let entries = portable_entries(prefix, version);
    let borrowed = entries
        .iter()
        .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
        .collect::<Vec<_>>();
    write_zip(path, &borrowed);
}

fn write_portable_zip_with_release(path: &Path, prefix: &str, release: serde_json::Value) {
    let entries = portable_entries_with_release(prefix, release);
    let borrowed = entries
        .iter()
        .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
        .collect::<Vec<_>>();
    write_zip(path, &borrowed);
}

fn create_current_install(root: &Path, version: &str) {
    std::fs::create_dir_all(root.join("app")).unwrap();
    std::fs::create_dir_all(root.join("sidecars")).unwrap();
    std::fs::create_dir_all(root.join("data/profiles/default")).unwrap();
    std::fs::write(root.join("nte-gacha.exe"), b"old launcher").unwrap();
    std::fs::write(root.join("app/nte-gacha-desktop.exe"), b"old app").unwrap();
    std::fs::write(root.join("app/nte-gacha-updater.exe"), b"old updater").unwrap();
    std::fs::write(
        root.join("app/release.json"),
        json!({
            "schema": "nte-gacha-release",
            "schema_version": 1,
            "version": version
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(
        root.join("sidecars/nte-gacha-python-core.exe"),
        b"old sidecar",
    )
    .unwrap();
    std::fs::write(
        root.join("data/profiles/default/records.json"),
        b"keep data",
    )
    .unwrap();
}

#[test]
fn update_manifest_accepts_newer_stable_and_filters_channel() {
    let stable = check_update_manifest(
        manifest("0.2.0", UpdateChannel::Stable),
        "0.1.0",
        UpdateChannel::Stable,
    )
    .unwrap();
    let prerelease = check_update_manifest(
        manifest("0.2.0-beta.1", UpdateChannel::Beta),
        "0.1.0",
        UpdateChannel::Stable,
    )
    .unwrap();
    let beta = check_update_manifest(
        manifest("0.2.0-beta.1", UpdateChannel::Beta),
        "0.1.0",
        UpdateChannel::Beta,
    )
    .unwrap();
    let older = check_update_manifest(
        manifest("0.1.0", UpdateChannel::Stable),
        "0.1.0",
        UpdateChannel::Stable,
    )
    .unwrap();

    assert!(stable.available);
    assert!(!prerelease.available);
    assert!(beta.available);
    assert!(!older.available);
}

#[test]
fn stage_update_accepts_single_root_package_and_prepares_helper() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("install");
    std::fs::create_dir_all(&root).unwrap();
    let archive = tmp.path().join("package.zip");
    write_portable_zip(&archive, "nte-gacha-0.2.0/", "0.2.0");
    let package = package_for_archive(&archive, "0.2.0");

    let report = stage_update_archive(&root, &package, &archive).unwrap();
    let plan = prepare_update_install(&root, "0.2.0").unwrap();

    assert!(Path::new(&report.staging_path)
        .join("payload/app/release.json")
        .exists());
    assert!(Path::new(&plan.helper_path).is_file());
}

#[test]
fn stage_update_rejects_zip_slip_and_data_files() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("install");
    std::fs::create_dir_all(&root).unwrap();
    let bad_path = tmp.path().join("bad-path.zip");
    write_zip(&bad_path, &[("../evil.txt", b"bad")]);
    let bad_path_package = package_for_archive(&bad_path, "0.2.0");
    let data_path = tmp.path().join("data.zip");
    let mut entries = portable_entries("nte-gacha-0.2.0/", "0.2.0");
    entries.push((
        "nte-gacha-0.2.0/data/profiles/default/records.json".to_string(),
        b"must reject".to_vec(),
    ));
    let borrowed = entries
        .iter()
        .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
        .collect::<Vec<_>>();
    write_zip(&data_path, &borrowed);
    let data_package = package_for_archive(&data_path, "0.2.0");

    assert!(stage_update_archive(&root, &bad_path_package, &bad_path).is_err());
    assert!(stage_update_archive(&root, &data_package, &data_path).is_err());
}

#[test]
fn stage_update_rejects_development_sidecar_artifacts() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("install");
    std::fs::create_dir_all(&root).unwrap();
    let archive = tmp.path().join("dev-sidecar.zip");
    let mut entries = portable_entries("nte-gacha-0.2.0/", "0.2.0");
    entries.push((
        "nte-gacha-0.2.0/sidecars/nte-gacha-python-core.cmd".to_string(),
        br#"@echo off
D:\game\nte_tool\nte_gacha_exporter\.local\pybin\python.exe
"#
        .to_vec(),
    ));
    let borrowed = entries
        .iter()
        .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
        .collect::<Vec<_>>();
    write_zip(&archive, &borrowed);
    let package = package_for_archive(&archive, "0.2.0");

    assert!(stage_update_archive(&root, &package, &archive).is_err());
}

#[test]
fn stage_update_rejects_incomplete_sidecar_layout() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("install");
    std::fs::create_dir_all(&root).unwrap();
    let archive = tmp.path().join("incomplete-sidecar.zip");
    let entries = portable_entries("nte-gacha-0.2.0/", "0.2.0")
        .into_iter()
        .filter(|(name, _bytes)| !name.ends_with("sidecars/bin/nte-gacha-core.exe"))
        .collect::<Vec<_>>();
    let borrowed = entries
        .iter()
        .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
        .collect::<Vec<_>>();
    write_zip(&archive, &borrowed);
    let package = package_for_archive(&archive, "0.2.0");

    assert!(stage_update_archive(&root, &package, &archive).is_err());
}

#[test]
fn stage_update_rejects_release_schema_mismatch() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("install");
    std::fs::create_dir_all(&root).unwrap();
    let archive = tmp.path().join("bad-release-schema.zip");
    write_portable_zip_with_release(
        &archive,
        "nte-gacha-0.2.0/",
        json!({
            "schema": "other-release",
            "schema_version": 1,
            "version": "0.2.0"
        }),
    );
    let package = package_for_archive(&archive, "0.2.0");

    assert!(stage_update_archive(&root, &package, &archive).is_err());
}

#[test]
fn stage_update_rejects_release_version_mismatch() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("install");
    std::fs::create_dir_all(&root).unwrap();
    let archive = tmp.path().join("bad-release-version.zip");
    write_portable_zip(&archive, "nte-gacha-0.2.0/", "0.1.0");
    let package = package_for_archive(&archive, "0.2.0");

    assert!(stage_update_archive(&root, &package, &archive).is_err());
}

#[test]
fn prepare_update_install_rejects_staged_release_version_mismatch() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("install");
    std::fs::create_dir_all(&root).unwrap();
    let archive = tmp.path().join("package.zip");
    write_portable_zip(&archive, "", "0.2.0");
    let package = package_for_archive(&archive, "0.2.0");
    stage_update_archive(&root, &package, &archive).unwrap();
    std::fs::write(
        root.join("update/staging/0.2.0/payload/app/release.json"),
        json!({
            "schema": "nte-gacha-release",
            "schema_version": 1,
            "version": "0.1.0"
        })
        .to_string(),
    )
    .unwrap();

    assert!(prepare_update_install(&root, "0.2.0").is_err());
    assert!(apply_staged_update(&root, "0.2.0").is_err());
}

#[test]
fn apply_staged_update_replaces_release_and_preserves_data() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("install");
    create_current_install(&root, "0.1.0");
    let archive = tmp.path().join("package.zip");
    write_portable_zip(&archive, "", "0.2.0");
    let package = package_for_archive(&archive, "0.2.0");
    stage_update_archive(&root, &package, &archive).unwrap();

    apply_staged_update(&root, "0.2.0").unwrap();
    let status = update_status(&root, "0.2.0").unwrap();

    assert_eq!(
        std::fs::read(root.join("nte-gacha.exe")).unwrap(),
        b"launcher"
    );
    assert_eq!(
        std::fs::read(root.join("data/profiles/default/records.json")).unwrap(),
        b"keep data"
    );
    assert!(status.supported_layout);
    assert!(status.rollback_version.is_some());
}
