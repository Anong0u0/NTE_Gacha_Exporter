from __future__ import annotations

import hashlib
from collections import Counter
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone, tzinfo
from typing import cast

from nte_gacha_exporter import __version__
from nte_gacha_exporter.core.models import GachaRecord, ParsedRow, ParseWarning
from nte_gacha_exporter.core.schema import (
    DebugDocument,
    ExportDocument,
    JsonValue,
    LocalizationMap,
    PoolMeta,
    PublicDocument,
)

SCHEMA_VERSION = "1.0"
POOL_WINDOW_TIMEZONE = timezone(timedelta(hours=8))


@dataclass(frozen=True)
class ExportOptions:
    locale: str
    source: str
    privacy: str = "sanitized"


def _lookup(mapping: LocalizationMap, section: str, key: str | None) -> str:
    if not key:
        return ""
    values = mapping.get(section, {})
    if isinstance(values, dict):
        return str(values.get(key, key))
    return key


def _canonical_item_id(mapping: LocalizationMap, item_id: str | None) -> str | None:
    if not item_id:
        return item_id
    aliases = mapping.get("item_aliases", {})
    if isinstance(aliases, dict):
        target = aliases.get(item_id)
        if isinstance(target, str) and target:
            return target
    return item_id


def _roll_text(row: ParsedRow, mapping: LocalizationMap) -> str:
    if row.roll_label_id:
        return _lookup(mapping, "labels", row.roll_label_id)
    if row.roll_points is None:
        return ""
    return str(row.roll_points)


def _public_time(value: str | None) -> str | None:
    if not value:
        return None
    return value.replace("T", " ")[:19]


def _parse_public_datetime(value: str | None) -> datetime | None:
    if not value:
        return None
    try:
        parsed = datetime.fromisoformat(value.replace(" ", "T"))
    except ValueError:
        return None
    return parsed.replace(tzinfo=None)


def _host_local_naive_from_tz8(value: str, local_tz: tzinfo | None = None) -> datetime | None:
    parsed = _parse_public_datetime(value)
    if parsed is None:
        return None

    aware = parsed.replace(tzinfo=POOL_WINDOW_TIMEZONE)
    local = aware.astimezone(local_tz) if local_tz is not None else aware.astimezone()
    return local.replace(tzinfo=None)


def _pool_meta_for_id(mapping: LocalizationMap, pool_id: str | None) -> PoolMeta:
    if not pool_id:
        return {}
    pool_meta = mapping.get("pool_meta", {})
    if not isinstance(pool_meta, dict):
        return {}
    meta = pool_meta.get(pool_id, {})
    return meta if isinstance(meta, dict) else {}


def _pool_title_from_windows(
    meta: PoolMeta,
    record_time: str | None,
    *,
    local_tz: tzinfo | None = None,
) -> str | None:
    record_dt = _parse_public_datetime(record_time)
    windows = meta.get("title_windows")
    if record_dt is None or not isinstance(windows, list):
        return None

    for window in windows:
        if not isinstance(window, dict):
            continue
        title = str(window.get("title") or "")
        end_at_tz8 = str(window.get("end_at_tz8") or "")
        end_dt = _host_local_naive_from_tz8(end_at_tz8, local_tz)
        if title and end_dt is not None and record_dt <= end_dt:
            return title
    return None


def _pool_name(
    mapping: LocalizationMap,
    pool_id: str | None,
    record_time: str | None,
    *,
    local_tz: tzinfo | None = None,
) -> str:
    meta = _pool_meta_for_id(mapping, pool_id)
    window_title = _pool_title_from_windows(meta, record_time, local_tz=local_tz)
    if window_title:
        return window_title

    title = str(meta.get("title") or "")
    if title:
        return title
    return _lookup(mapping, "pools", pool_id)


def _record_id(row: ParsedRow) -> str:
    parts = [
        row.record_type,
        str(row.ticks),
        row.pool_id or "",
        str(row.source.row_index),
        str(row.roll_points) if row.roll_points is not None else "",
        row.item_id,
        str(row.count),
        row.secondary_item_id or "",
        str(row.secondary_count) if row.secondary_count is not None else "",
    ]
    return hashlib.sha256("\x1f".join(parts).encode("utf-8")).hexdigest()


def _secondary_fields(row: ParsedRow, mapping: LocalizationMap) -> tuple[str | None, str | None, int | None]:
    if not row.secondary_item_id or row.secondary_item_id == row.item_id:
        return None, None, None
    return row.secondary_item_id, _lookup(mapping, "items", row.secondary_item_id), row.secondary_count


def _debug_raw_row(row: ParsedRow) -> dict[str, JsonValue]:
    return {
        "record_type": row.record_type,
        "ticks": row.ticks,
        "time": row.time,
        "pool_id": row.pool_id,
        "item_id": row.item_id,
        "count": row.count,
        "roll_points": row.roll_points,
        "roll_label_id": row.roll_label_id,
        "secondary_item_id": row.secondary_item_id,
        "secondary_count": row.secondary_count,
        "source": row.source.to_debug_dict(),
    }


def public_records(rows: list[ParsedRow], mapping: LocalizationMap) -> list[GachaRecord]:
    """Localize parsed rows and shape them into stable public records."""

    records: list[GachaRecord] = []
    for row in rows:
        item_id = _canonical_item_id(mapping, row.item_id) or row.item_id
        secondary_raw_id = _canonical_item_id(mapping, row.secondary_item_id)
        public_time = _public_time(row.time)
        canonical_row = ParsedRow(
            record_type=row.record_type,
            ticks=row.ticks,
            time=row.time,
            pool_id=row.pool_id,
            item_id=item_id,
            count=row.count,
            roll_points=row.roll_points,
            roll_label_id=row.roll_label_id,
            secondary_item_id=secondary_raw_id,
            secondary_count=row.secondary_count,
            source=row.source,
        )
        secondary_item_id, secondary_item_name, secondary_count = _secondary_fields(canonical_row, mapping)
        records.append(
            GachaRecord(
                record_id=_record_id(canonical_row),
                record_type=row.record_type,
                time=public_time,
                pool_id=row.pool_id,
                pool_name=_pool_name(mapping, row.pool_id, public_time),
                item_id=item_id,
                item_name=_lookup(mapping, "items", item_id),
                count=row.count,
                roll_points=row.roll_points,
                roll_label=_roll_text(row, mapping),
                secondary_item_id=secondary_item_id,
                secondary_item_name=secondary_item_name,
                secondary_count=secondary_count,
                source=row.source,
            )
        )
    return records


def unknown_warnings(records: list[GachaRecord]) -> list[ParseWarning]:
    warnings: list[ParseWarning] = []
    for record in records:
        if record.item_name == record.item_id:
            warnings.append(
                ParseWarning(
                    code="unmapped_item",
                    message=f"item id is not in localization map: {record.item_id}",
                    session=record.source.session,
                    line=record.source.line,
                    packet_index=record.source.packet_index,
                    view=record.source.view,
                )
            )
        if record.pool_id and record.pool_name == record.pool_id:
            warnings.append(
                ParseWarning(
                    code="unmapped_pool",
                    message=f"pool id is not in localization map: {record.pool_id}",
                    session=record.source.session,
                    line=record.source.line,
                    packet_index=record.source.packet_index,
                    view=record.source.view,
                )
            )
    return warnings


def build_document(
    rows: list[ParsedRow],
    mapping: LocalizationMap,
    options: ExportOptions,
    warnings: list[ParseWarning] | None = None,
) -> ExportDocument:
    """Build the public UIGF-like v1 sanitized JSON export document."""

    records = public_records(rows, mapping)
    all_warnings = [*(warnings or []), *unknown_warnings(records)]
    times = sorted(record.time for record in records if record.time)
    pool_counts = Counter(record.pool_id or "<unknown>" for record in records)
    type_counts = Counter(record.record_type for record in records)
    export_timestamp = int(datetime.now(timezone.utc).timestamp())

    return {
        "info": {
            "schema": "nte-gacha-export",
            "schema_version": SCHEMA_VERSION,
            "export_app": "nte-gacha-exporter",
            "export_app_version": __version__,
            "export_timestamp": export_timestamp,
            "locale": options.locale,
            "name_source": "localization_map",
            "time_source": "decoded_dotnet_ticks",
            "privacy": options.privacy,
        },
        "nte": {
            "list": [record.to_dict() for record in records],
        },
        "_meta": {
            "csv_headers": mapping.get("csv_headers", {}),
            "pool_meta": mapping.get("pool_meta", {}),
        },
        "_debug": {
            "source": options.source,
            "summary": {
                "record_count": len(records),
                "time_range": [times[0], times[-1]] if times else None,
                "by_record_type": dict(type_counts.most_common()),
                "by_pool": dict(pool_counts.most_common()),
                "warning_count": len(all_warnings),
            },
            "warnings": [cast(dict[str, JsonValue], warning.to_dict()) for warning in all_warnings],
            "records": [cast(dict[str, JsonValue], record.debug_dict()) for record in records],
            "raw_rows": [_debug_raw_row(row) for row in rows],
        },
    }


def public_document(document: ExportDocument) -> PublicDocument:
    return {
        "info": document["info"],
        "nte": document["nte"],
    }


def debug_document(document: ExportDocument) -> DebugDocument:
    return document.get("_debug", {})
