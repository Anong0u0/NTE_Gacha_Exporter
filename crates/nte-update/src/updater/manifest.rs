fn validate_manifest(manifest: &UpdateManifest) -> Result<(), GuiError> {
    if manifest.schema != MANIFEST_SCHEMA || manifest.schema_version != MANIFEST_SCHEMA_VERSION {
        return Err(GuiError::InvalidUpdate(
            "update manifest schema must be nte-gacha-exporter-update v1".to_string(),
        ));
    }
    validate_package(&package_from_manifest(manifest.clone()))?;
    if manifest.release_url.trim().is_empty() || manifest.download_url.trim().is_empty() {
        return Err(GuiError::InvalidUpdate(
            "update manifest URLs must be non-empty".to_string(),
        ));
    }
    Ok(())
}

fn validate_package(package: &UpdatePackage) -> Result<(), GuiError> {
    validate_version_string(&package.version)?;
    if package.asset_name.trim().is_empty() {
        return Err(GuiError::InvalidUpdate(
            "update asset_name must be non-empty".to_string(),
        ));
    }
    if package.size == 0 {
        return Err(GuiError::InvalidUpdate(
            "update archive size must be greater than zero".to_string(),
        ));
    }
    if package.sha256.len() != 64 || !package.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(GuiError::InvalidUpdate(
            "update sha256 must be 64 hex characters".to_string(),
        ));
    }
    Ok(())
}

fn package_from_manifest(manifest: UpdateManifest) -> UpdatePackage {
    UpdatePackage {
        version: manifest.version,
        channel: manifest.channel,
        release_url: manifest.release_url,
        asset_name: manifest.asset_name,
        download_url: manifest.download_url,
        sha256: manifest.sha256,
        size: manifest.size,
    }
}

fn is_manifest_allowed(manifest: &UpdateManifest, requested_channel: UpdateChannel) -> bool {
    match requested_channel {
        UpdateChannel::Stable => {
            manifest.channel == UpdateChannel::Stable
                && parse_version(&manifest.version).is_ok_and(|version| version.pre.is_empty())
        }
        UpdateChannel::Beta => true,
    }
}

fn parse_version(value: &str) -> Result<Version, GuiError> {
    Version::parse(value)
        .map_err(|_| GuiError::InvalidUpdate(format!("invalid update version: {value}")))
}

fn validate_version_string(value: &str) -> Result<String, GuiError> {
    let value = value.trim();
    parse_version(value)?;
    Ok(value.to_string())
}

fn sha256_file(path: &Path) -> Result<String, GuiError> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
