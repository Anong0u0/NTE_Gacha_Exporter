fn asset_path(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_object)
        .and_then(|object| object.get("AssetPathName"))
        .and_then(Value::as_str)
        .filter(|path| path.starts_with("/Game/"))
        .map(ToString::to_string)
}

fn fork_banner_asset_refs(row: &JsonObject) -> JsonObject {
    let mut refs = JsonObject::new();
    if let Some(background) = asset_path(row.get("Bg")) {
        refs.insert("background".to_string(), Value::String(background));
    }
    if let Some(icon) = asset_path(row.get("Icon")) {
        refs.insert("icon".to_string(), Value::String(icon));
    }
    refs
}

fn standard_banner(
    locale: &str,
    localization: &Localization,
    standard_5_pool: Vec<String>,
    standard_4_pool: Vec<String>,
) -> Option<JsonObject> {
    let title = localized_monopoly_pool_title(localization, STANDARD_MONOPOLY_TITLE_TAIL)?;
    let mut banner = JsonObject::new();
    banner.insert(
        "banner_id".to_string(),
        Value::String("monopoly_standard".to_string()),
    );
    banner.insert(
        "pool_id".to_string(),
        Value::String("CardPool_NewRole".to_string()),
    );
    banner.insert(
        "pool_kind".to_string(),
        Value::String("monopoly_standard".to_string()),
    );
    banner.insert(
        "banner_type".to_string(),
        Value::String("standard".to_string()),
    );
    banner.insert("title".to_string(), Value::String(title));
    banner.insert("rate_up_5".to_string(), Value::Array(Vec::new()));
    banner.insert("rate_up_4".to_string(), Value::Array(Vec::new()));
    banner.insert(
        "standard_5_pool".to_string(),
        Value::Array(standard_5_pool.into_iter().map(Value::String).collect()),
    );
    banner.insert(
        "standard_4_pool".to_string(),
        Value::Array(standard_4_pool.into_iter().map(Value::String).collect()),
    );
    banner.insert(
        "rule_id".to_string(),
        Value::String("monopoly_standard".to_string()),
    );
    banner.insert(
        "source".to_string(),
        source_evidence(
            &[
                MONOPOLY_LOTTERY_TABLE.to_string(),
                format!("Localization/{locale}/game.json"),
            ],
            &["Standard pool uses the available monopoly lottery table; banner instance is not explicit."],
        ),
    );
    Some(banner)
}

fn limited_banners(
    locale: &str,
    localization: &Localization,
    canonicalizer: &ItemCanonicalizer,
    known_item_ids: &BTreeSet<String>,
    normalized_items: &JsonObject,
    standard_5_pool: &[String],
    standard_4_pool: &[String],
) -> JsonObject {
    let mut banners = JsonObject::new();
    let mut previous_end: Option<String> = None;
    for banner in LIMITED_BANNER_SEEDS {
        let Some(title) = localized_monopoly_pool_title(localization, banner.tail) else {
            previous_end = Some(banner.end_at_tz8.to_string());
            continue;
        };
        let rate_up_5 = item_ref_list(banner.rate_up_5, canonicalizer, known_item_ids);
        let mut asset_refs = JsonObject::new();
        let portraits = featured_portraits(normalized_items, &rate_up_5);
        if !portraits.is_empty() {
            asset_refs.insert(
                "featured_portraits".to_string(),
                Value::Array(portraits.into_iter().map(Value::String).collect()),
            );
        }
        if rate_up_5.len() == 1 {
            if let Some(image) = item_asset_ref(normalized_items, &rate_up_5[0], "banner") {
                asset_refs.insert("image".to_string(), Value::String(image));
            }
        }

        let mut entry = JsonObject::new();
        entry.insert(
            "banner_id".to_string(),
            Value::String(banner.banner_id.to_string()),
        );
        entry.insert(
            "pool_id".to_string(),
            Value::String("CardPool_Character".to_string()),
        );
        entry.insert(
            "pool_kind".to_string(),
            Value::String("monopoly_limited".to_string()),
        );
        entry.insert(
            "banner_type".to_string(),
            Value::String("limited".to_string()),
        );
        entry.insert("title".to_string(), Value::String(title));
        entry.insert(
            "end_at".to_string(),
            Value::String(banner.end_at_tz8.to_string()),
        );
        entry.insert(
            "timezone".to_string(),
            Value::String("Asia/Shanghai".to_string()),
        );
        entry.insert(
            "rate_up_5".to_string(),
            Value::Array(rate_up_5.into_iter().map(Value::String).collect()),
        );
        entry.insert("rate_up_4".to_string(), Value::Array(Vec::new()));
        entry.insert(
            "standard_5_pool".to_string(),
            Value::Array(standard_5_pool.iter().cloned().map(Value::String).collect()),
        );
        entry.insert(
            "standard_4_pool".to_string(),
            Value::Array(standard_4_pool.iter().cloned().map(Value::String).collect()),
        );
        entry.insert(
            "rule_id".to_string(),
            Value::String("monopoly_limited".to_string()),
        );
        entry.insert(
            "source".to_string(),
            source_evidence(
                &[
                    MONOPOLY_LOTTERY_TABLE.to_string(),
                    format!("Localization/{locale}/game.json"),
                ],
                &["Schedule and rate-up use available lottery data and localized text."],
            ),
        );
        if let Some(version) = banner.version {
            entry.insert("version".to_string(), Value::String(version.to_string()));
        }
        if let Some(start_at) = previous_end.clone() {
            entry.insert("start_at".to_string(), Value::String(start_at));
        }
        if !asset_refs.is_empty() {
            entry.insert("asset_refs".to_string(), Value::Object(asset_refs));
        }
        banners.insert(banner.banner_id.to_string(), Value::Object(entry));
        previous_end = Some(banner.end_at_tz8.to_string());
    }
    banners
}

fn fork_banners(
    assets_root: &Path,
    localization: &Localization,
    canonicalizer: &ItemCanonicalizer,
) -> Result<JsonObject, GuiError> {
    let mut banners = JsonObject::new();
    for (pool_id, row) in fork_pool_rows(assets_root)? {
        let Some(row) = row.as_object() else {
            continue;
        };
        if !pool_id.starts_with("ForkLottery_") {
            continue;
        }
        let Some(title) = clean_name(localized_text(row.get("ShowText1"), localization)) else {
            continue;
        };
        let mut banner = JsonObject::new();
        banner.insert("banner_id".to_string(), Value::String(pool_id.clone()));
        banner.insert("pool_id".to_string(), Value::String(pool_id.clone()));
        banner.insert(
            "pool_kind".to_string(),
            Value::String("fork_lottery".to_string()),
        );
        banner.insert("banner_type".to_string(), Value::String("fork".to_string()));
        banner.insert("title".to_string(), Value::String(title));
        banner.insert(
            "rate_up_5".to_string(),
            Value::Array(
                fork_pickup_item_ids(&pool_id, row, canonicalizer)?
                    .into_iter()
                    .map(Value::String)
                    .collect(),
            ),
        );
        banner.insert("rate_up_4".to_string(), Value::Array(Vec::new()));
        banner.insert(
            "rule_id".to_string(),
            Value::String("fork_lottery_s".to_string()),
        );
        banner.insert(
            "source".to_string(),
            source_evidence(&[FORK_POOL_TABLE.to_string()], &[]),
        );
        let refs = fork_banner_asset_refs(row);
        if !refs.is_empty() {
            banner.insert("asset_refs".to_string(), Value::Object(refs));
        }
        if let Some(currency_id) = row
            .get("CurrencyID")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
        {
            banner.insert(
                "currency_id".to_string(),
                Value::String(canonicalizer.canonicalize(currency_id)),
            );
        }
        for (source_key, target_key) in [
            ("CurrencyCnt", "currency_count"),
            ("OnceLotteryCnt", "roll_unit"),
        ] {
            if let Some(value) = row.get(source_key).and_then(Value::as_u64) {
                banner.insert(target_key.to_string(), json!(value));
            }
        }
        banners.insert(pool_id, Value::Object(banner));
    }
    Ok(banners)
}

fn build_banners(
    assets_root: &Path,
    locale: &str,
    localization: &Localization,
    canonicalizer: &ItemCanonicalizer,
    normalized_items: &JsonObject,
) -> Result<JsonObject, GuiError> {
    let known_item_ids = normalized_items.keys().cloned().collect::<BTreeSet<_>>();
    let standard_5_pool =
        lottery_item_ids(assets_root, "SSRItems", canonicalizer, &known_item_ids)?;
    let standard_4_pool = lottery_item_ids(assets_root, "SRItems", canonicalizer, &known_item_ids)?;
    let mut banners = JsonObject::new();
    if let Some(standard) = standard_banner(
        locale,
        localization,
        standard_5_pool.clone(),
        standard_4_pool.clone(),
    ) {
        banners.insert("monopoly_standard".to_string(), Value::Object(standard));
    }
    banners.extend(limited_banners(
        locale,
        localization,
        canonicalizer,
        &known_item_ids,
        normalized_items,
        &standard_5_pool,
        &standard_4_pool,
    ));
    banners.extend(fork_banners(assets_root, localization, canonicalizer)?);
    Ok(banners)
}

fn attach_banner_ids(pool_meta: &mut BTreeMap<String, JsonObject>, banners: &JsonObject) {
    let mut banner_ids_by_pool: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (banner_id, banner) in banners {
        let Some(pool_id) = banner
            .as_object()
            .and_then(|banner| banner.get("pool_id"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        banner_ids_by_pool
            .entry(pool_id.to_string())
            .or_default()
            .push(banner_id.clone());
    }
    for (pool_id, mut banner_ids) in banner_ids_by_pool {
        banner_ids.sort();
        pool_meta
            .entry(pool_id)
            .or_default()
            .insert("banner_ids".to_string(), json!(banner_ids));
    }
}
