#[cfg(windows)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WinDivertLoadError {
    DllLoadFailed(u32),
    SymbolMissing(&'static str),
    OpenFailed(u32),
}

#[cfg(windows)]
pub(super) fn unavailable_message(error: WinDivertLoadError) -> String {
    format!(
        "{}: {}. Ensure WinDivert.dll and WinDivert64.sys are installed, check antivirus quarantine or file damage, and allow the WinDivert driver if endpoint security blocks it. NTE uses WinDivert in sniff/recv-only mode and does not modify or reinject packets.",
        super::WINDIVERT_UNAVAILABLE_CODE,
        describe_load_error(&error),
    )
}

pub fn windivert_unavailable_for_platform() -> String {
    format!(
        "{}: WinDivert capture requires Windows. This build cannot run the WinDivert backend.",
        super::WINDIVERT_UNAVAILABLE_CODE
    )
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn describe_win32_code(code: u32) -> String {
    match code {
        2 => "WinDivert64.sys not found beside WinDivert.dll (ERROR_FILE_NOT_FOUND)".to_string(),
        5 => "access denied; run as administrator or check endpoint security policy (ERROR_ACCESS_DENIED)".to_string(),
        87 => "invalid WinDivert parameter or incompatible DLL (ERROR_INVALID_PARAMETER)".to_string(),
        577 => "driver signature/hash rejected (ERROR_INVALID_IMAGE_HASH)".to_string(),
        654 => {
            "WinDivert driver version mismatch (ERROR_DRIVER_FAILED_PRIOR_UNLOAD)".to_string()
        }
        1060 => {
            "WinDivert driver service is not installed/available (ERROR_SERVICE_DOES_NOT_EXIST)"
                .to_string()
        }
        1257 => "driver blocked by Windows security policy (ERROR_DRIVER_BLOCKED)".to_string(),
        1753 => "Base Filtering Engine unavailable (EPT_S_NOT_REGISTERED)".to_string(),
        193 => {
            "WinDivert.dll has wrong architecture or invalid image (ERROR_BAD_EXE_FORMAT)"
                .to_string()
        }
        other => format!("Win32 error {other}"),
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
