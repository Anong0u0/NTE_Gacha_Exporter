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
