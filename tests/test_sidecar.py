from __future__ import annotations

from pathlib import Path

import nte_gacha_exporter.sidecar.main as sidecar
from nte_gacha_exporter.automation.pager import AutoPageResult
from nte_gacha_exporter.sidecar.main import SidecarState, handle_request

FIXTURE = Path(__file__).parent / "fixtures" / "sample.raw.jsonl"


def test_sidecar_ping_returns_ok():
    response = handle_request(SidecarState(), {"jsonrpc": "2.0", "id": 1, "method": "app.ping"})

    assert response == {"jsonrpc": "2.0", "id": 1, "result": {"ok": True}}


def test_sidecar_raw_replay_returns_document_and_session_result():
    state = SidecarState()
    response = handle_request(
        state,
        {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "raw.replay",
            "params": {"path": str(FIXTURE), "locale": "zh-Hant"},
        },
    )

    result = response["result"]
    assert result["state"] == "completed"
    assert result["records_count"] == 2
    assert result["document"]["info"]["schema"] == "nte-gacha-export"
    assert len(result["document"]["nte"]["list"][0]["record_id"]) == 64

    session_response = handle_request(
        state,
        {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "session.result",
            "params": {"session_id": result["session_id"]},
        },
    )

    assert session_response["result"]["records_count"] == 2


def test_sidecar_maps_list_returns_bundled_locales(monkeypatch):
    monkeypatch.delenv("NTE_ASSETS_ROOT", raising=False)
    response = handle_request(
        SidecarState(),
        {"jsonrpc": "2.0", "id": 1, "method": "maps.list"},
    )

    result = response["result"]
    assert "zh-Hant" in result["locales"]
    assert "en" in result["locales"]


def test_sidecar_doctor_run_returns_structured_report(monkeypatch):
    monkeypatch.setattr(sidecar, "doctor", lambda: (3, ["Windows: unavailable"]))

    response = handle_request(SidecarState(), {"jsonrpc": "2.0", "id": 1, "method": "doctor.run"})

    assert response["result"] == {"ok": False, "exit_code": 3, "lines": ["Windows: unavailable"]}


def test_sidecar_live_capture_session_lifecycle(monkeypatch):
    def fake_capture_live(options):
        target = type(
            "Target",
            (),
            {"pid": "1234", "interface": "npcap0", "ports": [30230], "bpf": "port 30230"},
        )
        options.on_ready(target())
        options.on_progress({"packets_seen": 1, "decoded_packets": 1, "dropped_packets": 0})
        return {
            "info": {"schema": "nte-gacha-export"},
            "nte": {
                "list": [
                    {
                        "record_id": "live:1",
                        "record_type": "monopoly",
                        "item_id": "1003",
                        "item_name": "角色·早霧",
                    }
                ]
            },
        }

    monkeypatch.setattr(sidecar, "capture_live", fake_capture_live)
    state = SidecarState()
    start = handle_request(
        state,
        {"jsonrpc": "2.0", "id": 1, "method": "capture.start", "params": {"locale": "zh-Hant"}},
    )

    session_id = start["result"]["session_id"]
    state.capture_sessions[session_id].thread.join(timeout=2)
    result = handle_request(
        state,
        {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "capture.status",
            "params": {"session_id": session_id, "include_document": True},
        },
    )["result"]

    assert result["state"] == "completed"
    assert result["records_count"] == 1
    assert result["counters"]["packets_seen"] == 1
    assert result["document"]["nte"]["list"][0]["record_id"] == "live:1"


def test_sidecar_live_capture_stop_requests_session_stop(monkeypatch):
    def fake_capture_live(options):
        target = type(
            "Target",
            (),
            {"pid": "1234", "interface": "npcap0", "ports": [30230], "bpf": "port 30230"},
        )
        options.on_ready(target())
        options.stop_event.wait(timeout=1)
        return {"info": {"schema": "nte-gacha-export"}, "nte": {"list": []}}

    monkeypatch.setattr(sidecar, "capture_live", fake_capture_live)
    state = SidecarState()
    start = handle_request(
        state,
        {"jsonrpc": "2.0", "id": 1, "method": "capture.start", "params": {"locale": "zh-Hant"}},
    )
    session_id = start["result"]["session_id"]

    stop = handle_request(
        state,
        {"jsonrpc": "2.0", "id": 2, "method": "capture.stop", "params": {"session_id": session_id}},
    )
    state.capture_sessions[session_id].thread.join(timeout=2)
    result = handle_request(
        state,
        {
            "jsonrpc": "2.0",
            "id": 3,
            "method": "capture.status",
            "params": {"session_id": session_id, "include_document": True},
        },
    )["result"]

    assert stop["result"]["state"] in {"stopping", "completed"}
    assert result["state"] == "completed"
    assert result["document"]["nte"]["list"] == []


def test_sidecar_auto_page_incremental_passes_known_records_and_status(monkeypatch, tmp_path):
    records = [{"record_id": f"known-{index}", "pool_id": "CardPool_Character"} for index in range(5)]
    seen: dict[str, object] = {}

    def fake_capture_live(options):
        target = type(
            "Target",
            (),
            {"pid": "1234", "interface": "npcap0", "ports": [30230], "bpf": "port 30230"},
        )
        options.on_ready(target())
        options.on_records(records)
        options.stop_event.wait(timeout=1)
        return {"info": {"schema": "nte-gacha-export"}, "nte": {"list": records}}

    def fake_run_auto_page(options):
        seen["full_update"] = options.full_update
        seen["known_record_ids"] = options.known_record_ids
        seen["snapshot"] = options.record_snapshot()
        options.on_status(
            type(
                "Status",
                (),
                {
                    "message": "known page found; skipping pool",
                    "kind": "pool_skipped",
                    "step": "limitedBoardPages",
                    "pool": "limited",
                    "currentPage": 1,
                    "totalPages": 3,
                    "technicalDetail": "",
                },
            )()
        )
        return AutoPageResult("completed", "auto page completed", skippedPools=("limited",))

    monkeypatch.setattr(sidecar, "capture_live", fake_capture_live)
    monkeypatch.setattr("nte_gacha_exporter.automation.pager.run_auto_page", fake_run_auto_page)
    raw_path = tmp_path / "raw.jsonl"
    state = SidecarState()
    start = handle_request(
        state,
        {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "capture.start",
            "params": {
                "locale": "zh-Hant",
                "auto_page": True,
                "known_record_ids": [record["record_id"] for record in records],
                "output_raw": str(raw_path),
            },
        },
    )

    session_id = start["result"]["session_id"]
    state.capture_sessions[session_id].thread.join(timeout=2)
    result = handle_request(
        state,
        {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "capture.status",
            "params": {"session_id": session_id, "include_document": True},
        },
    )["result"]

    assert seen == {
        "full_update": False,
        "known_record_ids": tuple(record["record_id"] for record in records),
        "snapshot": records,
    }
    assert result["mode"] == "auto_page_incremental"
    assert result["raw_path"] == str(raw_path)
    assert result["auto_page"]["skipped_pools"] == ["limited"]
    assert result["document"]["nte"]["list"] == records


def test_sidecar_unknown_method_returns_structured_error():
    response = handle_request(SidecarState(), {"jsonrpc": "2.0", "id": 1, "method": "missing"})

    assert response["error"]["code"] == "method_not_found"
