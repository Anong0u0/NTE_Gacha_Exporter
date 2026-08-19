pub fn read_zip_manifest<R: Read + Seek>(
    zip: &mut zip::ZipArchive<R>,
) -> Result<AssetsPackManifest, GuiError> {
    let mut entry = zip
        .by_name("manifest.json")
        .map_err(|_| invalid_pack("assets pack missing manifest.json"))?;
    let mut text = String::new();
    entry.read_to_string(&mut text)?;
    let manifest: AssetsPackManifest = serde_json::from_str(&text)?;
    validate_assets_pack_manifest(&manifest)?;
    Ok(manifest)
}
