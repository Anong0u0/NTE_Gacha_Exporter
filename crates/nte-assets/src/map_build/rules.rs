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

