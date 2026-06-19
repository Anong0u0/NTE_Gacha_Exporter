#[tauri::command]
pub(crate) fn assets_pack_status(state: State<'_, AppState>) -> Result<AssetsPackStatus, ApiError> {
    let root = root_from_state(&state)?;
    Ok(status_for_root(&root))
}

#[tauri::command]
pub(crate) fn assets_pack_check(
    state: State<'_, AppState>,
    channel: Option<String>,
) -> Result<AssetsPackCheckReport, ApiError> {
    let root = root_from_state(&state)?;
    let status = status_for_root(&root);
    let requested_channel = update_channel_or_settings(&state, channel)?;
    let package = match fetch_assets_pack_package(requested_channel) {
        Ok(package) if package_matches_current(&package) => Some(package),
        Ok(_) => None,
        Err(error) => return Err(error),
    };
    Ok(AssetsPackCheckReport {
        current_app_version: app_version().to_string(),
        expected_map_hash: bundled_maps_hash(),
        channel: requested_channel,
        installed: status.installed,
        compatible: status.compatible,
        package,
    })
}

#[tauri::command]
pub(crate) fn assets_pack_download_and_install(
    state: State<'_, AppState>,
    package: AssetsPackPackage,
) -> Result<AssetsPackInstallReport, ApiError> {
    if !package_matches_current(&package) {
        return Err(api_error_message(
            "assets_pack_incompatible",
            "assets pack does not match current app version and bundled maps",
        ));
    }
    let root = root_from_state(&state)?;
    let archive_path = download_assets_pack(&root, &package)?;
    install_assets_pack(&root, &package, &archive_path)
}

#[tauri::command]
pub(crate) fn assets_pack_remove(state: State<'_, AppState>) -> Result<AssetsPackStatus, ApiError> {
    let root = root_from_state(&state)?;
    let current = current_dir(&root);
    if current.exists() {
        let disabled = assets_pack_root(&root).join(format!("disabled-{}", timestamp_millis()));
        fs::rename(&current, disabled).map_err(api_error)?;
    }
    Ok(status_for_root(&root))
}

#[tauri::command]
pub(crate) fn assets_resolve_refs(
    state: State<'_, AppState>,
    refs: Vec<AssetResolveRequest>,
) -> Result<Vec<AssetResolveResult>, ApiError> {
    let root = root_from_state(&state)?;
    let Some(manifest) = compatible_manifest(&root) else {
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
