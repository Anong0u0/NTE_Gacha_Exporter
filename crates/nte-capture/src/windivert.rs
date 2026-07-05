#[cfg(windows)]
use std::ffi::{CString, OsStr, c_void};
use std::fs;
use std::io::{Read, Write};
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[cfg(windows)]
use windows_sys::Win32::Foundation::{
    FreeLibrary, GetLastError, HANDLE, HMODULE, INVALID_HANDLE_VALUE,
};
#[cfg(windows)]
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
#[cfg(windows)]
use windows_sys::core::PCSTR;

#[cfg(windows)]
const WINDIVERT_LAYER_NETWORK: u32 = 0;
#[cfg(windows)]
const WINDIVERT_FLAG_SNIFF: u64 = 0x0001;
#[cfg(windows)]
const WINDIVERT_FLAG_RECV_ONLY: u64 = 0x0004;
#[cfg(windows)]
const WINDIVERT_SHUTDOWN_RECV: u32 = 0x1;
#[cfg(windows)]
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
#[cfg(windows)]
const ERROR_NO_DATA: u32 = 232;

pub const WINDIVERT_UNAVAILABLE_CODE: &str = "windivert_unavailable";
pub const WINDIVERT_VERSION: &str = "2.2.2-A";
pub const WINDIVERT_DOWNLOAD_URL: &str =
    "https://github.com/basil00/WinDivert/releases/download/v2.2.2/WinDivert-2.2.2-A.zip";
pub const WINDIVERT_ZIP_SHA256: &str =
    "63cb41763bb4b20f600b6de04e991a9c2be73279e317d4d82f237b150c5f3f15";
const WINDIVERT_ZIP_MAX_BYTES: u64 = 4 * 1024 * 1024;
const WINDIVERT_ZIP_NAME: &str = "WinDivert-2.2.2-A.zip";
const WINDIVERT_DLL_ENTRY: &str = "WinDivert-2.2.2-A/x64/WinDivert.dll";
const WINDIVERT_SYS_ENTRY: &str = "WinDivert-2.2.2-A/x64/WinDivert64.sys";
const WINDIVERT_LICENSE_ENTRY: &str = "WinDivert-2.2.2-A/LICENSE";

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

#[cfg(windows)]
type WinDivertOpenFn =
    unsafe extern "system" fn(filter: *const i8, layer: u32, priority: i16, flags: u64) -> HANDLE;
#[cfg(windows)]
type WinDivertRecvFn = unsafe extern "system" fn(
    handle: HANDLE,
    packet: *mut c_void,
    packet_len: u32,
    recv_len: *mut u32,
    addr: *mut WinDivertAddress,
) -> i32;
#[cfg(windows)]
type WinDivertShutdownFn = unsafe extern "system" fn(handle: HANDLE, how: u32) -> i32;
#[cfg(windows)]
type WinDivertCloseFn = unsafe extern "system" fn(handle: HANDLE) -> i32;

#[cfg(windows)]
#[repr(C, align(8))]
#[derive(Clone, Copy)]
struct WinDivertAddress {
    bytes: [u8; 80],
}

#[cfg(windows)]
impl Default for WinDivertAddress {
    fn default() -> Self {
        Self { bytes: [0; 80] }
    }
}

#[cfg(windows)]
struct WinDivertLibrary {
    module: HMODULE,
    open: WinDivertOpenFn,
    recv: WinDivertRecvFn,
    shutdown: WinDivertShutdownFn,
    close: WinDivertCloseFn,
}

#[cfg(windows)]
impl Drop for WinDivertLibrary {
    fn drop(&mut self) {
        unsafe {
            FreeLibrary(self.module);
        }
    }
}

#[cfg(windows)]
#[derive(Clone)]
pub struct WinDivertHandle {
    inner: Arc<WinDivertHandleInner>,
}

#[cfg(windows)]
struct WinDivertHandleInner {
    lib: WinDivertLibrary,
    handle: Mutex<Option<HANDLE>>,
}

#[cfg(windows)]
unsafe impl Send for WinDivertHandleInner {}
#[cfg(windows)]
unsafe impl Sync for WinDivertHandleInner {}

#[cfg(windows)]
impl Drop for WinDivertHandleInner {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.handle.lock() {
            if let Some(handle) = guard.take() {
                unsafe {
                    (self.lib.close)(handle);
                }
            }
        }
    }
}

#[cfg(windows)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WinDivertRecvError {
    Shutdown,
    InsufficientBuffer,
    Other(u32),
}

#[cfg(windows)]
impl WinDivertHandle {
    pub fn open_ip_sniff(installed_dir: Option<&Path>) -> Result<Self, String> {
        let lib = WinDivertLibrary::load(installed_dir).map_err(unavailable_message)?;
        let filter = CString::new("ip").expect("static filter has no nul byte");
        let handle = unsafe {
            (lib.open)(
                filter.as_ptr(),
                WINDIVERT_LAYER_NETWORK,
                0,
                WINDIVERT_FLAG_SNIFF | WINDIVERT_FLAG_RECV_ONLY,
            )
        };
        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            return Err(unavailable_message(WinDivertLoadError::OpenFailed(
                last_error(),
            )));
        }
        Ok(Self {
            inner: Arc::new(WinDivertHandleInner {
                lib,
                handle: Mutex::new(Some(handle)),
            }),
        })
    }

    pub fn recv(&self, buffer: &mut [u8]) -> Result<usize, WinDivertRecvError> {
        let handle = self.current_handle().ok_or(WinDivertRecvError::Shutdown)?;
        let mut recv_len = 0_u32;
        let mut addr = WinDivertAddress::default();
        let ok = unsafe {
            (self.inner.lib.recv)(
                handle,
                buffer.as_mut_ptr().cast(),
                buffer.len() as u32,
                &mut recv_len,
                &mut addr,
            )
        };
        if ok != 0 {
            return Ok(recv_len as usize);
        }
        match last_error() {
            ERROR_NO_DATA => Err(WinDivertRecvError::Shutdown),
            ERROR_INSUFFICIENT_BUFFER => Err(WinDivertRecvError::InsufficientBuffer),
            code => Err(WinDivertRecvError::Other(code)),
        }
    }

    pub fn shutdown_recv(&self) {
        if let Some(handle) = self.current_handle() {
            unsafe {
                (self.inner.lib.shutdown)(handle, WINDIVERT_SHUTDOWN_RECV);
            }
        }
    }

    pub fn close(&self) {
        if let Ok(mut guard) = self.inner.handle.lock() {
            if let Some(handle) = guard.take() {
                unsafe {
                    (self.inner.lib.close)(handle);
                }
            }
        }
    }

    fn current_handle(&self) -> Option<HANDLE> {
        self.inner.handle.lock().ok().and_then(|guard| *guard)
    }
}

#[cfg(windows)]
impl WinDivertLibrary {
    fn load(installed_dir: Option<&Path>) -> Result<Self, WinDivertLoadError> {
        let module = load_windivert_module(installed_dir)?;
        unsafe {
            let open = load_symbol::<WinDivertOpenFn>(module, b"WinDivertOpen\0")?;
            let recv = load_symbol::<WinDivertRecvFn>(module, b"WinDivertRecv\0")?;
            let shutdown = load_symbol::<WinDivertShutdownFn>(module, b"WinDivertShutdown\0")?;
            let close = load_symbol::<WinDivertCloseFn>(module, b"WinDivertClose\0")?;
            Ok(Self {
                module,
                open,
                recv,
                shutdown,
                close,
            })
        }
    }
}

#[cfg(windows)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WinDivertLoadError {
    DllLoadFailed(u32),
    SymbolMissing(&'static str),
    OpenFailed(u32),
}

#[cfg(windows)]
fn load_windivert_module(installed_dir: Option<&Path>) -> Result<HMODULE, WinDivertLoadError> {
    for path in windivert_search_paths(installed_dir) {
        let module = unsafe { LoadLibraryW(wide_null(path.as_os_str()).as_ptr()) };
        if !module.is_null() {
            return Ok(module);
        }
    }
    let module = unsafe { LoadLibraryW(wide_null(OsStr::new("WinDivert.dll")).as_ptr()) };
    if module.is_null() {
        Err(WinDivertLoadError::DllLoadFailed(last_error()))
    } else {
        Ok(module)
    }
}

#[cfg(any(windows, test))]
fn windivert_search_paths(installed_dir: Option<&Path>) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(dir) = installed_dir {
        paths.push(dir.join("WinDivert.dll"));
    }
    if let Some(dir) = std::env::var_os("NTE_WINDIVERT_DIR") {
        paths.push(Path::new(&dir).join("WinDivert.dll"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            paths.push(dir.join("WinDivert.dll"));
        }
    }
    paths
}

#[cfg(windows)]
unsafe fn load_symbol<T: Copy>(
    module: HMODULE,
    name: &'static [u8],
) -> Result<T, WinDivertLoadError> {
    let symbol = unsafe { GetProcAddress(module, name.as_ptr() as PCSTR) };
    let Some(symbol) = symbol else {
        return Err(WinDivertLoadError::SymbolMissing(
            std::str::from_utf8(&name[..name.len().saturating_sub(1)]).unwrap_or("unknown"),
        ));
    };
    Ok(unsafe { std::mem::transmute_copy::<_, T>(&symbol) })
}

#[cfg(windows)]
fn wide_null(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
fn last_error() -> u32 {
    unsafe { GetLastError() }
}

#[cfg(windows)]
fn unavailable_message(error: WinDivertLoadError) -> String {
    format!(
        "{}: {}. Ensure WinDivert.dll and WinDivert64.sys are installed, check antivirus quarantine or file damage, and allow the WinDivert driver if endpoint security blocks it. NTE uses WinDivert in sniff/recv-only mode and does not modify or reinject packets.",
        WINDIVERT_UNAVAILABLE_CODE,
        describe_load_error(&error),
    )
}

#[cfg(windows)]
pub fn recv_error_message(error: WinDivertRecvError) -> String {
    match error {
        WinDivertRecvError::Shutdown => "windivert recv stopped".to_string(),
        WinDivertRecvError::InsufficientBuffer => {
            "windivert recv buffer too small for packet".to_string()
        }
        WinDivertRecvError::Other(code) => {
            format!("windivert recv failed: {}", describe_win32_code(code))
        }
    }
}

#[cfg(windows)]
fn describe_load_error(error: &WinDivertLoadError) -> String {
    match error {
        WinDivertLoadError::DllLoadFailed(code) => {
            format!(
                "failed to load WinDivert.dll ({})",
                describe_win32_code(*code)
            )
        }
        WinDivertLoadError::SymbolMissing(name) => {
            format!("WinDivert.dll is missing export {name}")
        }
        WinDivertLoadError::OpenFailed(code) => {
            format!("WinDivertOpen failed ({})", describe_win32_code(*code))
        }
    }
}

pub fn windivert_unavailable_for_platform() -> String {
    format!(
        "{}: WinDivert capture requires Windows. This build cannot run the WinDivert backend.",
        WINDIVERT_UNAVAILABLE_CODE
    )
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn describe_win32_code(code: u32) -> String {
    match code {
        2 => "WinDivert64.sys not found beside WinDivert.dll (ERROR_FILE_NOT_FOUND)".to_string(),
        5 => "access denied; run as administrator or check endpoint security policy (ERROR_ACCESS_DENIED)".to_string(),
        87 => "invalid WinDivert parameter or incompatible DLL (ERROR_INVALID_PARAMETER)".to_string(),
        577 => "driver signature/hash rejected (ERROR_INVALID_IMAGE_HASH)".to_string(),
        654 => "WinDivert driver version mismatch (ERROR_DRIVER_FAILED_PRIOR_UNLOAD)".to_string(),
        1060 => "WinDivert driver service is not installed/available (ERROR_SERVICE_DOES_NOT_EXIST)".to_string(),
        1257 => "driver blocked by Windows security policy (ERROR_DRIVER_BLOCKED)".to_string(),
        1753 => "Base Filtering Engine unavailable (EPT_S_NOT_REGISTERED)".to_string(),
        193 => "WinDivert.dll has wrong architecture or invalid image (ERROR_BAD_EXE_FORMAT)".to_string(),
        other => format!("Win32 error {other}"),
    }
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
    WinDivertHandle::open_ip_sniff(Some(install_dir)).map(|handle| {
        handle.shutdown_recv();
        handle.close();
    })
}

#[cfg(not(windows))]
fn windivert_loadable(_install_dir: &Path) -> Result<(), String> {
    Err(windivert_unavailable_for_platform())
}

fn verify_windivert_zip_sha(hash: &str) -> Result<(), String> {
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

fn extract_windivert_runtime_files(
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

fn cleanup_windivert_extract_temps(install_dir: &Path) {
    for file_name in ["WinDivert.dll.tmp", "WinDivert64.sys.tmp", "LICENSE.tmp"] {
        let _ = fs::remove_file(install_dir.join(file_name));
    }
    let _ = fs::remove_file(install_dir.join(format!("{WINDIVERT_ZIP_NAME}.tmp")));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

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
        let paths = windivert_search_paths(Some(installed_dir));

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
}
