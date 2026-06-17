from __future__ import annotations

from datetime import datetime
from typing import cast

from nte_gacha_exporter.core.schema import LocalizationMap, ResolvedBanner, SourceBanner


def normalize_game_time(value: str | None) -> str | None:
    """Normalize game-local timestamp text for lexicographic comparison."""

    if not value:
        return None
    try:
        parsed = datetime.fromisoformat(value.strip().replace(" ", "T"))
    except ValueError:
        return None
    if parsed.tzinfo is not None:
        return None
    return parsed.strftime("%Y-%m-%d %H:%M:%S")


def resolve_banner(mapping: LocalizationMap, pool_id: str | None, record_time: str | None) -> ResolvedBanner:
    if not pool_id:
        return _unmatched("unknown_pool", "pool id is missing")

    pools = mapping.get("pools", {})
    pool_meta = mapping.get("pool_meta", {})
    pool_known = pool_id in pools or pool_id in pool_meta
    if not pool_known:
        return _unmatched("unknown_pool", f"pool is not in localization map: {pool_id}")

    banner_ids = _pool_banner_ids(mapping, pool_id)
    if not banner_ids:
        return _unmatched("unknown_pool", f"pool has no linked banners: {pool_id}")

    banners = mapping.get("banners", {})
    candidates: list[tuple[str, SourceBanner]] = []
    for banner_id in banner_ids:
        banner = banners.get(banner_id)
        if banner and banner.get("pool_id") == pool_id:
            candidates.append((banner_id, banner))
    if not candidates:
        return _unmatched("unknown_pool", f"pool has no usable linked banners: {pool_id}")

    if pool_id == "CardPool_NewRole":
        return _single_banner(candidates, "standard", "standard")
    if pool_id.startswith("ForkLottery_"):
        exact = [(banner_id, banner) for banner_id, banner in candidates if banner_id == pool_id]
        if exact:
            return _single_banner(exact, "fork", "fork")
        return _single_banner(candidates, "fork", "fork")
    if pool_id == "CardPool_Character":
        return _resolve_limited(candidates, record_time)

    return _unmatched("unknown_pool", f"pool has unsupported banner resolution: {pool_id}")


def banner_label(mapping: LocalizationMap, pool_id: str | None, record_time: str | None) -> str:
    resolved = resolve_banner(mapping, pool_id, record_time)
    if resolved.get("status") == "matched" and resolved.get("title"):
        return str(resolved["title"])
    return pool_fallback_label(mapping, pool_id, record_time)


def pool_fallback_label(mapping: LocalizationMap, pool_id: str | None, record_time: str | None) -> str:
    meta = _pool_meta_for_id(mapping, pool_id)
    window_title = _pool_title_from_windows(meta, record_time)
    if window_title:
        return window_title

    title = str(meta.get("title") or "")
    if title:
        return title

    if not pool_id:
        return ""
    pools = mapping.get("pools", {})
    if isinstance(pools, dict):
        return str(pools.get(pool_id, pool_id))
    return pool_id


def _resolve_limited(candidates: list[tuple[str, SourceBanner]], record_time: str | None) -> ResolvedBanner:
    limited = [(banner_id, banner) for banner_id, banner in candidates if banner.get("banner_type") == "limited"]
    if not limited:
        return _unmatched("unknown_pool", "pool has no linked limited banners")

    normalized_record_time = normalize_game_time(record_time)
    if normalized_record_time is None:
        return _unmatched("unknown_time", "limited banner resolution requires valid record time")

    windows: list[tuple[str, str | None, str, SourceBanner]] = []
    for banner_id, banner in limited:
        end_at = normalize_game_time(cast(str | None, banner.get("end_at")))
        if end_at is None:
            return _unmatched("outside_known_windows", f"limited banner has no valid end_at: {banner_id}")
        windows.append((banner_id, normalize_game_time(cast(str | None, banner.get("start_at"))), end_at, banner))
    windows.sort(key=lambda entry: entry[2])

    matches: list[SourceBanner] = []
    previous_end: str | None = None
    for _, start_at, end_at, banner in windows:
        effective_start = start_at or previous_end
        if effective_start is None:
            in_window = normalized_record_time <= end_at
        else:
            in_window = effective_start < normalized_record_time <= end_at
        if in_window:
            matches.append(banner)
        previous_end = end_at

    if len(matches) == 1:
        return _matched(matches[0])
    if len(matches) > 1:
        return _unmatched("ambiguous", "multiple limited banners match record time")
    return _unmatched("outside_known_windows", "record time is outside known limited banner windows")


def _single_banner(
    candidates: list[tuple[str, SourceBanner]],
    banner_type: str,
    reason_label: str,
) -> ResolvedBanner:
    matching = [banner for _, banner in candidates if banner.get("banner_type") == banner_type]
    if len(matching) == 1:
        return _matched(matching[0])
    if len(matching) > 1:
        return _unmatched("ambiguous", f"multiple {reason_label} banners are linked")
    return _unmatched("unknown_pool", f"pool has no linked {reason_label} banner")


def _matched(banner: SourceBanner) -> ResolvedBanner:
    source = banner.get("source", {})
    resolved: ResolvedBanner = {
        "status": "matched",
        "reason": "matched",
        "banner_id": str(banner["banner_id"]),
        "pool_id": str(banner["pool_id"]),
        "pool_kind": str(banner["pool_kind"]),
        "banner_type": str(banner["banner_type"]),
        "title": str(banner["title"]),
        "rate_up_5": list(banner.get("rate_up_5", [])),
        "rate_up_4": list(banner.get("rate_up_4", [])),
        "rule_id": str(banner["rule_id"]),
        "asset_refs": banner.get("asset_refs", {}),
        "source_confidence": str(source.get("confidence") or "unknown"),
    }
    for key in ("version", "phase", "start_at", "end_at", "timezone"):
        value = banner.get(key)
        if value:
            resolved[key] = str(value)
    return resolved


def _unmatched(status: str, reason: str) -> ResolvedBanner:
    return cast(ResolvedBanner, {"status": status, "reason": reason})


def _pool_banner_ids(mapping: LocalizationMap, pool_id: str) -> list[str]:
    meta = _pool_meta_for_id(mapping, pool_id)
    banner_ids = meta.get("banner_ids", [])
    if isinstance(banner_ids, list):
        return [str(banner_id) for banner_id in banner_ids if banner_id]
    return []


def _pool_meta_for_id(mapping: LocalizationMap, pool_id: str | None) -> dict[str, object]:
    if not pool_id:
        return {}
    pool_meta = mapping.get("pool_meta", {})
    if not isinstance(pool_meta, dict):
        return {}
    meta = pool_meta.get(pool_id, {})
    return cast(dict[str, object], meta) if isinstance(meta, dict) else {}


def _pool_title_from_windows(meta: dict[str, object], record_time: str | None) -> str | None:
    normalized_record_time = normalize_game_time(record_time)
    windows = meta.get("title_windows")
    if normalized_record_time is None or not isinstance(windows, list):
        return None

    for window in windows:
        if not isinstance(window, dict):
            continue
        title = str(window.get("title") or "")
        end_at = normalize_game_time(str(window.get("end_at_tz8") or ""))
        if title and end_at is not None and normalized_record_time <= end_at:
            return title
    return None
