use crate::{
    GachaRuleView, GuiError, InternalRecord, ItemKind, PoolKind, RateUpResult, RecordDerived,
    ResolvedBanner, RuleResolutionIssue,
};
use crate::{MapData, MapGachaRule};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GachaRule {
    pub rule_id: Option<String>,
    pub pool_kind: PoolKind,
    pub hard_pity_5: Option<u64>,
    pub hard_up_pity_5: Option<u64>,
    pub pickup_win_rate_5: Option<u8>,
    pub has_guarantee_5: Option<bool>,
    pub guarantee_scope: Option<String>,
    pub carry_scope: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleResolution {
    pub resolution_issue: Option<RuleResolutionIssue>,
    pub reason: Option<String>,
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
    pub guarantee_before: Option<bool>,
    pub guarantee_after: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolKindDerivedStats {
    pub total_pulls: u64,
    pub current_5star_pity: u64,
    pub current_5star_guarantee: Option<bool>,
    pub five_star_history: Vec<DerivedHit>,
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
            hard_up_pity_5: None,
            pickup_win_rate_5: None,
            has_guarantee_5: Some(false),
            guarantee_scope: Some("unknown".to_string()),
            carry_scope: Some("pool_kind".to_string()),
        },
        PoolKind::MonopolyStandard => GachaRule {
            rule_id: Some("fallback_monopoly_standard".to_string()),
            pool_kind: kind,
            hard_pity_5: Some(90),
            hard_up_pity_5: None,
            pickup_win_rate_5: None,
            has_guarantee_5: Some(false),
            guarantee_scope: Some("unknown".to_string()),
            carry_scope: Some("pool_kind".to_string()),
        },
        PoolKind::ForkLottery => GachaRule {
            rule_id: Some("fallback_fork_lottery".to_string()),
            pool_kind: kind,
            hard_pity_5: Some(60),
            hard_up_pity_5: Some(80),
            pickup_win_rate_5: Some(25),
            has_guarantee_5: Some(true),
            guarantee_scope: Some("pool_kind".to_string()),
            carry_scope: Some("pool_kind".to_string()),
        },
    }
}

pub fn rule_for(kind: PoolKind) -> GachaRule {
    fallback_rule_for(kind)
}

pub fn fallback_rule_resolution(
    kind: PoolKind,
    issue: RuleResolutionIssue,
    reason: impl Into<String>,
) -> RuleResolution {
    RuleResolution {
        resolution_issue: Some(issue),
        reason: Some(reason.into()),
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
    if let Some(issue) = banner.resolution_issue {
        return Ok(fallback_rule_resolution(
            kind,
            RuleResolutionIssue::MissingBanner,
            format!("banner resolution issue is {issue:?}"),
        ));
    }
    let Some(rule_id) = banner.rule_id.as_deref() else {
        return Ok(fallback_rule_resolution(
            kind,
            RuleResolutionIssue::MissingRule,
            "resolved banner has no rule_id",
        ));
    };
    let Some(rule) = map.gacha_rule(rule_id) else {
        return Ok(fallback_rule_resolution(
            kind,
            RuleResolutionIssue::MissingRule,
            format!("gacha rule is not in map: {rule_id}"),
        ));
    };
    let normalized = rule_from_map(rule, kind);
    let resolution_issue = if unsupported_scope(rule) {
        Some(RuleResolutionIssue::UnsupportedScope)
    } else {
        None
    };
    let reason = if resolution_issue == Some(RuleResolutionIssue::UnsupportedScope) {
        Some("gacha rule has unsupported scope".to_string())
    } else {
        None
    };
    Ok(RuleResolution {
        resolution_issue,
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
    if banner.resolution_issue.is_some() {
        return RateUpResult::Unknown;
    }
    let Some((_, item)) = map.item(&record.item_id) else {
        return RateUpResult::Unknown;
    };
    if rarity == 5 && banner.banner_type.as_deref() == Some("standard") {
        return if map.item_kind(&record.item_id) == ItemKind::Character {
            RateUpResult::Up
        } else {
            RateUpResult::NotApplicable
        };
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
    let Some(item_domain) = item
        .category
        .as_deref()
        .or(item.subtype.as_deref())
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

fn rate_up_domains(map: &MapData, candidates: &[String]) -> Vec<String> {
    let mut domains = Vec::new();
    for candidate in candidates {
        let canonical = map.canonical_item_id(candidate);
        let Some((_, item)) = map.item(canonical) else {
            continue;
        };
        let Some(domain) = item
            .category
            .as_deref()
            .or(item.subtype.as_deref())
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
