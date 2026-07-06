mod error;
#[cfg(any(windows, test))]
mod ffi;
mod install;

pub const WINDIVERT_UNAVAILABLE_CODE: &str = "windivert_unavailable";
pub const WINDIVERT_VERSION: &str = "2.2.2-A";
pub const WINDIVERT_DOWNLOAD_URL: &str =
    "https://github.com/basil00/WinDivert/releases/download/v2.2.2/WinDivert-2.2.2-A.zip";
pub const WINDIVERT_ZIP_SHA256: &str =
    "63cb41763bb4b20f600b6de04e991a9c2be73279e317d4d82f237b150c5f3f15";

pub(super) const WINDIVERT_ZIP_MAX_BYTES: u64 = 4 * 1024 * 1024;
pub(super) const WINDIVERT_ZIP_NAME: &str = "WinDivert-2.2.2-A.zip";
pub(super) const WINDIVERT_DLL_ENTRY: &str = "WinDivert-2.2.2-A/x64/WinDivert.dll";
pub(super) const WINDIVERT_SYS_ENTRY: &str = "WinDivert-2.2.2-A/x64/WinDivert64.sys";
pub(super) const WINDIVERT_LICENSE_ENTRY: &str = "WinDivert-2.2.2-A/LICENSE";

#[cfg(windows)]
pub use error::WinDivertLoadError;
pub use error::{describe_win32_code, windivert_unavailable_for_platform};
#[cfg(windows)]
pub use ffi::{WinDivertHandle, WinDivertRecvError, recv_error_message};
pub use install::{
    WinDivertInstallReport, WinDivertInstallStatus, install_windivert, windivert_install_dir,
    windivert_status,
};

#[cfg(test)]
mod tests;
