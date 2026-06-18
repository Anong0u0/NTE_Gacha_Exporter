use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;
use serde_json::{Map, Value, json};

use nte_core::GuiError;

const MAP_SCHEMA_VERSION: u64 = 4;
const ASSET_FALLBACK_LOCALE: &str = "en";
const REMOVED_MAP_LOCALES: &[&str] = &["en-JM"];

const TABLES: &[(&str, &str)] = &[
    ("inventory", "DataTable/Inventory/DT_ItemConfig.json"),
    ("capital", "DataTable/Inventory/DT_CapitalItemConfig.json"),
    ("fork", "DataTable/Fork/DT_ForkItemData.json"),
    ("character", "DataTable/Character/DT_Character.json"),
];
const ITEM_TYPE_TABLE: &str = "DataTable/Inventory/DT_ItemType.json";
const APPEARANCE_TABLES: &[(&str, &str)] = &[(
    "appearance",
    "DataTable/Character/Appearance/DT_AppearanceData.json",
)];
const VEHICLE_TABLES: &[(&str, &str)] = &[("vehicle", "DataTable/Vehicle/DT_VehicleData.json")];
const VEHICLE_MODULE_TABLES: &[(&str, &str)] = &[(
    "vehicle_module",
    "DataTable/Vehicle/DT_vehicleModuleData.json",
)];
const POOL_TABLES: &[(&str, &str)] = &[("fork_pool", "DataTable/Fork/DT_ForkLotteryPoolData.json")];
const GACHA_ILLUSTRATE_TABLE: &str = "DataTable/Gacha/GachaIllustrate.json";
const DROP_GROUP_TABLE: &str = "DataTable/Drop/Client/ClientDropGroupDataTable.json";
const DROP_SEQUENCE_TABLE: &str = "DataTable/Drop/DropSequenceDataTable.json";
const MONOPOLY_TITLE_NAMESPACE: &str = "ST_Ui";
const MONOPOLY_TITLE_PREFIX: &str = "Lottery_Kachimingcheng_";
const MONOPOLY_DESCRIPTION_KEYS: &[&str] = &[
    "LotteryDes_Jishishuoming_{tail}Des",
    "LotteryDes_JIshishuoming_{tail}Des",
];
const STANDARD_MONOPOLY_TITLE_TAIL: &str = "changzhu";
const MONOPOLY_LIMITED_RULE_TEXT_KEY: &str = "LotteryDes_XiandingJishiguize_Des";
const MONOPOLY_STANDARD_RULE_TEXT_KEY: &str = "LotteryDes_Changzhujishiguize_Des";
const MONOPOLY_LOTTERY_TABLE: &str = "DataTable/Gacha/DT_LotteryDataTable_Nanali.json";
const FORK_POOL_TABLE: &str = "DataTable/Fork/DT_ForkLotteryPoolData.json";

#[derive(Debug, Clone)]
pub struct AssetMapBuild {
    pub locale: String,
    pub map: Value,
    pub item_count: usize,
    pub pool_count: usize,
    pub label_count: usize,
}

#[derive(Debug, Clone)]
struct CuratedLimitedBanner {
    tail: &'static str,
    banner_id: &'static str,
    end_at_tz8: &'static str,
    rate_up_5: &'static [&'static str],
    version: Option<&'static str>,
    phase: Option<&'static str>,
}

const CURATED_LIMITED_BANNERS: &[CuratedLimitedBanner] = &[
    CuratedLimitedBanner {
        tail: "Nanali",
        banner_id: "monopoly_limited_Nanali",
        end_at_tz8: "2026-05-13 05:59:00",
        rate_up_5: &["1010"],
        version: None,
        phase: Some("limited_2026_05_13"),
    },
    CuratedLimitedBanner {
        tail: "Xun",
        banner_id: "monopoly_limited_Xun",
        end_at_tz8: "2026-06-03 05:59:00",
        rate_up_5: &["1052"],
        version: None,
        phase: Some("limited_2026_06_03"),
    },
    CuratedLimitedBanner {
        tail: "AnHunQu",
        banner_id: "monopoly_limited_AnHunQu",
        end_at_tz8: "2026-06-24 05:59:00",
        rate_up_5: &["1004"],
        version: None,
        phase: Some("limited_2026_06_24"),
    },
    CuratedLimitedBanner {
        tail: "Kaesi",
        banner_id: "monopoly_limited_Kaesi",
        end_at_tz8: "2026-07-08 05:59:00",
        rate_up_5: &["1020"],
        version: None,
        phase: Some("limited_2026_07_08"),
    },
];

#[derive(Debug, Clone)]
struct ItemRef {
    id: String,
    raw_id: String,
}

#[derive(Debug, Clone)]
struct ItemCanonicalizer {
    item_priorities: BTreeMap<String, i32>,
    by_folded: BTreeMap<String, Vec<String>>,
    signature_aliases: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
struct ItemBuildContext {
    localization: Localization,
    item_type_prefixes: BTreeMap<String, String>,
    canonicalizer: ItemCanonicalizer,
    required_item_ids: BTreeSet<String>,
    item_aliases: BTreeMap<String, String>,
}

type Localization = BTreeMap<String, BTreeMap<String, String>>;
type JsonObject = Map<String, Value>;
type PoolBuildData = (BTreeMap<String, String>, BTreeMap<String, JsonObject>);

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

fn load_json(path: &Path) -> Result<Value, GuiError> {
    let bytes = fs::read(path)?;
    let text = String::from_utf8_lossy(&bytes);
    Ok(serde_json::from_str(&text)?)
}

fn load_localization(assets_root: &Path, locale: &str) -> Result<Localization, GuiError> {
    let mut localization = Localization::new();
    let mut fallbacks = vec![locale.to_string(), ASSET_FALLBACK_LOCALE.to_string()];
    fallbacks.dedup();
    fallbacks.reverse();
    for fallback in fallbacks {
        let path = assets_root
            .join("Localization")
            .join(fallback)
            .join("game.json");
        if !path.exists() {
            continue;
        }
        let Value::Object(loaded) = load_json(&path)? else {
            continue;
        };
        for (namespace, values) in loaded {
            if let Value::Object(values) = values {
                let target = localization.entry(namespace).or_default();
                for (key, value) in values {
                    if let Some(text) = value_to_text(&value) {
                        target.insert(key, text);
                    }
                }
            }
        }
    }
    Ok(localization)
}

fn rows_from_datatable(path: &Path) -> Result<JsonObject, GuiError> {
    match load_json(path)? {
        Value::Array(values) => values
            .first()
            .and_then(Value::as_object)
            .and_then(|object| object.get("Rows"))
            .and_then(Value::as_object)
            .cloned()
            .ok_or_else(|| invalid(format!("cannot locate Rows in {}", path.display()))),
        Value::Object(mut object) => {
            if let Some(Value::Object(rows)) = object.remove("Rows") {
                Ok(rows)
            } else {
                Ok(object)
            }
        }
        _ => Err(invalid(format!("cannot locate Rows in {}", path.display()))),
    }
}

fn rows_from_table(path: &Path) -> Result<JsonObject, GuiError> {
    rows_from_datatable(path).or_else(|_| Ok(JsonObject::new()))
}

fn namespace_from_table_id(table_id: Option<&Value>) -> Option<String> {
    let text = table_id.and_then(value_to_text)?;
    let tail = text.rsplit('/').next().unwrap_or(&text);
    if let Some((_, namespace)) = tail.rsplit_once('.') {
        Some(namespace.to_string()).filter(|value| !value.is_empty())
    } else {
        Some(tail.to_string()).filter(|value| !value.is_empty())
    }
}

fn localized_text(value: Option<&Value>, localization: &Localization) -> Option<String> {
    match value? {
        Value::String(text) => Some(text.clone()),
        Value::Object(text_ref) => {
            if let Some(text) = text_ref
                .get("CultureInvariantString")
                .and_then(value_to_text)
                .filter(|value| !value.is_empty())
            {
                return Some(text);
            }
            let key = text_ref.get("Key").and_then(value_to_text);
            let namespace = namespace_from_table_id(text_ref.get("TableId"));
            if let (Some(key), Some(namespace)) = (&key, namespace) {
                if let Some(text) = localized_key(localization, &namespace, key) {
                    return Some(text);
                }
            }
            if let Some(key) = key {
                if let Some(text) = localized_key(localization, "", &key)
                    .or_else(|| unique_localized_key(localization, &key))
                {
                    return Some(text);
                }
            }
            text_ref_fallback(text_ref)
        }
        _ => None,
    }
}

fn text_ref_fallback(text_ref: &JsonObject) -> Option<String> {
    ["SourceString", "LocalizedString"]
        .into_iter()
        .find_map(|field| text_ref.get(field).and_then(value_to_text))
        .filter(|value| !value.is_empty())
}

fn text_ref_key(text_ref: Option<&Value>) -> Option<String> {
    text_ref
        .and_then(Value::as_object)
        .and_then(|object| object.get("Key"))
        .and_then(value_to_text)
        .filter(|value| !value.is_empty())
}

fn localized_key(localization: &Localization, namespace: &str, key: &str) -> Option<String> {
    localization
        .get(namespace)
        .and_then(|values| values.get(key))
        .cloned()
}

fn unique_localized_key(localization: &Localization, key: &str) -> Option<String> {
    let hits = localization
        .values()
        .filter_map(|values| values.get(key).cloned())
        .collect::<Vec<_>>();
    let unique = hits.iter().collect::<BTreeSet<_>>();
    (unique.len() == 1).then(|| hits[0].clone())
}

fn clean_name(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

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
        "appearance" => "Appearance",
        "capital" => "Currency",
        "character" => "Character",
        "glide" => "Glider",
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

fn pool_label_key(pool_id: &str) -> Option<(&'static str, &'static str)> {
    match pool_id {
        "CardPool_NewRole" => Some(("ST_Ui", "BPUI_LotteryDiceRecord_biaozhunqipan")),
        "CardPool_Character" => Some(("ST_Ui", "BPUI_LotteryDiceRecord_xiandingqipan")),
        _ => None,
    }
}

fn add_pool(pools: &mut BTreeMap<String, String>, pool_id: String, name: String, overwrite: bool) {
    if overwrite || !pools.contains_key(&pool_id) {
        pools.insert(pool_id, name);
    }
}

fn source_evidence(confidence: &str, tables: &[String], notes: &[&str]) -> Value {
    let mut object = JsonObject::new();
    object.insert(
        "confidence".to_string(),
        Value::String(confidence.to_string()),
    );
    object.insert(
        "tables".to_string(),
        Value::Array(tables.iter().cloned().map(Value::String).collect()),
    );
    if !notes.is_empty() {
        object.insert(
            "notes".to_string(),
            Value::Array(
                notes
                    .iter()
                    .map(|note| Value::String((*note).to_string()))
                    .collect(),
            ),
        );
    }
    Value::Object(object)
}

fn asset_path(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_object)
        .and_then(|object| object.get("AssetPathName"))
        .and_then(Value::as_str)
        .filter(|path| path.starts_with("/Game/"))
        .map(ToString::to_string)
}

fn pool_asset_refs(row: &JsonObject) -> JsonObject {
    let mut refs = JsonObject::new();
    if let Some(background) = asset_path(row.get("Bg")) {
        refs.insert("background".to_string(), Value::String(background));
    }
    if let Some(icon) = asset_path(row.get("Icon")) {
        refs.insert("icon".to_string(), Value::String(icon));
    }
    refs
}

fn fork_pickup_item_ids(
    pool_id: &str,
    row: &JsonObject,
    canonicalizer: &ItemCanonicalizer,
) -> Result<Vec<String>, GuiError> {
    if let Some(raw_ids) = row.get("UpList").and_then(Value::as_array) {
        let mut pickup_item_ids = raw_ids
            .iter()
            .filter_map(value_to_text)
            .filter(|item_id| !item_id.is_empty() && item_id != "None")
            .map(|item_id| canonicalizer.canonicalize(&item_id))
            .collect::<Vec<_>>();
        dedupe_strings(&mut pickup_item_ids);
        if !pickup_item_ids.is_empty() {
            return Ok(pickup_item_ids);
        }
    }

    if let Some(show_rewards) = row.get("ShowRewards").and_then(Value::as_array) {
        let mut pickup_item_ids = show_rewards
            .iter()
            .filter_map(Value::as_object)
            .filter(|reward| reward.get("IsUp").and_then(Value::as_bool) == Some(true))
            .filter_map(|reward| reward.get("ItemID"))
            .filter_map(value_to_text)
            .filter(|item_id| !item_id.is_empty())
            .map(|item_id| canonicalizer.canonicalize(&item_id))
            .collect::<Vec<_>>();
        dedupe_strings(&mut pickup_item_ids);
        if !pickup_item_ids.is_empty() {
            return Ok(pickup_item_ids);
        }
    }

    Err(invalid(format!(
        "fork pool missing pickup item ids from UpList or ShowRewards: {pool_id}"
    )))
}

fn fork_pool_meta(
    pool_id: &str,
    row: &JsonObject,
    localization: &Localization,
    canonicalizer: &ItemCanonicalizer,
) -> Result<JsonObject, GuiError> {
    let mut meta = JsonObject::new();
    if let Some(group_label) = clean_name(localized_key(
        localization,
        "ST_Ui",
        "UW_LotteryBase_BP_Hupanyanmu",
    )) {
        meta.insert("group_label".to_string(), Value::String(group_label));
    }
    if let Some(title) = clean_name(localized_text(row.get("ShowText1"), localization)) {
        meta.insert("title".to_string(), Value::String(title));
    }
    meta.insert(
        "pickup_item_ids".to_string(),
        Value::Array(
            fork_pickup_item_ids(pool_id, row, canonicalizer)?
                .into_iter()
                .map(Value::String)
                .collect(),
        ),
    );
    let refs = pool_asset_refs(row);
    if !refs.is_empty() {
        meta.insert("asset_refs".to_string(), Value::Object(refs));
    }
    Ok(meta)
}

fn strip_rich_text(value: &str) -> String {
    Regex::new(r"</?[^>]+>")
        .expect("valid regex")
        .replace_all(value, "")
        .trim()
        .to_string()
}

fn clean_pool_title(value: Option<String>) -> Option<String> {
    let text = clean_name(value)?;
    for (left, right) in [
        ("「", "」"),
        ("『", "』"),
        ("“", "”"),
        ("\"", "\""),
        ("'", "'"),
    ] {
        if text.starts_with(left) && text.ends_with(right) && text.len() >= left.len() + right.len()
        {
            return clean_name(Some(text[left.len()..text.len() - right.len()].to_string()));
        }
    }
    Some(text)
}

fn description_pool_title(value: Option<String>) -> Option<String> {
    let value = value?;
    let rich = Regex::new(r"<[^/>][^>]*>([^<]+)</>").expect("valid regex");
    if let Some(captures) = rich.captures(&value) {
        return clean_pool_title(captures.get(1).map(|match_| match_.as_str().to_string()));
    }
    let clean_text = strip_rich_text(&value);
    let quoted = Regex::new(r"「([^」]+)」\s*(?:屬於|属于|は|은|는)").expect("valid regex");
    if let Some(captures) = quoted.captures(&clean_text) {
        return clean_pool_title(captures.get(1).map(|match_| match_.as_str().to_string()));
    }
    let first_line = clean_text.lines().next().unwrap_or(&clean_text);
    let roman = Regex::new(
        r"(?i)^\s*\d+[.)．]?\s*(?P<title>.+?)\s+(?:is|ist|est)\s+(?:a|an|ein|eine|un|une)\b",
    )
    .expect("valid regex");
    roman
        .captures(first_line)
        .and_then(|captures| captures.name("title"))
        .and_then(|match_| clean_pool_title(Some(match_.as_str().to_string())))
}

fn title_suffix_candidates(tail: &str) -> Vec<String> {
    let folded_tail = tail.to_lowercase();
    let parts = Regex::new(r"[A-Z]?[a-z0-9]+|[A-Z]+")
        .expect("valid regex")
        .find_iter(tail)
        .map(|match_| match_.as_str().to_string())
        .collect::<Vec<_>>();
    let mut candidates = vec![folded_tail];
    if !parts.is_empty() {
        candidates.push(
            parts
                .iter()
                .filter_map(|part| part.chars().next())
                .collect::<String>()
                .to_lowercase(),
        );
    }
    dedupe_strings(&mut candidates);
    candidates.retain(|candidate| !candidate.is_empty());
    candidates
}

fn localized_monopoly_pool_title(localization: &Localization, tail: &str) -> Option<String> {
    for template in MONOPOLY_DESCRIPTION_KEYS {
        let key = template.replace("{tail}", tail);
        let title =
            description_pool_title(localized_key(localization, MONOPOLY_TITLE_NAMESPACE, &key));
        if title.is_some() {
            return title;
        }
    }
    for suffix in title_suffix_candidates(tail) {
        let key = format!("{MONOPOLY_TITLE_PREFIX}{suffix}");
        if let Some(title) = clean_name(localized_key(localization, MONOPOLY_TITLE_NAMESPACE, &key))
        {
            return Some(title);
        }
    }
    None
}

fn monopoly_pool_meta(localization: &Localization, pool_id: &str) -> JsonObject {
    let mut meta = JsonObject::new();
    if let Some((namespace, key)) = pool_label_key(pool_id) {
        if let Some(group_label) = clean_name(localized_key(localization, namespace, key)) {
            meta.insert("group_label".to_string(), Value::String(group_label));
        }
    }
    if pool_id == "CardPool_NewRole" {
        if let Some(title) =
            localized_monopoly_pool_title(localization, STANDARD_MONOPOLY_TITLE_TAIL)
        {
            meta.insert("title".to_string(), Value::String(title));
        }
        return meta;
    }

    let mut title_windows = Vec::new();
    for banner in CURATED_LIMITED_BANNERS {
        if let Some(title) = localized_monopoly_pool_title(localization, banner.tail) {
            title_windows.push(json!({"end_at_tz8": banner.end_at_tz8, "title": title}));
        }
    }
    if !title_windows.is_empty() {
        meta.insert("title_windows".to_string(), Value::Array(title_windows));
    }
    meta
}

fn add_monopoly_pools(
    pools: &mut BTreeMap<String, String>,
    pool_meta: &mut BTreeMap<String, JsonObject>,
    localization: &Localization,
) {
    for pool_id in ["CardPool_NewRole", "CardPool_Character"] {
        if let Some((namespace, key)) = pool_label_key(pool_id) {
            if let Some(name) = clean_name(localized_key(localization, namespace, key)) {
                add_pool(pools, pool_id.to_string(), name, true);
            }
        }
        let meta = monopoly_pool_meta(localization, pool_id);
        if !meta.is_empty() {
            pool_meta.insert(pool_id.to_string(), meta);
        }
    }
}

fn add_fork_pools(
    pools: &mut BTreeMap<String, String>,
    pool_meta: &mut BTreeMap<String, JsonObject>,
    assets_root: &Path,
    localization: &Localization,
    canonicalizer: &ItemCanonicalizer,
) -> Result<(), GuiError> {
    for &(_, rel_path) in POOL_TABLES {
        let path = assets_root.join(rel_path);
        if !path.exists() {
            continue;
        }
        for (pool_id, row) in rows_from_datatable(&path)? {
            let Some(row) = row.as_object() else {
                continue;
            };
            if let Some(name) = clean_name(localized_text(row.get("Name"), localization)) {
                add_pool(pools, pool_id.clone(), name, true);
            }
            if pool_id.starts_with("ForkLottery_") {
                let meta = fork_pool_meta(&pool_id, row, localization, canonicalizer)?;
                if !meta.is_empty() {
                    pool_meta.insert(pool_id, meta);
                }
            }
        }
    }
    Ok(())
}

fn build_pools(
    assets_root: &Path,
    localization: &Localization,
    canonicalizer: &ItemCanonicalizer,
) -> Result<PoolBuildData, GuiError> {
    let mut pools = BTreeMap::new();
    let mut pool_meta = BTreeMap::new();
    add_monopoly_pools(&mut pools, &mut pool_meta, localization);
    add_fork_pools(
        &mut pools,
        &mut pool_meta,
        assets_root,
        localization,
        canonicalizer,
    )?;
    Ok((pools, pool_meta))
}

fn asset_refs_from_fields(row: &JsonObject, fields: &[(&str, &str)]) -> JsonObject {
    let mut refs = JsonObject::new();
    for (source_field, target_field) in fields {
        if let Some(asset_path) = asset_path(row.get(*source_field)) {
            refs.insert(
                (*target_field).to_string(),
                Value::String(asset_path.clone()),
            );
            if *target_field == "icon" {
                refs.insert("head_icon".to_string(), Value::String(asset_path));
            }
        }
    }
    refs
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

fn add_lottery_table_meta(
    meta: &mut BTreeMap<String, JsonObject>,
    assets_root: &Path,
    known_ids: &BTreeSet<String>,
    canonicalizer: &ItemCanonicalizer,
) -> Result<(), GuiError> {
    for path in lottery_table_paths(assets_root)? {
        for row in rows_from_table(&path)?.values() {
            let Some(row) = row.as_object() else {
                continue;
            };
            add_lottery_items(meta, row.get("SSRItems"), known_ids, canonicalizer, 5);
            add_lottery_items(meta, row.get("SRItems"), known_ids, canonicalizer, 4);
            add_lottery_items(meta, row.get("RItems"), known_ids, canonicalizer, 3);
        }
    }
    Ok(())
}

fn add_lottery_items(
    meta: &mut BTreeMap<String, JsonObject>,
    values: Option<&Value>,
    known_ids: &BTreeSet<String>,
    canonicalizer: &ItemCanonicalizer,
    rarity: u64,
) {
    let Some(values) = values.and_then(Value::as_array) else {
        return;
    };
    for value in values {
        let Some(value) = value.as_object() else {
            continue;
        };
        let item_id = value
            .get("ItemID")
            .and_then(value_to_text)
            .map(|item_id| canonicalizer.canonicalize(&item_id));
        if let Some(item_id) = item_id.filter(|item_id| known_ids.contains(item_id)) {
            merge_item_meta(
                &mut *meta,
                item_id,
                map_from_pairs([("rarity", json!(rarity))]),
            );
        }
    }
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
    add_lottery_table_meta(&mut meta, assets_root, &known_ids, canonicalizer)?;
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
            row.insert("asset_refs".to_string(), Value::Object(refs.clone()));
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

fn lottery_item_ids(
    assets_root: &Path,
    key: &str,
    canonicalizer: &ItemCanonicalizer,
    known_item_ids: &BTreeSet<String>,
) -> Result<Vec<String>, GuiError> {
    let path = assets_root.join(MONOPOLY_LOTTERY_TABLE);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut item_ids = Vec::new();
    for row in rows_from_datatable(&path)?.values() {
        let Some(row) = row.as_object() else {
            continue;
        };
        let Some(values) = row.get(key).and_then(Value::as_array) else {
            continue;
        };
        for value in values {
            let Some(value) = value.as_object() else {
                continue;
            };
            let Some(raw_item_id) = value.get("ItemID").and_then(value_to_text) else {
                continue;
            };
            if raw_item_id.is_empty() || raw_item_id == "None" {
                continue;
            }
            let item_id = canonicalizer.canonicalize(&raw_item_id);
            if known_item_ids.contains(&item_id) {
                item_ids.push(item_id);
            }
        }
    }
    dedupe_strings(&mut item_ids);
    Ok(item_ids)
}

fn item_ref_list(
    item_ids: &[&str],
    canonicalizer: &ItemCanonicalizer,
    known_item_ids: &BTreeSet<String>,
) -> Vec<String> {
    let mut refs = item_ids
        .iter()
        .map(|item_id| canonicalizer.canonicalize(item_id))
        .filter(|item_id| known_item_ids.contains(item_id))
        .collect::<Vec<_>>();
    dedupe_strings(&mut refs);
    refs
}

fn item_asset_ref(items: &JsonObject, item_id: &str, key: &str) -> Option<String> {
    items
        .get(item_id)
        .and_then(Value::as_object)
        .and_then(|item| item.get("asset_refs"))
        .and_then(Value::as_object)
        .and_then(|refs| refs.get(key))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn featured_portraits(items: &JsonObject, item_ids: &[String]) -> Vec<String> {
    item_ids
        .iter()
        .filter_map(|item_id| item_asset_ref(items, item_id, "portrait"))
        .collect()
}

fn monopoly_rule_text_refs(limited: bool) -> JsonObject {
    let key = if limited {
        MONOPOLY_LIMITED_RULE_TEXT_KEY
    } else {
        MONOPOLY_STANDARD_RULE_TEXT_KEY
    };
    map_from_pairs([("rule_desc_1", Value::String(key.to_string()))])
}

fn fork_pool_rows(assets_root: &Path) -> Result<JsonObject, GuiError> {
    let path = assets_root.join(FORK_POOL_TABLE);
    if path.exists() {
        rows_from_datatable(&path)
    } else {
        Ok(JsonObject::new())
    }
}

fn fork_hard_pity_5(fork_rows: &JsonObject) -> Option<u64> {
    let mut values = BTreeSet::new();
    for (pool_id, row) in fork_rows {
        let Some(row) = row.as_object() else {
            continue;
        };
        if !pool_id.starts_with("ForkLottery_") {
            continue;
        }
        if let Some(value) = row.get("UpGuaranteeCnt").and_then(Value::as_u64) {
            values.insert(value);
        }
    }
    (values.len() == 1).then(|| *values.iter().next().expect("one value"))
}

fn fork_gold_sequence_ids(drop_group_rows: &JsonObject, base_drop_id: &str) -> Vec<String> {
    let mut sequence_ids = Vec::new();
    for (row_id, row) in drop_group_rows {
        let Some(row) = row.as_object() else {
            continue;
        };
        if !matches_numbered_row(row_id, base_drop_id) {
            continue;
        }
        if let Some(sequence_id) = row
            .get("SequenceId")
            .and_then(Value::as_str)
            .filter(|value| value.contains("_gold"))
        {
            sequence_ids.push(sequence_id.to_string());
        }
    }
    dedupe_strings(&mut sequence_ids);
    sequence_ids
}

fn weighted_pickup_rate(
    sequence_rows: &JsonObject,
    sequence_id: &str,
    pickup_item_ids: &BTreeSet<String>,
    canonicalizer: &ItemCanonicalizer,
) -> Option<u64> {
    let mut total_weight = 0.0_f64;
    let mut pickup_weight = 0.0_f64;
    for (row_id, row) in sequence_rows {
        let Some(row) = row.as_object() else {
            continue;
        };
        if !matches_numbered_row(row_id, sequence_id) {
            continue;
        }
        let Some(weight) = row.get("Weight").and_then(Value::as_f64) else {
            continue;
        };
        let item_id = canonicalizer.canonicalize(
            &row.get("ItemID")
                .and_then(value_to_text)
                .unwrap_or_default(),
        );
        total_weight += weight;
        if pickup_item_ids.contains(&item_id) {
            pickup_weight += weight;
        }
    }
    if total_weight <= 0.0 || pickup_weight <= 0.0 {
        return None;
    }
    Some((pickup_weight * 100.0 / total_weight).round() as u64)
}

fn fork_pickup_win_rate_5(
    assets_root: &Path,
    fork_rows: &JsonObject,
    canonicalizer: &ItemCanonicalizer,
) -> Result<Option<u64>, GuiError> {
    let drop_group_path = assets_root.join(DROP_GROUP_TABLE);
    let drop_sequence_path = assets_root.join(DROP_SEQUENCE_TABLE);
    if !drop_group_path.exists() || !drop_sequence_path.exists() {
        return Ok(None);
    }
    let drop_group_rows = rows_from_datatable(&drop_group_path)?;
    let sequence_rows = rows_from_datatable(&drop_sequence_path)?;
    let mut rates = Vec::new();
    for (pool_id, row) in fork_rows {
        let Some(row) = row.as_object() else {
            continue;
        };
        if !pool_id.starts_with("ForkLottery_") {
            continue;
        }
        let Some(base_drop_id) = row.get("BaseDropID").and_then(value_to_text) else {
            continue;
        };
        let pickup_item_ids = fork_pickup_item_ids(pool_id, row, canonicalizer)?
            .into_iter()
            .collect::<BTreeSet<_>>();
        for sequence_id in fork_gold_sequence_ids(&drop_group_rows, &base_drop_id) {
            if let Some(rate) = weighted_pickup_rate(
                &sequence_rows,
                &sequence_id,
                &pickup_item_ids,
                canonicalizer,
            ) {
                rates.push(rate);
            }
        }
    }
    let unique = rates.into_iter().collect::<BTreeSet<_>>();
    Ok((unique.len() == 1).then(|| *unique.iter().next().expect("one rate")))
}

fn fork_rule_text_refs(fork_rows: &JsonObject) -> JsonObject {
    for (pool_id, row) in fork_rows {
        let Some(row) = row.as_object() else {
            continue;
        };
        if !pool_id.starts_with("ForkLottery_") {
            continue;
        }
        let mut refs = JsonObject::new();
        for (source_key, target_key) in [
            ("RuleDesc1", "rule_desc_1"),
            ("RuleDesc2", "rule_desc_2"),
            ("ProbDesc", "probability_desc"),
        ] {
            if let Some(key) = text_ref_key(row.get(source_key)) {
                refs.insert(target_key.to_string(), Value::String(key));
            }
        }
        if !refs.is_empty() {
            return refs;
        }
    }
    JsonObject::new()
}

fn build_gacha_rules(
    assets_root: &Path,
    locale: &str,
    canonicalizer: &ItemCanonicalizer,
) -> Result<JsonObject, GuiError> {
    let mut rules = JsonObject::new();
    rules.insert(
        "monopoly_limited".to_string(),
        json!({
            "rule_id": "monopoly_limited",
            "pool_kind": "monopoly_limited",
            "hard_pity_5": 90,
            "has_guarantee_5": false,
            "guarantee_scope": "unknown",
            "carry_scope": "pool_kind",
            "rule_text_refs": monopoly_rule_text_refs(true),
            "source": source_evidence(
                "curated",
                &[format!("Localization/{locale}/game.json")],
                &["Numeric rule follows current desktop hard-pity behavior; rate-up precision is unknown."],
            ),
        }),
    );
    rules.insert(
        "monopoly_standard".to_string(),
        json!({
            "rule_id": "monopoly_standard",
            "pool_kind": "monopoly_standard",
            "hard_pity_5": 90,
            "has_guarantee_5": false,
            "guarantee_scope": "unknown",
            "carry_scope": "pool_kind",
            "rule_text_refs": monopoly_rule_text_refs(false),
            "source": source_evidence(
                "curated",
                &[format!("Localization/{locale}/game.json")],
                &["Numeric rule follows current desktop hard-pity behavior; standard rate-up is not modeled."],
            ),
        }),
    );

    let fork_rows = fork_pool_rows(assets_root)?;
    if fork_rows
        .iter()
        .any(|(pool_id, row)| pool_id.starts_with("ForkLottery_") && row.is_object())
    {
        let hard_pity_5 = fork_hard_pity_5(&fork_rows);
        let pickup_win_rate_5 = fork_pickup_win_rate_5(assets_root, &fork_rows, canonicalizer)?;
        let source_is_exact = hard_pity_5.is_some() && pickup_win_rate_5.is_some();
        let notes = if source_is_exact {
            vec![
                "Fork S-class pickup rate is backed by gold drop sequence weights in the asset dump.",
            ]
        } else {
            vec![
                "Fallback numeric rule follows current desktop behavior when structured values are absent.",
            ]
        };
        rules.insert(
            "fork_lottery_s".to_string(),
            json!({
                "rule_id": "fork_lottery_s",
                "pool_kind": "fork_lottery",
                "hard_pity_5": hard_pity_5.unwrap_or(80),
                "pickup_win_rate_5": pickup_win_rate_5.unwrap_or(25),
                "has_guarantee_5": true,
                "guarantee_scope": "pool_kind",
                "carry_scope": "pool_kind",
                "rule_text_refs": fork_rule_text_refs(&fork_rows),
                "source": source_evidence(
                    if source_is_exact { "exact" } else { "curated" },
                    &[
                        FORK_POOL_TABLE.to_string(),
                        DROP_GROUP_TABLE.to_string(),
                        DROP_SEQUENCE_TABLE.to_string(),
                    ],
                    &notes,
                ),
            }),
        );
    }
    Ok(rules)
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
            "curated",
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
    for banner in CURATED_LIMITED_BANNERS {
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
                "curated",
                &[
                    MONOPOLY_LOTTERY_TABLE.to_string(),
                    format!("Localization/{locale}/game.json"),
                ],
                &[
                    "Schedule and rate-up are curated because no structured limited banner table was found.",
                    "Version/phase metadata is curated when present.",
                ],
            ),
        );
        if let Some(version) = banner.version {
            entry.insert("version".to_string(), Value::String(version.to_string()));
        }
        if let Some(phase) = banner.phase {
            entry.insert("phase".to_string(), Value::String(phase.to_string()));
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
            source_evidence("exact", &[FORK_POOL_TABLE.to_string()], &[]),
        );
        let refs = pool_asset_refs(row);
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

fn custom_csv_header(field: &str, locale: &str) -> Option<&'static str> {
    match (field, locale) {
        ("pool_name", "de" | "en") => Some("Pool"),
        ("pool_name", "es") => Some("Banner"),
        ("pool_name", "fr") => Some("Bannière"),
        ("pool_name", "ja") => Some("ガチャ"),
        ("pool_name", "ko") => Some("뽑기"),
        ("pool_name", "ru") => Some("Баннер"),
        ("pool_name", "zh-CN" | "zh-Hans" | "zh-Hant") => Some("卡池"),
        _ => None,
    }
}

fn csv_header_keys(field: &str) -> &'static [(&'static str, &'static str)] {
    match field {
        "time" => &[("ST_Ui", "BPUI_GashaponRecord_time")],
        "pool_group" => &[("ST_Ui", "BPUI_LotteryDiceRecord_qipanleixing")],
        "item_name" => &[
            ("ST_Ui", "BPUI_LotteryDiceRecord_daojumingcheng"),
            ("ST_Ui", "BPUI_GashaponRecord_Name"),
        ],
        "count" => &[
            ("ST_UI_hpy", "MangHe_09"),
            ("ST_UI_hpy", "MangHe_23"),
            ("ST_Ui", "BPUI_ConsumableUse_UseNumber"),
        ],
        "roll_label" => &[("ST_Ui", "BPUI_LotteryDiceRecord_touzhidianshu")],
        "secondary_item_name" => &[("ST_Ui", "BPUI_LotteryResult_AdditionalReward")],
        _ => &[],
    }
}

fn csv_header_joiner(locale: &str) -> &'static str {
    match locale {
        "ja" | "zh-CN" | "zh-Hans" | "zh-Hant" => "",
        _ => " ",
    }
}

fn csv_headers(localization: &Localization, locale: &str) -> JsonObject {
    let mut headers = BTreeMap::new();
    for field in [
        "time",
        "pool_group",
        "pool_name",
        "item_name",
        "count",
        "roll_label",
        "secondary_item_name",
        "secondary_count",
    ] {
        let mut text = custom_csv_header(field, locale).map(ToString::to_string);
        for (namespace, key) in csv_header_keys(field) {
            if text.is_some() {
                break;
            }
            text = clean_name(localized_key(localization, namespace, key));
        }
        headers.insert(field.to_string(), text.unwrap_or_else(|| field.to_string()));
    }
    if headers.get("secondary_item_name").map(String::as_str) != Some("secondary_item_name")
        && headers.get("count").map(String::as_str) != Some("count")
    {
        let joiner = csv_header_joiner(locale);
        let secondary_item_name = headers
            .get("secondary_item_name")
            .cloned()
            .unwrap_or_default();
        let count = headers.get("count").cloned().unwrap_or_default();
        headers.insert(
            "secondary_count".to_string(),
            format!("{secondary_item_name}{joiner}{count}"),
        );
    }
    let pool_header = custom_csv_header("pool_name", locale).map(ToString::to_string);
    let pool_type_header = clean_name(localized_key(
        localization,
        "ST_UI_C",
        "BPUI_CharacterEquipDevFilter_15",
    ));
    if let (Some(pool_header), Some(pool_type_header)) = (pool_header, pool_type_header) {
        let joiner = csv_header_joiner(locale);
        headers.insert(
            "pool_group".to_string(),
            format!("{pool_header}{joiner}{pool_type_header}"),
        );
    }
    string_map_value(headers)
}

fn build_labels(localization: &Localization) -> BTreeMap<String, String> {
    let label_keys = [
        ("Abyss_GamepadKeys_1", "ST_Ui", "Abyss_GamepadKeys_1"),
        ("AbyssClone_Award_02", "ST_Ui", "AbyssClone_Award_02"),
        (
            "BPUI_LotteryResult_jidianzengli",
            "ST_Ui",
            "BPUI_LotteryResult_jidianzengli",
        ),
        (
            "BPUI_LotteryResult_chenmiandi",
            "ST_Ui",
            "BPUI_LotteryResult_chenmiandi",
        ),
        (
            "BPUI_LotteryDiceRecord_biaozhunqipan",
            "ST_Ui",
            "BPUI_LotteryDiceRecord_biaozhunqipan",
        ),
        (
            "BPUI_LotteryDiceRecord_qipanleixing",
            "ST_Ui",
            "BPUI_LotteryDiceRecord_qipanleixing",
        ),
        (
            "BPUI_LotteryDiceRecord_xiandingqipan",
            "ST_Ui",
            "BPUI_LotteryDiceRecord_xiandingqipan",
        ),
        (
            "BPUI_LotteryModuleEntrance_Title",
            "ST_Ui",
            "BPUI_LotteryModuleEntrance_Title",
        ),
        ("TreasureBox_2", "ST_Ui", "TreasureBox_2"),
        (
            "UI_CloneSystemChallengeFailed_Retry",
            "ST_Ui",
            "UI_CloneSystemChallengeFailed_Retry",
        ),
        (
            "UI_CloneSystemStaminaTips_Enter",
            "ST_Ui",
            "UI_CloneSystemStaminaTips_Enter",
        ),
        (
            "UI_Lottery_GachaDetails_Zhitoujilu",
            "ST_Ui",
            "UI_Lottery_GachaDetails_Zhitoujilu",
        ),
        (
            "UI_Lottery_GachaDetails_title",
            "ST_Ui",
            "UI_Lottery_GachaDetails_title",
        ),
        (
            "UW_LotteryBase_BP_Hupanyanmu",
            "ST_Ui",
            "UW_LotteryBase_BP_Hupanyanmu",
        ),
        ("Mall_8_name", "ST_Ui", "Mall_8_name"),
        (
            "W_Vehicle_Button_Choose",
            "ST_Ui",
            "W_Vehicle_Button_Choose",
        ),
        ("W_HTButton_Next_Page", "ST_Ui", "W_HTButton_Next_Page"),
        ("common_3", "ST_Ui", "common_3"),
        ("ui_forkshop_03", "ST_Ui", "ui_forkshop_03"),
        ("ui_forkshop_07", "ST_Ui", "ui_forkshop_07"),
        ("ui_forkshop_10", "ST_Ui", "ui_forkshop_10"),
        ("ui_appearance_02", "ST_Ui", "ui_appearance_02"),
    ];
    label_keys
        .into_iter()
        .filter_map(|(label_id, namespace, key)| {
            clean_name(localized_key(localization, namespace, key))
                .map(|text| (label_id.to_string(), text))
        })
        .collect()
}

fn validate_map_source(map: &Value, source: &str) -> Result<(), GuiError> {
    let object = map
        .as_object()
        .ok_or_else(|| invalid(format!("localization map must be an object: {source}")))?;
    if object.get("schema_version").and_then(Value::as_u64) != Some(MAP_SCHEMA_VERSION) {
        return Err(invalid(format!(
            "map schema_version must be {MAP_SCHEMA_VERSION}: {source}"
        )));
    }
    let items = required_section(object, source, "items")?;
    let pools = required_section(object, source, "pools")?;
    let banners = required_section(object, source, "banners")?;
    let rules = required_section(object, source, "gacha_rules")?;
    for (item_id, item) in items {
        let item = item
            .as_object()
            .ok_or_else(|| invalid(format!("item must be object: {source}:items.{item_id}")))?;
        required_non_empty_str(item, source, &format!("items.{item_id}.name"))?;
        if item.get("rarity").and_then(Value::as_u64).is_none() {
            return Err(invalid(format!(
                "item rarity must be integer: {source}:items.{item_id}.rarity"
            )));
        }
    }
    for (pool_id, pool) in pools {
        let pool = pool
            .as_object()
            .ok_or_else(|| invalid(format!("pool must be object: {source}:pools.{pool_id}")))?;
        required_non_empty_str(pool, source, &format!("pools.{pool_id}.name"))?;
    }
    for (banner_id, banner) in banners {
        let banner = banner.as_object().ok_or_else(|| {
            invalid(format!(
                "banner must be object: {source}:banners.{banner_id}"
            ))
        })?;
        let declared_id =
            required_non_empty_str(banner, source, &format!("banners.{banner_id}.banner_id"))?;
        if declared_id != banner_id {
            return Err(invalid(format!(
                "banner_id mismatch: {source}:banners.{banner_id}"
            )));
        }
        let pool_id =
            required_non_empty_str(banner, source, &format!("banners.{banner_id}.pool_id"))?;
        if !pools.contains_key(pool_id) {
            return Err(invalid(format!(
                "banner targets unknown pool: {source}:banners.{banner_id}.pool_id"
            )));
        }
        let rule_id =
            required_non_empty_str(banner, source, &format!("banners.{banner_id}.rule_id"))?;
        if !rules.contains_key(rule_id) {
            return Err(invalid(format!(
                "banner targets unknown rule: {source}:banners.{banner_id}.rule_id"
            )));
        }
    }
    Ok(())
}

fn required_section<'a>(
    object: &'a JsonObject,
    source: &str,
    section: &str,
) -> Result<&'a JsonObject, GuiError> {
    object
        .get(section)
        .and_then(Value::as_object)
        .ok_or_else(|| {
            invalid(format!(
                "localization map section must be object: {source}:{section}"
            ))
        })
}

fn required_non_empty_str<'a>(
    object: &'a JsonObject,
    source: &str,
    path: &str,
) -> Result<&'a str, GuiError> {
    object
        .get(path.rsplit('.').next().unwrap_or(path))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid(format!("field must be non-empty string: {source}:{path}")))
}

fn map_from_pairs<const N: usize>(pairs: [(&str, Value); N]) -> JsonObject {
    pairs
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
}

fn string_map_value(values: BTreeMap<String, String>) -> JsonObject {
    values
        .into_iter()
        .map(|(key, value)| (key, Value::String(value)))
        .collect()
}

fn value_to_text(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        Value::Array(_) | Value::Object(_) => None,
    }
}

fn dedupe_strings(values: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    values.retain(|value| seen.insert(value.clone()));
}

fn section_len(map: &Value, section: &str) -> usize {
    map.get(section)
        .and_then(Value::as_object)
        .map_or(0, |object| object.len())
}

fn expand_home(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if text == "~" {
        return dirs_home().unwrap_or_else(|| path.to_path_buf());
    }
    if let Some(tail) = text.strip_prefix("~/") {
        if let Some(home) = dirs_home() {
            return home.join(tail);
        }
    }
    path.to_path_buf()
}

fn dirs_home() -> Option<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("USERPROFILE").map(PathBuf::from))
}

fn invalid(message: impl Into<String>) -> GuiError {
    GuiError::InvalidDocument(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_map_from_minimal_assets() {
        let tmp = tempfile::tempdir().unwrap();
        write_minimal_assets(tmp.path());

        let map = build_asset_map(tmp.path(), "zh-Hant").unwrap();

        assert_eq!(map["schema_version"], 4);
        assert_eq!(map["items"]["1010"]["name"], "Character·Nanali");
        assert_eq!(map["items"]["1010"]["rarity"], 5);
        assert_eq!(map["items"]["1010"]["asset_refs"]["banner"], "/Game/Banner");
        assert_eq!(map["pools"]["CardPool_NewRole"]["title"], "Standard Title");
        assert_eq!(
            map["pools"]["ForkLottery_Test"]["banner_ids"][0],
            "ForkLottery_Test"
        );
        assert_eq!(
            map["banners"]["monopoly_limited_Nanali"]["phase"],
            "limited_2026_05_13"
        );
        assert_eq!(
            map["gacha_rules"]["fork_lottery_s"]["hard_pity_5"],
            json!(80)
        );
        assert_eq!(map["labels"]["UW_LotteryBase_BP_Hupanyanmu"], "Fork Group");
    }

    #[test]
    fn discovers_locales_and_skips_removed_locale() {
        let tmp = tempfile::tempdir().unwrap();
        write_json(tmp.path().join("DataTable/.keep"), json!({}));
        fs::create_dir_all(tmp.path().join("DataTable")).unwrap();
        for locale in ["en", "zh-Hant", "en-JM"] {
            write_json(
                tmp.path()
                    .join("Localization")
                    .join(locale)
                    .join("game.json"),
                json!({}),
            );
        }

        let locales = discover_asset_locales(tmp.path()).unwrap();

        assert_eq!(locales, vec!["en", "zh-Hant"]);
    }

    fn write_minimal_assets(root: &Path) {
        write_json(
            root.join("Localization/en/game.json"),
            json!({
                "ST_Common": {
                    "item_type_2": "Item",
                    "item_type_3": "Character",
                    "item_type_5": "Arc",
                    "item_type_10": "Mod Parts"
                },
                "ST_Ui": {
                    "BPUI_LotteryDiceRecord_biaozhunqipan": "Standard",
                    "BPUI_LotteryDiceRecord_xiandingqipan": "Limited",
                    "UW_LotteryBase_BP_Hupanyanmu": "Fork Group",
                    "Lottery_Kachimingcheng_changzhu": "Standard Title",
                    "Lottery_Kachimingcheng_nanali": "Nanali Banner",
                    "BPUI_GashaponRecord_time": "Time",
                    "BPUI_LotteryDiceRecord_qipanleixing": "Pool Type",
                    "BPUI_LotteryDiceRecord_daojumingcheng": "Item Name",
                    "BPUI_ConsumableUse_UseNumber": "Count",
                    "BPUI_LotteryDiceRecord_touzhidianshu": "Roll",
                    "BPUI_LotteryResult_AdditionalReward": "Extra"
                },
                "ST_UI_C": {
                    "BPUI_CharacterEquipDevFilter_15": "Type"
                }
            }),
        );
        write_json(
            root.join("Localization/zh-Hant/game.json"),
            json!({
                "ST_Common": {
                    "item_type_3": "Character",
                    "item_type_5": "Arc"
                },
                "ST_Ui": {
                    "BPUI_LotteryDiceRecord_biaozhunqipan": "標準棋盤",
                    "BPUI_LotteryDiceRecord_xiandingqipan": "限定棋盤",
                    "UW_LotteryBase_BP_Hupanyanmu": "Fork Group",
                    "Lottery_Kachimingcheng_changzhu": "Standard Title",
                    "Lottery_Kachimingcheng_nanali": "Nanali Banner"
                }
            }),
        );
        write_json(
            root.join("DataTable/Character/DT_Character.json"),
            json!({"Rows": {
                "1010": {
                    "ItemName": {"Key": "char_1010", "TableId": "ST_Item"},
                    "ItemQuality": "EItemQuality::ITEM_QUALITY_ORANGE",
                    "ItemIcon": {"AssetPathName": "/Game/Icon"},
                    "ItemIconBig": {"AssetPathName": "/Game/Portrait"}
                }
            }}),
        );
        write_json(
            root.join("DataTable/Fork/DT_ForkItemData.json"),
            json!({"Rows": {
                "200": {
                    "ItemName": {"Key": "fork_200", "TableId": "ST_Item"},
                    "ItemQuality": "EItemQuality::ITEM_QUALITY_ORANGE"
                }
            }}),
        );
        write_json(
            root.join("Localization/zh-Hant/game.json"),
            json!({
                "ST_Common": {
                    "item_type_3": "Character",
                    "item_type_5": "Arc"
                },
                "ST_Item": {
                    "char_1010": "Nanali",
                    "fork_200": "Fork Weapon"
                },
                "ST_Ui": {
                    "BPUI_LotteryDiceRecord_biaozhunqipan": "標準棋盤",
                    "BPUI_LotteryDiceRecord_xiandingqipan": "限定棋盤",
                    "UW_LotteryBase_BP_Hupanyanmu": "Fork Group",
                    "Lottery_Kachimingcheng_changzhu": "Standard Title",
                    "Lottery_Kachimingcheng_nanali": "Nanali Banner"
                }
            }),
        );
        write_json(
            root.join("DataTable/Gacha/DT_LotteryDataTable_Nanali.json"),
            json!({"Rows": {
                "row": {
                    "SSRItems": [{"ItemID": "1010"}],
                    "SRItems": []
                }
            }}),
        );
        write_json(
            root.join("DataTable/Gacha/GachaIllustrate.json"),
            json!({"Rows": {
                "1010": {
                    "ItemIcon": {"AssetPathName": "/Game/Portrait"},
                    "ActivityHeadIcon": {"AssetPathName": "/Game/Banner"},
                    "OutlineColor": {"Hex": "ffcc00"}
                }
            }}),
        );
        write_json(
            root.join("DataTable/Fork/DT_ForkLotteryPoolData.json"),
            json!({"Rows": {
                "ForkLottery_Test": {
                    "Name": "Fork Pool",
                    "ShowText1": "Fork Banner",
                    "UpList": ["200"],
                    "BaseDropID": "drop_fork",
                    "UpGuaranteeCnt": 80,
                    "CurrencyID": "1010",
                    "CurrencyCnt": 1,
                    "OnceLotteryCnt": 1
                }
            }}),
        );
    }

    fn write_json(path: PathBuf, value: Value) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    }
}
