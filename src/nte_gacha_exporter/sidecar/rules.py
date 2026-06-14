from __future__ import annotations

from typing import Any

from nte_gacha_exporter.mapping.runtime import load_map

JsonObject = dict[str, Any]

POOL_RULE_KEYS = {"pool_id", "pool_name", "group_label"}
ITEM_META_KEYS = {"item_id", "item_name", "rarity", "category"}
ITEM_ALIAS_KEYS = {"alias_id", "item_id"}


def build_rules(locale: str) -> JsonObject:
    mapping = load_map(locale)
    pool_rules = _required_list(mapping, "pool_rules")
    item_meta = _required_list(mapping, "item_meta")
    item_aliases = _required_dict(mapping, "item_aliases")
    return {
        "pool_rules": [_validate_pool_rule(rule) for rule in pool_rules],
        "item_meta": [_validate_item_meta(item) for item in item_meta],
        "item_aliases": [_validate_item_alias(alias, item_id) for alias, item_id in sorted(item_aliases.items())],
    }


def _required_list(mapping: JsonObject, key: str) -> list[Any]:
    value = mapping.get(key)
    if not isinstance(value, list):
        raise ValueError(f"localization map missing required rules section: {key}")
    return value


def _required_dict(mapping: JsonObject, key: str) -> dict[str, Any]:
    value = mapping.get(key)
    if not isinstance(value, dict):
        raise ValueError(f"localization map missing required rules section: {key}")
    return value


def _validate_pool_rule(value: Any) -> JsonObject:
    rule = _object(value, "pool_rules entry")
    _require_keys(rule, POOL_RULE_KEYS, "pool_rules entry")
    result: JsonObject = {
        "pool_id": _required_str(rule, "pool_id"),
        "pool_name": _required_str(rule, "pool_name"),
        "group_label": _required_str(rule, "group_label"),
    }
    pickup_item_ids = _optional_str_list(rule, "pickup_item_ids")
    if pickup_item_ids:
        result["pickup_item_ids"] = pickup_item_ids
    return result


def _validate_item_meta(value: Any) -> JsonObject:
    item = _object(value, "item_meta entry")
    _require_keys(item, ITEM_META_KEYS, "item_meta entry")
    return {
        "item_id": _required_str(item, "item_id"),
        "item_name": _required_str(item, "item_name"),
        "rarity": _required_int(item, "rarity"),
        "category": _optional_str(item, "category"),
    }


def _validate_item_alias(alias_id: Any, item_id: Any) -> JsonObject:
    alias = {"alias_id": alias_id, "item_id": item_id}
    _require_keys(alias, ITEM_ALIAS_KEYS, "item_aliases entry")
    return {
        "alias_id": _required_str(alias, "alias_id"),
        "item_id": _required_str(alias, "item_id"),
    }


def _object(value: Any, name: str) -> JsonObject:
    if not isinstance(value, dict):
        raise ValueError(f"{name} must be an object")
    return value


def _require_keys(value: JsonObject, keys: set[str], name: str) -> None:
    missing = sorted(keys - set(value))
    if missing:
        raise ValueError(f"{name} missing required keys: {', '.join(missing)}")


def _required_str(value: JsonObject, key: str) -> str:
    field = value.get(key)
    if not isinstance(field, str) or not field:
        raise ValueError(f"field must be a non-empty string: {key}")
    return field


def _optional_str(value: JsonObject, key: str) -> str | None:
    field = value.get(key)
    if field is None:
        return None
    if not isinstance(field, str):
        raise ValueError(f"field must be a string or null: {key}")
    return field


def _optional_str_list(value: JsonObject, key: str) -> list[str] | None:
    field = value.get(key)
    if field is None:
        return None
    if not isinstance(field, list):
        raise ValueError(f"field must be a list of non-empty strings: {key}")
    result: list[str] = []
    for index, item in enumerate(field):
        if not isinstance(item, str) or not item:
            raise ValueError(f"field must be a list of non-empty strings: {key}[{index}]")
        result.append(item)
    return result


def _optional_int(value: JsonObject, key: str) -> int | None:
    field = value.get(key)
    if field is None:
        return None
    if isinstance(field, bool) or not isinstance(field, int):
        raise ValueError(f"field must be an integer or null: {key}")
    return field


def _required_int(value: JsonObject, key: str) -> int:
    field = _optional_int(value, key)
    if field is None:
        raise ValueError(f"field must be an integer: {key}")
    return field
