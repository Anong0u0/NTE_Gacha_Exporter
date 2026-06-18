use std::collections::BTreeMap;

use serde::Deserialize;

use crate::model::{BannerResolutionStatus, GuiError, PoolKind, ResolvedBanner};

const BUNDLED_MAPS: &[(&str, &str)] = &[
    (
        "de",
        include_str!("../../nte-assets/resources/maps/de.json"),
    ),
    (
        "en",
        include_str!("../../nte-assets/resources/maps/en.json"),
    ),
    (
        "es",
        include_str!("../../nte-assets/resources/maps/es.json"),
    ),
    (
        "fr",
        include_str!("../../nte-assets/resources/maps/fr.json"),
    ),
    (
        "ja",
        include_str!("../../nte-assets/resources/maps/ja.json"),
    ),
    (
        "ko",
        include_str!("../../nte-assets/resources/maps/ko.json"),
    ),
    (
        "ru",
        include_str!("../../nte-assets/resources/maps/ru.json"),
    ),
    (
        "zh-CN",
        include_str!("../../nte-assets/resources/maps/zh-CN.json"),
    ),
    (
        "zh-Hans",
        include_str!("../../nte-assets/resources/maps/zh-Hans.json"),
    ),
    (
        "zh-Hant",
        include_str!("../../nte-assets/resources/maps/zh-Hant.json"),
    ),
];
const MAP_SCHEMA_VERSION: u64 = 4;

#[derive(Debug, Clone, Deserialize)]
pub struct MapData {
    #[serde(default)]
    pub csv_headers: BTreeMap<String, String>,
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
    pub domain_type: Option<String>,
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
    #[serde(default)]
    pub asset_refs: BTreeMap<String, serde_json::Value>,
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
    pub phase: Option<String>,
    pub start_at: Option<String>,
    pub end_at: Option<String>,
    pub timezone: Option<String>,
    #[serde(default)]
    pub rate_up_5: Vec<String>,
    #[serde(default)]
    pub rate_up_4: Vec<String>,
    #[serde(default)]
    pub standard_5_pool: Vec<String>,
    #[serde(default)]
    pub standard_4_pool: Vec<String>,
    pub rule_id: String,
    #[serde(default)]
    pub asset_refs: BTreeMap<String, serde_json::Value>,
    pub color: Option<String>,
    pub currency_id: Option<String>,
    pub currency_count: Option<u64>,
    pub roll_unit: Option<u64>,
    pub source: MapSourceEvidence,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MapGachaRule {
    pub rule_id: String,
    pub pool_kind: String,
    pub hard_pity_5: Option<u64>,
    pub hard_pity_4: Option<u64>,
    pub pickup_win_rate_5: Option<u8>,
    pub pickup_win_rate_4: Option<u8>,
    pub has_guarantee_5: Option<bool>,
    pub has_guarantee_4: Option<bool>,
    pub guarantee_scope: Option<String>,
    pub carry_scope: Option<String>,
    #[serde(default)]
    pub rule_text_refs: BTreeMap<String, String>,
    pub source: MapSourceEvidence,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MapSourceEvidence {
    pub confidence: String,
    #[serde(default)]
    pub tables: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
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
        != Some(MAP_SCHEMA_VERSION)
    {
        return Err(GuiError::InvalidDocument(format!(
            "map schema_version must be {MAP_SCHEMA_VERSION}: {locale}"
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

    pub fn gacha_rule(&self, rule_id: &str) -> Option<&MapGachaRule> {
        self.gacha_rules.get(rule_id)
    }

    pub fn rule_source_confidence(&self, rule_id: &str) -> Option<&str> {
        self.gacha_rule(rule_id)
            .map(|rule| rule.source.confidence.as_str())
    }

    pub fn pool_label(&self, pool_id: &str, time: Option<&str>) -> String {
        self.banner_label(pool_id, time)
    }

    pub fn banner_label(&self, pool_id: &str, time: Option<&str>) -> String {
        let resolved = self.resolve_banner(pool_id, time);
        if resolved.status == BannerResolutionStatus::Matched {
            if let Some(title) = resolved.title {
                return title;
            }
        }
        self.pool_fallback_label(pool_id, time)
    }

    fn pool_fallback_label(&self, pool_id: &str, time: Option<&str>) -> String {
        let Some(pool) = self.pools.get(pool_id) else {
            return pool_id.to_string();
        };
        if let (Some(record_time), Some(windows)) =
            (normalize_game_time(time), pool.title_windows.as_ref())
        {
            for window in windows {
                if let Some(end_at) = normalize_game_time(Some(&window.end_at_tz8)) {
                    if record_time.as_str() <= end_at.as_str() {
                        return window.title.clone();
                    }
                } else if record_time.as_str() <= window.end_at_tz8.as_str() {
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

    pub fn is_banner_rate_up(
        &self,
        pool_id: &str,
        time: Option<&str>,
        item_id: &str,
        rarity: Option<u8>,
    ) -> bool {
        let canonical = self.canonical_item_id(item_id);
        let resolved = self.resolve_banner(pool_id, time);
        if resolved.status != BannerResolutionStatus::Matched {
            return self.is_pickup_item(pool_id, item_id);
        }
        match rarity {
            Some(5) => resolved
                .rate_up_5
                .iter()
                .any(|candidate| candidate == canonical),
            Some(4) => resolved
                .rate_up_4
                .iter()
                .any(|candidate| candidate == canonical),
            _ => false,
        }
    }

    pub fn resolve_banner(&self, pool_id: &str, time: Option<&str>) -> ResolvedBanner {
        let Some(pool) = self.pools.get(pool_id) else {
            return unresolved(
                BannerResolutionStatus::UnknownPool,
                format!("pool is not in localization map: {pool_id}"),
            );
        };
        let Some(banner_ids) = pool.banner_ids.as_ref() else {
            return unresolved(
                BannerResolutionStatus::UnknownPool,
                format!("pool has no linked banners: {pool_id}"),
            );
        };

        let candidates = banner_ids
            .iter()
            .filter_map(|banner_id| self.banners.get(banner_id))
            .filter(|banner| banner.pool_id == pool_id)
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            return unresolved(
                BannerResolutionStatus::UnknownPool,
                format!("pool has no usable linked banners: {pool_id}"),
            );
        }

        match pool_id {
            "CardPool_NewRole" => single_banner(candidates, "standard", "standard"),
            "CardPool_Character" => resolve_limited_banner(candidates, time),
            value if value.starts_with("ForkLottery_") => {
                let exact = candidates
                    .iter()
                    .copied()
                    .filter(|banner| banner.banner_id == pool_id)
                    .collect::<Vec<_>>();
                if exact.is_empty() {
                    single_banner(candidates, "fork", "fork")
                } else {
                    single_banner(exact, "fork", "fork")
                }
            }
            _ => unresolved(
                BannerResolutionStatus::UnknownPool,
                format!("pool has unsupported banner resolution: {pool_id}"),
            ),
        }
    }
}

fn normalize_game_time(value: Option<&str>) -> Option<String> {
    let raw = value?.trim();
    if raw.len() < 19 || raw.contains('+') || raw.contains('Z') || raw.contains('z') {
        return None;
    }
    let mut text = raw.get(..19)?.replace('T', " ");
    let bytes = text.as_bytes();
    let valid = bytes.len() == 19
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b' '
        && bytes[13] == b':'
        && bytes[16] == b':'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7 | 10 | 13 | 16) || byte.is_ascii_digit());
    if !valid {
        return None;
    }
    if raw.len() > 19 {
        let suffix = raw.get(19..)?;
        if !suffix.starts_with('.') || !suffix[1..].bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
    }
    text.truncate(19);
    Some(text)
}

fn resolve_limited_banner(candidates: Vec<&MapBanner>, time: Option<&str>) -> ResolvedBanner {
    let record_time = match normalize_game_time(time) {
        Some(value) => value,
        None => {
            return unresolved(
                BannerResolutionStatus::UnknownTime,
                "limited banner resolution requires valid record time",
            );
        }
    };
    let mut windows = candidates
        .into_iter()
        .filter(|banner| banner.banner_type == "limited")
        .filter_map(|banner| {
            normalize_game_time(banner.end_at.as_deref()).map(|end_at| {
                (
                    normalize_game_time(banner.start_at.as_deref()),
                    end_at,
                    banner,
                )
            })
        })
        .collect::<Vec<_>>();
    windows.sort_by(|left, right| left.1.cmp(&right.1));

    if windows.is_empty() {
        return unresolved(
            BannerResolutionStatus::UnknownPool,
            "pool has no linked limited banners",
        );
    }

    let mut matches = Vec::new();
    let mut previous_end: Option<String> = None;
    for (start_at, end_at, banner) in windows {
        let effective_start = start_at.as_ref().or(previous_end.as_ref());
        let in_window = match effective_start {
            Some(start) => {
                start.as_str() < record_time.as_str() && record_time.as_str() <= end_at.as_str()
            }
            None => record_time.as_str() <= end_at.as_str(),
        };
        if in_window {
            matches.push(banner);
        }
        previous_end = Some(end_at);
    }

    match matches.len() {
        1 => matched(matches[0]),
        0 => unresolved(
            BannerResolutionStatus::OutsideKnownWindows,
            "record time is outside known limited banner windows",
        ),
        _ => unresolved(
            BannerResolutionStatus::Ambiguous,
            "multiple limited banners match record time",
        ),
    }
}

fn single_banner(
    candidates: Vec<&MapBanner>,
    banner_type: &str,
    reason_label: &str,
) -> ResolvedBanner {
    let matches = candidates
        .into_iter()
        .filter(|banner| banner.banner_type == banner_type)
        .collect::<Vec<_>>();
    match matches.len() {
        1 => matched(matches[0]),
        0 => unresolved(
            BannerResolutionStatus::UnknownPool,
            format!("pool has no linked {reason_label} banner"),
        ),
        _ => unresolved(
            BannerResolutionStatus::Ambiguous,
            format!("multiple {reason_label} banners are linked"),
        ),
    }
}

fn matched(banner: &MapBanner) -> ResolvedBanner {
    ResolvedBanner {
        status: BannerResolutionStatus::Matched,
        reason: "matched".to_string(),
        banner_id: Some(banner.banner_id.clone()),
        pool_id: Some(banner.pool_id.clone()),
        pool_kind: Some(banner.pool_kind.clone()),
        banner_type: Some(banner.banner_type.clone()),
        title: Some(banner.title.clone()),
        version: banner.version.clone(),
        phase: banner.phase.clone(),
        start_at: banner.start_at.clone(),
        end_at: banner.end_at.clone(),
        timezone: banner.timezone.clone(),
        rate_up_5: banner.rate_up_5.clone(),
        rate_up_4: banner.rate_up_4.clone(),
        rule_id: Some(banner.rule_id.clone()),
        asset_refs: banner.asset_refs.clone(),
        source_confidence: Some(banner.source.confidence.clone()),
    }
}

fn unresolved(status: BannerResolutionStatus, reason: impl Into<String>) -> ResolvedBanner {
    ResolvedBanner {
        status,
        reason: reason.into(),
        banner_id: None,
        pool_id: None,
        pool_kind: None,
        banner_type: None,
        title: None,
        version: None,
        phase: None,
        start_at: None,
        end_at: None,
        timezone: None,
        rate_up_5: Vec::new(),
        rate_up_4: Vec::new(),
        rule_id: None,
        asset_refs: BTreeMap::new(),
        source_confidence: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::BannerResolutionStatus;

    #[test]
    fn load_bundled_v4_map_keeps_banner_and_rule_sections() {
        let map = load_map("zh-Hant").expect("zh-Hant map should load");

        let banner = map
            .banners
            .get("ForkLottery_AnHunQu")
            .expect("fork banner should exist");
        assert_eq!(banner.banner_id, "ForkLottery_AnHunQu");
        assert_eq!(banner.pool_id, "ForkLottery_AnHunQu");
        assert_eq!(banner.rate_up_5, vec!["fork_Rose"]);
        assert_eq!(banner.source.confidence, "exact");
        assert_eq!(banner.currency_id.as_deref(), Some("WeaponGacha"));
        assert_eq!(
            map.banners
                .get("monopoly_limited_Nanali")
                .expect("limited banner should exist")
                .phase
                .as_deref(),
            Some("limited_2026_05_13")
        );

        let rule = map
            .gacha_rules
            .get("fork_lottery_s")
            .expect("fork rule should exist");
        assert_eq!(rule.hard_pity_5, Some(80));
        assert_eq!(rule.pickup_win_rate_5, Some(25));
        assert_eq!(rule.has_guarantee_5, Some(true));

        let item = map.items.get("1010").expect("item asset refs should exist");
        assert!(item.asset_refs.contains_key("portrait"));
        assert_eq!(item.color.as_deref(), Some("#F24B7E"));

        assert_eq!(
            map.pool_label("CardPool_Character", Some("2026-06-04 00:00:00")),
            "久夢初醒時"
        );
        assert!(map.is_pickup_item("ForkLottery_AnHunQu", "fork_Rose"));
    }

    #[test]
    fn resolve_banner_handles_standard_fork_and_limited_boundaries() {
        let map = load_map("zh-Hant").expect("zh-Hant map should load");

        let standard = map.resolve_banner("CardPool_NewRole", None);
        assert_eq!(standard.status, BannerResolutionStatus::Matched);
        assert_eq!(standard.banner_id.as_deref(), Some("monopoly_standard"));
        assert_eq!(standard.banner_type.as_deref(), Some("standard"));
        assert_eq!(standard.version, None);
        assert_eq!(standard.phase, None);

        let fork = map.resolve_banner("ForkLottery_AnHunQu", None);
        assert_eq!(fork.status, BannerResolutionStatus::Matched);
        assert_eq!(fork.banner_id.as_deref(), Some("ForkLottery_AnHunQu"));
        assert_eq!(fork.source_confidence.as_deref(), Some("exact"));
        assert_eq!(fork.rate_up_5, vec!["fork_Rose"]);

        for (record_time, banner_id) in [
            ("2026-05-13 05:59:00", "monopoly_limited_Nanali"),
            ("2026-05-13 05:59:01", "monopoly_limited_Xun"),
            ("2026-06-03 05:59:00", "monopoly_limited_Xun"),
            ("2026-06-03 05:59:01", "monopoly_limited_AnHunQu"),
            ("2026-06-24 05:59:00", "monopoly_limited_AnHunQu"),
            ("2026-06-24 05:59:01", "monopoly_limited_Kaesi"),
        ] {
            let resolved = map.resolve_banner("CardPool_Character", Some(record_time));
            assert_eq!(resolved.status, BannerResolutionStatus::Matched);
            assert_eq!(resolved.banner_id.as_deref(), Some(banner_id));
        }
        let limited = map.resolve_banner("CardPool_Character", Some("2026-05-13 05:59:00"));
        assert_eq!(limited.phase.as_deref(), Some("limited_2026_05_13"));
        assert_eq!(limited.version, None);
    }

    #[test]
    fn resolve_banner_reports_limited_unmatched_edges_and_label_fallback() {
        let map = load_map("zh-Hant").expect("zh-Hant map should load");

        assert_eq!(
            map.resolve_banner("CardPool_Character", None).status,
            BannerResolutionStatus::UnknownTime
        );
        assert_eq!(
            map.resolve_banner("CardPool_Character", Some("not a time"))
                .status,
            BannerResolutionStatus::UnknownTime
        );
        assert_eq!(
            map.resolve_banner("CardPool_Character", Some("2026-07-08 05:59:01"))
                .status,
            BannerResolutionStatus::OutsideKnownWindows
        );
        let unmatched = map.resolve_banner("CardPool_Character", Some("2026-07-08 05:59:01"));
        assert_eq!(unmatched.version, None);
        assert_eq!(unmatched.phase, None);
        assert_eq!(
            map.pool_label("CardPool_Character", Some("2026-07-08 05:59:01")),
            "限定棋盤"
        );
        assert_eq!(
            normalize_game_time(Some("2026-06-03T05:59:01.341000")).as_deref(),
            Some("2026-06-03 05:59:01")
        );
        assert!(normalize_game_time(Some("2026-06-03T05:59:01+08:00")).is_none());
    }
}
