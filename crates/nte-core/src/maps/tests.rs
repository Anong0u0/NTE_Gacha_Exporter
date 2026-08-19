#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::BannerResolutionIssue;

    #[test]
    fn load_bundled_map_keeps_banner_and_rule_sections() {
        let map = load_map("zh-Hant").expect("zh-Hant map should load");

        let banner = map
            .banner_sources
            .get("ForkLottery_AnHunQu")
            .expect("fork banner should exist");
        assert_eq!(banner.source_id, "ForkLottery_AnHunQu");
        assert_eq!(banner.pool_id, "ForkLottery_AnHunQu");
        assert_eq!(banner.rate_up_5, vec!["fork_Rose"]);
        assert_eq!(banner.currency_id.as_deref(), Some("WeaponGacha"));
        assert!(map.banner_sources.contains_key("Lottery_Nanali"));
        assert!(map
            .banner_occurrences
            .iter()
            .any(|occurrence| occurrence.occurrence_id == "Lottery_Nanali@2026-08"));

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
            map.banner_sources
                .get("Lottery_Nanali")
                .and_then(|banner| banner.asset_refs.get("image"))
                .and_then(|value| value.as_str()),
            Some(
                "/Game/UI/UI/PlayerInfo/BusinessCards/Card_Small/YH_UI_bg_card_show_strip_08_s.YH_UI_bg_card_show_strip_08_s"
            )
        );
        assert_eq!(item.color.as_deref(), Some("#F24B7E"));
        assert_eq!(
            map.items.get("1036").map(|item| item.name.as_str()),
            Some("角色·殘虹")
        );
        assert_eq!(
            map.banner_sources
                .get("Lottery_Canhong")
                .map(|banner| banner.title.as_str()),
            Some("惑心譎影")
        );

        assert_eq!(
            map.pool_label(BannerResolveInput {
                pool_id: "CardPool_Character",
                lottery_id: Some("Lottery_AnHunQu"),
            }),
            "久夢初醒時"
        );
        assert!(map.is_pickup_item("ForkLottery_AnHunQu", "fork_Rose"));
    }

    #[test]
    fn canonical_item_id_uses_unique_case_folded_item_key() {
        let map = load_map("zh-Hant").expect("zh-Hant map should load");

        assert_eq!(
            map.canonical_item_id("fork_Wushoutieyu"),
            "fork_wushoutieyu"
        );
        assert_eq!(map.item_name("fork_Wushoutieyu"), "弧盤·焰魂狂飆");
        assert_eq!(map.item_rarity("fork_Wushoutieyu"), Some(5));
        assert_eq!(map.canonical_item_id("fork_Rose"), "fork_Rose");
        assert_eq!(map.canonical_item_id("UnknownItem"), "UnknownItem");
    }

    #[test]
    fn bundled_maps_have_no_case_folded_item_id_collisions() {
        for locale in available_locales() {
            let map = load_map(&locale).expect("bundled map should load");
            let mut folded = std::collections::BTreeMap::new();
            for item_id in map.items.keys() {
                let key = item_id.to_ascii_lowercase();
                assert_eq!(
                    folded.insert(key, item_id),
                    None,
                    "{locale}: case-folded duplicate item_id: {item_id}"
                );
            }
        }
    }

    #[test]
    fn resolve_banner_uses_wire_lottery_id() {
        let map = load_map("zh-Hant").expect("zh-Hant map should load");

        let standard = map.resolve_banner_source(BannerResolveInput {
            pool_id: "CardPool_NewRole",
            lottery_id: Some("Lottery_Permanent"),
        });
        assert_eq!(standard.resolution_issue, None);
        assert_eq!(standard.source_id.as_deref(), Some("Lottery_Permanent"));
        assert_eq!(standard.banner_type.as_deref(), Some("standard"));

        let fork = map.resolve_banner_source(BannerResolveInput {
            pool_id: "ForkLottery_AnHunQu",
            lottery_id: None,
        });
        assert_eq!(fork.resolution_issue, None);
        assert_eq!(fork.source_id.as_deref(), Some("ForkLottery_AnHunQu"));
        assert_eq!(fork.rate_up_5, vec!["fork_Rose"]);

        for lottery_id in [
            "Lottery_Nanali",
            "Lottery_Xun",
            "Lottery_AnHunQu",
            "Lottery_Kaesi",
            "Lottery_ZhenHong",
            "Lottery_Yiluoyi",
            "Lottery_Canhong",
        ] {
            let resolved = map.resolve_banner_source(BannerResolveInput {
                pool_id: "CardPool_Character",
                lottery_id: Some(lottery_id),
            });
            assert_eq!(resolved.resolution_issue, None);
            assert_eq!(resolved.source_id.as_deref(), Some(lottery_id));
        }
    }

    #[test]
    fn legacy_resolution_prefers_explicit_id_and_uses_unambiguous_time_fallback() {
        let map = load_map("zh-Hant").expect("zh-Hant map should load");

        for (record_time, lottery_id) in [
            ("2026-05-01 00:00:00", "Lottery_Nanali"),
            ("2026-05-20 00:00:00", "Lottery_Xun"),
            ("2026-06-09 00:00:00", "Lottery_AnHunQu"),
            ("2026-06-28 00:00:00", "Lottery_Kaesi"),
            ("2026-07-10 00:00:00", "Lottery_ZhenHong"),
            ("2026-08-01 00:00:00", "Lottery_Yiluoyi"),
        ] {
            assert_eq!(
                map.resolve_legacy_lottery_id(
                    "CardPool_Character",
                    Some(record_time),
                    None
                )
                .unwrap()
                .as_deref(),
                Some(lottery_id)
            );
        }
        assert_eq!(
            map.resolve_legacy_lottery_id(
                "CardPool_Character",
                Some("2026-08-13 16:11:30"),
                Some("monopoly_limited_Canhong")
            )
            .unwrap()
            .as_deref(),
            Some("Lottery_Canhong")
        );
        for ambiguous_time in ["2026-05-10 00:00:00", "2026-08-13 16:11:30"] {
            assert_eq!(
                map.resolve_legacy_lottery_id(
                    "CardPool_Character",
                    Some(ambiguous_time),
                    None
                )
                .unwrap(),
                None
            );
        }
        assert_eq!(
            normalize_game_time(Some("2026-06-03T05:59:01.341000")).as_deref(),
            Some("2026-06-03 05:59:01")
        );
        assert!(normalize_game_time(Some("2026-06-03T05:59:01+08:00")).is_none());

        let error = map
            .resolve_legacy_lottery_id(
                "CardPool_Character",
                None,
                Some("monopoly_standard"),
            )
            .unwrap_err();
        assert!(error.to_string().contains("legacy banner conflicts with pool"));

        assert_eq!(
            map.resolve_legacy_lottery_id(
                "CardPool_Character",
                Some("2026-05-01 00:00:00"),
                Some("monopoly_limited_Unknown")
            )
            .unwrap()
            .as_deref(),
            Some("Lottery_Unknown")
        );
        let error = map
            .resolve_legacy_lottery_id(
                "CardPool_NewRole",
                Some("2026-05-01 00:00:00"),
                Some("unknown_banner"),
            )
            .unwrap_err();
        assert!(error.to_string().contains("unknown legacy banner_id"));
    }

    #[test]
    fn resolve_banner_reports_lottery_id_errors() {
        let map = load_map("zh-Hant").expect("zh-Hant map should load");

        assert_eq!(
            map.resolve_banner_source(BannerResolveInput {
                pool_id: "CardPool_Character",
                lottery_id: None,
            })
            .resolution_issue,
            Some(BannerResolutionIssue::MissingLotteryId)
        );
        assert_eq!(
            map.resolve_banner_source(BannerResolveInput {
                pool_id: "CardPool_Character",
                lottery_id: Some("Lottery_Unknown"),
            })
            .resolution_issue,
            Some(BannerResolutionIssue::UnknownLotteryId)
        );
        assert_eq!(
            map.resolve_banner_source(BannerResolveInput {
                pool_id: "CardPool_NewRole",
                lottery_id: Some("Lottery_Nanali"),
            })
            .resolution_issue,
            Some(BannerResolutionIssue::LotteryPoolMismatch)
        );
    }

    #[test]
    fn resolve_banner_returns_synthetic_unknown_fork() {
        let map = load_map("zh-Hant").expect("zh-Hant map should load");
        let banner = map.resolve_banner_source(BannerResolveInput {
            pool_id: "ForkLottery_KaesiNew",
            lottery_id: None,
        });

        assert_eq!(
            banner.resolution_issue,
            Some(BannerResolutionIssue::UnknownPool)
        );
        assert_eq!(banner.source_id.as_deref(), Some("ForkLottery_KaesiNew"));
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
