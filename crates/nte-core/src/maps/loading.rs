use std::collections::BTreeSet;

use chrono::NaiveDateTime;

pub fn available_locales() -> Vec<String> {
    BUNDLED_MAP_LOCALES
        .iter()
        .map(|(locale, _)| (*locale).to_string())
        .collect()
}

pub fn load_map(locale: &str) -> Result<MapData, GuiError> {
    let maps = MAP_CACHE
        .get_or_init(load_all_maps)
        .as_ref()
        .map_err(|error| GuiError::InvalidDocument(error.clone()))?;
    maps.get(locale)
        .cloned()
        .ok_or_else(|| GuiError::LocaleNotFound(locale.to_string()))
}

fn load_all_maps() -> Result<BTreeMap<&'static str, MapData>, String> {
    let core: MapCoreDocument = parse_document("core.json", BUNDLED_MAP_CORE)?;
    validate_core(&core)?;
    let catalog: BannerOccurrenceDocument =
        parse_document("banner-occurrences.json", BUNDLED_OCCURRENCES)?;
    validate_catalog(&catalog, &core)?;

    let mut maps = BTreeMap::new();
    for (locale, text) in BUNDLED_MAP_LOCALES {
        let overlay: MapLocaleDocument = parse_document(locale, text)?;
        maps.insert(*locale, assemble_map(locale, &core, &overlay, &catalog)?);
    }
    Ok(maps)
}

fn parse_document<T: serde::de::DeserializeOwned>(source: &str, text: &str) -> Result<T, String> {
    serde_json::from_str(text).map_err(|error| format!("{source}: {error}"))
}

fn validate_core(core: &MapCoreDocument) -> Result<(), String> {
    validate_schema(
        "core.json",
        &core.schema,
        core.schema_version,
        MAP_CORE_SCHEMA,
        MAP_CORE_SCHEMA_VERSION,
    )?;
    let mut folded = BTreeMap::new();
    for item_id in core.items.keys() {
        let key = item_id.to_ascii_lowercase();
        if let Some(existing) = folded.insert(key, item_id) {
            return Err(format!(
                "core.json: case-insensitive duplicate item_id: {existing} and {item_id}"
            ));
        }
    }
    for (source_id, source) in &core.banner_sources {
        if source.source_id != *source_id {
            return Err(format!(
                "core.json: banner source key/id mismatch: {source_id}"
            ));
        }
        if !core.pools.contains_key(&source.pool_id) {
            return Err(format!(
                "core.json: banner source targets unknown pool: {source_id} -> {}",
                source.pool_id
            ));
        }
        if !core.gacha_rules.contains_key(&source.rule_id) {
            return Err(format!(
                "core.json: banner source targets unknown rule: {source_id} -> {}",
                source.rule_id
            ));
        }
    }
    for (rule_id, rule) in &core.gacha_rules {
        if rule.rule_id != *rule_id {
            return Err(format!("core.json: rule key/id mismatch: {rule_id}"));
        }
    }
    Ok(())
}

fn validate_catalog(
    catalog: &BannerOccurrenceDocument,
    core: &MapCoreDocument,
) -> Result<(), String> {
    validate_schema(
        "banner-occurrences.json",
        &catalog.schema,
        catalog.schema_version,
        BANNER_OCCURRENCES_SCHEMA,
        BANNER_OCCURRENCES_SCHEMA_VERSION,
    )?;
    let mut ids = BTreeSet::new();
    let mut by_source: BTreeMap<&str, Vec<(&str, NaiveDateTime, NaiveDateTime)>> =
        BTreeMap::new();
    for occurrence in &catalog.occurrences {
        if occurrence.occurrence_id.trim().is_empty() || occurrence.source_id.trim().is_empty() {
            return Err("banner-occurrences.json: occurrence IDs must be non-empty".to_string());
        }
        if !ids.insert(occurrence.occurrence_id.as_str()) {
            return Err(format!(
                "banner-occurrences.json: duplicate occurrence_id: {}",
                occurrence.occurrence_id
            ));
        }
        if !core.banner_sources.contains_key(&occurrence.source_id) {
            return Err(format!(
                "banner-occurrences.json: unknown source_id: {}",
                occurrence.source_id
            ));
        }
        if occurrence.source_id == "Lottery_Permanent" {
            if occurrence.active_from.is_some() || occurrence.active_until.is_some() {
                return Err(
                    "banner-occurrences.json: permanent occurrence must be unbounded".to_string(),
                );
            }
            continue;
        }
        let start = parse_catalog_time(occurrence.active_from.as_deref(), &occurrence.occurrence_id)?;
        let end = parse_catalog_time(occurrence.active_until.as_deref(), &occurrence.occurrence_id)?;
        if start >= end {
            return Err(format!(
                "banner-occurrences.json: active_from must be before active_until: {}",
                occurrence.occurrence_id
            ));
        }
        by_source.entry(&occurrence.source_id).or_default().push((
            &occurrence.occurrence_id,
            start,
            end,
        ));
    }
    for (source_id, windows) in &mut by_source {
        windows.sort_by_key(|(_, start, _)| *start);
        for pair in windows.windows(2) {
            if pair[0].2 > pair[1].1 {
                return Err(format!(
                    "banner-occurrences.json: overlapping windows for {source_id}: {} and {}",
                    pair[0].0, pair[1].0
                ));
            }
        }
    }
    Ok(())
}

fn assemble_map(
    locale: &str,
    core: &MapCoreDocument,
    overlay: &MapLocaleDocument,
    catalog: &BannerOccurrenceDocument,
) -> Result<MapData, String> {
    validate_schema(
        locale,
        &overlay.schema,
        overlay.schema_version,
        MAP_LOCALE_SCHEMA,
        MAP_LOCALE_SCHEMA_VERSION,
    )?;
    if overlay.locale != locale {
        return Err(format!("{locale}: locale field is {}", overlay.locale));
    }
    validate_same_keys(locale, "items", &core.items, &overlay.items)?;
    validate_same_keys(locale, "pools", &core.pools, &overlay.pools)?;
    validate_same_keys(
        locale,
        "banner_sources",
        &core.banner_sources,
        &overlay.banner_sources,
    )?;

    let items = core
        .items
        .iter()
        .map(|(item_id, item)| {
            let localized = &overlay.items[item_id];
            if localized.name.trim().is_empty() {
                return Err(format!("{locale}: empty item name: {item_id}"));
            }
            Ok((
                item_id.clone(),
                MapItem {
                    name: localized.name.clone(),
                    rarity: item.rarity,
                    category: item.category.clone(),
                    subtype: item.subtype.clone(),
                    asset_refs: item.asset_refs.clone(),
                    color: item.color.clone(),
                },
            ))
        })
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    let pools = core
        .pools
        .iter()
        .map(|(pool_id, pool)| {
            let localized = &overlay.pools[pool_id];
            if localized.name.trim().is_empty() {
                return Err(format!("{locale}: empty pool name: {pool_id}"));
            }
            Ok((
                pool_id.clone(),
                MapPool {
                    name: localized.name.clone(),
                    group_label: localized.group_label.clone(),
                    title: localized.title.clone(),
                    pickup_item_ids: pool.pickup_item_ids.clone(),
                },
            ))
        })
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    let banner_sources = core
        .banner_sources
        .iter()
        .map(|(source_id, source)| {
            let localized = &overlay.banner_sources[source_id];
            if localized.title.trim().is_empty() {
                return Err(format!("{locale}: empty banner title: {source_id}"));
            }
            Ok((
                source_id.clone(),
                MapBannerSource {
                    source_id: source.source_id.clone(),
                    pool_id: source.pool_id.clone(),
                    pool_kind: source.pool_kind.clone(),
                    banner_type: source.banner_type.clone(),
                    title: localized.title.clone(),
                    short_title: localized.short_title.clone(),
                    rate_up_5: source.rate_up_5.clone(),
                    rate_up_4: source.rate_up_4.clone(),
                    rule_id: source.rule_id.clone(),
                    asset_refs: source.asset_refs.clone(),
                    color: source.color.clone(),
                    currency_id: source.currency_id.clone(),
                    currency_count: source.currency_count,
                    roll_unit: source.roll_unit,
                },
            ))
        })
        .collect::<Result<BTreeMap<_, _>, String>>()?;

    Ok(MapData {
        csv_headers: overlay.csv_headers.clone(),
        labels: overlay.labels.clone(),
        items,
        item_aliases: core.item_aliases.clone(),
        pools,
        banner_sources,
        banner_occurrences: catalog.occurrences.clone(),
        gacha_rules: core.gacha_rules.clone(),
    })
}

fn validate_schema(
    source: &str,
    actual_schema: &str,
    actual_version: u64,
    expected_schema: &str,
    expected_version: u64,
) -> Result<(), String> {
    if actual_schema != expected_schema || actual_version != expected_version {
        return Err(format!(
            "{source}: expected {expected_schema} schema_version {expected_version}"
        ));
    }
    Ok(())
}

fn validate_same_keys<A, B>(
    locale: &str,
    section: &str,
    core: &BTreeMap<String, A>,
    localized: &BTreeMap<String, B>,
) -> Result<(), String> {
    if core.keys().eq(localized.keys()) {
        Ok(())
    } else {
        Err(format!(
            "{locale}: locale keys must exactly match core: {section}"
        ))
    }
}

fn parse_catalog_time(value: Option<&str>, occurrence_id: &str) -> Result<NaiveDateTime, String> {
    let value = value.ok_or_else(|| {
        format!("banner-occurrences.json: bounded occurrence missing time: {occurrence_id}")
    })?;
    NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S").map_err(|error| {
        format!("banner-occurrences.json: invalid time for {occurrence_id}: {error}")
    })
}

#[cfg(test)]
mod loading_tests {
    use super::*;

    fn core() -> MapCoreDocument {
        serde_json::from_str(BUNDLED_MAP_CORE).expect("bundled core should parse")
    }

    fn occurrence(
        occurrence_id: &str,
        source_id: &str,
        active_from: &str,
        active_until: &str,
    ) -> MapBannerOccurrence {
        MapBannerOccurrence {
            occurrence_id: occurrence_id.to_string(),
            source_id: source_id.to_string(),
            active_from: Some(active_from.to_string()),
            active_until: Some(active_until.to_string()),
            version: None,
        }
    }

    fn catalog(occurrences: Vec<MapBannerOccurrence>) -> BannerOccurrenceDocument {
        BannerOccurrenceDocument {
            schema: BANNER_OCCURRENCES_SCHEMA.to_string(),
            schema_version: BANNER_OCCURRENCES_SCHEMA_VERSION,
            occurrences,
        }
    }

    #[test]
    fn catalog_rejects_duplicate_occurrence_id() {
        let entry = occurrence(
            "Lottery_Nanali@test",
            "Lottery_Nanali",
            "2027-01-01 00:00:00",
            "2027-02-01 00:00:00",
        );
        let error = validate_catalog(&catalog(vec![entry.clone(), entry]), &core())
            .expect_err("duplicate occurrence ID should fail");

        assert!(error.contains("duplicate occurrence_id"));
    }

    #[test]
    fn catalog_rejects_unknown_source_and_invalid_time() {
        let unknown = occurrence(
            "Lottery_Unknown@test",
            "Lottery_Unknown",
            "2027-01-01 00:00:00",
            "2027-02-01 00:00:00",
        );
        let error = validate_catalog(&catalog(vec![unknown]), &core())
            .expect_err("unknown source should fail");
        assert!(error.contains("unknown source_id"));

        let invalid = occurrence(
            "Lottery_Nanali@test",
            "Lottery_Nanali",
            "not-a-time",
            "2027-02-01 00:00:00",
        );
        let error = validate_catalog(&catalog(vec![invalid]), &core())
            .expect_err("invalid time should fail");
        assert!(error.contains("invalid time"));
    }

    #[test]
    fn catalog_rejects_same_source_overlap_but_allows_other_sources() {
        let first = occurrence(
            "Lottery_Nanali@test-a",
            "Lottery_Nanali",
            "2027-01-01 00:00:00",
            "2027-02-01 00:00:00",
        );
        let overlap = occurrence(
            "Lottery_Nanali@test-b",
            "Lottery_Nanali",
            "2027-01-15 00:00:00",
            "2027-02-15 00:00:00",
        );
        let error = validate_catalog(&catalog(vec![first.clone(), overlap]), &core())
            .expect_err("same source overlap should fail");
        assert!(error.contains("overlapping windows"));

        let other_source = occurrence(
            "Lottery_Xun@test",
            "Lottery_Xun",
            "2027-01-15 00:00:00",
            "2027-02-15 00:00:00",
        );
        validate_catalog(&catalog(vec![first, other_source]), &core())
            .expect("different sources may overlap");
    }

    #[test]
    fn catalog_rejects_wrong_schema_and_bounded_permanent_pool() {
        let mut wrong_schema = catalog(Vec::new());
        wrong_schema.schema_version += 1;
        let error = validate_catalog(&wrong_schema, &core())
            .expect_err("unknown schema version should fail");
        assert!(error.contains("expected nte-banner-occurrences schema_version 1"));

        let permanent = occurrence(
            "Lottery_Permanent@test",
            "Lottery_Permanent",
            "2027-01-01 00:00:00",
            "2027-02-01 00:00:00",
        );
        let error = validate_catalog(&catalog(vec![permanent]), &core())
            .expect_err("permanent pool must be unbounded");
        assert!(error.contains("permanent occurrence must be unbounded"));
    }
}
