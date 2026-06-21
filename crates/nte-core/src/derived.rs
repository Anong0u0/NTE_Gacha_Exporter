use std::collections::HashMap;

use crate::MapData;
use crate::{
    BannerResolutionStatus, GuiError, InternalRecord, PoolKind, RateUpResult, RecordDerived,
};
use crate::{
    classify_pool_id, rate_up_result, rule_for_resolved_banner, update_guarantee_state_for,
};

pub fn derive_records(
    records: &[InternalRecord],
    map: &MapData,
) -> Result<Vec<RecordDerived>, GuiError> {
    let mut ordered = records.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        left.time
            .cmp(&right.time)
            .then_with(|| left.record_id.cmp(&right.record_id))
    });

    let mut pull_no_by_pool_kind: HashMap<PoolKind, u64> = HashMap::new();
    let mut pull_no_by_banner: HashMap<String, u64> = HashMap::new();
    let mut pity_5_by_pool_kind: HashMap<PoolKind, u64> = HashMap::new();
    let mut pity_4_by_pool_kind: HashMap<PoolKind, u64> = HashMap::new();
    let mut guarantee_state: HashMap<(String, u8), bool> = HashMap::new();
    let mut derived = Vec::with_capacity(records.len());

    for record in ordered {
        let pool_kind = classify_pool_id(&record.pool_id)?;
        let banner = map.resolve_banner(&record.pool_id, record.time.as_deref());
        let rule = rule_for_resolved_banner(map, record, &banner)?;

        let pull_no_in_pool_kind = next_counter(&mut pull_no_by_pool_kind, pool_kind);
        let matched_banner_id = if banner.status == BannerResolutionStatus::Matched {
            banner.banner_id.clone()
        } else {
            None
        };
        let pull_no_in_banner = matched_banner_id
            .as_ref()
            .map(|banner_id| next_counter(&mut pull_no_by_banner, banner_id.clone()));

        let pity_5_before = *pity_5_by_pool_kind.get(&pool_kind).unwrap_or(&0);
        let pity_4_before = *pity_4_by_pool_kind.get(&pool_kind).unwrap_or(&0);
        let rarity = map.item_rarity(&record.item_id);
        let hit_rarity = rarity.filter(|rarity| matches!(rarity, 4 | 5));
        let rate_up = hit_rarity
            .map(|rarity| rate_up_result(map, record, rarity, &banner))
            .unwrap_or(RateUpResult::Unknown);

        let result_5 = (hit_rarity == Some(5)).then_some(rate_up);
        let result_4 = (hit_rarity == Some(4)).then_some(rate_up);
        let (guarantee_5_before, guarantee_5_after) =
            update_guarantee_state_for(&mut guarantee_state, &rule, &banner, 5, result_5);
        let (guarantee_4_before, guarantee_4_after) =
            update_guarantee_state_for(&mut guarantee_state, &rule, &banner, 4, result_4);

        let pity_5_after = if hit_rarity == Some(5) {
            0
        } else {
            pity_5_before + 1
        };
        let pity_4_after = if hit_rarity == Some(4) {
            0
        } else {
            pity_4_before + 1
        };
        pity_5_by_pool_kind.insert(pool_kind, pity_5_after);
        pity_4_by_pool_kind.insert(pool_kind, pity_4_after);

        derived.push(RecordDerived {
            record_id: record.record_id.clone(),
            banner_id: matched_banner_id,
            banner_version: banner.version.clone(),
            pull_no_in_pool_kind,
            pull_no_in_banner,
            pity_5_before,
            pity_5_after,
            pity_4_before,
            pity_4_after,
            hit_rarity,
            rate_up_result: rate_up,
            guarantee_5_before,
            guarantee_5_after,
            guarantee_4_before,
            guarantee_4_after,
            rule: rule.view(),
        });
    }

    Ok(derived)
}

fn next_counter<K>(counters: &mut HashMap<K, u64>, key: K) -> u64
where
    K: Eq + std::hash::Hash,
{
    let value = counters.entry(key).or_default();
    *value += 1;
    *value
}

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
    fn fork_sequence_tracks_pull_pity_rate_up_and_guarantee() {
        let map = load_map("zh-Hant").expect("map should load");
        let records = vec![
            record(
                "r4",
                "ForkLottery_AnHunQu",
                "fork_Rose",
                "2026-01-01 00:03:00",
            ),
            record(
                "r1",
                "ForkLottery_AnHunQu",
                "fork_dustbin",
                "2026-01-01 00:00:00",
            ),
            record(
                "r2",
                "ForkLottery_AnHunQu",
                "fork_Arachne",
                "2026-01-01 00:01:00",
            ),
            record(
                "r3",
                "ForkLottery_AnHunQu",
                "fork_dustbin",
                "2026-01-01 00:02:00",
            ),
        ];

        let derived = derive_records(&records, &map).expect("records should derive");

        assert_eq!(
            derived
                .iter()
                .map(|record| record.record_id.as_str())
                .collect::<Vec<_>>(),
            vec!["r1", "r2", "r3", "r4"]
        );
        assert_eq!(derived[0].pull_no_in_pool_kind, 1);
        assert_eq!(derived[1].pull_no_in_pool_kind, 2);
        assert_eq!(derived[2].pull_no_in_pool_kind, 3);
        assert_eq!(derived[3].pull_no_in_pool_kind, 4);
        assert_eq!(derived[1].pity_5_before, 1);
        assert_eq!(derived[1].pity_5_after, 0);
        assert_eq!(derived[1].rate_up_result, RateUpResult::OffRate);
        assert_eq!(derived[1].guarantee_5_before, Some(false));
        assert_eq!(derived[1].guarantee_5_after, Some(true));
        assert_eq!(derived[3].pity_5_before, 1);
        assert_eq!(derived[3].rate_up_result, RateUpResult::Up);
        assert_eq!(derived[3].guarantee_5_before, Some(true));
        assert_eq!(derived[3].guarantee_5_after, Some(false));
    }

    #[test]
    fn limited_rate_up_applies_to_character_domain_only() {
        let map = load_map("zh-Hant").expect("map should load");
        let records = vec![
            record("up", "CardPool_Character", "1010", "2026-05-13 05:57:00"),
            record("off", "CardPool_Character", "1003", "2026-05-13 05:58:00"),
            record(
                "vehicle",
                "CardPool_Character",
                "Fashion_vehicle_1010_V008",
                "2026-05-13 05:59:00",
            ),
        ];

        let derived = derive_records(&records, &map).expect("records should derive");

        assert_eq!(derived[0].rate_up_result, RateUpResult::Up);
        assert_eq!(derived[1].rate_up_result, RateUpResult::OffRate);
        assert_eq!(derived[2].rate_up_result, RateUpResult::NotApplicable);
    }

    #[test]
    fn four_star_hit_resets_only_four_star_pity() {
        let map = load_map("zh-Hant").expect("map should load");
        let records = vec![
            record(
                "r1",
                "ForkLottery_AnHunQu",
                "fork_dustbin",
                "2026-01-01 00:00:00",
            ),
            record(
                "r2",
                "ForkLottery_AnHunQu",
                "fork_jiaojuan",
                "2026-01-01 00:01:00",
            ),
        ];

        let derived = derive_records(&records, &map).expect("records should derive");

        assert_eq!(derived[1].hit_rarity, Some(4));
        assert_eq!(derived[1].pity_4_before, 1);
        assert_eq!(derived[1].pity_4_after, 0);
        assert_eq!(derived[1].pity_5_before, 1);
        assert_eq!(derived[1].pity_5_after, 2);
    }

    #[test]
    fn missing_rarity_record_stays_and_increments_pity() {
        let map = load_map("zh-Hant").expect("map should load");
        let records = vec![record(
            "missing",
            "CardPool_Character",
            "UnknownItem",
            "2026-05-13 05:59:00",
        )];

        let derived = derive_records(&records, &map).expect("records should derive");

        assert_eq!(derived.len(), 1);
        assert_eq!(derived[0].hit_rarity, None);
        assert_eq!(derived[0].rate_up_result, RateUpResult::Unknown);
        assert_eq!(derived[0].pity_5_after, 1);
        assert_eq!(derived[0].pity_4_after, 1);
    }

    #[test]
    fn same_timestamp_uses_record_id_tiebreaker() {
        let map = load_map("zh-Hant").expect("map should load");
        let records = vec![
            record(
                "same-b",
                "ForkLottery_AnHunQu",
                "DiceNormal",
                "2026-01-01 00:00:00",
            ),
            record(
                "same-a",
                "ForkLottery_AnHunQu",
                "fork_dustbin",
                "2026-01-01 00:00:00",
            ),
        ];

        let derived = derive_records(&records, &map).expect("records should derive");

        assert_eq!(
            derived
                .iter()
                .map(|record| record.record_id.as_str())
                .collect::<Vec<_>>(),
            vec!["same-a", "same-b"]
        );
        assert_eq!(derived[0].pull_no_in_pool_kind, 1);
        assert_eq!(derived[1].pull_no_in_pool_kind, 2);
        assert_eq!(derived[1].pity_5_before, 1);
    }

    #[test]
    fn limited_boundaries_have_independent_banner_pull_numbers() {
        let map = load_map("zh-Hant").expect("map should load");
        let records = vec![
            record(
                "nanali",
                "CardPool_Character",
                "Fashion_vehicle_1010_V008",
                "2026-05-13 05:59:00",
            ),
            record(
                "xun",
                "CardPool_Character",
                "Fashion_vehicle_1052_V024",
                "2026-05-13 05:59:01",
            ),
        ];

        let derived = derive_records(&records, &map).expect("records should derive");

        assert_eq!(
            derived[0].banner_id.as_deref(),
            Some("monopoly_limited_Nanali")
        );
        assert_eq!(derived[0].pull_no_in_banner, Some(1));
        assert_eq!(
            derived[1].banner_id.as_deref(),
            Some("monopoly_limited_Xun")
        );
        assert_eq!(derived[1].pull_no_in_banner, Some(1));
    }

    #[test]
    fn outside_known_limited_window_keeps_pool_kind_state_without_banner() {
        let map = load_map("zh-Hant").expect("map should load");
        let records = vec![record(
            "outside",
            "CardPool_Character",
            "fork_dustbin",
            "2026-07-08 05:59:01",
        )];

        let derived = derive_records(&records, &map).expect("records should derive");

        assert_eq!(derived[0].banner_id, None);
        assert_eq!(derived[0].pull_no_in_banner, None);
        assert_eq!(derived[0].pull_no_in_pool_kind, 1);
        assert_eq!(derived[0].pity_5_after, 1);
    }
}
