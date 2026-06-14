from __future__ import annotations

import json
from pathlib import Path
from types import SimpleNamespace

import pytest

import nte_gacha_exporter.capture.live as live
import nte_gacha_exporter.capture.windows_net as windows_net
from nte_gacha_exporter.capture.live import _load_scapy
from nte_gacha_exporter.core.models import ParsedBlock, ParsedRow, ProtocolEnvelope, SourceRef

FIXTURE = Path(__file__).parent / "fixtures" / "sample.raw.jsonl"


def test_candidate_ports_excludes_https_remote_port(monkeypatch):
    conns = [
        SimpleNamespace(pid=1234, remote_port=443, remote_ip="203.0.113.1"),
        SimpleNamespace(pid=1234, remote_port=30231, remote_ip="203.0.113.1"),
    ]

    monkeypatch.setattr(windows_net, "_winapi_module", lambda: SimpleNamespace(get_tcp_table=lambda: conns))
    monkeypatch.setattr(windows_net, "_udp_local_ports_windows", lambda _pid: [])

    ports = windows_net.candidate_ports("1234")

    assert 443 not in ports
    assert 30231 in ports


def _snapshot_block(segment_index: int, item_indexes: list[int]) -> ParsedBlock:
    rows = tuple(
        ParsedRow(
            record_type="monopoly",
            ticks=639000000000000000 + item_index,
            time=f"2026-06-10T08:00:{item_index:02d}.000000",
            pool_id="CardPool_Character",
            item_id=f"item_{item_index}",
            count=1,
            roll_points=item_index,
            roll_label_id=None,
            secondary_item_id=None,
            secondary_count=None,
            source=SourceRef(
                session=0,
                line=item_index,
                packet_index=item_index,
                view="src",
                row_index=item_index,
                offset=item_index,
                stream_key="character",
                page_index=(segment_index + 1) // 2,
                query_high=segment_index % 2 == 0,
                segment_index=segment_index,
            ),
        )
        for item_index in item_indexes
    )
    return ParsedBlock(
        "monopoly",
        0,
        0,
        len(rows),
        rows,
        ProtocolEnvelope(
            record_type="monopoly",
            stream_key="character",
            page_index=(segment_index + 1) // 2,
            query_high=segment_index % 2 == 0,
            segment_index=segment_index,
        ),
    )


def test_load_scapy_registers_ethernet_linktype():
    pytest.importorskip("scapy")

    conf, _sniff = _load_scapy()

    assert getattr(conf.l2types[1], "__name__", "") == "Ether"


def test_capture_history_writes_outputs_after_stop_event(monkeypatch, tmp_path):
    packet_record = json.loads(FIXTURE.read_text(encoding="utf-8").splitlines()[1])
    stop_event = live.Event()
    seen_records = []
    ready_targets = []

    def fake_sniff(*, prn, timeout, **_kwargs):
        assert timeout == 0.5
        prn(object())
        stop_event.set()

    monkeypatch.setattr(live, "_require_windows", lambda: None)
    monkeypatch.setattr(live, "_load_scapy", lambda: (object(), fake_sniff))
    monkeypatch.setattr(
        live,
        "_target_for_capture",
        lambda *_args, **_kwargs: SimpleNamespace(pid="1234", interface="npcap0", ports=[30230], bpf="port 30230"),
    )
    monkeypatch.setattr(live, "resolve_scapy_iface", lambda iface: iface)
    monkeypatch.setattr(live, "packet_to_raw_record", lambda _packet, *, capture_index: packet_record)

    document = live.capture_history(
        live.CaptureHistoryOptions(
            json_out=tmp_path / "history.json",
            csv_out=tmp_path / "history.csv",
            locale="zh-Hant",
            output_raw=tmp_path / "raw.jsonl",
            on_records=seen_records.extend,
            on_ready=ready_targets.append,
            stop_event=stop_event,
        )
    )

    raw_lines = (tmp_path / "raw.jsonl").read_text(encoding="utf-8").splitlines()
    assert document["_debug"]["summary"]["record_count"] == 1
    assert (tmp_path / "history.json").exists()
    assert (tmp_path / "history.csv").exists()
    assert json.loads(raw_lines[0])["type"] == "capture_start"
    assert json.loads(raw_lines[-1])["type"] == "capture_stop"
    assert seen_records[0].pool_id == "CardPool_Character"
    assert ready_targets[0].pid == "1234"


def test_capture_live_returns_document_without_public_outputs(monkeypatch, tmp_path):
    packet_record = json.loads(FIXTURE.read_text(encoding="utf-8").splitlines()[1])
    stop_event = live.Event()
    progress = []

    def fake_sniff(*, prn, timeout, **_kwargs):
        assert timeout == 0.5
        prn(object())
        stop_event.set()

    monkeypatch.setattr(live, "_require_windows", lambda: None)
    monkeypatch.setattr(live, "_load_scapy", lambda: (object(), fake_sniff))
    monkeypatch.setattr(
        live,
        "_target_for_capture",
        lambda *_args, **_kwargs: SimpleNamespace(pid="1234", interface="npcap0", ports=[30230], bpf="port 30230"),
    )
    monkeypatch.setattr(live, "resolve_scapy_iface", lambda iface: iface)
    monkeypatch.setattr(live, "packet_to_raw_record", lambda _packet, *, capture_index: packet_record)

    document = live.capture_live(
        live.CaptureLiveOptions(
            locale="zh-Hant",
            on_progress=progress.append,
            stop_event=stop_event,
        )
    )

    assert document["_debug"]["summary"]["record_count"] == 1
    assert progress[-1] == {"packets_seen": 1, "decoded_packets": 1, "dropped_packets": 0}
    assert not (tmp_path / "history.json").exists()
    assert not (tmp_path / "history.csv").exists()


def test_capture_history_silently_ignores_duplicate_records(monkeypatch, tmp_path):
    packet_record = json.loads(FIXTURE.read_text(encoding="utf-8").splitlines()[1])
    stop_event = live.Event()
    seen_batches = []

    def fake_sniff(*, prn, timeout, **_kwargs):
        assert timeout == 0.5
        prn(object())
        prn(object())
        stop_event.set()

    monkeypatch.setattr(live, "_require_windows", lambda: None)
    monkeypatch.setattr(live, "_load_scapy", lambda: (object(), fake_sniff))
    monkeypatch.setattr(
        live,
        "_target_for_capture",
        lambda *_args, **_kwargs: SimpleNamespace(pid="1234", interface="npcap0", ports=[30230], bpf="port 30230"),
    )
    monkeypatch.setattr(live, "resolve_scapy_iface", lambda iface: iface)
    monkeypatch.setattr(live, "packet_to_raw_record", lambda _packet, *, capture_index: packet_record)

    document = live.capture_history(
        live.CaptureHistoryOptions(
            json_out=tmp_path / "history.json",
            csv_out=tmp_path / "history.csv",
            locale="zh-Hant",
            output_raw=tmp_path / "raw.jsonl",
            on_records=seen_batches.append,
            stop_event=stop_event,
        )
    )

    raw_records = [json.loads(line) for line in (tmp_path / "raw.jsonl").read_text(encoding="utf-8").splitlines()]
    raw_packets = [record for record in raw_records if record.get("type") == "packet"]
    assert document["_debug"]["summary"]["record_count"] == 1
    assert [len(batch) for batch in seen_batches] == [0, 1]
    assert len(raw_packets) == 2


def test_capture_history_progress_receives_current_snapshot_after_tail_append(monkeypatch, tmp_path):
    stop_event = live.Event()
    seen_batches = []

    def fake_sniff(*, prn, timeout, **_kwargs):
        assert timeout == 0.5
        prn(object())
        prn(object())
        stop_event.set()

    def fake_packet_record(_packet, *, capture_index):
        return {"type": "packet", "schema_version": 1, "capture_index": capture_index, "payload_b64": ""}

    def fake_parse_packet_record(record, **_kwargs):
        capture_index = int(record["capture_index"])
        block = _snapshot_block(capture_index - 1, [capture_index - 1])
        return [block], []

    monkeypatch.setattr(live, "_require_windows", lambda: None)
    monkeypatch.setattr(live, "_load_scapy", lambda: (object(), fake_sniff))
    monkeypatch.setattr(
        live,
        "_target_for_capture",
        lambda *_args, **_kwargs: SimpleNamespace(pid="1234", interface="npcap0", ports=[30230], bpf="port 30230"),
    )
    monkeypatch.setattr(live, "resolve_scapy_iface", lambda iface: iface)
    monkeypatch.setattr(live, "packet_to_raw_record", fake_packet_record)
    monkeypatch.setattr(live, "parse_packet_record", fake_parse_packet_record)

    document = live.capture_history(
        live.CaptureHistoryOptions(
            json_out=tmp_path / "history.json",
            csv_out=tmp_path / "history.csv",
            locale="zh-Hant",
            output_raw=tmp_path / "raw.jsonl",
            on_records=seen_batches.append,
            stop_event=stop_event,
        )
    )

    assert document["_debug"]["summary"]["record_count"] == 2
    assert [len(batch) for batch in seen_batches] == [0, 1, 2]
