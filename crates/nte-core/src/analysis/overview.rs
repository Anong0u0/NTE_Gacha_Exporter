use std::collections::{BTreeMap, HashMap};

use crate::MapData;
use crate::RuleResolutionIssue;
use crate::derive_records;
use crate::{
    BannerSummary, DashboardOverview, DashboardSelection, DashboardSelectionDetail,
    DisplayRecord, FiveStarRecord, FiveStarResult, ForkResultMark, GuiError, ImportReport,
    InternalRecord, ItemKind, ItemRank, PoolKind, PoolKindDetail, PoolKindSummary, Profile,
    ProfileAnalysisView, PullRarityBucket, PullRarityBucketKey, RarityBucket, RateUpResult,
    RecordBannerOption, RecordDerived, RecordFilter, RecordFilterOptions, RecordItemKindOption,
    RecordList, RecordRollBucketOption, RollBucket, SortDirection, TimeBucketSummary, TimeStats,
};
use crate::{classify_pool_id, fallback_rule_for, fallback_rule_resolution};
use crate::{compare_display_chronological, compare_display_newest_first};

pub fn dashboard_overview(
    profile: Profile,
    last_run: Option<ImportReport>,
    records: &[InternalRecord],
    map: &MapData,
) -> Result<DashboardOverview, GuiError> {
    Ok(AnalysisSnapshot::new(records, map)?.dashboard_overview(profile, last_run))
}

pub fn pool_kind_detail(
    records: &[InternalRecord],
    map: &MapData,
    pool_kind: PoolKind,
) -> Result<PoolKindDetail, GuiError> {
    Ok(AnalysisSnapshot::new(records, map)?
        .pool_kind_detail(pool_kind)
        .into())
}

pub fn dashboard_selection_detail(
    records: &[InternalRecord],
    map: &MapData,
    selection: &DashboardSelection,
) -> Result<DashboardSelectionDetail, GuiError> {
    Ok(AnalysisSnapshot::new(records, map)?.dashboard_selection_detail(selection))
}

pub fn profile_analysis_view(
    profile: Profile,
    last_run: Option<ImportReport>,
    records: &[InternalRecord],
    map: &MapData,
    selection: &DashboardSelection,
    record_filter: &RecordFilter,
) -> Result<ProfileAnalysisView, GuiError> {
    Ok(AnalysisSnapshot::new(records, map)?
        .profile_analysis_view(profile, last_run, selection, record_filter))
}

struct AnalysisSnapshot<'a> {
    records: Vec<DisplayRecord>,
    map: &'a MapData,
}

impl<'a> AnalysisSnapshot<'a> {
    fn new(records: &[InternalRecord], map: &'a MapData) -> Result<Self, GuiError> {
        Ok(Self {
            records: display_records(records, map)?,
            map,
        })
    }

    fn profile_analysis_view(
        &self,
        profile: Profile,
        last_run: Option<ImportReport>,
        selection: &DashboardSelection,
        record_filter: &RecordFilter,
    ) -> ProfileAnalysisView {
        ProfileAnalysisView {
            overview: self.dashboard_overview(profile, last_run),
            selected_detail: self.dashboard_selection_detail(selection),
            record_filter_options: self.record_filter_options(),
            record_page: self.record_page(record_filter),
        }
    }

    fn dashboard_overview(
        &self,
        profile: Profile,
        last_run: Option<ImportReport>,
    ) -> DashboardOverview {
        let mut pool_kinds = Vec::new();
        for kind in [
            PoolKind::MonopolyLimited,
            PoolKind::MonopolyStandard,
            PoolKind::ForkLottery,
        ] {
            let detail = self.pool_kind_detail(kind);
            pool_kinds.push(detail.summary);
        }
        DashboardOverview {
            profile,
            last_run,
            total_records: self.records.len() as u64,
            pool_kinds,
            banners: banner_summaries(&self.records),
            time_stats: time_stats(&self.records),
            rarity_distribution: rarity_distribution_from_display_refs(self.records.iter()),
            item_ranking: item_ranking_from_display_refs(self.records.iter(), self.map),
        }
    }

    fn pool_kind_detail(&self, pool_kind: PoolKind) -> DashboardSelectionDetail {
        selection_detail_from_display_records(
            &self.records,
            self.map,
            pool_kind,
            self.map.pool_kind_label(pool_kind),
            None,
        )
    }

    fn dashboard_selection_detail(
        &self,
        selection: &DashboardSelection,
    ) -> DashboardSelectionDetail {
        match selection {
            DashboardSelection::PoolKind { pool_kind } => self.pool_kind_detail(*pool_kind),
            DashboardSelection::Banner {
                pool_kind,
                banner_id,
            } => {
                let label = self
                    .records
                    .iter()
                    .find(|record| record.derived.banner_id.as_deref() == Some(banner_id.as_str()))
                    .and_then(|record| record.banner.title.clone())
                    .unwrap_or_else(|| banner_id.clone());
                selection_detail_from_display_records(
                    &self.records,
                    self.map,
                    *pool_kind,
                    label,
                    Some(banner_id.as_str()),
                )
            }
        }
    }

    fn record_filter_options(&self) -> RecordFilterOptions {
        record_filter_options_from_display_records(&self.records)
    }

    fn record_page(&self, filter: &RecordFilter) -> RecordList {
        record_page_from_display_records(&self.records, filter)
    }
}

impl From<DashboardSelectionDetail> for PoolKindDetail {
    fn from(value: DashboardSelectionDetail) -> Self {
        Self {
            summary: value.summary,
            five_star_history: value.five_star_history,
        }
    }
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
    let mut five_star_records = pool_records
        .iter()
        .copied()
        .filter(|record| record.derived.hit_rarity == Some(5))
        .collect::<Vec<_>>();
    five_star_records.sort_by(|left, right| compare_scoped_analysis(left, right, banner_id));
    let five_star_history = five_star_records
        .into_iter()
        .map(five_star_record)
        .collect::<Vec<_>>();
    let pity_distances = five_star_history
        .iter()
        .map(|record| record.pity_distance)
        .collect::<Vec<_>>();
    let hit_count = five_star_history.len() as u64;
    let five_star_item_count = count_items_by_rarity(&pool_records, 5);
    let four_star_count = count_hits(&pool_records, 4);
    let average_5star_pity = (!pity_distances.is_empty())
        .then(|| pity_distances.iter().sum::<u64>() as f64 / pity_distances.len() as f64);
    let average_4star_pity =
        average_4star_pity_from_display_refs(pool_records.iter().copied(), banner_id);
    let min_5star_pity = pity_distances.iter().min().copied();
    let max_5star_pity = pity_distances.iter().max().copied();

    let up_count = count_item_rate_up(&pool_records, 5, RateUpResult::Up);
    let off_rate_count = count_item_rate_up(&pool_records, 5, RateUpResult::OffRate);
    let not_applicable_rate_up_count =
        count_item_rate_up(&pool_records, 5, RateUpResult::NotApplicable);
    let unknown_rate_up_count = count_item_rate_up(&pool_records, 5, RateUpResult::Unknown);
    let rate_up_4_count = count_hit_rate_up(&pool_records, 4, RateUpResult::Up);
    let off_rate_4_count = count_hit_rate_up(&pool_records, 4, RateUpResult::OffRate);
    let not_applicable_rate_up_4_count =
        count_hit_rate_up(&pool_records, 4, RateUpResult::NotApplicable);
    let unknown_rate_up_4_count = count_hit_rate_up(&pool_records, 4, RateUpResult::Unknown);
    let summary_rule = pool_records
        .first()
        .map(|record| record.derived.rule.clone())
        .unwrap_or_else(|| {
            fallback_rule_resolution(
                pool_kind,
                RuleResolutionIssue::FallbackPoolKind,
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
    let fork_win_stats = fork_win_stats(pool_records.iter().copied());
    let latest_5star = five_star_history.last().map(|hit| hit.record.clone());
    let latest = latest_countable_record(&pool_records, banner_id);
    let resource = resource_counters(pool_records.iter().copied());
    let roll_point_costs = roll_point_costs_to_5star(pool_records.iter().copied(), banner_id);

    DashboardSelectionDetail {
        summary: PoolKindSummary {
            pool_kind,
            label,
            total_pulls: count_pulls(&pool_records),
            roll_points_total: resource.total,
            known_roll_point_records: resource.known,
            missing_roll_point_records: resource.missing,
            hit_count,
            five_star_item_count,
            current_pity: latest.map(current_5star_pity_after_record).unwrap_or_default(),
            current_ten_pull_progress: latest
                .and_then(|record| record.derived.ten_pull_progress_after),
            current_guarantee: latest
                .and_then(|record| record.derived.guarantee_5_after)
                .unwrap_or(false),
            hard_pity,
            average_5star_pity,
            average_4star_pity,
            min_5star_pity,
            max_5star_pity,
            early_hit_count,
            up_count,
            off_rate_count,
            not_applicable_rate_up_count,
            unknown_rate_up_count,
            observed_up_rate,
            fork_win_count: fork_win_stats.win_count,
            fork_loss_count: fork_win_stats.loss_count,
            fork_forced_up_count: fork_win_stats.forced_up_count,
            fork_observed_25_75_win_rate: fork_win_stats.observed_win_rate(),
            latest_5star,
            four_star_count,
            rate_up_4_count,
            off_rate_4_count,
            not_applicable_rate_up_4_count,
            unknown_rate_up_4_count,
            average_roll_points_to_5star: average_i64(&roll_point_costs),
            roll_point_cost_samples_5star: roll_point_costs.len() as u64,
        },
        five_star_history,
        rarity_distribution: rarity_distribution_from_display_refs(pool_records.iter().copied()),
        hit_rarity_distribution: hit_rarity_distribution_from_display_refs(
            pool_records.iter().copied(),
        ),
        pull_rarity_distribution: pull_rarity_distribution_from_display_refs(
            pool_records.iter().copied(),
            pool_kind,
        ),
        item_ranking: item_ranking_from_display_refs(pool_records.iter().copied(), map),
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

fn five_star_result(result: RateUpResult) -> FiveStarResult {
    match result {
        RateUpResult::Up => FiveStarResult::Up,
        RateUpResult::OffRate => FiveStarResult::OffRate,
        RateUpResult::NotApplicable => FiveStarResult::NotApplicable,
        RateUpResult::Unknown => FiveStarResult::Unknown,
    }
}

#[derive(Default)]
struct ForkWinStats {
    win_count: u64,
    loss_count: u64,
    forced_up_count: u64,
}

impl ForkWinStats {
    fn observed_win_rate(&self) -> Option<f64> {
        let sample_count = self.win_count + self.loss_count;
        (sample_count > 0).then(|| self.win_count as f64 / sample_count as f64)
    }
}

fn fork_win_stats<'a>(records: impl IntoIterator<Item = &'a DisplayRecord>) -> ForkWinStats {
    let mut stats = ForkWinStats::default();
    for record in records {
        if record.pool_kind != PoolKind::ForkLottery || record.derived.hit_rarity != Some(5) {
            continue;
        }
        match record.derived.rate_up_result {
            RateUpResult::Up if record.derived.fork_forced_up == Some(true) => {
                stats.forced_up_count += 1;
            }
            RateUpResult::Up => stats.win_count += 1,
            RateUpResult::OffRate => stats.loss_count += 1,
            RateUpResult::NotApplicable | RateUpResult::Unknown => {}
        }
    }
    stats
}
