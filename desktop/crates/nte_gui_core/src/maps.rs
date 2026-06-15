use std::collections::BTreeMap;

use serde::Deserialize;

use crate::model::{GuiError, PoolKind};

const BUNDLED_MAPS: &[(&str, &str)] = &[
    (
        "de",
        include_str!("../../../../src/nte_gacha_exporter/resources/maps/de.json"),
    ),
    (
        "en",
        include_str!("../../../../src/nte_gacha_exporter/resources/maps/en.json"),
    ),
    (
        "es",
        include_str!("../../../../src/nte_gacha_exporter/resources/maps/es.json"),
    ),
    (
        "fr",
        include_str!("../../../../src/nte_gacha_exporter/resources/maps/fr.json"),
    ),
    (
        "ja",
        include_str!("../../../../src/nte_gacha_exporter/resources/maps/ja.json"),
    ),
    (
        "ko",
        include_str!("../../../../src/nte_gacha_exporter/resources/maps/ko.json"),
    ),
    (
        "ru",
        include_str!("../../../../src/nte_gacha_exporter/resources/maps/ru.json"),
    ),
    (
        "zh-CN",
        include_str!("../../../../src/nte_gacha_exporter/resources/maps/zh-CN.json"),
    ),
    (
        "zh-Hans",
        include_str!("../../../../src/nte_gacha_exporter/resources/maps/zh-Hans.json"),
    ),
    (
        "zh-Hant",
        include_str!("../../../../src/nte_gacha_exporter/resources/maps/zh-Hant.json"),
    ),
];

#[derive(Debug, Clone, Deserialize)]
pub struct MapData {
    #[serde(default)]
    pub csv_headers: BTreeMap<String, String>,
    pub items: BTreeMap<String, MapItem>,
    #[serde(default)]
    pub item_aliases: BTreeMap<String, String>,
    pub pools: BTreeMap<String, MapPool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MapItem {
    pub name: String,
    pub rarity: u8,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MapPool {
    pub name: String,
    pub group_label: Option<String>,
    pub title: Option<String>,
    pub title_windows: Option<Vec<PoolTitleWindow>>,
    pub pickup_item_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PoolTitleWindow {
    pub end_at_tz8: String,
    pub title: String,
}

pub fn available_locales() -> Vec<String> {
    BUNDLED_MAPS
        .iter()
        .map(|(locale, _)| (*locale).to_string())
        .collect()
}

pub fn load_map(locale: &str) -> Result<MapData, GuiError> {
    let text = BUNDLED_MAPS
        .iter()
        .find(|(candidate, _)| *candidate == locale)
        .map(|(_, text)| *text)
        .ok_or_else(|| GuiError::LocaleNotFound(locale.to_string()))?;
    let value: serde_json::Value = serde_json::from_str(text)?;
    if value
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        != Some(2)
    {
        return Err(GuiError::InvalidDocument(format!(
            "map schema_version must be 2: {locale}"
        )));
    }
    Ok(serde_json::from_value(value)?)
}

impl MapData {
    pub fn canonical_item_id<'a>(&'a self, item_id: &'a str) -> &'a str {
        self.item_aliases
            .get(item_id)
            .map(String::as_str)
            .unwrap_or(item_id)
    }

    pub fn item<'a>(&'a self, item_id: &'a str) -> Option<(&'a str, &'a MapItem)> {
        let canonical = self.canonical_item_id(item_id);
        self.items
            .get(canonical)
            .map(|item| (canonical, item))
            .or_else(|| self.items.get(item_id).map(|item| (item_id, item)))
    }

    pub fn item_name(&self, item_id: &str) -> String {
        self.item(item_id)
            .map(|(_, item)| item.name.clone())
            .unwrap_or_else(|| item_id.to_string())
    }

    pub fn item_rarity(&self, item_id: &str) -> Option<u8> {
        self.item(item_id).map(|(_, item)| item.rarity)
    }

    pub fn pool_label(&self, pool_id: &str, time: Option<&str>) -> String {
        let Some(pool) = self.pools.get(pool_id) else {
            return pool_id.to_string();
        };
        if let (Some(record_time), Some(windows)) = (time, pool.title_windows.as_ref()) {
            for window in windows {
                if record_time <= window.end_at_tz8.as_str() {
                    return window.title.clone();
                }
            }
        }
        pool.title
            .clone()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| pool.name.clone())
    }

    pub fn pool_kind_label(&self, kind: PoolKind) -> String {
        let pool_id = match kind {
            PoolKind::MonopolyLimited => "CardPool_Character",
            PoolKind::MonopolyStandard => "CardPool_NewRole",
            PoolKind::ForkLottery => self
                .pools
                .keys()
                .find(|pool_id| pool_id.starts_with("ForkLottery_"))
                .map(String::as_str)
                .unwrap_or("ForkLottery"),
        };
        self.pools
            .get(pool_id)
            .map(|pool| {
                pool.group_label
                    .clone()
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| pool.name.clone())
            })
            .unwrap_or_else(|| kind.as_str().to_string())
    }

    pub fn has_pool_id(&self, pool_id: &str) -> bool {
        self.pools.contains_key(pool_id)
    }

    pub fn is_pickup_item(&self, pool_id: &str, item_id: &str) -> bool {
        let canonical = self.canonical_item_id(item_id);
        self.pools
            .get(pool_id)
            .and_then(|pool| pool.pickup_item_ids.as_ref())
            .is_some_and(|items| items.iter().any(|candidate| candidate == canonical))
    }
}
