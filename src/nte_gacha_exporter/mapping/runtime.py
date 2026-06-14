from __future__ import annotations

import json
from pathlib import Path
from typing import cast

from nte_gacha_exporter.core.schema import LocalizationMap, LocalizationMapSource, PoolMeta, SourcePool
from nte_gacha_exporter.resources.json_data import available_json, load_json

DEFAULT_LOCALE = "en"
MAP_PACKAGE = "nte_gacha_exporter.resources.maps"
MAP_SCHEMA_VERSION = 2
LEGACY_MAP_KEYS = {"item_meta", "pool_meta", "pool_rules"}


def available_locales() -> list[str]:
    """Return bundled localization map names."""

    return available_json(MAP_PACKAGE)


def _object(value: object, *, source: str, section: str) -> dict[str, object]:
    if not isinstance(value, dict):
        raise ValueError(f"localization map section must be an object: {source}:{section}")
    return cast(dict[str, object], value)


def _string_map(value: object, *, source: str, section: str) -> dict[str, str]:
    data = _object(value, source=source, section=section)
    result: dict[str, str] = {}
    for key, text in data.items():
        if not isinstance(key, str) or not key:
            raise ValueError(f"localization map key must be a non-empty string: {source}:{section}")
        if not isinstance(text, str) or not text:
            raise ValueError(f"localization map value must be a non-empty string: {source}:{section}.{key}")
        result[key] = text
    return result


def _optional_str(value: object, *, source: str, path: str) -> str | None:
    if value is None:
        return None
    if not isinstance(value, str):
        raise ValueError(f"localization map field must be a string or null: {source}:{path}")
    return value


def _required_str(value: object, *, source: str, path: str) -> str:
    if not isinstance(value, str) or not value:
        raise ValueError(f"localization map field must be a non-empty string: {source}:{path}")
    return value


def _required_int(value: object, *, source: str, path: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise ValueError(f"localization map field must be an integer: {source}:{path}")
    return value


def _title_windows(value: object, *, source: str, path: str) -> list[dict[str, str]]:
    if not isinstance(value, list):
        raise ValueError(f"localization map field must be a list: {source}:{path}")

    windows: list[dict[str, str]] = []
    for index, entry in enumerate(value):
        entry_path = f"{path}[{index}]"
        window = _object(entry, source=source, section=entry_path)
        windows.append(
            {
                "end_at_tz8": _required_str(window.get("end_at_tz8"), source=source, path=f"{entry_path}.end_at_tz8"),
                "title": _required_str(window.get("title"), source=source, path=f"{entry_path}.title"),
            }
        )
    return windows


def _string_list(value: object, *, source: str, path: str) -> list[str]:
    if not isinstance(value, list):
        raise ValueError(f"localization map field must be a list: {source}:{path}")
    result: list[str] = []
    for index, item in enumerate(value):
        if not isinstance(item, str) or not item:
            raise ValueError(f"localization map field must be a non-empty string: {source}:{path}[{index}]")
        result.append(item)
    return result


def _validate_items(value: object, *, source: str) -> dict[str, dict[str, object]]:
    items = _object(value, source=source, section="items")
    result: dict[str, dict[str, object]] = {}
    for item_id, item_value in items.items():
        if not isinstance(item_id, str) or not item_id:
            raise ValueError(f"localization map item id must be a non-empty string: {source}:items")
        item = _object(item_value, source=source, section=f"items.{item_id}")
        normalized: dict[str, object] = {
            "name": _required_str(item.get("name"), source=source, path=f"items.{item_id}.name"),
            "rarity": _required_int(item.get("rarity"), source=source, path=f"items.{item_id}.rarity"),
        }
        category = _optional_str(item.get("category"), source=source, path=f"items.{item_id}.category")
        if category is not None:
            normalized["category"] = category
        result[item_id] = normalized
    return result


def _validate_pools(value: object, *, source: str) -> dict[str, SourcePool]:
    pools = _object(value, source=source, section="pools")
    result: dict[str, SourcePool] = {}
    allowed_keys = {"name", "group_label", "title", "title_windows", "pickup_item_ids"}
    for pool_id, pool_value in pools.items():
        if not isinstance(pool_id, str) or not pool_id:
            raise ValueError(f"localization map pool id must be a non-empty string: {source}:pools")
        pool = _object(pool_value, source=source, section=f"pools.{pool_id}")
        unknown = sorted(set(pool) - allowed_keys)
        if unknown:
            raise ValueError(f"localization map pool has unknown keys: {source}:pools.{pool_id}: {', '.join(unknown)}")

        normalized: SourcePool = {
            "name": _required_str(pool.get("name"), source=source, path=f"pools.{pool_id}.name"),
        }
        for key in ("group_label", "title"):
            text = _optional_str(pool.get(key), source=source, path=f"pools.{pool_id}.{key}")
            if text:
                normalized[key] = text
        if "title_windows" in pool:
            normalized["title_windows"] = _title_windows(
                pool["title_windows"],
                source=source,
                path=f"pools.{pool_id}.title_windows",
            )
        if "pickup_item_ids" in pool:
            if not pool_id.startswith("ForkLottery_"):
                raise ValueError(
                    f"localization map pickup_item_ids only supports fork pools: {source}:pools.{pool_id}"
                )
            pickup_item_ids = _string_list(
                pool["pickup_item_ids"],
                source=source,
                path=f"pools.{pool_id}.pickup_item_ids",
            )
            if not pickup_item_ids:
                raise ValueError(f"localization map pickup_item_ids must not be empty: {source}:pools.{pool_id}")
            normalized["pickup_item_ids"] = pickup_item_ids
        result[pool_id] = normalized
    return result


def _validate_pool_item_refs(pools: dict[str, SourcePool], item_ids: set[str], *, source: str) -> None:
    unknown: list[str] = []
    for pool_id, pool in pools.items():
        for item_id in pool.get("pickup_item_ids", []):
            if item_id not in item_ids:
                unknown.append(f"{pool_id}.{item_id}")
    if unknown:
        raise ValueError(f"localization map pickup_item_ids target unknown item ids: {source}: {', '.join(unknown)}")


def _validate_map_source(data: object, *, source: str) -> LocalizationMapSource:
    if not isinstance(data, dict):
        raise ValueError(f"localization map must be an object: {source}")

    legacy_keys = sorted(LEGACY_MAP_KEYS & set(data))
    if legacy_keys:
        raise ValueError(f"localization map uses legacy v1 sections: {source}: {', '.join(legacy_keys)}")

    if data.get("schema_version") != MAP_SCHEMA_VERSION:
        raise ValueError(f"localization map schema_version must be {MAP_SCHEMA_VERSION}: {source}")

    items = _validate_items(data.get("items", {}), source=source)
    aliases = _string_map(data.get("item_aliases", {}), source=source, section="item_aliases")
    unknown_alias_targets = sorted(set(aliases.values()) - set(items))
    if unknown_alias_targets:
        raise ValueError(
            f"localization map item_aliases target unknown item ids: {source}: {', '.join(unknown_alias_targets)}"
        )

    pools = _validate_pools(data.get("pools", {}), source=source)
    _validate_pool_item_refs(pools, set(items), source=source)

    source_map: LocalizationMapSource = {
        "schema_version": MAP_SCHEMA_VERSION,
        "csv_headers": _string_map(data.get("csv_headers", {}), source=source, section="csv_headers"),
        "items": cast(dict[str, object], dict(sorted(items.items()))),
        "item_aliases": dict(sorted(aliases.items())),
        "pools": pools,
        "labels": _string_map(data.get("labels", {}), source=source, section="labels"),
    }
    return source_map


def expand_map_source(source_map: LocalizationMapSource) -> LocalizationMap:
    """Expand normalized v2 map data into runtime lookup/rule sections."""

    source_items = cast(dict[str, dict[str, object]], source_map.get("items", {}))
    source_pools = cast(dict[str, SourcePool], source_map.get("pools", {}))

    items = {item_id: str(item["name"]) for item_id, item in sorted(source_items.items())}
    pools = {pool_id: str(pool["name"]) for pool_id, pool in sorted(source_pools.items())}
    pool_meta: dict[str, PoolMeta] = {}
    pool_rules = []
    item_meta = []

    for pool_id, pool in sorted(source_pools.items()):
        meta = cast(PoolMeta, {key: value for key, value in pool.items() if key != "name"})
        if meta:
            pool_meta[pool_id] = meta
        rule = {
            "pool_id": pool_id,
            "pool_name": str(pool["name"]),
            "group_label": str(pool.get("group_label") or pool["name"]),
        }
        pickup_item_ids = pool.get("pickup_item_ids")
        if pickup_item_ids:
            rule["pickup_item_ids"] = pickup_item_ids
        pool_rules.append(rule)

    for item_id, item in sorted(source_items.items()):
        item_meta.append(
            {
                "item_id": item_id,
                "item_name": str(item["name"]),
                "rarity": cast(int, item["rarity"]),
                "category": cast(str | None, item.get("category")),
            }
        )

    return {
        "csv_headers": source_map.get("csv_headers", {}),
        "items": items,
        "item_aliases": source_map.get("item_aliases", {}),
        "pools": pools,
        "pool_meta": pool_meta,
        "labels": source_map.get("labels", {}),
        "pool_rules": pool_rules,
        "item_meta": item_meta,
    }


def _validate_map(data: object, *, source: str) -> LocalizationMap:
    return expand_map_source(_validate_map_source(data, source=source))


def load_map(locale: str = DEFAULT_LOCALE) -> LocalizationMap:
    """Load a bundled localization map."""

    try:
        data = load_json(MAP_PACKAGE, locale)
    except FileNotFoundError as exc:
        choices = ", ".join(available_locales()) or "<none>"
        raise FileNotFoundError(f"locale not found: {locale}; available: {choices}") from exc
    return _validate_map(data, source=f"{locale}.json")


def load_map_file(path: str | Path) -> LocalizationMap:
    """Load a user-provided localization map file."""

    map_path = Path(path)
    return _validate_map(json.loads(map_path.read_text(encoding="utf-8")), source=str(map_path))


def load_locale_map(locale_spec: str = DEFAULT_LOCALE) -> tuple[str, LocalizationMap]:
    """Load a bundled locale or a user-provided map from 'locale=path.json'."""

    if "=" not in locale_spec:
        return locale_spec, load_map(locale_spec)

    locale, path = locale_spec.split("=", 1)
    if not locale or not path:
        raise ValueError("custom locale map must use locale=path.json")
    return locale, load_map_file(path)
