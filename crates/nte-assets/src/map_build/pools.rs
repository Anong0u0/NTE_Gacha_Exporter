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

fn source_evidence(tables: &[String], notes: &[&str]) -> Value {
    let mut object = JsonObject::new();
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

fn monopoly_pool_meta(
    assets_root: &Path,
    localization: &Localization,
    canonicalizer: &ItemCanonicalizer,
    pool_id: &str,
) -> Result<JsonObject, GuiError> {
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
        return Ok(meta);
    }

    let mut title_windows = Vec::new();
    for banner in limited_monopoly_banners(assets_root, localization, canonicalizer, None)? {
        title_windows.push(json!({"end_at_tz8": banner.end_at_tz8, "title": banner.title}));
    }
    if !title_windows.is_empty() {
        meta.insert("title_windows".to_string(), Value::Array(title_windows));
    }
    Ok(meta)
}

fn add_monopoly_pools(
    pools: &mut BTreeMap<String, String>,
    pool_meta: &mut BTreeMap<String, JsonObject>,
    assets_root: &Path,
    localization: &Localization,
    canonicalizer: &ItemCanonicalizer,
) -> Result<(), GuiError> {
    for pool_id in ["CardPool_NewRole", "CardPool_Character"] {
        if let Some((namespace, key)) = pool_label_key(pool_id) {
            if let Some(name) = clean_name(localized_key(localization, namespace, key)) {
                add_pool(pools, pool_id.to_string(), name, true);
            }
        }
        let meta = monopoly_pool_meta(assets_root, localization, canonicalizer, pool_id)?;
        if !meta.is_empty() {
            pool_meta.insert(pool_id.to_string(), meta);
        }
    }
    Ok(())
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
    add_monopoly_pools(
        &mut pools,
        &mut pool_meta,
        assets_root,
        localization,
        canonicalizer,
    )?;
    add_fork_pools(
        &mut pools,
        &mut pool_meta,
        assets_root,
        localization,
        canonicalizer,
    )?;
    Ok((pools, pool_meta))
}
