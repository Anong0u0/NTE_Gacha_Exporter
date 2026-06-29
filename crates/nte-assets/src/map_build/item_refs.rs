fn add_item_ref(
    refs: &mut Vec<ItemRef>,
    ref_id: Option<&Value>,
    canonicalizer: &ItemCanonicalizer,
) {
    let Some(raw_id) = ref_id.and_then(value_to_text) else {
        return;
    };
    if raw_id.is_empty() || raw_id == "None" {
        return;
    }
    refs.push(ItemRef {
        id: canonicalizer.canonicalize(&raw_id),
        raw_id,
    });
}

fn iter_item_ids(value: &Value, item_ids: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if key == "ItemID" {
                    if child.is_array() {
                        iter_item_ids(child, item_ids);
                    } else if let Some(text) = value_to_text(child) {
                        if !text.is_empty() && text != "None" {
                            item_ids.push(text);
                        }
                    }
                    continue;
                }
                iter_item_ids(child, item_ids);
            }
        }
        Value::Array(values) => {
            for child in values {
                iter_item_ids(child, item_ids);
            }
        }
        _ => {}
    }
}

fn add_item_refs_from_value(
    refs: &mut Vec<ItemRef>,
    value: &Value,
    canonicalizer: &ItemCanonicalizer,
) {
    let mut item_ids = Vec::new();
    iter_item_ids(value, &mut item_ids);
    for item_id in item_ids {
        add_item_ref(refs, Some(&Value::String(item_id)), canonicalizer);
    }
}

fn dedupe_item_refs(refs: Vec<ItemRef>) -> Vec<ItemRef> {
    let mut seen = BTreeSet::new();
    let mut unique = Vec::new();
    for item_ref in refs {
        let key = (item_ref.id.clone(), item_ref.raw_id.clone());
        if seen.insert(key) {
            unique.push(item_ref);
        }
    }
    unique
}

fn matches_numbered_row(row_id: &str, prefix: &str) -> bool {
    row_id == prefix
        || row_id
            .strip_prefix(&format!("{prefix}_"))
            .is_some_and(|tail| tail.chars().all(|char| char.is_ascii_digit()))
}

fn add_sequence_refs(
    refs: &mut Vec<ItemRef>,
    sequence_rows: &JsonObject,
    sequence_id: &str,
    canonicalizer: &ItemCanonicalizer,
) {
    for (row_id, row) in sequence_rows {
        let Some(row) = row.as_object() else {
            continue;
        };
        if matches_numbered_row(row_id, sequence_id) {
            add_item_ref(refs, row.get("ItemID"), canonicalizer);
        }
    }
}

fn add_drop_group_refs(
    refs: &mut Vec<ItemRef>,
    drop_group_rows: &JsonObject,
    sequence_rows: &JsonObject,
    mut row_filter: impl FnMut(&str) -> bool,
    canonicalizer: &ItemCanonicalizer,
) {
    for (row_id, row) in drop_group_rows {
        let Some(row) = row.as_object() else {
            continue;
        };
        if !row_filter(row_id) {
            continue;
        }
        if let Some(sequence_id) = row.get("SequenceId").and_then(value_to_text) {
            add_sequence_refs(refs, sequence_rows, &sequence_id, canonicalizer);
        }
    }
}

fn add_gacha_illustrate_refs(
    refs: &mut Vec<ItemRef>,
    assets_root: &Path,
    canonicalizer: &ItemCanonicalizer,
) -> Result<(), GuiError> {
    let path = assets_root.join(GACHA_ILLUSTRATE_TABLE);
    if path.exists() {
        for row_id in rows_from_datatable(&path)?.keys() {
            add_item_ref(refs, Some(&Value::String(row_id.clone())), canonicalizer);
        }
    }
    Ok(())
}

fn lottery_table_paths(assets_root: &Path) -> Result<Vec<PathBuf>, GuiError> {
    let gacha_dir = assets_root.join("DataTable").join("Gacha");
    if !gacha_dir.exists() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    for entry in fs::read_dir(gacha_dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("DT_LotteryDataTable") && name.ends_with(".json") {
            paths.push(entry.path());
        }
    }
    paths.sort();
    Ok(paths)
}

fn add_lottery_table_refs(
    refs: &mut Vec<ItemRef>,
    assets_root: &Path,
    canonicalizer: &ItemCanonicalizer,
) -> Result<(), GuiError> {
    for path in lottery_table_paths(assets_root)? {
        for row in rows_from_datatable(&path)?.values() {
            add_item_refs_from_value(refs, row, canonicalizer);
        }
    }
    Ok(())
}

fn add_lottery_show_role_refs(
    refs: &mut Vec<ItemRef>,
    assets_root: &Path,
    canonicalizer: &ItemCanonicalizer,
) -> Result<(), GuiError> {
    for item_id in lottery_show_roles(assets_root, &Localization::new(), canonicalizer, None)?
        .into_values()
        .flatten()
    {
        add_item_ref(refs, Some(&Value::String(item_id)), canonicalizer);
    }
    Ok(())
}

fn add_fork_pool_refs(
    refs: &mut Vec<ItemRef>,
    assets_root: &Path,
    canonicalizer: &ItemCanonicalizer,
) -> Result<JsonObject, GuiError> {
    let path = assets_root.join(FORK_POOL_TABLE);
    if !path.exists() {
        return Ok(JsonObject::new());
    }
    let rows = rows_from_datatable(&path)?;
    for row in rows.values() {
        add_item_refs_from_value(refs, row, canonicalizer);
    }
    Ok(rows)
}

fn add_drop_table_refs(
    refs: &mut Vec<ItemRef>,
    assets_root: &Path,
    fork_pool_rows: &JsonObject,
    canonicalizer: &ItemCanonicalizer,
) -> Result<(), GuiError> {
    let drop_group_path = assets_root.join(DROP_GROUP_TABLE);
    let drop_sequence_path = assets_root.join(DROP_SEQUENCE_TABLE);
    if !drop_group_path.exists() || !drop_sequence_path.exists() {
        return Ok(());
    }
    let drop_group_rows = rows_from_datatable(&drop_group_path)?;
    let sequence_rows = rows_from_datatable(&drop_sequence_path)?;
    for row in fork_pool_rows.values() {
        let Some(row) = row.as_object() else {
            continue;
        };
        let Some(base_drop_id) = row.get("BaseDropID").and_then(value_to_text) else {
            continue;
        };
        add_drop_group_refs(
            refs,
            &drop_group_rows,
            &sequence_rows,
            |row_key| matches_numbered_row(row_key, &base_drop_id),
            canonicalizer,
        );
    }
    add_drop_group_refs(
        refs,
        &drop_group_rows,
        &sequence_rows,
        |row_key| row_key.starts_with("drop_Monopoly_"),
        canonicalizer,
    );
    Ok(())
}
