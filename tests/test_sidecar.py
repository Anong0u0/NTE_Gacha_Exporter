from __future__ import annotations

from pathlib import Path

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


def test_sidecar_rules_build_returns_pool_and_item_meta():
    response = handle_request(
        SidecarState(),
        {"jsonrpc": "2.0", "id": 1, "method": "rules.build", "params": {"locale": "zh-Hant"}},
    )

    result = response["result"]
    assert any(rule["pool_id"] == "CardPool_Character" for rule in result["pool_rules"])
    assert any(item["item_id"] == "Fashion_vehicle_1010_V008" for item in result["item_meta"])


def test_sidecar_unknown_method_returns_structured_error():
    response = handle_request(SidecarState(), {"jsonrpc": "2.0", "id": 1, "method": "missing"})

    assert response["error"]["code"] == "method_not_found"
