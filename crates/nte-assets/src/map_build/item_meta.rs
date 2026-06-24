fn asset_refs_from_fields(row: &JsonObject, fields: &[(&str, &str)]) -> JsonObject {
    let mut refs = JsonObject::new();
    for (source_field, target_field) in fields {
        if let Some(asset_path) = asset_path(row.get(*source_field)) {
            refs.insert((*target_field).to_string(), Value::String(asset_path));
        }
    }
    refs
}

fn asset_ref_str<'a>(refs: &'a JsonObject, key: &str) -> Option<&'a str> {
    refs.get(key).and_then(Value::as_str)
}

fn same_asset_ref(refs: &JsonObject, left: &str, right: &str) -> bool {
    asset_ref_str(refs, left).zip(asset_ref_str(refs, right)).is_some_and(
        |(left, right)| left == right,
    )
}

fn prune_item_asset_refs(refs: &mut JsonObject) {
    if same_asset_ref(refs, "icon", "portrait") {
        refs.remove("icon");
    }
    if same_asset_ref(refs, "head_icon", "icon") || same_asset_ref(refs, "head_icon", "portrait") {
        refs.remove("head_icon");
    }
    if same_asset_ref(refs, "material", "banner") {
        refs.remove("material");
    }
}

fn rarity_from_quality(value: Option<&Value>) -> Option<u64> {
    let text = value.and_then(value_to_text)?;
    match text.rsplit("::").next().unwrap_or(&text) {
        "ITEM_QUALITY_WHITE" => Some(1),
        "ITEM_QUALITY_GREEN" => Some(2),
        "ITEM_QUALITY_BLUE" => Some(3),
        "ITEM_QUALITY_PURPLE" => Some(4),
        "ITEM_QUALITY_ORANGE" => Some(5),
        _ => None,
    }
}

fn row_item_meta(row: &JsonObject, category: &str) -> JsonObject {
    let mut meta = JsonObject::new();
    meta.insert("category".to_string(), Value::String(category.to_string()));
    meta.insert(
        "domain_type".to_string(),
        Value::String(category.to_string()),
    );
    if let Some(rarity) = rarity_from_quality(row.get("ItemQuality").or_else(|| row.get("Quality")))
    {
        meta.insert("rarity".to_string(), json!(rarity));
    }
    let fields = if category == "appearance" {
        &[
            ("DisplayIcon", "icon"),
            ("HeadIcon", "head_icon"),
            ("HeadIconBig", "portrait"),
            ("PortraitImg", "banner"),
        ][..]
    } else {
        &[
            ("ItemIcon", "icon"),
            ("ItemIconSmall", "head_icon"),
            ("ItemIconBig", "portrait"),
        ][..]
    };
    let refs = asset_refs_from_fields(row, fields);
    if !refs.is_empty() {
        meta.insert("asset_refs".to_string(), Value::Object(refs));
    }
    meta
}

fn merge_item_meta(meta: &mut BTreeMap<String, JsonObject>, item_id: String, patch: JsonObject) {
    let existing = meta.entry(item_id).or_default();
    for (key, value) in patch {
        if value.is_null() {
            continue;
        }
        match (existing.get_mut(&key), value) {
            (Some(Value::Object(existing_child)), Value::Object(patch_child)) => {
                existing_child.extend(patch_child);
            }
            (_, value) => {
                existing.insert(key, value);
            }
        }
    }
}

fn add_vehicle_module_meta(
    meta: &mut BTreeMap<String, JsonObject>,
    assets_root: &Path,
    known_ids: &BTreeSet<String>,
    canonicalizer: &ItemCanonicalizer,
) -> Result<(), GuiError> {
    let path = assets_root.join("DataTable/Vehicle/DT_vehicleModuleData.json");
    if !path.exists() {
        return Ok(());
    }
    for row in rows_from_datatable(&path)?.values() {
        let Some(row) = row.as_object() else {
            continue;
        };
        for item_id in vehicle_module_item_ids(row) {
            let item_id = canonicalizer.canonicalize(&item_id);
            if !known_ids.contains(&item_id) {
                continue;
            }
            let mut patch = JsonObject::new();
            patch.insert(
                "category".to_string(),
                Value::String("vehicle_module".to_string()),
            );
            patch.insert(
                "domain_type".to_string(),
                Value::String("vehicle_module".to_string()),
            );
            let refs = asset_refs_from_fields(
                row,
                &[
                    ("UnLockNormalIcon", "icon"),
                    ("UnLockSelectedIcon", "head_icon"),
                    ("NameplateBg", "banner"),
                ],
            );
            if !refs.is_empty() {
                patch.insert("asset_refs".to_string(), Value::Object(refs));
            }
            merge_item_meta(meta, item_id, patch);
        }
    }
    Ok(())
}

fn add_gacha_illustrate_meta(
    meta: &mut BTreeMap<String, JsonObject>,
    assets_root: &Path,
    known_ids: &BTreeSet<String>,
    canonicalizer: &ItemCanonicalizer,
) -> Result<(), GuiError> {
    let path = assets_root.join(GACHA_ILLUSTRATE_TABLE);
    if !path.exists() {
        return Ok(());
    }
    for (raw_item_id, row) in rows_from_table(&path)? {
        let Some(row) = row.as_object() else {
            continue;
        };
        let item_id = canonicalizer.canonicalize(&raw_item_id);
        if !known_ids.contains(&item_id) {
            continue;
        }
        let mut patch = JsonObject::new();
        let refs = asset_refs_from_fields(
            row,
            &[
                ("HeadIcon", "icon"),
                ("ItemIcon", "portrait"),
                ("ActivityHeadIcon", "banner"),
                ("MaterialTexture", "material"),
            ],
        );
        if !refs.is_empty() {
            patch.insert("asset_refs".to_string(), Value::Object(refs));
        }
        if let Some(color) = row_hex_color(row) {
            patch.insert("color".to_string(), Value::String(color));
        }
        merge_item_meta(meta, item_id, patch);
    }
    Ok(())
}

fn row_hex_color(row: &JsonObject) -> Option<String> {
    for field in ["OutlineColor", "BloomColor"] {
        let Some(value) = row
            .get(field)
            .and_then(Value::as_object)
            .and_then(|object| object.get("Hex"))
            .and_then(value_to_text)
        else {
            continue;
        };
        let text = value.trim().trim_start_matches('#');
        if text.len() == 6 && text.chars().all(|char| char.is_ascii_hexdigit()) {
            return Some(format!("#{}", text.to_uppercase()));
        }
    }
    None
}

fn build_item_meta_rows(
    assets_root: &Path,
    items: &BTreeMap<String, String>,
    canonicalizer: &ItemCanonicalizer,
) -> Result<Vec<JsonObject>, GuiError> {
    let known_ids = items.keys().cloned().collect::<BTreeSet<_>>();
    let mut meta = BTreeMap::new();
    for &(category, rel_path) in TABLES.iter().chain(APPEARANCE_TABLES.iter()) {
        let path = assets_root.join(rel_path);
        if !path.exists() {
            continue;
        }
        for (item_id, row) in rows_from_table(&path)? {
            let Some(row) = row.as_object() else {
                continue;
            };
            if known_ids.contains(&item_id) {
                merge_item_meta(&mut meta, item_id, row_item_meta(row, category));
            }
        }
    }
    add_vehicle_module_meta(&mut meta, assets_root, &known_ids, canonicalizer)?;
    add_gacha_illustrate_meta(&mut meta, assets_root, &known_ids, canonicalizer)?;

    let mut rows = Vec::new();
    for (item_id, item_name) in items {
        let Some(asset_item) = meta.get(item_id) else {
            continue;
        };
        let Some(rarity) = asset_item.get("rarity").and_then(Value::as_u64) else {
            continue;
        };
        let mut row = JsonObject::new();
        row.insert("item_id".to_string(), Value::String(item_id.clone()));
        row.insert("item_name".to_string(), Value::String(item_name.clone()));
        row.insert("rarity".to_string(), json!(rarity));
        for key in ["category", "domain_type", "color"] {
            if let Some(value) = asset_item.get(key).and_then(Value::as_str) {
                if !value.is_empty() {
                    row.insert(key.to_string(), Value::String(value.to_string()));
                }
            }
        }
        if let Some(Value::Object(refs)) = asset_item
            .get("asset_refs")
            .filter(|value| value.as_object().is_some_and(|object| !object.is_empty()))
        {
            let mut refs = refs.clone();
            prune_item_asset_refs(&mut refs);
            if !refs.is_empty() {
                row.insert("asset_refs".to_string(), Value::Object(refs));
            }
        }
        rows.push(row);
    }
    Ok(rows)
}

fn normalized_items(items: &BTreeMap<String, String>, item_meta: &[JsonObject]) -> JsonObject {
    let meta_by_id = item_meta
        .iter()
        .filter_map(|item| {
            item.get("item_id")
                .and_then(Value::as_str)
                .map(|id| (id, item))
        })
        .collect::<BTreeMap<_, _>>();
    let mut normalized = JsonObject::new();
    for (item_id, item_name) in items {
        let Some(meta) = meta_by_id.get(item_id.as_str()) else {
            continue;
        };
        let Some(rarity) = meta.get("rarity").and_then(Value::as_u64) else {
            continue;
        };
        let mut entry = JsonObject::new();
        entry.insert("name".to_string(), Value::String(item_name.clone()));
        entry.insert("rarity".to_string(), json!(rarity));
        if let Some(category) = meta.get("category").and_then(Value::as_str) {
            entry.insert("category".to_string(), Value::String(category.to_string()));
        }
        for key in ["domain_type", "subtype", "color"] {
            if let Some(value) = meta.get(key).and_then(Value::as_str) {
                if !value.is_empty() {
                    entry.insert(key.to_string(), Value::String(value.to_string()));
                }
            }
        }
        if let Some(Value::Object(refs)) = meta
            .get("asset_refs")
            .filter(|value| value.as_object().is_some_and(|object| !object.is_empty()))
        {
            entry.insert("asset_refs".to_string(), Value::Object(refs.clone()));
        }
        normalized.insert(item_id.clone(), Value::Object(entry));
    }
    normalized
}

fn normalized_pools(
    pools: &BTreeMap<String, String>,
    pool_meta: &BTreeMap<String, JsonObject>,
) -> JsonObject {
    let mut normalized = JsonObject::new();
    for (pool_id, pool_name) in pools {
        let mut entry = JsonObject::new();
        entry.insert("name".to_string(), Value::String(pool_name.clone()));
        if let Some(meta) = pool_meta.get(pool_id) {
            entry.extend(meta.clone());
        }
        normalized.insert(pool_id.clone(), Value::Object(entry));
    }
    normalized
}
