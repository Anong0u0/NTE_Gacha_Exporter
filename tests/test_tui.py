from __future__ import annotations

import io
import json
import os
import sys
import threading
from pathlib import Path
from types import SimpleNamespace

from rich.console import Console

from nte_gacha_exporter.app.operations import OperationResult
from nte_gacha_exporter.app.output import DefaultHistoryPaths
from nte_gacha_exporter.automation.pager import AutoPageStatus
from nte_gacha_exporter.tui import main as tui_main
from nte_gacha_exporter.tui.i18n import validateI18nKeys
from nte_gacha_exporter.tui.main import (
    TuiAdminRelaunchRequested,
    TuiApp,
    read_key,
    read_text_input,
    scan_raw_files,
)
from nte_gacha_exporter.tui.rendering import (
    TUI_FRAME_WIDTH,
    TuiDisplayState,
    TuiDisplayStateWriter,
    TuiRenderer,
    readDisplayState,
)
from nte_gacha_exporter.tui.settings import TuiSettings, load_settings, save_settings, settings_from_dict


def test_tui_routes_mas_like_main_choices():
    app = TuiApp(console=Console(file=io.StringIO()), settings=TuiSettings())

    assert app.route_main_choice("1") == "capture"
    assert app.route_main_choice("2") == "advanced"
    assert app.route_main_choice("3") is None
    assert app.route_main_choice("L") == "language"
    assert app.route_main_choice("S") is None
    assert app.route_main_choice("Q") == "quit"


def test_tui_main_menu_uses_centered_single_key_panel():
    output = io.StringIO()
    line_prompts: list[str] = []
    key_prompts: list[str] = []
    app = TuiApp(
        console=Console(file=output, width=120, force_terminal=False),
        input_func=lambda prompt: line_prompts.append(prompt) or "unused",
        key_input_func=lambda prompt: key_prompts.append(prompt) or "q",
        settings=TuiSettings(),
    )

    assert app._main_menu() == "q"

    lines = output.getvalue().splitlines()
    title_line = next(line for line in lines if "NTE Gacha Exporter" in line)
    assert len(title_line.strip()) <= TUI_FRAME_WIDTH
    assert any("[1] Capture" in line and line.lstrip().startswith("│") for line in lines)
    assert "重播 raw JSONL" not in output.getvalue()
    assert "Record locale" not in output.getvalue()
    assert "Output directory" not in output.getvalue()
    assert "語言切換成中文" in output.getvalue()
    assert line_prompts == []
    assert key_prompts
    assert "[1,2,L,Q]" in key_prompts[0]


def test_tui_main_menu_language_action_names_target_language():
    output = io.StringIO()
    app = TuiApp(
        console=Console(file=output, width=120, force_terminal=False),
        key_input_func=lambda _prompt: "q",
        settings=TuiSettings(uiLanguage="en"),
    )

    assert app._main_menu() == "q"

    assert "語言切換成中文" in output.getvalue()


def test_tui_zh_main_menu_language_action_names_target_language():
    output = io.StringIO()
    app = TuiApp(
        console=Console(file=output, width=120, force_terminal=False),
        key_input_func=lambda _prompt: "q",
        settings=TuiSettings(uiLanguage="zh-Hant"),
    )

    assert app._main_menu() == "q"

    assert "Language switch to English" in output.getvalue()


def test_tui_capture_and_raw_export_pages_own_settings_and_ignore_invalid_keys():
    capture_keys = iter(["x", "b"])
    capture_output = io.StringIO()
    capture_app = TuiApp(
        console=Console(file=capture_output, width=120, force_terminal=False),
        key_input_func=lambda _prompt: next(capture_keys),
        settings=TuiSettings(uiLanguage="zh-Hant"),
    )

    capture_app._capture_menu()

    capture_text = capture_output.getvalue()
    capture_lines = capture_text.splitlines()
    assert capture_text.count("自動翻頁") >= 2
    assert any("[1] 紀錄語系" in line and line.lstrip().startswith("│") for line in capture_lines)
    assert any("[4] 自動翻頁" in line and line.lstrip().startswith("│") for line in capture_lines)
    assert any("[R] 重置設定" in line and line.lstrip().startswith("│") for line in capture_lines)
    assert "Excel CSV" not in capture_text
    assert "包含 HTTPS" not in capture_text
    assert len(next(line for line in capture_lines if "擷取" in line).strip()) <= TUI_FRAME_WIDTH

    replay_keys = iter(["x", "b"])
    replay_output = io.StringIO()
    replay_app = TuiApp(
        console=Console(file=replay_output, width=120, force_terminal=False),
        key_input_func=lambda _prompt: next(replay_keys),
        settings=TuiSettings(uiLanguage="zh-Hant"),
    )

    replay_app._raw_export_menu()

    replay_text = replay_output.getvalue()
    replay_lines = replay_text.splitlines()
    assert replay_text.count("raw JSONL") >= 2
    assert any("[3] 輸出 debug JSON" in line and line.lstrip().startswith("│") for line in replay_lines)
    assert any("[4] raw JSONL" in line and line.lstrip().startswith("│") for line in replay_lines)
    assert any("[R] 重置設定" in line and line.lstrip().startswith("│") for line in replay_lines)
    assert "Excel CSV" not in replay_text


def test_tui_advanced_menu_uses_centered_single_key_panel():
    output = io.StringIO()
    app = TuiApp(
        console=Console(file=output, width=120, force_terminal=False),
        key_input_func=lambda _prompt: "b",
        settings=TuiSettings(uiLanguage="zh-Hant"),
    )

    app._advanced_menu()

    lines = output.getvalue().splitlines()
    assert any("[1] 環境檢查" in line and line.lstrip().startswith("│") for line in lines)
    assert any("[5] 匯出 raw JSONL" in line and line.lstrip().startswith("│") for line in lines)
    assert len(next(line for line in lines if "NTE Gacha Exporter" in line).strip()) <= TUI_FRAME_WIDTH


def test_tui_raw_file_menu_uses_single_key_and_limits_choices(tmp_path):
    for index in range(11):
        path = tmp_path / f"raw-{index}.jsonl"
        path.write_text("", encoding="utf-8")
        os.utime(path, (index, index))
    visible_files = scan_raw_files(tmp_path)[:9]
    output = io.StringIO()
    keys = iter(["x", "9"])
    app = TuiApp(
        console=Console(file=output, width=120, force_terminal=False),
        key_input_func=lambda _prompt: next(keys),
        settings=TuiSettings(uiLanguage="zh-Hant", outputDir=str(tmp_path)),
    )

    assert app._choose_raw_path() == visible_files[8]

    text = output.getvalue()
    assert text.count("檔案:") == 2
    assert "只顯示最新 9 筆。" in text
    assert "[10]" not in text


def test_tui_raw_manual_path_prompt_names_field_and_esc_cancels(tmp_path):
    prompts: list[str] = []
    app = TuiApp(
        console=Console(file=io.StringIO(), width=120, force_terminal=False),
        text_input_func=lambda prompt: prompts.append(prompt) or None,
        settings=TuiSettings(uiLanguage="zh-Hant", outputDir=str(tmp_path)),
    )

    assert app._choose_raw_path() is None

    assert prompts
    assert "正在輸入手動輸入路徑" in prompts[0]
    assert "Esc" in prompts[0]


def test_tui_capture_auto_defaults_on_and_starts_capture():
    keys = iter(["s", "x", "b"])
    output = io.StringIO()
    seen: dict[str, object] = {}
    app = TuiApp(
        console=Console(file=output, width=120, force_terminal=False),
        key_input_func=lambda _prompt: next(keys),
        settings=TuiSettings(uiLanguage="zh-Hant"),
    )

    def fake_run_live_capture(*, auto_page: bool) -> OperationResult:
        seen["auto_page"] = auto_page
        return OperationResult(0, lines=("ok",))

    app.run_live_capture = fake_run_live_capture

    app._capture_menu()

    assert seen == {"auto_page": True}


def test_tui_capture_auto_toggle_persists():
    keys = iter(["4", "b"])
    app = TuiApp(
        console=Console(file=io.StringIO(), width=120, force_terminal=False),
        key_input_func=lambda _prompt: next(keys),
        settings=TuiSettings(),
    )
    app._save_settings = lambda: None

    app._capture_menu()

    assert app.settings.autoPage is False


def test_tui_raw_export_start_without_raw_only_refreshes():
    keys = iter(["s", "b"])
    output = io.StringIO()
    app = TuiApp(
        console=Console(file=output, width=120, force_terminal=False),
        key_input_func=lambda _prompt: next(keys),
        settings=TuiSettings(uiLanguage="zh-Hant"),
    )

    def fail_run_raw_replay(_raw_path=None) -> OperationResult:
        raise AssertionError("replay should not start without raw path")

    app.run_raw_replay = fail_run_raw_replay

    app._raw_export_menu()

    assert output.getvalue().count("未選擇") == 2


def test_tui_status_text_marks_enabled_and_disabled_with_color():
    app = TuiApp(console=Console(file=io.StringIO()), settings=TuiSettings())

    assert "green" in str(app._status_text(True).style)
    assert "red" in str(app._status_text(False).style)


def test_tui_text_input_reads_chars_and_esc_cancels(monkeypatch):
    class TtyStdin:
        def isatty(self) -> bool:
            return True

    output = io.StringIO()
    keys = iter(["a", "\x7f", "b", "\r", "\x1b"])
    monkeypatch.setattr(sys, "stdin", TtyStdin())
    monkeypatch.setattr(tui_main, "_read_terminal_key", lambda: next(keys))

    value = read_text_input(
        "Input> ",
        console=Console(file=output, force_terminal=False),
        fallback_input=lambda _prompt: "unused",
    )
    cancelled = read_text_input(
        "Cancel> ",
        console=Console(file=output, force_terminal=False),
        fallback_input=lambda _prompt: "unused",
    )

    assert value == "b"
    assert cancelled is None
    assert "Input> " in output.getvalue()
    assert "Cancel> " in output.getvalue()


def test_tui_setting_text_input_prompt_names_field_and_esc_cancels():
    keys = iter(["1", "b"])
    prompts: list[str] = []
    app = TuiApp(
        console=Console(file=io.StringIO(), width=120, force_terminal=False),
        key_input_func=lambda _prompt: next(keys),
        text_input_func=lambda prompt: prompts.append(prompt) or None,
        settings=TuiSettings(uiLanguage="zh-Hant", recordLocale="ja"),
    )
    app._save_settings = lambda: None

    app._capture_menu()

    assert app.settings.recordLocale == "ja"
    assert prompts
    assert "正在輸入紀錄語系" in prompts[0]
    assert "Esc" in prompts[0]


def test_tui_capture_page_reset_restores_visible_defaults():
    keys = iter(["4", "r", "b"])
    output = io.StringIO()
    app = TuiApp(
        console=Console(file=output, width=120, force_terminal=False),
        key_input_func=lambda _prompt: next(keys),
        settings=TuiSettings(
            uiLanguage="zh-Hant",
            recordLocale="ja",
            outputDir="custom",
            saveRaw=True,
            writeDebug=True,
        ),
    )
    app._save_settings = lambda: None

    app._capture_menu()

    defaults = TuiSettings(uiLanguage="zh-Hant")
    assert app.settings.recordLocale == defaults.recordLocale
    assert app.settings.outputDir == defaults.outputDir
    assert app.settings.saveRaw is defaults.saveRaw
    assert app.settings.autoPage is True
    assert app.settings.writeDebug is True
    assert "自動翻頁: 開" in output.getvalue()
    assert "自動翻頁: 關" in output.getvalue()


def test_tui_raw_export_page_reset_restores_visible_defaults_and_raw_file(tmp_path, monkeypatch):
    monkeypatch.chdir(tmp_path)
    raw_path = Path("raw.jsonl")
    raw_path.write_text("", encoding="utf-8")
    keys = iter(["4", "1", "r", "b"])
    output = io.StringIO()
    app = TuiApp(
        console=Console(file=output, width=120, force_terminal=False),
        key_input_func=lambda _prompt: next(keys),
        settings=TuiSettings(
            uiLanguage="zh-Hant",
            recordLocale="ja",
            outputDir=".",
            saveRaw=True,
            writeDebug=True,
        ),
    )
    app._save_settings = lambda: None

    app._raw_export_menu()

    defaults = TuiSettings(uiLanguage="zh-Hant")
    assert app.settings.recordLocale == defaults.recordLocale
    assert app.settings.outputDir == defaults.outputDir
    assert app.settings.writeDebug is defaults.writeDebug
    assert app.settings.rawFile == ""
    assert app.settings.saveRaw is True
    assert str(raw_path) in output.getvalue()
    assert "raw JSONL: 未選擇" in output.getvalue()


def test_tui_raw_export_page_restores_and_persists_raw_file(tmp_path, monkeypatch):
    monkeypatch.chdir(tmp_path)
    raw_path = Path("raw.jsonl")
    raw_path.write_text("", encoding="utf-8")
    keys = iter(["b"])
    output = io.StringIO()
    app = TuiApp(
        console=Console(file=output, width=120, force_terminal=False),
        key_input_func=lambda _prompt: next(keys),
        settings=TuiSettings(uiLanguage="zh-Hant", outputDir=".", rawFile=str(raw_path)),
    )

    app._raw_export_menu()

    assert f"raw JSONL: {raw_path}" in output.getvalue()


def test_tui_raw_export_page_persists_selected_raw_file(tmp_path, monkeypatch):
    monkeypatch.chdir(tmp_path)
    raw_path = Path("raw.jsonl")
    raw_path.write_text("", encoding="utf-8")
    keys = iter(["4", "1", "b"])
    app = TuiApp(
        console=Console(file=io.StringIO(), width=120, force_terminal=False),
        key_input_func=lambda _prompt: next(keys),
        settings=TuiSettings(uiLanguage="zh-Hant", outputDir="."),
    )
    app._save_settings = lambda: None

    app._raw_export_menu()

    assert app.settings.rawFile == str(raw_path)


def test_tui_maps_build_text_inputs_name_fields(monkeypatch):
    prompts: list[str] = []
    values = iter(["assets", "ja", "maps-out"])
    seen: dict[str, object] = {}
    app = TuiApp(
        console=Console(file=io.StringIO(), width=120, force_terminal=False),
        text_input_func=lambda prompt: prompts.append(prompt) or next(values),
        settings=TuiSettings(uiLanguage="zh-Hant"),
    )
    app._save_settings = lambda: None

    def fake_run_maps_build(*, assets_root, locale, out_dir):
        seen["assets_root"] = assets_root
        seen["locale"] = locale
        seen["out_dir"] = out_dir
        return OperationResult(0, lines=("ok",))

    monkeypatch.setattr(tui_main, "run_maps_build", fake_run_maps_build)

    assert app.run_maps_build() == OperationResult(0, lines=("ok",))

    assert seen == {"assets_root": "assets", "locale": "ja", "out_dir": Path("maps-out")}
    assert app.settings.mapsBuildAssetsRoot == "assets"
    assert app.settings.mapsBuildLocale == "ja"
    assert app.settings.mapsBuildOutDir == "maps-out"
    assert "正在輸入assets-root" in prompts[0]
    assert "正在輸入locale" in prompts[1]
    assert "正在輸入out-dir" in prompts[2]
    assert all("Esc" in prompt for prompt in prompts)


def test_tui_maps_build_esc_does_not_persist_partial(monkeypatch):
    prompts: list[str] = []
    values = iter(["assets", None])
    app = TuiApp(
        console=Console(file=io.StringIO(), width=120, force_terminal=False),
        text_input_func=lambda prompt: prompts.append(prompt) or next(values),
        settings=TuiSettings(mapsBuildAssetsRoot="old"),
    )
    app._save_settings = lambda: None

    assert app.run_maps_build() is None

    assert app.settings.mapsBuildAssetsRoot == "old"
    assert app.settings.mapsBuildLocale == ""


def test_tui_run_returns_home_after_capture_task():
    keys = iter(["1", "s", "x", "q"])
    output = io.StringIO()
    app = TuiApp(
        console=Console(file=output, width=120, force_terminal=False),
        key_input_func=lambda _prompt: next(keys),
        settings=TuiSettings(uiLanguage="zh-Hant"),
    )

    app.run_live_capture = lambda *, auto_page: OperationResult(0, lines=("ok",))

    assert app.run() == 0
    assert output.getvalue().count("NTE Gacha Exporter") >= 2


def test_tui_run_returns_home_after_advanced_task():
    keys = iter(["2", "1", "x", "q"])
    output = io.StringIO()
    app = TuiApp(
        console=Console(file=output, width=120, force_terminal=False),
        key_input_func=lambda _prompt: next(keys),
        settings=TuiSettings(),
    )

    app.run_doctor = lambda: OperationResult(0, lines=("ok",))

    assert app.run() == 0
    assert output.getvalue().count("NTE Gacha Exporter") >= 2


def test_tui_auto_confirm_ignores_invalid_key_before_back():
    keys = iter(["x", "b"])
    output = io.StringIO()
    app = TuiApp(
        console=Console(file=output, width=120, force_terminal=False),
        key_input_func=lambda _prompt: next(keys),
        settings=TuiSettings(uiLanguage="zh-Hant"),
    )

    assert app._confirm_auto_page() is False
    text = output.getvalue()
    assert text.count("開始前確認") == 2
    assert "1920x1080" in text
    assert "可能錯誤" in text


def test_tui_auto_back_cancels_without_not_started_result():
    keys = iter(["b"])
    output = io.StringIO()
    app = TuiApp(
        console=Console(file=output, width=120, force_terminal=False),
        key_input_func=lambda _prompt: next(keys),
        settings=TuiSettings(),
    )

    assert app.run_live_capture(auto_page=True) is None
    assert "未開始" not in output.getvalue()


def test_tui_auto_page_uses_i18n_formatter_for_status_and_tooltip(monkeypatch):
    seen_statuses: list[str] = []
    seen: dict[str, object] = {}
    app = TuiApp(console=Console(file=io.StringIO(), width=120, force_terminal=False), settings=TuiSettings())
    original_render = TuiRenderer.renderCaptureState

    def fake_capture_history(options):
        options.on_records([])
        options.on_ready(SimpleNamespace(pid="1234"))
        while not options.stop_event.is_set():
            options.stop_event.wait(0.01)
        return {"_debug": {"summary": {"record_count": 0, "warning_count": 0}}, "nte": {"list": []}}

    def fake_run_auto_page(options):
        from nte_gacha_exporter.automation.pager import AutoPageResult

        status = AutoPageStatus(
            elapsedSeconds=1.0,
            message="page next",
            kind="page",
            step="limitedBoardPages",
            pool="limited",
            currentPage=2,
            totalPages=2,
        )
        seen["tooltip"] = options.status_formatter(status)
        options.on_status(status)
        return AutoPageResult("completed", "done", ("limited",))

    def spy_render(self, state):
        seen_statuses.append(state.status)
        return original_render(self, state)

    monkeypatch.setattr("nte_gacha_exporter.tui.main.capture_history", fake_capture_history)
    monkeypatch.setattr("nte_gacha_exporter.automation.pager.run_auto_page", fake_run_auto_page)
    monkeypatch.setattr(TuiRenderer, "renderCaptureState", spy_render)

    result = app.run_live_capture(auto_page=True, confirmed_auto_page=True)

    assert result is not None
    assert result.exitCode == 0
    assert seen["tooltip"] == "Next Limited Board page=2/2"
    assert any("Next Limited Board page=2/2" in status for status in seen_statuses)
    assert "page next" not in seen_statuses


def test_tui_result_and_live_views_are_centered_and_narrow():
    document = {
        "_debug": {"summary": {"record_count": 1, "warning_count": 0}},
        "nte": {
            "list": [
                {
                    "time": "2026-06-11T12:00:00",
                    "record_type": "gacha",
                    "pool_name": "Limited",
                    "item_name": "Item",
                    "count": 1,
                    "roll_label": "10",
                }
            ]
        },
    }
    result_output = io.StringIO()
    app = TuiApp(console=Console(file=result_output, width=120, force_terminal=False), settings=TuiSettings())

    app._render_result(OperationResult(0, document=document, captureCounts="限定棋盤=1"))

    result_lines = [line for line in result_output.getvalue().splitlines() if line.strip()]
    assert all(len(line.strip()) <= TUI_FRAME_WIDTH for line in result_lines)
    assert "限定棋盤=1" in result_output.getvalue()

    history_json = Path("output/history.json")
    paths = DefaultHistoryPaths(
        timestamp="260611-120000",
        json=history_json,
        csv=history_json.with_suffix(".csv"),
        raw=Path("output/raw.jsonl"),
        debugJson=Path("output/debug.json"),
    )
    live_output = io.StringIO()
    live_console = Console(file=live_output, width=120, force_terminal=False)
    live_state = TuiDisplayState(
        state="running",
        titleKey="live",
        status="running",
        paths={"json": paths.json, "csv": paths.csv},
    )
    live_state.replaceRecords([], {})
    live_console.print(TuiRenderer(live_console, app.i18n).renderCaptureState(live_state))

    live_lines = [line for line in live_output.getvalue().splitlines() if line.strip()]
    assert all(len(line.strip()) <= TUI_FRAME_WIDTH for line in live_lines)


def test_tui_display_state_keeps_capture_snapshot_across_status_updates():
    mapping = {
        "pools": {
            "CardPool_Character": "限定棋盤",
            "CardPool_NewRole": "常駐棋盤",
        },
        "labels": {},
    }
    record = {
        "time": "2026-06-11T12:00:00",
        "record_type": "monopoly",
        "pool_id": "CardPool_Character",
        "pool_name": "限定棋盤",
        "item_name": "Item",
        "count": 1,
        "roll_label": "10",
    }
    state = TuiDisplayState(state="running", titleKey="auto", status="start")

    state.replaceRecords([record], mapping)
    state.status = "page next"
    restored = TuiDisplayState.fromPayload(state.toPayload())

    assert restored.status == "page next"
    assert restored.captureCounts.startswith("限定棋盤=1")
    assert restored.lastRecords == (record,)

    state.result = OperationResult(0)
    restored = TuiDisplayState.fromPayload(state.toPayload())

    assert restored.result is not None
    assert restored.result.captureCounts == restored.captureCounts
    assert restored.result.lastRecords == (record,)


def test_tui_i18n_keys_are_complete():
    validateI18nKeys()


def test_read_key_falls_back_to_line_input_when_stdin_is_not_tty(monkeypatch):
    class NonTtyStdin:
        def isatty(self) -> bool:
            return False

    prompts: list[str] = []
    output = io.StringIO()
    monkeypatch.setattr(sys, "stdin", NonTtyStdin())

    value = read_key(
        "Choice> ",
        console=Console(file=output, force_terminal=False),
        fallback_input=lambda prompt: prompts.append(prompt) or "x",
    )

    assert value == "x"
    assert prompts == [""]
    assert "Choice> " in output.getvalue()


def test_tui_auto_page_once_returns_zero_after_admin_relaunch(monkeypatch):
    def fake_run_live_capture(self, *, auto_page, confirmed_auto_page=False, handoff_path=None):
        assert auto_page is True
        assert confirmed_auto_page is True
        assert handoff_path is None
        raise TuiAdminRelaunchRequested

    monkeypatch.setattr(TuiApp, "run_live_capture", fake_run_live_capture)

    assert tui_main.main(["--auto-page-once"]) == 0


def test_tui_admin_handoff_parent_reads_running_updates(monkeypatch):
    output = io.StringIO()
    seen_states: list[str] = []
    seen_record_counts: list[int] = []
    seen_capture_counts: list[str] = []
    app = TuiApp(console=Console(file=output, width=120, force_terminal=False), settings=TuiSettings())
    original_render = TuiRenderer.renderCaptureState

    def spy_render(self, state):
        seen_states.append(state.state)
        seen_record_counts.append(len(state.lastRecords))
        seen_capture_counts.append(state.captureCounts)
        return original_render(self, state)

    def fake_request_admin_relaunch(arguments, purpose):
        assert purpose == "auto page"
        handoff_path = Path(arguments[-1])

        def worker():
            writer = TuiDisplayStateWriter(handoff_path)
            record = {
                "time": "2026-06-11T12:00:00",
                "record_type": "monopoly",
                "pool_name": "限定棋盤",
                "item_name": "Item",
                "count": 1,
                "roll_label": "10",
            }
            state = TuiDisplayState(
                state="running",
                titleKey="auto",
                status="page next",
                captureCounts="限定棋盤=1",
                lastRecords=(record,),
            )
            writer.write(state)
            threading.Event().wait(0.05)
            result = OperationResult(
                0,
                document={"_debug": {"summary": {"record_count": 1, "warning_count": 0}}, "nte": {"list": []}},
                captureCounts="限定棋盤=1",
                lastRecords=(record,),
            )
            state.state = "completed"
            state.status = "完成"
            state.attachResult(result)
            writer.write(state)

        threading.Thread(target=worker, daemon=True).start()
        return True

    monkeypatch.setattr(tui_main, "HANDOFF_POLL_SECONDS", 0.005)
    monkeypatch.setattr(TuiRenderer, "renderCaptureState", spy_render)
    app._request_admin_relaunch = fake_request_admin_relaunch

    result = app._run_admin_auto_page_handoff()

    assert result.captureCounts == "限定棋盤=1"
    assert result.lastRecords
    assert "running" in seen_states
    assert "completed" in seen_states
    assert max(seen_record_counts) == 1
    assert "限定棋盤=1" in seen_capture_counts


def test_tui_admin_handoff_stores_parent_absolute_output_paths(monkeypatch, tmp_path):
    handoff_path = tmp_path / "handoff.json"
    work_dir = tmp_path / "work"
    work_dir.mkdir()
    app = TuiApp(
        console=Console(file=io.StringIO(), width=120, force_terminal=False),
        settings=TuiSettings(outputDir="output", saveRaw=True),
    )
    seen: dict[str, object] = {}

    def fake_request_admin_relaunch(arguments, purpose):
        assert purpose == "auto page"
        assert Path(arguments[-1]) == handoff_path
        state = readDisplayState(handoff_path)
        assert state is not None
        seen["state"] = state
        return False

    monkeypatch.chdir(work_dir)
    monkeypatch.setattr(app, "_new_auto_page_handoff_path", lambda: handoff_path)
    app._request_admin_relaunch = fake_request_admin_relaunch

    assert app._run_admin_auto_page_handoff() is None

    state = seen["state"]
    assert isinstance(state, TuiDisplayState)
    assert state.paths["json"].is_absolute()
    assert state.paths["json"].parent == work_dir / "output"
    assert state.paths["csv"].parent == work_dir / "output"
    assert state.paths["private_raw"].parent == work_dir / "output"
    assert state.handoffContext["paths"]["json"] == str(state.paths["json"])
    assert state.handoffContext["settings"]["saveRaw"] is True


def test_tui_auto_page_once_uses_handoff_paths_not_child_cwd(monkeypatch, tmp_path):
    from nte_gacha_exporter.automation.pager import AutoPageResult

    parent_output = tmp_path / "parent-output"
    system32 = tmp_path / "Windows" / "System32"
    system32.mkdir(parents=True)
    handoff_path = tmp_path / "handoff.json"
    json_path = parent_output / "history.json"
    csv_path = parent_output / "history.csv"
    raw_path = parent_output / "raw.jsonl"
    state = TuiDisplayState(
        state="launching",
        titleKey="auto",
        handoffContext={
            "schemaVersion": 1,
            "timestamp": "260612-100000",
            "paths": {
                "json": str(json_path),
                "csv": str(csv_path),
                "private_raw": str(raw_path),
            },
            "settings": {
                "recordLocale": "zh-Hant",
                "saveRaw": True,
            },
        },
    )
    TuiDisplayStateWriter(handoff_path).write(state)
    captured: dict[str, object] = {}

    def fake_capture_history(options):
        captured["json_out"] = options.json_out
        captured["csv_out"] = options.csv_out
        captured["output_raw"] = options.output_raw
        captured["locale"] = options.locale
        options.on_records([])
        options.on_ready(SimpleNamespace(pid="1234"))
        return {"_debug": {"summary": {"record_count": 0, "warning_count": 0}}, "nte": {"list": []}}

    def fake_run_auto_page(options):
        return AutoPageResult("completed", "done", ("limited",))

    monkeypatch.chdir(system32)
    monkeypatch.setattr("nte_gacha_exporter.tui.main.capture_history", fake_capture_history)
    monkeypatch.setattr("nte_gacha_exporter.automation.pager.run_auto_page", fake_run_auto_page)

    app = TuiApp(console=Console(file=io.StringIO(), width=120, force_terminal=False), settings=TuiSettings())
    app._load_auto_page_handoff_context(handoff_path)
    result = app.run_live_capture(auto_page=True, confirmed_auto_page=True, handoff_path=handoff_path)

    assert result is not None
    assert result.exitCode == 0
    assert captured == {
        "json_out": json_path,
        "csv_out": csv_path,
        "output_raw": raw_path,
        "locale": "zh-Hant",
    }
    assert result.paths["json"] == json_path
    assert result.paths["csv"] == csv_path
    assert result.paths["private_raw"] == raw_path


def test_tui_settings_load_save_roundtrip(tmp_path):
    path = tmp_path / "tui-settings.json"
    settings = TuiSettings(
        uiLanguage="en",
        recordLocale="ja",
        outputDir="out",
        saveRaw=True,
        writeDebug=True,
        autoPage=False,
        rawFile="raw.jsonl",
        mapsBuildAssetsRoot="assets",
        mapsBuildLocale="ko",
        mapsBuildOutDir="maps",
    )

    save_settings(settings, path)

    assert load_settings(path) == settings
    data = json.loads(path.read_text(encoding="utf-8"))
    assert sorted(data) == [
        "autoPage",
        "mapsBuildAssetsRoot",
        "mapsBuildLocale",
        "mapsBuildOutDir",
        "outputDir",
        "rawFile",
        "recordLocale",
        "saveRaw",
        "uiLanguage",
        "writeDebug",
    ]


def test_tui_settings_ignores_removed_excel_and_https_keys(tmp_path):
    path = tmp_path / "tui-settings.json"
    path.write_text(
        json.dumps(
            {
                "uiLanguage": "zh-Hant",
                "recordLocale": "ja",
                "outputDir": "out",
                "excel": True,
                "includeHttps": True,
            }
        ),
        encoding="utf-8",
    )

    settings = load_settings(path)
    save_settings(settings, path)

    data = json.loads(path.read_text(encoding="utf-8"))
    assert data["uiLanguage"] == "zh-Hant"
    assert data["recordLocale"] == "ja"
    assert data["outputDir"] == "out"
    assert "excel" not in data
    assert "includeHttps" not in data


def test_tui_settings_defaults_use_english_and_ui_record_locale_default():
    assert TuiSettings().uiLanguage == "en"
    assert TuiSettings().recordLocale == "en"
    assert TuiSettings().autoPage is True
    assert settings_from_dict({"uiLanguage": "en"}).recordLocale == "en"
    assert settings_from_dict({"uiLanguage": "zh-Hant"}).recordLocale == "zh-Hant"


def test_tui_settings_bad_json_returns_defaults(tmp_path):
    path = tmp_path / "tui-settings.json"
    path.write_text("{", encoding="utf-8")

    assert load_settings(path) == TuiSettings()


def test_scan_raw_files_sorts_newest_first(tmp_path):
    older = tmp_path / "older.jsonl"
    newer = tmp_path / "newer.jsonl"
    ignored = tmp_path / "history.json"
    older.write_text("", encoding="utf-8")
    newer.write_text("", encoding="utf-8")
    ignored.write_text("", encoding="utf-8")
    os.utime(older, (10, 10))
    os.utime(newer, (20, 20))

    files = scan_raw_files(tmp_path)

    assert files[0] == newer
    assert files[1] == older
    assert ignored not in files
