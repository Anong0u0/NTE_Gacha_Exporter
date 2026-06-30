pub fn read_zip_manifest<R: Read + Seek>(
    zip: &mut zip::ZipArchive<R>,
) -> Result<AssetsPackManifest, GuiError> {
    let mut entry = zip
        .by_name("manifest.json")
        .map_err(|_| invalid_pack("assets pack missing manifest.json"))?;
    let mut text = String::new();
    entry.read_to_string(&mut text)?;
    let manifest: AssetsPackManifest = serde_json::from_str(&text)?;
    validate_manifest_shape(&manifest)?;
    Ok(manifest)
}

pub fn validate_manifest_shape(manifest: &AssetsPackManifest) -> Result<(), GuiError> {
    if manifest.schema != PACK_SCHEMA || manifest.schema_version != PACK_SCHEMA_VERSION {
        return Err(invalid_pack(
            "assets pack manifest schema must be nte-gacha-exporter-assets-pack v1",
        ));
    }
    if manifest.format != "webp" {
        return Err(invalid_pack("assets pack format must be webp"));
    }
    if manifest.file_count != manifest.assets.len() as u64 {
        return Err(invalid_pack("assets pack file_count mismatch"));
    }
    for asset in &manifest.assets {
        if asset.asset_ref.trim().is_empty()
            || asset.kind.trim().is_empty()
            || asset.source_path.trim().is_empty()
            || asset.pack_path.trim().is_empty()
        {
            return Err(invalid_pack("assets pack asset fields must be non-empty"));
        }
        if !asset.pack_path.starts_with("assets/")
            || !asset.pack_path.ends_with(".webp")
            || asset.pack_path.contains('\\')
            || asset.pack_path.contains("..")
        {
            return Err(invalid_pack(format!(
                "assets pack contains invalid asset path: {}",
                asset.pack_path
            )));
        }
    }
    Ok(())
}
