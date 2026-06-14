from __future__ import annotations

import json
from collections.abc import Callable
from pathlib import Path
from typing import Any

JsonObject = dict[str, Any]

QUALITY_RARITY = {
    "ITEM_QUALITY_WHITE": 1,
    "ITEM_QUALITY_GREEN": 2,
    "ITEM_QUALITY_BLUE": 3,
    "ITEM_QUALITY_PURPLE": 4,
    "ITEM_QUALITY_ORANGE": 5,
}
ITEM_TABLES = [
    ("item", "DataTable/Inventory/DT_ItemConfig.json"),
    ("fork", "DataTable/Fork/DT_ForkItemData.json"),
    ("character", "DataTable/Character/DT_Character.json"),
    ("appearance", "DataTable/Character/Appearance/DT_AppearanceData.json"),
]


def build_rules_map(
    assets_root: Path,
    *,
    items: dict[str, str],
    pools: dict[str, str],
    pool_meta: dict[str, dict[str, Any]],
    canonicalize_item_id: Callable[[str], str] | None = None,
) -> JsonObject:
    item_meta = _asset_item_meta(assets_root, items, canonicalize_item_id or _identity)
    return {
        "pool_rules": _pool_rule_rows(pools, pool_meta),
        "item_meta": _item_meta_rows(items, item_meta),
    }


def _identity(value: str) -> str:
    return value


def _asset_item_meta(
    assets_root: Path,
    items: dict[str, str],
    canonicalize_item_id: Callable[[str], str],
) -> JsonObject:
    meta: JsonObject = {}
    known_ids = {str(item_id) for item_id in items}

    for category, rel_path in ITEM_TABLES:
        table_path = assets_root / rel_path
        if not table_path.exists():
            continue
        for item_id, row in _rows_from_table(table_path).items():
            item_key = str(item_id)
            if item_key not in known_ids or not isinstance(row, dict):
                continue
            _merge_item_meta(meta, item_key, _row_item_meta(row, category))

    _add_vehicle_module_meta(meta, assets_root, known_ids, canonicalize_item_id)
    _add_lottery_table_meta(meta, assets_root, known_ids, canonicalize_item_id)
    return meta


def _pool_rule_rows(
    pools: dict[str, str],
    pool_meta: dict[str, dict[str, Any]],
) -> list[JsonObject]:
    pool_rules: list[JsonObject] = []
    for pool_id, pool_name in sorted(pools.items()):
        meta = pool_meta.get(pool_id, {})
        meta = meta if isinstance(meta, dict) else {}
        rule: JsonObject = {
            "pool_id": str(pool_id),
            "pool_name": str(pool_name),
            "group_label": str(meta.get("group_label") or pool_name),
        }
        pickup_item_ids = meta.get("pickup_item_ids")
        if isinstance(pickup_item_ids, list) and pickup_item_ids:
            rule["pickup_item_ids"] = [str(item_id) for item_id in pickup_item_ids]
        pool_rules.append(rule)
    return pool_rules


def _item_meta_rows(items: dict[str, str], item_meta: JsonObject) -> list[JsonObject]:
    rows: list[JsonObject] = []
    for item_id, item_name in sorted(items.items()):
        asset_item = item_meta.get(str(item_id), {})
        asset_item = asset_item if isinstance(asset_item, dict) else {}
        rarity = _int_or_none(asset_item.get("rarity"))
        if rarity is None:
            continue
        rows.append(
            {
                "item_id": str(item_id),
                "item_name": str(item_name),
                "rarity": rarity,
                "category": _str_or_none(asset_item.get("category")),
            }
        )
    return rows


def _rows_from_table(path: Path) -> JsonObject:
    data = json.loads(path.read_text(encoding="utf-8", errors="replace"))
    if isinstance(data, list) and data and isinstance(data[0], dict):
        rows = data[0].get("Rows")
        if isinstance(rows, dict):
            return rows
    if isinstance(data, dict):
        rows = data.get("Rows", data)
        if isinstance(rows, dict):
            return rows
    return {}


def _row_item_meta(row: JsonObject, category: str) -> JsonObject:
    rarity = _rarity_from_quality(row.get("ItemQuality") or row.get("Quality"))
    return {
        "category": category,
        "rarity": rarity,
    }


def _add_vehicle_module_meta(
    meta: JsonObject,
    assets_root: Path,
    known_ids: set[str],
    canonicalize_item_id: Callable[[str], str],
) -> None:
    module_path = assets_root / "DataTable/Vehicle/DT_vehicleModuleData.json"
    if not module_path.exists():
        return
    for row in _rows_from_table(module_path).values():
        if not isinstance(row, dict):
            continue
        active_data = row.get("FeatureActiveData")
        requires = active_data.get("Requires") if isinstance(active_data, dict) else None
        if not isinstance(requires, list):
            continue
        for requirement in requires:
            if not isinstance(requirement, dict):
                continue
            item_id = canonicalize_item_id(str(requirement.get("ID") or ""))
            if item_id in known_ids:
                _merge_item_meta(meta, item_id, {"category": "vehicle_module"})


def _add_lottery_table_meta(
    meta: JsonObject,
    assets_root: Path,
    known_ids: set[str],
    canonicalize_item_id: Callable[[str], str],
) -> None:
    for table_path in sorted((assets_root / "DataTable" / "Gacha").glob("DT_LotteryDataTable*.json")):
        for row in _rows_from_table(table_path).values():
            if not isinstance(row, dict):
                continue
            _add_lottery_items(meta, row.get("SSRItems"), known_ids, canonicalize_item_id, rarity=5)
            _add_lottery_items(meta, row.get("SRItems"), known_ids, canonicalize_item_id, rarity=4)
            _add_lottery_items(meta, row.get("RItems"), known_ids, canonicalize_item_id, rarity=3)


def _add_lottery_items(
    meta: JsonObject,
    values: Any,
    known_ids: set[str],
    canonicalize_item_id: Callable[[str], str],
    *,
    rarity: int,
) -> None:
    if not isinstance(values, list):
        return
    for value in values:
        if not isinstance(value, dict):
            continue
        item_id = canonicalize_item_id(str(value.get("ItemID") or ""))
        if item_id in known_ids:
            _merge_item_meta(meta, item_id, {"rarity": rarity})


def _merge_item_meta(meta: JsonObject, item_id: str, patch: JsonObject) -> None:
    existing = meta.setdefault(item_id, {})
    if not isinstance(existing, dict):
        existing = {}
        meta[item_id] = existing
    for key, value in patch.items():
        if value is not None:
            existing[key] = value


def _rarity_from_quality(value: Any) -> int | None:
    if not value:
        return None
    return QUALITY_RARITY.get(str(value).rsplit("::", 1)[-1])


def _int_or_none(value: Any) -> int | None:
    if isinstance(value, bool):
        return None
    if isinstance(value, int):
        return value
    if isinstance(value, str) and value.isdigit():
        return int(value)
    return None


def _str_or_none(value: Any) -> str | None:
    return value if isinstance(value, str) and value else None
