use std::{collections::BTreeSet, time::Duration};

use anyhow::{Result, bail};
use image::RgbImage;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct WindowInfo {
    pub hwnd: usize,
    pub pid: u32,
    pub title: String,
    pub rect: [i32; 4],
}

#[derive(Debug, Serialize)]
pub struct ImageMetrics {
    pub width: u32,
    pub height: u32,
    pub mean: [f64; 3],
    pub extrema: [[u8; 2]; 3],
    pub variance_score: f64,
    pub is_flat: bool,
}

pub fn image_metrics(image: &RgbImage) -> ImageMetrics {
    let width = image.width();
    let height = image.height();
    let count = (width as f64 * height as f64).max(1.0);
    let mut sum = [0_u64; 3];
    let mut min = [u8::MAX; 3];
    let mut max = [0_u8; 3];
    for pixel in image.pixels() {
        for channel in 0..3 {
            let value = pixel[channel];
            sum[channel] += value as u64;
            min[channel] = min[channel].min(value);
            max[channel] = max[channel].max(value);
        }
    }
    let mean = [
        sum[0] as f64 / count,
        sum[1] as f64 / count,
        sum[2] as f64 / count,
    ];
    let mut deviation_sum = [0_f64; 3];
    for pixel in image.pixels() {
        for channel in 0..3 {
            deviation_sum[channel] += (pixel[channel] as f64 - mean[channel]).abs();
        }
    }
    let variance_score = deviation_sum.iter().map(|value| value / count).sum::<f64>();
    ImageMetrics {
        width,
        height,
        mean,
        extrema: [[min[0], max[0]], [min[1], max[1]], [min[2], max[2]]],
        variance_score,
        is_flat: variance_score < 2.0,
    }
}

#[cfg(not(windows))]
pub fn require_windows() -> Result<()> {
    bail!("agent smoke launch and screenshot require a Windows host")
}

#[cfg(windows)]
pub fn require_windows() -> Result<()> {
    Ok(())
}

#[cfg(not(windows))]
pub fn visible_nte_windows() -> Result<Vec<WindowInfo>> {
    require_windows()?;
    unreachable!()
}

#[cfg(not(windows))]
pub fn find_window(
    _pid: Option<u32>,
    _title: Option<&str>,
    _exclude_hwnds: Option<&BTreeSet<usize>>,
    _timeout: Duration,
) -> Result<WindowInfo> {
    require_windows()?;
    unreachable!()
}

#[cfg(not(windows))]
pub fn capture_window(_window: &WindowInfo) -> Result<RgbImage> {
    require_windows()?;
    unreachable!()
}

#[cfg(not(windows))]
pub fn close_window(_window: &WindowInfo) -> Result<()> {
    require_windows()
}

#[cfg(windows)]
pub fn visible_nte_windows() -> Result<Vec<WindowInfo>> {
    use crate::cli::APP_TITLE;

    windows_impl::visible_windows(None, Some(APP_TITLE))
}

#[cfg(windows)]
pub fn find_window(
    pid: Option<u32>,
    title: Option<&str>,
    exclude_hwnds: Option<&BTreeSet<usize>>,
    timeout: Duration,
) -> Result<WindowInfo> {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        let mut windows = windows_impl::visible_windows(pid, title)?;
        if let Some(excluded) = exclude_hwnds {
            windows.retain(|window| !excluded.contains(&window.hwnd));
        }
        if let Some(window) = windows.into_iter().next() {
            return Ok(window);
        }
        thread::sleep(Duration::from_millis(250));
    }
    bail!("window not found: pid={pid:?} title={title:?}")
}

#[cfg(windows)]
pub fn capture_window(window: &WindowInfo) -> Result<RgbImage> {
    windows_impl::capture_window(window)
}

#[cfg(windows)]
pub fn close_window(window: &WindowInfo) -> Result<()> {
    windows_impl::close_window(window)
}

#[cfg(windows)]
mod windows_impl {
    use anyhow::{Result, anyhow, bail};
    use image::RgbImage;
    use std::{mem, thread, time::Duration};
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HWND, LPARAM, RECT},
        Graphics::Gdi::{
            BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleBitmap, CreateCompatibleDC,
            DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDIBits, GetWindowDC, HBITMAP, HDC, HGDIOBJ,
            ReleaseDC, SelectObject,
        },
        Storage::Xps::PrintWindow,
        System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess},
        UI::WindowsAndMessaging::{
            EnumWindows, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
            GetWindowThreadProcessId, IsIconic, IsWindowVisible, PostMessageW, SW_RESTORE, SW_SHOW,
            SetForegroundWindow, ShowWindow, WM_CLOSE,
        },
    };
    use windows_sys::core::BOOL;

    use super::WindowInfo;

    pub fn visible_windows(pid: Option<u32>, title: Option<&str>) -> Result<Vec<WindowInfo>> {
        struct Context {
            pid: Option<u32>,
            title: Option<String>,
            windows: Vec<WindowInfo>,
        }

        unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let context = unsafe { &mut *(lparam as *mut Context) };
            if unsafe { IsWindowVisible(hwnd) } == 0 || unsafe { IsIconic(hwnd) } != 0 {
                return 1;
            }
            let mut window_pid = 0_u32;
            unsafe {
                GetWindowThreadProcessId(hwnd, &mut window_pid);
            }
            if context.pid.is_some_and(|pid| pid != window_pid) {
                return 1;
            }
            let Ok(window_title) = window_title(hwnd) else {
                return 1;
            };
            if context
                .title
                .as_deref()
                .is_some_and(|needle| !window_title.contains(needle))
            {
                return 1;
            }
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            if unsafe { GetWindowRect(hwnd, &mut rect) } == 0 {
                return 1;
            }
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;
            if width < 200 || height < 200 {
                return 1;
            }
            context.windows.push(WindowInfo {
                hwnd: hwnd as usize,
                pid: window_pid,
                title: window_title,
                rect: [rect.left, rect.top, width, height],
            });
            1
        }

        let mut context = Context {
            pid,
            title: title.map(str::to_string),
            windows: Vec::new(),
        };
        unsafe {
            EnumWindows(Some(enum_proc), &mut context as *mut Context as LPARAM);
        }
        context.windows.sort_by_key(|window| window.hwnd);
        Ok(context.windows)
    }

    pub fn capture_window(window: &WindowInfo) -> Result<RgbImage> {
        let hwnd = window.hwnd as HWND;
        unsafe {
            if IsIconic(hwnd) != 0 {
                ShowWindow(hwnd, SW_RESTORE);
            } else {
                ShowWindow(hwnd, SW_SHOW);
            }
            let _ = SetForegroundWindow(hwnd);
        }
        thread::sleep(Duration::from_millis(200));

        let width = window.rect[2];
        let height = window.rect[3];
        if width <= 0 || height <= 0 {
            bail!("invalid window rect: {:?}", window.rect);
        }

        unsafe {
            let window_dc = GetWindowDC(hwnd);
            if window_dc.is_null() {
                bail!("GetWindowDC failed");
            }
            let memory_dc = CreateCompatibleDC(window_dc);
            if memory_dc.is_null() {
                ReleaseDC(hwnd, window_dc);
                bail!("CreateCompatibleDC failed");
            }
            let bitmap = CreateCompatibleBitmap(window_dc, width, height);
            if bitmap.is_null() {
                DeleteDC(memory_dc);
                ReleaseDC(hwnd, window_dc);
                bail!("CreateCompatibleBitmap failed");
            }
            let old_bitmap = SelectObject(memory_dc, bitmap as HGDIOBJ);
            let result = read_window_bitmap(hwnd, memory_dc, bitmap, width, height);
            if !old_bitmap.is_null() {
                SelectObject(memory_dc, old_bitmap);
            }
            DeleteObject(bitmap as HGDIOBJ);
            DeleteDC(memory_dc);
            ReleaseDC(hwnd, window_dc);
            result
        }
    }

    pub fn close_window(window: &WindowInfo) -> Result<()> {
        let hwnd = window.hwnd as HWND;
        unsafe {
            let _ = PostMessageW(hwnd, WM_CLOSE, 0, 0);
        }
        thread::sleep(Duration::from_secs(2));
        if visible_windows(Some(window.pid), Some(&window.title))?.is_empty() {
            return Ok(());
        }
        unsafe {
            let process = OpenProcess(PROCESS_TERMINATE, 0, window.pid);
            if process.is_null() {
                bail!(
                    "OpenProcess(PROCESS_TERMINATE) failed for pid={}",
                    window.pid
                );
            }
            if TerminateProcess(process, 1) == 0 {
                CloseHandle(process);
                bail!("TerminateProcess failed for pid={}", window.pid);
            }
            CloseHandle(process);
        }
        Ok(())
    }

    unsafe fn read_window_bitmap(
        hwnd: HWND,
        memory_dc: HDC,
        bitmap: HBITMAP,
        width: i32,
        height: i32,
    ) -> Result<RgbImage> {
        if unsafe { PrintWindow(hwnd, memory_dc, 2) } == 0
            && unsafe { PrintWindow(hwnd, memory_dc, 0) } == 0
        {
            bail!("PrintWindow failed");
        }
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [Default::default()],
        };
        let mut bgra = vec![0_u8; width as usize * height as usize * 4];
        let lines = unsafe {
            GetDIBits(
                memory_dc,
                bitmap,
                0,
                height as u32,
                bgra.as_mut_ptr().cast(),
                &mut info,
                DIB_RGB_COLORS,
            )
        };
        if lines != height {
            bail!("GetDIBits returned {lines}, expected {height}");
        }
        let mut rgb = Vec::with_capacity(width as usize * height as usize * 3);
        for pixel in bgra.chunks_exact(4) {
            rgb.push(pixel[2]);
            rgb.push(pixel[1]);
            rgb.push(pixel[0]);
        }
        RgbImage::from_raw(width as u32, height as u32, rgb)
            .ok_or_else(|| anyhow!("invalid image buffer"))
    }

    fn window_title(hwnd: HWND) -> Result<String> {
        let length = unsafe { GetWindowTextLengthW(hwnd) };
        if length <= 0 {
            return Ok(String::new());
        }
        let mut buffer = vec![0_u16; length as usize + 1];
        let written = unsafe { GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
        if written < 0 {
            bail!("GetWindowTextW failed");
        }
        Ok(String::from_utf16_lossy(&buffer[..written as usize]))
    }
}
