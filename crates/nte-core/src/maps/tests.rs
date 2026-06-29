#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::BannerResolutionIssue;

    #[test]
    fn load_bundled_map_keeps_banner_and_rule_sections() {
        let map = load_map("zh-Hant").expect("zh-Hant map should load");

        let banner = map
            .banners
            .get("ForkLottery_AnHunQu")
            .expect("fork banner should exist");
        assert_eq!(banner.banner_id, "ForkLottery_AnHunQu");
        assert_eq!(banner.pool_id, "ForkLottery_AnHunQu");
        assert_eq!(banner.rate_up_5, vec!["fork_Rose"]);
        assert_eq!(banner.currency_id.as_deref(), Some("WeaponGacha"));
        assert!(map
            .banners
            .get("monopoly_limited_Nanali")
            .expect("limited banner should exist")
            .start_at
            .is_none());

        let rule = map
            .gacha_rules
            .get("fork_lottery_s")
            .expect("fork rule should exist");
        assert_eq!(rule.hard_pity_5, Some(60));
        assert_eq!(rule.hard_up_pity_5, Some(80));
        assert_eq!(rule.pickup_win_rate_5, Some(25));
        assert_eq!(rule.has_guarantee_5, Some(true));

        let item = map.items.get("1010").expect("item asset refs should exist");
        assert!(!item.asset_refs.contains_key("portrait"));
        assert!(item.asset_refs.contains_key("head_icon"));
        assert_eq!(
            item.asset_refs.get("banner").and_then(|value| value.as_str()),
            Some(
                "/Game/UI/UI/PlayerInfo/BusinessCards/Card_Small/YH_UI_bg_card_show_strip_08_s.YH_UI_bg_card_show_strip_08_s"
            )
        );
        assert_eq!(
            map.banners
                .get("monopoly_limited_Nanali")
                .and_then(|banner| banner.asset_refs.get("image"))
                .and_then(|value| value.as_str()),
            Some(
                "/Game/UI/UI/PlayerInfo/BusinessCards/Card_Small/YH_UI_bg_card_show_strip_08_s.YH_UI_bg_card_show_strip_08_s"
            )
        );
        assert_eq!(item.color.as_deref(), Some("#F24B7E"));

        assert_eq!(
            map.pool_label("CardPool_Character", Some("2026-06-04 00:00:00")),
            "久夢初醒時"
        );
        assert!(map.is_pickup_item("ForkLottery_AnHunQu", "fork_Rose"));
    }

    #[test]
    fn resolve_banner_handles_standard_fork_and_limited_boundaries() {
        let map = load_map("zh-Hant").expect("zh-Hant map should load");

        let standard = map.resolve_banner("CardPool_NewRole", None);
        assert_eq!(standard.resolution_issue, None);
        assert_eq!(standard.banner_id.as_deref(), Some("monopoly_standard"));
        assert_eq!(standard.banner_type.as_deref(), Some("standard"));
        assert_eq!(standard.version, None);

        let fork = map.resolve_banner("ForkLottery_AnHunQu", None);
        assert_eq!(fork.resolution_issue, None);
        assert_eq!(fork.banner_id.as_deref(), Some("ForkLottery_AnHunQu"));
        assert_eq!(fork.rate_up_5, vec!["fork_Rose"]);

        for (record_time, banner_id) in [
            ("2026-05-13 05:59:00", "monopoly_limited_Nanali"),
            ("2026-05-13 05:59:01", "monopoly_limited_Xun"),
            ("2026-06-03 05:59:00", "monopoly_limited_Xun"),
            ("2026-06-03 05:59:01", "monopoly_limited_AnHunQu"),
            ("2026-06-24 05:59:00", "monopoly_limited_AnHunQu"),
            ("2026-06-24 05:59:01", "monopoly_limited_Kaesi"),
        ] {
            let resolved = map.resolve_banner("CardPool_Character", Some(record_time));
            assert_eq!(resolved.resolution_issue, None);
            assert_eq!(resolved.banner_id.as_deref(), Some(banner_id));
        }
        let limited = map.resolve_banner("CardPool_Character", Some("2026-05-13 05:59:00"));
        assert_eq!(limited.version, None);
    }

    #[test]
    fn resolve_banner_reports_limited_unresolved_edges_and_label_fallback() {
        let map = load_map("zh-Hant").expect("zh-Hant map should load");

        assert_eq!(
            map.resolve_banner("CardPool_Character", None)
                .resolution_issue,
            Some(BannerResolutionIssue::UnknownTime)
        );
        assert_eq!(
            map.resolve_banner("CardPool_Character", Some("not a time"))
                .resolution_issue,
            Some(BannerResolutionIssue::UnknownTime)
        );
        assert_eq!(
            map.resolve_banner("CardPool_Character", Some("2026-07-08 05:59:01"))
                .resolution_issue,
            Some(BannerResolutionIssue::OutsideKnownWindows)
        );
        let unresolved = map.resolve_banner("CardPool_Character", Some("2026-07-08 05:59:01"));
        assert_eq!(unresolved.banner_id.as_deref(), Some("CardPool_Character"));
        assert!(unresolved.asset_refs.is_empty());
        assert_eq!(unresolved.version, None);
        assert_eq!(
            map.pool_label("CardPool_Character", Some("2026-07-08 05:59:01")),
            "限定棋盤"
        );
        assert_eq!(
            normalize_game_time(Some("2026-06-03T05:59:01.341000")).as_deref(),
            Some("2026-06-03 05:59:01")
        );
        assert!(normalize_game_time(Some("2026-06-03T05:59:01+08:00")).is_none());
    }

    #[test]
    fn resolve_banner_returns_synthetic_unknown_fork() {
        let map = load_map("zh-Hant").expect("zh-Hant map should load");
        let banner = map.resolve_banner("ForkLottery_KaesiNew", Some("2026-08-01 10:00:00"));

        assert_eq!(
            banner.resolution_issue,
            Some(BannerResolutionIssue::UnknownPool)
        );
        assert_eq!(banner.banner_id.as_deref(), Some("ForkLottery_KaesiNew"));
        assert_eq!(banner.pool_id.as_deref(), Some("ForkLottery_KaesiNew"));
        assert_eq!(banner.pool_kind.as_deref(), Some("fork_lottery"));
        assert_eq!(banner.banner_type.as_deref(), Some("fork"));
        assert_eq!(banner.title.as_deref(), Some("KaesiNew"));
        assert!(banner.asset_refs.is_empty());
    }

    #[test]
    fn load_map_returns_cached_clone_without_shared_mutation() {
        let mut first = load_map("zh-Hant").expect("zh-Hant map should load");
        first.items.get_mut("1010").expect("item should exist").name = "mutated".to_string();

        let second = load_map("zh-Hant").expect("zh-Hant map should load again");

        assert_ne!(
            second.items.get("1010").map(|item| item.name.as_str()),
            Some("mutated")
        );
    }
}
