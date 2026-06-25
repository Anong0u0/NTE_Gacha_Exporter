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
