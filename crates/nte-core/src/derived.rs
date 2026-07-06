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

        let banner_id = banner.banner_id.clone();
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
        || item.subtype.as_deref() == Some(expected_domain);
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
mod tests;
