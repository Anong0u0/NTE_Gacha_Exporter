use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::model::{BannerResolutionIssue, GuiError, ItemKind, PoolKind, ResolvedBannerSource};

const BUNDLED_MAP_CORE: &str =
    include_str!("../../../nte-assets/resources/maps/core.json");
const BUNDLED_OCCURRENCES: &str =
    include_str!("../../../nte-assets/resources/maps/banner-occurrences.json");
const BUNDLED_MAP_LOCALES: &[(&str, &str)] = &[
    (
        "de",
        include_str!("../../../nte-assets/resources/maps/locales/de.json"),
    ),
    (
        "en",
        include_str!("../../../nte-assets/resources/maps/locales/en.json"),
    ),
    (
        "es",
        include_str!("../../../nte-assets/resources/maps/locales/es.json"),
    ),
    (
        "fr",
        include_str!("../../../nte-assets/resources/maps/locales/fr.json"),
    ),
    (
        "ja",
        include_str!("../../../nte-assets/resources/maps/locales/ja.json"),
    ),
    (
        "ko",
        include_str!("../../../nte-assets/resources/maps/locales/ko.json"),
    ),
    (
        "ru",
        include_str!("../../../nte-assets/resources/maps/locales/ru.json"),
    ),
    (
        "zh-CN",
        include_str!("../../../nte-assets/resources/maps/locales/zh-CN.json"),
    ),
    (
        "zh-Hans",
        include_str!("../../../nte-assets/resources/maps/locales/zh-Hans.json"),
    ),
    (
        "zh-Hant",
        include_str!("../../../nte-assets/resources/maps/locales/zh-Hant.json"),
    ),
];
const MAP_CORE_SCHEMA: &str = "nte-map-core";
const MAP_CORE_SCHEMA_VERSION: u64 = 1;
const MAP_LOCALE_SCHEMA: &str = "nte-map-locale";
const MAP_LOCALE_SCHEMA_VERSION: u64 = 1;
const BANNER_OCCURRENCES_SCHEMA: &str = "nte-banner-occurrences";
const BANNER_OCCURRENCES_SCHEMA_VERSION: u64 = 1;
static MAP_CACHE: OnceLock<Result<BTreeMap<&'static str, MapData>, String>> = OnceLock::new();

#[derive(Debug, Clone, Copy)]
pub struct BannerResolveInput<'a> {
    pub pool_id: &'a str,
    pub lottery_id: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub struct MapData {
    pub csv_headers: BTreeMap<String, String>,
    pub labels: BTreeMap<String, String>,
    pub items: BTreeMap<String, MapItem>,
    pub item_aliases: BTreeMap<String, String>,
    pub pools: BTreeMap<String, MapPool>,
    pub banner_sources: BTreeMap<String, MapBannerSource>,
    pub banner_occurrences: Vec<MapBannerOccurrence>,
    pub gacha_rules: BTreeMap<String, MapGachaRule>,
}

#[derive(Debug, Clone)]
pub struct MapItem {
    pub name: String,
    pub rarity: u8,
    pub category: Option<String>,
    pub subtype: Option<String>,
    pub asset_refs: BTreeMap<String, serde_json::Value>,
    pub color: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MapPool {
    pub name: String,
    pub group_label: Option<String>,
    pub title: Option<String>,
    pub pickup_item_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct MapBannerSource {
    pub source_id: String,
    pub pool_id: String,
    pub pool_kind: String,
    pub banner_type: String,
    pub title: String,
    pub short_title: Option<String>,
    pub rate_up_5: Vec<String>,
    pub rate_up_4: Vec<String>,
    pub rule_id: String,
    pub asset_refs: BTreeMap<String, serde_json::Value>,
    pub color: Option<String>,
    pub currency_id: Option<String>,
    pub currency_count: Option<u64>,
    pub roll_unit: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MapBannerOccurrence {
    pub occurrence_id: String,
    pub source_id: String,
    pub active_from: Option<String>,
    pub active_until: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapGachaRule {
    pub rule_id: String,
    pub pool_kind: String,
    pub hard_pity_5: Option<u64>,
    pub hard_up_pity_5: Option<u64>,
    pub pickup_win_rate_5: Option<u8>,
    pub has_guarantee_5: Option<bool>,
    pub guarantee_scope: Option<String>,
    pub carry_scope: Option<String>,
    #[serde(default)]
    pub rule_text_refs: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct MapCoreDocument {
    schema: String,
    schema_version: u64,
    items: BTreeMap<String, MapItemCore>,
    #[serde(default)]
    item_aliases: BTreeMap<String, String>,
    pools: BTreeMap<String, MapPoolCore>,
    banner_sources: BTreeMap<String, MapBannerSourceCore>,
    #[serde(default)]
    gacha_rules: BTreeMap<String, MapGachaRule>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct MapItemCore {
    rarity: u8,
    category: Option<String>,
    subtype: Option<String>,
    #[serde(default)]
    asset_refs: BTreeMap<String, serde_json::Value>,
    color: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct MapPoolCore {
    pickup_item_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct MapBannerSourceCore {
    source_id: String,
    pool_id: String,
    pool_kind: String,
    banner_type: String,
    #[serde(default)]
    rate_up_5: Vec<String>,
    #[serde(default)]
    rate_up_4: Vec<String>,
    rule_id: String,
    #[serde(default)]
    asset_refs: BTreeMap<String, serde_json::Value>,
    color: Option<String>,
    currency_id: Option<String>,
    currency_count: Option<u64>,
    roll_unit: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct MapLocaleDocument {
    schema: String,
    schema_version: u64,
    locale: String,
    #[serde(default)]
    csv_headers: BTreeMap<String, String>,
    #[serde(default)]
    labels: BTreeMap<String, String>,
    items: BTreeMap<String, MapItemLocale>,
    pools: BTreeMap<String, MapPoolLocale>,
    banner_sources: BTreeMap<String, MapBannerSourceLocale>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct MapItemLocale {
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct MapPoolLocale {
    name: String,
    group_label: Option<String>,
    title: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct MapBannerSourceLocale {
    title: String,
    short_title: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct BannerOccurrenceDocument {
    schema: String,
    schema_version: u64,
    occurrences: Vec<MapBannerOccurrence>,
}
