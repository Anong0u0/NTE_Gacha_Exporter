pub fn derive_pool_kind_hits(
    records: &[InternalRecord],
    map: &MapData,
    pool_kind: PoolKind,
) -> Result<PoolKindDerivedStats, GuiError> {
    let derived = crate::derive_records(records, map)?;
    let mut five_star_history = Vec::new();
    let mut summary_rule: Option<RuleResolution> = None;
    let mut current_5star_pity = 0_u64;
    let mut current_5star_guarantee = None;
    let mut total_pulls = 0_u64;

    for record in derived
        .into_iter()
        .filter(|record| record.rule.pool_kind == pool_kind)
    {
        if !record.counts_as_pull {
            continue;
        }
        total_pulls += 1;
        current_5star_pity = if record.hit_rarity == Some(5) {
            0
        } else {
            record.pity_5_after
        };
        current_5star_guarantee = record.guarantee_5_after;
        if summary_rule.is_none() {
            summary_rule = Some(rule_resolution_from_view(&record.rule));
        }
        let Some(rarity) = record.hit_rarity else {
            continue;
        };
        if rarity == 5 {
            let Some(source) = records
                .iter()
                .find(|source| source.record_id == record.record_id)
                .cloned()
            else {
                continue;
            };
            let banner = map.resolve_banner(&source.pool_id, source.time.as_deref());
            let hit = derived_hit_from_record(source, banner, &record, rarity);
            five_star_history.push(hit);
        }
    }

    Ok(PoolKindDerivedStats {
        total_pulls,
        current_5star_pity,
        current_5star_guarantee,
        five_star_history,
        summary_rule: summary_rule.unwrap_or_else(|| {
            fallback_rule_resolution(
                pool_kind,
                RuleResolutionIssue::FallbackPoolKind,
                "pool has no records; using pool-kind fallback",
            )
        }),
    })
}

impl RuleResolution {
    pub fn view(&self) -> GachaRuleView {
        GachaRuleView {
            resolution_issue: self.resolution_issue,
            reason: self.reason.clone(),
            rule_id: self.rule.rule_id.clone(),
            pool_kind: self.rule.pool_kind,
            hard_pity_5: self.rule.hard_pity_5,
            hard_up_pity_5: self.rule.hard_up_pity_5,
            pickup_win_rate_5: self.rule.pickup_win_rate_5,
            has_guarantee_5: self.rule.has_guarantee_5,
            guarantee_scope: self.rule.guarantee_scope.clone(),
            carry_scope: self.rule.carry_scope.clone(),
        }
    }
}

fn derived_hit_from_record(
    record: InternalRecord,
    banner: ResolvedBanner,
    derived: &RecordDerived,
    rarity: u8,
) -> DerivedHit {
    DerivedHit {
        record,
        banner,
        rule: rule_resolution_from_view(&derived.rule),
        rarity,
        pity_distance: match rarity {
            5 => derived.pity_5_before + 1,
            _ => 0,
        },
        result: derived.rate_up_result,
        guarantee_before: (rarity == 5).then_some(derived.guarantee_5_before).flatten(),
        guarantee_after: (rarity == 5).then_some(derived.guarantee_5_after).flatten(),
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
        resolution_issue: view.resolution_issue,
        reason: view.reason.clone(),
        rule: GachaRule {
            rule_id: view.rule_id.clone(),
            pool_kind: view.pool_kind,
            hard_pity_5: view.hard_pity_5,
            hard_up_pity_5: view.hard_up_pity_5,
            pickup_win_rate_5: view.pickup_win_rate_5,
            has_guarantee_5: view.has_guarantee_5,
            guarantee_scope: view.guarantee_scope.clone(),
            carry_scope: view.carry_scope.clone(),
        },
    }
}

fn rule_from_map(rule: &MapGachaRule, fallback_kind: PoolKind) -> GachaRule {
    GachaRule {
        rule_id: Some(rule.rule_id.clone()),
        pool_kind: pool_kind_from_str(&rule.pool_kind).unwrap_or(fallback_kind),
        hard_pity_5: rule.hard_pity_5,
        hard_up_pity_5: rule.hard_up_pity_5,
        pickup_win_rate_5: rule.pickup_win_rate_5,
        has_guarantee_5: rule.has_guarantee_5,
        guarantee_scope: rule.guarantee_scope.clone(),
        carry_scope: rule.carry_scope.clone(),
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
