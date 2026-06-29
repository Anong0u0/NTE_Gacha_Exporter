#[cfg(windows)]
use std::thread;
#[cfg(windows)]
use std::time::Duration;

use crate::error::{AutomationError, AutomationResult};
use crate::model::{Point, Size};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl ClientRect {
    pub fn size(self) -> Size {
        Size {
            width: (self.right - self.left).max(0) as u32,
            height: (self.bottom - self.top).max(0) as u32,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameWindow {
    pub hwnd: usize,
    pub pid: u32,
    pub class_name: String,
    pub title: String,
    pub client_rect: ClientRect,
}

impl GameWindow {
    pub fn client_size(&self) -> Size {
        self.client_rect.size()
    }
}

#[cfg(not(windows))]
pub fn require_windows() -> AutomationResult<()> {
    Err(AutomationError::message("auto page requires Windows"))
}

#[cfg(windows)]
pub fn require_windows() -> AutomationResult<()> {
    Ok(())
}

#[cfg(not(windows))]
pub fn set_dpi_aware() -> AutomationResult<()> {
    require_windows()
}

#[cfg(windows)]
pub fn set_dpi_aware() -> AutomationResult<()> {
    use windows_sys::Win32::UI::HiDpi::{
        DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::SetProcessDPIAware;

    unsafe {
        if SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) == 0 {
            let _ = SetProcessDPIAware();
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn resolve_game_window(_pid: u32, _class_name: &str) -> AutomationResult<GameWindow> {
    require_windows()?;
    unreachable!()
}

#[cfg(windows)]
pub fn resolve_game_window(pid: u32, class_name: &str) -> AutomationResult<GameWindow> {
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
    };
    use windows_sys::core::BOOL;

    set_dpi_aware()?;
    struct EnumContext {
        pid: u32,
        class_name: String,
        windows: Vec<GameWindow>,
    }

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let context = unsafe { &mut *(lparam as *mut EnumContext) };
        if unsafe { IsWindowVisible(hwnd) } == 0 || unsafe { IsIconic(hwnd) } != 0 {
            return 1;
        }
        let mut window_pid = 0_u32;
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut window_pid);
        }
        if window_pid != context.pid {
            return 1;
        }
        if let Ok(window) = window_from_hwnd(hwnd as usize, context.pid) {
            if window.class_name == context.class_name && window.client_size().width > 0 {
                context.windows.push(window);
            }
        }
        1
    }

    let mut context = EnumContext {
        pid,
        class_name: class_name.to_string(),
        windows: Vec::new(),
    };
    unsafe {
        EnumWindows(Some(enum_proc), &mut context as *mut EnumContext as LPARAM);
    }
    context.windows.into_iter().next().ok_or_else(|| {
        AutomationError::message(format!(
            "HTGame.exe window not found for pid={pid} class={class_name}"
        ))
    })
}

#[cfg(not(windows))]
pub fn refresh_window(_window: &GameWindow) -> AutomationResult<GameWindow> {
    require_windows()?;
    unreachable!()
}

#[cfg(windows)]
pub fn refresh_window(window: &GameWindow) -> AutomationResult<GameWindow> {
    window_from_hwnd(window.hwnd, window.pid)
}

#[cfg(not(windows))]
pub fn force_foreground(_window: &GameWindow) -> AutomationResult<()> {
    require_windows()
}

#[cfg(windows)]
pub fn force_foreground(window: &GameWindow) -> AutomationResult<()> {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{SetActiveWindow, SetFocus};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        ASFW_ANY, AllowSetForegroundWindow, BringWindowToTop, GetForegroundWindow, IsIconic,
        SW_RESTORE, SW_SHOW, SetForegroundWindow, ShowWindow,
    };

    let hwnd = window.hwnd as HWND;
    unsafe {
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        } else {
            ShowWindow(hwnd, SW_SHOW);
        }
        let _ = AllowSetForegroundWindow(ASFW_ANY);
    }

    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        try_force_foreground(hwnd);
        unsafe {
            BringWindowToTop(hwnd);
            SetForegroundWindow(hwnd);
            SetActiveWindow(hwnd);
            SetFocus(hwnd);
            if GetForegroundWindow() == hwnd {
                return Ok(());
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err(AutomationError::message(format!(
        "failed to bring game window to foreground: hwnd={}",
        window.hwnd
    )))
}

#[cfg(not(windows))]
pub fn foreground_click(_window: &GameWindow, _point: Point) -> AutomationResult<()> {
    require_windows()
}

#[cfg(windows)]
pub fn foreground_click(window: &GameWindow, point: Point) -> AutomationResult<()> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, mouse_event,
    };

    force_foreground(window)?;
    let screen_point = client_to_screen(window.hwnd, point)?;
    unsafe {
        move_cursor(screen_point)?;
        thread::sleep(Duration::from_millis(25));
        mouse_event(MOUSEEVENTF_LEFTDOWN, 0, 0, 0, 0);
        thread::sleep(Duration::from_millis(35));
        mouse_event(MOUSEEVENTF_LEFTUP, 0, 0, 0, 0);
    }
    Ok(())
}

#[cfg(windows)]
fn move_cursor(point: Point) -> AutomationResult<()> {
    use windows_sys::Win32::Foundation::{GetLastError, POINT};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_MOVE, mouse_event,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetCursorPos, GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
        SM_YVIRTUALSCREEN, SetCursorPos,
    };

    if unsafe { SetCursorPos(point.x, point.y) } != 0 {
        return Ok(());
    }
    let last_error = unsafe { GetLastError() };

    let left = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
    let top = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
    let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) }.max(1);
    let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) }.max(1);
    let right = left + width.saturating_sub(1);
    let bottom = top + height.saturating_sub(1);
    let target_x = point.x.clamp(left, right);
    let target_y = point.y.clamp(top, bottom);
    let dx = (((target_x - left) as i64 * 65_535) / width.saturating_sub(1).max(1) as i64) as i32;
    let dy = (((target_y - top) as i64 * 65_535) / height.saturating_sub(1).max(1) as i64) as i32;
    unsafe {
        mouse_event(MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE, dx, dy, 0, 0);
    }
    thread::sleep(Duration::from_millis(25));

    let mut cursor = POINT { x: 0, y: 0 };
    if unsafe { GetCursorPos(&mut cursor) } != 0
        && (cursor.x - target_x).abs() <= 2
        && (cursor.y - target_y).abs() <= 2
    {
        return Ok(());
    }
    Err(AutomationError::message(format!(
        "SetCursorPos failed: target={},{} virtual={},{} {}x{} cursor={},{} winerr={}",
        point.x, point.y, left, top, width, height, cursor.x, cursor.y, last_error
    )))
}

#[cfg(not(windows))]
pub fn foreground_escape(_window: &GameWindow) -> AutomationResult<()> {
    require_windows()
}

#[cfg(windows)]
pub fn foreground_escape(window: &GameWindow) -> AutomationResult<()> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        KEYEVENTF_KEYUP, VK_ESCAPE, keybd_event,
    };

    force_foreground(window)?;
    unsafe {
        keybd_event(VK_ESCAPE as u8, 0, 0, 0);
        thread::sleep(Duration::from_millis(35));
        keybd_event(VK_ESCAPE as u8, 0, KEYEVENTF_KEYUP, 0);
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn escape_pressed() -> bool {
    false
}

#[cfg(windows)]
pub fn escape_pressed() -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_ESCAPE};

    unsafe { (GetAsyncKeyState(VK_ESCAPE as i32) & 0x8000_u16 as i16) != 0 }
}

#[cfg(not(windows))]
pub fn client_to_screen(_hwnd: usize, _point: Point) -> AutomationResult<Point> {
    require_windows()?;
    unreachable!()
}

#[cfg(windows)]
pub fn client_to_screen(hwnd: usize, point: Point) -> AutomationResult<Point> {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::ClientToScreen;

    let mut native = POINT {
        x: point.x,
        y: point.y,
    };
    unsafe {
        if ClientToScreen(hwnd as _, &mut native) == 0 {
            return Err(AutomationError::message("ClientToScreen failed"));
        }
    }
    Ok(Point {
        x: native.x,
        y: native.y,
    })
}

#[cfg(not(windows))]
pub fn current_cursor_client_position(_window: &GameWindow) -> AutomationResult<Point> {
    require_windows()?;
    unreachable!()
}

#[cfg(windows)]
pub fn current_cursor_client_position(window: &GameWindow) -> AutomationResult<Point> {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::ScreenToClient;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

    let mut native = POINT { x: 0, y: 0 };
    unsafe {
        if GetCursorPos(&mut native) == 0 {
            return Err(AutomationError::message("GetCursorPos failed"));
        }
        if ScreenToClient(window.hwnd as _, &mut native) == 0 {
            return Err(AutomationError::message("ScreenToClient failed"));
        }
    }
    Ok(Point {
        x: native.x,
        y: native.y,
    })
}

#[cfg(windows)]
fn window_from_hwnd(hwnd: usize, pid: u32) -> AutomationResult<GameWindow> {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetClientRect;

    let mut rect = RECT::default();
    unsafe {
        if GetClientRect(hwnd as _, &mut rect) == 0 {
            return Err(AutomationError::message("GetClientRect failed"));
        }
    }
    Ok(GameWindow {
        hwnd,
        pid,
        class_name: class_name(hwnd),
        title: window_text(hwnd),
        client_rect: ClientRect {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
        },
    })
}

#[cfg(windows)]
fn class_name(hwnd: usize) -> String {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetClassNameW;

    let mut buffer = [0_u16; 256];
    let len = unsafe { GetClassNameW(hwnd as _, buffer.as_mut_ptr(), buffer.len() as i32) };
    String::from_utf16_lossy(&buffer[..len.max(0) as usize])
}

#[cfg(windows)]
fn window_text(hwnd: usize) -> String {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowTextLengthW, GetWindowTextW};

    let len = unsafe { GetWindowTextLengthW(hwnd as _) };
    if len <= 0 {
        return String::new();
    }
    let mut buffer = vec![0_u16; len as usize + 1];
    let read = unsafe { GetWindowTextW(hwnd as _, buffer.as_mut_ptr(), buffer.len() as i32) };
    String::from_utf16_lossy(&buffer[..read.max(0) as usize])
}

#[cfg(windows)]
fn try_force_foreground(hwnd: windows_sys::Win32::Foundation::HWND) {
    use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{KEYEVENTF_KEYUP, VK_MENU, keybd_event};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow,
    };

    unsafe {
        let current_thread = GetCurrentThreadId();
        let target_thread = GetWindowThreadProcessId(hwnd, std::ptr::null_mut());
        let foreground = GetForegroundWindow();
        let foreground_thread = if foreground.is_null() {
            0
        } else {
            GetWindowThreadProcessId(foreground, std::ptr::null_mut())
        };
        let mut attached = Vec::new();
        for thread_id in [target_thread, foreground_thread] {
            if thread_id != 0
                && thread_id != current_thread
                && AttachThreadInput(current_thread, thread_id, 1) != 0
            {
                attached.push(thread_id);
            }
        }
        SetForegroundWindow(hwnd);
        for thread_id in attached {
            let _ = AttachThreadInput(current_thread, thread_id, 0);
        }
        if GetForegroundWindow() != hwnd {
            keybd_event(VK_MENU as u8, 0, 0, 0);
            keybd_event(VK_MENU as u8, 0, KEYEVENTF_KEYUP, 0);
            SetForegroundWindow(hwnd);
        }
    }
}
