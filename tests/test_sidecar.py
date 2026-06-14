from __future__ import annotations

from pathlib import Path

import nte_gacha_exporter.sidecar.main as sidecar
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


def test_sidecar_rules_build_returns_asset_backed_pool_and_item_meta(monkeypatch):
    monkeypatch.delenv("NTE_ASSETS_ROOT", raising=False)
    response = handle_request(
        SidecarState(),
        {"jsonrpc": "2.0", "id": 1, "method": "rules.build", "params": {"locale": "zh-Hant"}},
    )

    result = response["result"]
    assert any(rule["pool_id"] == "CardPool_Character" for rule in result["pool_rules"])
    fork_rule = next(rule for rule in result["pool_rules"] if rule["pool_id"] == "ForkLottery_AnHunQu")
    vehicle_item = next(item for item in result["item_meta"] if item["item_id"] == "Fashion_vehicle_1010_V008")
    dice_alias = next(alias for alias in result["item_aliases"] if alias["alias_id"] == "DIceNormal")
    assert fork_rule == {
        "pool_id": "ForkLottery_AnHunQu",
        "pool_name": "奇蹟盒盒",
        "group_label": "弧盤研募",
        "pickup_item_ids": ["fork_Rose"],
    }
    assert set(vehicle_item) == {"item_id", "item_name", "rarity", "category"}
    assert set(dice_alias) == {"alias_id", "item_id"}
    assert vehicle_item["rarity"] == 5
    assert dice_alias["item_id"] == "DiceNormal"


def test_sidecar_rules_build_errors_when_map_has_no_rules_sections(monkeypatch):
    monkeypatch.delenv("NTE_ASSETS_ROOT", raising=False)

    monkeypatch.setattr(
        "nte_gacha_exporter.sidecar.rules.load_map",
        lambda _locale: {"items": {}, "pools": {}, "pool_meta": {}},
    )

    response = handle_request(
        SidecarState(),
        {"jsonrpc": "2.0", "id": 1, "method": "rules.build", "params": {"locale": "zh-Hant"}},
    )

    assert response["error"]["code"] == "rules_build_failed"
    assert "pool_rules" in response["error"]["message"]


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


def test_sidecar_unknown_method_returns_structured_error():
    response = handle_request(SidecarState(), {"jsonrpc": "2.0", "id": 1, "method": "missing"})

    assert response["error"]["code"] == "method_not_found"
