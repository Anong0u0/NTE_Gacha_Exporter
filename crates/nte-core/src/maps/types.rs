use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::model::{BannerResolutionIssue, GuiError, ItemKind, PoolKind, ResolvedBanner};

const BUNDLED_MAPS: &[(&str, &str)] = &[
    (
        "de",
        include_str!("../../../nte-assets/resources/maps/de.json"),
    ),
    (
        "en",
        include_str!("../../../nte-assets/resources/maps/en.json"),
    ),
    (
        "es",
        include_str!("../../../nte-assets/resources/maps/es.json"),
    ),
    (
        "fr",
        include_str!("../../../nte-assets/resources/maps/fr.json"),
    ),
    (
        "ja",
        include_str!("../../../nte-assets/resources/maps/ja.json"),
    ),
    (
        "ko",
        include_str!("../../../nte-assets/resources/maps/ko.json"),
    ),
    (
        "ru",
        include_str!("../../../nte-assets/resources/maps/ru.json"),
    ),
    (
        "zh-CN",
        include_str!("../../../nte-assets/resources/maps/zh-CN.json"),
    ),
    (
        "zh-Hans",
        include_str!("../../../nte-assets/resources/maps/zh-Hans.json"),
    ),
    (
        "zh-Hant",
        include_str!("../../../nte-assets/resources/maps/zh-Hant.json"),
    ),
];
const MAP_SCHEMA_VERSION: u64 = 4;
static MAP_CACHE: OnceLock<Result<BTreeMap<&'static str, MapData>, String>> = OnceLock::new();

#[derive(Debug, Clone, Deserialize)]
pub struct MapData {
    #[serde(default)]
    pub csv_headers: BTreeMap<String, String>,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
    pub items: BTreeMap<String, MapItem>,
    #[serde(default)]
    pub item_aliases: BTreeMap<String, String>,
    pub pools: BTreeMap<String, MapPool>,
    #[serde(default)]
    pub banners: BTreeMap<String, MapBanner>,
    #[serde(default)]
    pub gacha_rules: BTreeMap<String, MapGachaRule>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MapItem {
    pub name: String,
    pub rarity: u8,
    pub category: Option<String>,
    pub subtype: Option<String>,
    #[serde(default)]
    pub asset_refs: BTreeMap<String, serde_json::Value>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MapPool {
    pub name: String,
    pub group_label: Option<String>,
    pub title: Option<String>,
    pub title_windows: Option<Vec<PoolTitleWindow>>,
    pub pickup_item_ids: Option<Vec<String>>,
    pub banner_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PoolTitleWindow {
    pub end_at_tz8: String,
    pub title: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MapBanner {
    pub banner_id: String,
    pub pool_id: String,
    pub pool_kind: String,
    pub banner_type: String,
    pub title: String,
    pub short_title: Option<String>,
    pub version: Option<String>,
    pub start_at: Option<String>,
    pub end_at: Option<String>,
    pub timezone: Option<String>,
    #[serde(default)]
    pub rate_up_5: Vec<String>,
    #[serde(default)]
    pub rate_up_4: Vec<String>,
    pub rule_id: String,
    #[serde(default)]
    pub asset_refs: BTreeMap<String, serde_json::Value>,
    pub color: Option<String>,
    pub currency_id: Option<String>,
    pub currency_count: Option<u64>,
    pub roll_unit: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
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
