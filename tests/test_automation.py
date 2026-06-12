from __future__ import annotations

import threading
from types import SimpleNamespace

import pytest

from nte_gacha_exporter.automation import winapi
from nte_gacha_exporter.automation.errors import AutomationEnvironmentError
from nte_gacha_exporter.automation.ocr import PageNumber, parse_page_text
from nte_gacha_exporter.automation.profile import (
    Point,
    Rect,
    Size,
    ensure_supported_client_size,
    load_profile,
)
from nte_gacha_exporter.automation.template import find_template
from nte_gacha_exporter.automation.winapi import (
    _hwnd_value,
    _parse_windows_build_from_ver,
    build_admin_relaunch_command,
)
from nte_gacha_exporter.capture.windows_net import CaptureTarget


def test_parse_page_text_accepts_ocr_noise():
    page = parse_page_text("I / O3")

    assert page.current == 1
    assert page.total == 3


def test_default_profile_is_ready_workflow():
    profile = load_profile()

    assert profile.schemaVersion == 2
    assert profile.baseClientSize == Size(1920, 1080)
    assert profile.points["homeBoardFile"] == Point(450, 1000)
    assert profile.points["forkActivityCard"] == Point(420, 560)
    assert set(profile.templates) == {
        "homeBoardFileIcon",
        "homeForkEntryIcon",
        "recordTabSelectedCap",
        "forkShopSelectedIcon",
        "forkDetailFileIcon",
        "forkActivityTimeIcon",
    }
    assert [step.action for step in profile.workflow][:3] == ["verifyTemplate", "click", "click"]
    assert any(step.action == "clickTemplateUntilTemplate" for step in profile.workflow)
    assert any(
        step.action == "clickTemplateUntilTemplate"
        and step.template == "forkActivityTimeIcon"
        and step.targetTemplate == "forkDetailFileIcon"
        and step.point == "forkActivityCard"
        for step in profile.workflow
    )


def test_profile_scales_coordinates_and_templates():
    profile = load_profile()
    scaled = profile.scaled(Size(960, 540))

    assert scaled.points["homeBoardFile"] == Point(225, 500)
    assert scaled.rects["boardPageNumber"] == Rect(458, 430, 48, 30)
    assert scaled.templates["homeBoardFileIcon"].rect == Rect(210, 485, 27, 25)


def test_profile_rejects_non_16_9_client_size():
    with pytest.raises(AutomationEnvironmentError, match="16:9"):
        ensure_supported_client_size(Size(1920, 1200))


def test_auto_pager_warns_when_client_is_not_1920x1080(monkeypatch):
    from nte_gacha_exporter.automation import pager

    statuses = []

    class FakeTooltip:
        unavailable_reason = None

        def __init__(self, *, enabled=True):
            self.enabled = enabled

        def show(self, message):
            return None

        def close(self):
            return None

    monkeypatch.setattr(pager.winapi, "require_windows", lambda: None)
    monkeypatch.setattr(
        pager.winapi,
        "resolve_game_window",
        lambda pid, class_name: SimpleNamespace(hwnd=1, pid=pid, className=class_name, clientSize=Size(2560, 1440)),
    )
    monkeypatch.setattr(pager, "WindowCaptureClient", lambda hwnd: SimpleNamespace(hwnd=hwnd))
    monkeypatch.setattr(pager, "WindowsOcrClient", lambda: object())
    monkeypatch.setattr(pager, "ImageTemplateMatcher", lambda profile: SimpleNamespace(profile=profile))
    monkeypatch.setattr(pager, "AutomationTooltip", FakeTooltip)

    pager.AutoPager(
        pager.AutoPageOptions(
            target=CaptureTarget("1234", "iface", [], None, ""),
            stop_event=threading.Event(),
            on_status=statuses.append,
        )
    )

    assert any(
        status.kind == "diagnostic" and "non-1920x1080 auto page may be inaccurate" in status.message
        for status in statuses
    )


def test_hwnd_value_handles_ctypes_handles():
    import ctypes

    assert _hwnd_value(ctypes.c_void_p(123)) == 123
    assert _hwnd_value(456) == 456


def test_admin_relaunch_command_preserves_cmd_shell():
    command = build_admin_relaunch_command(
        ["-m", "nte_gacha_exporter.tui.main"],
        target_executable=r"C:\Python\python.exe",
        parent_process_name="cmd.exe",
        env={},
        working_directory=r"C:\Tools\nte-gacha",
    )

    assert command.executable == "cmd.exe"
    assert command.workingDirectory == r"C:\Tools\nte-gacha"
    assert "/d /c" in command.parameters
    assert r"C:\Python\python.exe" in command.parameters
    assert "nte_gacha_exporter.tui.main" in command.parameters


def test_admin_relaunch_command_preserves_powershell_shell():
    command = build_admin_relaunch_command(
        ["-m", "nte_gacha_exporter.tui.main"],
        target_executable=r"C:\Python\python.exe",
        parent_process_name="powershell.exe",
        env={},
    )

    assert command.executable == "powershell.exe"
    assert "-NoProfile" in command.parameters
    assert "-Command" in command.parameters
    assert "& 'C:\\Python\\python.exe'" in command.parameters


def test_admin_relaunch_command_uses_windows_terminal_when_present():
    command = build_admin_relaunch_command(
        ["-m", "nte_gacha_exporter.tui.main"],
        target_executable=r"C:\Python\python.exe",
        parent_process_name="pwsh.exe",
        env={"WT_SESSION": "1"},
        working_directory=r"C:\Tools\nte-gacha",
    )

    assert command.executable == "wt.exe"
    assert command.workingDirectory == r"C:\Tools\nte-gacha"
    assert "new-tab" in command.parameters
    assert "pwsh.exe" in command.parameters
    assert "nte_gacha_exporter.tui.main" in command.parameters


def test_admin_relaunch_command_defaults_to_current_working_directory(monkeypatch, tmp_path):
    monkeypatch.chdir(tmp_path)

    command = build_admin_relaunch_command(
        ["capture", "--auto-page"],
        target_executable=r"C:\Python\python.exe",
        parent_process_name="cmd.exe",
        env={},
    )

    assert command.workingDirectory == str(tmp_path)


def test_admin_relaunch_command_uses_process_module_executable(monkeypatch):
    monkeypatch.setattr(winapi, "is_windows", lambda: True)
    monkeypatch.setattr(winapi, "_module_file_name", lambda: r"C:\app\nte-gacha-cli.exe")
    monkeypatch.setattr(winapi.sys, "executable", r"C:\app\python.exe")

    command = winapi.build_admin_relaunch_command(
        ["capture", "--auto-page"],
        parent_process_name="powershell.exe",
        env={},
    )

    assert r"& 'C:\app\nte-gacha-cli.exe'" in command.parameters
    assert "python.exe" not in command.parameters


def test_admin_relaunch_command_prefers_launcher_env(monkeypatch):
    monkeypatch.setattr(winapi, "is_windows", lambda: True)
    monkeypatch.setattr(winapi, "_module_file_name", lambda: r"C:\app\bin\nte-gacha-core.exe")
    monkeypatch.setattr(winapi.sys, "executable", r"C:\app\bin\nte-gacha-core.exe")

    command = winapi.build_admin_relaunch_command(
        ["capture", "--auto-page"],
        parent_process_name="powershell.exe",
        env={"NTE_GACHA_LAUNCHER": r"C:\app\nte-gacha-cli.exe"},
    )

    assert r"& 'C:\app\nte-gacha-cli.exe'" in command.parameters
    assert "nte-gacha-core.exe" not in command.parameters


def test_current_process_executable_falls_back_to_sys_executable(monkeypatch):
    monkeypatch.setattr(winapi, "is_windows", lambda: True)
    monkeypatch.setattr(winapi, "_module_file_name", lambda: None)
    monkeypatch.setattr(winapi.sys, "executable", r"C:\Python\python.exe")

    assert winapi.current_process_executable() == r"C:\Python\python.exe"


def test_template_matcher_finds_bounded_offset():
    screen = FakeImage.solid(8, 8, (0, 0, 0))
    template = FakeImage.solid(2, 2, (255, 0, 0))
    screen.paint(5, 4, template)

    match = find_template(
        name="red",
        screen_image=screen,
        template_image=template,
        expected_rect=Rect(4, 3, 2, 2),
        search_padding=Point(2, 2),
        threshold=0,
        step=1,
    )

    assert match.matched is True
    assert match.point == Point(5, 4)


def test_template_matcher_returns_best_match_not_first_under_threshold():
    screen = FakeImage.solid(8, 8, (0, 0, 0))
    template = FakeImage.solid(2, 2, (255, 0, 0))
    near_template = FakeImage.solid(2, 2, (250, 0, 0))
    screen.paint(1, 1, near_template)
    screen.paint(4, 4, template)

    match = find_template(
        name="red",
        screen_image=screen,
        template_image=template,
        expected_rect=Rect(1, 1, 2, 2),
        search_padding=Point(4, 4),
        threshold=2,
        step=1,
    )

    assert match.matched is True
    assert match.score == 0
    assert match.point == Point(4, 4)


def test_template_matcher_can_prefer_expected_rect_for_profile_verification():
    screen = FakeImage.solid(8, 8, (0, 0, 0))
    template = FakeImage.solid(2, 2, (255, 0, 0))
    near_template = FakeImage.solid(2, 2, (250, 0, 0))
    screen.paint(1, 1, near_template)
    screen.paint(4, 4, template)

    match = find_template(
        name="red",
        screen_image=screen,
        template_image=template,
        expected_rect=Rect(1, 1, 2, 2),
        search_padding=Point(4, 4),
        threshold=2,
        step=1,
        prefer_expected=True,
    )

    assert match.matched is True
    assert match.point == Point(1, 1)


def test_template_matcher_prefer_expected_does_not_scan_on_miss():
    screen = FakeImage.solid(8, 8, (0, 0, 0))
    template = FakeImage.solid(2, 2, (255, 0, 0))
    screen.paint(4, 4, template)

    match = find_template(
        name="red",
        screen_image=screen,
        template_image=template,
        expected_rect=Rect(1, 1, 2, 2),
        search_padding=Point(4, 4),
        threshold=0,
        step=1,
        prefer_expected=True,
    )

    assert match.matched is False
    assert match.point == Point(1, 1)


def test_auto_pager_runs_v2_workflow_sequence(monkeypatch):
    from nte_gacha_exporter.automation import pager

    profile = load_profile()
    click_names: list[str] = []
    escape_count = 0
    statuses = []
    sleeps: list[float] = []

    class FakeCapture:
        def __init__(self, hwnd):
            self.hwnd = hwnd

        def capture_client(self, size):
            return object()

        def capture_rect(self, rect):
            return object()

    class FakeOcr:
        def __init__(self):
            self.pages = [
                PageNumber(1, 2, "1/2"),
                PageNumber(2, 2, "2/2"),
                PageNumber(1, 1, "1/1"),
                PageNumber(1, 3, "1/3"),
                PageNumber(2, 3, "2/3"),
                PageNumber(3, 3, "3/3"),
            ]

        def read_page_number(self, image):
            return self.pages.pop(0)

    class FakeMatcher:
        def __init__(self, profile):
            self.profile = profile

        def verify(self, name, image):
            if name == "forkDetailFileIcon" and "forkActivityCard" not in click_names:
                raise AutomationEnvironmentError("detail pending")
            return SimpleNamespace(score=0.0, point=Point(0, 0))

        def verify_exact_crop(self, name, image):
            return self.verify(name, image)

        def find(self, name, image):
            return SimpleNamespace(score=0.0, point=Point(401, 541))

    class FakeTooltip:
        unavailable_reason = None

        def __init__(self, *, enabled=True):
            self.enabled = enabled

        def show(self, message):
            return None

        def close(self):
            return None

    def fake_foreground_click(window, point, *, down_time=0.03):
        for name, candidate in profile.points.items():
            if candidate == point:
                click_names.append(name)
                return
        click_names.append(f"{point.x},{point.y}")

    def fake_foreground_escape(window):
        nonlocal escape_count
        escape_count += 1

    monkeypatch.setattr(pager, "load_profile", lambda: profile)
    monkeypatch.setattr(pager.winapi, "require_windows", lambda: None)
    monkeypatch.setattr(
        pager.winapi,
        "resolve_game_window",
        lambda pid, class_name: SimpleNamespace(hwnd=1, pid=pid, className=class_name, clientSize=Size(1920, 1080)),
    )
    monkeypatch.setattr(pager.winapi, "refresh_window", lambda window: window)
    monkeypatch.setattr(pager.winapi, "force_foreground", lambda window: None)
    monkeypatch.setattr(pager.winapi, "escape_pressed", lambda: False)
    monkeypatch.setattr(pager.winapi, "foreground_click", fake_foreground_click)
    monkeypatch.setattr(pager.winapi, "foreground_escape", fake_foreground_escape)
    monkeypatch.setattr(pager, "WindowCaptureClient", FakeCapture)
    monkeypatch.setattr(pager, "WindowsOcrClient", FakeOcr)
    monkeypatch.setattr(pager, "ImageTemplateMatcher", FakeMatcher)
    monkeypatch.setattr(pager, "AutomationTooltip", FakeTooltip)
    monkeypatch.setattr(pager.time, "sleep", sleeps.append)

    result = pager.run_auto_page(
        pager.AutoPageOptions(
            target=CaptureTarget("1234", "iface", [], None, ""),
            stop_event=threading.Event(),
            on_status=statuses.append,
        )
    )

    assert result.succeeded is True
    assert result.completedPools == ("limited", "standard", "fork")
    assert escape_count == 1
    assert "homeBoardFile" in click_names
    assert "forkActivityCard" in click_names
    assert click_names.count("boardNextButton") == 1
    assert click_names.count("forkNextButton") == 2
    assert 0.6 not in sleeps
    assert any(status.kind == "step" and status.step == "verifyMarketHome" for status in statuses)
    assert any(
        status.kind == "page" and status.pool == "limited" and status.currentPage == 2 and status.totalPages == 2
        for status in statuses
    )
    assert any(status.kind == "pool_completed" and status.pool == "fork" for status in statuses)


def test_click_template_until_template_waits_for_target_after_source_disappears(monkeypatch):
    from nte_gacha_exporter.automation import pager

    profile = SimpleNamespace(
        points={"forkActivityCard": Point(420, 560)},
        templates={"forkActivityTimeIcon": SimpleNamespace(rect=Rect(407, 546, 26, 28))},
    )
    pager_instance = pager.AutoPager.__new__(pager.AutoPager)
    pager_instance.profile = profile
    pager_instance.options = SimpleNamespace(
        template_timeout=0.5,
        click_poll_interval=0.001,
        stop_event=threading.Event(),
    )
    clicked = False
    clicks: list[Point] = []
    target_checks = 0
    source_checks = 0
    statuses = []

    def fake_try_template(name):
        nonlocal target_checks
        assert name == "forkDetailFileIcon"
        target_checks += 1
        if target_checks < 3:
            raise AutomationEnvironmentError("target pending")
        return SimpleNamespace(score=0.6, point=Point(1708, 262))

    def fake_find_template(name):
        nonlocal source_checks
        assert name == "forkActivityTimeIcon"
        source_checks += 1
        if clicked:
            raise AutomationEnvironmentError("source gone")
        return SimpleNamespace(score=0.0, point=Point(407, 546))

    def fake_click(point, *, settle=None):
        nonlocal clicked
        clicked = True
        clicks.append(point)

    pager_instance._try_template = fake_try_template
    pager_instance._find_template = fake_find_template
    pager_instance._click = fake_click
    pager_instance._should_stop = lambda: False
    pager_instance._status = lambda message, **kwargs: statuses.append((message, kwargs))
    monkeypatch.setattr(pager.time, "sleep", lambda seconds: None)

    pager_instance._click_template_until_template(
        SimpleNamespace(
            template="forkActivityTimeIcon",
            targetTemplate="forkDetailFileIcon",
            point="forkActivityCard",
            settle=0.0,
            status="arcResearch",
        )
    )

    assert clicks == [Point(420, 560)]
    assert source_checks == 1
    assert target_checks == 3
    assert statuses[-1][0] == "template verified"


def test_tooltip_disabled_reports_unavailable_reason():
    from nte_gacha_exporter.automation.tooltip import AutomationTooltip

    tooltip = AutomationTooltip(enabled=False)

    assert tooltip.offset_x == 48
    assert tooltip.offset_y == 32
    assert tooltip.unavailable_reason == "disabled"
    assert tooltip.show("hello") is False


def test_parse_windows_build_from_ver_uses_build_not_ubr():
    build = _parse_windows_build_from_ver("Microsoft Windows [Version 10.0.22631.4460]")

    assert build == 22631


class FakeImage:
    def __init__(self, width: int, height: int, pixels: list[tuple[int, int, int]]) -> None:
        self.size = (width, height)
        self.pixels = pixels

    @classmethod
    def solid(cls, width: int, height: int, color: tuple[int, int, int]) -> FakeImage:
        return cls(width, height, [color for _ in range(width * height)])

    def convert(self, mode: str) -> FakeImage:
        assert mode == "RGB"
        return self

    def crop(self, box: tuple[int, int, int, int]) -> FakeImage:
        left, top, right, bottom = box
        width, _height = self.size
        pixels = []
        for y in range(top, bottom):
            for x in range(left, right):
                pixels.append(self.pixels[y * width + x])
        return FakeImage(right - left, bottom - top, pixels)

    def tobytes(self) -> bytes:
        return bytes(channel for pixel in self.pixels for channel in pixel)

    def paint(self, left: int, top: int, image: FakeImage) -> None:
        width, _height = self.size
        image_width, image_height = image.size
        for y in range(image_height):
            for x in range(image_width):
                self.pixels[(top + y) * width + left + x] = image.pixels[y * image_width + x]
