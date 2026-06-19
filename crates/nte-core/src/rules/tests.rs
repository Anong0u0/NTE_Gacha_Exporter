#[cfg(test)]
mod tests {
    use super::*;
    use crate::load_map;

    fn record(record_id: &str, pool_id: &str, item_id: &str, time: &str) -> InternalRecord {
        InternalRecord {
            record_id: record_id.to_string(),
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
            secondary_item_id: None,
            secondary_count: None,
        }
    }

    #[test]
    fn rule_for_record_uses_matched_banner_rule() {
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
        assert_eq!(fork.status, RuleResolutionStatus::Matched);
        assert_eq!(fork.rule.rule_id.as_deref(), Some("fork_lottery_s"));
        assert_eq!(fork.rule.hard_pity_5, Some(80));
        assert_eq!(fork.rule.pickup_win_rate_5, Some(25));
        assert_eq!(fork.rule.has_guarantee_5, Some(true));
        assert_eq!(fork.rule.source_confidence.as_deref(), Some("exact"));

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
        assert_eq!(standard.status, RuleResolutionStatus::Matched);
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
        assert_eq!(limited.status, RuleResolutionStatus::Matched);
        assert_eq!(limited.rule.rule_id.as_deref(), Some("monopoly_limited"));
        assert_eq!(limited.rule.source_confidence.as_deref(), Some("curated"));
    }

    #[test]
    fn rule_for_record_falls_back_when_banner_is_unmatched() {
        let map = load_map("zh-Hant").expect("map should load");

        let resolution = rule_for_record(
            &map,
            &record(
                "outside",
                "CardPool_Character",
                "fork_dustbin",
                "2026-07-08 05:59:01",
            ),
        )
        .expect("fallback rule should resolve");

        assert_eq!(resolution.status, RuleResolutionStatus::MissingBanner);
        assert_eq!(resolution.rule.pool_kind, PoolKind::MonopolyLimited);
        assert_eq!(resolution.rule.hard_pity_5, Some(90));
        assert_eq!(
            resolution.rule.source_confidence.as_deref(),
            Some("unknown")
        );
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

        assert_eq!(resolution.status, RuleResolutionStatus::MissingRule);
        assert_eq!(resolution.rule.pool_kind, PoolKind::ForkLottery);
        assert_eq!(resolution.rule.hard_pity_5, Some(80));
    }

    #[test]
    fn derive_pool_kind_hits_tracks_five_and_four_star_state() {
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
        assert_eq!(stats.four_star_history.len(), 1);
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
        assert_eq!(stats.current_4star_pity, 1);
        assert_eq!(stats.summary_rule.status, RuleResolutionStatus::Matched);
    }
}
