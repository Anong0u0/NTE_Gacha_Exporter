from __future__ import annotations

import io
import json
import re
import signal
import sys
import threading
from pathlib import Path
from types import SimpleNamespace

from nte_gacha_exporter.app.auto_page_status import AutoPageStatusFormatter
from nte_gacha_exporter.app.summary import summary_text
from nte_gacha_exporter.automation.pager import AutoPageStatus
from nte_gacha_exporter.cli.main import AutoPageProgress, CaptureProgress, _relaunch_auto_as_admin, _wait_for_q, main
from nte_gacha_exporter.core.models import GachaRecord, SourceRef
from nte_gacha_exporter.mapping.runtime import load_map

FIXTURE = Path(__file__).parent / "fixtures" / "sample.raw.jsonl"


def test_maps_list(capsys):
    assert main(["debug", "maps", "list"]) == 0
    assert "zh-Hant" in capsys.readouterr().out


def test_summary_text_hides_zero_warnings_and_keeps_nonzero_warnings():
    assert summary_text({"_debug": {"summary": {"record_count": 2, "warning_count": 0}}}) == "records=2"
    assert summary_text({"_debug": {"summary": {"record_count": 2, "warning_count": 3}}}) == "records=2 warnings=3"


def test_debug_export_writes_public_outputs(tmp_path, capsys):
    json_out = tmp_path / "history.json"
    csv_out = tmp_path / "history.csv"
    debug_out = tmp_path / "history.debug.json"

    assert (
        main(
            [
                "debug",
                "export",
                str(FIXTURE),
                "--json",
                str(json_out),
                "--csv",
                str(csv_out),
                "--debug-json",
                str(debug_out),
            ]
        )
        == 0
    )

    out = capsys.readouterr().out
    assert "records=2" in out
    assert "Limited Board=1" in out
    assert "Arc Research=1" in out
    assert f"debug_json={debug_out}" in out
    assert json_out.exists()
    assert csv_out.exists()
    assert debug_out.exists()
    assert sorted(json.loads(json_out.read_text(encoding="utf-8"))) == ["info", "nte"]
    assert "summary" in json.loads(debug_out.read_text(encoding="utf-8"))


def test_auto_page_status_formatter_restores_i18n_page_tooltip():
    mapping = {
        "pools": {
            "CardPool_Character": "限定棋盤",
            "CardPool_NewRole": "標準棋盤",
        },
        "pool_meta": {
            "ForkLottery_AnHunQu": {"group_label": "弧盤研募"},
        },
        "labels": {
            "W_HTButton_Next_Page": "下一頁",
        },
    }
    formatter = AutoPageStatusFormatter(mapping)
    status = AutoPageStatus(
        elapsedSeconds=1.0,
        message="page next",
        kind="page",
        step="limitedBoardPages",
        pool="limited",
        currentPage=2,
        totalPages=2,
    )

    assert formatter.tooltip_text(status) == "下一頁 限定棋盤 page=2/2"


def test_debug_export_accepts_locale_map_spec(tmp_path, capsys):
    map_data = load_map("zh-Hant")
    map_data["pools"]["CardPool_Character"] = "自訂限定"
    map_data["pool_meta"]["CardPool_Character"] = {"group_label": "自訂限定", "title": "自訂池"}
    map_data["items"]["Fashion_vehicle_1010_V008"] = "自訂道具"
    map_path = tmp_path / "custom.json"
    map_path.write_text(json.dumps(map_data, ensure_ascii=False), encoding="utf-8")
    json_out = tmp_path / "history.json"
    csv_out = tmp_path / "history.csv"

    assert (
        main(
            [
                "debug",
                "export",
                str(FIXTURE),
                "--locale",
                f"custom={map_path}",
                "--json",
                str(json_out),
                "--csv",
                str(csv_out),
            ]
        )
        == 0
    )

    out = capsys.readouterr().out
    data = json.loads(json_out.read_text(encoding="utf-8"))
    assert "自訂限定=1" in out
    assert data["info"]["locale"] == "custom"
    assert data["nte"]["list"][0]["pool_name"] == "自訂池"
    assert data["nte"]["list"][0]["item_name"] == "自訂道具"


def test_debug_export_debug_json_flag_without_path_uses_default_name(tmp_path, monkeypatch, capsys):
    json_out = tmp_path / "history.json"
    csv_out = tmp_path / "history.csv"
    fixture = FIXTURE.resolve()
    monkeypatch.chdir(tmp_path)

    assert main(["debug", "export", str(fixture), "--json", str(json_out), "--csv", str(csv_out), "--debug-json"]) == 0

    out = capsys.readouterr().out
    match = re.search(r"debug_json=(output/history-debug-\d{6}-\d{6}\.json)", out)
    assert match
    debug_out = tmp_path / match.group(1)
    assert debug_out.exists()
    assert "summary" in json.loads(debug_out.read_text(encoding="utf-8"))


def test_export_command_is_removed():
    assert main(["export", str(FIXTURE)]) == 2


def test_capture_input_raw_is_removed():
    assert main(["capture", "--input-raw", str(FIXTURE)]) == 2


def test_excel_and_include_https_flags_are_removed():
    assert main(["capture", "--excel"]) == 2
    assert main(["capture", "--include-https"]) == 2
    assert main(["debug", "export", str(FIXTURE), "--excel"]) == 2


def test_watch_command_is_removed():
    assert main(["watch", str(FIXTURE)]) == 2


def test_old_top_level_debug_tools_are_removed():
    assert main(["maps", "list"]) == 2
    assert main(["doctor"]) == 2
    assert main(["interfaces"]) == 2


def test_top_level_help_has_clean_command_surface(capsys):
    assert main(["-h"]) == 0

    out = capsys.readouterr().out
    assert "{capture,debug}" in out
    assert "maps" not in out
    assert "doctor" not in out
    assert "interfaces" not in out


def test_capture_help_is_live_only(capsys):
    assert main(["capture", "-h"]) == 0

    out = capsys.readouterr().out
    assert "--input-raw" not in out
    assert "--debug-json" not in out
    assert "--output-raw" in out
    assert "--auto-page" in out
    assert "--excel" not in out
    assert "--include-https" not in out
    assert "--auto-profile" not in out
    assert "--pid" in out
    assert "--iface" in out
    assert "--seconds" not in out
    assert "--save-raw" not in out
    assert "--session" not in out
    assert "--name-map" not in out


def test_debug_help_has_tool_commands(capsys):
    assert main(["debug", "-h"]) == 0

    out = capsys.readouterr().out
    assert "auto-profile" not in out


def test_debug_export_help_has_raw_input_and_debug_json(capsys):
    assert main(["debug", "export", "-h"]) == 0

    out = capsys.readouterr().out
    assert "raw_jsonl" in out
    assert "--debug-json" in out
    assert "--output-raw" not in out
    assert "--excel" not in out


def test_maps_build_help_has_no_no_prefix(capsys):
    assert main(["debug", "maps", "build", "-h"]) == 0

    out = capsys.readouterr().out
    assert "--no-prefix" not in out


def test_live_capture_auto_page_requests_admin_before_capture(monkeypatch, tmp_path, capsys):
    seen: dict[str, object] = {}

    def fake_relaunch(args):
        seen["auto_page"] = args.auto_page
        seen["json"] = args.json
        return True

    def fail_capture(_options):
        raise AssertionError("capture should not start before administrator relaunch")

    monkeypatch.setattr("nte_gacha_exporter.cli.main._relaunch_auto_as_admin", fake_relaunch)
    monkeypatch.setattr("nte_gacha_exporter.cli.main.capture_history", fail_capture)

    assert main(["capture", "--auto-page", "--json", str(tmp_path / "history.json")]) == 0

    assert capsys.readouterr().out == ""
    assert seen == {"auto_page": True, "json": str(tmp_path / "history.json")}


def test_packaged_auto_page_admin_relaunch_reuses_cli_arguments(monkeypatch, capsys):
    seen: dict[str, object] = {}

    def fake_relaunch(arguments):
        seen["arguments"] = list(arguments)
        return 42

    monkeypatch.setattr("nte_gacha_exporter.automation.winapi.is_windows", lambda: True)
    monkeypatch.setattr("nte_gacha_exporter.automation.winapi.is_admin", lambda: False)
    monkeypatch.setattr("nte_gacha_exporter.automation.winapi.relaunch_as_admin", fake_relaunch)
    monkeypatch.setattr("nte_gacha_exporter.cli.main.is_frozen", lambda: True)

    result = _relaunch_auto_as_admin(SimpleNamespace(_argv=["capture", "--auto-page"]))

    assert result is True
    assert seen == {"arguments": ["capture", "--auto-page"]}
    assert "Requesting administrator permission for auto page." in capsys.readouterr().out


def test_source_auto_page_admin_relaunch_runs_cli_module(monkeypatch):
    seen: dict[str, object] = {}

    def fake_relaunch(arguments):
        seen["arguments"] = list(arguments)
        return 42

    monkeypatch.setattr("nte_gacha_exporter.automation.winapi.is_windows", lambda: True)
    monkeypatch.setattr("nte_gacha_exporter.automation.winapi.is_admin", lambda: False)
    monkeypatch.setattr("nte_gacha_exporter.automation.winapi.relaunch_as_admin", fake_relaunch)
    monkeypatch.setattr("nte_gacha_exporter.cli.main.is_frozen", lambda: False)

    result = _relaunch_auto_as_admin(SimpleNamespace(_argv=["capture", "--auto-page"]))

    assert result is True
    assert seen == {"arguments": ["-m", "nte_gacha_exporter.cli.main", "capture", "--auto-page"]}


def test_live_capture_does_not_write_raw_by_default(monkeypatch, tmp_path, capsys):
    captured: dict[str, object] = {}

    def fake_capture_history(options):
        captured.update(vars(options))
        options.on_records([])
        return {"_debug": {"summary": {"record_count": 0, "warning_count": 0}}}

    monkeypatch.setattr("nte_gacha_exporter.cli.main.capture_history", fake_capture_history)

    assert main(["capture", "--json", str(tmp_path / "history.json"), "--csv", str(tmp_path / "history.csv")]) == 0

    out = capsys.readouterr().out
    assert "Press Ctrl+C to stop." in out
    assert "private_raw=" not in out
    assert "seconds" not in captured
    assert "name_map" not in captured
    assert captured["output_raw"] is None
    assert isinstance(captured["stop_event"], threading.Event)


def test_windows_q_listener_treats_ctrl_c_as_stop(monkeypatch):
    class FakeMsvcrt:
        @staticmethod
        def kbhit():
            return True

        @staticmethod
        def getwch():
            raise KeyboardInterrupt

    monkeypatch.setattr("nte_gacha_exporter.cli.main.platform.system", lambda: "Windows")
    monkeypatch.setitem(sys.modules, "msvcrt", FakeMsvcrt)
    stop_event = threading.Event()

    _wait_for_q(stop_event, sys.stdin)

    assert stop_event.is_set()


def test_live_capture_sigint_requests_stop_and_restores_handler(monkeypatch, tmp_path):
    previous_handler = signal.getsignal(signal.SIGINT)

    def fake_capture_history(options):
        signal.raise_signal(signal.SIGINT)
        assert options.stop_event.is_set()
        options.on_records([])
        return {"_debug": {"summary": {"record_count": 0, "warning_count": 0}}}

    monkeypatch.setattr("nte_gacha_exporter.cli.main.capture_history", fake_capture_history)

    assert main(["capture", "--json", str(tmp_path / "history.json"), "--csv", str(tmp_path / "history.csv")]) == 0
    assert signal.getsignal(signal.SIGINT) == previous_handler


def test_live_capture_output_raw_is_opt_in(monkeypatch, tmp_path, capsys):
    captured: dict[str, object] = {}
    raw_out = tmp_path / "raw.jsonl"

    def fake_capture_history(options):
        captured.update(vars(options))
        options.on_records([])
        return {"_debug": {"summary": {"record_count": 0, "warning_count": 0}}}

    monkeypatch.setattr("nte_gacha_exporter.cli.main.capture_history", fake_capture_history)

    assert (
        main(
            [
                "capture",
                "--json",
                str(tmp_path / "history.json"),
                "--csv",
                str(tmp_path / "history.csv"),
                "--output-raw",
                str(raw_out),
            ]
        )
        == 0
    )

    out = capsys.readouterr().out
    assert f"private_raw={raw_out}" in out
    assert captured["output_raw"] == raw_out


def test_live_capture_output_raw_without_path_uses_default(monkeypatch, tmp_path, capsys):
    captured: dict[str, object] = {}
    monkeypatch.chdir(tmp_path)

    def fake_capture_history(options):
        captured.update(vars(options))
        options.on_records([])
        return {"_debug": {"summary": {"record_count": 0, "warning_count": 0}}}

    monkeypatch.setattr("nte_gacha_exporter.cli.main.capture_history", fake_capture_history)

    assert (
        main(
            [
                "capture",
                "--json",
                str(tmp_path / "history.json"),
                "--csv",
                str(tmp_path / "history.csv"),
                "--output-raw",
            ]
        )
        == 0
    )

    out = capsys.readouterr().out
    match = re.search(r"private_raw=(output/raw-\d{6}-\d{6}\.jsonl)", out)
    assert match
    assert captured["output_raw"] == Path(match.group(1))


def test_live_capture_auto_page_stops_after_success(monkeypatch, tmp_path, capsys):
    captured: dict[str, object] = {}
    auto_seen: dict[str, object] = {}

    def fake_capture_history(options):
        captured.update(vars(options))
        options.on_records([])
        options.on_ready(SimpleNamespace(pid="1234"))
        while not options.stop_event.is_set():
            options.stop_event.wait(0.01)
        return {"_debug": {"summary": {"record_count": 0, "warning_count": 0}}, "nte": {"list": []}}

    def fake_run_auto_page(options):
        from nte_gacha_exporter.automation.pager import AutoPageResult

        auto_seen["target"] = options.target
        status = AutoPageStatus(
            elapsedSeconds=1.0,
            message="page next",
            kind="page",
            step="limitedBoardPages",
            pool="limited",
            currentPage=2,
            totalPages=2,
        )
        auto_seen["tooltip"] = options.status_formatter(status)
        options.on_status(status)
        return AutoPageResult("completed", "done", ("limited", "standard", "fork"))

    monkeypatch.setattr("nte_gacha_exporter.cli.main._relaunch_auto_as_admin", lambda _args: False)
    monkeypatch.setattr("nte_gacha_exporter.cli.main.capture_history", fake_capture_history)
    monkeypatch.setattr("nte_gacha_exporter.automation.pager.run_auto_page", fake_run_auto_page)

    assert (
        main(
            [
                "capture",
                "--auto-page",
                "--json",
                str(tmp_path / "history.json"),
                "--csv",
                str(tmp_path / "history.csv"),
            ]
        )
        == 0
    )

    out = capsys.readouterr().out
    assert "\r+1.00s Next Limited Board page=2/2 | Limited Board=0 Standard Board=0 Arc Research=0" in out
    assert "auto_page: +1.00s" not in out
    assert "auto_page=completed" in out
    assert captured["stop_event"].is_set()
    assert auto_seen["target"].pid == "1234"
    assert auto_seen["tooltip"] == "Next Limited Board page=2/2"


def test_live_capture_auto_page_failure_keeps_capture_alive(monkeypatch, tmp_path, capsys):
    captured: dict[str, object] = {}

    def fake_capture_history(options):
        captured["stop_event"] = options.stop_event
        options.on_records([])
        options.on_ready(SimpleNamespace(pid="1234"))
        return {"_debug": {"summary": {"record_count": 0, "warning_count": 0}}, "nte": {"list": []}}

    def fake_run_auto_page(options):
        from nte_gacha_exporter.automation.pager import AutoPageResult

        return AutoPageResult("failed", "template missing")

    monkeypatch.setattr("nte_gacha_exporter.cli.main._relaunch_auto_as_admin", lambda _args: False)
    monkeypatch.setattr("nte_gacha_exporter.cli.main.capture_history", fake_capture_history)
    monkeypatch.setattr("nte_gacha_exporter.automation.pager.run_auto_page", fake_run_auto_page)

    assert (
        main(
            [
                "capture",
                "--auto-page",
                "--json",
                str(tmp_path / "history.json"),
                "--csv",
                str(tmp_path / "history.csv"),
            ]
        )
        == 0
    )

    out = capsys.readouterr().out
    assert "auto_page=failed message=template missing" in out
    assert "capture remains live" in out
    assert not captured["stop_event"].is_set()


def test_auto_page_progress_merges_status_and_counts_with_cr():
    stream = io.StringIO()
    mapping = {
        "pools": {
            "CardPool_Character": "限定棋盤",
            "CardPool_NewRole": "標準棋盤",
        },
        "pool_meta": {
            "ForkLottery_AnHunQu": {"group_label": "弧盤研募"},
        },
        "labels": {
            "W_HTButton_Next_Page": "下一頁",
        },
    }
    progress = CaptureProgress(mapping, verbose=False, stream=stream)
    progress.replace([{"pool_id": "CardPool_Character"}])
    auto_progress = AutoPageProgress(mapping, progress, stream=stream, use_cr=True)

    auto_progress.update(
        AutoPageStatus(
            elapsedSeconds=1.23,
            message="page next",
            kind="page",
            step="limitedBoardPages",
            pool="limited",
            currentPage=2,
            totalPages=3,
        )
    )

    out = stream.getvalue()
    assert "\n" not in out
    assert "\r+1.23s 下一頁 限定棋盤 page=2/3 | 限定棋盤=1 標準棋盤=0 弧盤研募=0" in out


def test_auto_page_progress_preserves_action_intent_with_i18n_terms():
    mapping = {
        "pools": {
            "CardPool_Character": "限定棋盤",
            "CardPool_NewRole": "標準棋盤",
        },
        "pool_meta": {
            "ForkLottery_AnHunQu": {"group_label": "弧盤研募"},
        },
        "labels": {
            "Abyss_GamepadKeys_1": "切換",
            "AbyssClone_Award_02": "已完成",
            "BPUI_LotteryDiceRecord_qipanleixing": "棋盤類型",
            "BPUI_LotteryDiceRecord_xiandingqipan": "限定棋盤",
            "BPUI_LotteryModuleEntrance_Title": "斯卡布羅集市",
            "Mall_8_name": "弧盤商店",
            "TreasureBox_2": "開啟",
            "UI_CloneSystemChallengeFailed_Retry": "重試",
            "UI_CloneSystemStaminaTips_Enter": "進入",
            "UI_Lottery_GachaDetails_Zhitoujilu": "擲骰記錄",
            "UI_Lottery_GachaDetails_title": "棋盤詳情",
            "UW_LotteryBase_BP_Hupanyanmu": "弧盤研募",
            "W_HTButton_Next_Page": "下一頁",
            "W_Vehicle_Button_Choose": "選擇",
            "common_3": "返回",
            "ui_forkshop_07": "研募詳情",
            "ui_forkshop_10": "研募記錄",
        },
    }
    progress = CaptureProgress(mapping, verbose=False, stream=io.StringIO())
    auto_progress = AutoPageProgress(mapping, progress, stream=io.StringIO(), use_cr=False)

    def line(kind: str, *, message: str = "", step: str | None = None, pool: str | None = None) -> str:
        return auto_progress.status_line(
            AutoPageStatus(elapsedSeconds=0, message=message, kind=kind, step=step, pool=pool),
            include_elapsed=False,
            include_detail=False,
        )

    assert line("step", step="boardDetails") == "進入 棋盤詳情"
    assert line("step", step="diceRecords") == "切換 擲骰記錄"
    assert line("step", step="boardType") == "開啟 棋盤類型"
    assert line("step", step="limitedBoard") == "選擇 限定棋盤"
    assert line("step", step="marketHome") == "返回 斯卡布羅集市"
    assert line("step", step="arcShop") == "進入 弧盤商店"
    assert line("step", step="arcResearchDetails") == "進入 研募詳情"
    assert line("step", step="arcResearchRecords") == "進入 研募記錄"
    assert line("template", step="verifyDiceRecords") == "Validate 擲骰記錄 verified"
    assert line("retry") == "重試: page did not change"
    assert (
        auto_progress.status_line(
            AutoPageStatus(
                elapsedSeconds=0,
                message="page next",
                kind="page",
                step="limitedBoardPages",
                pool="limited",
                currentPage=2,
                totalPages=3,
            ),
            include_elapsed=False,
            include_detail=False,
        )
        == "下一頁 限定棋盤 page=2/3"
    )
    assert (
        auto_progress.status_line(
            AutoPageStatus(
                elapsedSeconds=0,
                message="pool completed",
                kind="pool_completed",
                step="limitedBoardPages",
                pool="limited",
                currentPage=3,
                totalPages=3,
            ),
            include_elapsed=False,
            include_detail=False,
        )
        == "限定棋盤 已完成 page=3/3"
    )


def test_auto_page_progress_uses_i18n_label_and_detail_for_log_stream():
    stream = io.StringIO()
    mapping = {
        "pools": {
            "CardPool_Character": "Limited Board",
            "CardPool_NewRole": "Standard Board",
        },
        "pool_meta": {},
        "labels": {
            "BPUI_LotteryModuleEntrance_Title": "Scarborough Fair",
            "UW_LotteryBase_BP_Hupanyanmu": "Arc Research",
        },
    }
    progress = CaptureProgress(mapping, verbose=False, stream=stream)
    auto_progress = AutoPageProgress(mapping, progress, stream=stream, use_cr=False)
    status = AutoPageStatus(
        elapsedSeconds=0.5,
        message="template verified",
        kind="template",
        step="verifyMarketHome",
        technicalDetail="homeBoardFileIcon score=0.00",
    )

    auto_progress.update(status)

    assert auto_progress.tooltip_text(status) == "Validate Scarborough Fair verified"
    assert stream.getvalue() == (
        "auto_page: +0.50s Validate Scarborough Fair verified: homeBoardFileIcon score=0.00 "
        "| Limited Board=0 Standard Board=0 Arc Research=0\n"
    )


def test_live_capture_final_counts_use_document_records(monkeypatch, tmp_path, capsys):
    def fake_capture_history(options):
        options.on_records([{"pool_id": "CardPool_Character"} for _ in range(5)])
        return {
            "_debug": {"summary": {"record_count": 105, "warning_count": 0}},
            "nte": {"list": [{"pool_id": "CardPool_Character"} for _ in range(105)]},
        }

    monkeypatch.setattr("nte_gacha_exporter.cli.main.capture_history", fake_capture_history)

    assert main(["capture", "--json", str(tmp_path / "history.json"), "--csv", str(tmp_path / "history.csv")]) == 0

    out = capsys.readouterr().out
    assert "records=105 Limited Board=105 Standard Board=0 Arc Research=0" in out
    assert "warnings=0" not in out


def test_capture_progress_counts_and_verbose_rows(capsys):
    mapping = {
        "pools": {
            "CardPool_Character": "限定棋盤",
            "CardPool_NewRole": "標準棋盤",
        },
        "pool_meta": {
            "ForkLottery_AnHunQu": {"group_label": "弧盤研募"},
        },
    }
    source = SourceRef(session=0, line=1, packet_index=0, view="src", row_index=0, offset=0)
    records = [
        GachaRecord(
            record_id="a",
            record_type="monopoly",
            time="2026-04-30 17:02:15",
            pool_id="CardPool_Character",
            pool_name="限定棋盤",
            item_id="item_a",
            item_name="測試道具",
            count=1,
            roll_points=2,
            roll_label="2",
            secondary_item_id=None,
            secondary_item_name="",
            secondary_count=None,
            source=source,
        ),
        GachaRecord(
            record_id="b",
            record_type="fork",
            time="2026-04-30 17:03:15",
            pool_id="ForkLottery_AnHunQu",
            pool_name="奇蹟盒盒",
            item_id="item_b",
            item_name="弧盤道具",
            count=1,
            roll_points=None,
            roll_label="",
            secondary_item_id=None,
            secondary_item_name="",
            secondary_count=None,
            source=source,
        ),
    ]

    progress = CaptureProgress(mapping, verbose=True)
    progress.update([])
    progress.update(records)
    progress.finish()

    out = capsys.readouterr().out
    assert "限定棋盤=0 標準棋盤=0 弧盤研募=0" in out
    assert "2026-04-30 17:02:15 | 限定棋盤 | roll=2 | 測試道具 x1" in out
    assert "2026-04-30 17:03:15 | 奇蹟盒盒 | 弧盤道具 x1" in out
    assert "限定棋盤=1 標準棋盤=0 弧盤研募=1" in out
