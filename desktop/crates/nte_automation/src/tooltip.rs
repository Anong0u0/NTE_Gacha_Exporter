#[derive(Debug)]
pub struct AutomationTooltip {
    #[cfg_attr(not(windows), allow(dead_code))]
    enabled: bool,
    #[cfg_attr(not(windows), allow(dead_code))]
    offset_x: i32,
    #[cfg_attr(not(windows), allow(dead_code))]
    offset_y: i32,
    unavailable_reason: Option<String>,
    #[cfg(windows)]
    hwnd: usize,
}

impl AutomationTooltip {
    pub fn new(enabled: bool) -> Self {
        Self::with_offset(enabled, 48, 32)
    }

    pub fn with_offset(enabled: bool, offset_x: i32, offset_y: i32) -> Self {
        if !enabled {
            return Self {
                enabled,
                offset_x,
                offset_y,
                unavailable_reason: Some("disabled".to_string()),
                #[cfg(windows)]
                hwnd: 0,
            };
        }
        Self::create(enabled, offset_x, offset_y)
    }

    pub fn unavailable_reason(&self) -> Option<&str> {
        self.unavailable_reason.as_deref()
    }

    #[cfg(not(windows))]
    fn create(enabled: bool, offset_x: i32, offset_y: i32) -> Self {
        Self {
            enabled,
            offset_x,
            offset_y,
            unavailable_reason: Some("Windows tooltip requires Windows".to_string()),
        }
    }

    #[cfg(windows)]
    fn create(enabled: bool, offset_x: i32, offset_y: i32) -> Self {
        let hwnd = create_window();
        let unavailable_reason = hwnd.is_none().then(|| "CreateWindowExW failed".to_string());
        Self {
            enabled,
            offset_x,
            offset_y,
            unavailable_reason,
            hwnd: hwnd.unwrap_or_default(),
        }
    }

    #[cfg(not(windows))]
    pub fn show(&self, _message: &str) -> bool {
        let _ = self;
        false
    }

    #[cfg(windows)]
    pub fn show(&self, message: &str) -> bool {
        use std::ptr;

        use windows_sys::Win32::Foundation::{HWND, POINT};
        use windows_sys::Win32::Graphics::Gdi::{InvalidateRect, UpdateWindow};
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetCursorPos, GetSystemMetrics, SetWindowPos, SetWindowTextW, ShowWindow, HWND_TOPMOST,
            SM_CXSCREEN, SM_CYSCREEN, SWP_NOACTIVATE, SWP_SHOWWINDOW, SW_HIDE, SW_SHOWNOACTIVATE,
        };

        if !self.enabled || self.hwnd == 0 {
            return false;
        }
        let hwnd = self.hwnd as HWND;
        if message.is_empty() {
            unsafe {
                ShowWindow(hwnd, SW_HIDE);
            }
            return true;
        }
        let (width, height) = self.measure(message);
        let mut point = POINT { x: 0, y: 0 };
        unsafe {
            GetCursorPos(&mut point);
        }
        let screen_width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let screen_height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        let x = (point.x + self.offset_x).clamp(0, (screen_width - width).max(0));
        let y = (point.y + self.offset_y).clamp(0, (screen_height - height).max(0));
        let text = wide(message);
        unsafe {
            SetWindowTextW(hwnd, text.as_ptr());
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                x,
                y,
                width,
                height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            InvalidateRect(hwnd, ptr::null(), 1);
            UpdateWindow(hwnd);
        }
        true
    }

    #[cfg(windows)]
    fn measure(&self, message: &str) -> (i32, i32) {
        use windows_sys::Win32::Foundation::RECT;
        use windows_sys::Win32::Graphics::Gdi::{
            DrawTextW, GetDC, ReleaseDC, DT_CALCRECT, DT_LEFT, DT_NOPREFIX, DT_WORDBREAK,
        };

        const MAX_TOOLTIP_WIDTH: i32 = 420;
        let hwnd = self.hwnd as _;
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: MAX_TOOLTIP_WIDTH,
            bottom: 0,
        };
        let hdc = unsafe { GetDC(hwnd) };
        if hdc.is_null() {
            return (
                ((message.chars().count() as i32 * 8) + 12).clamp(80, MAX_TOOLTIP_WIDTH),
                28,
            );
        }
        let text = wide(message);
        unsafe {
            DrawTextW(
                hdc,
                text.as_ptr(),
                -1,
                &mut rect,
                DT_LEFT | DT_WORDBREAK | DT_CALCRECT | DT_NOPREFIX,
            );
            ReleaseDC(hwnd, hdc);
        }
        (
            (rect.right - rect.left + 12).clamp(80, MAX_TOOLTIP_WIDTH),
            (rect.bottom - rect.top + 8).max(24),
        )
    }
}

#[cfg(windows)]
impl Drop for AutomationTooltip {
    fn drop(&mut self) {
        use windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow;

        if self.hwnd != 0 {
            unsafe {
                DestroyWindow(self.hwnd as _);
            }
            self.hwnd = 0;
        }
    }
}

#[cfg(windows)]
fn create_window() -> Option<usize> {
    use std::ptr;

    use windows_sys::Win32::Graphics::Gdi::{GetStockObject, DEFAULT_GUI_FONT};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, SendMessageW, WM_SETFONT, WS_BORDER, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
        WS_EX_TOPMOST, WS_POPUP,
    };

    const SS_LEFT: u32 = 0;
    const SS_NOPREFIX: u32 = 0x0000_0080;

    let class_name = wide("STATIC");
    let empty = wide("");
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class_name.as_ptr(),
            empty.as_ptr(),
            WS_POPUP | WS_BORDER | SS_LEFT | SS_NOPREFIX,
            0,
            0,
            1,
            1,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null(),
        )
    };
    if hwnd.is_null() {
        return None;
    }
    let font = unsafe { GetStockObject(DEFAULT_GUI_FONT) };
    if !font.is_null() {
        unsafe {
            SendMessageW(hwnd, WM_SETFONT, font as usize, 1);
        }
    }
    Some(hwnd as usize)
}

#[cfg(windows)]
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
