from __future__ import annotations

from collections import OrderedDict
from collections.abc import Iterable
from dataclasses import dataclass, replace
from typing import TypeAlias

from nte_gacha_exporter.core.models import ParsedBlock, ParsedRow, ParseWarning, SourceRef

RowSignature: TypeAlias = tuple[str, int, str | None, str, int, int | None, str | None, str | None, int | None]
BlockSignature: TypeAlias = tuple[str, tuple[RowSignature, ...]]


def row_signature(row: ParsedRow) -> RowSignature:
    return (
        row.record_type,
        row.ticks,
        row.pool_id,
        row.item_id,
        row.count,
        row.roll_points,
        row.roll_label_id,
        row.secondary_item_id,
        row.secondary_count,
    )


def block_signature(record_type: str, rows: Iterable[ParsedRow]) -> BlockSignature:
    return record_type, tuple(row_signature(row) for row in rows)


@dataclass(frozen=True)
class _Segment:
    index: int
    rows: tuple[ParsedRow, ...]
    signature: BlockSignature


@dataclass
class _Generation:
    index: int
    segments: dict[int, _Segment]


@dataclass
class _StreamState:
    key: str
    generations: list[_Generation]

    @property
    def current(self) -> _Generation | None:
        return self.generations[-1] if self.generations else None

    def start_generation(self) -> _Generation:
        generation = _Generation(index=len(self.generations), segments={})
        self.generations.append(generation)
        return generation


def _rows_with_generation(generation: _Generation) -> list[ParsedRow]:
    rows: list[ParsedRow] = []
    for segment_index in sorted(generation.segments):
        segment = generation.segments[segment_index]
        for row in segment.rows:
            rows.append(replace(row, source=replace(row.source, generation_index=generation.index)))
    return rows


def _row_signatures(rows: list[ParsedRow]) -> list[RowSignature]:
    return [row_signature(row) for row in rows]


def _partial_snapshot_merge(new_rows: list[ParsedRow], old_rows: list[ParsedRow]) -> list[ParsedRow] | None:
    if not new_rows:
        return old_rows
    if not old_rows:
        return new_rows

    new_signatures = _row_signatures(new_rows)
    old_signatures = _row_signatures(old_rows)
    matches: list[tuple[int, int]] = []
    max_overlap = min(len(new_signatures), len(old_signatures))

    for overlap in range(max_overlap, 0, -1):
        suffix = new_signatures[-overlap:]
        for position in range(len(old_signatures) - overlap + 1):
            if old_signatures[position : position + overlap] == suffix:
                matches.append((overlap, position))
        if matches:
            break

    if len(matches) != 1:
        return None

    overlap, position = matches[0]
    return [*new_rows, *old_rows[position + overlap :]]


def _new_prefix_rows(before: list[ParsedRow], after: list[ParsedRow]) -> list[ParsedRow]:
    if not after:
        return []
    if not before:
        return after

    before_signatures = _row_signatures(before)
    after_signatures = _row_signatures(after)
    if before_signatures == after_signatures:
        return []

    matches: list[tuple[int, int]] = []
    for position in range(len(after_signatures)):
        overlap = min(len(before_signatures), len(after_signatures) - position)
        if overlap <= 0:
            continue
        if after_signatures[position : position + overlap] == before_signatures[:overlap]:
            matches.append((overlap, position))

    if not matches:
        return []

    best_overlap = max(overlap for overlap, _position in matches)
    best_positions = [position for overlap, position in matches if overlap == best_overlap]
    if len(best_positions) != 1:
        return []
    return after[: best_positions[0]]


class ProtocolAssembler:
    """Build export rows from protocol page segments instead of row-level dedupe."""

    def __init__(self) -> None:
        self._order: list[str] = []
        self._streams: OrderedDict[str, _StreamState] = OrderedDict()
        self._legacy_rows: list[ParsedRow] = []
        self._legacy_blocks: set[BlockSignature] = set()
        self.warnings: list[ParseWarning] = []
        self._warning_keys: set[tuple[str, str, int]] = set()

    def add_blocks(self, blocks: Iterable[ParsedBlock]) -> list[ParsedRow]:
        before = self.rows()
        for block in blocks:
            self.add_block(block)
        after = self.rows()
        return _new_prefix_rows(before, after)

    def add_block(self, block: ParsedBlock) -> None:
        if block.envelope is None:
            self._add_legacy_block(block)
            return

        stream = self._streams.get(block.envelope.stream_key)
        if stream is None:
            stream = _StreamState(block.envelope.stream_key, [])
            self._streams[block.envelope.stream_key] = stream
            self._order.append(block.envelope.stream_key)

        segment = _Segment(
            index=block.envelope.segment_index,
            rows=block.rows,
            signature=block_signature(block.record_type, block.rows),
        )
        generation = stream.current or stream.start_generation()
        existing = generation.segments.get(segment.index)
        if existing and existing.signature == segment.signature:
            return
        # A lower missing segment can arrive late; only a conflicting segment proves a new snapshot.
        if existing:
            generation = stream.start_generation()
        generation.segments[segment.index] = segment

    def rows(self) -> list[ParsedRow]:
        rows: list[ParsedRow] = []
        for key in self._order:
            if key == "__legacy__":
                rows.extend(self._legacy_rows)
                continue
            rows.extend(self._assemble_stream(self._streams[key]))
        return rows

    def _add_legacy_block(self, block: ParsedBlock) -> None:
        signature = block_signature(block.record_type, block.rows)
        if signature in self._legacy_blocks:
            return
        if not self._legacy_rows:
            self._order.append("__legacy__")
        self._legacy_blocks.add(signature)
        self._legacy_rows.extend(block.rows)

    def _assemble_stream(self, stream: _StreamState) -> list[ParsedRow]:
        result_rows: list[ParsedRow] = []
        result_max_segment: int | None = None

        for generation in stream.generations:
            if not generation.segments:
                continue
            generation_rows = _rows_with_generation(generation)
            segment_indexes = sorted(generation.segments)
            generation_min = segment_indexes[0]
            generation_max = segment_indexes[-1]

            if not result_rows:
                result_rows = generation_rows
                result_max_segment = generation_max
                continue

            if generation_min == 0:
                if result_max_segment is None or generation_max >= result_max_segment:
                    result_rows = generation_rows
                    result_max_segment = generation_max
                    continue

                merged = _partial_snapshot_merge(generation_rows, result_rows)
                if merged is None:
                    self._warn_generation(
                        "ambiguous_snapshot_merge",
                        f"{stream.key}: partial snapshot cannot be merged safely",
                        generation,
                    )
                    continue
                result_rows = merged
                continue

            if result_max_segment is not None and generation_min > result_max_segment:
                result_rows.extend(generation_rows)
                result_max_segment = generation_max
                continue

            self._warn_generation(
                "ambiguous_snapshot_merge",
                f"{stream.key}: non-zero snapshot reset cannot be merged safely",
                generation,
            )

        return result_rows

    def _warn_generation(self, code: str, message: str, generation: _Generation) -> None:
        first_row = self._first_generation_row(generation)
        if first_row is None:
            return
        key = (code, first_row.source.stream_key or "", generation.index)
        if key in self._warning_keys:
            return
        self._warning_keys.add(key)
        source: SourceRef = first_row.source
        self.warnings.append(
            ParseWarning(
                code=code,
                message=message,
                session=source.session,
                line=source.line,
                packet_index=source.packet_index,
                view=source.view,
            )
        )

    @staticmethod
    def _first_generation_row(generation: _Generation) -> ParsedRow | None:
        if not generation.segments:
            return None
        first_segment = generation.segments[min(generation.segments)]
        return first_segment.rows[0] if first_segment.rows else None


def assemble_blocks(blocks: Iterable[ParsedBlock]) -> tuple[list[ParsedRow], list[ParseWarning]]:
    assembler = ProtocolAssembler()
    assembler.add_blocks(blocks)
    return assembler.rows(), assembler.warnings
