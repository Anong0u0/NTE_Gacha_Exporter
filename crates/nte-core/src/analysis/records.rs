pub fn list_records(
    records: &[InternalRecord],
    map: &MapData,
    filter: &RecordFilter,
) -> Result<RecordList, GuiError> {
    let display_records = display_records(records, map)?;
    Ok(record_page_from_display_records(&display_records, filter))
}

fn record_page_from_display_records(
    display_records: &[DisplayRecord],
    filter: &RecordFilter,
) -> RecordList {
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
        if !filter.banner_ids.is_empty()
            && !display
                .derived
                .banner_id
                .as_ref()
                .is_some_and(|banner_id| filter.banner_ids.iter().any(|id| id == banner_id))
        {
            continue;
        }
        if !filter.rarities.is_empty()
            && !filter
                .rarities
                .iter()
                .any(|rarity| display.rarity == Some(*rarity))
        {
            continue;
        }
        if !filter.focused_rarities.is_empty()
            && !filter
                .focused_rarities
                .iter()
                .any(|rarity| matches_focused_rarity(display, *rarity))
        {
            continue;
        }
        if !filter.rate_up_results.is_empty()
            && !filter
                .rate_up_results
                .contains(&display.derived.rate_up_result)
        {
            continue;
        }
        if !filter.roll_buckets.is_empty()
            && !filter.roll_buckets.contains(&display.roll_bucket)
        {
            continue;
        }
        if !filter.item_kinds.is_empty()
            && !filter.item_kinds.contains(&display.item_kind)
        {
            continue;
        }
        if !filter.fork_result_marks.is_empty()
            && !display
                .fork_result_mark
                .as_ref()
                .is_some_and(|mark| filter.fork_result_marks.contains(mark))
        {
            continue;
        }
        if !filter.fork_pity_badges.is_empty()
            && !display
                .derived
                .pity_badge
                .as_ref()
                .is_some_and(|badge| filter.fork_pity_badges.contains(badge))
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
                "{} {} {} {} {} {} {} {} {} {}",
                display.record_id.to_ascii_lowercase(),
                display.record_type.to_ascii_lowercase(),
                display.item_id.to_ascii_lowercase(),
                display.item_name.to_ascii_lowercase(),
                display.pool_id.to_ascii_lowercase(),
                pool_label,
                banner_id,
                banner_title,
                rate_up_result_key(display.derived.rate_up_result),
                rule_id
            );
            if !haystack.contains(search) {
                continue;
            }
            matching.push(display.clone());
            continue;
        }
        matching.push(display.clone());
    }

    sort_display_records(&mut matching, filter.sort_direction.unwrap_or_default());
    let total = matching.len() as u64;
    let page = matching
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect::<Vec<_>>();
    RecordList {
        total,
        records: page,
    }
}

fn matches_focused_rarity(display: &DisplayRecord, rarity: u8) -> bool {
    match rarity {
        5 => is_focused_five_star_wall_record(display),
        3 | 4 => display.derived.hit_rarity == Some(rarity),
        _ => false,
    }
}

pub fn record_filter_options(
    records: &[InternalRecord],
    map: &MapData,
) -> Result<RecordFilterOptions, GuiError> {
    let display_records = display_records(records, map)?;
    Ok(record_filter_options_from_display_records(&display_records))
}

fn record_filter_options_from_display_records(records: &[DisplayRecord]) -> RecordFilterOptions {
    struct BannerOptionAccumulator {
        option: RecordBannerOption,
        latest_item_time: Option<String>,
    }

    let mut banners: BTreeMap<String, BannerOptionAccumulator> = BTreeMap::new();
    let mut roll_buckets: HashMap<RollBucket, u64> = HashMap::new();
    let mut item_kinds: HashMap<ItemKind, u64> = HashMap::new();

    for record in records {
        *roll_buckets.entry(record.roll_bucket).or_default() += 1;
        *item_kinds.entry(record.item_kind).or_default() += 1;
        if let Some(banner_id) = record.derived.banner_id.as_ref() {
            banners
                .entry(banner_id.clone())
                .and_modify(|accumulator| {
                    accumulator.option.count += 1;
                    if accumulator.latest_item_time.as_deref() < record.time.as_deref() {
                        accumulator.latest_item_time = record.time.clone();
                    }
                })
                .or_insert_with(|| BannerOptionAccumulator {
                    latest_item_time: record.time.clone(),
                    option: RecordBannerOption {
                        banner_id: banner_id.clone(),
                        pool_kind: record.pool_kind,
                        resolution_issue: record.banner.resolution_issue,
                        title: record
                            .banner
                            .title
                            .clone()
                            .unwrap_or_else(|| banner_id.clone()),
                        count: 1,
                    },
                });
        }
    }

    let mut banners = banners.into_values().collect::<Vec<_>>();
    banners.sort_by(|left, right| {
        compare_latest_item_time_desc(
            left.latest_item_time.as_deref(),
            right.latest_item_time.as_deref(),
        )
    });
    let banners = banners
        .into_iter()
        .map(|accumulator| accumulator.option)
        .collect();

    RecordFilterOptions {
        banners,
        roll_buckets: roll_bucket_order()
            .iter()
            .map(|bucket| RecordRollBucketOption {
                bucket: *bucket,
                count: *roll_buckets.get(bucket).unwrap_or(&0),
            })
            .collect(),
        item_kinds: item_kind_order()
            .iter()
            .map(|item_kind| RecordItemKindOption {
                item_kind: *item_kind,
                count: *item_kinds.get(item_kind).unwrap_or(&0),
            })
            .collect(),
    }
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

fn roll_bucket_from_fields(roll_label_id: Option<&str>, roll_points: Option<i64>) -> RollBucket {
    match roll_label_id {
        Some("BPUI_LotteryResult_jidianzengli") => return RollBucket::Gift,
        Some("BPUI_LotteryResult_chenmiandi") => return RollBucket::Sleep,
        _ => {}
    }
    match roll_points {
        Some(1) => RollBucket::One,
        Some(2) => RollBucket::Two,
        Some(3) => RollBucket::Three,
        Some(4) => RollBucket::Four,
        Some(5) => RollBucket::Five,
        Some(6) => RollBucket::Six,
        _ => RollBucket::NotApplicable,
    }
}

fn roll_bucket_order() -> &'static [RollBucket] {
    &[
        RollBucket::Gift,
        RollBucket::Sleep,
        RollBucket::One,
        RollBucket::Two,
        RollBucket::Three,
        RollBucket::Four,
        RollBucket::Five,
        RollBucket::Six,
        RollBucket::NotApplicable,
    ]
}

fn fork_result_mark_from_fields(
    pool_kind: PoolKind,
    derived: &RecordDerived,
) -> Option<ForkResultMark> {
    if pool_kind != PoolKind::ForkLottery || derived.hit_rarity != Some(5) {
        return None;
    }
    match derived.rate_up_result {
        RateUpResult::Up if fork_up_guarantee_hit(derived) => Some(ForkResultMark::Guaranteed),
        RateUpResult::Up => Some(ForkResultMark::Win),
        RateUpResult::OffRate => Some(ForkResultMark::Lose),
        RateUpResult::NotApplicable | RateUpResult::Unknown => None,
    }
}

fn fork_up_guarantee_hit(derived: &RecordDerived) -> bool {
    derived
        .fork_up_pity_before
        .zip(derived.rule.hard_up_pity_5)
        .is_some_and(|(before, hard)| before + 1 == hard)
}

fn item_kind_order() -> &'static [ItemKind] {
    &[
        ItemKind::Character,
        ItemKind::Fork,
        ItemKind::Appearance,
        ItemKind::Inventory,
        ItemKind::VehicleModule,
        ItemKind::Unknown,
    ]
}

fn display_record(
    record: &InternalRecord,
    map: &MapData,
    derived: RecordDerived,
) -> Result<DisplayRecord, GuiError> {
    let pool_kind = classify_pool_id(&record.pool_id)?;
    let item_id = map.canonical_item_id(&record.item_id).to_string();
    let item_kind = map.item_kind(&item_id);
    let roll_bucket =
        roll_bucket_from_fields(record.roll_label_id.as_deref(), record.roll_points);
    let fork_result_mark = fork_result_mark_from_fields(pool_kind, &derived);
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
        source_order: record.source_order,
        record_type: record.record_type.clone(),
        time: record.time.clone(),
        pool_kind,
        pool_id: record.pool_id.clone(),
        pool_label: map.pool_label(&record.pool_id, record.time.as_deref()),
        banner,
        item_id: item_id.clone(),
        item_name: map.item_name(&item_id),
        item_asset_refs,
        item_kind,
        rarity: map.item_rarity(&item_id),
        count: record.count,
        roll_points: record.roll_points,
        roll_label_id: record.roll_label_id.clone(),
        roll_label: record_label(record, map),
        roll_bucket,
        fork_result_mark,
        secondary_item_name: secondary_item_id
            .as_deref()
            .map(|item_id| map.item_name(item_id)),
        secondary_item_id,
        secondary_item_asset_refs,
        secondary_count: record.secondary_count,
        derived,
    })
}

fn sort_display_records(records: &mut [DisplayRecord], sort_direction: SortDirection) {
    records.sort_by(|left, right| match sort_direction {
        SortDirection::Asc => compare_record_page_oldest_first(left, right),
        SortDirection::Desc => compare_display_newest_first(left, right),
    });
}

fn compare_record_page_oldest_first(
    left: &DisplayRecord,
    right: &DisplayRecord,
) -> std::cmp::Ordering {
    compare_display_newest_first(right, left)
}

fn record_label(record: &InternalRecord, map: &MapData) -> Option<String> {
    record
        .roll_label_id
        .as_deref()
        .map(|label_id| map.label(label_id))
        .or_else(|| record.roll_points.map(|value| value.to_string()))
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
