from __future__ import annotations

import json
import platform
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
from threading import Event
from typing import Any, TextIO

from nte_gacha_exporter.capture.packet import packet_to_raw_record
from nte_gacha_exporter.capture.windows_net import (
    CaptureTarget,
    find_htgame_pid,
    resolve_scapy_iface,
    select_capture_target,
)
from nte_gacha_exporter.core.models import GachaRecord, ParsedRow, ParseWarning
from nte_gacha_exporter.core.schema import (
    CaptureStartRecord,
    CaptureStopRecord,
    ExportDocument,
    LocalizationMap,
    RawCaptureRecord,
    RawPacketRecord,
)
from nte_gacha_exporter.decode.binary import parse_packet_record
from nte_gacha_exporter.export.assembler import ProtocolAssembler
from nte_gacha_exporter.export.document import ExportOptions, build_document, public_records
from nte_gacha_exporter.export.writers import write_csv, write_debug_json, write_json
from nte_gacha_exporter.mapping.runtime import load_locale_map


class CaptureEnvironmentError(RuntimeError):
    """Raised when live capture cannot start in the current environment."""


@dataclass(frozen=True)
class CaptureHistoryOptions:
    json_out: Path
    csv_out: Path | None
    locale: str
    pid: str | None = None
    iface: str | None = None
    output_raw: Path | None = None
    debug_json_out: Path | None = None
    on_records: Callable[[list[GachaRecord]], None] | None = None
    on_ready: Callable[[CaptureTarget], None] | None = None
    on_progress: Callable[[dict[str, int]], None] | None = None
    stop_event: Event | None = None


@dataclass(frozen=True)
class CaptureLiveOptions:
    locale: str
    pid: str | None = None
    iface: str | None = None
    output_raw: Path | None = None
    on_records: Callable[[list[GachaRecord]], None] | None = None
    on_ready: Callable[[CaptureTarget], None] | None = None
    on_progress: Callable[[dict[str, int]], None] | None = None
    stop_event: Event | None = None


@dataclass
class _CaptureCounters:
    seen: int = 0
    decoded_packets: int = 0
    dropped: int = 0


def _require_windows() -> None:
    if platform.system() != "Windows":
        raise CaptureEnvironmentError("live capture requires Windows + Npcap")


def _load_scapy():
    try:
        from scapy.config import conf  # type: ignore
        from scapy.layers import inet, l2  # type: ignore  # noqa: F401
        from scapy.sendrecv import sniff  # type: ignore
    except Exception as exc:
        raise CaptureEnvironmentError(f"Scapy unavailable: {exc}") from exc
    return conf, sniff


def list_interfaces() -> list[str]:
    _require_windows()
    conf, _sniff = _load_scapy()
    return [
        f"{key}: name={getattr(iface, 'name', '')} ip={getattr(iface, 'ip', '')}" for key, iface in conf.ifaces.items()
    ]


def _target_for_capture(pid: str | None, iface: str | None) -> CaptureTarget:
    conf, _sniff = _load_scapy()
    actual_pid = pid or find_htgame_pid()
    if not actual_pid:
        raise CaptureEnvironmentError("HTGame.exe PID not found")

    if iface:
        from nte_gacha_exporter.capture.windows_net import candidate_ports

        ports = candidate_ports(actual_pid)
        return CaptureTarget(
            pid=actual_pid,
            interface=iface,
            ports=ports,
            selected_by_port=None,
            bpf=" or ".join(f"port {port}" for port in ports),
        )
    try:
        return select_capture_target(actual_pid, conf)
    except RuntimeError as exc:
        raise CaptureEnvironmentError(str(exc)) from exc


def _raw_start(target: CaptureTarget) -> CaptureStartRecord:
    return {
        "type": "capture_start",
        "schema_version": 1,
        "pid": target.pid,
        "iface": target.interface,
        "ports": target.ports,
        "bpf": target.bpf,
    }


def _raw_stop(counters: _CaptureCounters) -> CaptureStopRecord:
    return {
        "type": "capture_stop",
        "schema_version": 1,
        "seen": counters.seen,
        "decoded_packets": counters.decoded_packets,
        "dropped": counters.dropped,
    }


def _progress_payload(counters: _CaptureCounters) -> dict[str, int]:
    return {
        "packets_seen": counters.seen,
        "decoded_packets": counters.decoded_packets,
        "dropped_packets": counters.dropped,
    }


def _write_raw_record(raw_fh: TextIO | None, record: RawCaptureRecord) -> None:
    if raw_fh is None:
        return
    raw_fh.write(json.dumps(record, ensure_ascii=False) + "\n")
    raw_fh.flush()


def _packet_rows(
    record: RawPacketRecord,
    *,
    counters: _CaptureCounters,
    warnings: list[ParseWarning],
    assembler: ProtocolAssembler,
) -> list[ParsedRow]:
    before = assembler.rows()
    blocks, found_warnings = parse_packet_record(
        record,
        session=0,
        line=counters.seen,
        packet_index=counters.seen - 1,
    )
    warnings.extend(found_warnings)
    if not blocks:
        return []
    counters.decoded_packets += 1
    assembler.add_blocks(blocks)
    after = assembler.rows()
    return after if after != before else []


def _open_raw_writer(path: Path | None, target: CaptureTarget) -> TextIO | None:
    if path is None:
        return None
    path.parent.mkdir(parents=True, exist_ok=True)
    raw_fh = path.open("w", encoding="utf-8")
    _write_raw_record(raw_fh, _raw_start(target))
    return raw_fh


def _run_sniff_loop(
    sniff: Callable[..., Any],
    *,
    scapy_iface: Any,
    target: CaptureTarget,
    stop: Event,
    on_packet: Callable[[Any], None],
) -> None:
    try:
        while not stop.is_set():
            sniff(iface=scapy_iface, filter=target.bpf, prn=on_packet, store=False, timeout=0.5)
    except KeyboardInterrupt:
        stop.set()


def _build_live_document(
    assembler: ProtocolAssembler,
    warnings: list[ParseWarning],
    counters: _CaptureCounters,
    mapping: LocalizationMap,
    locale_name: str,
) -> ExportDocument:
    document = build_document(
        assembler.rows(),
        mapping,
        ExportOptions(locale=locale_name, source="live-capture"),
        [*warnings, *assembler.warnings],
    )
    summary = document["_debug"]["summary"]
    summary["packets_seen"] = counters.seen
    summary["decoded_packets"] = counters.decoded_packets
    summary["dropped_packets"] = counters.dropped
    return document


def _write_history_outputs(options: CaptureHistoryOptions, document: ExportDocument) -> None:
    write_json(options.json_out, document)
    if options.debug_json_out:
        write_debug_json(options.debug_json_out, document)
    if options.csv_out:
        write_csv(options.csv_out, document)


def capture_history(options: CaptureHistoryOptions) -> ExportDocument:
    """Capture live packets and write sanitized history outputs."""

    document = capture_live(
        CaptureLiveOptions(
            locale=options.locale,
            pid=options.pid,
            iface=options.iface,
            output_raw=options.output_raw,
            on_records=options.on_records,
            on_ready=options.on_ready,
            on_progress=options.on_progress,
            stop_event=options.stop_event,
        )
    )
    _write_history_outputs(options, document)
    return document


def capture_live(options: CaptureLiveOptions) -> ExportDocument:
    """Capture live packets and return a sanitized document without public file output."""

    _require_windows()
    _conf, sniff = _load_scapy()
    target = _target_for_capture(options.pid, options.iface)
    scapy_iface = resolve_scapy_iface(target.interface)
    locale_name, mapping = load_locale_map(options.locale)
    assembler = ProtocolAssembler()
    warnings: list[ParseWarning] = []
    counters = _CaptureCounters()
    stop = options.stop_event or Event()
    raw_fh = _open_raw_writer(options.output_raw, target)
    if options.on_records:
        options.on_records([])
    if options.on_ready:
        options.on_ready(target)
    if options.on_progress:
        options.on_progress(_progress_payload(counters))

    def on_packet(packet: Any) -> None:
        counters.seen += 1
        record = packet_to_raw_record(packet, capture_index=counters.seen)
        if record is None:
            counters.dropped += 1
            if options.on_progress:
                options.on_progress(_progress_payload(counters))
            return
        _write_raw_record(raw_fh, record)
        snapshot_rows = _packet_rows(record, counters=counters, warnings=warnings, assembler=assembler)
        if snapshot_rows and options.on_records:
            options.on_records(public_records(snapshot_rows, mapping))
        if options.on_progress:
            options.on_progress(_progress_payload(counters))

    try:
        _run_sniff_loop(sniff, scapy_iface=scapy_iface, target=target, stop=stop, on_packet=on_packet)
    finally:
        if raw_fh:
            _write_raw_record(raw_fh, _raw_stop(counters))
            raw_fh.close()

    return _build_live_document(assembler, warnings, counters, mapping, locale_name)


def doctor() -> tuple[int, list[str]]:
    """Return environment diagnostics for live capture."""

    lines: list[str] = []
    ok = True
    if platform.system() != "Windows":
        return 3, ["Windows: unavailable", "Live capture requires Windows + Npcap."]

    lines.append("Windows: ok")
    try:
        conf, _sniff = _load_scapy()
    except CaptureEnvironmentError as exc:
        return 3, ["Scapy: unavailable", str(exc)]

    iface_count = len(list(conf.ifaces.items()))
    lines.append(f"Scapy: ok ({iface_count} interfaces)")
    pid = find_htgame_pid()
    if pid:
        lines.append(f"HTGame.exe: pid={pid}")
    else:
        ok = False
        lines.append("HTGame.exe: not running")

    return (0 if ok else 3), lines
