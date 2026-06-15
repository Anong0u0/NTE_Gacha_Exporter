use std::collections::{BTreeMap, HashMap};

use crate::maps::MapData;
use crate::model::{
    DashboardOverview, DisplayRecord, FiveStarRecord, FiveStarResult, GuiError, ImportReport,
    InternalRecord, ItemRank, PoolKind, PoolKindDetail, PoolKindSummary, Profile, RarityBucket,
    RecordFilter, RecordFilterOptions, RecordList, RecordPoolOption, RecordSortKey,
    RecordTypeOption, SortDirection,
};
use crate::rules::{classify_pool_id, rule_for};

pub fn dashboard_overview(
    profile: Profile,
    last_run: Option<ImportReport>,
    records: &[InternalRecord],
    map: &MapData,
) -> Result<DashboardOverview, GuiError> {
    let mut pool_kinds = Vec::new();
    for kind in [
        PoolKind::MonopolyLimited,
        PoolKind::MonopolyStandard,
        PoolKind::ForkLottery,
    ] {
        let detail = pool_kind_detail(records, map, kind)?;
        pool_kinds.push(detail.summary);
    }
    let latest_records = records
        .iter()
        .rev()
        .take(12)
        .map(|record| display_record(record, map))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DashboardOverview {
        profile,
        last_run,
        total_records: records.len() as u64,
        pool_kinds,
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
    let rule = rule_for(pool_kind);
    let pool_records = records
        .iter()
        .filter(|record| classify_pool_id(&record.pool_id).ok() == Some(pool_kind))
        .collect::<Vec<_>>();

    let mut current_pity = 0_u64;
    let mut current_guarantee = false;
    let mut pity_distances = Vec::new();
    let mut up_count = 0_u64;
    let mut off_rate_count = 0_u64;
    let mut history = Vec::new();
    let mut latest_5star = None;

    for record in &pool_records {
        current_pity += 1;
        let is_hit = map.item_rarity(&record.item_id) == Some(5);
        if !is_hit {
            continue;
        }

        let guarantee_before = current_guarantee;
        let result = if pool_kind == PoolKind::ForkLottery
            && !map.is_pickup_item(&record.pool_id, &record.item_id)
        {
            FiveStarResult::OffRate
        } else {
            FiveStarResult::Up
        };
        let guarantee_after =
            pool_kind == PoolKind::ForkLottery && result == FiveStarResult::OffRate;
        current_guarantee = guarantee_after;

        let display = display_record(record, map)?;
        latest_5star = Some(display.clone());
        let pity_distance = current_pity;
        pity_distances.push(pity_distance);
        match result {
            FiveStarResult::Up => up_count += 1,
            FiveStarResult::OffRate => off_rate_count += 1,
        }
        history.push(FiveStarRecord {
            record: display,
            pity_distance,
            result,
            guarantee_before,
            guarantee_after,
        });
        current_pity = 0;
    }

    let hit_count = pity_distances.len() as u64;
    let average_5star_pity = (!pity_distances.is_empty())
        .then(|| pity_distances.iter().sum::<u64>() as f64 / pity_distances.len() as f64);
    let min_5star_pity = pity_distances.iter().min().copied();
    let max_5star_pity = pity_distances.iter().max().copied();
    let early_hit_count = pity_distances
        .iter()
        .filter(|distance| **distance < rule.hard_pity)
        .count() as u64;
    let observed_up_rate = (hit_count > 0).then(|| up_count as f64 / hit_count as f64);

    Ok(PoolKindDetail {
        summary: PoolKindSummary {
            pool_kind,
            label: map.pool_kind_label(pool_kind),
            total_pulls: pool_records.len() as u64,
            hit_count,
            current_pity,
            current_guarantee,
            hard_pity: rule.hard_pity,
            average_5star_pity,
            min_5star_pity,
            max_5star_pity,
            early_hit_count,
            up_count,
            off_rate_count,
            observed_up_rate,
            latest_5star,
        },
        five_star_history: history,
    })
}

pub fn list_records(
    records: &[InternalRecord],
    map: &MapData,
    filter: &RecordFilter,
) -> Result<RecordList, GuiError> {
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

    for record in records {
        let kind = classify_pool_id(&record.pool_id)?;
        if filter.pool_kind.is_some_and(|expected| expected != kind) {
            continue;
        }
        if filter
            .pool_id
            .as_deref()
            .is_some_and(|pool_id| pool_id != record.pool_id)
        {
            continue;
        }
        if filter
            .record_type
            .as_deref()
            .is_some_and(|record_type| record_type != record.record_type)
        {
            continue;
        }
        if date_from.is_some() || date_to.is_some() {
            let Some(time) = record.time.as_deref() else {
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
            let item_name = map.item_name(&record.item_id).to_ascii_lowercase();
            let pool_label = map
                .pool_label(&record.pool_id, record.time.as_deref())
                .to_ascii_lowercase();
            let haystack = format!(
                "{} {} {} {} {} {}",
                record.record_id.to_ascii_lowercase(),
                record.record_type.to_ascii_lowercase(),
                record.item_id.to_ascii_lowercase(),
                item_name,
                record.pool_id.to_ascii_lowercase(),
                pool_label
            );
            if !haystack.contains(search) {
                continue;
            }
        }
        matching.push(display_record(record, map)?);
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
    let mut record_types: HashMap<String, u64> = HashMap::new();

    for record in records {
        let pool_kind = classify_pool_id(&record.pool_id)?;
        pools
            .entry(record.pool_id.clone())
            .and_modify(|option| option.count += 1)
            .or_insert_with(|| RecordPoolOption {
                pool_id: record.pool_id.clone(),
                pool_kind,
                label: map.pool_label(&record.pool_id, record.time.as_deref()),
                count: 1,
            });
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

    Ok(RecordFilterOptions {
        pools,
        record_types,
    })
}

pub fn display_record(record: &InternalRecord, map: &MapData) -> Result<DisplayRecord, GuiError> {
    let pool_kind = classify_pool_id(&record.pool_id)?;
    let item_id = map.canonical_item_id(&record.item_id).to_string();
    let secondary_item_id = record
        .secondary_item_id
        .as_deref()
        .map(|value| map.canonical_item_id(value).to_string());
    Ok(DisplayRecord {
        record_id: record.record_id.clone(),
        record_type: record.record_type.clone(),
        time: record.time.clone(),
        pool_kind,
        pool_id: record.pool_id.clone(),
        pool_label: map.pool_label(&record.pool_id, record.time.as_deref()),
        item_id: item_id.clone(),
        item_name: map.item_name(&item_id),
        rarity: map.item_rarity(&item_id),
        count: record.count,
        roll_points: record.roll_points,
        secondary_item_name: secondary_item_id
            .as_deref()
            .map(|item_id| map.item_name(item_id)),
        secondary_item_id,
        secondary_count: record.secondary_count,
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
        };
        match sort_direction {
            SortDirection::Asc => ordering,
            SortDirection::Desc => ordering.reverse(),
        }
    });
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
