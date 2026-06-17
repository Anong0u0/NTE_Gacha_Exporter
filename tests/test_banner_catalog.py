from __future__ import annotations

from nte_gacha_exporter.mapping.banner_catalog import banner_label, normalize_game_time, resolve_banner
from nte_gacha_exporter.mapping.runtime import load_map


def test_resolve_standard_banner():
    mapping = load_map("zh-Hant")

    resolved = resolve_banner(mapping, "CardPool_NewRole", None)

    assert resolved["status"] == "matched"
    assert resolved["banner_id"] == "monopoly_standard"
    assert resolved["banner_type"] == "standard"
    assert "version" not in resolved
    assert "phase" not in resolved
    assert resolved["source_confidence"] == "curated"
    assert banner_label(mapping, "CardPool_NewRole", None) == "世間奇遇"


def test_resolve_fork_banner():
    mapping = load_map("zh-Hant")

    resolved = resolve_banner(mapping, "ForkLottery_AnHunQu", None)

    assert resolved["status"] == "matched"
    assert resolved["banner_id"] == "ForkLottery_AnHunQu"
    assert resolved["banner_type"] == "fork"
    assert "version" not in resolved
    assert "phase" not in resolved
    assert resolved["source_confidence"] == "exact"
    assert resolved["rate_up_5"] == ["fork_Rose"]


def test_resolve_limited_banner_boundaries():
    mapping = load_map("zh-Hant")

    cases = [
        ("2026-05-13 05:59:00", "monopoly_limited_Nanali"),
        ("2026-05-13 05:59:01", "monopoly_limited_Xun"),
        ("2026-06-03 05:59:00", "monopoly_limited_Xun"),
        ("2026-06-03 05:59:01", "monopoly_limited_AnHunQu"),
        ("2026-06-24 05:59:00", "monopoly_limited_AnHunQu"),
        ("2026-06-24 05:59:01", "monopoly_limited_Kaesi"),
    ]

    for record_time, banner_id in cases:
        resolved = resolve_banner(mapping, "CardPool_Character", record_time)
        assert resolved["status"] == "matched"
        assert resolved["banner_id"] == banner_id
    resolved = resolve_banner(mapping, "CardPool_Character", "2026-05-13 05:59:00")
    assert resolved["phase"] == "limited_2026_05_13"
    assert "version" not in resolved


def test_resolve_limited_banner_unmatched_edges():
    mapping = load_map("zh-Hant")

    assert resolve_banner(mapping, "CardPool_Character", None)["status"] == "unknown_time"
    assert resolve_banner(mapping, "CardPool_Character", "not a time")["status"] == "unknown_time"
    resolved = resolve_banner(mapping, "CardPool_Character", "2026-07-08 05:59:01")
    assert resolved["status"] == "outside_known_windows"
    assert "version" not in resolved
    assert "phase" not in resolved
    assert banner_label(mapping, "CardPool_Character", "2026-07-08 05:59:01") == "限定棋盤"


def test_normalize_game_time_accepts_space_and_iso_without_timezone():
    assert normalize_game_time("2026-06-03 05:59:01") == "2026-06-03 05:59:01"
    assert normalize_game_time("2026-06-03T05:59:01.341000") == "2026-06-03 05:59:01"
    assert normalize_game_time("2026-06-03T05:59:01+08:00") is None
