fn validate_full_map_source(map: &Value, source: &str) -> Result<(), GuiError> {
    let object = map
        .as_object()
        .ok_or_else(|| invalid(format!("localization map must be an object: {source}")))?;
    let items = required_section(object, source, "items")?;
    let pools = required_section(object, source, "pools")?;
    let banners = required_section(object, source, "banners")?;
    let rules = required_section(object, source, "gacha_rules")?;
    validate_item_id_case_folds(items, source)?;
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

fn validate_core_source(core: &Value, source: &str) -> Result<(), GuiError> {
    let object = core
        .as_object()
        .ok_or_else(|| invalid(format!("map core must be an object: {source}")))?;
    validate_schema(
        object,
        source,
        MAP_CORE_SCHEMA,
        MAP_CORE_SCHEMA_VERSION,
    )?;
    validate_exact_keys(
        object,
        source,
        &[
            "schema",
            "schema_version",
            "items",
            "item_aliases",
            "pools",
            "banner_sources",
            "gacha_rules",
        ],
    )?;
    let items = required_section(object, source, "items")?;
    let pools = required_section(object, source, "pools")?;
    let banners = required_section(object, source, "banner_sources")?;
    let rules = required_section(object, source, "gacha_rules")?;
    validate_item_id_case_folds(items, source)?;
    for (item_id, item) in items {
        let item = item
            .as_object()
            .ok_or_else(|| invalid(format!("item must be object: {source}:items.{item_id}")))?;
        if item.contains_key("name") {
            return Err(invalid(format!(
                "localized field is forbidden in core: {source}:items.{item_id}.name"
            )));
        }
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
        for field in ["name", "group_label", "title"] {
            if pool.contains_key(field) {
                return Err(invalid(format!(
                    "localized field is forbidden in core: {source}:pools.{pool_id}.{field}"
                )));
            }
        }
    }
    for (source_id, banner) in banners {
        let banner = banner.as_object().ok_or_else(|| {
            invalid(format!(
                "banner source must be object: {source}:banner_sources.{source_id}"
            ))
        })?;
        let declared_id = required_non_empty_str(
            banner,
            source,
            &format!("banner_sources.{source_id}.source_id"),
        )?;
        if declared_id != source_id {
            return Err(invalid(format!(
                "source_id mismatch: {source}:banner_sources.{source_id}"
            )));
        }
        for field in [
            "title",
            "short_title",
            "version",
            "start_at",
            "end_at",
            "mainland_start_at",
            "mainland_end_at",
            "timezone",
        ] {
            if banner.contains_key(field) {
                return Err(invalid(format!(
                    "locale or occurrence field is forbidden in core: {source}:banner_sources.{source_id}.{field}"
                )));
            }
        }
        let pool_id = required_non_empty_str(
            banner,
            source,
            &format!("banner_sources.{source_id}.pool_id"),
        )?;
        if !pools.contains_key(pool_id) {
            return Err(invalid(format!(
                "banner source targets unknown pool: {source}:banner_sources.{source_id}.pool_id"
            )));
        }
        let rule_id = required_non_empty_str(
            banner,
            source,
            &format!("banner_sources.{source_id}.rule_id"),
        )?;
        if !rules.contains_key(rule_id) {
            return Err(invalid(format!(
                "banner source targets unknown rule: {source}:banner_sources.{source_id}.rule_id"
            )));
        }
    }
    Ok(())
}

fn validate_locale_source(
    overlay: &Value,
    core: &Value,
    source: &str,
) -> Result<(), GuiError> {
    let object = overlay
        .as_object()
        .ok_or_else(|| invalid(format!("map locale must be an object: {source}")))?;
    validate_schema(
        object,
        source,
        MAP_LOCALE_SCHEMA,
        MAP_LOCALE_SCHEMA_VERSION,
    )?;
    validate_exact_keys(
        object,
        source,
        &[
            "schema",
            "schema_version",
            "locale",
            "csv_headers",
            "labels",
            "items",
            "pools",
            "banner_sources",
        ],
    )?;
    if object.get("locale").and_then(Value::as_str) != Some(source) {
        return Err(invalid(format!("locale mismatch: {source}")));
    }
    let core = core
        .as_object()
        .ok_or_else(|| invalid("map core must be an object"))?;
    for section in ["items", "pools", "banner_sources"] {
        let expected = required_section(core, "core", section)?;
        let localized = required_section(object, source, section)?;
        if expected.keys().ne(localized.keys()) {
            return Err(invalid(format!(
                "locale keys must exactly match core: {source}:{section}"
            )));
        }
    }
    for (item_id, item) in required_section(object, source, "items")? {
        let item = item
            .as_object()
            .ok_or_else(|| invalid(format!("item locale must be object: {source}:items.{item_id}")))?;
        validate_exact_keys(item, source, &["name"])?;
        required_non_empty_str(item, source, &format!("items.{item_id}.name"))?;
    }
    for (pool_id, pool) in required_section(object, source, "pools")? {
        let pool = pool.as_object().ok_or_else(|| {
            invalid(format!(
                "pool locale must be object: {source}:pools.{pool_id}"
            ))
        })?;
        validate_allowed_keys(pool, source, &["name", "group_label", "title"])?;
        required_non_empty_str(pool, source, &format!("pools.{pool_id}.name"))?;
    }
    for (source_id, banner) in required_section(object, source, "banner_sources")? {
        let banner = banner.as_object().ok_or_else(|| {
            invalid(format!(
                "banner locale must be object: {source}:banner_sources.{source_id}"
            ))
        })?;
        validate_allowed_keys(banner, source, &["title", "short_title"])?;
        required_non_empty_str(
            banner,
            source,
            &format!("banner_sources.{source_id}.title"),
        )?;
    }
    required_section(object, source, "csv_headers")?;
    required_section(object, source, "labels")?;
    Ok(())
}

fn validate_schema(
    object: &JsonObject,
    source: &str,
    schema: &str,
    schema_version: u64,
) -> Result<(), GuiError> {
    if object.get("schema").and_then(Value::as_str) != Some(schema)
        || object.get("schema_version").and_then(Value::as_u64) != Some(schema_version)
    {
        return Err(invalid(format!(
            "expected {schema} schema_version {schema_version}: {source}"
        )));
    }
    Ok(())
}

fn validate_exact_keys(
    object: &JsonObject,
    source: &str,
    expected: &[&str],
) -> Result<(), GuiError> {
    validate_allowed_keys(object, source, expected)?;
    if expected.iter().any(|key| !object.contains_key(*key)) {
        return Err(invalid(format!("required field missing: {source}")));
    }
    Ok(())
}

fn validate_allowed_keys(
    object: &JsonObject,
    source: &str,
    allowed: &[&str],
) -> Result<(), GuiError> {
    if let Some(key) = object
        .keys()
        .find(|key| !allowed.contains(&key.as_str()))
    {
        return Err(invalid(format!("unexpected field: {source}:{key}")));
    }
    Ok(())
}

fn validate_item_id_case_folds(items: &JsonObject, source: &str) -> Result<(), GuiError> {
    let mut folded: BTreeMap<String, &str> = BTreeMap::new();
    for item_id in items.keys() {
        let key = item_id.to_ascii_lowercase();
        if let Some(existing) = folded.insert(key, item_id.as_str()) {
            return Err(invalid(format!(
                "case-insensitive duplicate item_id: {source}:items.{existing} and items.{item_id}"
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
