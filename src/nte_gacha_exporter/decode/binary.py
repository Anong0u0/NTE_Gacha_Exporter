from __future__ import annotations

import base64
import binascii
import struct
from collections.abc import Callable
from dataclasses import dataclass

from nte_gacha_exporter.core.models import ParsedBlock, ParsedRow, ParseWarning, ProtocolEnvelope, RecordType, SourceRef
from nte_gacha_exporter.core.schema import RawPacketRecord
from nte_gacha_exporter.core.time import dotnet_ticks_to_iso

MONOPOLY_MARKER = b"FMonopolyLotteryRecordData"
FORK_MARKER = b"FForkLotteryRecordData"
ROLL_POINT_LABEL_IDS = {
    0: "BPUI_LotteryResult_jidianzengli",
    0xFFFFFFFF: "BPUI_LotteryResult_chenmiandi",
}
MAX_ROWS_PER_BLOCK = 100
PROTOCOL_CONSTANT = 0x03000000
MONOPOLY_BLOCK_KIND = 527
FORK_BLOCK_KIND = 5906


class ParseError(ValueError):
    """Raised when a candidate binary block is not a valid history block."""


@dataclass(frozen=True)
class ParseContext:
    session: int
    line: int
    packet_index: int
    view: str


def payload_from_packet(record: RawPacketRecord) -> bytes:
    """Read payload bytes from the v1 raw packet schema."""

    if record.get("type") != "packet":
        raise ParseError("record type is not packet")
    payload = record.get("payload_b64")
    if not isinstance(payload, str):
        raise ParseError("packet missing payload_b64")
    try:
        return base64.b64decode(payload, validate=True)
    except (binascii.Error, ValueError) as exc:
        raise ParseError("invalid payload_b64") from exc


def _decode_shifted_bytes(data: bytes, byte_off: int, bit_shift: int, count: int | None = None) -> bytes:
    if count is None:
        count = max(0, len(data) - byte_off)

    out = bytearray()
    for i in range(count):
        bit_pos = (byte_off + i) * 8 + bit_shift
        b_off = bit_pos // 8
        b_shift = bit_pos % 8
        if b_off >= len(data):
            break

        value = data[b_off] >> b_shift
        if b_shift and b_off + 1 < len(data):
            value |= (data[b_off + 1] << (8 - b_shift)) & 0xFF
        out.append(value & 0xFF)
    return bytes(out)


def packet_views(data: bytes) -> list[tuple[str, bytes]]:
    """Return raw and bit-shifted payload views used by NTE history packets."""

    shifted = [
        (f"shift8:{shift}", _decode_shifted_bytes(data, 8, shift, count=max(0, len(data) - 8))) for shift in range(1, 8)
    ]
    return [("raw", data), *shifted]


def _u32(data: bytes, pos: int) -> int:
    if pos + 4 > len(data):
        raise ParseError("u32 out of range")
    return struct.unpack_from("<I", data, pos)[0]


def _u64(data: bytes, pos: int) -> int:
    if pos + 8 > len(data):
        raise ParseError("u64 out of range")
    return struct.unpack_from("<Q", data, pos)[0]


def _relative_u32(data: bytes, marker_pos: int, relative_pos: int) -> int:
    pos = marker_pos + relative_pos
    if pos < 0:
        raise ParseError("protocol envelope out of range")
    return _u32(data, pos)


def _segment_index(page_index: int, query_high: bool) -> int:
    segment_index = page_index * 2 if query_high else page_index * 2 - 1
    if segment_index < 0:
        raise ParseError("invalid protocol segment index")
    return segment_index


def _source_with_envelope(
    ctx: ParseContext,
    envelope: ProtocolEnvelope | None,
    *,
    row_index: int,
    offset: int,
) -> SourceRef:
    if envelope is None:
        return SourceRef(ctx.session, ctx.line, ctx.packet_index, ctx.view, row_index, offset)
    return SourceRef(
        ctx.session,
        ctx.line,
        ctx.packet_index,
        ctx.view,
        row_index,
        offset,
        stream_key=envelope.stream_key,
        page_index=envelope.page_index,
        query_high=envelope.query_high,
        segment_index=envelope.segment_index,
    )


def parse_protocol_envelope(
    record_type: RecordType,
    data: bytes,
    marker_pos: int,
    view: str,
) -> ProtocolEnvelope | None:
    """Read validated protocol page metadata before a history marker.

    Synthetic fixtures may place a marker at offset 0. Real shifted packets must
    match the observed envelope layout, otherwise the marker is a false positive.
    """

    if marker_pos == 0:
        return None

    if record_type == "monopoly":
        if not view.startswith("shift8:"):
            raise ParseError("invalid monopoly protocol view")
        if marker_pos < 26:
            raise ParseError("invalid monopoly protocol envelope")

        protocol_constant = _relative_u32(data, marker_pos, -26)
        query_raw = _relative_u32(data, marker_pos, -22)
        page_raw = _relative_u32(data, marker_pos, -18)
        block_kind = _relative_u32(data, marker_pos, -14)
        pool_token = _relative_u32(data, marker_pos, -10)
        footer = _relative_u32(data, marker_pos, -6)
        if protocol_constant != PROTOCOL_CONSTANT or block_kind != MONOPOLY_BLOCK_KIND or footer != 1774080:
            raise ParseError("invalid monopoly protocol constants")

        page_index = page_raw & 0x7FFFFFFF
        query_high = bool(query_raw & 0x80000000)
        return ProtocolEnvelope(
            record_type="monopoly",
            stream_key=f"monopoly:{pool_token}",
            page_index=page_index,
            query_high=query_high,
            segment_index=_segment_index(page_index, query_high),
        )

    if record_type == "fork":
        if not view.startswith("shift8:"):
            raise ParseError("invalid fork protocol view")
        if marker_pos < 17:
            raise ParseError("invalid fork protocol envelope")

        protocol_constant = _relative_u32(data, marker_pos, -17)
        query_raw = _relative_u32(data, marker_pos, -13)
        page_raw = _relative_u32(data, marker_pos, -9)
        block_kind = _relative_u32(data, marker_pos, -5)
        if protocol_constant != PROTOCOL_CONSTANT or block_kind != FORK_BLOCK_KIND:
            raise ParseError("invalid fork protocol constants")

        page_index = page_raw & 0x7FFFFFFF
        query_high = bool(query_raw & 0x80000000)
        return ProtocolEnvelope(
            record_type="fork",
            stream_key="fork",
            page_index=page_index,
            query_high=query_high,
            segment_index=_segment_index(page_index, query_high),
        )

    raise ParseError(f"unsupported record type: {record_type}")


class _Reader:
    def __init__(self, data: bytes, pos: int) -> None:
        self.data = data
        self.pos = pos

    def _require(self, size: int) -> None:
        if self.pos + size > len(self.data):
            raise ParseError("block read out of payload range")

    def u32(self) -> int:
        self._require(4)
        value = _u32(self.data, self.pos)
        self.pos += 4
        return value

    def u64(self) -> int:
        self._require(8)
        value = _u64(self.data, self.pos)
        self.pos += 8
        return value

    def string(self) -> str:
        length = self.u32()
        if length <= 0 or length > 256:
            raise ParseError(f"invalid string length {length} at {self.pos - 4}")
        start = self.pos
        end = start + length
        if end > len(self.data):
            raise ParseError("string out of payload range")

        raw = self.data[start:end]
        self.pos = end
        if raw.endswith(b"\x00"):
            raw = raw[:-1]
        return raw.decode("utf-8", "replace")

    def try_string(self) -> str | None:
        start = self.pos
        try:
            return self.string()
        except ParseError:
            self.pos = start
            return None


def parse_item_spec(value: str) -> tuple[str, int]:
    if "," not in value:
        return value, 1

    item_id, amount_text = value.rsplit(",", 1)
    try:
        amount = int(amount_text)
    except ValueError:
        return value, 1
    if amount <= 0:
        return item_id, 1
    return item_id, amount


def _roll_label_id(roll_points: int) -> str | None:
    return ROLL_POINT_LABEL_IDS.get(roll_points)


def _parse_monopoly_block(data: bytes, marker_pos: int, ctx: ParseContext) -> ParsedBlock:
    envelope = parse_protocol_envelope("monopoly", data, marker_pos, ctx.view)
    pos = marker_pos + len(MONOPOLY_MARKER)
    if pos < len(data) and data[pos] == 0:
        pos += 1

    _reserved = _u32(data, pos)
    declared_size = _u32(data, pos + 4)
    row_count = _u32(data, pos + 8)
    pos += 12

    if row_count > MAX_ROWS_PER_BLOCK:
        raise ParseError(f"row_count too large: {row_count}")

    reader = _Reader(data, pos)
    rows: list[ParsedRow] = []
    for index in range(row_count):
        row_start = reader.pos
        roll_points = reader.u32()
        item_spec = reader.string()
        _zero = reader.u32()
        secondary_count = reader.u32()
        secondary_item = reader.string()
        result_or_pool = reader.string()

        pool_start = reader.pos
        pool_id = reader.try_string()
        if pool_id and pool_id.startswith("CardPool_"):
            pass
        else:
            reader.pos = pool_start
            pool_id = result_or_pool if result_or_pool.startswith("CardPool_") else None

        ticks = reader.u64()
        item_id, count = parse_item_spec(item_spec)
        rows.append(
            ParsedRow(
                record_type="monopoly",
                ticks=ticks,
                time=dotnet_ticks_to_iso(ticks),
                pool_id=pool_id,
                item_id=item_id,
                count=count,
                roll_points=roll_points,
                roll_label_id=_roll_label_id(roll_points),
                secondary_item_id=secondary_item or None,
                secondary_count=secondary_count,
                source=_source_with_envelope(ctx, envelope, row_index=index, offset=row_start),
            )
        )

    return ParsedBlock("monopoly", marker_pos, declared_size, row_count, tuple(rows), envelope)


def _parse_fork_block(data: bytes, marker_pos: int, ctx: ParseContext) -> ParsedBlock:
    envelope = parse_protocol_envelope("fork", data, marker_pos, ctx.view)
    pos = marker_pos + len(FORK_MARKER)
    if pos < len(data) and data[pos] == 0:
        pos += 1

    _reserved = _u32(data, pos)
    declared_size = _u32(data, pos + 4)
    row_count = _u32(data, pos + 8)
    pos += 12

    if row_count > MAX_ROWS_PER_BLOCK:
        raise ParseError(f"row_count too large: {row_count}")

    reader = _Reader(data, pos)
    rows: list[ParsedRow] = []
    for index in range(row_count):
        row_start = reader.pos
        item_spec = reader.string()
        pool_id = reader.string()
        ticks = reader.u64()
        item_id, count = parse_item_spec(item_spec)
        rows.append(
            ParsedRow(
                record_type="fork",
                ticks=ticks,
                time=dotnet_ticks_to_iso(ticks),
                pool_id=pool_id,
                item_id=item_id,
                count=count,
                roll_points=None,
                roll_label_id=None,
                secondary_item_id=None,
                secondary_count=None,
                source=_source_with_envelope(ctx, envelope, row_index=index, offset=row_start),
            )
        )

    return ParsedBlock("fork", marker_pos, declared_size, row_count, tuple(rows), envelope)


RecordParser = Callable[[bytes, int, ParseContext], ParsedBlock]
RECORD_PARSERS: tuple[tuple[bytes, RecordParser], ...] = (
    (MONOPOLY_MARKER, _parse_monopoly_block),
    (FORK_MARKER, _parse_fork_block),
)


def parse_payload_blocks(
    payload: bytes,
    *,
    session: int,
    line: int,
    packet_index: int,
) -> tuple[list[ParsedBlock], list[ParseWarning]]:
    blocks: list[ParsedBlock] = []
    warnings: list[ParseWarning] = []

    for view_name, data in packet_views(payload):
        for marker, parser in RECORD_PARSERS:
            pos = 0
            while True:
                marker_pos = data.find(marker, pos)
                if marker_pos < 0:
                    break
                ctx = ParseContext(session=session, line=line, packet_index=packet_index, view=view_name)
                try:
                    blocks.append(parser(data, marker_pos, ctx))
                except ParseError as exc:
                    warnings.append(
                        ParseWarning(
                            code="parse_error",
                            message=f"{marker.decode('ascii', 'replace')}: {exc}",
                            session=session,
                            line=line,
                            packet_index=packet_index,
                            view=view_name,
                        )
                    )
                pos = marker_pos + len(marker)

    return blocks, warnings


def parse_packet_record(
    record: RawPacketRecord,
    *,
    session: int,
    line: int,
    packet_index: int,
) -> tuple[list[ParsedBlock], list[ParseWarning]]:
    try:
        payload = payload_from_packet(record)
    except ParseError as exc:
        return [], [ParseWarning("bad_packet", str(exc), session, line, packet_index)]
    return parse_payload_blocks(payload, session=session, line=line, packet_index=packet_index)
