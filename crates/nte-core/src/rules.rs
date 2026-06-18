use crate::{
    BannerResolutionStatus, GachaRuleView, GuiError, InternalRecord, PoolKind, RateUpResult,
    RecordDerived, ResolvedBanner, RuleResolutionStatus,
};
use crate::{MapData, MapGachaRule};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GachaRule {
    pub rule_id: Option<String>,
    pub pool_kind: PoolKind,
    pub hard_pity_5: Option<u64>,
    pub hard_pity_4: Option<u64>,
    pub pickup_win_rate_5: Option<u8>,
    pub pickup_win_rate_4: Option<u8>,
    pub has_guarantee_5: Option<bool>,
    pub has_guarantee_4: Option<bool>,
    pub guarantee_scope: Option<String>,
    pub carry_scope: Option<String>,
    pub source_confidence: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleResolution {
    pub status: RuleResolutionStatus,
    pub reason: String,
    pub rule: GachaRule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedHit {
    pub record: InternalRecord,
    pub banner: ResolvedBanner,
    pub rule: RuleResolution,
    pub rarity: u8,
    pub pity_distance: u64,
    pub result: RateUpResult,
    pub result_confidence: String,
    pub guarantee_before: Option<bool>,
    pub guarantee_after: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolKindDerivedStats {
    pub total_pulls: u64,
    pub current_5star_pity: u64,
    pub current_4star_pity: u64,
    pub current_5star_guarantee: Option<bool>,
    pub five_star_history: Vec<DerivedHit>,
    pub four_star_history: Vec<DerivedHit>,
    pub summary_rule: RuleResolution,
}

pub fn classify_pool_id(pool_id: &str) -> Result<PoolKind, GuiError> {
    match pool_id {
        "CardPool_Character" => Ok(PoolKind::MonopolyLimited),
        "CardPool_NewRole" => Ok(PoolKind::MonopolyStandard),
        value if value.starts_with("ForkLottery_") => Ok(PoolKind::ForkLottery),
        value => Err(GuiError::UnknownPoolId(value.to_string())),
    }
}

pub fn fallback_rule_for(kind: PoolKind) -> GachaRule {
    match kind {
        PoolKind::MonopolyLimited => GachaRule {
            rule_id: Some("fallback_monopoly_limited".to_string()),
            pool_kind: kind,
            hard_pity_5: Some(90),
            hard_pity_4: None,
            pickup_win_rate_5: None,
            pickup_win_rate_4: None,
            has_guarantee_5: Some(false),
            has_guarantee_4: None,
            guarantee_scope: Some("unknown".to_string()),
            carry_scope: Some("pool_kind".to_string()),
            source_confidence: Some("unknown".to_string()),
        },
        PoolKind::MonopolyStandard => GachaRule {
            rule_id: Some("fallback_monopoly_standard".to_string()),
            pool_kind: kind,
            hard_pity_5: Some(90),
            hard_pity_4: None,
            pickup_win_rate_5: None,
            pickup_win_rate_4: None,
            has_guarantee_5: Some(false),
            has_guarantee_4: None,
            guarantee_scope: Some("unknown".to_string()),
            carry_scope: Some("pool_kind".to_string()),
            source_confidence: Some("unknown".to_string()),
        },
        PoolKind::ForkLottery => GachaRule {
            rule_id: Some("fallback_fork_lottery".to_string()),
            pool_kind: kind,
            hard_pity_5: Some(80),
            hard_pity_4: None,
            pickup_win_rate_5: Some(25),
            pickup_win_rate_4: None,
            has_guarantee_5: Some(true),
            has_guarantee_4: None,
            guarantee_scope: Some("pool_kind".to_string()),
            carry_scope: Some("pool_kind".to_string()),
            source_confidence: Some("unknown".to_string()),
        },
    }
}

pub fn rule_for(kind: PoolKind) -> GachaRule {
    fallback_rule_for(kind)
}

pub fn fallback_rule_resolution(
    kind: PoolKind,
    status: RuleResolutionStatus,
    reason: impl Into<String>,
) -> RuleResolution {
    RuleResolution {
        status,
        reason: reason.into(),
        rule: fallback_rule_for(kind),
    }
}

pub fn rule_for_record(map: &MapData, record: &InternalRecord) -> Result<RuleResolution, GuiError> {
    let banner = map.resolve_banner(&record.pool_id, record.time.as_deref());
    rule_for_resolved_banner(map, record, &banner)
}

pub fn rule_for_resolved_banner(
    map: &MapData,
    record: &InternalRecord,
    banner: &ResolvedBanner,
) -> Result<RuleResolution, GuiError> {
    let kind = classify_pool_id(&record.pool_id)?;
    if banner.status != BannerResolutionStatus::Matched {
        return Ok(fallback_rule_resolution(
            kind,
            RuleResolutionStatus::MissingBanner,
            format!("banner resolution is {:?}", banner.status),
        ));
    }
    let Some(rule_id) = banner.rule_id.as_deref() else {
        return Ok(fallback_rule_resolution(
            kind,
            RuleResolutionStatus::MissingRule,
            "matched banner has no rule_id",
        ));
    };
    let Some(rule) = map.gacha_rule(rule_id) else {
        return Ok(fallback_rule_resolution(
            kind,
            RuleResolutionStatus::MissingRule,
            format!("gacha rule is not in map: {rule_id}"),
        ));
    };
    let normalized = rule_from_map(rule, kind);
    let status = if unsupported_scope(rule) {
        RuleResolutionStatus::UnsupportedScope
    } else {
        RuleResolutionStatus::Matched
    };
    let reason = if status == RuleResolutionStatus::UnsupportedScope {
        "gacha rule has unsupported scope".to_string()
    } else {
        "matched".to_string()
    };
    Ok(RuleResolution {
        status,
        reason,
        rule: normalized,
    })
}

pub fn rate_up_result(
    map: &MapData,
    record: &InternalRecord,
    rarity: u8,
    banner: &ResolvedBanner,
) -> RateUpResult {
    if banner.status != BannerResolutionStatus::Matched {
        return RateUpResult::Unknown;
    }
    let canonical = map.canonical_item_id(&record.item_id);
    let candidates = match rarity {
        5 => &banner.rate_up_5,
        4 => &banner.rate_up_4,
        _ => return RateUpResult::Unknown,
    };
    if candidates.is_empty() {
        return RateUpResult::Unknown;
    }
    let Some((_, item)) = map.item(&record.item_id) else {
        return RateUpResult::Unknown;
    };
    let Some(item_domain) = item
        .domain_type
        .as_deref()
        .filter(|value| !value.is_empty())
    else {
        return RateUpResult::Unknown;
    };
    let candidate_domains = rate_up_domains(map, candidates);
    if candidate_domains.is_empty() {
        return RateUpResult::Unknown;
    }
    if !candidate_domains
        .iter()
        .any(|candidate_domain| candidate_domain == item_domain)
    {
        return RateUpResult::NotApplicable;
    }
    if candidates
        .iter()
        .any(|candidate| map.canonical_item_id(candidate) == canonical)
    {
        RateUpResult::Up
    } else {
        RateUpResult::OffRate
    }
}

pub fn result_confidence(result: RateUpResult, banner: &ResolvedBanner) -> String {
    if result == RateUpResult::Unknown {
        return "unknown".to_string();
    }
    banner
        .source_confidence
        .clone()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn rate_up_domains(map: &MapData, candidates: &[String]) -> Vec<String> {
    let mut domains = Vec::new();
    for candidate in candidates {
        let canonical = map.canonical_item_id(candidate);
        let Some((_, item)) = map.item(canonical) else {
            continue;
        };
        let Some(domain) = item
            .domain_type
            .as_deref()
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if !domains.iter().any(|existing| existing == domain) {
            domains.push(domain.to_string());
        }
    }
    domains
}

pub fn derive_pool_kind_hits(
    records: &[InternalRecord],
    map: &MapData,
    pool_kind: PoolKind,
) -> Result<PoolKindDerivedStats, GuiError> {
    let derived = crate::derive_records(records, map)?;
    let mut five_star_history = Vec::new();
    let mut four_star_history = Vec::new();
    let mut summary_rule: Option<RuleResolution> = None;
    let mut current_5star_pity = 0_u64;
    let mut current_4star_pity = 0_u64;
    let mut current_5star_guarantee = None;
    let mut total_pulls = 0_u64;

    for record in derived
        .into_iter()
        .filter(|record| record.rule.pool_kind == pool_kind)
    {
        total_pulls += 1;
        current_5star_pity = record.pity_5_after;
        current_4star_pity = record.pity_4_after;
        current_5star_guarantee = record.guarantee_5_after;
        if summary_rule.is_none() {
            summary_rule = Some(rule_resolution_from_view(&record.rule));
        }
        let Some(rarity) = record.hit_rarity else {
            continue;
        };
        if rarity == 5 || rarity == 4 {
            let Some(source) = records
                .iter()
                .find(|source| source.record_id == record.record_id)
                .cloned()
            else {
                continue;
            };
            let banner = map.resolve_banner(&source.pool_id, source.time.as_deref());
            let hit = derived_hit_from_record(source, banner, &record, rarity);
            if rarity == 5 {
                five_star_history.push(hit);
            } else {
                four_star_history.push(hit);
            }
        }
    }

    Ok(PoolKindDerivedStats {
        total_pulls,
        current_5star_pity,
        current_4star_pity,
        current_5star_guarantee,
        five_star_history,
        four_star_history,
        summary_rule: summary_rule.unwrap_or_else(|| {
            fallback_rule_resolution(
                pool_kind,
                RuleResolutionStatus::FallbackPoolKind,
                "pool has no records; using pool-kind fallback",
            )
        }),
    })
}

impl RuleResolution {
    pub fn view(&self) -> GachaRuleView {
        GachaRuleView {
            status: self.status,
            reason: self.reason.clone(),
            rule_id: self.rule.rule_id.clone(),
            pool_kind: self.rule.pool_kind,
            hard_pity_5: self.rule.hard_pity_5,
            hard_pity_4: self.rule.hard_pity_4,
            pickup_win_rate_5: self.rule.pickup_win_rate_5,
            pickup_win_rate_4: self.rule.pickup_win_rate_4,
            has_guarantee_5: self.rule.has_guarantee_5,
            has_guarantee_4: self.rule.has_guarantee_4,
            guarantee_scope: self.rule.guarantee_scope.clone(),
            carry_scope: self.rule.carry_scope.clone(),
            source_confidence: self.rule.source_confidence.clone(),
        }
    }
}

fn derived_hit_from_record(
    record: InternalRecord,
    banner: ResolvedBanner,
    derived: &RecordDerived,
    rarity: u8,
) -> DerivedHit {
    let (guarantee_before, guarantee_after) = match rarity {
        5 => (derived.guarantee_5_before, derived.guarantee_5_after),
        4 => (derived.guarantee_4_before, derived.guarantee_4_after),
        _ => (None, None),
    };
    DerivedHit {
        record,
        banner,
        rule: rule_resolution_from_view(&derived.rule),
        rarity,
        pity_distance: match rarity {
            5 => derived.pity_5_before + 1,
            4 => derived.pity_4_before + 1,
            _ => 0,
        },
        result: derived.rate_up_result,
        result_confidence: derived.result_confidence.clone(),
        guarantee_before,
        guarantee_after,
    }
}

pub fn guarantee_state_for(
    state: &std::collections::HashMap<(String, u8), bool>,
    resolution: &RuleResolution,
    banner: &ResolvedBanner,
    rarity: u8,
    result: Option<RateUpResult>,
) -> (Option<bool>, Option<bool>) {
    match has_guarantee(&resolution.rule, rarity) {
        Some(false) => (Some(false), Some(false)),
        None => (None, None),
        Some(true) => {
            let Some(key) = guarantee_key(&resolution.rule, banner, rarity) else {
                return (None, None);
            };
            let before = *state.get(&key).unwrap_or(&false);
            let after = match result {
                Some(RateUpResult::OffRate) => true,
                Some(RateUpResult::Up) => false,
                Some(RateUpResult::NotApplicable | RateUpResult::Unknown) | None => before,
            };
            (Some(before), Some(after))
        }
    }
}

pub fn update_guarantee_state_for(
    state: &mut std::collections::HashMap<(String, u8), bool>,
    resolution: &RuleResolution,
    banner: &ResolvedBanner,
    rarity: u8,
    result: Option<RateUpResult>,
) -> (Option<bool>, Option<bool>) {
    let values = guarantee_state_for(state, resolution, banner, rarity, result);
    if let (Some(_), Some(after), Some(key)) = (
        values.0,
        values.1,
        guarantee_key(&resolution.rule, banner, rarity),
    ) {
        if has_guarantee(&resolution.rule, rarity) == Some(true) {
            state.insert(key, after);
        }
    }
    values
}

fn rule_resolution_from_view(view: &GachaRuleView) -> RuleResolution {
    RuleResolution {
        status: view.status,
        reason: view.reason.clone(),
        rule: GachaRule {
            rule_id: view.rule_id.clone(),
            pool_kind: view.pool_kind,
            hard_pity_5: view.hard_pity_5,
            hard_pity_4: view.hard_pity_4,
            pickup_win_rate_5: view.pickup_win_rate_5,
            pickup_win_rate_4: view.pickup_win_rate_4,
            has_guarantee_5: view.has_guarantee_5,
            has_guarantee_4: view.has_guarantee_4,
            guarantee_scope: view.guarantee_scope.clone(),
            carry_scope: view.carry_scope.clone(),
            source_confidence: view.source_confidence.clone(),
        },
    }
}

fn rule_from_map(rule: &MapGachaRule, fallback_kind: PoolKind) -> GachaRule {
    GachaRule {
        rule_id: Some(rule.rule_id.clone()),
        pool_kind: pool_kind_from_str(&rule.pool_kind).unwrap_or(fallback_kind),
        hard_pity_5: rule.hard_pity_5,
        hard_pity_4: rule.hard_pity_4,
        pickup_win_rate_5: rule.pickup_win_rate_5,
        pickup_win_rate_4: rule.pickup_win_rate_4,
        has_guarantee_5: rule.has_guarantee_5,
        has_guarantee_4: rule.has_guarantee_4,
        guarantee_scope: rule.guarantee_scope.clone(),
        carry_scope: rule.carry_scope.clone(),
        source_confidence: Some(rule.source.confidence.clone()),
    }
}

fn unsupported_scope(rule: &MapGachaRule) -> bool {
    let valid = |value: &str| matches!(value, "pool_kind" | "banner" | "unknown");
    rule.guarantee_scope
        .as_deref()
        .is_some_and(|value| !valid(value))
        || rule
            .carry_scope
            .as_deref()
            .is_some_and(|value| !valid(value))
}

fn pool_kind_from_str(value: &str) -> Option<PoolKind> {
    match value {
        "monopoly_limited" => Some(PoolKind::MonopolyLimited),
        "monopoly_standard" => Some(PoolKind::MonopolyStandard),
        "fork_lottery" => Some(PoolKind::ForkLottery),
        _ => None,
    }
}

fn has_guarantee(rule: &GachaRule, rarity: u8) -> Option<bool> {
    match rarity {
        5 => rule.has_guarantee_5,
        4 => rule.has_guarantee_4,
        _ => None,
    }
}

fn guarantee_key(rule: &GachaRule, banner: &ResolvedBanner, rarity: u8) -> Option<(String, u8)> {
    match rule.carry_scope.as_deref() {
        Some("pool_kind") => Some((rule.pool_kind.as_str().to_string(), rarity)),
        Some("banner") => banner
            .banner_id
            .as_ref()
            .map(|banner_id| (banner_id.clone(), rarity)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load_map;

    fn record(record_id: &str, pool_id: &str, item_id: &str, time: &str) -> InternalRecord {
        InternalRecord {
            record_id: record_id.to_string(),
            record_type: if pool_id.starts_with("ForkLottery_") {
                "fork".to_string()
            } else {
                "monopoly".to_string()
            },
            time: Some(time.to_string()),
            pool_id: pool_id.to_string(),
            item_id: item_id.to_string(),
            count: Some(1),
            roll_points: Some(1),
            secondary_item_id: None,
            secondary_count: None,
        }
    }

    #[test]
    fn rule_for_record_uses_matched_banner_rule() {
        let map = load_map("zh-Hant").expect("map should load");

        let fork = rule_for_record(
            &map,
            &record(
                "fork",
                "ForkLottery_AnHunQu",
                "fork_Rose",
                "2026-01-01 00:00:00",
            ),
        )
        .expect("fork rule should resolve");
        assert_eq!(fork.status, RuleResolutionStatus::Matched);
        assert_eq!(fork.rule.rule_id.as_deref(), Some("fork_lottery_s"));
        assert_eq!(fork.rule.hard_pity_5, Some(80));
        assert_eq!(fork.rule.pickup_win_rate_5, Some(25));
        assert_eq!(fork.rule.has_guarantee_5, Some(true));
        assert_eq!(fork.rule.source_confidence.as_deref(), Some("exact"));

        let standard = rule_for_record(
            &map,
            &record(
                "standard",
                "CardPool_NewRole",
                "fork_dustbin",
                "2026-01-01 00:00:00",
            ),
        )
        .expect("standard rule should resolve");
        assert_eq!(standard.status, RuleResolutionStatus::Matched);
        assert_eq!(standard.rule.rule_id.as_deref(), Some("monopoly_standard"));
        assert_eq!(standard.rule.hard_pity_5, Some(90));

        let limited = rule_for_record(
            &map,
            &record(
                "limited",
                "CardPool_Character",
                "1004",
                "2026-06-04 00:00:00",
            ),
        )
        .expect("limited rule should resolve");
        assert_eq!(limited.status, RuleResolutionStatus::Matched);
        assert_eq!(limited.rule.rule_id.as_deref(), Some("monopoly_limited"));
        assert_eq!(limited.rule.source_confidence.as_deref(), Some("curated"));
    }

    #[test]
    fn rule_for_record_falls_back_when_banner_is_unmatched() {
        let map = load_map("zh-Hant").expect("map should load");

        let resolution = rule_for_record(
            &map,
            &record(
                "outside",
                "CardPool_Character",
                "fork_dustbin",
                "2026-07-08 05:59:01",
            ),
        )
        .expect("fallback rule should resolve");

        assert_eq!(resolution.status, RuleResolutionStatus::MissingBanner);
        assert_eq!(resolution.rule.pool_kind, PoolKind::MonopolyLimited);
        assert_eq!(resolution.rule.hard_pity_5, Some(90));
        assert_eq!(
            resolution.rule.source_confidence.as_deref(),
            Some("unknown")
        );
    }

    #[test]
    fn rule_for_record_falls_back_when_rule_id_is_missing() {
        let mut map = load_map("zh-Hant").expect("map should load");
        map.banners
            .get_mut("ForkLottery_AnHunQu")
            .expect("fork banner should exist")
            .rule_id = "missing_rule".to_string();

        let resolution = rule_for_record(
            &map,
            &record(
                "missing-rule",
                "ForkLottery_AnHunQu",
                "fork_Rose",
                "2026-01-01 00:00:00",
            ),
        )
        .expect("fallback rule should resolve");

        assert_eq!(resolution.status, RuleResolutionStatus::MissingRule);
        assert_eq!(resolution.rule.pool_kind, PoolKind::ForkLottery);
        assert_eq!(resolution.rule.hard_pity_5, Some(80));
    }

    #[test]
    fn derive_pool_kind_hits_tracks_five_and_four_star_state() {
        let map = load_map("zh-Hant").expect("map should load");
        let records = vec![
            record(
                "r1",
                "ForkLottery_AnHunQu",
                "fork_Arachne",
                "2026-01-01 00:00:00",
            ),
            record(
                "r2",
                "ForkLottery_AnHunQu",
                "fork_jiaojuan",
                "2026-01-01 00:01:00",
            ),
            record(
                "r3",
                "ForkLottery_AnHunQu",
                "fork_Rose",
                "2026-01-01 00:02:00",
            ),
        ];

        let stats = derive_pool_kind_hits(&records, &map, PoolKind::ForkLottery)
            .expect("stats should derive");
        let derived = crate::derive_records(&records, &map).expect("records should derive");
        let derived_five_star_distances = derived
            .iter()
            .filter(|record| record.hit_rarity == Some(5))
            .map(|record| record.pity_5_before + 1)
            .collect::<Vec<_>>();

        assert_eq!(stats.total_pulls, 3);
        assert_eq!(stats.five_star_history.len(), 2);
        assert_eq!(stats.four_star_history.len(), 1);
        assert_eq!(
            stats
                .five_star_history
                .iter()
                .map(|hit| hit.pity_distance)
                .collect::<Vec<_>>(),
            derived_five_star_distances
        );
        assert_eq!(stats.five_star_history[0].result, RateUpResult::OffRate);
        assert_eq!(stats.five_star_history[0].guarantee_after, Some(true));
        assert_eq!(stats.five_star_history[1].result, RateUpResult::Up);
        assert_eq!(stats.five_star_history[1].guarantee_before, Some(true));
        assert_eq!(stats.current_4star_pity, 1);
        assert_eq!(stats.summary_rule.status, RuleResolutionStatus::Matched);
    }
}
