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
            let latest = latest_countable_record(&banner_records, Some(&banner_id));
            let resource = resource_counters(banner_records.iter().copied());
            let five_star_pity = banner_records
                .iter()
                .filter_map(|record| hit_pity_distance(record, 5))
                .collect::<Vec<_>>();
            let roll_point_costs =
                roll_point_costs_to_5star(banner_records.iter().copied(), Some(&banner_id));
            let fork_win_stats = fork_win_stats(banner_records.iter().copied());
            let latest_hit = banner_records
                .iter()
                .copied()
                .filter(|record| matches!(record.derived.hit_rarity, Some(5 | 4)))
                .max_by(|left, right| compare_scoped_analysis(left, right, Some(&banner_id)))
                .cloned();

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
                start_at: first.banner.start_at.clone(),
                end_at: first.banner.end_at.clone(),
                asset_refs: first.banner.asset_refs.clone(),
                total_pulls: count_pulls(&banner_records),
                roll_points_total: resource.total,
                known_roll_point_records: resource.known,
                missing_roll_point_records: resource.missing,
                five_star_count: count_hits(&banner_records, 5),
                four_star_count: count_hits(&banner_records, 4),
                current_5star_pity: latest
                    .map(|record| record.derived.pity_5_after)
                    .unwrap_or_default(),
                average_5star_pity: average_u64(&five_star_pity),
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
                fork_win_count: fork_win_stats.win_count,
                fork_loss_count: fork_win_stats.loss_count,
                fork_forced_up_count: fork_win_stats.forced_up_count,
                fork_observed_25_75_win_rate: fork_win_stats.observed_win_rate(),
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
                average_roll_points_to_5star: average_i64(&roll_point_costs),
                roll_point_cost_samples_5star: roll_point_costs.len() as u64,
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

fn time_stats(records: &[DisplayRecord]) -> TimeStats {
    let mut monthly: BTreeMap<String, BucketAccumulator> = BTreeMap::new();
    let mut daily: BTreeMap<String, BucketAccumulator> = BTreeMap::new();
    let mut missing_time_records = 0;

    for record in records {
        if !record.derived.counts_as_pull {
            continue;
        }
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
        if !record.derived.counts_as_pull {
            continue;
        }
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

fn roll_point_costs_to_5star<'a>(
    records: impl IntoIterator<Item = &'a DisplayRecord>,
    banner_id: Option<&str>,
) -> Vec<i64> {
    let mut ordered = records.into_iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| compare_scoped_analysis(left, right, banner_id));

    let mut costs = Vec::new();
    let mut five_star_interval = RollPointCostInterval::default();
    for record in ordered {
        if !record.derived.counts_as_pull {
            continue;
        }
        five_star_interval.add(record.roll_points);
        if record.derived.hit_rarity == Some(5) {
            five_star_interval.close_into(&mut costs);
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
}

impl BucketAccumulator {
    fn add(&mut self, record: &DisplayRecord) {
        if !record.derived.counts_as_pull {
            return;
        }
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
        }
    }
}

fn count_hits(records: &[&DisplayRecord], rarity: u8) -> u64 {
    records
        .iter()
        .filter(|record| {
            record.derived.counts_as_pull && record.derived.hit_rarity == Some(rarity)
        })
        .count() as u64
}

fn count_hit_rate_up(records: &[&DisplayRecord], rarity: u8, result: RateUpResult) -> u64 {
    records
        .iter()
        .filter(|record| {
            record.derived.counts_as_pull
                && record.derived.hit_rarity == Some(rarity)
                && record.derived.rate_up_result == result
        })
        .count() as u64
}

fn hit_pity_distance(record: &DisplayRecord, rarity: u8) -> Option<u64> {
    if !record.derived.counts_as_pull {
        return None;
    }
    if record.derived.hit_rarity != Some(rarity) {
        return None;
    }
    match rarity {
        5 => Some(record.derived.pity_5_before + 1),
        _ => None,
    }
}

fn count_pulls(records: &[&DisplayRecord]) -> u64 {
    records
        .iter()
        .filter(|record| record.derived.counts_as_pull)
        .count() as u64
}

fn latest_countable_record<'a>(
    records: &[&'a DisplayRecord],
    banner_id: Option<&str>,
) -> Option<&'a DisplayRecord> {
    records
        .iter()
        .copied()
        .filter(|record| record.derived.counts_as_pull)
        .max_by_key(|record| match banner_id {
            Some(_) => record.derived.pull_no_in_banner,
            None => record.derived.pull_no_in_pool_kind,
        })
}

fn compare_scoped_analysis(
    left: &DisplayRecord,
    right: &DisplayRecord,
    banner_id: Option<&str>,
) -> std::cmp::Ordering {
    match banner_id {
        Some(_) => left
            .derived
            .pull_no_in_banner
            .cmp(&right.derived.pull_no_in_banner),
        None => left
            .derived
            .pull_no_in_pool_kind
            .cmp(&right.derived.pull_no_in_pool_kind),
    }
    .then_with(|| compare_display_chronological(left, right))
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
