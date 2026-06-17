from __future__ import annotations

import json
import os
import re
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from nte_gacha_exporter.mapping.rules import build_rules_map
from nte_gacha_exporter.mapping.runtime import DEFAULT_LOCALE, MAP_SCHEMA_VERSION, _validate_map_source

TABLES: list[tuple[str, str]] = [
    ("inventory", "DataTable/Inventory/DT_ItemConfig.json"),
    ("capital", "DataTable/Inventory/DT_CapitalItemConfig.json"),
    ("fork", "DataTable/Fork/DT_ForkItemData.json"),
    ("character", "DataTable/Character/DT_Character.json"),
]
ITEM_TYPE_TABLE = "DataTable/Inventory/DT_ItemType.json"
APPEARANCE_TABLES: list[tuple[str, str]] = [
    ("appearance", "DataTable/Character/Appearance/DT_AppearanceData.json"),
]
VEHICLE_TABLES: list[tuple[str, str]] = [
    ("vehicle", "DataTable/Vehicle/DT_VehicleData.json"),
]
VEHICLE_MODULE_TABLES: list[tuple[str, str]] = [
    ("vehicle_module", "DataTable/Vehicle/DT_vehicleModuleData.json"),
]
POOL_TABLES: list[tuple[str, str]] = [
    ("fork_pool", "DataTable/Fork/DT_ForkLotteryPoolData.json"),
]
GACHA_ILLUSTRATE_TABLE = "DataTable/Gacha/GachaIllustrate.json"
DROP_GROUP_TABLE = "DataTable/Drop/Client/ClientDropGroupDataTable.json"
DROP_SEQUENCE_TABLE = "DataTable/Drop/DropSequenceDataTable.json"
POOL_LABEL_KEYS: dict[str, tuple[str, str]] = {
    "CardPool_NewRole": ("ST_Ui", "BPUI_LotteryDiceRecord_biaozhunqipan"),
    "CardPool_Character": ("ST_Ui", "BPUI_LotteryDiceRecord_xiandingqipan"),
}
FORK_GROUP_LABEL: tuple[str, str] = ("ST_Ui", "UW_LotteryBase_BP_Hupanyanmu")
POOL_GROUP_TYPE_HEADER_KEY: tuple[str, str] = ("ST_UI_C", "BPUI_CharacterEquipDevFilter_15")
MONOPOLY_TITLE_NAMESPACE = "ST_Ui"
MONOPOLY_TITLE_PREFIX = "Lottery_Kachimingcheng_"
MONOPOLY_DESCRIPTION_KEYS = (
    "LotteryDes_Jishishuoming_{tail}Des",
    "LotteryDes_JIshishuoming_{tail}Des",
)
STANDARD_MONOPOLY_TITLE_TAIL = "changzhu"
MONOPOLY_LIMITED_RULE_TEXT_KEY = "LotteryDes_XiandingJishiguize_Des"
MONOPOLY_STANDARD_RULE_TEXT_KEY = "LotteryDes_Changzhujishiguize_Des"
MONOPOLY_LOTTERY_TABLE = "DataTable/Gacha/DT_LotteryDataTable_Nanali.json"
FORK_POOL_TABLE = "DataTable/Fork/DT_ForkLotteryPoolData.json"
LABEL_KEYS: dict[str, tuple[str, str]] = {
    "Abyss_GamepadKeys_1": ("ST_Ui", "Abyss_GamepadKeys_1"),
    "AbyssClone_Award_02": ("ST_Ui", "AbyssClone_Award_02"),
    "BPUI_LotteryResult_jidianzengli": ("ST_Ui", "BPUI_LotteryResult_jidianzengli"),
    "BPUI_LotteryResult_chenmiandi": ("ST_Ui", "BPUI_LotteryResult_chenmiandi"),
    "BPUI_LotteryDiceRecord_biaozhunqipan": ("ST_Ui", "BPUI_LotteryDiceRecord_biaozhunqipan"),
    "BPUI_LotteryDiceRecord_qipanleixing": ("ST_Ui", "BPUI_LotteryDiceRecord_qipanleixing"),
    "BPUI_LotteryDiceRecord_xiandingqipan": ("ST_Ui", "BPUI_LotteryDiceRecord_xiandingqipan"),
    "BPUI_LotteryModuleEntrance_Title": ("ST_Ui", "BPUI_LotteryModuleEntrance_Title"),
    "TreasureBox_2": ("ST_Ui", "TreasureBox_2"),
    "UI_CloneSystemChallengeFailed_Retry": ("ST_Ui", "UI_CloneSystemChallengeFailed_Retry"),
    "UI_CloneSystemStaminaTips_Enter": ("ST_Ui", "UI_CloneSystemStaminaTips_Enter"),
    "UI_Lottery_GachaDetails_Zhitoujilu": ("ST_Ui", "UI_Lottery_GachaDetails_Zhitoujilu"),
    "UI_Lottery_GachaDetails_title": ("ST_Ui", "UI_Lottery_GachaDetails_title"),
    "UW_LotteryBase_BP_Hupanyanmu": ("ST_Ui", "UW_LotteryBase_BP_Hupanyanmu"),
    "Mall_8_name": ("ST_Ui", "Mall_8_name"),
    "W_Vehicle_Button_Choose": ("ST_Ui", "W_Vehicle_Button_Choose"),
    "W_HTButton_Next_Page": ("ST_Ui", "W_HTButton_Next_Page"),
    "common_3": ("ST_Ui", "common_3"),
    "ui_forkshop_03": ("ST_Ui", "ui_forkshop_03"),
    "ui_forkshop_07": ("ST_Ui", "ui_forkshop_07"),
    "ui_forkshop_10": ("ST_Ui", "ui_forkshop_10"),
    "ui_appearance_02": ("ST_Ui", "ui_appearance_02"),
}
CSV_HEADER_FIELDS = (
    "time",
    "pool_group",
    "pool_name",
    "item_name",
    "count",
    "roll_label",
    "secondary_item_name",
    "secondary_count",
)
CSV_HEADER_KEYS: dict[str, tuple[tuple[str, str], ...]] = {
    "time": (("ST_Ui", "BPUI_GashaponRecord_time"),),
    "pool_group": (("ST_Ui", "BPUI_LotteryDiceRecord_qipanleixing"),),
    "item_name": (("ST_Ui", "BPUI_LotteryDiceRecord_daojumingcheng"), ("ST_Ui", "BPUI_GashaponRecord_Name")),
    "count": (("ST_UI_hpy", "MangHe_09"), ("ST_UI_hpy", "MangHe_23"), ("ST_Ui", "BPUI_ConsumableUse_UseNumber")),
    "roll_label": (("ST_Ui", "BPUI_LotteryDiceRecord_touzhidianshu"),),
    "secondary_item_name": (("ST_Ui", "BPUI_LotteryResult_AdditionalReward"),),
}
CUSTOM_CSV_HEADERS: dict[str, dict[str, str]] = {
    "pool_name": {
        "de": "Pool",
        "en": "Pool",
        "es": "Banner",
        "fr": "Bannière",
        "ja": "ガチャ",
        "ko": "뽑기",
        "ru": "Баннер",
        "zh-CN": "卡池",
        "zh-Hans": "卡池",
        "zh-Hant": "卡池",
    },
}
ASSET_FALLBACK_LOCALE = "en"
ITEM_TYPE_KEYS: dict[str, tuple[str, str]] = {
    "inventory": ("ST_Common", "item_type_2"),
    "capital": ("ST_Common", "item_type_4"),
    "fork": ("ST_Common", "item_type_5"),
    "character": ("ST_Common", "item_type_3"),
    "vehicle_module": ("ST_Common", "item_type_10"),
}
DEFAULT_PREFIXES: dict[str, str] = {
    "appearance": "Appearance",
    "capital": "Currency",
    "character": "Character",
    "glide": "Glider",
    "inventory": "Item",
    "fork": "Arc",
    "vehicle": "Vehicle",
    "vehicle_module": "Mod Parts",
}
ITEM_ID_SOURCE_PRIORITY: dict[str, int] = {
    "character": 10,
    "fork": 10,
    "appearance": 10,
    "vehicle_module": 10,
    "vehicle": 20,
    "capital": 30,
    "inventory": 40,
    "st_item_fallback": 60,
}
RICH_TEXT_VALUE_RE = re.compile(r"<[^/>][^>]*>([^<]+)</>")
RICH_TEXT_TAG_RE = re.compile(r"</?[^>]+>")
QUOTED_POOL_TITLE_RE = re.compile(r"「([^」]+)」\s*(?:屬於|属于|は|은|는)")
ROMAN_POOL_TITLE_RE = re.compile(
    r"^\s*\d+[.)．]?\s*(?P<title>.+?)\s+(?:is|ist|est)\s+(?:a|an|ein|eine|un|une)\b",
    re.IGNORECASE,
)
TITLE_QUOTE_PAIRS = (("「", "」"), ("『", "』"), ("“", "”"), ('"', '"'), ("'", "'"))


@dataclass(frozen=True)
class _CuratedLimitedBanner:
    tail: str
    banner_id: str
    end_at_tz8: str
    rate_up_5: tuple[str, ...]
    version: str | None = None
    phase: str | None = None


CURATED_LIMITED_BANNERS: tuple[_CuratedLimitedBanner, ...] = (
    _CuratedLimitedBanner(
        "Nanali",
        "monopoly_limited_Nanali",
        "2026-05-13 05:59:00",
        ("1010",),
        phase="limited_2026_05_13",
    ),
    _CuratedLimitedBanner(
        "Xun",
        "monopoly_limited_Xun",
        "2026-06-03 05:59:00",
        ("1052",),
        phase="limited_2026_06_03",
    ),
    _CuratedLimitedBanner(
        "AnHunQu",
        "monopoly_limited_AnHunQu",
        "2026-06-24 05:59:00",
        ("1004",),
        phase="limited_2026_06_24",
    ),
    _CuratedLimitedBanner(
        "Kaesi",
        "monopoly_limited_Kaesi",
        "2026-07-08 05:59:00",
        ("1020",),
        phase="limited_2026_07_08",
    ),
)


@dataclass(frozen=True)
class _ItemBuildContext:
    localization: dict[str, Any]
    item_type_prefixes: dict[str, str]
    canonicalize_item_id: Callable[[str], str]
    required_item_ids: set[str]
    item_aliases: dict[str, str]


def candidate_roots() -> list[Path]:
    values = [os.environ.get("NTE_ASSETS_ROOT")]
    roots: list[Path] = []
    for value in values:
        if not value:
            continue
        path = Path(value).expanduser()
        if path not in roots:
            roots.append(path)
    return roots


def find_assets_root(explicit: str | None = None) -> Path:
    roots = [Path(explicit).expanduser()] if explicit else candidate_roots()
    if not roots:
        raise FileNotFoundError("NTE_Assets root not set. Pass --assets-root or set NTE_ASSETS_ROOT.")
    for root in roots:
        if (root / "DataTable").exists() and (root / "Localization").exists():
            return root
    checked = ", ".join(str(path) for path in roots)
    raise FileNotFoundError(f"NTE_Assets root not found. Checked: {checked}")


def _load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8", errors="replace"))


def _load_localization(assets_root: Path, locale: str) -> dict[str, Any]:
    localization: dict[str, Any] = {}

    fallbacks = [locale, ASSET_FALLBACK_LOCALE]
    for fallback in reversed(list(dict.fromkeys(fallbacks))):
        loc_path = assets_root / "Localization" / fallback / "game.json"
        if not loc_path.exists():
            continue
        loaded = _load_json(loc_path)
        if not isinstance(loaded, dict):
            continue
        for namespace, values in loaded.items():
            if isinstance(values, dict):
                existing = localization.get(namespace)
                if isinstance(existing, dict):
                    existing.update(values)
                else:
                    localization[namespace] = dict(values)
            else:
                localization[namespace] = values

    return localization


def _rows_from_datatable(path: Path) -> dict[str, Any]:
    data = _load_json(path)
    if isinstance(data, list) and data and isinstance(data[0], dict):
        rows = data[0].get("Rows")
        if isinstance(rows, dict):
            return rows
    if isinstance(data, dict):
        rows = data.get("Rows", data)
        if isinstance(rows, dict):
            return rows
    raise ValueError(f"cannot locate Rows in {path}")


def _namespace_from_table_id(table_id: str | None) -> str | None:
    if not table_id:
        return None
    tail = table_id.rsplit("/", 1)[-1]
    if "." in tail:
        return tail.rsplit(".", 1)[-1]
    return tail or None


def _unique_localized_key(localization: dict[str, Any], key: str) -> str | None:
    hits = [str(values[key]) for values in localization.values() if isinstance(values, dict) and key in values]
    unique_hits = set(hits)
    return hits[0] if len(unique_hits) == 1 else None


def _text_ref_fallback(text_ref: dict[str, Any]) -> str | None:
    for field in ("SourceString", "LocalizedString"):
        value = text_ref.get(field)
        if value:
            return str(value)
    return None


def _text_ref_key(text_ref: Any) -> str | None:
    if not isinstance(text_ref, dict):
        return None
    key = text_ref.get("Key")
    return str(key) if key else None


def _localized_text(text_ref: Any, localization: dict[str, Any]) -> str | None:
    if isinstance(text_ref, str):
        return text_ref
    if not isinstance(text_ref, dict):
        return None

    invariant = text_ref.get("CultureInvariantString")
    if invariant:
        return str(invariant)

    key = text_ref.get("Key")
    namespace = _namespace_from_table_id(text_ref.get("TableId"))
    if key and namespace:
        text = _localized_key(localization, namespace, str(key))
        if text is not None:
            return text

    if key:
        text = _localized_key(localization, "", str(key)) or _unique_localized_key(localization, str(key))
        if text is not None:
            return text

    return _text_ref_fallback(text_ref)


def _localized_key(localization: dict[str, Any], namespace: str, key: str) -> str | None:
    values = localization.get(namespace)
    if isinstance(values, dict) and key in values:
        return str(values[key])
    return None


def _clean_name(value: str | None) -> str | None:
    if value is None:
        return None
    text = value.strip()
    return text or None


def _item_type_key(value: Any) -> str | None:
    if not value:
        return None
    text = str(value)
    return text.rsplit("::", 1)[-1].casefold()


def _item_type_prefixes(assets_root: Path, localization: dict[str, Any]) -> dict[str, str]:
    table_path = assets_root / ITEM_TYPE_TABLE
    if not table_path.exists():
        return {}

    prefixes: dict[str, str] = {}
    for row_id, row in _rows_from_datatable(table_path).items():
        if not isinstance(row, dict):
            continue
        prefix = _clean_name(_localized_text(row.get("TypeName"), localization))
        item_type_key = _item_type_key(row_id)
        if prefix and item_type_key:
            prefixes[item_type_key] = prefix
    return prefixes


def _item_type_prefix(
    item_type: Any,
    item_type_prefixes: dict[str, str],
    *,
    fallback: str,
) -> str:
    item_type_key = _item_type_key(item_type)
    if item_type_key:
        prefix = item_type_prefixes.get(item_type_key)
        if prefix:
            return prefix
    return fallback


def _default_prefix(kind: str) -> str:
    return DEFAULT_PREFIXES[kind]


def _localized_prefix(table_kind: str, localization: dict[str, Any]) -> str:
    key = ITEM_TYPE_KEYS.get(table_kind)
    if key:
        return _localized_key(localization, *key) or _default_prefix(table_kind)
    return _default_prefix(table_kind)


def _appearance_prefix(row: dict[str, Any], localization: dict[str, Any]) -> str:
    if row.get("AppearanceType") == "EAppearanceType::Glide":
        return _localized_key(localization, "ST_Ui", "ui_appearance_02") or _default_prefix("glide")
    return _localized_key(localization, "ST_Common", "item_type_8") or _default_prefix("appearance")


def _vehicle_module_prefix(row: dict[str, Any], localization: dict[str, Any]) -> str:
    return _localized_prefix("vehicle_module", localization)


def _vehicle_prefix(localization: dict[str, Any]) -> str:
    return (
        _localized_key(localization, "ST_Ui", "DT_CharacterAbilityCityGroupUI_zaiju")
        or _localized_key(localization, "ST_TeachAndIllustrater", "Vehicle_name")
        or _default_prefix("vehicle")
    )


def _vehicle_module_item_ids(row: dict[str, Any]) -> list[str]:
    active_data = row.get("FeatureActiveData")
    if not isinstance(active_data, dict):
        return []

    requires = active_data.get("Requires")
    if not isinstance(requires, list):
        return []

    ids: list[str] = []
    for requirement in requires:
        if isinstance(requirement, dict) and requirement.get("ID"):
            ids.append(str(requirement["ID"]))
    return ids


def _add_item_priority(item_priorities: dict[str, int], item_id: str, kind: str) -> None:
    priority = ITEM_ID_SOURCE_PRIORITY[kind]
    existing = item_priorities.get(item_id)
    if existing is None or priority < existing:
        item_priorities[item_id] = priority


def _add_row_id_priorities(
    item_priorities: dict[str, int],
    assets_root: Path,
    tables: list[tuple[str, str]],
) -> None:
    for kind, rel_path in tables:
        table_path = assets_root / rel_path
        if table_path.exists():
            for row_id in _rows_from_datatable(table_path):
                _add_item_priority(item_priorities, str(row_id), kind)


def _add_vehicle_module_priorities(item_priorities: dict[str, int], assets_root: Path) -> None:
    for kind, rel_path in VEHICLE_MODULE_TABLES:
        table_path = assets_root / rel_path
        if not table_path.exists():
            continue
        for row in _rows_from_datatable(table_path).values():
            if isinstance(row, dict):
                for item_id in _vehicle_module_item_ids(row):
                    _add_item_priority(item_priorities, item_id, kind)


def _add_st_item_fallback_priorities(item_priorities: dict[str, int], localization: dict[str, Any]) -> None:
    item_text = localization.get("ST_Item")
    if not isinstance(item_text, dict):
        return

    for key in item_text:
        if key.endswith("_name"):
            _add_item_priority(item_priorities, str(key)[: -len("_name")], "st_item_fallback")


def _known_item_id_priorities(assets_root: Path, localization: dict[str, Any]) -> dict[str, int]:
    item_priorities: dict[str, int] = {}
    _add_row_id_priorities(item_priorities, assets_root, TABLES)
    _add_row_id_priorities(item_priorities, assets_root, VEHICLE_TABLES)
    _add_row_id_priorities(item_priorities, assets_root, APPEARANCE_TABLES)
    _add_vehicle_module_priorities(item_priorities, assets_root)
    _add_st_item_fallback_priorities(item_priorities, localization)
    return item_priorities


def _st_item_signature(localization: dict[str, Any], item_id: str) -> tuple[str, str] | None:
    item_text = localization.get("ST_Item")
    if not isinstance(item_text, dict):
        return None

    name = _clean_name(str(item_text.get(f"{item_id}_name") or ""))
    if not name:
        return None
    desc = _clean_name(str(item_text.get(f"{item_id}_desc") or ""))
    return (name.casefold(), (desc or "").casefold())


def _st_item_signature_aliases(
    item_priorities: dict[str, int],
    localization: dict[str, Any],
) -> dict[str, str]:
    fallback_priority = ITEM_ID_SOURCE_PRIORITY["st_item_fallback"]
    by_signature: dict[tuple[str, str], list[str]] = {}

    for item_id, priority in item_priorities.items():
        if priority >= fallback_priority:
            continue
        signature = _st_item_signature(localization, item_id)
        if signature:
            by_signature.setdefault(signature, []).append(item_id)

    aliases: dict[str, str] = {}
    for item_id, priority in item_priorities.items():
        if priority < fallback_priority:
            continue
        signature = _st_item_signature(localization, item_id)
        if not signature:
            continue
        candidates = sorted(set(by_signature.get(signature, [])))
        if len(candidates) == 1:
            aliases[item_id] = candidates[0]
    return aliases


def _item_id_canonicalizer(
    item_priorities: dict[str, int],
    localization: dict[str, Any] | None = None,
) -> Callable[[str], str]:
    by_folded: dict[str, list[str]] = {}
    for item_id in item_priorities:
        by_folded.setdefault(item_id.casefold(), []).append(item_id)
    signature_aliases = _st_item_signature_aliases(item_priorities, localization or {})

    def canonicalize(item_id: str) -> str:
        candidates = by_folded.get(item_id.casefold())
        if not candidates:
            return signature_aliases.get(item_id, item_id)
        best_priority = min(item_priorities[candidate] for candidate in candidates)
        best_candidates = sorted(candidate for candidate in candidates if item_priorities[candidate] == best_priority)
        if len(best_candidates) == 1:
            best = best_candidates[0]
            return signature_aliases.get(best, best)
        if item_id in best_candidates:
            return signature_aliases.get(item_id, item_id)
        return signature_aliases.get(item_id, item_id)

    return canonicalize


def _add_item_ref(
    refs: list[dict[str, str]],
    ref_id: Any,
    *,
    canonicalize: Callable[[str], str],
) -> None:
    if not ref_id or ref_id == "None":
        return
    raw_id = str(ref_id)
    item_id = canonicalize(raw_id)
    refs.append({"id": item_id, "raw_id": raw_id})


def _iter_item_ids(value: Any) -> list[str]:
    if isinstance(value, dict):
        item_ids: list[str] = []
        for key, child in value.items():
            if key == "ItemID":
                if isinstance(child, list):
                    item_ids.extend(_iter_item_ids(child))
                elif child and child != "None":
                    item_ids.append(str(child))
                continue
            item_ids.extend(_iter_item_ids(child))
        return item_ids
    if isinstance(value, list):
        item_ids = []
        for child in value:
            item_ids.extend(_iter_item_ids(child))
        return item_ids
    return []


def _add_item_refs_from_value(
    refs: list[dict[str, str]],
    value: Any,
    *,
    canonicalize: Callable[[str], str],
) -> None:
    for item_id in _iter_item_ids(value):
        _add_item_ref(refs, item_id, canonicalize=canonicalize)


def _dedupe_item_refs(refs: list[dict[str, str]]) -> list[dict[str, str]]:
    seen: set[tuple[tuple[str, str], ...]] = set()
    unique: list[dict[str, str]] = []
    for ref in refs:
        key = tuple(sorted(ref.items()))
        if key in seen:
            continue
        seen.add(key)
        unique.append(ref)
    return unique


def _matches_numbered_row(row_id: str, prefix: str) -> bool:
    if row_id == prefix:
        return True
    numbered_prefix = f"{prefix}_"
    if not row_id.startswith(numbered_prefix):
        return False
    return row_id[len(numbered_prefix) :].isdigit()


def _add_sequence_refs(
    refs: list[dict[str, str]],
    sequence_rows: dict[str, Any],
    sequence_id: str,
    *,
    canonicalize: Callable[[str], str],
) -> None:
    for row_id, row in sequence_rows.items():
        if not _matches_numbered_row(str(row_id), sequence_id) or not isinstance(row, dict):
            continue
        _add_item_ref(refs, row.get("ItemID"), canonicalize=canonicalize)


def _add_drop_group_refs(
    refs: list[dict[str, str]],
    drop_group_rows: dict[str, Any],
    sequence_rows: dict[str, Any],
    row_filter: Callable[[str], bool],
    canonicalize: Callable[[str], str],
) -> None:
    for row_id, row in drop_group_rows.items():
        row_key = str(row_id)
        if not row_filter(row_key) or not isinstance(row, dict):
            continue
        sequence_id = row.get("SequenceId")
        if sequence_id:
            _add_sequence_refs(refs, sequence_rows, str(sequence_id), canonicalize=canonicalize)


def _add_gacha_illustrate_refs(
    refs: list[dict[str, str]],
    assets_root: Path,
    canonicalize: Callable[[str], str],
) -> None:
    table_path = assets_root / GACHA_ILLUSTRATE_TABLE
    if table_path.exists():
        for row_id in _rows_from_datatable(table_path):
            _add_item_ref(refs, row_id, canonicalize=canonicalize)


def _add_lottery_table_refs(
    refs: list[dict[str, str]],
    assets_root: Path,
    canonicalize: Callable[[str], str],
) -> None:
    for lottery_path in sorted((assets_root / "DataTable" / "Gacha").glob("DT_LotteryDataTable*.json")):
        for row in _rows_from_datatable(lottery_path).values():
            _add_item_refs_from_value(refs, row, canonicalize=canonicalize)


def _add_fork_pool_refs(
    refs: list[dict[str, str]],
    assets_root: Path,
    canonicalize: Callable[[str], str],
) -> dict[str, Any]:
    fork_pool_path = assets_root / "DataTable/Fork/DT_ForkLotteryPoolData.json"
    if fork_pool_path.exists():
        fork_pool_rows = _rows_from_datatable(fork_pool_path)
        for row in fork_pool_rows.values():
            if not isinstance(row, dict):
                continue
            _add_item_refs_from_value(refs, row, canonicalize=canonicalize)
        return fork_pool_rows
    return {}


def _add_drop_table_refs(
    refs: list[dict[str, str]],
    assets_root: Path,
    fork_pool_rows: dict[str, Any],
    canonicalize: Callable[[str], str],
) -> None:
    drop_group_path = assets_root / DROP_GROUP_TABLE
    drop_sequence_path = assets_root / DROP_SEQUENCE_TABLE
    if not drop_group_path.exists() or not drop_sequence_path.exists():
        return

    drop_group_rows = _rows_from_datatable(drop_group_path)
    sequence_rows = _rows_from_datatable(drop_sequence_path)
    for row in fork_pool_rows.values():
        if not isinstance(row, dict) or not row.get("BaseDropID"):
            continue
        base_drop_id = str(row["BaseDropID"])
        _add_drop_group_refs(
            refs,
            drop_group_rows,
            sequence_rows,
            lambda row_key, base_drop_id=base_drop_id: _matches_numbered_row(row_key, base_drop_id),
            canonicalize,
        )

    _add_drop_group_refs(
        refs,
        drop_group_rows,
        sequence_rows,
        lambda row_key: row_key.startswith("drop_Monopoly_"),
        canonicalize,
    )


def _required_item_refs(assets_root: Path, canonicalize: Callable[[str], str]) -> list[dict[str, str]]:
    refs: list[dict[str, str]] = []
    _add_gacha_illustrate_refs(refs, assets_root, canonicalize)
    _add_lottery_table_refs(refs, assets_root, canonicalize)
    fork_pool_rows = _add_fork_pool_refs(refs, assets_root, canonicalize)
    _add_drop_table_refs(refs, assets_root, fork_pool_rows, canonicalize)

    return _dedupe_item_refs(refs)


def _inventory_prefix(
    row: dict[str, Any],
    localization: dict[str, Any],
    item_type_prefixes: dict[str, str],
) -> str:
    fallback = _localized_prefix("inventory", localization)
    return _item_type_prefix(row.get("ItemType"), item_type_prefixes, fallback=fallback)


def _add_pool(
    pools: dict[str, str],
    pool_id: str,
    name: str,
    *,
    overwrite: bool = True,
) -> None:
    if overwrite or pool_id not in pools:
        pools[pool_id] = name


def _source_evidence(
    confidence: str,
    tables: tuple[str, ...],
    *,
    notes: tuple[str, ...] = (),
) -> dict[str, Any]:
    evidence: dict[str, Any] = {
        "confidence": confidence,
        "tables": list(tables),
    }
    if notes:
        evidence["notes"] = list(notes)
    return evidence


def _asset_path(value: Any) -> str | None:
    if not isinstance(value, dict):
        return None
    path = value.get("AssetPathName")
    return path if isinstance(path, str) and path.startswith("/Game/") else None


def _pool_asset_refs(row: dict[str, Any]) -> dict[str, str]:
    refs: dict[str, str] = {}
    background = _asset_path(row.get("Bg"))
    if background:
        refs["background"] = background
    icon = _asset_path(row.get("Icon"))
    if icon:
        refs["icon"] = icon
    return refs


def _fork_pickup_item_ids(
    pool_id: str,
    row: dict[str, Any],
    canonicalize_item_id: Callable[[str], str],
) -> list[str]:
    raw_ids = row.get("UpList")
    if isinstance(raw_ids, list):
        pickup_item_ids = [canonicalize_item_id(str(item_id)) for item_id in raw_ids if item_id and item_id != "None"]
        if pickup_item_ids:
            return list(dict.fromkeys(pickup_item_ids))

    show_rewards = row.get("ShowRewards")
    if isinstance(show_rewards, list):
        pickup_item_ids = [
            canonicalize_item_id(str(reward.get("ItemID")))
            for reward in show_rewards
            if isinstance(reward, dict) and reward.get("IsUp") is True and reward.get("ItemID")
        ]
        if pickup_item_ids:
            return list(dict.fromkeys(pickup_item_ids))

    raise ValueError(f"fork pool missing pickup item ids from UpList or ShowRewards: {pool_id}")


def _fork_pool_meta(
    pool_id: str,
    row: dict[str, Any],
    localization: dict[str, Any],
    canonicalize_item_id: Callable[[str], str],
) -> dict[str, Any]:
    meta: dict[str, Any] = {}
    group_label = _clean_name(_localized_key(localization, *FORK_GROUP_LABEL))
    if group_label:
        meta["group_label"] = group_label

    title = _clean_name(_localized_text(row.get("ShowText1"), localization))
    if title:
        meta["title"] = title
    meta["pickup_item_ids"] = _fork_pickup_item_ids(pool_id, row, canonicalize_item_id)
    asset_refs = _pool_asset_refs(row)
    if asset_refs:
        meta["asset_refs"] = asset_refs
    return meta


def _strip_rich_text(value: str) -> str:
    return RICH_TEXT_TAG_RE.sub("", value).strip()


def _clean_pool_title(value: str | None) -> str | None:
    text = _clean_name(value)
    if not text:
        return None
    for left, right in TITLE_QUOTE_PAIRS:
        if text.startswith(left) and text.endswith(right):
            return _clean_name(text[len(left) : -len(right)])
    return text


def _description_pool_title(value: str | None) -> str | None:
    if not value:
        return None

    rich_title = RICH_TEXT_VALUE_RE.search(value)
    if rich_title:
        return _clean_pool_title(rich_title.group(1))

    clean_text = _strip_rich_text(value)
    quoted_title = QUOTED_POOL_TITLE_RE.search(clean_text)
    if quoted_title:
        return _clean_pool_title(quoted_title.group(1))

    first_line = clean_text.splitlines()[0] if clean_text.splitlines() else clean_text
    roman_title = ROMAN_POOL_TITLE_RE.search(first_line)
    if roman_title:
        return _clean_pool_title(roman_title.group("title"))
    return None


def _title_suffix_candidates(tail: str) -> list[str]:
    folded_tail = tail.casefold()
    parts = re.findall(r"[A-Z]?[a-z0-9]+|[A-Z]+(?=[A-Z]|$)", tail)
    candidates = [folded_tail]
    if parts:
        initials = "".join(part[:1] for part in parts).casefold()
        candidates.append(initials)

    unique: list[str] = []
    for candidate in candidates:
        if candidate and candidate not in unique:
            unique.append(candidate)
    return unique


def _localized_monopoly_pool_title(localization: dict[str, Any], tail: str) -> str | None:
    for template in MONOPOLY_DESCRIPTION_KEYS:
        title = _description_pool_title(
            _localized_key(localization, MONOPOLY_TITLE_NAMESPACE, template.format(tail=tail))
        )
        if title:
            return title

    for suffix in _title_suffix_candidates(tail):
        title = _clean_name(_localized_key(localization, MONOPOLY_TITLE_NAMESPACE, f"{MONOPOLY_TITLE_PREFIX}{suffix}"))
        if title:
            return title
    return None


def _monopoly_pool_meta(
    localization: dict[str, Any],
    pool_id: str,
) -> dict[str, Any]:
    meta: dict[str, Any] = {}
    group_label = _clean_name(_localized_key(localization, *POOL_LABEL_KEYS[pool_id]))
    if group_label:
        meta["group_label"] = group_label

    if pool_id == "CardPool_NewRole":
        title = _localized_monopoly_pool_title(localization, STANDARD_MONOPOLY_TITLE_TAIL)
        if title:
            meta["title"] = title
        return meta

    title_windows: list[dict[str, str]] = []
    for banner in CURATED_LIMITED_BANNERS:
        title = _localized_monopoly_pool_title(localization, banner.tail)
        if title:
            title_windows.append({"end_at_tz8": banner.end_at_tz8, "title": title})
    if title_windows:
        meta["title_windows"] = title_windows
    return meta


def _custom_csv_header(field: str, locale: str) -> str | None:
    values = CUSTOM_CSV_HEADERS.get(field)
    if not values:
        return None
    return values.get(locale)


def _csv_header_joiner(locale: str) -> str:
    return "" if locale in {"ja", "zh-CN", "zh-Hans", "zh-Hant"} else " "


def _csv_headers(localization: dict[str, Any], locale: str) -> dict[str, str]:
    headers: dict[str, str] = {}
    for field in CSV_HEADER_FIELDS:
        text = _custom_csv_header(field, locale)
        for namespace, key in CSV_HEADER_KEYS.get(field, ()):
            if text:
                break
            text = _clean_name(_localized_key(localization, namespace, key))
            if text:
                break
        headers[field] = text or field
    if headers["secondary_item_name"] != "secondary_item_name" and headers["count"] != "count":
        joiner = _csv_header_joiner(locale)
        headers["secondary_count"] = f"{headers['secondary_item_name']}{joiner}{headers['count']}"

    pool_header = _custom_csv_header("pool_name", locale)
    pool_type_header = _clean_name(_localized_key(localization, *POOL_GROUP_TYPE_HEADER_KEY))
    if pool_header and pool_type_header:
        joiner = _csv_header_joiner(locale)
        headers["pool_group"] = f"{pool_header}{joiner}{pool_type_header}"
    return headers


def _item_build_context(assets_root: Path, localization: dict[str, Any]) -> _ItemBuildContext:
    canonicalize_item_id = _item_id_canonicalizer(_known_item_id_priorities(assets_root, localization), localization)
    item_refs = _required_item_refs(assets_root, canonicalize_item_id)
    return _ItemBuildContext(
        localization=localization,
        item_type_prefixes=_item_type_prefixes(assets_root, localization),
        canonicalize_item_id=canonicalize_item_id,
        required_item_ids={ref["id"] for ref in item_refs},
        item_aliases={
            ref["raw_id"]: ref["id"] for ref in item_refs if ref.get("raw_id") and ref["raw_id"] != ref["id"]
        },
    )


def _add_required_item(items: dict[str, str], ctx: _ItemBuildContext, item_id: str, display: str) -> None:
    if item_id in ctx.required_item_ids:
        items[item_id] = display


def _add_table_items(items: dict[str, str], assets_root: Path, ctx: _ItemBuildContext) -> None:
    for kind, rel_path in TABLES:
        table_path = assets_root / rel_path
        if not table_path.exists():
            continue
        for item_id, row in _rows_from_datatable(table_path).items():
            item_key = str(item_id)
            if item_key not in ctx.required_item_ids or not isinstance(row, dict):
                continue
            name = _clean_name(_localized_text(row.get("ItemName"), ctx.localization))
            if not name:
                continue
            prefix = (
                _inventory_prefix(row, ctx.localization, ctx.item_type_prefixes)
                if kind == "inventory"
                else _localized_prefix(kind, ctx.localization)
            )
            _add_required_item(items, ctx, item_key, f"{prefix}·{name}")


def _add_vehicle_items(items: dict[str, str], assets_root: Path, ctx: _ItemBuildContext) -> None:
    for _, rel_path in VEHICLE_TABLES:
        table_path = assets_root / rel_path
        if not table_path.exists():
            continue
        prefix = _vehicle_prefix(ctx.localization)
        for item_id, row in _rows_from_datatable(table_path).items():
            item_key = str(item_id)
            if item_key not in ctx.required_item_ids or not isinstance(row, dict):
                continue
            name = _clean_name(_localized_text(row.get("Name"), ctx.localization))
            if name:
                _add_required_item(items, ctx, item_key, f"{prefix}·{name}")


def _add_appearance_items(items: dict[str, str], assets_root: Path, ctx: _ItemBuildContext) -> None:
    for _, rel_path in APPEARANCE_TABLES:
        table_path = assets_root / rel_path
        if not table_path.exists():
            continue
        for item_id, row in _rows_from_datatable(table_path).items():
            item_key = str(item_id)
            if item_key not in ctx.required_item_ids or not isinstance(row, dict):
                continue
            name = _clean_name(_localized_text(row.get("Name"), ctx.localization))
            if name:
                prefix = _appearance_prefix(row, ctx.localization)
                _add_required_item(items, ctx, item_key, f"{prefix}·{name}")


def _add_vehicle_module_items(items: dict[str, str], assets_root: Path, ctx: _ItemBuildContext) -> None:
    for _, rel_path in VEHICLE_MODULE_TABLES:
        table_path = assets_root / rel_path
        if not table_path.exists():
            continue
        for row in _rows_from_datatable(table_path).values():
            if not isinstance(row, dict):
                continue
            name = _clean_name(_localized_text(row.get("ModuleName"), ctx.localization))
            if not name:
                continue
            prefix = _vehicle_module_prefix(row, ctx.localization)
            for item_id in _vehicle_module_item_ids(row):
                _add_required_item(items, ctx, ctx.canonicalize_item_id(item_id), f"{prefix}·{name}")


def _add_fallback_items(items: dict[str, str], ctx: _ItemBuildContext) -> None:
    fallback_prefix = _localized_prefix("inventory", ctx.localization)
    for item_id in sorted(ctx.required_item_ids - set(items)):
        name = _clean_name(_localized_key(ctx.localization, "ST_Item", f"{item_id}_name"))
        if name:
            _add_required_item(items, ctx, item_id, f"{fallback_prefix}·{name}")


def _build_item_data(assets_root: Path, localization: dict[str, Any]) -> tuple[dict[str, str], _ItemBuildContext]:
    ctx = _item_build_context(assets_root, localization)
    items: dict[str, str] = {}
    _add_table_items(items, assets_root, ctx)
    _add_vehicle_items(items, assets_root, ctx)
    _add_appearance_items(items, assets_root, ctx)
    _add_vehicle_module_items(items, assets_root, ctx)
    _add_fallback_items(items, ctx)
    return items, ctx


def _add_monopoly_pools(
    pools: dict[str, str],
    pool_meta: dict[str, dict[str, Any]],
    localization: dict[str, Any],
) -> None:
    for pool_id, (namespace, key) in POOL_LABEL_KEYS.items():
        name = _clean_name(_localized_key(localization, namespace, key))
        if name:
            _add_pool(pools, pool_id, name)
        meta = _monopoly_pool_meta(localization, pool_id)
        if meta:
            pool_meta[pool_id] = meta


def _add_fork_pools(
    pools: dict[str, str],
    pool_meta: dict[str, dict[str, Any]],
    assets_root: Path,
    localization: dict[str, Any],
    canonicalize_item_id: Callable[[str], str],
) -> None:
    for _, rel_path in POOL_TABLES:
        table_path = assets_root / rel_path
        if not table_path.exists():
            continue
        for pool_id, row in _rows_from_datatable(table_path).items():
            if not isinstance(row, dict):
                continue
            pool_key = str(pool_id)
            name = _clean_name(_localized_text(row.get("Name"), localization))
            if name:
                _add_pool(pools, pool_key, name)
            if pool_key.startswith("ForkLottery_"):
                meta = _fork_pool_meta(pool_key, row, localization, canonicalize_item_id)
                if meta:
                    pool_meta[pool_key] = meta


def _build_pools(
    assets_root: Path,
    localization: dict[str, Any],
    canonicalize_item_id: Callable[[str], str],
) -> tuple[dict[str, str], dict[str, dict[str, Any]]]:
    pools: dict[str, str] = {}
    pool_meta: dict[str, dict[str, Any]] = {}
    _add_monopoly_pools(pools, pool_meta, localization)
    _add_fork_pools(pools, pool_meta, assets_root, localization, canonicalize_item_id)
    return pools, pool_meta


def _lottery_item_ids(
    assets_root: Path,
    key: str,
    canonicalize_item_id: Callable[[str], str],
    *,
    known_item_ids: set[str],
) -> list[str]:
    table_path = assets_root / MONOPOLY_LOTTERY_TABLE
    if not table_path.exists():
        return []

    item_ids: list[str] = []
    for row in _rows_from_datatable(table_path).values():
        if not isinstance(row, dict):
            continue
        values = row.get(key)
        if not isinstance(values, list):
            continue
        for value in values:
            if not isinstance(value, dict):
                continue
            raw_item_id = value.get("ItemID")
            if not raw_item_id or raw_item_id == "None":
                continue
            item_id = canonicalize_item_id(str(raw_item_id))
            if item_id in known_item_ids:
                item_ids.append(item_id)
    return list(dict.fromkeys(item_ids))


def _item_ref_list(
    item_ids: tuple[str, ...],
    canonicalize_item_id: Callable[[str], str],
    known_item_ids: set[str],
) -> list[str]:
    refs = [canonicalize_item_id(item_id) for item_id in item_ids]
    return [item_id for item_id in dict.fromkeys(refs) if item_id in known_item_ids]


def _item_asset_ref(items: dict[str, dict[str, Any]], item_id: str, key: str) -> str | None:
    item = items.get(item_id)
    if not isinstance(item, dict):
        return None
    refs = item.get("asset_refs")
    if not isinstance(refs, dict):
        return None
    value = refs.get(key)
    return value if isinstance(value, str) and value else None


def _featured_portraits(items: dict[str, dict[str, Any]], item_ids: list[str]) -> list[str]:
    refs = [_item_asset_ref(items, item_id, "portrait") for item_id in item_ids]
    return [ref for ref in refs if ref]


def _monopoly_rule_text_refs(*, limited: bool) -> dict[str, str]:
    key = MONOPOLY_LIMITED_RULE_TEXT_KEY if limited else MONOPOLY_STANDARD_RULE_TEXT_KEY
    return {"rule_desc_1": key}


def _build_gacha_rules(
    assets_root: Path,
    locale: str,
    canonicalize_item_id: Callable[[str], str],
) -> dict[str, dict[str, Any]]:
    rules: dict[str, dict[str, Any]] = {
        "monopoly_limited": {
            "rule_id": "monopoly_limited",
            "pool_kind": "monopoly_limited",
            "hard_pity_5": 90,
            "has_guarantee_5": False,
            "guarantee_scope": "unknown",
            "carry_scope": "pool_kind",
            "rule_text_refs": _monopoly_rule_text_refs(limited=True),
            "source": _source_evidence(
                "curated",
                (f"Localization/{locale}/game.json",),
                notes=("Numeric rule follows current desktop hard-pity behavior; rate-up precision is unknown.",),
            ),
        },
        "monopoly_standard": {
            "rule_id": "monopoly_standard",
            "pool_kind": "monopoly_standard",
            "hard_pity_5": 90,
            "has_guarantee_5": False,
            "guarantee_scope": "unknown",
            "carry_scope": "pool_kind",
            "rule_text_refs": _monopoly_rule_text_refs(limited=False),
            "source": _source_evidence(
                "curated",
                (f"Localization/{locale}/game.json",),
                notes=("Numeric rule follows current desktop hard-pity behavior; standard rate-up is not modeled.",),
            ),
        },
    }

    fork_rows = _fork_pool_rows(assets_root)
    if any(str(pool_id).startswith("ForkLottery_") and isinstance(row, dict) for pool_id, row in fork_rows.items()):
        hard_pity_5 = _fork_hard_pity_5(fork_rows)
        pickup_win_rate_5 = _fork_pickup_win_rate_5(assets_root, fork_rows, canonicalize_item_id)
        source_is_exact = hard_pity_5 is not None and pickup_win_rate_5 is not None
        rules["fork_lottery_s"] = {
            "rule_id": "fork_lottery_s",
            "pool_kind": "fork_lottery",
            "hard_pity_5": hard_pity_5 or 80,
            "pickup_win_rate_5": pickup_win_rate_5 or 25,
            "has_guarantee_5": True,
            "guarantee_scope": "pool_kind",
            "carry_scope": "pool_kind",
            "rule_text_refs": _fork_rule_text_refs(fork_rows),
            "source": _source_evidence(
                "exact" if source_is_exact else "curated",
                (FORK_POOL_TABLE, DROP_GROUP_TABLE, DROP_SEQUENCE_TABLE),
                notes=("Fallback numeric rule follows current desktop behavior when structured values are absent.",)
                if not source_is_exact
                else ("Fork S-class pickup rate is backed by gold drop sequence weights in the asset dump.",),
            ),
        }
    return dict(sorted(rules.items()))


def _fork_pool_rows(assets_root: Path) -> dict[str, Any]:
    table_path = assets_root / FORK_POOL_TABLE
    return _rows_from_datatable(table_path) if table_path.exists() else {}


def _fork_hard_pity_5(fork_rows: dict[str, Any]) -> int | None:
    values: set[int] = set()
    for pool_id, row in fork_rows.items():
        if not str(pool_id).startswith("ForkLottery_") or not isinstance(row, dict):
            continue
        value = row.get("UpGuaranteeCnt")
        if isinstance(value, bool):
            continue
        if isinstance(value, int):
            values.add(value)
    if len(values) == 1:
        return next(iter(values))
    return None


def _fork_pickup_win_rate_5(
    assets_root: Path,
    fork_rows: dict[str, Any],
    canonicalize_item_id: Callable[[str], str],
) -> int | None:
    drop_group_path = assets_root / DROP_GROUP_TABLE
    drop_sequence_path = assets_root / DROP_SEQUENCE_TABLE
    if not drop_group_path.exists() or not drop_sequence_path.exists():
        return None

    drop_group_rows = _rows_from_datatable(drop_group_path)
    sequence_rows = _rows_from_datatable(drop_sequence_path)
    rates: list[int] = []
    for pool_id, row in fork_rows.items():
        pool_key = str(pool_id)
        if not pool_key.startswith("ForkLottery_") or not isinstance(row, dict):
            continue
        base_drop_id = row.get("BaseDropID")
        if not base_drop_id:
            continue
        pickup_item_ids = set(_fork_pickup_item_ids(pool_key, row, canonicalize_item_id))
        for sequence_id in _fork_gold_sequence_ids(drop_group_rows, str(base_drop_id)):
            rate = _weighted_pickup_rate(sequence_rows, sequence_id, pickup_item_ids, canonicalize_item_id)
            if rate is not None:
                rates.append(rate)
    unique_rates = set(rates)
    if len(unique_rates) == 1:
        return next(iter(unique_rates))
    return None


def _fork_gold_sequence_ids(drop_group_rows: dict[str, Any], base_drop_id: str) -> list[str]:
    sequence_ids: list[str] = []
    for row_id, row in drop_group_rows.items():
        if not _matches_numbered_row(str(row_id), base_drop_id) or not isinstance(row, dict):
            continue
        sequence_id = row.get("SequenceId")
        if isinstance(sequence_id, str) and "_gold" in sequence_id:
            sequence_ids.append(sequence_id)
    return list(dict.fromkeys(sequence_ids))


def _weighted_pickup_rate(
    sequence_rows: dict[str, Any],
    sequence_id: str,
    pickup_item_ids: set[str],
    canonicalize_item_id: Callable[[str], str],
) -> int | None:
    total_weight = 0.0
    pickup_weight = 0.0
    for row_id, row in sequence_rows.items():
        if not _matches_numbered_row(str(row_id), sequence_id) or not isinstance(row, dict):
            continue
        weight = row.get("Weight")
        if isinstance(weight, bool) or not isinstance(weight, int | float):
            continue
        item_id = canonicalize_item_id(str(row.get("ItemID") or ""))
        total_weight += weight
        if item_id in pickup_item_ids:
            pickup_weight += weight
    if total_weight <= 0 or pickup_weight <= 0:
        return None
    return round(pickup_weight * 100 / total_weight)


def _fork_rule_text_refs(fork_rows: dict[str, Any]) -> dict[str, str]:
    for pool_id, row in sorted(fork_rows.items()):
        if not str(pool_id).startswith("ForkLottery_") or not isinstance(row, dict):
            continue
        refs: dict[str, str] = {}
        for source_key, target_key in (
            ("RuleDesc1", "rule_desc_1"),
            ("RuleDesc2", "rule_desc_2"),
            ("ProbDesc", "probability_desc"),
        ):
            key = _text_ref_key(row.get(source_key))
            if key:
                refs[target_key] = key
        if refs:
            return refs
    return {}


def _standard_banner(
    locale: str,
    localization: dict[str, Any],
    standard_5_pool: list[str],
    standard_4_pool: list[str],
) -> dict[str, Any] | None:
    title = _localized_monopoly_pool_title(localization, STANDARD_MONOPOLY_TITLE_TAIL)
    if not title:
        return None
    return {
        "banner_id": "monopoly_standard",
        "pool_id": "CardPool_NewRole",
        "pool_kind": "monopoly_standard",
        "banner_type": "standard",
        "title": title,
        "rate_up_5": [],
        "rate_up_4": [],
        "standard_5_pool": standard_5_pool,
        "standard_4_pool": standard_4_pool,
        "rule_id": "monopoly_standard",
        "source": _source_evidence(
            "curated",
            (MONOPOLY_LOTTERY_TABLE, f"Localization/{locale}/game.json"),
            notes=("Standard pool uses the available monopoly lottery table; banner instance is not explicit.",),
        ),
    }


def _limited_banners(
    locale: str,
    localization: dict[str, Any],
    canonicalize_item_id: Callable[[str], str],
    known_item_ids: set[str],
    normalized_items: dict[str, dict[str, Any]],
    standard_5_pool: list[str],
    standard_4_pool: list[str],
) -> dict[str, dict[str, Any]]:
    banners: dict[str, dict[str, Any]] = {}
    previous_end: str | None = None
    for banner in CURATED_LIMITED_BANNERS:
        title = _localized_monopoly_pool_title(localization, banner.tail)
        if not title:
            previous_end = banner.end_at_tz8
            continue
        rate_up_5 = _item_ref_list(banner.rate_up_5, canonicalize_item_id, known_item_ids)
        asset_refs: dict[str, Any] = {}
        featured_portraits = _featured_portraits(normalized_items, rate_up_5)
        if featured_portraits:
            asset_refs["featured_portraits"] = featured_portraits
        if len(rate_up_5) == 1:
            image = _item_asset_ref(normalized_items, rate_up_5[0], "banner")
            if image:
                asset_refs["image"] = image

        entry: dict[str, Any] = {
            "banner_id": banner.banner_id,
            "pool_id": "CardPool_Character",
            "pool_kind": "monopoly_limited",
            "banner_type": "limited",
            "title": title,
            "end_at": banner.end_at_tz8,
            "timezone": "Asia/Shanghai",
            "rate_up_5": rate_up_5,
            "rate_up_4": [],
            "standard_5_pool": standard_5_pool,
            "standard_4_pool": standard_4_pool,
            "rule_id": "monopoly_limited",
            "source": _source_evidence(
                "curated",
                (MONOPOLY_LOTTERY_TABLE, f"Localization/{locale}/game.json"),
                notes=(
                    "Schedule and rate-up are curated because no structured limited banner table was found.",
                    "Version/phase metadata is curated when present.",
                ),
            ),
        }
        if banner.version:
            entry["version"] = banner.version
        if banner.phase:
            entry["phase"] = banner.phase
        if previous_end:
            entry["start_at"] = previous_end
        if asset_refs:
            entry["asset_refs"] = asset_refs
        banners[banner.banner_id] = entry
        previous_end = banner.end_at_tz8
    return banners


def _fork_banners(
    assets_root: Path,
    localization: dict[str, Any],
    canonicalize_item_id: Callable[[str], str],
) -> dict[str, dict[str, Any]]:
    banners: dict[str, dict[str, Any]] = {}
    for pool_id, row in sorted(_fork_pool_rows(assets_root).items()):
        pool_key = str(pool_id)
        if not pool_key.startswith("ForkLottery_") or not isinstance(row, dict):
            continue
        title = _clean_name(_localized_text(row.get("ShowText1"), localization))
        if not title:
            continue
        banner: dict[str, Any] = {
            "banner_id": pool_key,
            "pool_id": pool_key,
            "pool_kind": "fork_lottery",
            "banner_type": "fork",
            "title": title,
            "rate_up_5": _fork_pickup_item_ids(pool_key, row, canonicalize_item_id),
            "rate_up_4": [],
            "rule_id": "fork_lottery_s",
            "source": _source_evidence("exact", (FORK_POOL_TABLE,)),
        }
        asset_refs = _pool_asset_refs(row)
        if asset_refs:
            banner["asset_refs"] = asset_refs
        currency_id = row.get("CurrencyID")
        if isinstance(currency_id, str) and currency_id:
            banner["currency_id"] = canonicalize_item_id(currency_id)
        for source_key, target_key in (("CurrencyCnt", "currency_count"), ("OnceLotteryCnt", "roll_unit")):
            value = row.get(source_key)
            if isinstance(value, int) and not isinstance(value, bool):
                banner[target_key] = value
        banners[pool_key] = banner
    return banners


def _build_banners(
    assets_root: Path,
    locale: str,
    localization: dict[str, Any],
    canonicalize_item_id: Callable[[str], str],
    normalized_items: dict[str, dict[str, Any]],
) -> dict[str, dict[str, Any]]:
    known_item_ids = set(normalized_items)
    standard_5_pool = _lottery_item_ids(
        assets_root,
        "SSRItems",
        canonicalize_item_id,
        known_item_ids=known_item_ids,
    )
    standard_4_pool = _lottery_item_ids(
        assets_root,
        "SRItems",
        canonicalize_item_id,
        known_item_ids=known_item_ids,
    )

    banners: dict[str, dict[str, Any]] = {}
    standard = _standard_banner(locale, localization, standard_5_pool, standard_4_pool)
    if standard:
        banners[standard["banner_id"]] = standard
    banners.update(
        _limited_banners(
            locale,
            localization,
            canonicalize_item_id,
            known_item_ids,
            normalized_items,
            standard_5_pool,
            standard_4_pool,
        )
    )
    banners.update(_fork_banners(assets_root, localization, canonicalize_item_id))
    return dict(sorted(banners.items()))


def _attach_banner_ids(pool_meta: dict[str, dict[str, Any]], banners: dict[str, dict[str, Any]]) -> None:
    banner_ids_by_pool: dict[str, list[str]] = {}
    for banner_id, banner in banners.items():
        pool_id = banner.get("pool_id")
        if isinstance(pool_id, str) and pool_id:
            banner_ids_by_pool.setdefault(pool_id, []).append(banner_id)
    for pool_id, banner_ids in banner_ids_by_pool.items():
        meta = pool_meta.setdefault(pool_id, {})
        meta["banner_ids"] = sorted(banner_ids)


def _build_labels(localization: dict[str, Any]) -> dict[str, str]:
    labels: dict[str, str] = {}
    for label_id, (namespace, key) in LABEL_KEYS.items():
        text = _clean_name(_localized_key(localization, namespace, key))
        if text:
            labels[label_id] = text
    return labels


def _normalized_items(items: dict[str, str], item_meta: list[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    meta_by_id = {str(item["item_id"]): item for item in item_meta if isinstance(item, dict) and item.get("item_id")}
    normalized: dict[str, dict[str, Any]] = {}
    for item_id, item_name in sorted(items.items()):
        meta = meta_by_id.get(str(item_id))
        if not meta:
            continue
        entry: dict[str, Any] = {
            "name": str(item_name),
            "rarity": meta["rarity"],
        }
        category = meta.get("category")
        if category is not None:
            entry["category"] = category
        for key in ("domain_type", "subtype", "color"):
            value = meta.get(key)
            if isinstance(value, str) and value:
                entry[key] = value
        asset_refs = meta.get("asset_refs")
        if isinstance(asset_refs, dict) and asset_refs:
            entry["asset_refs"] = dict(sorted(asset_refs.items()))
        normalized[str(item_id)] = entry
    return normalized


def _normalized_pools(
    pools: dict[str, str],
    pool_meta: dict[str, dict[str, Any]],
) -> dict[str, dict[str, Any]]:
    normalized: dict[str, dict[str, Any]] = {}
    for pool_id, pool_name in sorted(pools.items()):
        entry: dict[str, Any] = {"name": str(pool_name)}
        meta = pool_meta.get(pool_id, {})
        if isinstance(meta, dict):
            entry.update(meta)
        normalized[str(pool_id)] = entry
    return normalized


def build_map(
    assets_root: Path,
    locale: str = DEFAULT_LOCALE,
) -> dict[str, Any]:
    """Build a public display-name map from exported NTE assets."""

    localization = _load_localization(assets_root, locale)
    items, item_ctx = _build_item_data(assets_root, localization)
    pools, pool_meta = _build_pools(assets_root, localization, item_ctx.canonicalize_item_id)
    rules = build_rules_map(
        assets_root,
        items=items,
        pools=pools,
        pool_meta=pool_meta,
        canonicalize_item_id=item_ctx.canonicalize_item_id,
    )
    normalized_items = _normalized_items(items, rules["item_meta"])
    banners = _build_banners(assets_root, locale, localization, item_ctx.canonicalize_item_id, normalized_items)
    _attach_banner_ids(pool_meta, banners)
    map_data = {
        "schema_version": MAP_SCHEMA_VERSION,
        "csv_headers": dict(sorted(_csv_headers(localization, locale).items())),
        "items": normalized_items,
        "item_aliases": dict(sorted(item_ctx.item_aliases.items())),
        "pools": _normalized_pools(pools, pool_meta),
        "banners": banners,
        "gacha_rules": _build_gacha_rules(assets_root, locale, item_ctx.canonicalize_item_id),
        "labels": dict(sorted(_build_labels(localization).items())),
    }
    _validate_map_source(map_data, source=f"{locale}.json")
    return map_data
