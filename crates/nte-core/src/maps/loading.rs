pub fn available_locales() -> Vec<String> {
    BUNDLED_MAPS
        .iter()
        .map(|(locale, _)| (*locale).to_string())
        .collect()
}

pub fn bundled_maps_hash() -> String {
    let mut hasher = Sha256::new();
    for (locale, text) in BUNDLED_MAPS {
        hasher.update(locale.as_bytes());
        hasher.update([0]);
        hasher.update(text.as_bytes());
        hasher.update([0]);
    }
    format!("{:x}", hasher.finalize())
}

pub fn load_map(locale: &str) -> Result<MapData, GuiError> {
    let maps = MAP_CACHE
        .get_or_init(load_all_maps)
        .as_ref()
        .map_err(|error| GuiError::InvalidDocument(error.clone()))?;
    maps.get(locale)
        .cloned()
        .ok_or_else(|| GuiError::LocaleNotFound(locale.to_string()))
}

fn load_all_maps() -> Result<BTreeMap<&'static str, MapData>, String> {
    let mut maps = BTreeMap::new();
    for (locale, text) in BUNDLED_MAPS {
        maps.insert(*locale, parse_map(locale, text)?);
    }
    Ok(maps)
}

fn parse_map(locale: &str, text: &str) -> Result<MapData, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|error| format!("{locale}: {error}"))?;
    if value
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        != Some(MAP_SCHEMA_VERSION)
    {
        return Err(format!(
            "map schema_version must be {MAP_SCHEMA_VERSION}: {locale}"
        ));
    }
    serde_json::from_value(value).map_err(|error| format!("{locale}: {error}"))
}

