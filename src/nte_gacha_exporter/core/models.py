from __future__ import annotations

from dataclasses import asdict, dataclass
from typing import Any, Literal

RecordType = Literal["monopoly", "fork"]


@dataclass(frozen=True)
class SourceRef:
    """Non-private source coordinates for debugging sanitized exports."""

    session: int
    line: int
    packet_index: int
    view: str
    row_index: int
    offset: int
    stream_key: str | None = None
    page_index: int | None = None
    query_high: bool | None = None
    segment_index: int | None = None
    generation_index: int | None = None

    def to_debug_dict(self) -> dict[str, Any]:
        data = asdict(self)
        return {
            key: value for key, value in data.items() if key not in {"session", "line", "view"} and value is not None
        }


@dataclass(frozen=True)
class ProtocolEnvelope:
    record_type: RecordType
    stream_key: str
    page_index: int
    query_high: bool
    segment_index: int


@dataclass(frozen=True)
class ParsedRow:
    """Raw decoded history row before localization and export shaping."""

    record_type: RecordType
    ticks: int
    time: str | None
    pool_id: str | None
    item_id: str
    count: int
    roll_points: int | None
    roll_label_id: str | None
    secondary_item_id: str | None
    secondary_count: int | None
    source: SourceRef


@dataclass(frozen=True)
class ParsedBlock:
    record_type: RecordType
    marker_offset: int
    declared_size: int
    row_count: int
    rows: tuple[ParsedRow, ...]
    envelope: ProtocolEnvelope | None = None


@dataclass(frozen=True)
class ParseWarning:
    code: str
    message: str
    session: int | None = None
    line: int | None = None
    packet_index: int | None = None
    view: str | None = None

    def to_dict(self) -> dict[str, Any]:
        return {key: value for key, value in asdict(self).items() if value is not None}


@dataclass(frozen=True)
class GachaRecord:
    """Stable public export row."""

    record_id: str
    record_type: RecordType
    time: str | None
    pool_id: str | None
    pool_name: str
    item_id: str
    item_name: str
    count: int
    roll_points: int | None
    roll_label: str
    secondary_item_id: str | None
    secondary_item_name: str | None
    secondary_count: int | None
    source: SourceRef
    banner_id: str | None = None
    banner_name: str | None = None
    banner_type: str | None = None
    banner_version: str | None = None
    banner_phase: str | None = None
    banner_source_confidence: str | None = None
    banner_resolution_status: str | None = None

    def to_dict(self) -> dict[str, Any]:
        data = asdict(self)
        data.pop("source")
        return {key: value for key, value in data.items() if value not in (None, "")}

    def debug_dict(self) -> dict[str, Any]:
        data = asdict(self)
        data.pop("source")
        data = {key: value for key, value in data.items() if value not in (None, "")}
        data["source"] = self.source.to_debug_dict()
        return data
