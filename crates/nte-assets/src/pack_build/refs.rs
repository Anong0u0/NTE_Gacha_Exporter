pub fn normalize_asset_ref(asset_ref: &str) -> Option<String> {
    let trimmed = asset_ref.trim();
    let rel = trimmed
        .strip_prefix("/Game/UI/UI_Icon/")
        .map(|rest| format!("UI_Icon/{rest}"))
        .or_else(|| {
            trimmed
                .strip_prefix("/Game/UI/UI/")
                .map(|rest| format!("UI/{rest}"))
        })?;
    let (prefix, leaf) = rel
        .rsplit_once('/')
        .map_or(("", rel.as_str()), |(prefix, leaf)| (prefix, leaf));
    let stem = leaf.split('.').next().filter(|value| !value.is_empty())?;
    let path = if prefix.is_empty() {
        format!("{stem}.png")
    } else {
        format!("{prefix}/{stem}.png")
    };
    (!path.contains('\\') && !path.contains("..")).then_some(path)
}

fn collect_asset_ref_uses(maps_dir: &Path) -> Result<Vec<AssetRefUse>, GuiError> {
    let mut refs: BTreeMap<(String, String), AssetRefUse> = BTreeMap::new();
    for entry in fs::read_dir(maps_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file()
            || entry.path().extension().and_then(|value| value.to_str()) != Some("json")
        {
            continue;
        }
        let text = fs::read_to_string(entry.path())?;
        let value: Value = serde_json::from_str(&text)?;
        collect_asset_refs_from_value(&value, &mut refs);
    }
    let max_edges_by_ref = refs
        .values()
        .map(|asset| (asset.asset_ref.clone(), asset.max_edge))
        .fold(
            BTreeMap::<String, u32>::new(),
            |mut max_edges, (asset_ref, max_edge)| {
                max_edges
                    .entry(asset_ref)
                    .and_modify(|current| *current = (*current).max(max_edge))
                    .or_insert(max_edge);
                max_edges
            },
        );
    Ok(refs
        .into_values()
        .map(|mut asset| {
            if let Some(max_edge) = max_edges_by_ref.get(&asset.asset_ref) {
                asset.max_edge = *max_edge;
            }
            asset
        })
        .collect())
}

fn collect_asset_refs_from_value(
    value: &Value,
    refs: &mut BTreeMap<(String, String), AssetRefUse>,
) {
    match value {
        Value::Object(object) => {
            if let Some(asset_refs) = object.get("asset_refs") {
                collect_asset_refs_from_asset_refs(asset_refs, None, refs);
            }
            for (key, value) in object {
                if key != "asset_refs" {
                    collect_asset_refs_from_value(value, refs);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_asset_refs_from_value(value, refs);
            }
        }
        _ => {}
    }
}

fn collect_asset_refs_from_asset_refs(
    value: &Value,
    current_key: Option<&str>,
    refs: &mut BTreeMap<(String, String), AssetRefUse>,
) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                collect_asset_refs_from_asset_refs(value, Some(key), refs);
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_asset_refs_from_asset_refs(value, current_key, refs);
            }
        }
        Value::String(asset_ref) => {
            let Some(source_path) = normalize_asset_ref(asset_ref) else {
                return;
            };
            let kind = current_key.unwrap_or("asset").to_string();
            let max_edge = max_edge_for_kind(&kind);
            refs.entry((asset_ref.clone(), kind.clone()))
                .or_insert_with(|| AssetRefUse {
                    asset_ref: asset_ref.clone(),
                    kind,
                    source_path,
                    max_edge,
                });
        }
        _ => {}
    }
}

fn max_edge_for_kind(kind: &str) -> u32 {
    match kind {
        "icon" => 256,
        "head_icon" => 128,
        "portrait" | "featured_portraits" => 512,
        "image" | "background" | "banner" => 768,
        _ => 512,
    }
}
