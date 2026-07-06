use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    WINDIVERT_DLL_ENTRY, WINDIVERT_DOWNLOAD_URL, WINDIVERT_LICENSE_ENTRY, WINDIVERT_SYS_ENTRY,
    WINDIVERT_VERSION, WINDIVERT_ZIP_MAX_BYTES, WINDIVERT_ZIP_NAME, WINDIVERT_ZIP_SHA256,
    windivert_unavailable_for_platform,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WinDivertInstallStatus {
    pub platform_supported: bool,
    pub installed: bool,
    pub version: String,
    pub install_dir: String,
    pub dll_path: String,
    pub sys_path: String,
    pub license_path: String,
    pub download_url: String,
    pub zip_sha256: String,
    pub loadable: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WinDivertInstallReport {
    pub status: WinDivertInstallStatus,
    pub downloaded: bool,
    pub verified_sha256: String,
    pub installed_files: Vec<String>,
}

pub fn windivert_install_dir(root: &Path) -> PathBuf {
    root.join("drivers").join("windivert")
}

pub fn windivert_status(root: &Path, check_load: bool) -> WinDivertInstallStatus {
    let install_dir = windivert_install_dir(root);
    let dll_path = install_dir.join("WinDivert.dll");
    let sys_path = install_dir.join("WinDivert64.sys");
    let license_path = install_dir.join("LICENSE");
    let installed = dll_path.is_file() && sys_path.is_file() && license_path.is_file();
    let mut status = WinDivertInstallStatus {
        platform_supported: cfg!(windows),
        installed,
        version: WINDIVERT_VERSION.to_string(),
        install_dir: install_dir.to_string_lossy().to_string(),
        dll_path: dll_path.to_string_lossy().to_string(),
        sys_path: sys_path.to_string_lossy().to_string(),
        license_path: license_path.to_string_lossy().to_string(),
        download_url: WINDIVERT_DOWNLOAD_URL.to_string(),
        zip_sha256: WINDIVERT_ZIP_SHA256.to_string(),
        loadable: false,
        error: None,
    };
    if !status.platform_supported {
        status.error = Some(windivert_unavailable_for_platform());
        return status;
    }
    if !installed {
        status.error = Some("WinDivert is not installed".to_string());
        return status;
    }
    if check_load {
        match windivert_loadable(&install_dir) {
            Ok(()) => status.loadable = true,
            Err(error) => status.error = Some(error),
        }
    }
    status
}

pub fn install_windivert(root: &Path) -> Result<WinDivertInstallReport, String> {
    if !cfg!(windows) {
        return Err(windivert_unavailable_for_platform());
    }
    let install_dir = windivert_install_dir(root);
    fs::create_dir_all(&install_dir)
        .map_err(|error| format!("failed to create WinDivert directory: {error}"))?;
    let zip_path = install_dir.join(format!("{WINDIVERT_ZIP_NAME}.tmp"));
    let result = (|| {
        let hash = download_windivert_zip(&zip_path)?;
        verify_windivert_zip_sha(&hash)?;
        let installed_files = extract_windivert_runtime_files(&zip_path, &install_dir)?;
        Ok(WinDivertInstallReport {
            status: windivert_status(root, false),
            downloaded: true,
            verified_sha256: hash,
            installed_files,
        })
    })();
    let _ = fs::remove_file(&zip_path);
    if result.is_err() {
        cleanup_windivert_extract_temps(&install_dir);
    }
    result
}

#[cfg(windows)]
fn windivert_loadable(install_dir: &Path) -> Result<(), String> {
    super::ffi::WinDivertHandle::open_ip_sniff(Some(install_dir)).map(|handle| {
        handle.shutdown_recv();
        handle.close();
    })
}

#[cfg(not(windows))]
fn windivert_loadable(_install_dir: &Path) -> Result<(), String> {
    Err(windivert_unavailable_for_platform())
}

pub(super) fn verify_windivert_zip_sha(hash: &str) -> Result<(), String> {
    if hash.eq_ignore_ascii_case(WINDIVERT_ZIP_SHA256) {
        return Ok(());
    }
    Err(format!(
        "WinDivert zip sha256 mismatch: expected {WINDIVERT_ZIP_SHA256}, got {hash}"
    ))
}

fn download_windivert_zip(path: &Path) -> Result<String, String> {
    let response = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .timeout_write(Duration::from_secs(30))
        .build()
        .get(WINDIVERT_DOWNLOAD_URL)
        .set("User-Agent", "nte-gacha-exporter-windivert")
        .call()
        .map_err(|error| format!("failed to download WinDivert: {error}"))?;
    let mut reader = response.into_reader();
    let mut file = fs::File::create(path)
        .map_err(|error| format!("failed to write WinDivert zip: {error}"))?;
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("failed to read WinDivert download: {error}"))?;
        if read == 0 {
            break;
        }
        total += read as u64;
        if total > WINDIVERT_ZIP_MAX_BYTES {
            return Err("WinDivert download exceeded expected size".to_string());
        }
        hasher.update(&buffer[..read]);
        file.write_all(&buffer[..read])
            .map_err(|error| format!("failed to write WinDivert zip: {error}"))?;
    }
    file.flush()
        .map_err(|error| format!("failed to flush WinDivert zip: {error}"))?;
    Ok(format!("{:x}", hasher.finalize()))
}

pub(super) fn extract_windivert_runtime_files(
    zip_path: &Path,
    install_dir: &Path,
) -> Result<Vec<String>, String> {
    let file = fs::File::open(zip_path)
        .map_err(|error| format!("failed to open WinDivert zip: {error}"))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|error| format!("failed to read WinDivert zip: {error}"))?;
    let mut installed = Vec::new();
    for (entry_name, file_name) in [
        (WINDIVERT_DLL_ENTRY, "WinDivert.dll"),
        (WINDIVERT_SYS_ENTRY, "WinDivert64.sys"),
        (WINDIVERT_LICENSE_ENTRY, "LICENSE"),
    ] {
        let mut entry = archive
            .by_name(entry_name)
            .map_err(|error| format!("WinDivert zip missing {entry_name}: {error}"))?;
        let target = install_dir.join(file_name);
        let tmp = install_dir.join(format!("{file_name}.tmp"));
        {
            let mut out = fs::File::create(&tmp)
                .map_err(|error| format!("failed to create {}: {error}", tmp.display()))?;
            std::io::copy(&mut entry, &mut out)
                .map_err(|error| format!("failed to extract {entry_name}: {error}"))?;
            out.flush()
                .map_err(|error| format!("failed to flush {}: {error}", tmp.display()))?;
        }
        let _ = fs::remove_file(&target);
        fs::rename(&tmp, &target).map_err(|error| {
            let _ = fs::remove_file(&tmp);
            format!("failed to install {}: {error}", target.display())
        })?;
        installed.push(target.to_string_lossy().to_string());
    }
    Ok(installed)
}

pub(super) fn cleanup_windivert_extract_temps(install_dir: &Path) {
    for file_name in ["WinDivert.dll.tmp", "WinDivert64.sys.tmp", "LICENSE.tmp"] {
        let _ = fs::remove_file(install_dir.join(file_name));
    }
    let _ = fs::remove_file(install_dir.join(format!("{WINDIVERT_ZIP_NAME}.tmp")));
}
