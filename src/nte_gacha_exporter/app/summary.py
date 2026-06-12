from __future__ import annotations

from typing import Any

from nte_gacha_exporter.core.models import GachaRecord
from nte_gacha_exporter.core.schema import ExportDocument, LocalizationMap

CAPTURE_COUNT_KEYS = ("character", "standard", "fork")
CAPTURE_POOL_IDS = {
    "character": "CardPool_Character",
    "standard": "CardPool_NewRole",
}
FORK_LABEL_KEY = "UW_LotteryBase_BP_Hupanyanmu"


def capture_bucket(pool_id: str | None) -> str | None:
    if pool_id == CAPTURE_POOL_IDS["character"]:
        return "character"
    if pool_id == CAPTURE_POOL_IDS["standard"]:
        return "standard"
    if pool_id and pool_id.startswith("ForkLottery_"):
        return "fork"
    return None


def capture_count_labels(mapping: LocalizationMap) -> dict[str, str]:
    pools = mapping.get("pools", {})
    pool_meta = mapping.get("pool_meta", {})
    fork_label = ""
    if isinstance(pool_meta, dict):
        for pool_id in sorted(pool_meta):
            meta = pool_meta.get(pool_id)
            if isinstance(meta, dict) and pool_id.startswith("ForkLottery_"):
                fork_label = str(meta.get("group_label") or "")
                if fork_label:
                    break
    labels = mapping.get("labels", {})
    if not fork_label and isinstance(labels, dict):
        fork_label = str(labels.get(FORK_LABEL_KEY) or "")

    return {
        "character": str(pools.get(CAPTURE_POOL_IDS["character"], CAPTURE_POOL_IDS["character"])),
        "standard": str(pools.get(CAPTURE_POOL_IDS["standard"], CAPTURE_POOL_IDS["standard"])),
        "fork": fork_label or "ForkLottery",
    }


def empty_capture_counts() -> dict[str, int]:
    return dict.fromkeys(CAPTURE_COUNT_KEYS, 0)


def add_capture_counts(counts: dict[str, int], records: list[GachaRecord] | list[dict[str, Any]]) -> None:
    for record in records:
        pool_id = record.get("pool_id") if isinstance(record, dict) else record.pool_id
        bucket = capture_bucket(pool_id)
        if bucket:
            counts[bucket] += 1


def format_capture_counts(mapping: LocalizationMap, counts: dict[str, int]) -> str:
    labels = capture_count_labels(mapping)
    return " ".join(f"{labels[key]}={counts[key]}" for key in CAPTURE_COUNT_KEYS)


def offline_capture_counts(document: ExportDocument, mapping: LocalizationMap) -> str:
    counts = empty_capture_counts()
    add_capture_counts(counts, document.get("nte", {}).get("list", []))
    return format_capture_counts(mapping, counts)


def summary_text(document: ExportDocument, *, capture_counts: str | None = None) -> str:
    summary = document.get("_debug", {}).get("summary", {})
    parts = [f"records={summary.get('record_count', 0)}"]
    warning_count = int(summary.get("warning_count") or 0)
    if warning_count:
        parts.append(f"warnings={warning_count}")
    text = " ".join(parts)
    if capture_counts:
        return f"{text} {capture_counts}"
    return text


def record_line(record: GachaRecord | dict[str, Any]) -> str:
    if isinstance(record, dict):
        time_text = str(record.get("time") or "").replace("T", " ")[:19]
        record_type = record.get("record_type")
        pool_name = record.get("pool_name") or ""
        item_name = record.get("item_name") or ""
        count = record.get("count") or 0
        roll_label = record.get("roll_label") or ""
    else:
        time_text = (record.time or "").replace("T", " ")[:19]
        record_type = record.record_type
        pool_name = record.pool_name
        item_name = record.item_name
        count = record.count
        roll_label = record.roll_label

    if record_type == "fork":
        return f"{time_text} | {pool_name} | {item_name} x{count}"
    return f"{time_text} | {pool_name} | roll={roll_label} | {item_name} x{count}"
