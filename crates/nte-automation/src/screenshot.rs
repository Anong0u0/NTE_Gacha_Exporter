use image::RgbaImage;

#[cfg(windows)]
use crate::error::AutomationError;
use crate::error::AutomationResult;
#[cfg(windows)]
use crate::model::Point;
use crate::model::{Rect, Size};
use crate::window;

#[derive(Debug, Clone)]
pub struct WindowCaptureClient {
    #[cfg_attr(not(windows), allow(dead_code))]
    hwnd: usize,
}

impl WindowCaptureClient {
    pub fn new(hwnd: usize) -> Self {
        Self { hwnd }
    }

    pub fn capture_client(&self, size: Size) -> AutomationResult<RgbaImage> {
        self.capture_rect(Rect {
            x: 0,
            y: 0,
            width: size.width,
            height: size.height,
        })
    }

    #[cfg(not(windows))]
    pub fn capture_rect(&self, _rect: Rect) -> AutomationResult<RgbaImage> {
        let _ = self;
        window::require_windows()?;
        unreachable!()
    }

    #[cfg(windows)]
    pub fn capture_rect(&self, rect: Rect) -> AutomationResult<RgbaImage> {
        use std::mem;
        use std::ptr::null_mut;

        use windows_sys::Win32::Graphics::Gdi::{
            BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleBitmap,
            CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, HGDIOBJ,
            ReleaseDC, SRCCOPY, SelectObject,
        };

        if rect.width == 0 || rect.height == 0 {
            return Err(AutomationError::message("capture rect must be positive"));
        }
        let top_left = window::client_to_screen(
            self.hwnd,
            Point {
                x: rect.x,
                y: rect.y,
            },
        )?;
        let width = rect.width as i32;
        let height = rect.height as i32;
        unsafe {
            let screen_dc = GetDC(null_mut());
            if screen_dc.is_null() {
                return Err(AutomationError::message("GetDC failed"));
            }
            let mem_dc = CreateCompatibleDC(screen_dc);
            if mem_dc.is_null() {
                ReleaseDC(null_mut(), screen_dc);
                return Err(AutomationError::message("CreateCompatibleDC failed"));
            }
            let bitmap = CreateCompatibleBitmap(screen_dc, width, height);
            if bitmap.is_null() {
                DeleteDC(mem_dc);
                ReleaseDC(null_mut(), screen_dc);
                return Err(AutomationError::message("CreateCompatibleBitmap failed"));
            }
            let old = SelectObject(mem_dc, bitmap as HGDIOBJ);
            let result = (|| {
                if BitBlt(
                    mem_dc, 0, 0, width, height, screen_dc, top_left.x, top_left.y, SRCCOPY,
                ) == 0
                {
                    return Err(AutomationError::message("BitBlt failed"));
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
                let mut bgra = vec![0_u8; rect.width as usize * rect.height as usize * 4];
                let lines = GetDIBits(
                    mem_dc,
                    bitmap,
                    0,
                    rect.height,
                    bgra.as_mut_ptr().cast(),
                    &mut info,
                    DIB_RGB_COLORS,
                );
                if lines == 0 {
                    return Err(AutomationError::message("GetDIBits failed"));
                }
                for pixel in bgra.chunks_exact_mut(4) {
                    pixel.swap(0, 2);
                    pixel[3] = 255;
                }
                RgbaImage::from_raw(rect.width, rect.height, bgra)
                    .ok_or_else(|| AutomationError::message("invalid capture buffer"))
            })();
            SelectObject(mem_dc, old);
            DeleteObject(bitmap as HGDIOBJ);
            DeleteDC(mem_dc);
            ReleaseDC(null_mut(), screen_dc);
            result
        }
    }
}
