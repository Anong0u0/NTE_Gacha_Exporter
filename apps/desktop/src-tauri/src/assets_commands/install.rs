fn install_assets_pack(
    root: &Path,
    package: &AssetsPackPackage,
    archive_path: &Path,
) -> Result<AssetsPackInstallReport, ApiError> {
    let actual_hash = sha256_file(archive_path).map_err(api_error)?;
    if !actual_hash.eq_ignore_ascii_case(&package.sha256) {
        return Err(api_error_message(
            "assets_pack_sha256_mismatch",
            format!(
                "assets pack sha256 mismatch: expected {}, got {actual_hash}",
                package.sha256
            ),
        ));
    }

    let file = fs::File::open(archive_path).map_err(api_error)?;
    let mut zip = zip::ZipArchive::new(file).map_err(api_error)?;
    let manifest = read_zip_manifest(&mut zip).map_err(api_error)?;
    validate_pack_manifest(&manifest, package)?;

    let expected = manifest
        .assets
        .iter()
        .map(|asset| (asset.pack_path.clone(), asset.sha256.clone()))
        .collect::<BTreeMap<_, _>>();
    validate_zip_entries(&mut zip, &expected)?;

    let pack_root = assets_pack_root(root);
    fs::create_dir_all(&pack_root).map_err(api_error)?;
    let staging = pack_root
        .join("staging")
        .join(timestamp_millis().to_string());
    fs::create_dir_all(staging.join("assets")).map_err(api_error)?;
    fs::write(
        staging.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).map_err(api_error)?,
    )
    .map_err(api_error)?;

    for (pack_path, expected_hash) in &expected {
        let bytes = read_zip_entry_limited(&mut zip, pack_path, package.size)?;
        let actual_hash = sha256_bytes(&bytes);
        if !actual_hash.eq_ignore_ascii_case(expected_hash) {
            return Err(api_error_message(
                "assets_pack_asset_sha256_mismatch",
                format!("assets pack asset sha256 mismatch: {pack_path}"),
            ));
        }
        let output = staging.join(pack_path);
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(api_error)?;
        }
        fs::write(output, bytes).map_err(api_error)?;
    }

    let current = current_dir(root);
    if current.exists() {
        let previous = pack_root.join(format!("previous-{}", timestamp_millis()));
        fs::rename(&current, previous).map_err(api_error)?;
    }
    fs::rename(&staging, &current).map_err(api_error)?;

    Ok(AssetsPackInstallReport {
        app_version: manifest.app_version,
        map_hash: manifest.map_hash,
        source_commit: manifest.source_commit,
        file_count: manifest.file_count,
        install_path: current.display().to_string(),
    })
}

fn validate_pack_manifest(
    manifest: &AssetsPackManifest,
    package: &AssetsPackPackage,
) -> Result<(), ApiError> {
    if !manifest_matches_current(manifest) {
        return Err(api_error_message(
            "assets_pack_incompatible",
            "assets pack manifest does not match current app version and bundled maps",
        ));
    }
    if manifest.file_count != package.file_count
        || manifest.source_commit != package.source_commit
        || manifest.map_hash != package.map_hash
        || manifest.app_version != package.app_version
    {
        return Err(api_error_message(
            "assets_pack_manifest_mismatch",
            "assets pack manifest does not match release metadata",
        ));
    }
    Ok(())
}

fn validate_zip_entries(
    zip: &mut zip::ZipArchive<fs::File>,
    expected: &BTreeMap<String, String>,
) -> Result<(), ApiError> {
    let mut seen = BTreeSet::new();
    for index in 0..zip.len() {
        let entry = zip.by_index(index).map_err(api_error)?;
        let name = entry.name().to_string();
        if name.ends_with('/') {
            continue;
        }
        if name != "manifest.json" && !expected.contains_key(&name) {
            return Err(api_error_message(
                "assets_pack_unexpected_entry",
                format!("assets pack contains unexpected entry: {name}"),
            ));
        }
        seen.insert(name);
    }
    for pack_path in expected.keys() {
        if !seen.contains(pack_path) {
            return Err(api_error_message(
                "assets_pack_missing_entry",
                format!("assets pack missing entry: {pack_path}"),
            ));
        }
    }
    Ok(())
}

fn read_zip_entry_limited(
    zip: &mut zip::ZipArchive<fs::File>,
    pack_path: &str,
    max_bytes: u64,
) -> Result<Vec<u8>, ApiError> {
    let mut entry = zip.by_name(pack_path).map_err(api_error)?;
    let mut bytes = Vec::new();
    copy_limited(&mut entry, &mut bytes, max_bytes)?;
    Ok(bytes)
}
