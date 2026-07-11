#[cfg(test)]
mod tests {
    use super::*;
    use crate::load_map;

    fn record(record_id: &str, pool_id: &str, item_id: &str, time: &str) -> InternalRecord {
        InternalRecord {
            record_id: record_id.to_string(),
            source_order: 0,
            record_type: if pool_id.starts_with("ForkLottery_") {
                "fork".to_string()
            } else {
                "monopoly".to_string()
            },
            time: Some(time.to_string()),
            pool_id: pool_id.to_string(),
            item_id: item_id.to_string(),
            count: Some(1),
            roll_points: Some(1),
            roll_label_id: None,
            secondary_item_id: None,
            secondary_count: None,
        }
    }

    #[test]
    fn rule_for_record_uses_resolved_banner_rule() {
        let map = load_map("zh-Hant").expect("map should load");

        let fork = rule_for_record(
            &map,
            &record(
                "fork",
                "ForkLottery_AnHunQu",
                "fork_Rose",
                "2026-01-01 00:00:00",
            ),
        )
        .expect("fork rule should resolve");
        assert_eq!(fork.resolution_issue, None);
        assert_eq!(fork.rule.rule_id.as_deref(), Some("fork_lottery_s"));
        assert_eq!(fork.rule.hard_pity_5, Some(60));
        assert_eq!(fork.rule.hard_up_pity_5, Some(80));
        assert_eq!(fork.rule.pickup_win_rate_5, Some(25));
        assert_eq!(fork.rule.has_guarantee_5, Some(true));

        let standard = rule_for_record(
            &map,
            &record(
                "standard",
                "CardPool_NewRole",
                "fork_dustbin",
                "2026-01-01 00:00:00",
            ),
        )
        .expect("standard rule should resolve");
        assert_eq!(standard.resolution_issue, None);
        assert_eq!(standard.rule.rule_id.as_deref(), Some("monopoly_standard"));
        assert_eq!(standard.rule.hard_pity_5, Some(90));

        let limited = rule_for_record(
            &map,
            &record(
                "limited",
                "CardPool_Character",
                "1004",
                "2026-06-04 00:00:00",
            ),
        )
        .expect("limited rule should resolve");
        assert_eq!(limited.resolution_issue, None);
        assert_eq!(limited.rule.rule_id.as_deref(), Some("monopoly_limited"));
    }

    #[test]
    fn rule_for_record_falls_back_when_banner_is_unresolved() {
        let map = load_map("zh-Hant").expect("map should load");

        let resolution = rule_for_record(
            &map,
            &record(
                "outside",
                "CardPool_Character",
                "fork_dustbin",
                "2026-08-19 05:59:01",
            ),
        )
        .expect("fallback rule should resolve");

        assert_eq!(
            resolution.resolution_issue,
            Some(RuleResolutionIssue::MissingBanner)
        );
        assert_eq!(resolution.rule.pool_kind, PoolKind::MonopolyLimited);
        assert_eq!(resolution.rule.hard_pity_5, Some(90));
    }

    #[test]
    fn rule_for_record_falls_back_when_rule_id_is_missing() {
        let mut map = load_map("zh-Hant").expect("map should load");
        map.banners
            .get_mut("ForkLottery_AnHunQu")
            .expect("fork banner should exist")
            .rule_id = "missing_rule".to_string();

        let resolution = rule_for_record(
            &map,
            &record(
                "missing-rule",
                "ForkLottery_AnHunQu",
                "fork_Rose",
                "2026-01-01 00:00:00",
            ),
        )
        .expect("fallback rule should resolve");

        assert_eq!(
            resolution.resolution_issue,
            Some(RuleResolutionIssue::MissingRule)
        );
        assert_eq!(resolution.rule.pool_kind, PoolKind::ForkLottery);
        assert_eq!(resolution.rule.hard_pity_5, Some(60));
        assert_eq!(resolution.rule.hard_up_pity_5, Some(80));
    }

    #[test]
    fn standard_five_star_result_uses_item_kind() {
        let map = load_map("zh-Hant").expect("map should load");
        let banner = map.resolve_banner("CardPool_NewRole", None);

        for item_id in ["1003", "1023", "1054", "1055"] {
            assert_eq!(
                rate_up_result(
                    &map,
                    &record("character", "CardPool_NewRole", item_id, "2026-01-01 00:00:00"),
                    5,
                    &banner,
                ),
                RateUpResult::Up,
                "standard five-star character should be classified as UP: {item_id}"
            );
        }
        for item_id in ["fork_rishi", "DiceNormal"] {
            assert_eq!(
                rate_up_result(
                    &map,
                    &record("item", "CardPool_NewRole", item_id, "2026-01-01 00:00:00"),
                    5,
                    &banner,
                ),
                RateUpResult::NotApplicable,
                "standard non-character five-star should not use UP classification: {item_id}"
            );
        }
        assert_eq!(
            rate_up_result(
                &map,
                &record(
                    "unknown",
                    "CardPool_NewRole",
                    "UnknownItem",
                    "2026-01-01 00:00:00",
                ),
                5,
                &banner,
            ),
            RateUpResult::Unknown
        );
        assert_eq!(
            rate_up_result(
                &map,
                &record("four", "CardPool_NewRole", "1019", "2026-01-01 00:00:00"),
                4,
                &banner,
            ),
            RateUpResult::Unknown
        );
    }

    #[test]
    fn derive_pool_kind_hits_tracks_five_star_state() {
        let map = load_map("zh-Hant").expect("map should load");
        let records = vec![
            record(
                "r1",
                "ForkLottery_AnHunQu",
                "fork_Arachne",
                "2026-01-01 00:00:00",
            ),
            record(
                "r2",
                "ForkLottery_AnHunQu",
                "fork_jiaojuan",
                "2026-01-01 00:01:00",
            ),
            record(
                "r3",
                "ForkLottery_AnHunQu",
                "fork_Rose",
                "2026-01-01 00:02:00",
            ),
        ];

        let stats = derive_pool_kind_hits(&records, &map, PoolKind::ForkLottery)
            .expect("stats should derive");
        let derived = crate::derive_records(&records, &map).expect("records should derive");
        let derived_five_star_distances = derived
            .iter()
            .filter(|record| record.hit_rarity == Some(5))
            .map(|record| record.pity_5_before + 1)
            .collect::<Vec<_>>();

        assert_eq!(stats.total_pulls, 3);
        assert_eq!(stats.five_star_history.len(), 2);
        assert_eq!(
            stats
                .five_star_history
                .iter()
                .map(|hit| hit.pity_distance)
                .collect::<Vec<_>>(),
            derived_five_star_distances
        );
        assert_eq!(stats.five_star_history[0].result, RateUpResult::OffRate);
        assert_eq!(stats.five_star_history[0].guarantee_after, Some(true));
        assert_eq!(stats.five_star_history[1].result, RateUpResult::Up);
        assert_eq!(stats.five_star_history[1].guarantee_before, Some(true));
        assert_eq!(stats.summary_rule.resolution_issue, None);
    }
}
