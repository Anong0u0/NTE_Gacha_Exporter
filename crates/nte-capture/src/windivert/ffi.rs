#[cfg(windows)]
use std::ffi::{CString, OsStr, c_void};
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::sync::{Arc, Mutex};

#[cfg(windows)]
use windows_sys::Win32::Foundation::{
    FreeLibrary, GetLastError, HANDLE, HMODULE, INVALID_HANDLE_VALUE,
};
#[cfg(windows)]
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
#[cfg(windows)]
use windows_sys::core::PCSTR;

#[cfg(windows)]
use super::error::{WinDivertLoadError, describe_win32_code, unavailable_message};

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

pub(super) fn windivert_search_paths(installed_dir: Option<&Path>) -> Vec<PathBuf> {
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
