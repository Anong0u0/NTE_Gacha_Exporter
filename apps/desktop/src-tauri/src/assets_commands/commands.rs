#[tauri::command]
pub(crate) fn assets_resolve_refs(
    state: State<'_, AppState>,
    refs: Vec<AssetResolveRequest>,
) -> Result<Vec<AssetResolveResult>, ApiError> {
    let root = root_from_state(&state)?;
    let Some(manifest) = bundled_manifest(&root) else {
        return Ok(refs
            .into_iter()
            .map(|item| AssetResolveResult {
                asset_ref: item.asset_ref,
                kind: item.kind,
                url: None,
            })
            .collect());
    };

    let mut exact = BTreeMap::new();
    let mut by_ref = BTreeMap::new();
    for asset in manifest.assets {
        let url = asset_url(&asset.pack_path);
        exact.insert((asset.asset_ref.clone(), asset.kind.clone()), url.clone());
        by_ref.entry(asset.asset_ref).or_insert(url);
    }

    Ok(refs
        .into_iter()
        .map(|item| {
            let url = item
                .kind
                .as_ref()
                .and_then(|kind| exact.get(&(item.asset_ref.clone(), kind.clone())).cloned())
                .or_else(|| by_ref.get(&item.asset_ref).cloned());
            AssetResolveResult {
                asset_ref: item.asset_ref,
                kind: item.kind,
                url,
            }
        })
        .collect())
}

pub(crate) fn assets_protocol_response(
    root: &Path,
    request: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    match resolve_protocol_path(root, request.uri().path()) {
        Ok(path) => match fs::read(path) {
            Ok(bytes) => response(200, "image/webp", bytes),
            Err(_) => response(
                404,
                "text/plain; charset=utf-8",
                b"asset not found".to_vec(),
            ),
        },
        Err(message) => response(400, "text/plain; charset=utf-8", message.into_bytes()),
    }
}
