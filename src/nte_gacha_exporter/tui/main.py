from __future__ import annotations

import argparse
import json
import sys
import tempfile
import threading
import time
import uuid
from collections.abc import Callable
from dataclasses import replace
from pathlib import Path
from typing import Any

from rich.console import Console
from rich.live import Live

from nte_gacha_exporter.app.auto_page_status import AutoPageStatusFormatter
from nte_gacha_exporter.app.operations import (
    OperationResult,
    run_debug_export,
    run_doctor,
    run_interfaces,
    run_maps_build,
    run_maps_list,
)
from nte_gacha_exporter.app.output import DefaultHistoryPaths, default_history_paths
from nte_gacha_exporter.app.summary import (
    offline_capture_counts,
)
from nte_gacha_exporter.app.terminal import install_sigint_stop, restore_sigint_handler, start_q_listener
from nte_gacha_exporter.capture.live import CaptureEnvironmentError, CaptureHistoryOptions, capture_history
from nte_gacha_exporter.core.models import GachaRecord
from nte_gacha_exporter.core.schema import ExportDocument, LocalizationMap
from nte_gacha_exporter.mapping.runtime import DEFAULT_LOCALE, available_locales, load_locale_map
from nte_gacha_exporter.tui.i18n import TuiI18n
from nte_gacha_exporter.tui.rendering import (
    TuiDisplayState,
    TuiDisplayStateWriter,
    TuiRenderer,
    readDisplayState,
)
from nte_gacha_exporter.tui.settings import (
    SettingsAdminRelaunch,
    TuiSettings,
    import_settings,
    load_settings,
    recordLocaleDefaultForUi,
    save_settings,
)

InputFunc = Callable[[str], str]
TextInputFunc = Callable[[str], str | None]
RAW_CHOICE_LIMIT = 9
HANDOFF_POLL_SECONDS = 0.2
ESCAPE_KEY = "\x1b"
BACKSPACE_KEYS = {"\b", "\x7f"}
AUTO_PAGE_HANDOFF_CONTEXT_VERSION = 1


class TuiAdminRelaunchRequested(RuntimeError):
    """Raised after a successful administrator relaunch request."""


def _absolute_path(path: Path) -> Path:
    return path if path.is_absolute() else path.resolve()


def _absolute_history_paths(paths: DefaultHistoryPaths) -> DefaultHistoryPaths:
    return DefaultHistoryPaths(
        timestamp=paths.timestamp,
        json=_absolute_path(paths.json),
        csv=_absolute_path(paths.csv),
        raw=_absolute_path(paths.raw),
        debugJson=_absolute_path(paths.debugJson),
    )


def _history_paths_payload(paths: DefaultHistoryPaths, *, save_raw: bool) -> dict[str, str]:
    payload = {
        "json": str(paths.json),
        "csv": str(paths.csv),
    }
    if save_raw:
        payload["private_raw"] = str(paths.raw)
    return payload


def _paths_from_payload(payload: dict[str, object], timestamp: str) -> DefaultHistoryPaths | None:
    try:
        paths = payload["paths"]
        if not isinstance(paths, dict):
            return None
        json_path = Path(str(paths["json"]))
        csv_path = Path(str(paths["csv"]))
    except (KeyError, TypeError, ValueError):
        return None
    raw_path = Path(str(paths.get("private_raw") or json_path.with_name(f"raw-{timestamp}.jsonl")))
    return DefaultHistoryPaths(
        timestamp=timestamp,
        json=json_path,
        csv=csv_path,
        raw=raw_path,
        debugJson=json_path.with_name(f"history-debug-{timestamp}.json"),
    )


def _read_terminal_key() -> str:
    if sys.platform == "win32":
        import msvcrt

        char = msvcrt.getwch()
        if char in ("\x00", "\xe0"):
            return char + msvcrt.getwch()
        return char

    import termios
    import tty

    fd = sys.stdin.fileno()
    previous = termios.tcgetattr(fd)
    try:
        tty.setraw(fd)
        return sys.stdin.read(1)
    finally:
        termios.tcsetattr(fd, termios.TCSADRAIN, previous)


def read_key(prompt: str, *, console: Console, fallback_input: InputFunc) -> str:
    console.print(prompt, end="")
    if not sys.stdin.isatty():
        return fallback_input("")

    try:
        char = _read_terminal_key()
    except (AttributeError, ImportError, OSError, ValueError):
        return fallback_input("")
    if char == "\x03":
        raise KeyboardInterrupt
    if char in ("\r", "\n"):
        console.print()
        return ""
    console.print(char if char.isprintable() else "")
    return char


def read_text_input(prompt: str, *, console: Console, fallback_input: InputFunc) -> str | None:
    console.print(prompt, end="")
    if not sys.stdin.isatty():
        value = fallback_input("")
        return None if value == ESCAPE_KEY else value

    chars: list[str] = []
    while True:
        try:
            char = _read_terminal_key()
        except (AttributeError, ImportError, OSError, ValueError):
            value = fallback_input("")
            return None if value == ESCAPE_KEY else value
        if char == "\x03":
            raise KeyboardInterrupt
        if char == ESCAPE_KEY:
            console.print()
            return None
        if char in ("\r", "\n"):
            console.print()
            return "".join(chars)
        if char in BACKSPACE_KEYS:
            if chars:
                chars.pop()
                console.file.write("\b \b")
                console.file.flush()
            continue
        if len(char) == 1 and char.isprintable():
            chars.append(char)
            console.print(char, end="")


class TuiApp:
    def __init__(
        self,
        *,
        console: Console | None = None,
        input_func: InputFunc = input,
        key_input_func: InputFunc | None = None,
        text_input_func: TextInputFunc | None = None,
        settings: TuiSettings | None = None,
    ) -> None:
        self.console = console or Console()
        self.input = input_func
        self.key_input = key_input_func or (
            lambda prompt: read_key(prompt, console=self.console, fallback_input=self.input)
        )
        self.text_input = text_input_func or (
            lambda prompt: read_text_input(prompt, console=self.console, fallback_input=self.input)
        )
        self.settings = settings or load_settings()
        self._handoff_history_paths: DefaultHistoryPaths | None = None

    @property
    def i18n(self) -> TuiI18n:
        return TuiI18n(self.settings.uiLanguage)

    @property
    def renderer(self) -> TuiRenderer:
        return TuiRenderer(self.console, self.i18n)

    def _t(self, key: str) -> str:
        return self.i18n.text(key)

    def _frame_width(self) -> int:
        return self.renderer.frame_width()

    def _content_divider(self) -> str:
        return self.renderer.content_divider()

    def _prompt(self, prompt: str) -> str:
        return self.renderer.prompt(prompt)

    def _read_text_input(self, fieldKey: str, current: object | None = None) -> str | None:
        current_text = "" if current is None else str(current)
        prompt = self.i18n.format("inputPrompt", field=self._t(fieldKey), current=current_text)
        return self.text_input(self._prompt(prompt))

    def _read_key(self, prompt: str) -> str:
        return self.key_input(self._prompt(prompt))

    def _read_choice(self, prompt: str, valid_keys: set[str], *, allow_enter: bool = False) -> str | None:
        choice = self._read_key(prompt).strip().lower()
        if allow_enter and choice == "":
            return ""
        if choice in valid_keys:
            return choice
        return None

    def _print_frame(self, renderable: object, *, title: str, subtitle: str | None = None) -> None:
        self.renderer.printFrameText(renderable, title=title, subtitle=subtitle)

    def _print_centered(self, renderable: object) -> None:
        self.renderer.printCentered(renderable)

    def _status_text(self, value: bool):
        return self.renderer.statusText(value)

    def _value_line(self, labelKey: str, value: object):
        return self.renderer.valueLine(labelKey, value)

    def _toggle_line(self, labelKey: str, value: bool):
        return self.renderer.toggleLine(labelKey, value)

    def _replace_settings(self, **changes: Any) -> None:
        updated = replace(self.settings, **changes)
        if updated == self.settings:
            return
        self.settings = updated
        self._save_settings()

    def _capture_settings_lines(self, *, auto_page: bool) -> list[object]:
        return [
            self._value_line("recordLocale", self.settings.recordLocale),
            self._value_line("outputDir", self.settings.outputDir),
            self._toggle_line("saveRaw", self.settings.saveRaw),
            self._toggle_line("autoPage", auto_page),
        ]

    def _replay_settings_lines(self, raw_path: Path | None) -> list[object]:
        raw_value = str(raw_path) if raw_path else self._t("notSelected")
        return [
            self._value_line("recordLocale", self.settings.recordLocale),
            self._value_line("outputDir", self.settings.outputDir),
            self._toggle_line("writeDebug", self.settings.writeDebug),
            self._value_line("rawFile", raw_value),
        ]

    def run(self) -> int:
        try:
            while True:
                choice = self._main_menu()
                action = self.route_main_choice(choice)
                if action == "quit":
                    return 0
                if action == "language":
                    self._toggle_language()
                elif action == "capture":
                    self._capture_menu()
                elif action == "advanced":
                    self._advanced_menu()
        except TuiAdminRelaunchRequested:
            return 0

    def route_main_choice(self, choice: str) -> str | None:
        normalized = choice.strip().lower()
        return {
            "1": "capture",
            "2": "advanced",
            "l": "language",
            "q": "quit",
        }.get(normalized)

    def _main_menu(self) -> str:
        self.console.clear()
        menu = self.renderer.renderMenu(
            headingKey="actions",
            items=[("1", "capture"), ("2", "advanced")],
            footerItems=[("L", "languageSwitch"), ("Q", "quit")],
        )
        self.renderer.printFrame(menu, titleKey="title", subtitleKey="subtitle")
        choice = self._read_choice(self._t("mainPrompt"), {"1", "2", "l", "q"})
        return choice or ""

    def _toggle_language(self) -> None:
        current_language = self.settings.uiLanguage
        next_language = "en" if current_language == "zh-Hant" else "zh-Hant"
        changes: dict[str, Any] = {"uiLanguage": next_language}
        if self.settings.recordLocale == recordLocaleDefaultForUi(current_language):
            changes["recordLocale"] = recordLocaleDefaultForUi(next_language)
        self._replace_settings(**changes)

    def _capture_menu(self) -> None:
        auto_page = self.settings.autoPage
        while True:
            self.console.clear()
            menu = self.renderer.renderMenu(
                settings=self._capture_settings_lines(auto_page=auto_page),
                items=[
                    ("1", "recordLocale"),
                    ("2", "outputDir"),
                    ("3", "saveRaw"),
                    ("4", "autoPage"),
                ],
                footerItems=[("S", "start"), ("R", "reset"), ("B", "back")],
            )
            self.renderer.printFrame(menu, titleKey="capture")
            choice = self._read_choice(
                self._t("capturePrompt"),
                {"1", "2", "3", "4", "s", "r", "b"},
            )
            if choice is None:
                continue
            if choice == "b":
                return
            if choice == "s":
                if self._run_and_pause(lambda auto_page=auto_page: self.run_live_capture(auto_page=auto_page)):
                    return
                continue
            if choice == "4":
                auto_page = not auto_page
                self._replace_settings(autoPage=auto_page)
                continue
            if choice == "r":
                self._reset_capture_settings()
                auto_page = self.settings.autoPage
                continue
            self._apply_capture_settings_choice(choice)

    def _apply_common_settings_choice(self, choice: str) -> bool:
        if choice == "1":
            self.renderer.printFrame(", ".join(available_locales()), titleKey="recordLocale")
            value = self._read_text_input("recordLocale", self.settings.recordLocale)
            if value is not None:
                value = value.strip()
                if value:
                    self._replace_settings(recordLocale=value)
            return True
        if choice == "2":
            value = self._read_text_input("outputDir", self.settings.outputDir)
            if value is not None:
                value = value.strip()
                if value:
                    self._replace_settings(outputDir=value)
            return True
        return False

    def _reset_capture_settings(self) -> None:
        defaults = TuiSettings(uiLanguage=self.settings.uiLanguage)
        self._replace_settings(
            recordLocale=defaults.recordLocale,
            outputDir=defaults.outputDir,
            saveRaw=defaults.saveRaw,
            autoPage=defaults.autoPage,
        )

    def _reset_replay_settings(self) -> None:
        defaults = TuiSettings(uiLanguage=self.settings.uiLanguage)
        self._replace_settings(
            recordLocale=defaults.recordLocale,
            outputDir=defaults.outputDir,
            writeDebug=defaults.writeDebug,
            rawFile=defaults.rawFile,
        )

    def _apply_capture_settings_choice(self, choice: str) -> None:
        if self._apply_common_settings_choice(choice):
            return
        if choice == "3":
            self._replace_settings(saveRaw=not self.settings.saveRaw)

    def _apply_replay_settings_choice(self, choice: str) -> None:
        if self._apply_common_settings_choice(choice):
            return
        if choice == "3":
            self._replace_settings(writeDebug=not self.settings.writeDebug)

    def _replay_menu(self) -> bool:
        return self._raw_export_menu()

    def _raw_export_menu(self) -> bool:
        raw_path = Path(self.settings.rawFile) if self.settings.rawFile else None
        while True:
            self.console.clear()
            menu = self.renderer.renderMenu(
                settings=self._replay_settings_lines(raw_path),
                items=[
                    ("1", "recordLocale"),
                    ("2", "outputDir"),
                    ("3", "writeDebug"),
                    ("4", "rawFile"),
                ],
                footerItems=[("S", "start"), ("R", "reset"), ("B", "back")],
            )
            self.renderer.printFrame(menu, titleKey="rawExport")
            choice = self._read_choice(self._t("rawExportPrompt"), {"1", "2", "3", "4", "s", "r", "b"})
            if choice is None:
                continue
            if choice == "b":
                return False
            if choice == "s":
                if raw_path is None:
                    continue
                if self._run_and_pause(lambda raw_path=raw_path: self.run_raw_replay(raw_path)):
                    return True
                continue
            if choice == "4":
                selected = self._choose_raw_path()
                if selected is not None:
                    raw_path = selected
                    self._replace_settings(rawFile=str(selected))
                continue
            if choice == "r":
                self._reset_replay_settings()
                raw_path = Path(self.settings.rawFile) if self.settings.rawFile else None
                continue
            self._apply_replay_settings_choice(choice)

    def _save_settings(self) -> None:
        try:
            save_settings(self.settings)
        except SettingsAdminRelaunch as exc:
            raise TuiAdminRelaunchRequested from exc
        except OSError as exc:
            self.console.print(f"{self._t('error')}: {exc}")

    def _advanced_menu(self) -> None:
        while True:
            self.console.clear()
            menu = self.renderer.renderMenu(
                headingKey="advanced",
                items=[
                    ("1", "doctor"),
                    ("2", "interfaces"),
                    ("3", "mapsList"),
                    ("4", "mapsBuild"),
                    ("5", "rawExport"),
                ],
                footerItems=[("B", "back")],
            )
            self.renderer.printFrame(menu, titleKey="title")
            choice = self._read_choice(self._t("advancedPrompt"), {"1", "2", "3", "4", "5", "b"})
            if choice is None:
                continue
            if choice == "b":
                return
            if choice == "5":
                if self._raw_export_menu():
                    return
                continue
            actions = {
                "1": self.run_doctor,
                "2": self.run_interfaces,
                "3": self.run_maps_list,
                "4": self.run_maps_build,
            }
            action = actions.get(choice)
            if action and self._run_and_pause(action):
                return

    def _run_and_pause(self, action: Callable[[], OperationResult | None]) -> bool:
        self.console.clear()
        result = action()
        if result is None:
            return False
        self._render_result(result)
        self._read_key(f"{self._t('pressAnyKey')} ")
        return True

    def _history_paths(self) -> DefaultHistoryPaths:
        if self._handoff_history_paths is not None:
            return self._handoff_history_paths
        return default_history_paths(output_dir=Path(self.settings.outputDir))

    def run_live_capture(
        self,
        *,
        auto_page: bool,
        confirmed_auto_page: bool = False,
        handoff_path: Path | None = None,
    ) -> OperationResult | None:
        if auto_page and not confirmed_auto_page:
            if not self._confirm_auto_page():
                return None
            self.console.clear()
            admin_result = self._run_admin_auto_page_handoff()
            if admin_result is not None:
                return admin_result
        self.console.clear()

        _locale_name, mapping = load_locale_map(self.settings.recordLocale)
        paths = self._history_paths()
        output_raw = paths.raw if self.settings.saveRaw else None
        paths_dict = {"json": paths.json, "csv": paths.csv}
        if output_raw:
            paths_dict["private_raw"] = output_raw
        handoff = TuiDisplayStateWriter(handoff_path) if handoff_path else None
        display_state = TuiDisplayState(
            state="running",
            titleKey="auto" if auto_page else "live",
            status=self._t("runningHint"),
            paths=paths_dict,
        )
        display_state.replaceRecords([], mapping)
        if handoff:
            handoff.write(display_state)
        stop_event = threading.Event()
        previous_sigint = install_sigint_stop(stop_event)
        start_q_listener(stop_event)
        document_box: dict[str, ExportDocument] = {}
        error_box: list[BaseException] = []

        try:
            if auto_page:
                self._run_auto_page_capture(
                    paths=paths,
                    mapping=mapping,
                    display_state=display_state,
                    stop_event=stop_event,
                    output_raw=output_raw,
                    document_box=document_box,
                    error_box=error_box,
                    handoff=handoff,
                )
            else:
                with Live(
                    self.renderer.renderCaptureState(display_state), console=self.console, refresh_per_second=4
                ) as live:

                    def on_records(records: list[GachaRecord]) -> None:
                        display_state.replaceRecords(records, mapping)
                        if handoff:
                            handoff.write(display_state)
                        live.update(self.renderer.renderCaptureState(display_state))

                    try:
                        document_box["document"] = capture_history(
                            CaptureHistoryOptions(
                                json_out=paths.json,
                                csv_out=paths.csv,
                                locale=self.settings.recordLocale,
                                output_raw=output_raw,
                                on_records=on_records,
                                stop_event=stop_event,
                            )
                        )
                    except BaseException as exc:
                        error_box.append(exc)
        finally:
            restore_sigint_handler(previous_sigint)

        if error_box:
            result = self._capture_error_result(error_box[0])
            if handoff:
                display_state.state = "error"
                display_state.status = result.error or ""
                display_state.attachResult(result)
                handoff.write(display_state)
            return result
        document = document_box.get("document")
        if not document:
            result = OperationResult(3, error="capture stopped before output was written")
            if handoff:
                display_state.state = "error"
                display_state.status = result.error or ""
                display_state.attachResult(result)
                handoff.write(display_state)
            return result
        display_state.replaceDocument(document, mapping)
        result = OperationResult(0, document=document, paths=paths_dict)
        display_state.attachResult(result)
        result = display_state.result or result
        if handoff:
            display_state.state = "completed"
            display_state.status = self._t("done")
            display_state.attachResult(result)
            handoff.write(display_state)
        return result

    def _run_auto_page_capture(
        self,
        *,
        paths: DefaultHistoryPaths,
        mapping: LocalizationMap,
        display_state: TuiDisplayState,
        stop_event: threading.Event,
        output_raw: Path | None,
        document_box: dict[str, ExportDocument],
        error_box: list[BaseException],
        handoff: TuiDisplayStateWriter | None = None,
    ) -> None:
        ready_event = threading.Event()
        target_box: dict[str, Any] = {}

        def on_ready(target: Any) -> None:
            target_box["target"] = target
            ready_event.set()

        def capture_worker() -> None:
            try:
                document_box["document"] = capture_history(
                    CaptureHistoryOptions(
                        json_out=paths.json,
                        csv_out=paths.csv,
                        locale=self.settings.recordLocale,
                        output_raw=output_raw,
                        on_records=on_records,
                        on_ready=on_ready,
                        stop_event=stop_event,
                    )
                )
            except BaseException as exc:
                error_box.append(exc)
            finally:
                ready_event.set()

        with Live(self.renderer.renderCaptureState(display_state), console=self.console, refresh_per_second=4) as live:

            def on_records(records: list[GachaRecord]) -> None:
                display_state.replaceRecords(records, mapping)
                if handoff:
                    handoff.write(display_state)
                live.update(self.renderer.renderCaptureState(display_state))

            thread = threading.Thread(target=capture_worker, name="nte-tui-live-capture", daemon=False)
            thread.start()
            while not ready_event.wait(0.1):
                if stop_event.is_set():
                    break
            if error_box or "target" not in target_box:
                stop_event.set()
                thread.join()
                return

            from nte_gacha_exporter.automation.pager import AutoPageOptions, run_auto_page

            formatter = AutoPageStatusFormatter(mapping)

            def on_status(status: Any) -> None:
                display_state.status = formatter.status_line(status, include_elapsed=True, include_detail=False)
                if handoff:
                    handoff.write(display_state)
                live.update(self.renderer.renderCaptureState(display_state))

            result = run_auto_page(
                AutoPageOptions(
                    target=target_box["target"],
                    stop_event=stop_event,
                    non_interactive=not sys.stdin.isatty(),
                    on_status=on_status,
                    status_formatter=formatter.tooltip_text,
                )
            )
            display_state.status = f"auto_page={result.status} message={result.message}"
            if handoff:
                handoff.write(display_state)
            live.update(self.renderer.renderCaptureState(display_state))
            if result.succeeded:
                stop_event.set()
            thread.join()

    def _confirm_auto_page(self) -> bool:
        while True:
            self.console.clear()
            self.renderer.printFrame(self._t("autoPrereq"), titleKey="auto")
            choice = self._read_choice(self._t("autoConfirmPrompt"), {"b"}, allow_enter=True)
            if choice is None:
                continue
            return choice == ""

    def run_raw_replay(self, raw_path: Path | None = None) -> OperationResult:
        raw_path = raw_path or self._choose_raw_path()
        if raw_path is None:
            return OperationResult(0, lines=(self._t("notStarted"),))
        paths = self._history_paths()
        debug_json = paths.debugJson if self.settings.writeDebug else None
        result = run_debug_export(
            raw_jsonl=raw_path,
            locale=self.settings.recordLocale,
            json_out=paths.json,
            csv_out=paths.csv,
            debug_json_out=debug_json,
        )
        return self._with_capture_counts(result)

    def _choose_raw_path(self) -> Path | None:
        candidates = scan_raw_files(Path(self.settings.outputDir))
        if candidates:
            visible_candidates = candidates[:RAW_CHOICE_LIMIT]
            valid_keys = {str(index) for index in range(1, len(visible_candidates) + 1)} | {"m", "b"}
            while True:
                self.console.clear()
                lines = [f"{self._t('files')}:", ""]
                if len(candidates) > RAW_CHOICE_LIMIT:
                    lines.extend([self._t("newestOnly"), ""])
                for index, path in enumerate(visible_candidates, 1):
                    lines.append(f"  [{index}] {path}")
                lines.extend(
                    [
                        "",
                        self._content_divider(),
                        "",
                        f"  [M] {self._t('manualPath')}",
                        f"  [B] {self._t('back')}",
                    ]
                )
                self.renderer.printFrame("\n".join(lines), titleKey="rawExport")
                choice = self._read_choice(self._t("rawFilePrompt"), valid_keys)
                if choice is None:
                    continue
                if choice == "b":
                    return None
                if choice == "m":
                    manual = self._read_text_input("manualPath", "")
                    if manual is None:
                        return None
                    manual = manual.strip()
                    return Path(manual) if manual else None
                return visible_candidates[int(choice) - 1]
        self.console.clear()
        self.renderer.printFrame(self._t("noRaw"), titleKey="rawExport")
        manual = self._read_text_input("manualPath", "")
        if manual is None:
            return None
        manual = manual.strip()
        return Path(manual) if manual else None

    def run_doctor(self) -> OperationResult:
        return run_doctor()

    def run_interfaces(self) -> OperationResult:
        return run_interfaces()

    def run_maps_list(self) -> OperationResult:
        return run_maps_list()

    def run_maps_build(self) -> OperationResult | None:
        self.console.clear()
        self.renderer.printFrame(self._t("mapsBuild"), titleKey="advanced")
        assets_root_text = self._read_text_input("assetsRoot", self.settings.mapsBuildAssetsRoot)
        if assets_root_text is None:
            return None
        locale_text = self._read_text_input("mapsLocale", self.settings.mapsBuildLocale or f"{DEFAULT_LOCALE}/all")
        if locale_text is None:
            return None
        out_dir_text = self._read_text_input("mapsOutDir", self.settings.mapsBuildOutDir or self._t("defaultValue"))
        if out_dir_text is None:
            return None
        assets_root_text = assets_root_text.strip() or self.settings.mapsBuildAssetsRoot
        locale_text = locale_text.strip() or self.settings.mapsBuildLocale
        out_dir_text = out_dir_text.strip() or self.settings.mapsBuildOutDir
        self._replace_settings(
            mapsBuildAssetsRoot=assets_root_text,
            mapsBuildLocale=locale_text,
            mapsBuildOutDir=out_dir_text,
        )
        assets_root = assets_root_text or None
        locale = locale_text or None
        out_dir = Path(out_dir_text) if out_dir_text else None
        result = run_maps_build(assets_root=assets_root, locale=locale, out_dir=out_dir)
        if (
            result.error
            and "Permission" in result.error
            and self._request_admin_maps_build(assets_root, locale, out_dir)
        ):
            raise TuiAdminRelaunchRequested
        return result

    def _render_result(self, result: OperationResult) -> None:
        self.console.clear()
        self.renderer.printResult(result)

    def _capture_error_result(self, exc: BaseException) -> OperationResult:
        if isinstance(exc, CaptureEnvironmentError):
            return OperationResult(3, error=str(exc))
        return OperationResult(2, error=f"capture failed: {exc}")

    def _with_capture_counts(self, result: OperationResult) -> OperationResult:
        if not result.document or result.captureCounts:
            return result
        _locale_name, mapping = load_locale_map(self.settings.recordLocale)
        return replace(result, captureCounts=offline_capture_counts(result.document, mapping))

    def _new_auto_page_handoff_path(self) -> Path:
        return Path(tempfile.gettempdir()) / f"nte-gacha-auto-page-{uuid.uuid4().hex}.json"

    def _load_auto_page_handoff_context(self, handoff_path: Path | None) -> None:
        if handoff_path is None:
            return
        state = readDisplayState(handoff_path)
        if state is None or not state.handoffContext:
            return
        context = state.handoffContext
        paths = _paths_from_payload(context, str(context.get("timestamp") or ""))
        if paths is not None:
            self._handoff_history_paths = paths
        settings = context.get("settings")
        if isinstance(settings, dict):
            changes: dict[str, Any] = {}
            if isinstance(settings.get("recordLocale"), str):
                changes["recordLocale"] = settings["recordLocale"]
            if isinstance(settings.get("saveRaw"), bool):
                changes["saveRaw"] = settings["saveRaw"]
            if changes:
                self.settings = replace(self.settings, **changes)

    def _run_admin_auto_page_handoff(self) -> OperationResult | None:
        handoff_path = self._new_auto_page_handoff_path()
        paths = _absolute_history_paths(self._history_paths())
        paths_payload = _history_paths_payload(paths, save_raw=self.settings.saveRaw)
        state = TuiDisplayState(
            state="launching",
            titleKey="auto",
            status=self._t("startedAdmin"),
            paths={key: Path(value) for key, value in paths_payload.items()},
            handoffContext={
                "schemaVersion": AUTO_PAGE_HANDOFF_CONTEXT_VERSION,
                "timestamp": paths.timestamp,
                "paths": paths_payload,
                "settings": {
                    "recordLocale": self.settings.recordLocale,
                    "saveRaw": self.settings.saveRaw,
                },
            },
        )
        writer = TuiDisplayStateWriter(handoff_path)
        writer.write(state)
        if not self._request_admin_relaunch(["--auto-page-once", "--handoff-json", str(handoff_path)], "auto page"):
            return None
        return self._wait_auto_page_handoff(handoff_path)

    def _wait_auto_page_handoff(self, handoff_path: Path) -> OperationResult:
        state = readDisplayState(handoff_path) or TuiDisplayState(
            state="launching",
            titleKey="auto",
            status=self._t("waitingAdmin"),
        )
        with Live(self.renderer.renderCaptureState(state), console=self.console, refresh_per_second=4) as live:
            while True:
                next_state = readDisplayState(handoff_path)
                if next_state:
                    state = next_state
                    live.update(self.renderer.renderCaptureState(state))
                    if state.result:
                        return state.result
                    if state.state == "error":
                        return OperationResult(
                            2, error=state.error or state.status or self._t("error"), lastRecords=state.lastRecords
                        )
                time.sleep(HANDOFF_POLL_SECONDS)

    def _request_admin_relaunch(self, arguments: list[str], purpose: str) -> bool:
        try:
            from nte_gacha_exporter.app.operations import is_frozen
            from nte_gacha_exporter.automation import winapi
        except Exception:
            return False
        if not winapi.is_windows():
            return False
        if winapi.is_admin():
            return False
        relaunch_args = arguments if is_frozen() else ["-m", "nte_gacha_exporter.tui.main", *arguments]
        self.renderer.printFrame(self.i18n.format("requestAdmin", purpose=purpose), titleKey="settings")
        winapi.relaunch_as_admin(relaunch_args)
        return True

    def _request_admin_maps_build(self, assets_root: str | None, locale: str | None, out_dir: Path | None) -> bool:
        payload = {"assetsRoot": assets_root, "locale": locale, "outDir": str(out_dir) if out_dir else None}
        with tempfile.NamedTemporaryFile(
            "w",
            encoding="utf-8",
            delete=False,
            prefix="nte-gacha-maps-build-",
            suffix=".json",
        ) as temp_file:
            json.dump(payload, temp_file, ensure_ascii=False)
            temp_path = Path(temp_file.name)
        return self._request_admin_relaunch(["--maps-build-json", str(temp_path)], "maps build")


def scan_raw_files(output_dir: Path) -> list[Path]:
    if not output_dir.exists():
        return []
    return sorted(output_dir.glob("*.jsonl"), key=lambda path: path.stat().st_mtime, reverse=True)


def _run_maps_build_json(path: Path) -> int:
    data = json.loads(path.read_text(encoding="utf-8"))
    result = run_maps_build(
        assets_root=data.get("assetsRoot"),
        locale=data.get("locale"),
        out_dir=Path(data["outDir"]) if data.get("outDir") else None,
    )
    console = Console()
    if result.error:
        console.print(result.error)
    for line in result.lines:
        console.print(line)
    return result.exitCode


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="nte-gacha")
    parser.add_argument("--import-settings")
    parser.add_argument("--auto-page-once", action="store_true")
    parser.add_argument("--handoff-json")
    parser.add_argument("--maps-build-json")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.import_settings:
        import_settings(Path(args.import_settings))
        return TuiApp().run()
    if args.maps_build_json:
        return _run_maps_build_json(Path(args.maps_build_json))
    app = TuiApp()
    try:
        if args.auto_page_once:
            handoff_path = Path(args.handoff_json) if args.handoff_json else None
            app._load_auto_page_handoff_context(handoff_path)
            result = app.run_live_capture(
                auto_page=True,
                confirmed_auto_page=True,
                handoff_path=handoff_path,
            )
            if result is None:
                return 0
            if not args.handoff_json:
                app._render_result(result)
            return result.exitCode
        return app.run()
    except TuiAdminRelaunchRequested:
        return 0


if __name__ == "__main__":
    raise SystemExit(main())
