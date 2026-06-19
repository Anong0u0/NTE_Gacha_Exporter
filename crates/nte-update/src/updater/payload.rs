fn validate_payload_layout(payload: &Path) -> Result<(), GuiError> {
    for path in [
        payload.join(ROOT_LAUNCHER),
        payload.join(ROOT_CLI),
        payload.join(APP_DIR),
        payload.join(APP_DIR).join(APP_EXE),
        payload.join(APP_DIR).join(UPDATER_EXE),
        payload.join(APP_DIR).join(RELEASE_JSON),
    ] {
        if !path.exists() {
            return Err(GuiError::InvalidUpdate(format!(
                "update package missing required path: {}",
                path.to_string_lossy()
            )));
        }
    }
    validate_no_legacy_sidecars(payload)?;
    if payload.join(DATA_DIR).is_file() {
        return Err(GuiError::InvalidUpdate(
            "update package data path must not be a file".to_string(),
        ));
    }
    Ok(())
}

fn validate_no_legacy_sidecars(payload: &Path) -> Result<(), GuiError> {
    let sidecars = payload.join(LEGACY_SIDECAR_DIR);
    if sidecars.exists() {
        return Err(GuiError::InvalidUpdate(
            "update package must not contain legacy sidecars".to_string(),
        ));
    }
    Ok(())
}

fn validate_payload_release(payload: &Path, expected_version: &str) -> Result<(), GuiError> {
    let path = payload.join(APP_DIR).join(RELEASE_JSON);
    let text = fs::read_to_string(&path)?;
    let value: serde_json::Value = serde_json::from_str(&text)?;
    if value.get("schema").and_then(serde_json::Value::as_str) != Some(RELEASE_SCHEMA)
        || value
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            != Some(u64::from(RELEASE_SCHEMA_VERSION))
    {
        return Err(GuiError::InvalidUpdate(
            "update package release schema must be nte-gacha-exporter-release v1".to_string(),
        ));
    }
    let actual_version = value
        .get("version")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            GuiError::InvalidUpdate("update package release version must be a string".to_string())
        })?;
    if actual_version != expected_version {
        return Err(GuiError::InvalidUpdate(format!(
            "update package release version mismatch: expected {expected_version}, got {actual_version}"
        )));
    }
    Ok(())
}

fn validate_no_payload_data_files(payload: &Path) -> Result<(), GuiError> {
    let data = payload.join(DATA_DIR);
    if !data.exists() {
        return Ok(());
    }
    if data.is_file() {
        return Err(GuiError::InvalidUpdate(
            "update package data path must not be a file".to_string(),
        ));
    }
    if let Some(entry) = walk_files(&data)?.into_iter().next() {
        return Err(GuiError::InvalidUpdate(format!(
            "update package must not contain data files: {}",
            entry.to_string_lossy()
        )));
    }
    Ok(())
}

fn walk_files(path: &Path) -> Result<Vec<PathBuf>, GuiError> {
    let mut files = Vec::new();
    if !path.exists() {
        return Ok(files);
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            files.extend(walk_files(&entry.path())?);
        } else if file_type.is_file() {
            files.push(entry.path());
        }
    }
    Ok(files)
}
