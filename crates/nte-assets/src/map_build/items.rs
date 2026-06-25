fn required_item_refs(
    assets_root: &Path,
    canonicalizer: &ItemCanonicalizer,
) -> Result<Vec<ItemRef>, GuiError> {
    let mut refs = Vec::new();
    add_gacha_illustrate_refs(&mut refs, assets_root, canonicalizer)?;
    add_lottery_table_refs(&mut refs, assets_root, canonicalizer)?;
    let fork_pool_rows = add_fork_pool_refs(&mut refs, assets_root, canonicalizer)?;
    add_drop_table_refs(&mut refs, assets_root, &fork_pool_rows, canonicalizer)?;
    Ok(dedupe_item_refs(refs))
}

fn inventory_prefix(
    row: &JsonObject,
    localization: &Localization,
    item_type_prefixes: &BTreeMap<String, String>,
) -> String {
    let fallback = localized_prefix("inventory", localization);
    item_type_prefix(row.get("ItemType"), item_type_prefixes, fallback)
}

fn item_build_context(
    assets_root: &Path,
    localization: Localization,
) -> Result<ItemBuildContext, GuiError> {
    let canonicalizer = ItemCanonicalizer::new(
        known_item_id_priorities(assets_root, &localization)?,
        &localization,
    );
    let item_refs = required_item_refs(assets_root, &canonicalizer)?;
    let required_item_ids = item_refs
        .iter()
        .map(|item_ref| item_ref.id.clone())
        .collect::<BTreeSet<_>>();
    let item_aliases = item_refs
        .iter()
        .filter(|item_ref| item_ref.raw_id != item_ref.id)
        .map(|item_ref| (item_ref.raw_id.clone(), item_ref.id.clone()))
        .collect::<BTreeMap<_, _>>();
    let item_type_prefixes = item_type_prefixes(assets_root, &localization)?;
    Ok(ItemBuildContext {
        localization,
        item_type_prefixes,
        canonicalizer,
        required_item_ids,
        item_aliases,
    })
}

fn add_required_item(
    items: &mut BTreeMap<String, String>,
    ctx: &ItemBuildContext,
    item_id: String,
    display: String,
) {
    if ctx.required_item_ids.contains(&item_id) {
        items.insert(item_id, display);
    }
}

fn add_table_items(
    items: &mut BTreeMap<String, String>,
    assets_root: &Path,
    ctx: &ItemBuildContext,
) -> Result<(), GuiError> {
    for &(kind, rel_path) in TABLES {
        let path = assets_root.join(rel_path);
        if !path.exists() {
            continue;
        }
        for (item_id, row) in rows_from_datatable(&path)? {
            let Some(row) = row.as_object() else {
                continue;
            };
            if !ctx.required_item_ids.contains(&item_id) {
                continue;
            }
            let Some(name) = clean_name(localized_text(row.get("ItemName"), &ctx.localization))
            else {
                continue;
            };
            let prefix = if kind == "inventory" {
                inventory_prefix(row, &ctx.localization, &ctx.item_type_prefixes)
            } else {
                localized_prefix(kind, &ctx.localization)
            };
            add_required_item(items, ctx, item_id, format!("{prefix}·{name}"));
        }
    }
    Ok(())
}

fn add_vehicle_items(
    items: &mut BTreeMap<String, String>,
    assets_root: &Path,
    ctx: &ItemBuildContext,
) -> Result<(), GuiError> {
    for &(_, rel_path) in VEHICLE_TABLES {
        let path = assets_root.join(rel_path);
        if !path.exists() {
            continue;
        }
        let prefix = vehicle_prefix(&ctx.localization);
        for (item_id, row) in rows_from_datatable(&path)? {
            let Some(row) = row.as_object() else {
                continue;
            };
            if !ctx.required_item_ids.contains(&item_id) {
                continue;
            }
            if let Some(name) = clean_name(localized_text(row.get("Name"), &ctx.localization)) {
                add_required_item(items, ctx, item_id, format!("{prefix}·{name}"));
            }
        }
    }
    Ok(())
}

fn add_appearance_items(
    items: &mut BTreeMap<String, String>,
    assets_root: &Path,
    ctx: &ItemBuildContext,
) -> Result<(), GuiError> {
    for &(_, rel_path) in APPEARANCE_TABLES {
        let path = assets_root.join(rel_path);
        if !path.exists() {
            continue;
        }
        for (item_id, row) in rows_from_datatable(&path)? {
            let Some(row) = row.as_object() else {
                continue;
            };
            if !ctx.required_item_ids.contains(&item_id) {
                continue;
            }
            if let Some(name) = clean_name(localized_text(row.get("Name"), &ctx.localization)) {
                let prefix = appearance_prefix(row, &ctx.localization);
                add_required_item(items, ctx, item_id, format!("{prefix}·{name}"));
            }
        }
    }
    Ok(())
}

fn add_vehicle_module_items(
    items: &mut BTreeMap<String, String>,
    assets_root: &Path,
    ctx: &ItemBuildContext,
) -> Result<(), GuiError> {
    for &(_, rel_path) in VEHICLE_MODULE_TABLES {
        let path = assets_root.join(rel_path);
        if !path.exists() {
            continue;
        }
        for row in rows_from_datatable(&path)?.values() {
            let Some(row) = row.as_object() else {
                continue;
            };
            let Some(name) = clean_name(localized_text(row.get("ModuleName"), &ctx.localization))
            else {
                continue;
            };
            let prefix = localized_prefix("vehicle_module", &ctx.localization);
            for item_id in vehicle_module_item_ids(row) {
                add_required_item(
                    items,
                    ctx,
                    ctx.canonicalizer.canonicalize(&item_id),
                    format!("{prefix}·{name}"),
                );
            }
        }
    }
    Ok(())
}

fn add_fallback_items(items: &mut BTreeMap<String, String>, ctx: &ItemBuildContext) {
    let fallback_prefix = localized_prefix("inventory", &ctx.localization);
    let missing = ctx
        .required_item_ids
        .difference(&items.keys().cloned().collect::<BTreeSet<_>>())
        .cloned()
        .collect::<Vec<_>>();
    for item_id in missing {
        if let Some(name) = clean_name(localized_key(
            &ctx.localization,
            "ST_Item",
            &format!("{item_id}_name"),
        )) {
            add_required_item(items, ctx, item_id, format!("{fallback_prefix}·{name}"));
        }
    }
}

fn build_item_data(
    assets_root: &Path,
    localization: Localization,
) -> Result<(BTreeMap<String, String>, ItemBuildContext), GuiError> {
    let ctx = item_build_context(assets_root, localization)?;
    let mut items = BTreeMap::new();
    add_table_items(&mut items, assets_root, &ctx)?;
    add_vehicle_items(&mut items, assets_root, &ctx)?;
    add_appearance_items(&mut items, assets_root, &ctx)?;
    add_vehicle_module_items(&mut items, assets_root, &ctx)?;
    add_fallback_items(&mut items, &ctx);
    Ok((items, ctx))
}

