fn item_type_key(value: Option<&Value>) -> Option<String> {
    let text = value.and_then(value_to_text)?;
    text.rsplit("::").next().map(|value| value.to_lowercase())
}

fn item_type_prefixes(
    assets_root: &Path,
    localization: &Localization,
) -> Result<BTreeMap<String, String>, GuiError> {
    let path = assets_root.join(ITEM_TYPE_TABLE);
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let mut prefixes = BTreeMap::new();
    for (row_id, row) in rows_from_datatable(&path)? {
        let Some(row) = row.as_object() else {
            continue;
        };
        let prefix = clean_name(localized_text(row.get("TypeName"), localization));
        let item_type_key = row_id.rsplit("::").next().map(|value| value.to_lowercase());
        if let (Some(prefix), Some(item_type_key)) = (prefix, item_type_key) {
            prefixes.insert(item_type_key, prefix);
        }
    }
    Ok(prefixes)
}

fn item_type_prefix(
    item_type: Option<&Value>,
    item_type_prefixes: &BTreeMap<String, String>,
    fallback: String,
) -> String {
    item_type
        .and_then(|value| item_type_key(Some(value)))
        .and_then(|key| item_type_prefixes.get(&key).cloned())
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback)
}

fn default_prefix(kind: &str) -> &'static str {
    match kind {
        "appearance" | "fashion" => "Fashion",
        "capital" => "Currency",
        "character" => "Character",
        "glide" | "glider" => "Glider",
        "inventory" => "Item",
        "fork" => "Arc",
        "vehicle" => "Vehicle",
        "vehicle_module" => "Mod Parts",
        _ => "Item",
    }
}

fn localized_prefix(table_kind: &str, localization: &Localization) -> String {
    let key = match table_kind {
        "inventory" => Some(("ST_Common", "item_type_2")),
        "capital" => Some(("ST_Common", "item_type_4")),
        "fork" => Some(("ST_Common", "item_type_5")),
        "character" => Some(("ST_Common", "item_type_3")),
        "fashion" => Some(("ST_Common", "item_type_8")),
        "vehicle_module" => Some(("ST_Common", "item_type_10")),
        _ => None,
    };
    key.and_then(|(namespace, key)| localized_key(localization, namespace, key))
        .unwrap_or_else(|| default_prefix(table_kind).to_string())
}

fn appearance_prefix(row: &JsonObject, localization: &Localization) -> String {
    if row.get("AppearanceType").and_then(value_to_text).as_deref()
        == Some("EAppearanceType::Glide")
    {
        return localized_key(localization, "ST_Ui", "ui_appearance_02")
            .unwrap_or_else(|| default_prefix("glide").to_string());
    }
    localized_key(localization, "ST_Common", "item_type_8")
        .unwrap_or_else(|| default_prefix("appearance").to_string())
}

fn vehicle_prefix(localization: &Localization) -> String {
    localized_key(
        localization,
        "ST_Ui",
        "DT_CharacterAbilityCityGroupUI_zaiju",
    )
    .or_else(|| localized_key(localization, "ST_TeachAndIllustrater", "Vehicle_name"))
    .unwrap_or_else(|| default_prefix("vehicle").to_string())
}

fn vehicle_module_item_ids(row: &JsonObject) -> Vec<String> {
    row.get("FeatureActiveData")
        .and_then(Value::as_object)
        .and_then(|active_data| active_data.get("Requires"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|requirement| requirement.as_object())
        .filter_map(|requirement| requirement.get("ID"))
        .filter_map(value_to_text)
        .filter(|value| !value.is_empty())
        .collect()
}

fn add_item_priority(item_priorities: &mut BTreeMap<String, i32>, item_id: String, kind: &str) {
    let priority = match kind {
        "character" | "fork" | "appearance" | "vehicle_module" => 10,
        "vehicle" => 20,
        "capital" => 30,
        "inventory" => 40,
        "st_item_fallback" => 60,
        _ => 100,
    };
    match item_priorities.get(&item_id) {
        Some(existing) if *existing <= priority => {}
        _ => {
            item_priorities.insert(item_id, priority);
        }
    }
}

fn add_row_id_priorities(
    item_priorities: &mut BTreeMap<String, i32>,
    assets_root: &Path,
    tables: &[(&str, &str)],
) -> Result<(), GuiError> {
    for (kind, rel_path) in tables {
        let path = assets_root.join(rel_path);
        if !path.exists() {
            continue;
        }
        for row_id in rows_from_datatable(&path)?.keys() {
            add_item_priority(item_priorities, row_id.clone(), kind);
        }
    }
    Ok(())
}

fn add_vehicle_module_priorities(
    item_priorities: &mut BTreeMap<String, i32>,
    assets_root: &Path,
) -> Result<(), GuiError> {
    for &(kind, rel_path) in VEHICLE_MODULE_TABLES {
        let path = assets_root.join(rel_path);
        if !path.exists() {
            continue;
        }
        for row in rows_from_datatable(&path)?.values() {
            let Some(row) = row.as_object() else {
                continue;
            };
            for item_id in vehicle_module_item_ids(row) {
                add_item_priority(item_priorities, item_id, kind);
            }
        }
    }
    Ok(())
}

fn add_st_item_fallback_priorities(
    item_priorities: &mut BTreeMap<String, i32>,
    localization: &Localization,
) {
    let Some(item_text) = localization.get("ST_Item") else {
        return;
    };
    for key in item_text.keys() {
        if let Some(item_id) = key.strip_suffix("_name") {
            add_item_priority(item_priorities, item_id.to_string(), "st_item_fallback");
        }
    }
}

fn known_item_id_priorities(
    assets_root: &Path,
    localization: &Localization,
) -> Result<BTreeMap<String, i32>, GuiError> {
    let mut item_priorities = BTreeMap::new();
    add_row_id_priorities(&mut item_priorities, assets_root, TABLES)?;
    add_row_id_priorities(&mut item_priorities, assets_root, VEHICLE_TABLES)?;
    add_row_id_priorities(&mut item_priorities, assets_root, APPEARANCE_TABLES)?;
    add_vehicle_module_priorities(&mut item_priorities, assets_root)?;
    add_st_item_fallback_priorities(&mut item_priorities, localization);
    Ok(item_priorities)
}

fn st_item_signature(localization: &Localization, item_id: &str) -> Option<(String, String)> {
    let item_text = localization.get("ST_Item")?;
    let name = clean_name(item_text.get(&format!("{item_id}_name")).cloned())?;
    let desc = clean_name(item_text.get(&format!("{item_id}_desc")).cloned()).unwrap_or_default();
    Some((name.to_lowercase(), desc.to_lowercase()))
}

fn st_item_signature_aliases(
    item_priorities: &BTreeMap<String, i32>,
    localization: &Localization,
) -> BTreeMap<String, String> {
    let fallback_priority = 60;
    let mut by_signature: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    for (item_id, priority) in item_priorities {
        if *priority >= fallback_priority {
            continue;
        }
        if let Some(signature) = st_item_signature(localization, item_id) {
            by_signature
                .entry(signature)
                .or_default()
                .push(item_id.clone());
        }
    }

    let mut aliases = BTreeMap::new();
    for (item_id, priority) in item_priorities {
        if *priority < fallback_priority {
            continue;
        }
        let Some(signature) = st_item_signature(localization, item_id) else {
            continue;
        };
        let mut candidates = by_signature.get(&signature).cloned().unwrap_or_default();
        candidates.sort();
        candidates.dedup();
        if candidates.len() == 1 {
            aliases.insert(item_id.clone(), candidates[0].clone());
        }
    }
    aliases
}

impl ItemCanonicalizer {
    fn new(item_priorities: BTreeMap<String, i32>, localization: &Localization) -> Self {
        let mut by_folded: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for item_id in item_priorities.keys() {
            by_folded
                .entry(item_id.to_lowercase())
                .or_default()
                .push(item_id.clone());
        }
        Self {
            signature_aliases: st_item_signature_aliases(&item_priorities, localization),
            item_priorities,
            by_folded,
        }
    }

    fn canonicalize(&self, item_id: &str) -> String {
        let Some(candidates) = self.by_folded.get(&item_id.to_lowercase()) else {
            return self
                .signature_aliases
                .get(item_id)
                .cloned()
                .unwrap_or_else(|| item_id.to_string());
        };
        let best_priority = candidates
            .iter()
            .filter_map(|candidate| self.item_priorities.get(candidate))
            .min()
            .copied();
        let Some(best_priority) = best_priority else {
            return item_id.to_string();
        };
        let mut best_candidates = candidates
            .iter()
            .filter(|candidate| self.item_priorities.get(*candidate) == Some(&best_priority))
            .cloned()
            .collect::<Vec<_>>();
        best_candidates.sort();
        if best_candidates.len() == 1 {
            let best = &best_candidates[0];
            return self
                .signature_aliases
                .get(best)
                .cloned()
                .unwrap_or_else(|| best.clone());
        }
        if best_candidates.iter().any(|candidate| candidate == item_id) {
            return self
                .signature_aliases
                .get(item_id)
                .cloned()
                .unwrap_or_else(|| item_id.to_string());
        }
        self.signature_aliases
            .get(item_id)
            .cloned()
            .unwrap_or_else(|| item_id.to_string())
    }
}
