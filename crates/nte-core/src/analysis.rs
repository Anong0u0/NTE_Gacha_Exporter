use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::MapData;
use crate::RuleResolutionStatus;
use crate::derive_records;
use crate::{
    BannerSummary, DashboardOverview, DisplayRecord, FiveStarRecord, FiveStarResult,
    FourStarRecord, GuiError, ImportReport, InternalRecord, ItemRank, PhaseSummary, PoolKind,
    PoolKindDetail, PoolKindSummary, Profile, RarityBucket, RateUpResult, RecordBannerOption,
    RecordDerived, RecordFilter, RecordFilterOptions, RecordList, RecordPoolOption, RecordSortKey,
    RecordTypeOption, ResourcePoolKindSummary, ResourceSummary, SortDirection, TimeBucketSummary,
    TimeStats,
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
    let latest_records = display_records
        .iter()
        .rev()
        .take(12)
        .cloned()
        .collect::<Vec<_>>();
    Ok(DashboardOverview {
        profile,
        last_run,
        total_records: records.len() as u64,
        pool_kinds,
        banners: banner_summaries(&display_records),
        resource: resource_summary(&display_records, map),
        time_stats: time_stats(&display_records),
        rarity_distribution: rarity_distribution(records, map),
        item_ranking: item_ranking(records, map),
        latest_records,
    })
}

pub fn pool_kind_detail(
    records: &[InternalRecord],
    map: &MapData,
    pool_kind: PoolKind,
) -> Result<PoolKindDetail, GuiError> {
    let display_records = display_records(records, map)?;
    Ok(pool_kind_detail_from_display_records(
        &display_records,
        map,
        pool_kind,
    ))
}

fn pool_kind_detail_from_display_records(
    records: &[DisplayRecord],
    map: &MapData,
    pool_kind: PoolKind,
) -> PoolKindDetail {
    let pool_records = records
        .iter()
        .filter(|record| record.pool_kind == pool_kind)
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

    PoolKindDetail {
        summary: PoolKindSummary {
            pool_kind,
            label: map.pool_kind_label(pool_kind),
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
            rule_source_confidence: summary_rule.source_confidence.clone(),
            average_roll_points_to_5star: average_i64(&roll_point_costs.five_star),
            average_roll_points_to_4star: average_i64(&roll_point_costs.four_star),
            roll_point_cost_samples_5star: roll_point_costs.five_star.len() as u64,
            roll_point_cost_samples_4star: roll_point_costs.four_star.len() as u64,
        },
        five_star_history,
        four_star_history,
    }
}

fn five_star_record(record: &DisplayRecord) -> FiveStarRecord {
    FiveStarRecord {
        record: record.clone(),
        pity_distance: record.derived.pity_5_before + 1,
        result: five_star_result(record.derived.rate_up_result),
        result_confidence: record.derived.result_confidence.clone(),
        guarantee_before: record.derived.guarantee_5_before,
        guarantee_after: record.derived.guarantee_5_after,
    }
}

fn four_star_record(record: &DisplayRecord) -> FourStarRecord {
    FourStarRecord {
        record: record.clone(),
        pity_distance: record.derived.pity_4_before + 1,
        result: record.derived.rate_up_result,
        result_confidence: record.derived.result_confidence.clone(),
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

fn banner_summaries(records: &[DisplayRecord]) -> Vec<BannerSummary> {
    let mut grouped: BTreeMap<String, Vec<&DisplayRecord>> = BTreeMap::new();
    for record in records {
        if let Some(banner_id) = record.derived.banner_id.as_ref() {
            grouped.entry(banner_id.clone()).or_default().push(record);
        }
    }

    let mut summaries = grouped
        .into_iter()
        .filter_map(|(banner_id, banner_records)| {
            let first = banner_records.first().copied()?;
            let latest = banner_records.last().copied();
            let resource = resource_counters(banner_records.iter().copied());
            let five_star_pity = banner_records
                .iter()
                .filter_map(|record| hit_pity_distance(record, 5))
                .collect::<Vec<_>>();
            let four_star_pity = banner_records
                .iter()
                .filter_map(|record| hit_pity_distance(record, 4))
                .collect::<Vec<_>>();
            let roll_point_costs = roll_point_costs_to_hits(banner_records.iter().copied());
            let latest_hit = banner_records
                .iter()
                .rev()
                .find(|record| matches!(record.derived.hit_rarity, Some(5 | 4)))
                .map(|record| (*record).clone());

            Some(BannerSummary {
                banner_id: banner_id.clone(),
                pool_id: first.pool_id.clone(),
                pool_kind: first.pool_kind,
                banner_type: first.banner.banner_type.clone(),
                title: first
                    .banner
                    .title
                    .clone()
                    .unwrap_or_else(|| banner_id.clone()),
                version: first.derived.banner_version.clone(),
                phase: first.derived.banner_phase.clone(),
                start_at: first.banner.start_at.clone(),
                end_at: first.banner.end_at.clone(),
                source_confidence: first.banner.source_confidence.clone(),
                asset_refs: first.banner.asset_refs.clone(),
                total_pulls: banner_records.len() as u64,
                roll_points_total: resource.total,
                known_roll_point_records: resource.known,
                missing_roll_point_records: resource.missing,
                five_star_count: count_hits(&banner_records, 5),
                four_star_count: count_hits(&banner_records, 4),
                current_5star_pity: latest
                    .map(|record| record.derived.pity_5_after)
                    .unwrap_or_default(),
                current_4star_pity: latest
                    .map(|record| record.derived.pity_4_after)
                    .unwrap_or_default(),
                average_5star_pity: average_u64(&five_star_pity),
                average_4star_pity: average_u64(&four_star_pity),
                rate_up_5_count: count_hit_rate_up(&banner_records, 5, RateUpResult::Up),
                off_rate_5_count: count_hit_rate_up(&banner_records, 5, RateUpResult::OffRate),
                not_applicable_rate_up_5_count: count_hit_rate_up(
                    &banner_records,
                    5,
                    RateUpResult::NotApplicable,
                ),
                unknown_rate_up_5_count: count_hit_rate_up(
                    &banner_records,
                    5,
                    RateUpResult::Unknown,
                ),
                rate_up_4_count: count_hit_rate_up(&banner_records, 4, RateUpResult::Up),
                off_rate_4_count: count_hit_rate_up(&banner_records, 4, RateUpResult::OffRate),
                not_applicable_rate_up_4_count: count_hit_rate_up(
                    &banner_records,
                    4,
                    RateUpResult::NotApplicable,
                ),
                unknown_rate_up_4_count: count_hit_rate_up(
                    &banner_records,
                    4,
                    RateUpResult::Unknown,
                ),
                average_roll_points_to_5star: average_i64(&roll_point_costs.five_star),
                average_roll_points_to_4star: average_i64(&roll_point_costs.four_star),
                roll_point_cost_samples_5star: roll_point_costs.five_star.len() as u64,
                roll_point_cost_samples_4star: roll_point_costs.four_star.len() as u64,
                latest_hit,
            })
        })
        .collect::<Vec<_>>();

    summaries.sort_by(|left, right| {
        left.pool_kind
            .cmp(&right.pool_kind)
            .then_with(|| left.title.cmp(&right.title))
            .then_with(|| left.banner_id.cmp(&right.banner_id))
    });
    summaries
}

fn resource_summary(records: &[DisplayRecord], map: &MapData) -> ResourceSummary {
    let total = resource_counters(records.iter());
    let by_pool_kind = [
        PoolKind::MonopolyLimited,
        PoolKind::MonopolyStandard,
        PoolKind::ForkLottery,
    ]
    .into_iter()
    .map(|pool_kind| {
        let counters = resource_counters(
            records
                .iter()
                .filter(|record| record.pool_kind == pool_kind),
        );
        ResourcePoolKindSummary {
            pool_kind,
            label: map.pool_kind_label(pool_kind),
            roll_points_total: counters.total,
            known_roll_point_records: counters.known,
            missing_roll_point_records: counters.missing,
        }
    })
    .collect::<Vec<_>>();

    ResourceSummary {
        total_roll_points: total.total,
        known_roll_point_records: total.known,
        missing_roll_point_records: total.missing,
        by_pool_kind,
    }
}

fn time_stats(records: &[DisplayRecord]) -> TimeStats {
    let mut monthly: BTreeMap<String, BucketAccumulator> = BTreeMap::new();
    let mut daily: BTreeMap<String, BucketAccumulator> = BTreeMap::new();
    let mut phases: BTreeMap<(Option<String>, Option<String>), PhaseAccumulator> = BTreeMap::new();
    let mut missing_time_records = 0;

    for record in records {
        let monthly_bucket = record.time.as_deref().and_then(|time| date_bucket(time, 7));
        let daily_bucket = record
            .time
            .as_deref()
            .and_then(|time| date_bucket(time, 10));
        if monthly_bucket.is_none() || daily_bucket.is_none() {
            missing_time_records += 1;
        }
        if let Some(bucket) = monthly_bucket {
            monthly.entry(bucket).or_default().add(record);
        }
        if let Some(bucket) = daily_bucket {
            daily.entry(bucket).or_default().add(record);
        }
        if let Some(banner_id) = record.derived.banner_id.as_ref() {
            phases
                .entry((
                    record.derived.banner_version.clone(),
                    record.derived.banner_phase.clone(),
                ))
                .or_default()
                .add(record, banner_id);
        }
    }

    TimeStats {
        monthly: monthly
            .into_iter()
            .map(|(bucket, accumulator)| accumulator.into_summary(bucket))
            .collect(),
        daily: daily
            .into_iter()
            .map(|(bucket, accumulator)| accumulator.into_summary(bucket))
            .collect(),
        phases: phases
            .into_iter()
            .map(|((version, phase), accumulator)| accumulator.into_summary(version, phase))
            .collect(),
        missing_time_records,
    }
}

#[derive(Default)]
struct ResourceCounters {
    total: i64,
    known: u64,
    missing: u64,
}

fn resource_counters<'a>(records: impl IntoIterator<Item = &'a DisplayRecord>) -> ResourceCounters {
    let mut counters = ResourceCounters::default();
    for record in records {
        match record.roll_points {
            Some(roll_points) => {
                counters.total += roll_points;
                counters.known += 1;
            }
            None => counters.missing += 1,
        }
    }
    counters
}

#[derive(Default)]
struct RollPointCosts {
    five_star: Vec<i64>,
    four_star: Vec<i64>,
}

#[derive(Default)]
struct RollPointCostInterval {
    total: i64,
    has_missing: bool,
}

impl RollPointCostInterval {
    fn add(&mut self, roll_points: Option<i64>) {
        match roll_points {
            Some(value) => self.total += value,
            None => self.has_missing = true,
        }
    }

    fn close_into(&mut self, costs: &mut Vec<i64>) {
        if !self.has_missing {
            costs.push(self.total);
        }
        *self = Self::default();
    }
}

fn roll_point_costs_to_hits<'a>(
    records: impl IntoIterator<Item = &'a DisplayRecord>,
) -> RollPointCosts {
    let mut ordered = records.into_iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        left.time
            .cmp(&right.time)
            .then_with(|| left.record_id.cmp(&right.record_id))
    });

    let mut costs = RollPointCosts::default();
    let mut five_star_interval = RollPointCostInterval::default();
    let mut four_star_interval = RollPointCostInterval::default();
    for record in ordered {
        five_star_interval.add(record.roll_points);
        four_star_interval.add(record.roll_points);
        match record.derived.hit_rarity {
            Some(5) => five_star_interval.close_into(&mut costs.five_star),
            Some(4) => four_star_interval.close_into(&mut costs.four_star),
            _ => {}
        }
    }
    costs
}

#[derive(Default)]
struct BucketAccumulator {
    total_pulls: u64,
    five_star_count: u64,
    four_star_count: u64,
    roll_points_total: i64,
    known_roll_point_records: u64,
    missing_roll_point_records: u64,
    five_star_pity: Vec<u64>,
    four_star_pity: Vec<u64>,
}

impl BucketAccumulator {
    fn add(&mut self, record: &DisplayRecord) {
        self.total_pulls += 1;
        match record.roll_points {
            Some(roll_points) => {
                self.roll_points_total += roll_points;
                self.known_roll_point_records += 1;
            }
            None => self.missing_roll_point_records += 1,
        }
        match record.derived.hit_rarity {
            Some(5) => {
                self.five_star_count += 1;
                self.five_star_pity.push(record.derived.pity_5_before + 1);
            }
            Some(4) => {
                self.four_star_count += 1;
                self.four_star_pity.push(record.derived.pity_4_before + 1);
            }
            _ => {}
        }
    }

    fn into_summary(self, bucket: String) -> TimeBucketSummary {
        TimeBucketSummary {
            bucket,
            total_pulls: self.total_pulls,
            five_star_count: self.five_star_count,
            four_star_count: self.four_star_count,
            roll_points_total: self.roll_points_total,
            known_roll_point_records: self.known_roll_point_records,
            missing_roll_point_records: self.missing_roll_point_records,
            average_5star_pity: average_u64(&self.five_star_pity),
            average_4star_pity: average_u64(&self.four_star_pity),
        }
    }
}

#[derive(Default)]
struct PhaseAccumulator {
    bucket: BucketAccumulator,
    banner_ids: BTreeSet<String>,
}

impl PhaseAccumulator {
    fn add(&mut self, record: &DisplayRecord, banner_id: &str) {
        self.bucket.add(record);
        self.banner_ids.insert(banner_id.to_string());
    }

    fn into_summary(self, version: Option<String>, phase: Option<String>) -> PhaseSummary {
        PhaseSummary {
            version,
            phase,
            total_pulls: self.bucket.total_pulls,
            five_star_count: self.bucket.five_star_count,
            four_star_count: self.bucket.four_star_count,
            roll_points_total: self.bucket.roll_points_total,
            known_roll_point_records: self.bucket.known_roll_point_records,
            missing_roll_point_records: self.bucket.missing_roll_point_records,
            banner_count: self.banner_ids.len() as u64,
            average_5star_pity: average_u64(&self.bucket.five_star_pity),
            average_4star_pity: average_u64(&self.bucket.four_star_pity),
        }
    }
}

fn count_hits(records: &[&DisplayRecord], rarity: u8) -> u64 {
    records
        .iter()
        .filter(|record| record.derived.hit_rarity == Some(rarity))
        .count() as u64
}

fn count_hit_rate_up(records: &[&DisplayRecord], rarity: u8, result: RateUpResult) -> u64 {
    records
        .iter()
        .filter(|record| {
            record.derived.hit_rarity == Some(rarity) && record.derived.rate_up_result == result
        })
        .count() as u64
}

fn hit_pity_distance(record: &DisplayRecord, rarity: u8) -> Option<u64> {
    if record.derived.hit_rarity != Some(rarity) {
        return None;
    }
    match rarity {
        5 => Some(record.derived.pity_5_before + 1),
        4 => Some(record.derived.pity_4_before + 1),
        _ => None,
    }
}

fn average_u64(values: &[u64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<u64>() as f64 / values.len() as f64)
}

fn average_i64(values: &[i64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<i64>() as f64 / values.len() as f64)
}

fn date_bucket(time: &str, len: usize) -> Option<String> {
    let value = time.trim();
    match len {
        7 if value.len() >= 7 && is_valid_month_prefix(value) => Some(value[..7].to_string()),
        10 if value.len() >= 10 && is_valid_day_prefix(value) => Some(value[..10].to_string()),
        _ => None,
    }
}

fn is_valid_month_prefix(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 7
        && bytes[0..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
}

fn is_valid_day_prefix(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 10
        && is_valid_month_prefix(value)
        && bytes[7] == b'-'
        && bytes[8..10].iter().all(u8::is_ascii_digit)
}

pub fn list_records(
    records: &[InternalRecord],
    map: &MapData,
    filter: &RecordFilter,
) -> Result<RecordList, GuiError> {
    let display_records = display_records(records, map)?;
    let search = filter
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase);
    let limit = filter.limit.unwrap_or(200).min(1000) as usize;
    let offset = filter.offset.unwrap_or(0) as usize;

    let mut matching = Vec::new();
    let date_from = filter
        .date_from
        .as_deref()
        .map(date_start)
        .filter(|value| !value.is_empty());
    let date_to = filter
        .date_to
        .as_deref()
        .map(date_end)
        .filter(|value| !value.is_empty());

    for display in display_records {
        if filter
            .pool_kind
            .is_some_and(|expected| expected != display.pool_kind)
        {
            continue;
        }
        if filter
            .pool_id
            .as_deref()
            .is_some_and(|pool_id| pool_id != display.pool_id)
        {
            continue;
        }
        if filter
            .banner_id
            .as_deref()
            .is_some_and(|banner_id| display.derived.banner_id.as_deref() != Some(banner_id))
        {
            continue;
        }
        if filter
            .record_type
            .as_deref()
            .is_some_and(|record_type| record_type != display.record_type)
        {
            continue;
        }
        if filter
            .rarity
            .is_some_and(|rarity| display.rarity != Some(rarity))
        {
            continue;
        }
        if filter
            .hit_rarity
            .is_some_and(|rarity| display.derived.hit_rarity != Some(rarity))
        {
            continue;
        }
        if filter
            .rate_up_result
            .is_some_and(|result| display.derived.rate_up_result != result)
        {
            continue;
        }
        if filter
            .pity_5_min
            .is_some_and(|min| display.derived.pity_5_before < min)
        {
            continue;
        }
        if filter
            .pity_5_max
            .is_some_and(|max| display.derived.pity_5_before > max)
        {
            continue;
        }
        if filter
            .pity_4_min
            .is_some_and(|min| display.derived.pity_4_before < min)
        {
            continue;
        }
        if filter
            .pity_4_max
            .is_some_and(|max| display.derived.pity_4_before > max)
        {
            continue;
        }
        if date_from.is_some() || date_to.is_some() {
            let Some(time) = display.time.as_deref() else {
                continue;
            };
            if date_from
                .as_deref()
                .is_some_and(|date_from| time < date_from)
            {
                continue;
            }
            if date_to.as_deref().is_some_and(|date_to| time > date_to) {
                continue;
            }
        }
        if let Some(search) = &search {
            let pool_label = display.pool_label.to_ascii_lowercase();
            let banner_id = display
                .derived
                .banner_id
                .as_deref()
                .unwrap_or_default()
                .to_ascii_lowercase();
            let banner_phase = display
                .derived
                .banner_phase
                .as_deref()
                .unwrap_or_default()
                .to_ascii_lowercase();
            let banner_title = display
                .banner
                .title
                .as_deref()
                .unwrap_or_default()
                .to_ascii_lowercase();
            let rule_id = display
                .derived
                .rule
                .rule_id
                .as_deref()
                .unwrap_or_default()
                .to_ascii_lowercase();
            let haystack = format!(
                "{} {} {} {} {} {} {} {} {} {} {}",
                display.record_id.to_ascii_lowercase(),
                display.record_type.to_ascii_lowercase(),
                display.item_id.to_ascii_lowercase(),
                display.item_name.to_ascii_lowercase(),
                display.pool_id.to_ascii_lowercase(),
                pool_label,
                banner_id,
                banner_title,
                banner_phase,
                rate_up_result_key(display.derived.rate_up_result),
                rule_id
            );
            if !haystack.contains(search) {
                continue;
            }
            matching.push(display);
            continue;
        }
        matching.push(display);
    }

    sort_display_records(
        &mut matching,
        filter.sort_key.unwrap_or_default(),
        filter.sort_direction.unwrap_or_default(),
    );
    let total = matching.len() as u64;
    let page = matching
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect::<Vec<_>>();
    Ok(RecordList {
        total,
        records: page,
    })
}

pub fn record_filter_options(
    records: &[InternalRecord],
    map: &MapData,
) -> Result<RecordFilterOptions, GuiError> {
    let mut pools: HashMap<String, RecordPoolOption> = HashMap::new();
    let mut banners: HashMap<String, RecordBannerOption> = HashMap::new();
    let mut record_types: HashMap<String, u64> = HashMap::new();

    for record in display_records(records, map)? {
        pools
            .entry(record.pool_id.clone())
            .and_modify(|option| option.count += 1)
            .or_insert_with(|| RecordPoolOption {
                pool_id: record.pool_id.clone(),
                pool_kind: record.pool_kind,
                label: record.pool_label.clone(),
                count: 1,
            });
        if let Some(banner_id) = record.derived.banner_id.as_ref() {
            banners
                .entry(banner_id.clone())
                .and_modify(|option| option.count += 1)
                .or_insert_with(|| RecordBannerOption {
                    banner_id: banner_id.clone(),
                    pool_kind: record.pool_kind,
                    title: record
                        .banner
                        .title
                        .clone()
                        .unwrap_or_else(|| banner_id.clone()),
                    count: 1,
                    phase: record.derived.banner_phase.clone(),
                });
        }
        *record_types.entry(record.record_type.clone()).or_default() += 1;
    }

    let mut pools = pools.into_values().collect::<Vec<_>>();
    pools.sort_by(|left, right| {
        left.pool_kind
            .cmp(&right.pool_kind)
            .then_with(|| left.label.cmp(&right.label))
            .then_with(|| left.pool_id.cmp(&right.pool_id))
    });

    let mut record_types = record_types
        .into_iter()
        .map(|(record_type, count)| RecordTypeOption { record_type, count })
        .collect::<Vec<_>>();
    record_types.sort_by(|left, right| left.record_type.cmp(&right.record_type));
    let mut banners = banners.into_values().collect::<Vec<_>>();
    banners.sort_by(|left, right| {
        left.pool_kind
            .cmp(&right.pool_kind)
            .then_with(|| left.title.cmp(&right.title))
            .then_with(|| left.banner_id.cmp(&right.banner_id))
    });

    Ok(RecordFilterOptions {
        pools,
        banners,
        record_types,
    })
}

pub fn display_records(
    records: &[InternalRecord],
    map: &MapData,
) -> Result<Vec<DisplayRecord>, GuiError> {
    let mut derived_by_id = derive_records(records, map)?
        .into_iter()
        .map(|derived| (derived.record_id.clone(), derived))
        .collect::<HashMap<_, _>>();
    records
        .iter()
        .map(|record| {
            let derived = derived_by_id.remove(&record.record_id).ok_or_else(|| {
                GuiError::InvalidDocument(format!(
                    "record derived state missing: {}",
                    record.record_id
                ))
            })?;
            display_record(record, map, derived)
        })
        .collect()
}

fn display_record(
    record: &InternalRecord,
    map: &MapData,
    derived: RecordDerived,
) -> Result<DisplayRecord, GuiError> {
    let pool_kind = classify_pool_id(&record.pool_id)?;
    let item_id = map.canonical_item_id(&record.item_id).to_string();
    let secondary_item_id = record
        .secondary_item_id
        .as_deref()
        .map(|value| map.canonical_item_id(value).to_string());
    let banner = map.resolve_banner(&record.pool_id, record.time.as_deref());
    let item_asset_refs = map
        .item(&item_id)
        .map(|(_, item)| item.asset_refs.clone())
        .unwrap_or_default();
    let secondary_item_asset_refs = secondary_item_id
        .as_deref()
        .and_then(|item_id| map.item(item_id))
        .map(|(_, item)| item.asset_refs.clone())
        .unwrap_or_default();
    Ok(DisplayRecord {
        record_id: record.record_id.clone(),
        record_type: record.record_type.clone(),
        time: record.time.clone(),
        pool_kind,
        pool_id: record.pool_id.clone(),
        pool_label: map.pool_label(&record.pool_id, record.time.as_deref()),
        banner,
        item_id: item_id.clone(),
        item_name: map.item_name(&item_id),
        item_asset_refs,
        rarity: map.item_rarity(&item_id),
        count: record.count,
        roll_points: record.roll_points,
        secondary_item_name: secondary_item_id
            .as_deref()
            .map(|item_id| map.item_name(item_id)),
        secondary_item_id,
        secondary_item_asset_refs,
        secondary_count: record.secondary_count,
        derived,
    })
}

fn sort_display_records(
    records: &mut [DisplayRecord],
    sort_key: RecordSortKey,
    sort_direction: SortDirection,
) {
    records.sort_by(|left, right| {
        let ordering = match sort_key {
            RecordSortKey::Time => left
                .time
                .cmp(&right.time)
                .then_with(|| left.record_id.cmp(&right.record_id)),
            RecordSortKey::Pool => left
                .pool_label
                .cmp(&right.pool_label)
                .then_with(|| left.time.cmp(&right.time))
                .then_with(|| left.record_id.cmp(&right.record_id)),
            RecordSortKey::Item => left
                .item_name
                .cmp(&right.item_name)
                .then_with(|| left.time.cmp(&right.time))
                .then_with(|| left.record_id.cmp(&right.record_id)),
            RecordSortKey::Rarity => left
                .rarity
                .cmp(&right.rarity)
                .then_with(|| left.time.cmp(&right.time))
                .then_with(|| left.record_id.cmp(&right.record_id)),
            RecordSortKey::RecordType => left
                .record_type
                .cmp(&right.record_type)
                .then_with(|| left.time.cmp(&right.time))
                .then_with(|| left.record_id.cmp(&right.record_id)),
            RecordSortKey::Banner => banner_sort_key(left)
                .cmp(&banner_sort_key(right))
                .then_with(|| left.time.cmp(&right.time))
                .then_with(|| left.record_id.cmp(&right.record_id)),
            RecordSortKey::PullNo => pull_no_sort_value(left)
                .cmp(&pull_no_sort_value(right))
                .then_with(|| left.time.cmp(&right.time))
                .then_with(|| left.record_id.cmp(&right.record_id)),
            RecordSortKey::Pity5 => left
                .derived
                .pity_5_before
                .cmp(&right.derived.pity_5_before)
                .then_with(|| left.time.cmp(&right.time))
                .then_with(|| left.record_id.cmp(&right.record_id)),
            RecordSortKey::Pity4 => left
                .derived
                .pity_4_before
                .cmp(&right.derived.pity_4_before)
                .then_with(|| left.time.cmp(&right.time))
                .then_with(|| left.record_id.cmp(&right.record_id)),
            RecordSortKey::RateUp => rate_up_rank(left.derived.rate_up_result)
                .cmp(&rate_up_rank(right.derived.rate_up_result))
                .then_with(|| left.time.cmp(&right.time))
                .then_with(|| left.record_id.cmp(&right.record_id)),
        };
        match sort_direction {
            SortDirection::Asc => ordering,
            SortDirection::Desc => ordering.reverse(),
        }
    });
}

fn banner_sort_key(record: &DisplayRecord) -> String {
    record
        .banner
        .title
        .as_deref()
        .or(record.derived.banner_id.as_deref())
        .unwrap_or_default()
        .to_string()
}

fn pull_no_sort_value(record: &DisplayRecord) -> u64 {
    record
        .derived
        .pull_no_in_banner
        .unwrap_or(record.derived.pull_no_in_pool_kind)
}

fn rate_up_rank(result: RateUpResult) -> u8 {
    match result {
        RateUpResult::Up => 0,
        RateUpResult::OffRate => 1,
        RateUpResult::NotApplicable => 2,
        RateUpResult::Unknown => 3,
    }
}

fn rate_up_result_key(result: RateUpResult) -> &'static str {
    match result {
        RateUpResult::Up => "up",
        RateUpResult::OffRate => "off_rate",
        RateUpResult::NotApplicable => "not_applicable",
        RateUpResult::Unknown => "unknown",
    }
}

fn date_start(value: &str) -> String {
    let value = value.trim();
    if value.len() == 10 {
        format!("{value} 00:00:00")
    } else {
        value.to_string()
    }
}

fn date_end(value: &str) -> String {
    let value = value.trim();
    if value.len() == 10 {
        format!("{value} 23:59:59")
    } else {
        value.to_string()
    }
}

fn rarity_distribution(records: &[InternalRecord], map: &MapData) -> Vec<RarityBucket> {
    let mut counts: BTreeMap<u8, u64> = BTreeMap::new();
    for record in records {
        if let Some(rarity) = map.item_rarity(&record.item_id) {
            *counts.entry(rarity).or_default() += 1;
        }
    }
    let known_total = counts.values().sum::<u64>();
    counts
        .into_iter()
        .rev()
        .map(|(rarity, count)| RarityBucket {
            rarity,
            count,
            percent: if known_total == 0 {
                0.0
            } else {
                count as f64 / known_total as f64
            },
        })
        .collect()
}

fn item_ranking(records: &[InternalRecord], map: &MapData) -> Vec<ItemRank> {
    let mut counts: HashMap<String, u64> = HashMap::new();
    for record in records {
        let item_id = map.canonical_item_id(&record.item_id).to_string();
        *counts.entry(item_id).or_default() += 1;
    }
    let mut ranking = counts
        .into_iter()
        .map(|(item_id, count)| ItemRank {
            item_name: map.item_name(&item_id),
            rarity: map.item_rarity(&item_id),
            item_id,
            count,
        })
        .collect::<Vec<_>>();
    ranking.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| right.rarity.cmp(&left.rarity))
            .then_with(|| left.item_name.cmp(&right.item_name))
    });
    ranking.truncate(20);
    ranking
}
