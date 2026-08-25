use std::path::Path;

use anyhow::Result;
#[cfg(any(windows, test))]
use sha2::{Digest, Sha256};

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) enum AcquireResult {
    Primary(InstanceGuard),
    Existing,
}

pub(crate) struct InstanceGuard {
    #[cfg(windows)]
    handle: windows_sys::Win32::Foundation::HANDLE,
}

pub(crate) fn acquire(root: &Path, admin_handoff: bool) -> Result<AcquireResult> {
    platform::acquire(root, admin_handoff)
}

#[cfg(any(windows, test))]
fn canonical_identity(path: &Path) -> std::io::Result<String> {
    std::fs::canonicalize(path).map(|path| normalize_identity_text(&path.to_string_lossy()))
}

#[cfg(any(windows, test))]
fn instance_name(root_identity: &str) -> String {
    let digest = Sha256::digest(root_identity.as_bytes());
    format!("Local\\NTE_Gacha_Exporter_{digest:x}")
}

#[cfg(any(windows, test))]
fn normalize_identity_text(value: &str) -> String {
    let value = value.replace('/', "\\");
    let value = value
        .strip_prefix("\\\\?\\UNC\\")
        .map(|value| format!("\\\\{value}"))
        .or_else(|| value.strip_prefix("\\\\?\\").map(str::to_string))
        .unwrap_or(value);
    value.to_lowercase()
}

#[cfg(windows)]
mod platform {
    use std::ffi::{OsStr, OsString};
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::path::{Path, PathBuf};
    use std::ptr;

    use anyhow::{Context, Result, anyhow};
    use windows_sys::Win32::{
        Foundation::{
            CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HWND, LPARAM, WAIT_ABANDONED,
            WAIT_OBJECT_0, WAIT_TIMEOUT,
        },
        System::Threading::{
            CreateMutexW, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
            QueryFullProcessImageNameW, ReleaseMutex, WaitForSingleObject,
        },
        UI::WindowsAndMessaging::{
            EnumWindows, GetClassNameW, GetWindowThreadProcessId, IsIconic, SW_RESTORE, SW_SHOW,
            ShowWindow,
        },
    };
    use windows_sys::core::BOOL;

    use super::{AcquireResult, InstanceGuard, canonical_identity, instance_name};

    const ADMIN_HANDOFF_TIMEOUT_MS: u32 = 10_000;
    const WINDOW_CLASS: &str = "Tauri Window";

    pub(super) fn acquire(root: &Path, admin_handoff: bool) -> Result<AcquireResult> {
        let root_identity = canonical_identity(root)
            .with_context(|| format!("failed to canonicalize portable root: {}", root.display()))?;
        let name = instance_name(&root_identity);
        let name = wide(name);
        let handle = unsafe { CreateMutexW(ptr::null(), 1, name.as_ptr()) };
        if handle.is_null() {
            return Err(std::io::Error::last_os_error()).context("CreateMutexW failed");
        }
        let already_exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
        if !already_exists {
            return Ok(AcquireResult::Primary(InstanceGuard { handle }));
        }
        if admin_handoff {
            return acquire_after_handoff(handle);
        }

        let _ = show_existing_window();
        unsafe {
            CloseHandle(handle);
        }
        Ok(AcquireResult::Existing)
    }

    fn acquire_after_handoff(
        handle: windows_sys::Win32::Foundation::HANDLE,
    ) -> Result<AcquireResult> {
        let wait = unsafe { WaitForSingleObject(handle, ADMIN_HANDOFF_TIMEOUT_MS) };
        if matches!(wait, WAIT_OBJECT_0 | WAIT_ABANDONED) {
            return Ok(AcquireResult::Primary(InstanceGuard { handle }));
        }
        unsafe {
            CloseHandle(handle);
        }
        if wait == WAIT_TIMEOUT {
            return Err(anyhow!("timed out waiting for previous instance shutdown"));
        }
        Err(std::io::Error::last_os_error()).context("WaitForSingleObject failed")
    }

    // Duplicate launches restore or show the existing window; foreground activation is intentionally not forced.
    fn show_existing_window() -> Result<()> {
        struct Context {
            target_executable: String,
            hwnd: HWND,
        }

        unsafe extern "system" fn enum_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let context = unsafe { &mut *(lparam as *mut Context) };
            if window_class(hwnd) != WINDOW_CLASS {
                return 1;
            }
            let mut pid = 0;
            unsafe {
                GetWindowThreadProcessId(hwnd, &mut pid);
            }
            let matches = process_executable(pid)
                .and_then(|path| canonical_identity(&path).ok())
                .is_some_and(|path| path == context.target_executable);
            if !matches {
                return 1;
            }
            context.hwnd = hwnd;
            0
        }

        let executable = std::env::current_exe().context("failed to resolve current executable")?;
        let mut context = Context {
            target_executable: canonical_identity(&executable)
                .context("failed to canonicalize current executable")?,
            hwnd: ptr::null_mut(),
        };
        unsafe {
            EnumWindows(Some(enum_window), &mut context as *mut Context as LPARAM);
        }
        if context.hwnd.is_null() {
            return Ok(());
        }
        unsafe {
            if IsIconic(context.hwnd) != 0 {
                ShowWindow(context.hwnd, SW_RESTORE);
            } else {
                ShowWindow(context.hwnd, SW_SHOW);
            }
        }
        Ok(())
    }

    fn process_executable(pid: u32) -> Option<PathBuf> {
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if handle.is_null() {
            return None;
        }
        let mut buffer = vec![0_u16; 32_768];
        let mut length = buffer.len() as u32;
        let ok = unsafe { QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut length) };
        unsafe {
            CloseHandle(handle);
        }
        (ok != 0).then(|| PathBuf::from(OsString::from_wide(&buffer[..length as usize])))
    }

    fn window_class(hwnd: HWND) -> String {
        let mut buffer = [0_u16; 64];
        let length = unsafe { GetClassNameW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
        if length <= 0 {
            return String::new();
        }
        String::from_utf16_lossy(&buffer[..length as usize])
    }

    fn wide(value: impl AsRef<OsStr>) -> Vec<u16> {
        value
            .as_ref()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    impl Drop for InstanceGuard {
        fn drop(&mut self) {
            unsafe {
                let _ = ReleaseMutex(self.handle);
                CloseHandle(self.handle);
            }
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use std::path::Path;

    use anyhow::Result;

    use super::{AcquireResult, InstanceGuard};

    pub(super) fn acquire(_root: &Path, _admin_handoff: bool) -> Result<AcquireResult> {
        Ok(AcquireResult::Primary(InstanceGuard {}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_normalization_is_case_and_separator_insensitive() {
        assert_eq!(
            normalize_identity_text(r"\\?\D:\Tools\NTE"),
            normalize_identity_text("d:/tools/nte")
        );
    }

    #[test]
    fn instance_name_is_stable_per_root() {
        let left = instance_name(&normalize_identity_text("D:/one"));
        let same = instance_name(&normalize_identity_text(r"d:\one"));
        let other = instance_name(&normalize_identity_text("D:/two"));

        assert_eq!(left, same);
        assert_ne!(left, other);
        assert!(left.starts_with(r"Local\NTE_Gacha_Exporter_"));
    }

    #[test]
    fn relative_and_canonical_root_use_same_instance_name() {
        let current_dir = std::env::current_dir().unwrap();
        let root = tempfile::Builder::new()
            .prefix("instance-root-")
            .tempdir_in(&current_dir)
            .unwrap();
        let relative_root = root.path().strip_prefix(&current_dir).unwrap();
        let canonical_root = std::fs::canonicalize(root.path()).unwrap();

        let relative = instance_name(&canonical_identity(relative_root).unwrap());
        let canonical = instance_name(&canonical_identity(&canonical_root).unwrap());

        assert_eq!(relative, canonical);
    }

    #[cfg(not(windows))]
    #[test]
    fn non_windows_acquire_does_not_require_existing_root() {
        let root = Path::new("missing-instance-root");

        assert!(matches!(
            acquire(root, false).unwrap(),
            AcquireResult::Primary(_)
        ));
    }
}
