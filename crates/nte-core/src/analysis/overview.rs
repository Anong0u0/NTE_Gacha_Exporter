use std::collections::{BTreeMap, HashMap};

use crate::MapData;
use crate::RuleResolutionStatus;
use crate::derive_records;
use crate::{
    BannerSummary, DashboardOverview, DashboardSelection, DashboardSelectionDetail,
    DisplayRecord, FiveStarRecord, FiveStarResult, FourStarRecord, GuiError, ImportReport,
    InternalRecord, ItemRank, PoolKind, PoolKindDetail, PoolKindSummary, Profile, RarityBucket,
    RateUpResult, RecordBannerOption, RecordDerived, RecordFilter, RecordFilterOptions,
    RecordList, RecordPoolOption, RecordSortKey, RecordTypeOption, SortDirection,
    TimeBucketSummary, TimeStats,
};
use crate::{classify_pool_id, fallback_rule_for, fallback_rule_resolution};

pub fn dashboard_overview(
    profile: Profile,
    last_run: Option<ImportReport>,
    records: &[InternalRecord],
    map: &MapData,
) -> Result<DashboardOverview, GuiError> {
    let display_records = display_records(records, map)?;
    let mut pool_kinds = Vec::new();
    for kind in [
        PoolKind::MonopolyLimited,
        PoolKind::MonopolyStandard,
        PoolKind::ForkLottery,
    ] {
        let detail = pool_kind_detail_from_display_records(&display_records, map, kind);
        pool_kinds.push(detail.summary);
    }
    Ok(DashboardOverview {
        profile,
        last_run,
        total_records: records.len() as u64,
        pool_kinds,
        banners: banner_summaries(&display_records),
        time_stats: time_stats(&display_records),
        rarity_distribution: rarity_distribution(records, map),
        item_ranking: item_ranking(records, map),
    })
}

pub fn pool_kind_detail(
    records: &[InternalRecord],
    map: &MapData,
    pool_kind: PoolKind,
) -> Result<PoolKindDetail, GuiError> {
    let display_records = display_records(records, map)?;
    Ok(selection_detail_from_display_records(
        &display_records,
        map,
        pool_kind,
        map.pool_kind_label(pool_kind),
        None,
    )
    .into())
}

pub fn dashboard_selection_detail(
    records: &[InternalRecord],
    map: &MapData,
    selection: &DashboardSelection,
) -> Result<DashboardSelectionDetail, GuiError> {
    let display_records = display_records(records, map)?;
    Ok(match selection {
        DashboardSelection::PoolKind { pool_kind } => selection_detail_from_display_records(
            &display_records,
            map,
            *pool_kind,
            map.pool_kind_label(*pool_kind),
            None,
        ),
        DashboardSelection::Banner {
            pool_kind,
            banner_id,
        } => {
            let label = display_records
                .iter()
                .find(|record| record.derived.banner_id.as_deref() == Some(banner_id.as_str()))
                .and_then(|record| record.banner.title.clone())
                .unwrap_or_else(|| banner_id.clone());
            selection_detail_from_display_records(
                &display_records,
                map,
                *pool_kind,
                label,
                Some(banner_id.as_str()),
            )
        }
    })
}

impl From<DashboardSelectionDetail> for PoolKindDetail {
    fn from(value: DashboardSelectionDetail) -> Self {
        Self {
            summary: value.summary,
            five_star_history: value.five_star_history,
            four_star_history: value.four_star_history,
        }
    }
}

fn pool_kind_detail_from_display_records(
    records: &[DisplayRecord],
    map: &MapData,
    pool_kind: PoolKind,
) -> PoolKindDetail {
    selection_detail_from_display_records(
        records,
        map,
        pool_kind,
        map.pool_kind_label(pool_kind),
        None,
    )
    .into()
}

fn selection_detail_from_display_records(
    records: &[DisplayRecord],
    map: &MapData,
    pool_kind: PoolKind,
    label: String,
    banner_id: Option<&str>,
) -> DashboardSelectionDetail {
    let pool_records = records
        .iter()
        .filter(|record| record.pool_kind == pool_kind)
        .filter(|record| {
            banner_id.is_none_or(|banner_id| record.derived.banner_id.as_deref() == Some(banner_id))
        })
        .collect::<Vec<_>>();
    let five_star_history = pool_records
        .iter()
        .filter(|record| record.derived.hit_rarity == Some(5))
        .map(|record| five_star_record(record))
        .collect::<Vec<_>>();
    let four_star_history = pool_records
        .iter()
        .filter(|record| record.derived.hit_rarity == Some(4))
        .map(|record| four_star_record(record))
        .collect::<Vec<_>>();

    let pity_distances = five_star_history
        .iter()
        .map(|record| record.pity_distance)
        .collect::<Vec<_>>();
    let four_star_distances = four_star_history
        .iter()
        .map(|record| record.pity_distance)
        .collect::<Vec<_>>();
    let hit_count = five_star_history.len() as u64;
    let four_star_count = four_star_history.len() as u64;
    let average_5star_pity = (!pity_distances.is_empty())
        .then(|| pity_distances.iter().sum::<u64>() as f64 / pity_distances.len() as f64);
    let min_5star_pity = pity_distances.iter().min().copied();
    let max_5star_pity = pity_distances.iter().max().copied();
    let average_4star_pity = (!four_star_distances.is_empty())
        .then(|| four_star_distances.iter().sum::<u64>() as f64 / four_star_distances.len() as f64);
    let min_4star_pity = four_star_distances.iter().min().copied();
    let max_4star_pity = four_star_distances.iter().max().copied();

    let up_count = count_rate_up(&five_star_history, RateUpResult::Up);
    let off_rate_count = count_rate_up(&five_star_history, RateUpResult::OffRate);
    let not_applicable_rate_up_count =
        count_rate_up(&five_star_history, RateUpResult::NotApplicable);
    let unknown_rate_up_count = count_rate_up(&five_star_history, RateUpResult::Unknown);
    let rate_up_4_count = count_rate_up_4(&four_star_history, RateUpResult::Up);
    let off_rate_4_count = count_rate_up_4(&four_star_history, RateUpResult::OffRate);
    let not_applicable_rate_up_4_count =
        count_rate_up_4(&four_star_history, RateUpResult::NotApplicable);
    let unknown_rate_up_4_count = count_rate_up_4(&four_star_history, RateUpResult::Unknown);
    let summary_rule = pool_records
        .first()
        .map(|record| record.derived.rule.clone())
        .unwrap_or_else(|| {
            fallback_rule_resolution(
                pool_kind,
                RuleResolutionStatus::FallbackPoolKind,
                "pool has no records; using pool-kind fallback",
            )
            .view()
        });
    let hard_pity = summary_rule
        .hard_pity_5
        .or_else(|| fallback_rule_for(pool_kind).hard_pity_5)
        .unwrap_or_default();
    let early_hit_count = pity_distances
        .iter()
        .filter(|distance| **distance < hard_pity)
        .count() as u64;
    let rate_up_sample_count = up_count + off_rate_count;
    let observed_up_rate =
        (rate_up_sample_count > 0).then(|| up_count as f64 / rate_up_sample_count as f64);
    let latest_5star = five_star_history.last().map(|hit| hit.record.clone());
    let latest = pool_records.last();
    let resource = resource_counters(pool_records.iter().copied());
    let roll_point_costs = roll_point_costs_to_hits(pool_records.iter().copied());

    DashboardSelectionDetail {
        summary: PoolKindSummary {
            pool_kind,
            label,
            total_pulls: pool_records.len() as u64,
            roll_points_total: resource.total,
            known_roll_point_records: resource.known,
            missing_roll_point_records: resource.missing,
            hit_count,
            current_pity: latest
                .map(|record| record.derived.pity_5_after)
                .unwrap_or_default(),
            current_guarantee: latest
                .and_then(|record| record.derived.guarantee_5_after)
                .unwrap_or(false),
            hard_pity,
            average_5star_pity,
            min_5star_pity,
            max_5star_pity,
            early_hit_count,
            up_count,
            off_rate_count,
            not_applicable_rate_up_count,
            unknown_rate_up_count,
            observed_up_rate,
            latest_5star,
            current_4star_pity: latest
                .map(|record| record.derived.pity_4_after)
                .unwrap_or_default(),
            hard_pity_4: summary_rule.hard_pity_4,
            average_4star_pity,
            min_4star_pity,
            max_4star_pity,
            four_star_count,
            rate_up_4_count,
            off_rate_4_count,
            not_applicable_rate_up_4_count,
            unknown_rate_up_4_count,
            rule_resolution_status: summary_rule.status,
            average_roll_points_to_5star: average_i64(&roll_point_costs.five_star),
            average_roll_points_to_4star: average_i64(&roll_point_costs.four_star),
            roll_point_cost_samples_5star: roll_point_costs.five_star.len() as u64,
            roll_point_cost_samples_4star: roll_point_costs.four_star.len() as u64,
        },
        five_star_history,
        four_star_history,
        rarity_distribution: rarity_distribution_from_display(&pool_records),
        item_ranking: item_ranking_from_display(&pool_records, map),
    }
}

fn five_star_record(record: &DisplayRecord) -> FiveStarRecord {
    FiveStarRecord {
        record: record.clone(),
        pity_distance: record.derived.pity_5_before + 1,
        result: five_star_result(record.derived.rate_up_result),
        guarantee_before: record.derived.guarantee_5_before,
        guarantee_after: record.derived.guarantee_5_after,
    }
}

fn four_star_record(record: &DisplayRecord) -> FourStarRecord {
    FourStarRecord {
        record: record.clone(),
        pity_distance: record.derived.pity_4_before + 1,
        result: record.derived.rate_up_result,
        guarantee_before: record.derived.guarantee_4_before,
        guarantee_after: record.derived.guarantee_4_after,
    }
}

fn five_star_result(result: RateUpResult) -> FiveStarResult {
    match result {
        RateUpResult::Up => FiveStarResult::Up,
        RateUpResult::OffRate => FiveStarResult::OffRate,
        RateUpResult::NotApplicable => FiveStarResult::NotApplicable,
        RateUpResult::Unknown => FiveStarResult::Unknown,
    }
}

fn count_rate_up(history: &[FiveStarRecord], result: RateUpResult) -> u64 {
    history
        .iter()
        .filter(|hit| hit.record.derived.rate_up_result == result)
        .count() as u64
}

fn count_rate_up_4(history: &[FourStarRecord], result: RateUpResult) -> u64 {
    history.iter().filter(|hit| hit.result == result).count() as u64
}
