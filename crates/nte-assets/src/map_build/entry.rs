pub fn find_assets_root(explicit: Option<&Path>) -> Result<PathBuf, GuiError> {
    let roots = if let Some(path) = explicit {
        vec![path.to_path_buf()]
    } else {
        env::var_os("NTE_ASSETS_ROOT")
            .map(PathBuf::from)
            .into_iter()
            .collect()
    };
    if roots.is_empty() {
        return Err(invalid(
            "NTE_Assets root not set. Pass --assets-root or set NTE_ASSETS_ROOT.",
        ));
    }
    for root in &roots {
        let expanded = expand_home(root);
        if expanded.join("DataTable").exists() && expanded.join("Localization").exists() {
            return Ok(expanded);
        }
    }
    let checked = roots
        .iter()
        .map(|path| expand_home(path).display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    Err(invalid(format!(
        "NTE_Assets root not found. Checked: {checked}"
    )))
}

pub fn discover_asset_locales(assets_root: &Path) -> Result<Vec<String>, GuiError> {
    let mut locales = Vec::new();
    let root = assets_root.join("Localization");
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let locale = entry.file_name().to_string_lossy().to_string();
        if REMOVED_MAP_LOCALES.contains(&locale.as_str()) {
            continue;
        }
        if entry.path().join("game.json").exists() {
            locales.push(locale);
        }
    }
    locales.sort();
    Ok(locales)
}

pub fn build_asset_maps(
    assets_root: &Path,
    locale: Option<&str>,
) -> Result<Vec<AssetMapBuild>, GuiError> {
    let locales = match locale {
        Some(locale) => vec![locale.to_string()],
        None => discover_asset_locales(assets_root)?,
    };
    locales
        .into_iter()
        .filter(|locale| !REMOVED_MAP_LOCALES.contains(&locale.as_str()))
        .map(|locale| {
            let map = build_full_asset_map(assets_root, &locale)?;
            let item_count = section_len(&map, "items");
            let pool_count = section_len(&map, "pools");
            let label_count = section_len(&map, "labels");
            let (core, overlay) = split_asset_map(&map, &locale)?;
            Ok(AssetMapBuild {
                locale,
                core,
                overlay,
                item_count,
                pool_count,
                label_count,
            })
        })
        .collect()
}

fn build_full_asset_map(assets_root: &Path, locale: &str) -> Result<Value, GuiError> {
    let localization = load_localization(assets_root, locale)?;
    let (items, item_ctx) = build_item_data(assets_root, localization)?;
    let (pools, pool_meta) =
        build_pools(assets_root, &item_ctx.localization, &item_ctx.canonicalizer)?;
    let item_meta = build_item_meta_rows(assets_root, &items, &item_ctx.canonicalizer)?;
    let normalized_items = normalized_items(&items, &item_meta);
    let banners = build_banners(
        assets_root,
        locale,
        &item_ctx.localization,
        &item_ctx.canonicalizer,
        &normalized_items,
    )?;
    let mut map = JsonObject::new();
    map.insert(
        "csv_headers".to_string(),
        Value::Object(csv_headers(&item_ctx.localization, locale)),
    );
    map.insert("items".to_string(), Value::Object(normalized_items));
    map.insert(
        "item_aliases".to_string(),
        Value::Object(string_map_value(item_ctx.item_aliases)),
    );
    map.insert(
        "pools".to_string(),
        Value::Object(normalized_pools(&pools, &pool_meta)),
    );
    map.insert("banners".to_string(), Value::Object(banners));
    map.insert(
        "gacha_rules".to_string(),
        Value::Object(build_gacha_rules(
            assets_root,
            locale,
            &item_ctx.canonicalizer,
        )?),
    );
    map.insert(
        "labels".to_string(),
        Value::Object(string_map_value(build_labels(&item_ctx.localization))),
    );
    let value = Value::Object(map);
    validate_full_map_source(&value, &format!("{locale}.json"))?;
    Ok(value)
}

fn split_asset_map(map: &Value, locale: &str) -> Result<(Value, Value), GuiError> {
    let object = map
        .as_object()
        .ok_or_else(|| invalid(format!("generated map must be an object: {locale}")))?;
    let items = required_section(object, locale, "items")?;
    let pools = required_section(object, locale, "pools")?;
    let banners = required_section(object, locale, "banners")?;

    let mut core_items = JsonObject::new();
    let mut locale_items = JsonObject::new();
    for (item_id, value) in items {
        let mut item = value
            .as_object()
            .cloned()
            .ok_or_else(|| invalid(format!("item must be object: {locale}:items.{item_id}")))?;
        let name = item
            .remove("name")
            .ok_or_else(|| invalid(format!("item name missing: {locale}:items.{item_id}")))?;
        core_items.insert(item_id.clone(), Value::Object(item));
        locale_items.insert(item_id.clone(), Value::Object(map_from_pairs([("name", name)])));
    }

    let mut core_pools = JsonObject::new();
    let mut locale_pools = JsonObject::new();
    for (pool_id, value) in pools {
        let mut pool = value
            .as_object()
            .cloned()
            .ok_or_else(|| invalid(format!("pool must be object: {locale}:pools.{pool_id}")))?;
        let mut localized = JsonObject::new();
        for key in ["name", "group_label", "title"] {
            if let Some(value) = pool.remove(key) {
                localized.insert(key.to_string(), value);
            }
        }
        core_pools.insert(pool_id.clone(), Value::Object(pool));
        locale_pools.insert(pool_id.clone(), Value::Object(localized));
    }

    let mut core_banners = JsonObject::new();
    let mut locale_banners = JsonObject::new();
    for (source_id, value) in banners {
        let mut banner = value.as_object().cloned().ok_or_else(|| {
            invalid(format!(
                "banner must be object: {locale}:banners.{source_id}"
            ))
        })?;
        let mut localized = JsonObject::new();
        for key in ["title", "short_title"] {
            if let Some(value) = banner.remove(key) {
                localized.insert(key.to_string(), value);
            }
        }
        for key in [
            "version",
            "start_at",
            "end_at",
            "mainland_start_at",
            "mainland_end_at",
            "timezone",
        ] {
            banner.remove(key);
        }
        banner.remove("banner_id");
        banner.insert("source_id".to_string(), Value::String(source_id.clone()));
        core_banners.insert(source_id.clone(), Value::Object(banner));
        locale_banners.insert(source_id.clone(), Value::Object(localized));
    }

    let core = Value::Object(map_from_pairs([
        ("schema", json!(MAP_CORE_SCHEMA)),
        ("schema_version", json!(MAP_CORE_SCHEMA_VERSION)),
        ("items", Value::Object(core_items)),
        (
            "item_aliases",
            object
                .get("item_aliases")
                .cloned()
                .unwrap_or_else(|| json!({})),
        ),
        ("pools", Value::Object(core_pools)),
        ("banner_sources", Value::Object(core_banners)),
        (
            "gacha_rules",
            object
                .get("gacha_rules")
                .cloned()
                .unwrap_or_else(|| json!({})),
        ),
    ]));
    let overlay = Value::Object(map_from_pairs([
        ("schema", json!(MAP_LOCALE_SCHEMA)),
        ("schema_version", json!(MAP_LOCALE_SCHEMA_VERSION)),
        ("locale", json!(locale)),
        (
            "csv_headers",
            object
                .get("csv_headers")
                .cloned()
                .unwrap_or_else(|| json!({})),
        ),
        (
            "labels",
            object
                .get("labels")
                .cloned()
                .unwrap_or_else(|| json!({})),
        ),
        ("items", Value::Object(locale_items)),
        ("pools", Value::Object(locale_pools)),
        ("banner_sources", Value::Object(locale_banners)),
    ]));
    validate_core_source(&core, "generated core")?;
    validate_locale_source(&overlay, &core, locale)?;
    Ok((core, overlay))
}
