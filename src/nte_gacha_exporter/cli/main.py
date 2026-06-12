from __future__ import annotations

import argparse
import contextlib
import platform
import select
import signal
import sys
import threading
import time
from collections.abc import Callable
from pathlib import Path
from typing import TYPE_CHECKING, Any, TextIO

from nte_gacha_exporter.app.auto_page_status import AutoPageStatusFormatter
from nte_gacha_exporter.app.operations import (
    is_frozen,
    result_path_lines,
    run_debug_export,
    run_doctor,
    run_interfaces,
    run_maps_build,
    run_maps_list,
)
from nte_gacha_exporter.app.output import apply_history_output_defaults, history_default_help
from nte_gacha_exporter.app.summary import (
    add_capture_counts as _add_capture_counts,
)
from nte_gacha_exporter.app.summary import (
    empty_capture_counts as _empty_capture_counts,
)
from nte_gacha_exporter.app.summary import (
    format_capture_counts as _format_capture_counts,
)
from nte_gacha_exporter.app.summary import (
    offline_capture_counts as _offline_capture_counts,
)
from nte_gacha_exporter.app.summary import (
    record_line as _record_line,
)
from nte_gacha_exporter.app.summary import (
    summary_text as _summary_text,
)
from nte_gacha_exporter.capture.live import (
    CaptureEnvironmentError,
    CaptureHistoryOptions,
    capture_history,
)
from nte_gacha_exporter.core.models import GachaRecord
from nte_gacha_exporter.core.schema import ExportDocument, LocalizationMap
from nte_gacha_exporter.mapping.runtime import DEFAULT_LOCALE, load_locale_map

if TYPE_CHECKING:
    from nte_gacha_exporter.automation.pager import AutoPageStatus


def _add_locale_args(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--locale",
        default=DEFAULT_LOCALE,
        help=f"Bundled locale, or locale=custom.json for a custom map. Default: {DEFAULT_LOCALE}",
    )


def _add_history_output_args(parser: argparse.ArgumentParser, *, debug_json: bool = False) -> None:
    parser.add_argument("--json", help=f"Public JSON output path. Default: {history_default_help('json')}")
    parser.add_argument("--csv", help=f"Public CSV output path. Default: {history_default_help('csv')}")
    if debug_json:
        parser.add_argument(
            "--debug-json",
            nargs="?",
            const="",
            help=f"Diagnostics JSON output path. Uses {history_default_help('debug')} when omitted",
        )
    _add_locale_args(parser)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="nte-gacha-cli")
    parser.add_argument("--version", action="store_true", help="Show version and exit")
    sub = parser.add_subparsers(dest="command", required=False)

    capture = sub.add_parser("capture", help="Capture live game packets and write public JSON/CSV")
    _add_history_output_args(capture)
    capture.add_argument(
        "--output-raw",
        nargs="?",
        const="",
        help=f"Also save private raw packet JSONL. Uses {history_default_help('raw')} when omitted",
    )
    capture.add_argument("--pid")
    capture.add_argument("--iface")
    capture.add_argument("--auto-page", action="store_true", help="Automatically open and page through gacha history")
    capture.add_argument("-v", "--verbose", action="store_true", help="Print decoded records during capture")

    debug = sub.add_parser("debug", help="Debug, replay, and development tools")
    debug_sub = debug.add_subparsers(dest="debug_command", required=True)

    debug_export = debug_sub.add_parser("export", help="Rebuild public outputs from private raw JSONL")
    debug_export.add_argument("raw_jsonl", help="private raw packet JSONL")
    _add_history_output_args(debug_export, debug_json=True)

    debug_sub.add_parser("doctor", help="Check live capture environment")

    interfaces = debug_sub.add_parser("interfaces", help="List Scapy/Npcap interfaces")
    interfaces.set_defaults(debug_command="interfaces")

    maps = debug_sub.add_parser("maps", help="Localization map tools")
    maps_sub = maps.add_subparsers(dest="maps_command", required=True)
    maps_sub.add_parser("list", help="List bundled locales")
    maps_build = maps_sub.add_parser("build", help="Build localization maps from NTE_Assets")
    maps_build.add_argument("--assets-root")
    maps_build.add_argument("--locale")
    maps_build.add_argument("--out-dir")

    return parser


def _print_version() -> int:
    from nte_gacha_exporter import __version__

    print(__version__)
    return 0


class CaptureProgress:
    def __init__(self, mapping: LocalizationMap, *, verbose: bool, stream: TextIO | None = None) -> None:
        self.mapping = mapping
        self.counts = _empty_capture_counts()
        self.verbose = verbose
        self.stream = stream or sys.stdout
        self._last_width = 0
        self._rendered = False
        self._line_prefix = ""

    def update(self, records: list[GachaRecord]) -> None:
        _add_capture_counts(self.counts, records)

        if self.verbose and records:
            self._clear_line()
            for record in records:
                self.stream.write(_record_line(record) + "\n")
        self._redraw()

    def replace(self, records: list[GachaRecord] | list[dict[str, Any]]) -> None:
        self.counts = _empty_capture_counts()
        _add_capture_counts(self.counts, records)
        self._redraw()

    def summary_suffix(self) -> str:
        return _format_capture_counts(self.mapping, self.counts)

    def set_line_prefix(self, prefix: str) -> None:
        self._line_prefix = prefix
        self._redraw()

    def clear_line_prefix(self) -> None:
        self._line_prefix = ""

    def finish(self) -> None:
        if self._rendered:
            self.stream.write("\n")
            self.stream.flush()
            self._rendered = False

    def _redraw(self) -> None:
        text = self._line_text()
        padding = " " * max(0, self._last_width - len(text))
        self.stream.write(f"\r{text}{padding}")
        self.stream.flush()
        self._last_width = len(text)
        self._rendered = True

    def _line_text(self) -> str:
        suffix = self.summary_suffix()
        if self._line_prefix:
            return f"{self._line_prefix} | {suffix}"
        return suffix

    def _clear_line(self) -> None:
        if not self._rendered:
            return
        self.stream.write("\r" + (" " * self._last_width) + "\r")
        self.stream.flush()
        self._rendered = False


class AutoPageProgress:
    def __init__(
        self,
        mapping: LocalizationMap,
        capture_progress: CaptureProgress,
        *,
        stream: TextIO | None = None,
        use_cr: bool | None = None,
    ) -> None:
        self.mapping = mapping
        self.formatter = AutoPageStatusFormatter(mapping)
        self.capture_progress = capture_progress
        self.stream = stream or capture_progress.stream
        self.use_cr = True if use_cr is None else use_cr

    def update(self, status: AutoPageStatus) -> None:
        text = self.status_line(status, include_elapsed=True, include_detail=not self.use_cr)
        if self.use_cr:
            self.capture_progress.set_line_prefix(text)
            return
        self.stream.write(f"auto_page: {text} | {self.capture_progress.summary_suffix()}\n")
        self.stream.flush()

    def tooltip_text(self, status: AutoPageStatus) -> str:
        return self.status_line(status, include_elapsed=False, include_detail=False)

    def finish(self) -> None:
        if not self.use_cr:
            return
        self.capture_progress.clear_line_prefix()
        self.capture_progress.finish()

    def status_line(
        self,
        status: AutoPageStatus,
        *,
        include_elapsed: bool,
        include_detail: bool,
    ) -> str:
        return self.formatter.status_line(status, include_elapsed=include_elapsed, include_detail=include_detail)


def _print_output_paths(args: argparse.Namespace, *, private_raw: str | None = None) -> None:
    print(f"json={args.json}")
    if getattr(args, "debug_json", None):
        print(f"debug_json={args.debug_json}")
    if args.csv:
        print(f"csv={args.csv}")
    if private_raw:
        print(f"private_raw={private_raw}")


def _wait_for_q(stop_event: threading.Event, stream: TextIO) -> None:
    if platform.system() == "Windows":
        import msvcrt

        while not stop_event.is_set():
            try:
                key = msvcrt.getwch() if msvcrt.kbhit() else ""
            except KeyboardInterrupt:
                stop_event.set()
                return
            if key.lower() == "q":
                stop_event.set()
                return
            time.sleep(0.1)
        return

    try:
        while not stop_event.is_set():
            readable, _writable, _errors = select.select([stream], [], [], 0.1)
            if readable and stream.read(1).lower() == "q":
                stop_event.set()
                return
    except KeyboardInterrupt:
        stop_event.set()


def _start_q_listener(stop_event: threading.Event) -> threading.Thread | None:
    if not sys.stdin.isatty():
        return None
    thread = threading.Thread(target=_wait_for_q, args=(stop_event, sys.stdin), daemon=True)
    thread.start()
    return thread


def _install_sigint_stop(stop_event: threading.Event) -> Any:
    try:
        previous = signal.getsignal(signal.SIGINT)

        def handle_sigint(signum: int, frame: Any) -> None:
            if stop_event.is_set() and callable(previous):
                previous(signum, frame)
                return
            stop_event.set()

        signal.signal(signal.SIGINT, handle_sigint)
    except ValueError:
        return None
    return previous


def _restore_sigint_handler(previous: Any) -> None:
    if previous is None:
        return
    with contextlib.suppress(ValueError):
        signal.signal(signal.SIGINT, previous)


def _run_debug_export(args: argparse.Namespace, mapping: LocalizationMap) -> int:
    result = run_debug_export(
        raw_jsonl=Path(args.raw_jsonl),
        locale=args.locale,
        json_out=Path(args.json),
        csv_out=Path(args.csv) if args.csv else None,
        debug_json_out=Path(args.debug_json) if args.debug_json else None,
    )
    if result.error:
        print(result.error)
        return result.exitCode
    assert result.document is not None
    print(_summary_text(result.document, capture_counts=_offline_capture_counts(result.document, mapping)))
    for line in result_path_lines(result.paths):
        print(line)
    return result.exitCode


def _run_live_capture(args: argparse.Namespace, mapping: LocalizationMap) -> int:
    if args.auto_page:
        return _run_auto_live_capture(args, mapping)

    progress = CaptureProgress(mapping, verbose=args.verbose)
    stop_event = threading.Event()
    previous_sigint = _install_sigint_stop(stop_event)
    listener = _start_q_listener(stop_event)
    if listener:
        print("Press q or Ctrl+C to stop.")
    else:
        print("Press Ctrl+C to stop.")
    output_raw = Path(args.output_raw) if args.output_raw else None
    try:
        try:
            document = capture_history(
                CaptureHistoryOptions(
                    json_out=Path(args.json),
                    csv_out=Path(args.csv) if args.csv else None,
                    locale=args.locale,
                    pid=args.pid,
                    iface=args.iface,
                    output_raw=output_raw,
                    on_records=progress.replace,
                    stop_event=stop_event,
                )
            )
        except CaptureEnvironmentError as exc:
            stop_event.set()
            progress.finish()
            print(str(exc))
            return 3
    finally:
        _restore_sigint_handler(previous_sigint)
    progress.replace(document.get("nte", {}).get("list", []))
    progress.finish()
    print(_summary_text(document, capture_counts=progress.summary_suffix()))
    _print_output_paths(args, private_raw=str(output_raw) if output_raw else None)
    return 0


def _run_auto_live_capture(args: argparse.Namespace, mapping: LocalizationMap) -> int:
    try:
        if _relaunch_auto_as_admin(args):
            return 0
    except CaptureEnvironmentError as exc:
        print(str(exc))
        return 3

    progress = CaptureProgress(mapping, verbose=args.verbose)
    stop_event = threading.Event()
    ready_event = threading.Event()
    done_event = threading.Event()
    target_box: dict[str, Any] = {}
    document_box: dict[str, ExportDocument] = {}
    error_box: list[BaseException] = []
    previous_sigint = _install_sigint_stop(stop_event)
    listener = _start_q_listener(stop_event)
    if listener:
        print("Press Esc to stop auto. Press q or Ctrl+C to stop capture.")
    else:
        print("Press Esc to stop auto. Press Ctrl+C to stop capture.")

    auto_progress = AutoPageProgress(mapping, progress)
    output_raw = Path(args.output_raw) if args.output_raw else None

    def on_ready(target: Any) -> None:
        target_box["target"] = target
        ready_event.set()

    def capture_worker() -> None:
        try:
            document_box["document"] = capture_history(
                CaptureHistoryOptions(
                    json_out=Path(args.json),
                    csv_out=Path(args.csv) if args.csv else None,
                    locale=args.locale,
                    pid=args.pid,
                    iface=args.iface,
                    output_raw=output_raw,
                    on_records=progress.replace,
                    on_ready=on_ready,
                    stop_event=stop_event,
                )
            )
        except BaseException as exc:
            error_box.append(exc)
        finally:
            done_event.set()
            ready_event.set()

    thread = threading.Thread(target=capture_worker, name="nte-live-capture", daemon=False)
    thread.start()
    auto_exit_code = 0
    handled_capture_error = False
    try:
        while not ready_event.wait(0.1):
            if stop_event.is_set():
                break

        if error_box:
            auto_exit_code = _capture_thread_error(error_box[0], stop_event, progress)
            handled_capture_error = True
        elif "target" not in target_box:
            stop_event.set()
            auto_exit_code = 3
            progress.finish()
            print("capture stopped before auto page could start")
        else:
            from nte_gacha_exporter.automation.pager import AutoPageOptions, run_auto_page

            result = run_auto_page(
                AutoPageOptions(
                    target=target_box["target"],
                    stop_event=stop_event,
                    non_interactive=not sys.stdin.isatty(),
                    on_status=auto_progress.update,
                    status_formatter=auto_progress.tooltip_text,
                )
            )
            auto_progress.finish()
            print(f"auto_page={result.status} message={result.message}")
            if result.succeeded:
                stop_event.set()
            else:
                if listener:
                    print("auto page stopped; capture remains live. Press q or Ctrl+C to stop.")
                else:
                    print("auto page stopped; capture remains live. Press Ctrl+C to stop.")
    finally:
        _restore_sigint_handler(previous_sigint)

    thread.join()
    if error_box:
        if handled_capture_error:
            return auto_exit_code
        return _capture_thread_error(error_box[0], stop_event, progress)
    document = document_box.get("document")
    if document is None:
        progress.finish()
        return auto_exit_code or 3
    progress.replace(document.get("nte", {}).get("list", []))
    progress.finish()
    print(_summary_text(document, capture_counts=progress.summary_suffix()))
    _print_output_paths(args, private_raw=str(output_raw) if output_raw else None)
    return auto_exit_code


def _capture_thread_error(exc: BaseException, stop_event: threading.Event, progress: CaptureProgress) -> int:
    stop_event.set()
    progress.finish()
    if isinstance(exc, CaptureEnvironmentError):
        print(str(exc))
        return 3
    print(f"capture failed: {exc}")
    return 2


def _relaunch_auto_as_admin(args: argparse.Namespace, *, purpose: str = "auto page") -> bool:
    from nte_gacha_exporter.automation import winapi

    if not winapi.is_windows():
        raise CaptureEnvironmentError(f"{purpose} requires Windows")
    if winapi.is_admin():
        return False
    argv = list(getattr(args, "_argv", sys.argv[1:]))
    relaunch_args = argv if is_frozen() else ["-m", "nte_gacha_exporter.cli.main", *argv]
    print(f"Requesting administrator permission for {purpose}.")
    winapi.relaunch_as_admin(relaunch_args)
    return True


def _run_capture(args: argparse.Namespace) -> int:
    _locale_name, mapping = load_locale_map(args.locale)
    return _run_live_capture(args, mapping)


def _run_debug(args: argparse.Namespace) -> int:
    if args.debug_command == "export":
        _locale_name, mapping = load_locale_map(args.locale)
        return _run_debug_export(args, mapping)
    if args.debug_command == "doctor":
        return _run_doctor()
    if args.debug_command == "interfaces":
        return _run_interfaces()
    if args.debug_command == "maps":
        return _run_maps(args)
    return 2


def _run_maps(args: argparse.Namespace) -> int:
    if args.maps_command == "list":
        result = run_maps_list()
    else:
        result = run_maps_build(
            assets_root=args.assets_root,
            locale=args.locale,
            out_dir=Path(args.out_dir) if args.out_dir else None,
        )
    if result.error:
        print(result.error)
        return result.exitCode
    for line in result.lines:
        print(line)
    return result.exitCode


def _run_doctor() -> int:
    result = run_doctor()
    for line in result.lines:
        print(line)
    return result.exitCode


def _run_interfaces() -> int:
    result = run_interfaces()
    if result.error:
        print(result.error)
        return result.exitCode
    for line in result.lines:
        print(line)
    return result.exitCode


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    try:
        args = parser.parse_args(argv)
    except SystemExit as exc:
        return exc.code if isinstance(exc.code, int) else 2
    args._argv = list(argv) if argv is not None else sys.argv[1:]
    if args.version:
        return _print_version()
    if not args.command:
        parser.print_help()
        return 2
    if args.command == "capture" or (args.command == "debug" and getattr(args, "debug_command", None) == "export"):
        apply_history_output_defaults(args)
    handlers: dict[str, Callable[[argparse.Namespace], int]] = {
        "capture": _run_capture,
        "debug": _run_debug,
    }
    if handler := handlers.get(args.command):
        return handler(args)
    parser.error(f"unknown command: {args.command}")
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
