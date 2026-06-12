from __future__ import annotations

from nte_gacha_exporter.automation.errors import AutomationEnvironmentError
from nte_gacha_exporter.automation.profile import Point, Rect
from nte_gacha_exporter.automation.winapi import client_to_screen, require_windows


class WindowCaptureClient:
    def __init__(self, hwnd: int) -> None:
        self.hwnd = hwnd

    def capture_client(self, size):
        return self.capture_rect(Rect(0, 0, size.width, size.height))

    def capture_rect(self, rect: Rect):
        require_windows()
        try:
            from PIL import ImageGrab
        except Exception as exc:
            raise AutomationEnvironmentError(f"Pillow ImageGrab unavailable: {exc}") from exc

        left_top = client_to_screen(self.hwnd, Point(rect.x, rect.y))
        right_bottom = client_to_screen(self.hwnd, Point(rect.right, rect.bottom))
        bbox = (left_top.x, left_top.y, right_bottom.x, right_bottom.y)
        try:
            return ImageGrab.grab(bbox=bbox, all_screens=True).convert("RGBA")
        except Exception as exc:
            raise AutomationEnvironmentError(f"screen capture failed: {exc}") from exc
