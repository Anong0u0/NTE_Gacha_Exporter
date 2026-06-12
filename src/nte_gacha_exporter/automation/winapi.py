from __future__ import annotations

import contextlib
import ctypes
import ctypes.wintypes as wt
import os
import platform
import re
import subprocess
import sys
import time
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import ClassVar

from nte_gacha_exporter.automation.errors import AutomationEnvironmentError
from nte_gacha_exporter.automation.profile import Point, Size
from nte_gacha_exporter.runtime import RUNTIME_LAUNCHER_ENV

TARGET_EXE = "HTGame.exe"
TARGET_CLASS = "UnrealWindow"

SW_SHOWNORMAL = 1
SW_SHOW = 5
SW_RESTORE = 9
VK_ESCAPE = 0x1B
VK_LBUTTON = 0x01
VK_MENU = 0x12
KEYEVENTF_KEYUP = 0x0002
MOUSEEVENTF_LEFTDOWN = 0x0002
MOUSEEVENTF_LEFTUP = 0x0004
ASFW_ANY = -1
INPUT_MOUSE = 0
CMD_SHELL = "cmd.exe"
POWERSHELL_SHELLS = {"powershell.exe", "pwsh.exe"}
SUPPORTED_ADMIN_SHELLS = {CMD_SHELL, *POWERSHELL_SHELLS}


@dataclass(frozen=True)
class AdminRelaunchCommand:
    executable: str
    parameters: str
    workingDirectory: str | None = None


class _MouseInput(ctypes.Structure):
    _fields_ = [
        ("dx", wt.LONG),
        ("dy", wt.LONG),
        ("mouseData", wt.DWORD),
        ("dwFlags", wt.DWORD),
        ("time", wt.DWORD),
        ("dwExtraInfo", ctypes.c_size_t),
    ]


class _InputUnion(ctypes.Union):
    _fields_: ClassVar = [("mi", _MouseInput)]


class _Input(ctypes.Structure):
    _fields_ = [("type", wt.DWORD), ("union", _InputUnion)]


@dataclass(frozen=True)
class ClientRect:
    left: int
    top: int
    right: int
    bottom: int

    @property
    def width(self) -> int:
        return self.right - self.left

    @property
    def height(self) -> int:
        return self.bottom - self.top

    @property
    def size(self) -> Size:
        return Size(self.width, self.height)


@dataclass(frozen=True)
class GameWindow:
    hwnd: int
    pid: int
    className: str
    title: str
    clientRect: ClientRect

    @property
    def clientSize(self) -> Size:
        return self.clientRect.size


def is_windows() -> bool:
    return sys.platform == "win32" or platform.system() == "Windows"


def require_windows() -> None:
    if not is_windows():
        raise AutomationEnvironmentError("auto page requires Windows")


def require_supported_windows() -> None:
    require_windows()


def windows_build_number() -> int | None:
    require_windows()
    build = _rtl_windows_build_number()
    if build is not None:
        return build

    try:
        output = subprocess.run(
            ["cmd.exe", "/c", "ver"],
            check=False,
            capture_output=True,
            text=True,
            timeout=3,
        ).stdout
    except Exception:
        return None
    return _parse_windows_build_from_ver(output)


def _rtl_windows_build_number() -> int | None:
    try:

        class OSVERSIONINFOEXW(ctypes.Structure):
            _fields_ = [
                ("dwOSVersionInfoSize", wt.DWORD),
                ("dwMajorVersion", wt.DWORD),
                ("dwMinorVersion", wt.DWORD),
                ("dwBuildNumber", wt.DWORD),
                ("dwPlatformId", wt.DWORD),
                ("szCSDVersion", wt.WCHAR * 128),
                ("wServicePackMajor", wt.WORD),
                ("wServicePackMinor", wt.WORD),
                ("wSuiteMask", wt.WORD),
                ("wProductType", wt.BYTE),
                ("wReserved", wt.BYTE),
            ]

        version = OSVERSIONINFOEXW()
        version.dwOSVersionInfoSize = ctypes.sizeof(OSVERSIONINFOEXW)
        status = ctypes.windll.ntdll.RtlGetVersion(ctypes.byref(version))
    except Exception:
        return None
    if status != 0 or not version.dwBuildNumber:
        return None
    return int(version.dwBuildNumber)


def _parse_windows_build_from_ver(output: str) -> int | None:
    match = re.search(r"\b\d+\.\d+\.(\d+)(?:\.\d+)?\b", output)
    return int(match.group(1)) if match else None


def is_admin() -> bool:
    require_windows()
    try:
        return bool(ctypes.windll.shell32.IsUserAnAdmin())
    except Exception as exc:
        raise AutomationEnvironmentError(f"cannot detect administrator privilege: {exc}") from exc


def relaunch_as_admin(
    arguments: Sequence[str],
    *,
    target_executable: str | None = None,
    working_directory: str | os.PathLike[str] | None = None,
) -> int:
    require_windows()
    command = build_admin_relaunch_command(
        arguments,
        target_executable=target_executable,
        working_directory=working_directory,
    )
    result = ctypes.windll.shell32.ShellExecuteW(
        None,
        "runas",
        command.executable,
        command.parameters,
        command.workingDirectory,
        SW_SHOWNORMAL,
    )
    if result <= 32:
        raise AutomationEnvironmentError(f"administrator relaunch failed: ShellExecuteW={result}")
    return int(result)


def build_admin_relaunch_command(
    arguments: Sequence[str],
    *,
    target_executable: str | None = None,
    parent_process_name: str | None = None,
    env: dict[str, str] | None = None,
    working_directory: str | os.PathLike[str] | None = None,
) -> AdminRelaunchCommand:
    actual_env = env if env is not None else os.environ
    shell = _supported_shell_name(parent_process_name or _parent_process_name())
    command_args = [target_executable or current_process_executable(actual_env), *arguments]
    actual_working_directory = str(Path(working_directory)) if working_directory is not None else str(Path.cwd())

    if actual_env.get("WT_SESSION"):
        return AdminRelaunchCommand(
            executable="wt.exe",
            parameters=_windows_command_line(["new-tab", shell, *_shell_arguments(shell, command_args)]),
            workingDirectory=actual_working_directory,
        )
    return AdminRelaunchCommand(
        executable=shell,
        parameters=_windows_command_line(_shell_arguments(shell, command_args)),
        workingDirectory=actual_working_directory,
    )


def current_process_executable(env: dict[str, str] | None = None) -> str:
    launcher = (env or os.environ).get(RUNTIME_LAUNCHER_ENV)
    if launcher:
        return launcher
    if is_windows():
        module_file_name = _module_file_name()
        if module_file_name:
            return module_file_name
    return sys.executable


def set_dpi_aware() -> None:
    require_windows()
    user32 = _user32()
    with contextlib.suppress(Exception):
        user32.SetProcessDpiAwarenessContext(ctypes.c_void_p(-4))
        return
    with contextlib.suppress(Exception):
        user32.SetProcessDPIAware()


def resolve_game_window(pid: str | int, *, class_name: str = TARGET_CLASS) -> GameWindow:
    require_windows()
    set_dpi_aware()
    pid_int = int(pid)
    matches = _enum_windows(pid_int, class_name)
    if not matches:
        raise AutomationEnvironmentError(f"{TARGET_EXE} window not found for pid={pid_int} class={class_name}")
    return matches[0]


def refresh_window(window: GameWindow) -> GameWindow:
    return _window_from_hwnd(window.hwnd, window.pid)


def is_foreground(window: GameWindow) -> bool:
    user32 = _user32()
    return _hwnd_value(user32.GetForegroundWindow()) == int(window.hwnd)


def force_foreground(window: GameWindow, *, timeout: float = 2.0, poll_interval: float = 0.05) -> None:
    user32 = _user32()
    hwnd = wt.HWND(window.hwnd)
    if user32.IsIconic(hwnd):
        user32.ShowWindow(hwnd, SW_RESTORE)
    else:
        user32.ShowWindow(hwnd, SW_SHOW)

    with contextlib.suppress(Exception):
        user32.AllowSetForegroundWindow(ASFW_ANY)

    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        _try_force_foreground(hwnd)
        if is_foreground(window):
            return
        time.sleep(poll_interval)
    raise AutomationEnvironmentError(f"failed to bring game window to foreground: hwnd={window.hwnd}")


def foreground_click(window: GameWindow, point: Point, *, down_time: float = 0.03) -> None:
    user32 = _user32()
    force_foreground(window)
    screen_point = client_to_screen(window.hwnd, point)
    user32.SetCursorPos(screen_point.x, screen_point.y)
    time.sleep(0.025)
    if _send_mouse_button(user32, MOUSEEVENTF_LEFTDOWN) == 1:
        time.sleep(down_time)
        if _send_mouse_button(user32, MOUSEEVENTF_LEFTUP) == 1:
            return
    user32.mouse_event(MOUSEEVENTF_LEFTDOWN, 0, 0, 0, 0)
    time.sleep(down_time)
    user32.mouse_event(MOUSEEVENTF_LEFTUP, 0, 0, 0, 0)


def foreground_escape(window: GameWindow, *, down_time: float = 0.03) -> None:
    user32 = _user32()
    force_foreground(window)
    user32.keybd_event(VK_ESCAPE, 0, 0, 0)
    time.sleep(down_time)
    user32.keybd_event(VK_ESCAPE, 0, KEYEVENTF_KEYUP, 0)


def _send_mouse_button(user32, flags: int) -> int:
    event = _Input()
    event.type = INPUT_MOUSE
    event.union.mi = _MouseInput(0, 0, 0, flags, 0, 0)
    return int(user32.SendInput(1, ctypes.byref(event), ctypes.sizeof(_Input)))


def client_to_screen(hwnd: int, point: Point) -> Point:
    user32 = _user32()
    native_point = wt.POINT(point.x, point.y)
    if not user32.ClientToScreen(wt.HWND(hwnd), ctypes.byref(native_point)):
        raise AutomationEnvironmentError("ClientToScreen failed")
    return Point(int(native_point.x), int(native_point.y))


def screen_to_client(hwnd: int, point: Point) -> Point:
    user32 = _user32()
    native_point = wt.POINT(point.x, point.y)
    if not user32.ScreenToClient(wt.HWND(hwnd), ctypes.byref(native_point)):
        raise AutomationEnvironmentError("ScreenToClient failed")
    return Point(int(native_point.x), int(native_point.y))


def cursor_position() -> Point:
    user32 = _user32()
    native_point = wt.POINT()
    if not user32.GetCursorPos(ctypes.byref(native_point)):
        raise AutomationEnvironmentError("GetCursorPos failed")
    return Point(int(native_point.x), int(native_point.y))


def wait_for_client_click(window: GameWindow, *, timeout: float | None = None) -> Point:
    require_windows()
    deadline = time.monotonic() + timeout if timeout is not None else None
    was_down = bool(_user32().GetAsyncKeyState(VK_LBUTTON) & 0x8000)
    while True:
        if deadline is not None and time.monotonic() >= deadline:
            raise AutomationEnvironmentError("timed out waiting for click")
        is_down = bool(_user32().GetAsyncKeyState(VK_LBUTTON) & 0x8000)
        if is_down and not was_down:
            point = screen_to_client(window.hwnd, cursor_position())
            while _user32().GetAsyncKeyState(VK_LBUTTON) & 0x8000:
                time.sleep(0.02)
            return point
        was_down = is_down
        time.sleep(0.02)


def escape_pressed() -> bool:
    require_windows()
    return bool(_user32().GetAsyncKeyState(VK_ESCAPE) & 0x8000)


def _enum_windows(pid: int, class_name: str) -> list[GameWindow]:
    user32 = _user32()
    result: list[GameWindow] = []
    enum_proc_type = ctypes.WINFUNCTYPE(wt.BOOL, wt.HWND, wt.LPARAM)

    def callback(hwnd: wt.HWND, _lparam: wt.LPARAM) -> bool:
        try:
            hwnd_int = _hwnd_value(hwnd)
            if not user32.IsWindowVisible(hwnd) or user32.IsIconic(hwnd):
                return True
            window_pid = wt.DWORD()
            user32.GetWindowThreadProcessId(hwnd, ctypes.byref(window_pid))
            if int(window_pid.value) != pid:
                return True
            window = _window_from_hwnd(hwnd_int, pid)
            if class_name and window.className != class_name:
                return True
            if window.clientRect.width <= 0 or window.clientRect.height <= 0:
                return True
            result.append(window)
        except Exception:
            return True
        return True

    user32.EnumWindows(enum_proc_type(callback), 0)
    return result


def _window_from_hwnd(hwnd: int, pid: int) -> GameWindow:
    user32 = _user32()
    rect = wt.RECT()
    if not user32.GetClientRect(wt.HWND(hwnd), ctypes.byref(rect)):
        raise AutomationEnvironmentError("GetClientRect failed")
    return GameWindow(
        hwnd=hwnd,
        pid=pid,
        className=_class_name(hwnd),
        title=_window_text(hwnd),
        clientRect=ClientRect(int(rect.left), int(rect.top), int(rect.right), int(rect.bottom)),
    )


def _class_name(hwnd: int) -> str:
    buf = ctypes.create_unicode_buffer(256)
    _user32().GetClassNameW(wt.HWND(hwnd), buf, len(buf))
    return buf.value


def _window_text(hwnd: int) -> str:
    user32 = _user32()
    length = user32.GetWindowTextLengthW(wt.HWND(hwnd))
    buf = ctypes.create_unicode_buffer(length + 1)
    user32.GetWindowTextW(wt.HWND(hwnd), buf, len(buf))
    return buf.value


def _user32() -> ctypes.CDLL:
    require_windows()
    return ctypes.windll.user32


def _try_force_foreground(hwnd: wt.HWND) -> None:
    user32 = _user32()
    current_thread = int(ctypes.windll.kernel32.GetCurrentThreadId())
    target_thread = _window_thread_id(hwnd)
    foreground_hwnd = wt.HWND(_hwnd_value(user32.GetForegroundWindow()))
    foreground_thread = _window_thread_id(foreground_hwnd) if foreground_hwnd else 0
    attached: list[int] = []
    for thread_id in {target_thread, foreground_thread}:
        if thread_id and thread_id != current_thread and user32.AttachThreadInput(current_thread, thread_id, True):
            attached.append(thread_id)
    try:
        user32.BringWindowToTop(hwnd)
        user32.SetForegroundWindow(hwnd)
        try:
            user32.SetActiveWindow(hwnd)
            user32.SetFocus(hwnd)
        except Exception:
            pass
    finally:
        for thread_id in attached:
            user32.AttachThreadInput(current_thread, thread_id, False)

    if _hwnd_value(user32.GetForegroundWindow()) != _hwnd_value(hwnd):
        user32.keybd_event(VK_MENU, 0, 0, 0)
        user32.keybd_event(VK_MENU, 0, KEYEVENTF_KEYUP, 0)
        user32.SetForegroundWindow(hwnd)


def _window_thread_id(hwnd: wt.HWND) -> int:
    if not _hwnd_value(hwnd):
        return 0
    pid = wt.DWORD()
    return int(_user32().GetWindowThreadProcessId(hwnd, ctypes.byref(pid)))


def _hwnd_value(hwnd: wt.HWND) -> int:
    return int(getattr(hwnd, "value", hwnd) or 0)


def _supported_shell_name(name: str | None) -> str:
    normalized = Path(name or "").name.lower()
    if normalized in SUPPORTED_ADMIN_SHELLS:
        return normalized
    return CMD_SHELL


def _module_file_name() -> str | None:
    size = 260
    kernel32 = ctypes.windll.kernel32
    while size <= 32768:
        buffer = ctypes.create_unicode_buffer(size)
        length = kernel32.GetModuleFileNameW(None, buffer, size)
        if length == 0:
            return None
        if length < size - 1:
            return buffer.value
        size *= 2
    return None


def _shell_arguments(shell: str, command_args: Sequence[str]) -> list[str]:
    if shell == CMD_SHELL:
        return ["/d", "/c", _windows_command_line(command_args)]
    return [
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        "& " + " ".join(_quote_powershell_arg(part) for part in command_args),
    ]


def _quote_powershell_arg(arg: str) -> str:
    return "'" + str(arg).replace("'", "''") + "'"


def _windows_command_line(args: Sequence[str | os.PathLike[str]]) -> str:
    return subprocess.list2cmdline([str(Path(arg) if isinstance(arg, os.PathLike) else arg) for arg in args])


def _parent_process_name() -> str | None:
    if not is_windows():
        return None
    parent_pid = os.getppid()
    command = f"(Get-CimInstance Win32_Process -Filter 'ProcessId={parent_pid}').Name"
    try:
        output = subprocess.run(
            ["powershell.exe", "-NoProfile", "-Command", command],
            check=False,
            capture_output=True,
            text=True,
            timeout=2,
        ).stdout.strip()
    except Exception:
        return None
    return output or None
