#[cfg(windows)]
use std::thread;
#[cfg(windows)]
use std::time::Duration;

use crate::error::{AutomationError, AutomationResult};
use crate::model::{MouseButton, MouseClickDiagnostics, Point, Size};

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

pub fn primary_mouse_button(mouse_buttons_swapped: bool) -> MouseButton {
    if mouse_buttons_swapped {
        MouseButton::Right
    } else {
        MouseButton::Left
    }
}

#[cfg(not(windows))]
pub fn foreground_click(
    _window: &GameWindow,
    _point: Point,
) -> AutomationResult<MouseClickDiagnostics> {
    require_windows()?;
    unreachable!()
}

#[cfg(windows)]
pub fn foreground_click(
    window: &GameWindow,
    point: Point,
) -> AutomationResult<MouseClickDiagnostics> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP,
    };

    force_foreground(window)?;
    let screen_point = client_to_screen(window.hwnd, point)?;
    let mouse_buttons_swapped = mouse_buttons_swapped();
    let physical_button = primary_mouse_button(mouse_buttons_swapped);
    let (button_down, button_up) = match physical_button {
        MouseButton::Left => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
        MouseButton::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
    };
    move_cursor(screen_point)?;
    thread::sleep(Duration::from_millis(25));
    send_mouse_input("mouse_down", button_down, 0, 0, 0)?;
    thread::sleep(Duration::from_millis(35));
    send_mouse_input("mouse_up", button_up, 0, 0, 0)?;
    Ok(MouseClickDiagnostics {
        point,
        physical_button,
        mouse_buttons_swapped,
    })
}

#[cfg(windows)]
fn send_mouse_input(
    label: &str,
    flags: u32,
    dx: i32,
    dy: i32,
    mouse_data: u32,
) -> AutomationResult<()> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_MOUSE, MOUSEINPUT,
    };

    send_input_event(
        label,
        INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx,
                    dy,
                    mouseData: mouse_data,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
    )
}

#[cfg(windows)]
fn send_input_keyboard_event(label: &str, virtual_key: u16, flags: u32) -> AutomationResult<()> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
    };

    send_input_event(
        label,
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: virtual_key,
                    wScan: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
    )
}

#[cfg(windows)]
fn send_input_event(
    label: &str,
    input: windows_sys::Win32::UI::Input::KeyboardAndMouse::INPUT,
) -> AutomationResult<()> {
    use std::mem;

    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{INPUT, SendInput};

    let mut winerr = 0;
    for attempt in 0..=1 {
        let sent = unsafe { SendInput(1, &input, mem::size_of::<INPUT>() as i32) };
        if sent == 1 {
            return Ok(());
        }
        winerr = unsafe { GetLastError() };
        if attempt == 0 {
            thread::sleep(Duration::from_millis(10));
        }
    }
    Err(AutomationError::message(send_input_error_message(
        label, winerr,
    )))
}

#[cfg(any(windows, test))]
fn send_input_error_message(label: &str, winerr: u32) -> String {
    format!("SendInput {label} failed winerr={winerr}")
}

#[cfg(windows)]
fn mouse_buttons_swapped() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_SWAPBUTTON};

    unsafe { GetSystemMetrics(SM_SWAPBUTTON) != 0 }
}

#[cfg(windows)]
fn move_cursor(point: Point) -> AutomationResult<()> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_MOVE, MOUSEEVENTF_VIRTUALDESK,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
        SM_YVIRTUALSCREEN, SetCursorPos,
    };

    if unsafe { SetCursorPos(point.x, point.y) } != 0 {
        return Ok(());
    }

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
    send_mouse_input(
        "mouse_move",
        MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
        dx,
        dy,
        0,
    )?;
    thread::sleep(Duration::from_millis(25));
    Ok(())
}

#[cfg(not(windows))]
pub fn foreground_escape(_window: &GameWindow) -> AutomationResult<()> {
    require_windows()
}

#[cfg(windows)]
pub fn foreground_escape(window: &GameWindow) -> AutomationResult<()> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{KEYEVENTF_KEYUP, VK_ESCAPE};

    force_foreground(window)?;
    let _ = send_input_keyboard_event("escape_down", VK_ESCAPE, 0);
    thread::sleep(Duration::from_millis(35));
    let _ = send_input_keyboard_event("escape_up", VK_ESCAPE, KEYEVENTF_KEYUP);
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
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{KEYEVENTF_KEYUP, VK_MENU};
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
            let _ = send_input_keyboard_event("alt_down", VK_MENU, 0);
            let _ = send_input_keyboard_event("alt_up", VK_MENU, KEYEVENTF_KEYUP);
            SetForegroundWindow(hwnd);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_mouse_button_follows_swap_setting() {
        assert_eq!(primary_mouse_button(false), MouseButton::Left);
        assert_eq!(primary_mouse_button(true), MouseButton::Right);
    }

    #[test]
    fn send_input_error_message_includes_label_and_winerr() {
        assert_eq!(
            send_input_error_message("mouse_down", 5),
            "SendInput mouse_down failed winerr=5"
        );
    }
}
