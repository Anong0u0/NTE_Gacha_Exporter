from __future__ import annotations

import base64
import struct

import pytest

from nte_gacha_exporter.decode.binary import (
    FORK_MARKER,
    MONOPOLY_MARKER,
    ParseError,
    parse_item_spec,
    parse_packet_record,
    parse_payload_blocks,
    parse_protocol_envelope,
)


def fstring(value: str) -> bytes:
    raw = value.encode("utf-8") + b"\x00"
    return struct.pack("<I", len(raw)) + raw


def packet(payload: bytes) -> dict:
    return {
        "type": "packet",
        "schema_version": 1,
        "payload_b64": base64.b64encode(payload).decode("ascii"),
    }


def test_monopoly_record_decodes_public_fields():
    row = (
        struct.pack("<I", 2)
        + fstring("Fashion_vehicle_1010_V008")
        + struct.pack("<II", 0, 1)
        + fstring("Fashion_vehicle_1010_V008")
        + fstring("Fashion_vehicle_1010_V008")
        + fstring("CardPool_Character")
        + struct.pack("<Q", 639131653353040000)
    )
    payload = b"FMonopolyLotteryRecordData\x00" + struct.pack("<III", 0, len(row), 1) + row

    blocks, warnings = parse_packet_record(packet(payload), session=0, line=1, packet_index=0)

    assert warnings == []
    parsed = blocks[0].rows[0]
    assert parsed.record_type == "monopoly"
    assert parsed.roll_points == 2
    assert parsed.pool_id == "CardPool_Character"
    assert parsed.item_id == "Fashion_vehicle_1010_V008"
    assert parsed.secondary_item_id == "Fashion_vehicle_1010_V008"
    assert parsed.secondary_count == 1


def test_monopoly_record_decodes_secondary_fields_when_different():
    row = (
        struct.pack("<I", 5)
        + fstring("fork_nonos")
        + struct.pack("<II", 0, 2)
        + fstring("Dice_ticket_01")
        + fstring("fork_nonos")
        + fstring("CardPool_NewRole")
        + struct.pack("<Q", 639164696613410000)
    )
    payload = b"FMonopolyLotteryRecordData\x00" + struct.pack("<III", 0, len(row), 1) + row

    blocks, warnings = parse_packet_record(packet(payload), session=0, line=1, packet_index=0)

    assert warnings == []
    parsed = blocks[0].rows[0]
    assert parsed.item_id == "fork_nonos"
    assert parsed.count == 1
    assert parsed.secondary_item_id == "Dice_ticket_01"
    assert parsed.secondary_count == 2


def test_fork_record_decodes_public_fields():
    row = fstring("fork_dustbin") + fstring("ForkLottery_AnHunQu") + struct.pack("<Q", 639161037582960000)
    payload = b"FForkLotteryRecordData\x00" + struct.pack("<III", 0, len(row), 1) + row

    blocks, warnings = parse_packet_record(packet(payload), session=0, line=1, packet_index=0)

    assert warnings == []
    parsed = blocks[0].rows[0]
    assert parsed.record_type == "fork"
    assert parsed.pool_id == "ForkLottery_AnHunQu"
    assert parsed.item_id == "fork_dustbin"


def test_declared_size_shorter_than_payload_padding_does_not_block_complete_monopoly_row():
    row = (
        struct.pack("<I", 2)
        + fstring("Fashion_vehicle_1010_V008")
        + struct.pack("<II", 0, 1)
        + fstring("Fashion_vehicle_1010_V008")
        + fstring("Fashion_vehicle_1010_V008")
        + fstring("CardPool_Character")
        + struct.pack("<Q", 639131653353040000)
    )
    payload = b"FMonopolyLotteryRecordData\x00" + struct.pack("<III", 0, len(row) - 1, 1) + row

    blocks, warnings = parse_payload_blocks(payload, session=0, line=1, packet_index=0)

    assert warnings == []
    assert blocks[0].rows[0].item_id == "Fashion_vehicle_1010_V008"


def test_declared_size_longer_than_payload_padding_does_not_block_complete_fork_row():
    row = fstring("fork_dustbin") + fstring("ForkLottery_AnHunQu") + struct.pack("<Q", 639161037582960000)
    payload = b"FForkLotteryRecordData\x00" + struct.pack("<III", 0, len(row) + 1, 1) + row

    blocks, warnings = parse_payload_blocks(payload, session=0, line=1, packet_index=0)

    assert warnings == []
    assert blocks[0].rows[0].item_id == "fork_dustbin"


def test_truncated_fork_row_returns_warning():
    row = fstring("fork_dustbin") + fstring("ForkLottery_AnHunQu") + struct.pack("<Q", 639161037582960000)
    payload = b"FForkLotteryRecordData\x00" + struct.pack("<III", 0, len(row), 1) + row[:-1]

    blocks, warnings = parse_payload_blocks(payload, session=0, line=1, packet_index=0)

    assert blocks == []
    assert warnings[0].code == "parse_error"
    assert "payload range" in warnings[0].message


def test_invalid_candidate_block_returns_warning():
    payload = b"FMonopolyLotteryRecordData\x00" + struct.pack("<III", 0, 0, 101)

    blocks, warnings = parse_payload_blocks(payload, session=0, line=1, packet_index=0)

    assert blocks == []
    assert warnings[0].code == "parse_error"


def test_invalid_packet_payload_b64_returns_warning():
    blocks, warnings = parse_packet_record(
        {"type": "packet", "schema_version": 1, "payload_b64": "%%bad%%"},
        session=0,
        line=1,
        packet_index=0,
    )

    assert blocks == []
    assert warnings[0].code == "bad_packet"
    assert "payload_b64" in warnings[0].message


def test_item_spec_count_parser():
    assert parse_item_spec("Dice_ticket_02,30") == ("Dice_ticket_02", 30)
    assert parse_item_spec("fork_dustbin") == ("fork_dustbin", 1)
    assert parse_item_spec("bad,count") == ("bad,count", 1)


def test_monopoly_protocol_envelope_decodes_segment_metadata():
    values = [0, 0, 0, 0, 0x03000000, 0x80000002, 0x80000001, 527, 256, 1774080]
    prefix = b"".join(struct.pack("<I", value) for value in values) + b"\x00\x00"
    data = prefix + MONOPOLY_MARKER

    envelope = parse_protocol_envelope("monopoly", data, len(prefix), "shift8:2")

    assert envelope is not None
    assert envelope.stream_key == "monopoly:256"
    assert envelope.page_index == 1
    assert envelope.query_high is True
    assert envelope.segment_index == 2


def test_monopoly_protocol_envelope_decodes_unaligned_segment_metadata():
    prefix = (
        b"\x00" * 25
        + struct.pack("<I", 0x03000000)
        + struct.pack("<I", 2)
        + struct.pack("<I", 0x80000011)
        + struct.pack("<I", 527)
        + struct.pack("<I", 128)
        + struct.pack("<I", 1774080)
        + b"\x00\x00"
    )
    data = prefix + MONOPOLY_MARKER

    envelope = parse_protocol_envelope("monopoly", data, len(prefix), "shift8:4")

    assert envelope is not None
    assert envelope.stream_key == "monopoly:128"
    assert envelope.page_index == 17
    assert envelope.query_high is False
    assert envelope.segment_index == 33


def test_fork_protocol_envelope_decodes_segment_metadata():
    values = [0, 0, 0, 0, 0x03000000, 2, 8, 5906]
    prefix = b"".join(struct.pack("<I", value) for value in values) + b"\x00"
    data = prefix + FORK_MARKER

    envelope = parse_protocol_envelope("fork", data, len(prefix), "shift8:1")

    assert envelope is not None
    assert envelope.stream_key == "fork"
    assert envelope.page_index == 8
    assert envelope.query_high is False
    assert envelope.segment_index == 15


def test_invalid_shifted_protocol_envelope_is_rejected():
    values = [0, 0, 0, 0, 0x03000000, 0x80000002, 0, 528, 256, 1774080, 0, 0]
    prefix = b"".join(struct.pack("<I", value) for value in values) + b"\x00\x00"
    data = prefix + MONOPOLY_MARKER

    with pytest.raises(ParseError):
        parse_protocol_envelope("monopoly", data, len(prefix), "shift8:2")
