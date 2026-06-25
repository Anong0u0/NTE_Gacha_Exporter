use std::collections::HashMap;

use crate::{
    GuiError, InternalRecord, MapData, MapItem, PityBadge, PoolKind, RateUpResult, RecordDerived,
};
use crate::{
    classify_pool_id, compare_records_for_analysis, rate_up_result, rule_for_resolved_banner,
    update_guarantee_state_for,
};

const NON_PULL_ROLL_LABEL_IDS: &[&str] = &[
    "BPUI_LotteryResult_jidianzengli",
    "BPUI_LotteryResult_chenmiandi",
];

pub fn derive_records(
    records: &[InternalRecord],
    map: &MapData,
) -> Result<Vec<RecordDerived>, GuiError> {
    let mut ordered = records.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| compare_records_for_analysis(left, right));

    let mut pull_no_by_pool_kind: HashMap<PoolKind, u64> = HashMap::new();
    let mut pull_no_by_banner: HashMap<String, u64> = HashMap::new();
    let mut global_pull_no = 0_u64;
    let mut pity_5_by_pool_kind: HashMap<PoolKind, u64> = HashMap::new();
    let mut fork_4_pity = 0_u64;
    let mut fork_up_pity = 0_u64;
    let mut guarantee_state: HashMap<(String, u8), bool> = HashMap::new();
    let mut derived = Vec::with_capacity(records.len());

    for record in ordered {
        let pool_kind = classify_pool_id(&record.pool_id)?;
        let banner = map.resolve_banner(&record.pool_id, record.time.as_deref());
        let rule = rule_for_resolved_banner(map, record, &banner)?;
        let counts_as_pull = counts_as_pull(record);
        let item = map.item(&record.item_id).map(|(_, item)| item);
        let rarity = item.map(|item| item.rarity);
        let hit_rarity = counts_as_pull
            .then(|| hit_rarity_for_pity(item, pool_kind))
            .flatten();

        let banner_id = if banner.resolution_issue.is_none() {
            banner.banner_id.clone()
        } else {
            None
        };
        let (global_pull_no, pull_no_in_pool_kind, pull_no_in_banner) = if counts_as_pull {
            global_pull_no += 1;
            let pool_pull_no = next_counter(&mut pull_no_by_pool_kind, pool_kind);
            let banner_pull_no = banner_id
                .as_ref()
                .map(|banner_id| next_counter(&mut pull_no_by_banner, banner_id.clone()));
            (Some(global_pull_no), Some(pool_pull_no), banner_pull_no)
        } else {
            (None, None, None)
        };

        let pity_5_before = *pity_5_by_pool_kind.get(&pool_kind).unwrap_or(&0);
        let rate_up = if counts_as_pull {
            rarity
                .filter(|rarity| matches!(rarity, 4 | 5))
                .map(|rarity| rate_up_result(map, record, rarity, &banner))
                .unwrap_or(RateUpResult::Unknown)
        } else {
            RateUpResult::Unknown
        };

        let (guarantee_5_before, guarantee_5_after) = if counts_as_pull {
            let result_5 = (hit_rarity == Some(5)).then_some(rate_up);
            update_guarantee_state_for(&mut guarantee_state, &rule, &banner, 5, result_5)
        } else {
            (None, None)
        };

        let fork_up_pity_before =
            (counts_as_pull && pool_kind == PoolKind::ForkLottery).then_some(fork_up_pity);
        let fork_4_pity_before =
            (counts_as_pull && pool_kind == PoolKind::ForkLottery).then_some(fork_4_pity);
        let fork_forced_up = fork_up_pity_before.and_then(|before| {
            if hit_rarity == Some(5) && rate_up == RateUpResult::Up {
                Some(
                    rule.rule
                        .hard_up_pity_5
                        .is_some_and(|hard_up_pity| before + 1 >= hard_up_pity),
                )
            } else if hit_rarity == Some(5) && rate_up == RateUpResult::OffRate {
                Some(false)
            } else {
                None
            }
        });

        let pity_5_state_after = if !counts_as_pull {
            pity_5_before
        } else if hit_rarity == Some(5) {
            0
        } else {
            pity_5_before + 1
        };
        if counts_as_pull {
            pity_5_by_pool_kind.insert(pool_kind, pity_5_state_after);
        }
        let pity_5_after = if counts_as_pull {
            pity_5_before + 1
        } else {
            pity_5_state_after
        };
        let pity_badge = pity_badge_for(PityBadgeContext {
            counts_as_pull,
            pool_kind,
            hit_rarity,
            rate_up,
            pity_5_before,
            fork_4_pity_before,
            fork_up_pity_before,
            hard_pity_5: rule.rule.hard_pity_5,
            hard_up_pity_5: rule.rule.hard_up_pity_5,
        });
        let (ten_pull_progress_before, ten_pull_progress_after) = if counts_as_pull {
            match pool_kind {
                PoolKind::ForkLottery => {
                    let progress_before = ten_pull_progress_before_from_pity(fork_4_pity);
                    let progress_after = if matches!(hit_rarity, Some(4 | 5)) {
                        0
                    } else {
                        ten_pull_progress_after_from_pity(fork_4_pity + 1)
                    };
                    if matches!(hit_rarity, Some(4 | 5)) {
                        fork_4_pity = 0;
                    } else {
                        fork_4_pity = (fork_4_pity + 1).min(9);
                    }
                    (Some(progress_before), Some(progress_after))
                }
                PoolKind::MonopolyLimited | PoolKind::MonopolyStandard => (
                    pull_no_in_pool_kind.map(ten_pull_progress_before),
                    pull_no_in_pool_kind.map(ten_pull_progress_after),
                ),
            }
        } else {
            (None, None)
        };
        let fork_up_pity_after = if counts_as_pull && pool_kind == PoolKind::ForkLottery {
            let public_after = fork_up_pity + 1;
            if hit_rarity == Some(5) && rate_up == RateUpResult::Up {
                fork_up_pity = 0;
            } else {
                fork_up_pity += 1;
            }
            Some(public_after)
        } else {
            None
        };

        derived.push(RecordDerived {
            record_id: record.record_id.clone(),
            banner_id,
            banner_version: banner.version.clone(),
            counts_as_pull,
            global_pull_no,
            pull_no_in_pool_kind,
            pull_no_in_banner,
            pity_5_before,
            pity_5_after,
            ten_pull_progress_before,
            ten_pull_progress_after,
            hit_rarity,
            rate_up_result: rate_up,
            pity_badge,
            guarantee_5_before,
            guarantee_5_after,
            fork_up_pity_before,
            fork_up_pity_after,
            fork_forced_up,
            rule: rule.view(),
        });
    }

    Ok(derived)
}

struct PityBadgeContext {
    counts_as_pull: bool,
    pool_kind: PoolKind,
    hit_rarity: Option<u8>,
    rate_up: RateUpResult,
    pity_5_before: u64,
    fork_4_pity_before: Option<u64>,
    fork_up_pity_before: Option<u64>,
    hard_pity_5: Option<u64>,
    hard_up_pity_5: Option<u64>,
}

fn pity_badge_for(context: PityBadgeContext) -> Option<PityBadge> {
    if !context.counts_as_pull || context.pool_kind != PoolKind::ForkLottery {
        return None;
    }
    if context.hit_rarity == Some(5)
        && context.rate_up == RateUpResult::Up
        && context
            .fork_up_pity_before
            .zip(context.hard_up_pity_5)
            .is_some_and(|(before, hard)| before + 1 >= hard)
    {
        return Some(PityBadge::ForkUpGuarantee);
    }
    if context.hit_rarity == Some(5)
        && context
            .hard_pity_5
            .is_some_and(|hard_pity| context.pity_5_before + 1 >= hard_pity)
    {
        return Some(PityBadge::ForkFiveStarGuarantee);
    }
    if matches!(context.hit_rarity, Some(4 | 5))
        && context
            .fork_4_pity_before
            .is_some_and(|before| before + 1 >= 10)
    {
        return Some(PityBadge::ForkFourStarGuarantee);
    }
    None
}

fn counts_as_pull(record: &InternalRecord) -> bool {
    !record
        .roll_label_id
        .as_deref()
        .is_some_and(|label_id| NON_PULL_ROLL_LABEL_IDS.contains(&label_id))
}

fn hit_rarity_for_pity(item: Option<&MapItem>, pool_kind: PoolKind) -> Option<u8> {
    let item = item?;
    if !matches!(item.rarity, 3..=5) {
        return None;
    }
    if item.rarity == 3 {
        return Some(3);
    }
    let expected_domain = match pool_kind {
        PoolKind::ForkLottery => "fork",
        PoolKind::MonopolyLimited | PoolKind::MonopolyStandard => "character",
    };
    let matches_domain = item.category.as_deref() == Some(expected_domain)
        || item.domain_type.as_deref() == Some(expected_domain);
    matches_domain.then_some(item.rarity)
}

fn next_counter<K>(counters: &mut HashMap<K, u64>, key: K) -> u64
where
    K: Eq + std::hash::Hash,
{
    let value = counters.entry(key).or_default();
    *value += 1;
    *value
}

fn ten_pull_progress_before(pull_no: u64) -> u8 {
    (((pull_no - 1) % 10) + 1) as u8
}

fn ten_pull_progress_after(pull_no: u64) -> u8 {
    (pull_no % 10) as u8
}

fn ten_pull_progress_before_from_pity(pity: u64) -> u8 {
    (pity + 1).min(10) as u8
}

fn ten_pull_progress_after_from_pity(pity: u64) -> u8 {
    pity.min(9) as u8
}

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
        assert_eq!(derived[0].pull_no_in_pool_kind, Some(1));
        assert_eq!(derived[1].pull_no_in_pool_kind, Some(2));
        assert_eq!(derived[2].pull_no_in_pool_kind, Some(3));
        assert_eq!(derived[3].pull_no_in_pool_kind, Some(4));
        assert_eq!(derived[1].pity_5_before, 1);
        assert_eq!(derived[1].pity_5_after, 2);
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
        assert_eq!(derived[0].hit_rarity, Some(5));
        assert_eq!(derived[1].hit_rarity, Some(5));
        assert_eq!(derived[2].hit_rarity, None);
        assert_eq!(derived[2].pity_5_after, 1);
    }

    #[test]
    fn four_star_hit_keeps_five_star_pity_progress() {
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
        assert_eq!(derived[1].pity_5_before, 1);
        assert_eq!(derived[1].pity_5_after, 2);
    }

    #[test]
    fn fork_item_in_standard_five_pool_uses_source_quality_for_pity() {
        let map = load_map("zh-Hant").expect("map should load");
        let records = vec![
            record(
                "three",
                "ForkLottery_Nanali",
                "fork_dustbin",
                "2026-01-01 00:00:00",
            ),
            record(
                "forgotten",
                "ForkLottery_Nanali",
                "fork_wuhuakuang",
                "2026-01-01 00:01:00",
            ),
        ];

        let derived = derive_records(&records, &map).expect("records should derive");

        assert_eq!(derived[1].hit_rarity, Some(4));
        assert_eq!(derived[1].pity_5_before, 1);
        assert_eq!(derived[1].pity_5_after, 2);
        assert_eq!(derived[1].ten_pull_progress_after, Some(0));
    }

    #[test]
    fn monopoly_ten_pull_progress_has_before_and_after_state() {
        let map = load_map("zh-Hant").expect("map should load");
        let limited_records = (0..11)
            .map(|index| {
                record(
                    &format!("limited-r{index}"),
                    "CardPool_Character",
                    "1003",
                    &format!("2026-05-13 05:{index:02}:00"),
                )
            })
            .collect::<Vec<_>>();
        let standard_records = (0..11)
            .map(|index| {
                record(
                    &format!("standard-r{index}"),
                    "CardPool_NewRole",
                    "1003",
                    &format!("2026-01-01 00:{index:02}:00"),
                )
            })
            .collect::<Vec<_>>();

        let limited = derive_records(&limited_records, &map).expect("records should derive");
        let standard = derive_records(&standard_records, &map).expect("records should derive");

        assert_eq!(limited[0].ten_pull_progress_before, Some(1));
        assert_eq!(limited[0].ten_pull_progress_after, Some(1));
        assert_eq!(limited[8].ten_pull_progress_before, Some(9));
        assert_eq!(limited[8].ten_pull_progress_after, Some(9));
        assert_eq!(limited[9].ten_pull_progress_before, Some(10));
        assert_eq!(limited[9].ten_pull_progress_after, Some(0));
        assert_eq!(limited[10].ten_pull_progress_before, Some(1));
        assert_eq!(limited[10].ten_pull_progress_after, Some(1));
        assert_eq!(
            standard
                .iter()
                .map(|record| (
                    record.ten_pull_progress_before,
                    record.ten_pull_progress_after
                ))
                .collect::<Vec<_>>(),
            limited
                .iter()
                .map(|record| (
                    record.ten_pull_progress_before,
                    record.ten_pull_progress_after
                ))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn fork_ten_pull_progress_resets_on_four_star() {
        let map = load_map("zh-Hant").expect("map should load");
        let mut records = Vec::new();
        for index in 0..9 {
            records.push(record(
                &format!("r{index}"),
                "ForkLottery_AnHunQu",
                "fork_dustbin",
                &format!("2026-01-01 00:{index:02}:00"),
            ));
        }
        records.push(record(
            "r9",
            "ForkLottery_AnHunQu",
            "fork_jiaojuan",
            "2026-01-01 00:09:00",
        ));

        let derived = derive_records(&records, &map).expect("records should derive");

        assert_eq!(derived[8].ten_pull_progress_before, Some(9));
        assert_eq!(derived[8].ten_pull_progress_after, Some(9));
        assert_eq!(derived[9].hit_rarity, Some(4));
        assert_eq!(derived[9].ten_pull_progress_before, Some(10));
        assert_eq!(derived[9].ten_pull_progress_after, Some(0));
        assert_eq!(
            derived[9].pity_badge,
            Some(PityBadge::ForkFourStarGuarantee)
        );
    }

    #[test]
    fn fork_ten_pull_progress_resets_on_five_star() {
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
                "fork_Rose",
                "2026-01-01 00:01:00",
            ),
        ];

        let derived = derive_records(&records, &map).expect("records should derive");

        assert_eq!(derived[0].ten_pull_progress_before, Some(1));
        assert_eq!(derived[0].ten_pull_progress_after, Some(1));
        assert_eq!(derived[1].hit_rarity, Some(5));
        assert_eq!(derived[1].ten_pull_progress_before, Some(2));
        assert_eq!(derived[1].ten_pull_progress_after, Some(0));
    }

    #[test]
    fn non_pull_rows_do_not_advance_ten_pull_progress() {
        let map = load_map("zh-Hant").expect("map should load");
        let mut records = Vec::new();
        for index in 0..11 {
            records.push(record(
                &format!("r{index}"),
                "ForkLottery_AnHunQu",
                "fork_dustbin",
                &format!("2026-01-01 00:{index:02}:00"),
            ));
        }
        records[5].roll_label_id = Some("BPUI_LotteryResult_jidianzengli".to_string());

        let derived = derive_records(&records, &map).expect("records should derive");

        assert_eq!(derived[4].ten_pull_progress_before, Some(5));
        assert_eq!(derived[4].ten_pull_progress_after, Some(5));
        assert_eq!(derived[5].ten_pull_progress_before, None);
        assert_eq!(derived[5].ten_pull_progress_after, None);
        assert_eq!(derived[10].ten_pull_progress_before, Some(10));
        assert_eq!(derived[10].ten_pull_progress_after, Some(9));
    }

    #[test]
    fn fork_pity_badge_priority_prefers_up_then_five_then_four_star() {
        let map = load_map("zh-Hant").expect("map should load");
        let mut four_star_records = Vec::new();
        for index in 0..9 {
            four_star_records.push(record(
                &format!("four-r{index}"),
                "ForkLottery_AnHunQu",
                "fork_dustbin",
                &format!("2026-01-01 00:{index:02}:00"),
            ));
        }
        four_star_records.push(record(
            "four-hit",
            "ForkLottery_AnHunQu",
            "fork_jiaojuan",
            "2026-01-01 00:09:00",
        ));
        let four_star = derive_records(&four_star_records, &map).expect("records should derive");

        let mut five_star_records = Vec::new();
        for index in 0..59 {
            five_star_records.push(record(
                &format!("five-r{index}"),
                "ForkLottery_AnHunQu",
                "fork_dustbin",
                &format!("2026-01-01 01:{index:02}:00"),
            ));
        }
        five_star_records.push(record(
            "five-hit",
            "ForkLottery_AnHunQu",
            "fork_Arachne",
            "2026-01-01 02:00:00",
        ));
        let five_star = derive_records(&five_star_records, &map).expect("records should derive");

        let mut up_records = Vec::new();
        for index in 0..79 {
            up_records.push(record(
                &format!("up-r{index}"),
                "ForkLottery_AnHunQu",
                "fork_dustbin",
                &format!("2026-01-01 03:{:02}:00", index % 60),
            ));
        }
        up_records.push(record(
            "up-hit",
            "ForkLottery_AnHunQu",
            "fork_Rose",
            "2026-01-01 04:00:00",
        ));
        let up = derive_records(&up_records, &map).expect("records should derive");

        assert_eq!(
            four_star
                .last()
                .and_then(|record| record.pity_badge.clone()),
            Some(PityBadge::ForkFourStarGuarantee)
        );
        assert_eq!(
            five_star
                .last()
                .and_then(|record| record.pity_badge.clone()),
            Some(PityBadge::ForkFiveStarGuarantee)
        );
        assert_eq!(
            up.last().and_then(|record| record.pity_badge.clone()),
            Some(PityBadge::ForkUpGuarantee)
        );
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
    }

    #[test]
    fn same_timestamp_uses_source_order_tiebreaker() {
        let map = load_map("zh-Hant").expect("map should load");
        let mut first = record(
            "same-b",
            "ForkLottery_AnHunQu",
            "DiceNormal",
            "2026-01-01 00:00:00",
        );
        first.source_order = 1;
        let mut second = record(
            "same-a",
            "ForkLottery_AnHunQu",
            "fork_dustbin",
            "2026-01-01 00:00:00",
        );
        second.source_order = 0;
        let records = vec![first, second];

        let derived = derive_records(&records, &map).expect("records should derive");

        assert_eq!(
            derived
                .iter()
                .map(|record| record.record_id.as_str())
                .collect::<Vec<_>>(),
            vec!["same-a", "same-b"]
        );
        assert_eq!(derived[0].pull_no_in_pool_kind, Some(1));
        assert_eq!(derived[1].pull_no_in_pool_kind, Some(2));
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
        assert_eq!(derived[0].pull_no_in_pool_kind, Some(1));
        assert_eq!(derived[0].pity_5_after, 1);
    }

    #[test]
    fn sentinel_rows_stay_visible_but_do_not_advance_pull_state() {
        let map = load_map("zh-Hant").expect("map should load");
        let mut sentinel = record(
            "sentinel",
            "CardPool_Character",
            "1010",
            "2026-05-13 05:58:30",
        );
        sentinel.roll_label_id = Some("BPUI_LotteryResult_jidianzengli".to_string());
        sentinel.roll_points = None;
        let records = vec![
            record("first", "CardPool_Character", "1003", "2026-05-13 05:58:00"),
            sentinel,
            record("after", "CardPool_Character", "1004", "2026-05-13 05:59:00"),
        ];

        let derived = derive_records(&records, &map).expect("records should derive");

        assert_eq!(derived[0].record_id, "first");
        assert_eq!(derived[0].pull_no_in_pool_kind, Some(1));
        assert!(derived[0].counts_as_pull);
        assert_eq!(derived[1].record_id, "sentinel");
        assert!(!derived[1].counts_as_pull);
        assert_eq!(derived[1].pull_no_in_pool_kind, None);
        assert_eq!(derived[1].pull_no_in_banner, None);
        assert_eq!(derived[1].hit_rarity, None);
        assert_eq!(derived[1].pity_5_before, 0);
        assert_eq!(derived[1].pity_5_after, 0);
        assert_eq!(derived[1].ten_pull_progress_before, None);
        assert_eq!(derived[1].ten_pull_progress_after, None);
        assert_eq!(derived[2].pull_no_in_pool_kind, Some(2));
        assert_eq!(derived[2].pity_5_before, 0);
        assert_eq!(derived[2].ten_pull_progress_before, Some(2));
        assert_eq!(derived[2].ten_pull_progress_after, Some(2));
    }

    #[test]
    fn monopoly_pity_resets_only_on_character_hits() {
        let map = load_map("zh-Hant").expect("map should load");
        let records = vec![
            record(
                "vehicle",
                "CardPool_NewRole",
                "Fashion_vehicle_1010_V008",
                "2026-01-01 00:00:00",
            ),
            record(
                "fork",
                "CardPool_NewRole",
                "fork_Rose",
                "2026-01-01 00:01:00",
            ),
            record(
                "character",
                "CardPool_NewRole",
                "1010",
                "2026-01-01 00:02:00",
            ),
        ];

        let derived = derive_records(&records, &map).expect("records should derive");

        assert_eq!(derived[0].hit_rarity, None);
        assert_eq!(derived[0].pity_5_after, 1);
        assert_eq!(derived[1].hit_rarity, None);
        assert_eq!(derived[1].pity_5_after, 2);
        assert_eq!(derived[2].hit_rarity, Some(5));
        assert_eq!(derived[2].pity_5_before, 2);
        assert_eq!(derived[2].pity_5_after, 3);
    }

    #[test]
    fn fork_pity_resets_only_on_fork_hits() {
        let map = load_map("zh-Hant").expect("map should load");
        let records = vec![
            record(
                "character",
                "ForkLottery_AnHunQu",
                "1010",
                "2026-01-01 00:00:00",
            ),
            record(
                "fork",
                "ForkLottery_AnHunQu",
                "fork_Rose",
                "2026-01-01 00:01:00",
            ),
        ];

        let derived = derive_records(&records, &map).expect("records should derive");

        assert_eq!(derived[0].hit_rarity, None);
        assert_eq!(derived[0].pity_5_after, 1);
        assert_eq!(derived[1].hit_rarity, Some(5));
        assert_eq!(derived[1].pity_5_before, 1);
        assert_eq!(derived[1].pity_5_after, 2);
    }
}
