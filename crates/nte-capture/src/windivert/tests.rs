use std::fs;
use std::io::Write;
use std::path::Path;

use super::install::{
    cleanup_windivert_extract_temps, extract_windivert_runtime_files, verify_windivert_zip_sha,
};
use super::*;

#[test]
#[cfg(not(windows))]
fn platform_stub_uses_public_error_code() {
    assert!(windivert_unavailable_for_platform().starts_with(WINDIVERT_UNAVAILABLE_CODE));
}

#[test]
fn error_mapping_mentions_common_setup_failures() {
    assert!(describe_win32_code(2).contains("WinDivert64.sys"));
    assert!(describe_win32_code(5).contains("administrator"));
    assert!(describe_win32_code(577).contains("signature"));
    assert!(describe_win32_code(1257).contains("blocked"));
}

#[test]
fn install_dir_is_stable_root_child() {
    assert_eq!(
        windivert_install_dir(Path::new("root")),
        Path::new("root").join("drivers").join("windivert")
    );
}

#[test]
fn status_reports_missing_install() {
    let temp = tempfile::tempdir().unwrap();
    let status = windivert_status(temp.path(), false);
    assert!(!status.installed);
    assert_eq!(
        Path::new(&status.install_dir)
            .file_name()
            .and_then(|name| name.to_str()),
        Some("windivert")
    );
}

#[test]
fn metadata_is_fixed_to_official_release() {
    assert_eq!(WINDIVERT_VERSION, "2.2.2-A");
    assert_eq!(
        WINDIVERT_DOWNLOAD_URL,
        "https://github.com/basil00/WinDivert/releases/download/v2.2.2/WinDivert-2.2.2-A.zip"
    );
    assert_eq!(
        WINDIVERT_ZIP_SHA256,
        "63cb41763bb4b20f600b6de04e991a9c2be73279e317d4d82f237b150c5f3f15"
    );
}

#[test]
fn zip_sha_mismatch_is_reported() {
    let error = verify_windivert_zip_sha("bad").unwrap_err();

    assert!(error.contains("sha256 mismatch"));
    assert!(error.contains(WINDIVERT_ZIP_SHA256));
}

#[test]
fn extract_installs_only_runtime_files_and_overwrites() {
    let temp = tempfile::tempdir().unwrap();
    let zip_path = temp.path().join("windivert.zip");
    write_test_zip(
        &zip_path,
        &[
            (WINDIVERT_DLL_ENTRY, b"dll-v2".as_slice()),
            (WINDIVERT_SYS_ENTRY, b"sys-v2".as_slice()),
            (WINDIVERT_LICENSE_ENTRY, b"license-v2".as_slice()),
            ("WinDivert-2.2.2-A/README.md", b"ignore".as_slice()),
        ],
    );
    let install_dir = temp.path().join("drivers").join("windivert");
    fs::create_dir_all(&install_dir).unwrap();
    fs::write(install_dir.join("WinDivert.dll"), b"old").unwrap();

    let installed = extract_windivert_runtime_files(&zip_path, &install_dir).unwrap();

    assert_eq!(installed.len(), 3);
    assert_eq!(
        fs::read(install_dir.join("WinDivert.dll")).unwrap(),
        b"dll-v2"
    );
    assert_eq!(
        fs::read(install_dir.join("WinDivert64.sys")).unwrap(),
        b"sys-v2"
    );
    assert_eq!(
        fs::read(install_dir.join("LICENSE")).unwrap(),
        b"license-v2"
    );
    assert!(!install_dir.join("README.md").exists());
    assert!(!install_dir.join("WinDivert.dll.tmp").exists());
}

#[test]
fn extract_rejects_missing_runtime_file() {
    let temp = tempfile::tempdir().unwrap();
    let zip_path = temp.path().join("windivert.zip");
    write_test_zip(&zip_path, &[(WINDIVERT_DLL_ENTRY, b"dll".as_slice())]);

    let error = extract_windivert_runtime_files(&zip_path, temp.path()).unwrap_err();

    assert!(error.contains(WINDIVERT_SYS_ENTRY));
}

#[test]
fn cleanup_extract_temps_removes_only_windivert_temp_files() {
    let temp = tempfile::tempdir().unwrap();
    let install_dir = temp.path().join("drivers").join("windivert");
    fs::create_dir_all(&install_dir).unwrap();
    for name in [
        "WinDivert.dll.tmp",
        "WinDivert64.sys.tmp",
        "LICENSE.tmp",
        "WinDivert-2.2.2-A.zip.tmp",
    ] {
        fs::write(install_dir.join(name), b"tmp").unwrap();
    }
    fs::write(install_dir.join("notes.tmp"), b"keep").unwrap();
    fs::write(install_dir.join("WinDivert.dll"), b"keep").unwrap();

    cleanup_windivert_extract_temps(&install_dir);

    assert!(!install_dir.join("WinDivert.dll.tmp").exists());
    assert!(!install_dir.join("WinDivert64.sys.tmp").exists());
    assert!(!install_dir.join("LICENSE.tmp").exists());
    assert!(!install_dir.join("WinDivert-2.2.2-A.zip.tmp").exists());
    assert_eq!(fs::read(install_dir.join("notes.tmp")).unwrap(), b"keep");
    assert_eq!(
        fs::read(install_dir.join("WinDivert.dll")).unwrap(),
        b"keep"
    );
}

#[test]
fn loader_searches_installed_dir_before_env_and_exe() {
    let installed_dir = Path::new("installed");
    let paths = super::ffi::windivert_search_paths(Some(installed_dir));

    assert_eq!(paths.first(), Some(&installed_dir.join("WinDivert.dll")));
}

fn write_test_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let file = fs::File::create(path).unwrap();
    let mut writer = zip::ZipWriter::new(file);
    let options = zip::write::FileOptions::default();
    for (name, bytes) in entries {
        writer.start_file(*name, options).unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap();
}
