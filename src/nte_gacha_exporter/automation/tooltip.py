from __future__ import annotations

import ctypes
import ctypes.wintypes as wt

from nte_gacha_exporter.automation import winapi

WS_POPUP = 0x80000000
WS_BORDER = 0x00800000
SS_LEFT = 0x00000000
SS_NOPREFIX = 0x00000080
WS_EX_TOPMOST = 0x00000008
WS_EX_TOOLWINDOW = 0x00000080
WS_EX_NOACTIVATE = 0x08000000
SW_HIDE = 0
SW_SHOWNOACTIVATE = 4
SWP_NOACTIVATE = 0x0010
SWP_SHOWWINDOW = 0x0040
HWND_TOPMOST = -1
SM_CXSCREEN = 0
SM_CYSCREEN = 1
DT_LEFT = 0x00000000
DT_WORDBREAK = 0x00000010
DT_CALCRECT = 0x00000400
DT_NOPREFIX = 0x00000800
WM_SETFONT = 0x0030
DEFAULT_GUI_FONT = 17
MAX_TOOLTIP_WIDTH = 420


class AutomationTooltip:
    def __init__(self, *, enabled: bool = True, offset_x: int = 48, offset_y: int = 32) -> None:
        self.enabled = enabled
        self.offset_x = offset_x
        self.offset_y = offset_y
        self.hwnd: int | None = None
        self.unavailable_reason: str | None = None
        if not enabled:
            self.unavailable_reason = "disabled"
            return
        if not winapi.is_windows():
            self.unavailable_reason = "Windows tooltip requires Windows"
            return
        try:
            self.hwnd = self._create_window()
        except Exception as exc:
            self.hwnd = None
            self.unavailable_reason = str(exc)

    def show(self, message: str) -> bool:
        if self.hwnd is None:
            return False
        user32 = ctypes.windll.user32
        hwnd = wt.HWND(self.hwnd)
        if not message:
            user32.ShowWindow(hwnd, SW_HIDE)
            return True

        width, height = self._measure(message)
        point = wt.POINT()
        user32.GetCursorPos(ctypes.byref(point))
        screen_width = int(user32.GetSystemMetrics(SM_CXSCREEN))
        screen_height = int(user32.GetSystemMetrics(SM_CYSCREEN))
        x = min(max(0, int(point.x) + self.offset_x), max(0, screen_width - width))
        y = min(max(0, int(point.y) + self.offset_y), max(0, screen_height - height))

        user32.SetWindowTextW(hwnd, message)
        user32.SetWindowPos(
            hwnd,
            wt.HWND(HWND_TOPMOST),
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        )
        user32.ShowWindow(hwnd, SW_SHOWNOACTIVATE)
        user32.InvalidateRect(hwnd, None, True)
        user32.UpdateWindow(hwnd)
        return True

    def close(self) -> None:
        if self.hwnd is None:
            return
        try:
            ctypes.windll.user32.DestroyWindow(wt.HWND(self.hwnd))
        finally:
            self.hwnd = None

    def _create_window(self) -> int:
        user32 = ctypes.windll.user32
        kernel32 = ctypes.windll.kernel32
        gdi32 = ctypes.windll.gdi32
        user32.CreateWindowExW.restype = wt.HWND
        gdi32.GetStockObject.restype = wt.HGDIOBJ
        hwnd = user32.CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            "STATIC",
            "",
            WS_POPUP | WS_BORDER | SS_LEFT | SS_NOPREFIX,
            0,
            0,
            1,
            1,
            None,
            None,
            kernel32.GetModuleHandleW(None),
            None,
        )
        hwnd_value = _handle_value(hwnd)
        if not hwnd_value:
            raise OSError("CreateWindowExW failed")
        font = gdi32.GetStockObject(DEFAULT_GUI_FONT)
        if font:
            user32.SendMessageW(hwnd, WM_SETFONT, font, True)
        return hwnd_value

    def _measure(self, message: str) -> tuple[int, int]:
        user32 = ctypes.windll.user32
        hwnd = wt.HWND(self.hwnd or 0)
        rect = wt.RECT(0, 0, MAX_TOOLTIP_WIDTH, 0)
        hdc = user32.GetDC(hwnd)
        if not hdc:
            return (min(MAX_TOOLTIP_WIDTH, max(80, len(message) * 8 + 12)), 28)
        try:
            user32.DrawTextW(
                hdc,
                message,
                -1,
                ctypes.byref(rect),
                DT_LEFT | DT_WORDBREAK | DT_CALCRECT | DT_NOPREFIX,
            )
        finally:
            user32.ReleaseDC(hwnd, hdc)
        width = min(MAX_TOOLTIP_WIDTH, max(80, int(rect.right - rect.left) + 12))
        height = max(24, int(rect.bottom - rect.top) + 8)
        return (width, height)


def _handle_value(handle: object) -> int:
    return int(getattr(handle, "value", handle) or 0)
