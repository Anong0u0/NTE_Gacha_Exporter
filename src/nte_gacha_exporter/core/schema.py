from __future__ import annotations

from typing import Literal, TypeAlias, TypedDict

from nte_gacha_exporter.core.models import RecordType

JsonScalar: TypeAlias = str | int | float | bool | None
JsonValue: TypeAlias = JsonScalar | list["JsonValue"] | dict[str, "JsonValue"]
BannerResolutionStatus: TypeAlias = Literal[
    "matched", "unknown_pool", "unknown_time", "outside_known_windows", "ambiguous"
]


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


class SourceEvidence(TypedDict, total=False):
    confidence: Literal["exact", "inferred", "curated", "unknown"]
    tables: list[str]
    notes: list[str]


class ItemAssetRefs(TypedDict, total=False):
    icon: str
    portrait: str
    banner: str
    material: str
    head_icon: str


class BannerAssetRefs(TypedDict, total=False):
    icon: str
    image: str
    background: str
    featured_portraits: list[str]
    material: str


class RuleTextRefs(TypedDict, total=False):
    rule_desc_1: str
    rule_desc_2: str
    probability_desc: str


class PoolMeta(TypedDict, total=False):
    group_label: str
    title: str
    title_windows: list[PoolTitleWindow]
    pickup_item_ids: list[str]
    banner_ids: list[str]
    asset_refs: BannerAssetRefs


class PoolRule(TypedDict, total=False):
    pool_id: str
    pool_name: str
    group_label: str
    pickup_item_ids: list[str]


class ItemMeta(TypedDict, total=False):
    item_id: str
    item_name: str
    rarity: int | None
    category: str | None
    domain_type: str | None
    subtype: str | None
    asset_refs: ItemAssetRefs
    color: str | None
    source: SourceEvidence


class ItemAlias(TypedDict, total=False):
    alias_id: str
    item_id: str


class SourceItem(TypedDict, total=False):
    name: str
    rarity: int
    category: str | None
    domain_type: Literal["character", "fork", "appearance", "vehicle", "vehicle_module", "currency", "item"]
    subtype: str
    asset_refs: ItemAssetRefs
    color: str
    source: SourceEvidence


class SourcePool(TypedDict, total=False):
    name: str
    group_label: str
    title: str
    title_windows: list[PoolTitleWindow]
    pickup_item_ids: list[str]
    banner_ids: list[str]
    asset_refs: BannerAssetRefs


class SourceBanner(TypedDict, total=False):
    banner_id: str
    pool_id: str
    pool_kind: Literal["monopoly_limited", "monopoly_standard", "fork_lottery"]
    banner_type: Literal["limited", "standard", "fork"]
    title: str
    short_title: str
    version: str
    phase: str
    start_at: str
    end_at: str
    timezone: Literal["Asia/Shanghai"]
    rate_up_5: list[str]
    rate_up_4: list[str]
    standard_5_pool: list[str]
    standard_4_pool: list[str]
    rule_id: str
    asset_refs: BannerAssetRefs
    color: str
    currency_id: str
    currency_count: int
    roll_unit: int
    source: SourceEvidence


class ResolvedBanner(TypedDict, total=False):
    status: BannerResolutionStatus
    reason: str
    banner_id: str
    pool_id: str
    pool_kind: str
    banner_type: str
    title: str
    version: str
    phase: str
    start_at: str
    end_at: str
    timezone: str
    rate_up_5: list[str]
    rate_up_4: list[str]
    rule_id: str
    asset_refs: BannerAssetRefs
    source_confidence: str


class SourceGachaRule(TypedDict, total=False):
    rule_id: str
    pool_kind: str
    hard_pity_5: int
    hard_pity_4: int
    pickup_win_rate_5: int
    pickup_win_rate_4: int
    has_guarantee_5: bool
    has_guarantee_4: bool
    guarantee_scope: Literal["pool_kind", "banner", "unknown"]
    carry_scope: Literal["pool_kind", "banner", "unknown"]
    rule_text_refs: RuleTextRefs
    source: SourceEvidence


class LocalizationMapSource(TypedDict, total=False):
    schema_version: int
    csv_headers: dict[str, str]
    items: dict[str, SourceItem]
    item_aliases: dict[str, str]
    pools: dict[str, SourcePool]
    banners: dict[str, SourceBanner]
    gacha_rules: dict[str, SourceGachaRule]
    labels: dict[str, str]


class LocalizationMap(TypedDict, total=False):
    csv_headers: dict[str, str]
    items: dict[str, str]
    item_aliases: dict[str, str]
    pools: dict[str, str]
    pool_meta: dict[str, PoolMeta]
    labels: dict[str, str]
    pool_rules: list[PoolRule]
    item_meta: list[ItemMeta]
    banners: dict[str, SourceBanner]
    gacha_rules: dict[str, SourceGachaRule]


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
    banner_id: str
    banner_name: str
    banner_type: str
    banner_version: str
    banner_phase: str
    banner_source_confidence: str
    banner_resolution_status: BannerResolutionStatus
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
