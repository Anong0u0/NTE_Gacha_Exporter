fn root_from_state(state: &State<'_, AppState>) -> Result<PathBuf, ApiError> {
    with_store(state, |store| Ok(store.root().to_path_buf()))
}

fn bundled_manifest(root: &Path) -> Option<AssetsPackManifest> {
    read_bundled_manifest(root).ok()
}

fn read_bundled_manifest(root: &Path) -> Result<AssetsPackManifest, ApiError> {
    let file = fs::File::open(current_dir(root).join("manifest.json")).map_err(api_error)?;
    let manifest: AssetsPackManifest = serde_json::from_reader(file).map_err(api_error)?;
    Ok(manifest)
}
