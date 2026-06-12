from __future__ import annotations

from typing import Literal, TypeAlias, TypedDict

from nte_gacha_exporter.core.models import RecordType

JsonScalar: TypeAlias = str | int | float | bool | None
JsonValue: TypeAlias = JsonScalar | list["JsonValue"] | dict[str, "JsonValue"]


class CaptureStartRecord(TypedDict, total=False):
    type: Literal["capture_start"]
    schema_version: int
    pid: str
    iface: str
    ports: list[int]
    bpf: str


class CaptureStopRecord(TypedDict, total=False):
    type: Literal["capture_stop"]
    schema_version: int
    seen: int
    decoded_packets: int
    dropped: int


class RawPacketRecord(TypedDict, total=False):
    type: Literal["packet"]
    schema_version: int
    captured_at: float
    capture_index: int
    proto: str
    sport: int
    dport: int
    seq: int
    ack: int
    flags: int
    parser: str
    size: int
    payload_b64: str


RawCaptureRecord: TypeAlias = CaptureStartRecord | CaptureStopRecord | RawPacketRecord


class PoolTitleWindow(TypedDict, total=False):
    end_at_tz8: str
    title: str


class PoolMeta(TypedDict, total=False):
    group_label: str
    title: str
    subtitle: str
    title_windows: list[PoolTitleWindow]


class LocalizationMap(TypedDict, total=False):
    csv_headers: dict[str, str]
    items: dict[str, str]
    pools: dict[str, str]
    pool_meta: dict[str, PoolMeta]
    labels: dict[str, str]


class PublicRecordDict(TypedDict, total=False):
    record_id: str
    record_type: RecordType
    time: str
    pool_id: str
    pool_name: str
    item_id: str
    item_name: str
    count: int
    roll_points: int
    roll_label: str
    secondary_item_id: str
    secondary_item_name: str
    secondary_count: int


class ExportInfo(TypedDict, total=False):
    schema: str
    schema_version: str
    export_app: str
    export_app_version: str
    export_timestamp: int
    locale: str
    name_source: str
    time_source: str
    privacy: str


class ExportNteData(TypedDict, total=False):
    list: list[PublicRecordDict]


class ExportMeta(TypedDict, total=False):
    csv_headers: dict[str, str]
    pool_meta: dict[str, PoolMeta]


class DebugSummary(TypedDict, total=False):
    record_count: int
    time_range: list[str] | None
    by_record_type: dict[str, int]
    by_pool: dict[str, int]
    warning_count: int
    packets_seen: int
    decoded_packets: int
    dropped_packets: int


class DebugDocument(TypedDict, total=False):
    source: str
    summary: DebugSummary
    warnings: list[dict[str, JsonValue]]
    records: list[dict[str, JsonValue]]
    raw_rows: list[dict[str, JsonValue]]


class PublicDocument(TypedDict, total=False):
    info: ExportInfo
    nte: ExportNteData


class ExportDocument(PublicDocument, total=False):
    _meta: ExportMeta
    _debug: DebugDocument
