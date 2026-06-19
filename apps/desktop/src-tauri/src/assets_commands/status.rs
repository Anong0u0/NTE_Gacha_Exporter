fn root_from_state(state: &State<'_, AppState>) -> Result<PathBuf, ApiError> {
    with_store(state, |store| Ok(store.root().to_path_buf()))
}

fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn update_channel_or_settings(
    state: &State<'_, AppState>,
    channel: Option<String>,
) -> Result<UpdateChannel, ApiError> {
    if let Some(channel) = channel.filter(|value| !value.trim().is_empty()) {
        return Ok(UpdateChannel::from_settings(&channel));
    }
    with_store(state, |store| {
        Ok(UpdateChannel::from_settings(
            &store.settings()?.update_channel,
        ))
    })
}

fn status_for_root(root: &Path) -> AssetsPackStatus {
    let manifest = installed_manifest(root).ok();
    let compatible = manifest.as_ref().is_some_and(manifest_matches_current);
    AssetsPackStatus {
        installed: manifest.is_some(),
        compatible,
        current_app_version: app_version().to_string(),
        expected_map_hash: bundled_maps_hash(),
        installed_app_version: manifest.as_ref().map(|item| item.app_version.clone()),
        installed_map_hash: manifest.as_ref().map(|item| item.map_hash.clone()),
        source_commit: manifest.as_ref().map(|item| item.source_commit.clone()),
        file_count: manifest.as_ref().map_or(0, |item| item.file_count),
        install_path: current_dir(root).display().to_string(),
    }
}

fn compatible_manifest(root: &Path) -> Option<AssetsPackManifest> {
    installed_manifest(root)
        .ok()
        .filter(manifest_matches_current)
}

fn installed_manifest(root: &Path) -> Result<AssetsPackManifest, ApiError> {
    let file = fs::File::open(current_dir(root).join("manifest.json")).map_err(api_error)?;
    let manifest: AssetsPackManifest = serde_json::from_reader(file).map_err(api_error)?;
    validate_manifest_shape(&manifest).map_err(api_error)?;
    Ok(manifest)
}

fn manifest_matches_current(manifest: &AssetsPackManifest) -> bool {
    manifest.app_version == app_version() && manifest.map_hash == bundled_maps_hash()
}

fn package_matches_current(package: &AssetsPackPackage) -> bool {
    package.app_version == app_version() && package.map_hash == bundled_maps_hash()
}
