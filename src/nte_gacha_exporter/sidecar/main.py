from __future__ import annotations

import json
import sys
import uuid
from collections.abc import Callable
from pathlib import Path
from typing import Any

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from nte_gacha_exporter.export.pipeline import export_capture
from nte_gacha_exporter.mapping.runtime import available_locales, load_map

JsonObject = dict[str, Any]


class RpcError(RuntimeError):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code
        self.message = message


class SidecarState:
    def __init__(self) -> None:
        self.sessions: dict[str, JsonObject] = {}


def _object(value: Any, *, code: str, message: str) -> JsonObject:
    if not isinstance(value, dict):
        raise RpcError(code, message)
    return value


def _text_param(params: JsonObject, key: str, *, default: str | None = None) -> str:
    value = params.get(key, default)
    if not isinstance(value, str) or not value:
        raise RpcError("invalid_params", f"missing string param: {key}")
    return value


def _session_payload(state: SidecarState, document: JsonObject) -> JsonObject:
    session_id = uuid.uuid4().hex
    payload = {
        "session_id": session_id,
        "state": "completed",
        "document": document,
        "records_count": len(document.get("nte", {}).get("list", [])),
    }
    state.sessions[session_id] = payload
    return payload


def _handle_ping(_state: SidecarState, _params: JsonObject) -> JsonObject:
    return {"ok": True}


def _handle_maps_list(_state: SidecarState, _params: JsonObject) -> JsonObject:
    return {"locales": available_locales()}


def _handle_raw_replay(state: SidecarState, params: JsonObject) -> JsonObject:
    raw_path = Path(_text_param(params, "path"))
    locale = _text_param(params, "locale", default="zh-Hant")
    try:
        document = export_capture(raw_path, locale=locale)
    except Exception as exc:
        raise RpcError("raw_replay_failed", str(exc)) from exc
    return _session_payload(state, document)


def _handle_rules_build(_state: SidecarState, params: JsonObject) -> JsonObject:
    locale = _text_param(params, "locale", default="zh-Hant")
    try:
        mapping = load_map(locale)
    except Exception as exc:
        raise RpcError("rules_build_failed", str(exc)) from exc

    pools = mapping.get("pools", {})
    pool_meta = mapping.get("pool_meta", {})
    items = mapping.get("items", {})
    pool_rules = []
    if isinstance(pools, dict):
        for pool_id, pool_name in sorted(pools.items()):
            meta = pool_meta.get(pool_id, {}) if isinstance(pool_meta, dict) else {}
            meta = meta if isinstance(meta, dict) else {}
            pool_rules.append(
                {
                    "pool_id": str(pool_id),
                    "pool_name": str(pool_name),
                    "group_label": str(meta.get("group_label") or pool_name),
                    "rule_source": "map",
                }
            )

    item_meta = []
    if isinstance(items, dict):
        for item_id, item_name in sorted(items.items()):
            item_meta.append(
                {
                    "item_id": str(item_id),
                    "item_name": str(item_name),
                    "rule_source": "map",
                }
            )

    return {"pool_rules": pool_rules, "item_meta": item_meta}


def _handle_session_result(state: SidecarState, params: JsonObject) -> JsonObject:
    session_id = _text_param(params, "session_id")
    result = state.sessions.get(session_id)
    if result is None:
        raise RpcError("session_not_found", f"session not found: {session_id}")
    return result


Handler = Callable[[SidecarState, JsonObject], JsonObject]

HANDLERS: dict[str, Handler] = {
    "app.ping": _handle_ping,
    "maps.list": _handle_maps_list,
    "raw.replay": _handle_raw_replay,
    "raw.import": _handle_raw_replay,
    "rules.build": _handle_rules_build,
    "session.result": _handle_session_result,
}


def handle_request(state: SidecarState, request: JsonObject) -> JsonObject:
    request_id = request.get("id")
    method = request.get("method")
    if request.get("jsonrpc") != "2.0":
        return _error_response(request_id, "invalid_request", "jsonrpc must be 2.0")
    if not isinstance(method, str) or not method:
        return _error_response(request_id, "invalid_request", "method must be a string")
    handler = HANDLERS.get(method)
    if handler is None:
        return _error_response(request_id, "method_not_found", f"method not found: {method}")
    try:
        params = _object(request.get("params") or {}, code="invalid_params", message="params must be an object")
        return {"jsonrpc": "2.0", "id": request_id, "result": handler(state, params)}
    except RpcError as exc:
        return _error_response(request_id, exc.code, exc.message)
    except Exception as exc:
        return _error_response(request_id, "internal_error", str(exc))


def _error_response(request_id: Any, code: str, message: str) -> JsonObject:
    return {"jsonrpc": "2.0", "id": request_id, "error": {"code": code, "message": message}}


def main() -> int:
    state = SidecarState()
    for line in sys.stdin:
        if not line.strip():
            continue
        try:
            request = json.loads(line)
        except json.JSONDecodeError as exc:
            response = _error_response(None, "parse_error", str(exc))
        else:
            response = handle_request(
                state,
                _object(request, code="invalid_request", message="request must be an object"),
            )
        sys.stdout.write(json.dumps(response, ensure_ascii=False) + "\n")
        sys.stdout.flush()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
