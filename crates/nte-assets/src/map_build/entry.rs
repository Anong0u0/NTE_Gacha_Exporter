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
            let map = build_asset_map(assets_root, &locale)?;
            let item_count = section_len(&map, "items");
            let pool_count = section_len(&map, "pools");
            let label_count = section_len(&map, "labels");
            Ok(AssetMapBuild {
                locale,
                map,
                item_count,
                pool_count,
                label_count,
            })
        })
        .collect()
}

pub fn build_asset_map(assets_root: &Path, locale: &str) -> Result<Value, GuiError> {
    let localization = load_localization(assets_root, locale)?;
    let (items, item_ctx) = build_item_data(assets_root, localization)?;
    let (pools, mut pool_meta) =
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
    attach_banner_ids(&mut pool_meta, &banners);

    let mut map = JsonObject::new();
    map.insert("schema_version".to_string(), json!(MAP_SCHEMA_VERSION));
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
    validate_map_source(&value, &format!("{locale}.json"))?;
    Ok(value)
}

