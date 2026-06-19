fn load_json(path: &Path) -> Result<Value, GuiError> {
    let bytes = fs::read(path)?;
    let text = String::from_utf8_lossy(&bytes);
    Ok(serde_json::from_str(&text)?)
}

fn load_localization(assets_root: &Path, locale: &str) -> Result<Localization, GuiError> {
    let mut localization = Localization::new();
    let mut fallbacks = vec![locale.to_string(), ASSET_FALLBACK_LOCALE.to_string()];
    fallbacks.dedup();
    fallbacks.reverse();
    for fallback in fallbacks {
        let path = assets_root
            .join("Localization")
            .join(fallback)
            .join("game.json");
        if !path.exists() {
            continue;
        }
        let Value::Object(loaded) = load_json(&path)? else {
            continue;
        };
        for (namespace, values) in loaded {
            if let Value::Object(values) = values {
                let target = localization.entry(namespace).or_default();
                for (key, value) in values {
                    if let Some(text) = value_to_text(&value) {
                        target.insert(key, text);
                    }
                }
            }
        }
    }
    Ok(localization)
}

fn rows_from_datatable(path: &Path) -> Result<JsonObject, GuiError> {
    match load_json(path)? {
        Value::Array(values) => values
            .first()
            .and_then(Value::as_object)
            .and_then(|object| object.get("Rows"))
            .and_then(Value::as_object)
            .cloned()
            .ok_or_else(|| invalid(format!("cannot locate Rows in {}", path.display()))),
        Value::Object(mut object) => {
            if let Some(Value::Object(rows)) = object.remove("Rows") {
                Ok(rows)
            } else {
                Ok(object)
            }
        }
        _ => Err(invalid(format!("cannot locate Rows in {}", path.display()))),
    }
}

fn rows_from_table(path: &Path) -> Result<JsonObject, GuiError> {
    rows_from_datatable(path).or_else(|_| Ok(JsonObject::new()))
}

fn namespace_from_table_id(table_id: Option<&Value>) -> Option<String> {
    let text = table_id.and_then(value_to_text)?;
    let tail = text.rsplit('/').next().unwrap_or(&text);
    if let Some((_, namespace)) = tail.rsplit_once('.') {
        Some(namespace.to_string()).filter(|value| !value.is_empty())
    } else {
        Some(tail.to_string()).filter(|value| !value.is_empty())
    }
}

fn localized_text(value: Option<&Value>, localization: &Localization) -> Option<String> {
    match value? {
        Value::String(text) => Some(text.clone()),
        Value::Object(text_ref) => {
            if let Some(text) = text_ref
                .get("CultureInvariantString")
                .and_then(value_to_text)
                .filter(|value| !value.is_empty())
            {
                return Some(text);
            }
            let key = text_ref.get("Key").and_then(value_to_text);
            let namespace = namespace_from_table_id(text_ref.get("TableId"));
            if let (Some(key), Some(namespace)) = (&key, namespace) {
                if let Some(text) = localized_key(localization, &namespace, key) {
                    return Some(text);
                }
            }
            if let Some(key) = key {
                if let Some(text) = localized_key(localization, "", &key)
                    .or_else(|| unique_localized_key(localization, &key))
                {
                    return Some(text);
                }
            }
            text_ref_fallback(text_ref)
        }
        _ => None,
    }
}

fn text_ref_fallback(text_ref: &JsonObject) -> Option<String> {
    ["SourceString", "LocalizedString"]
        .into_iter()
        .find_map(|field| text_ref.get(field).and_then(value_to_text))
        .filter(|value| !value.is_empty())
}

fn text_ref_key(text_ref: Option<&Value>) -> Option<String> {
    text_ref
        .and_then(Value::as_object)
        .and_then(|object| object.get("Key"))
        .and_then(value_to_text)
        .filter(|value| !value.is_empty())
}

fn localized_key(localization: &Localization, namespace: &str, key: &str) -> Option<String> {
    localization
        .get(namespace)
        .and_then(|values| values.get(key))
        .cloned()
}

fn unique_localized_key(localization: &Localization, key: &str) -> Option<String> {
    let hits = localization
        .values()
        .filter_map(|values| values.get(key).cloned())
        .collect::<Vec<_>>();
    let unique = hits.iter().collect::<BTreeSet<_>>();
    (unique.len() == 1).then(|| hits[0].clone())
}

fn clean_name(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

