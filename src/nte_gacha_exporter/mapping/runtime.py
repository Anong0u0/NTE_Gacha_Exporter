from __future__ import annotations

import json
import re
from pathlib import Path
from typing import cast

from nte_gacha_exporter.core.schema import (
    BannerAssetRefs,
    ItemAssetRefs,
    LocalizationMap,
    LocalizationMapSource,
    PoolMeta,
    RuleTextRefs,
    SourceBanner,
    SourceEvidence,
    SourceGachaRule,
    SourcePool,
)
from nte_gacha_exporter.mapping.banner_catalog import normalize_game_time
from nte_gacha_exporter.resources.json_data import available_json, load_json

DEFAULT_LOCALE = "en"
MAP_PACKAGE = "nte_gacha_exporter.resources.maps"
MAP_SCHEMA_VERSION = 4
LEGACY_MAP_KEYS = {"item_meta", "pool_meta", "pool_rules"}
SOURCE_CONFIDENCE_VALUES = {"exact", "inferred", "curated", "unknown"}
POOL_KIND_VALUES = {"monopoly_limited", "monopoly_standard", "fork_lottery"}
BANNER_TYPE_VALUES = {"limited", "standard", "fork"}
SCOPE_VALUES = {"pool_kind", "banner", "unknown"}
MACHINE_ID_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$")


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


def _optional_machine_id(value: object, *, source: str, path: str) -> str | None:
    text = _optional_str(value, source=source, path=path)
    if text is None:
        return None
    if not MACHINE_ID_RE.fullmatch(text):
        raise ValueError(f"localization map field must be a stable machine id: {source}:{path}")
    return text


def _required_str(value: object, *, source: str, path: str) -> str:
    if not isinstance(value, str) or not value:
        raise ValueError(f"localization map field must be a non-empty string: {source}:{path}")
    return value


def _required_int(value: object, *, source: str, path: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise ValueError(f"localization map field must be an integer: {source}:{path}")
    return value


def _optional_int(value: object, *, source: str, path: str) -> int | None:
    if value is None:
        return None
    return _required_int(value, source=source, path=path)


def _optional_bool(value: object, *, source: str, path: str) -> bool | None:
    if value is None:
        return None
    if not isinstance(value, bool):
        raise ValueError(f"localization map field must be a boolean or null: {source}:{path}")
    return value


def _required_value(value: object, allowed: set[str], *, source: str, path: str) -> str:
    text = _required_str(value, source=source, path=path)
    if text not in allowed:
        choices = ", ".join(sorted(allowed))
        raise ValueError(f"localization map field must be one of {choices}: {source}:{path}")
    return text


def _optional_value(value: object, allowed: set[str], *, source: str, path: str) -> str | None:
    if value is None:
        return None
    return _required_value(value, allowed, source=source, path=path)


def _asset_ref_map(value: object, *, source: str, path: str) -> dict[str, object]:
    refs = _object(value, source=source, section=path)
    normalized: dict[str, object] = {}
    for key, ref in refs.items():
        if not isinstance(key, str) or not key:
            raise ValueError(f"localization map asset ref key must be a non-empty string: {source}:{path}")
        if key == "featured_portraits":
            normalized[key] = _string_list(ref, source=source, path=f"{path}.{key}")
            continue
        normalized[key] = _required_str(ref, source=source, path=f"{path}.{key}")
    return normalized


def _source_evidence(value: object, *, source: str, path: str) -> SourceEvidence:
    evidence = _object(value, source=source, section=path)
    normalized: SourceEvidence = {
        "confidence": _required_value(
            evidence.get("confidence"),
            SOURCE_CONFIDENCE_VALUES,
            source=source,
            path=f"{path}.confidence",
        ),
        "tables": _string_list(evidence.get("tables", []), source=source, path=f"{path}.tables"),
    }
    if "notes" in evidence:
        normalized["notes"] = _string_list(evidence["notes"], source=source, path=f"{path}.notes")
    return normalized


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
    allowed_keys = {"name", "rarity", "category", "domain_type", "subtype", "asset_refs", "color", "source"}
    for item_id, item_value in items.items():
        if not isinstance(item_id, str) or not item_id:
            raise ValueError(f"localization map item id must be a non-empty string: {source}:items")
        item = _object(item_value, source=source, section=f"items.{item_id}")
        unknown = sorted(set(item) - allowed_keys)
        if unknown:
            raise ValueError(f"localization map item has unknown keys: {source}:items.{item_id}: {', '.join(unknown)}")
        normalized: dict[str, object] = {
            "name": _required_str(item.get("name"), source=source, path=f"items.{item_id}.name"),
            "rarity": _required_int(item.get("rarity"), source=source, path=f"items.{item_id}.rarity"),
        }
        for key in ("category", "domain_type", "subtype", "color"):
            text = _optional_str(item.get(key), source=source, path=f"items.{item_id}.{key}")
            if text is not None:
                normalized[key] = text
        if "asset_refs" in item:
            normalized["asset_refs"] = cast(
                ItemAssetRefs,
                _asset_ref_map(item["asset_refs"], source=source, path=f"items.{item_id}.asset_refs"),
            )
        if "source" in item:
            normalized["source"] = _source_evidence(item["source"], source=source, path=f"items.{item_id}.source")
        result[item_id] = normalized
    return result


def _validate_pools(value: object, *, source: str) -> dict[str, SourcePool]:
    pools = _object(value, source=source, section="pools")
    result: dict[str, SourcePool] = {}
    allowed_keys = {"name", "group_label", "title", "title_windows", "pickup_item_ids", "banner_ids", "asset_refs"}
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
            pickup_item_ids = _string_list(
                pool["pickup_item_ids"],
                source=source,
                path=f"pools.{pool_id}.pickup_item_ids",
            )
            if not pickup_item_ids:
                raise ValueError(f"localization map pickup_item_ids must not be empty: {source}:pools.{pool_id}")
            normalized["pickup_item_ids"] = pickup_item_ids
        if "banner_ids" in pool:
            banner_ids = _string_list(pool["banner_ids"], source=source, path=f"pools.{pool_id}.banner_ids")
            if banner_ids:
                normalized["banner_ids"] = banner_ids
        if "asset_refs" in pool:
            normalized["asset_refs"] = cast(
                BannerAssetRefs,
                _asset_ref_map(pool["asset_refs"], source=source, path=f"pools.{pool_id}.asset_refs"),
            )
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


def _validate_banners(
    value: object,
    *,
    source: str,
    pool_ids: set[str],
    items: dict[str, dict[str, object]],
) -> dict[str, SourceBanner]:
    banners = _object(value, source=source, section="banners")
    result: dict[str, SourceBanner] = {}
    item_ids = set(items)
    allowed_keys = {
        "banner_id",
        "pool_id",
        "pool_kind",
        "banner_type",
        "title",
        "short_title",
        "version",
        "phase",
        "start_at",
        "end_at",
        "timezone",
        "rate_up_5",
        "rate_up_4",
        "standard_5_pool",
        "standard_4_pool",
        "rule_id",
        "asset_refs",
        "color",
        "currency_id",
        "currency_count",
        "roll_unit",
        "source",
    }
    item_ref_keys = ("rate_up_5", "rate_up_4", "standard_5_pool", "standard_4_pool")
    unknown_item_refs: list[str] = []
    missing_rate_up_domains: list[str] = []
    unknown_pools: list[str] = []
    for banner_id, banner_value in banners.items():
        if not isinstance(banner_id, str) or not banner_id:
            raise ValueError(f"localization map banner id must be a non-empty string: {source}:banners")
        banner = _object(banner_value, source=source, section=f"banners.{banner_id}")
        unknown = sorted(set(banner) - allowed_keys)
        if unknown:
            raise ValueError(
                f"localization map banner has unknown keys: {source}:banners.{banner_id}: {', '.join(unknown)}"
            )
        entry_id = _required_str(banner.get("banner_id"), source=source, path=f"banners.{banner_id}.banner_id")
        if entry_id != banner_id:
            raise ValueError(f"localization map banner_id must match object key: {source}:banners.{banner_id}")
        pool_id = _required_str(banner.get("pool_id"), source=source, path=f"banners.{banner_id}.pool_id")
        if pool_id not in pool_ids:
            unknown_pools.append(f"{banner_id}.{pool_id}")
        normalized: SourceBanner = {
            "banner_id": entry_id,
            "pool_id": pool_id,
            "pool_kind": _required_value(
                banner.get("pool_kind"),
                POOL_KIND_VALUES,
                source=source,
                path=f"banners.{banner_id}.pool_kind",
            ),
            "banner_type": _required_value(
                banner.get("banner_type"),
                BANNER_TYPE_VALUES,
                source=source,
                path=f"banners.{banner_id}.banner_type",
            ),
            "title": _required_str(banner.get("title"), source=source, path=f"banners.{banner_id}.title"),
            "rate_up_5": _string_list(
                banner.get("rate_up_5", []),
                source=source,
                path=f"banners.{banner_id}.rate_up_5",
            ),
            "rate_up_4": _string_list(
                banner.get("rate_up_4", []),
                source=source,
                path=f"banners.{banner_id}.rate_up_4",
            ),
            "rule_id": _required_str(banner.get("rule_id"), source=source, path=f"banners.{banner_id}.rule_id"),
            "source": _source_evidence(banner.get("source", {}), source=source, path=f"banners.{banner_id}.source"),
        }
        for key in ("short_title", "start_at", "end_at", "color", "currency_id"):
            text = _optional_str(banner.get(key), source=source, path=f"banners.{banner_id}.{key}")
            if text is not None:
                normalized[key] = text
        for key in ("version", "phase"):
            text = _optional_machine_id(banner.get(key), source=source, path=f"banners.{banner_id}.{key}")
            if text is not None:
                normalized[key] = text
        timezone = _optional_value(
            banner.get("timezone"),
            {"Asia/Shanghai"},
            source=source,
            path=f"banners.{banner_id}.timezone",
        )
        if timezone is not None:
            normalized["timezone"] = timezone
        for key in ("standard_5_pool", "standard_4_pool"):
            if key in banner:
                normalized[key] = _string_list(banner[key], source=source, path=f"banners.{banner_id}.{key}")
        for key in ("currency_count", "roll_unit"):
            number = _optional_int(banner.get(key), source=source, path=f"banners.{banner_id}.{key}")
            if number is not None:
                normalized[key] = number
        if "asset_refs" in banner:
            normalized["asset_refs"] = cast(
                BannerAssetRefs,
                _asset_ref_map(banner["asset_refs"], source=source, path=f"banners.{banner_id}.asset_refs"),
            )
        for key in item_ref_keys:
            for item_id in normalized.get(key, []):
                if item_id not in item_ids:
                    unknown_item_refs.append(f"{banner_id}.{key}.{item_id}")
                elif key in {"rate_up_5", "rate_up_4"} and not items[item_id].get("domain_type"):
                    missing_rate_up_domains.append(f"{banner_id}.{key}.{item_id}")
        result[banner_id] = normalized
    if unknown_pools:
        raise ValueError(f"localization map banners target unknown pool ids: {source}: {', '.join(unknown_pools)}")
    if unknown_item_refs:
        raise ValueError(f"localization map banners target unknown item ids: {source}: {', '.join(unknown_item_refs)}")
    if missing_rate_up_domains:
        raise ValueError(
            f"localization map rate_up item ids must have domain_type: {source}: {', '.join(missing_rate_up_domains)}"
        )
    return result


def _validate_gacha_rules(value: object, *, source: str) -> dict[str, SourceGachaRule]:
    rules = _object(value, source=source, section="gacha_rules")
    result: dict[str, SourceGachaRule] = {}
    allowed_keys = {
        "rule_id",
        "pool_kind",
        "hard_pity_5",
        "hard_pity_4",
        "pickup_win_rate_5",
        "pickup_win_rate_4",
        "has_guarantee_5",
        "has_guarantee_4",
        "guarantee_scope",
        "carry_scope",
        "rule_text_refs",
        "source",
    }
    for rule_id, rule_value in rules.items():
        if not isinstance(rule_id, str) or not rule_id:
            raise ValueError(f"localization map rule id must be a non-empty string: {source}:gacha_rules")
        rule = _object(rule_value, source=source, section=f"gacha_rules.{rule_id}")
        unknown = sorted(set(rule) - allowed_keys)
        if unknown:
            raise ValueError(
                f"localization map gacha rule has unknown keys: {source}:gacha_rules.{rule_id}: {', '.join(unknown)}"
            )
        entry_id = _required_str(rule.get("rule_id"), source=source, path=f"gacha_rules.{rule_id}.rule_id")
        if entry_id != rule_id:
            raise ValueError(f"localization map rule_id must match object key: {source}:gacha_rules.{rule_id}")
        normalized: SourceGachaRule = {
            "rule_id": entry_id,
            "pool_kind": _required_value(
                rule.get("pool_kind"),
                POOL_KIND_VALUES,
                source=source,
                path=f"gacha_rules.{rule_id}.pool_kind",
            ),
            "source": _source_evidence(rule.get("source", {}), source=source, path=f"gacha_rules.{rule_id}.source"),
        }
        for key in ("hard_pity_5", "hard_pity_4"):
            number = _optional_int(rule.get(key), source=source, path=f"gacha_rules.{rule_id}.{key}")
            if number is not None:
                if number <= 0:
                    raise ValueError(
                        f"localization map gacha rule hard pity must be positive: {source}:gacha_rules.{rule_id}.{key}"
                    )
                normalized[key] = number
        for key in ("pickup_win_rate_5", "pickup_win_rate_4"):
            number = _optional_int(rule.get(key), source=source, path=f"gacha_rules.{rule_id}.{key}")
            if number is not None:
                if number < 0 or number > 100:
                    raise ValueError(
                        "localization map gacha rule pickup win rate must be in 0..100: "
                        f"{source}:gacha_rules.{rule_id}.{key}"
                    )
                normalized[key] = number
        for key in ("has_guarantee_5", "has_guarantee_4"):
            flag = _optional_bool(rule.get(key), source=source, path=f"gacha_rules.{rule_id}.{key}")
            if flag is not None:
                normalized[key] = flag
        for key in ("guarantee_scope", "carry_scope"):
            text = _optional_value(rule.get(key), SCOPE_VALUES, source=source, path=f"gacha_rules.{rule_id}.{key}")
            if text is not None:
                normalized[key] = text
        if "rule_text_refs" in rule:
            refs = _object(rule["rule_text_refs"], source=source, section=f"gacha_rules.{rule_id}.rule_text_refs")
            normalized_refs: RuleTextRefs = {}
            for key in ("rule_desc_1", "rule_desc_2", "probability_desc"):
                text = _optional_str(refs.get(key), source=source, path=f"gacha_rules.{rule_id}.rule_text_refs.{key}")
                if text is not None:
                    normalized_refs[key] = text
            if normalized_refs:
                normalized["rule_text_refs"] = normalized_refs
        result[rule_id] = normalized
    return result


def _validate_pool_banner_refs(pools: dict[str, SourcePool], banner_ids: set[str], *, source: str) -> None:
    unknown: list[str] = []
    for pool_id, pool in pools.items():
        for banner_id in pool.get("banner_ids", []):
            if banner_id not in banner_ids:
                unknown.append(f"{pool_id}.{banner_id}")
    if unknown:
        raise ValueError(f"localization map banner_ids target unknown banner ids: {source}: {', '.join(unknown)}")


def _expected_banner_shape(pool_id: str) -> tuple[str, str] | None:
    if pool_id == "CardPool_Character":
        return "monopoly_limited", "limited"
    if pool_id == "CardPool_NewRole":
        return "monopoly_standard", "standard"
    if pool_id.startswith("ForkLottery_"):
        return "fork_lottery", "fork"
    return None


def _validate_banner_catalog(
    pools: dict[str, SourcePool],
    banners: dict[str, SourceBanner],
    *,
    source: str,
) -> None:
    wrong_pool: list[str] = []
    wrong_shape: list[str] = []
    bad_time: list[str] = []
    duplicate_end: list[str] = []
    overlapping: list[str] = []

    for pool_id, pool in pools.items():
        for banner_id in pool.get("banner_ids", []):
            banner = banners.get(banner_id)
            if banner is not None and banner["pool_id"] != pool_id:
                wrong_pool.append(f"{pool_id}.{banner_id}->{banner['pool_id']}")

    for banner_id, banner in banners.items():
        expected = _expected_banner_shape(banner["pool_id"])
        if expected is not None and (banner["pool_kind"], banner["banner_type"]) != expected:
            wrong_shape.append(f"{banner_id}.{banner['pool_kind']}.{banner['banner_type']}")
        if banner["banner_type"] != "limited":
            continue
        end_at = banner.get("end_at")
        if normalize_game_time(end_at) is None:
            bad_time.append(f"{banner_id}.end_at")
        start_at = banner.get("start_at")
        if start_at is not None and normalize_game_time(start_at) is None:
            bad_time.append(f"{banner_id}.start_at")

    limited_by_pool: dict[str, list[tuple[str, SourceBanner, str, str | None]]] = {}
    for banner_id, banner in banners.items():
        if banner["banner_type"] != "limited":
            continue
        end_at = normalize_game_time(banner.get("end_at"))
        if end_at is None:
            continue
        start_at = normalize_game_time(banner.get("start_at")) if banner.get("start_at") else None
        limited_by_pool.setdefault(banner["pool_id"], []).append((banner_id, banner, end_at, start_at))

    for pool_id, entries in limited_by_pool.items():
        seen_end: set[str] = set()
        previous_end: str | None = None
        for banner_id, _banner, end_at, start_at in sorted(entries, key=lambda entry: entry[2]):
            if end_at in seen_end:
                duplicate_end.append(f"{pool_id}.{end_at}")
            seen_end.add(end_at)
            if start_at is not None and start_at >= end_at:
                bad_time.append(f"{banner_id}.start_at>=end_at")
            if previous_end is not None:
                if end_at <= previous_end:
                    overlapping.append(f"{pool_id}.{banner_id}")
                if start_at is not None and start_at < previous_end:
                    overlapping.append(f"{pool_id}.{banner_id}")
            previous_end = end_at

    if wrong_pool:
        raise ValueError(
            f"localization map banner_ids target banners for other pools: {source}: {', '.join(wrong_pool)}"
        )
    if wrong_shape:
        raise ValueError(f"localization map banner shape does not match pool id: {source}: {', '.join(wrong_shape)}")
    if bad_time:
        raise ValueError(f"localization map banner time is invalid: {source}: {', '.join(bad_time)}")
    if duplicate_end:
        raise ValueError(
            f"localization map limited banner windows duplicate end_at: {source}: {', '.join(duplicate_end)}"
        )
    if overlapping:
        raise ValueError(f"localization map limited banner windows overlap: {source}: {', '.join(overlapping)}")


def _validate_banner_rule_refs(
    banners: dict[str, SourceBanner],
    rule_ids: set[str],
    *,
    source: str,
) -> None:
    unknown = [
        f"{banner_id}.{banner['rule_id']}" for banner_id, banner in banners.items() if banner["rule_id"] not in rule_ids
    ]
    if unknown:
        raise ValueError(f"localization map banners target unknown gacha rules: {source}: {', '.join(unknown)}")


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
    banners = _validate_banners(data.get("banners", {}), source=source, pool_ids=set(pools), items=items)
    gacha_rules = _validate_gacha_rules(data.get("gacha_rules", {}), source=source)
    _validate_pool_banner_refs(pools, set(banners), source=source)
    _validate_banner_catalog(pools, banners, source=source)
    _validate_banner_rule_refs(banners, set(gacha_rules), source=source)

    source_map: LocalizationMapSource = {
        "schema_version": MAP_SCHEMA_VERSION,
        "csv_headers": _string_map(data.get("csv_headers", {}), source=source, section="csv_headers"),
        "items": cast(dict[str, object], dict(sorted(items.items()))),
        "item_aliases": dict(sorted(aliases.items())),
        "pools": pools,
        "banners": banners,
        "gacha_rules": gacha_rules,
        "labels": _string_map(data.get("labels", {}), source=source, section="labels"),
    }
    return source_map


def expand_map_source(source_map: LocalizationMapSource) -> LocalizationMap:
    """Expand normalized v4 map data into runtime lookup/rule sections."""

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
                "domain_type": cast(str | None, item.get("domain_type")),
                "subtype": cast(str | None, item.get("subtype")),
                "asset_refs": cast(ItemAssetRefs, item.get("asset_refs", {})),
                "color": cast(str | None, item.get("color")),
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
        "banners": source_map.get("banners", {}),
        "gacha_rules": source_map.get("gacha_rules", {}),
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
