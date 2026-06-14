from __future__ import annotations

import json
import sys
import threading
import time
import uuid
from collections.abc import Callable
from pathlib import Path
from typing import Any

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from nte_gacha_exporter.capture.live import CaptureEnvironmentError, CaptureLiveOptions, capture_live, doctor
from nte_gacha_exporter.export.pipeline import export_capture
from nte_gacha_exporter.mapping.runtime import available_locales
from nte_gacha_exporter.sidecar.rules import build_rules

JsonObject = dict[str, Any]


class RpcError(RuntimeError):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code
        self.message = message


class SidecarState:
    def __init__(self) -> None:
        self.sessions: dict[str, JsonObject] = {}
        self.capture_sessions: dict[str, CaptureSession] = {}


class CaptureSession:
    def __init__(
        self,
        *,
        session_id: str,
        locale: str,
        pid: str | None,
        iface: str | None,
        output_raw: Path | None,
    ) -> None:
        self.session_id = session_id
        self.locale = locale
        self.pid = pid
        self.iface = iface
        self.output_raw = output_raw
        self.stop_event = threading.Event()
        self.lock = threading.Lock()
        self.state = "starting"
        self.started_at = time.time()
        self.updated_at = self.started_at
        self.records_count = 0
        self.latest_records: list[JsonObject] = []
        self.counters = {"packets_seen": 0, "decoded_packets": 0, "dropped_packets": 0}
        self.target: JsonObject | None = None
        self.document: JsonObject | None = None
        self.error: JsonObject | None = None
        self.thread = threading.Thread(target=self._run, name=f"nte-capture-{session_id}", daemon=True)

    def start(self) -> None:
        self.thread.start()

    def stop(self) -> None:
        self.stop_event.set()
        with self.lock:
            if self.state in {"starting", "running"}:
                self.state = "stopping"
                self.updated_at = time.time()

    def status(self, *, include_document: bool = False) -> JsonObject:
        with self.lock:
            payload: JsonObject = {
                "session_id": self.session_id,
                "state": self.state,
                "records_count": self.records_count,
                "latest_records": list(self.latest_records),
                "counters": dict(self.counters),
                "started_at": self.started_at,
                "updated_at": self.updated_at,
            }
            if self.target is not None:
                payload["target"] = dict(self.target)
            if self.error is not None:
                payload["error"] = dict(self.error)
            if include_document and self.document is not None:
                payload["document"] = self.document
            return payload

    def _run(self) -> None:
        try:
            document = capture_live(
                CaptureLiveOptions(
                    locale=self.locale,
                    pid=self.pid,
                    iface=self.iface,
                    output_raw=self.output_raw,
                    on_records=self._update_records,
                    on_ready=self._update_ready,
                    on_progress=self._update_progress,
                    stop_event=self.stop_event,
                )
            )
        except CaptureEnvironmentError as exc:
            self._fail("capture_environment", str(exc))
        except Exception as exc:
            self._fail("capture_failed", str(exc))
        else:
            self._complete(document)

    def _update_ready(self, target: Any) -> None:
        with self.lock:
            self.state = "running"
            self.target = {
                "pid": str(target.pid),
                "interface": str(target.interface),
                "ports": list(target.ports),
                "bpf": str(target.bpf),
            }
            self.updated_at = time.time()

    def _update_progress(self, counters: JsonObject) -> None:
        with self.lock:
            self.counters = {
                "packets_seen": int(counters.get("packets_seen", 0)),
                "decoded_packets": int(counters.get("decoded_packets", 0)),
                "dropped_packets": int(counters.get("dropped_packets", 0)),
            }
            self.updated_at = time.time()

    def _update_records(self, records: list[Any]) -> None:
        public_records = [record.to_dict() if hasattr(record, "to_dict") else record for record in records]
        with self.lock:
            self.records_count = len(public_records)
            self.latest_records = public_records[-12:]
            if self.state == "starting":
                self.state = "running"
            self.updated_at = time.time()

    def _complete(self, document: JsonObject) -> None:
        records = document.get("nte", {}).get("list", [])
        with self.lock:
            self.document = document
            self.records_count = len(records) if isinstance(records, list) else 0
            self.latest_records = records[-12:] if isinstance(records, list) else []
            self.state = "completed"
            self.updated_at = time.time()

    def _fail(self, code: str, message: str) -> None:
        with self.lock:
            self.error = {"code": code, "message": message}
            self.state = "failed"
            self.updated_at = time.time()


def _object(value: Any, *, code: str, message: str) -> JsonObject:
    if not isinstance(value, dict):
        raise RpcError(code, message)
    return value


def _text_param(params: JsonObject, key: str, *, default: str | None = None) -> str:
    value = params.get(key, default)
    if not isinstance(value, str) or not value:
        raise RpcError("invalid_params", f"missing string param: {key}")
    return value


def _optional_text_param(params: JsonObject, key: str) -> str | None:
    value = params.get(key)
    if value is None or value == "":
        return None
    if not isinstance(value, str):
        raise RpcError("invalid_params", f"param must be a string: {key}")
    return value


def _bool_param(params: JsonObject, key: str, *, default: bool = False) -> bool:
    value = params.get(key, default)
    if not isinstance(value, bool):
        raise RpcError("invalid_params", f"param must be a boolean: {key}")
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


def _handle_doctor_run(_state: SidecarState, _params: JsonObject) -> JsonObject:
    exit_code, lines = doctor()
    return {"ok": exit_code == 0, "exit_code": exit_code, "lines": lines}


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
        return build_rules(locale)
    except Exception as exc:
        raise RpcError("rules_build_failed", str(exc)) from exc


def _handle_capture_start(state: SidecarState, params: JsonObject) -> JsonObject:
    session_id = uuid.uuid4().hex
    output_raw = _optional_text_param(params, "output_raw")
    session = CaptureSession(
        session_id=session_id,
        locale=_text_param(params, "locale", default="zh-Hant"),
        pid=_optional_text_param(params, "pid"),
        iface=_optional_text_param(params, "iface"),
        output_raw=Path(output_raw) if output_raw else None,
    )
    state.capture_sessions[session_id] = session
    session.start()
    return session.status()


def _handle_capture_status(state: SidecarState, params: JsonObject) -> JsonObject:
    session = _capture_session(state, params)
    return session.status(include_document=_bool_param(params, "include_document"))


def _handle_capture_stop(state: SidecarState, params: JsonObject) -> JsonObject:
    session = _capture_session(state, params)
    session.stop()
    return session.status()


def _handle_session_result(state: SidecarState, params: JsonObject) -> JsonObject:
    session_id = _text_param(params, "session_id")
    result = state.sessions.get(session_id)
    if result is not None:
        return result
    session = state.capture_sessions.get(session_id)
    if session is not None:
        return session.status(include_document=True)
    raise RpcError("session_not_found", f"session not found: {session_id}")


def _capture_session(state: SidecarState, params: JsonObject) -> CaptureSession:
    session_id = _text_param(params, "session_id")
    session = state.capture_sessions.get(session_id)
    if session is None:
        raise RpcError("session_not_found", f"session not found: {session_id}")
    return session


Handler = Callable[[SidecarState, JsonObject], JsonObject]

HANDLERS: dict[str, Handler] = {
    "app.ping": _handle_ping,
    "doctor.run": _handle_doctor_run,
    "maps.list": _handle_maps_list,
    "raw.replay": _handle_raw_replay,
    "raw.import": _handle_raw_replay,
    "rules.build": _handle_rules_build,
    "capture.start": _handle_capture_start,
    "capture.status": _handle_capture_status,
    "capture.stop": _handle_capture_stop,
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
