from __future__ import annotations

import csv
import json
from pathlib import Path
from typing import Any

from nte_gacha_exporter.core.schema import ExportDocument
from nte_gacha_exporter.export.document import debug_document, public_document
from nte_gacha_exporter.mapping.runtime import load_map

CSV_FIELDS = [
    "time",
    "pool_group",
    "pool_name",
    "item_name",
    "count",
    "roll_label",
    "secondary_item_name",
    "secondary_count",
]


def write_json(path: Path, document: ExportDocument) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(public_document(document), ensure_ascii=False, indent=2), encoding="utf-8")


def write_debug_json(path: Path, document: ExportDocument) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(debug_document(document), ensure_ascii=False, indent=2), encoding="utf-8")


def _cell(value: Any) -> Any:
    if value is None:
        return ""
    return value


def _meta_dict(document: ExportDocument, key: str) -> dict[str, Any]:
    meta = document.get("_meta", {})
    if not isinstance(meta, dict):
        return {}
    value = meta.get(key, {})
    return value if isinstance(value, dict) else {}


def _bundled_csv_headers(locale: str) -> dict[str, str]:
    try:
        headers = load_map(locale).get("csv_headers", {})
    except Exception:
        return {}
    if not isinstance(headers, dict):
        return {}
    return {str(key): str(value) for key, value in headers.items() if value}


def _csv_headers(document: ExportDocument) -> dict[str, str]:
    locale = str(document.get("info", {}).get("locale") or "")
    meta_headers = {str(key): str(value) for key, value in _meta_dict(document, "csv_headers").items() if value}
    bundled_headers = _bundled_csv_headers(locale)

    headers: dict[str, str] = {}
    for field in CSV_FIELDS:
        headers[field] = meta_headers.get(field) or bundled_headers.get(field) or field

    duplicate_labels = {label for label in headers.values() if list(headers.values()).count(label) > 1}
    for field, label in list(headers.items()):
        if label in duplicate_labels:
            headers[field] = field
    return headers


def _pool_meta_for_record(record: dict[str, Any], pool_meta: dict[str, Any]) -> dict[str, Any]:
    pool_id = record.get("pool_id")
    if not pool_id:
        return {}
    value = pool_meta.get(str(pool_id), {})
    return value if isinstance(value, dict) else {}


def _csv_row(record: dict[str, Any], pool_meta: dict[str, Any]) -> dict[str, Any]:
    meta = _pool_meta_for_record(record, pool_meta)
    pool_name = record.get("pool_name") or meta.get("title") or ""
    return {
        "time": record.get("time") or "",
        "pool_group": meta.get("group_label") or pool_name,
        "pool_name": pool_name,
        "item_name": record.get("item_name") or "",
        "count": _cell(record.get("count")),
        "roll_label": record.get("roll_label") or "",
        "secondary_item_name": record.get("secondary_item_name") or "",
        "secondary_count": _cell(record.get("secondary_count")),
    }


def write_csv(path: Path, document: ExportDocument) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    headers = _csv_headers(document)
    pool_meta = _meta_dict(document, "pool_meta")
    with path.open("w", encoding="utf-8", newline="") as fh:
        writer = csv.DictWriter(fh, fieldnames=[headers[field] for field in CSV_FIELDS])
        writer.writeheader()
        for record in document["nte"]["list"]:
            row = _csv_row(record, pool_meta)
            writer.writerow({headers[field]: row[field] for field in CSV_FIELDS})
