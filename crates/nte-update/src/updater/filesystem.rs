fn is_supported_portable_layout(root: &Path) -> bool {
    root.join(ROOT_LAUNCHER).is_file() && root.join(APP_DIR).join(APP_EXE).is_file()
}

fn latest_child_name(path: &Path) -> Result<Option<String>, GuiError> {
    if !path.exists() {
        return Ok(None);
    }
    let mut names = fs::read_dir(path)?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    names.sort();
    Ok(names.pop())
}

fn clear_scoped_dir(path: &Path) -> Result<(), GuiError> {
    if path.exists() {
        if !path
            .components()
            .any(|component| matches!(component, Component::Normal(part) if part == "staging"))
        {
            return Err(GuiError::InvalidUpdate(format!(
                "refusing to clear non-staging path: {}",
                path.to_string_lossy()
            )));
        }
        fs::remove_dir_all(path)?;
    }
    fs::create_dir_all(path)?;
    Ok(())
}

fn validate_existing_data_preserved(root: &Path) -> Result<(), GuiError> {
    if root.join(DATA_DIR).is_file() {
        return Err(GuiError::InvalidUpdate(
            "portable data path must not be a file".to_string(),
        ));
    }
    Ok(())
}

fn backup_current_release(root: &Path, rollback: &Path) -> Result<(), GuiError> {
    for name in [ROOT_LAUNCHER, ROOT_CLI, APP_DIR, LEGACY_SIDECAR_DIR] {
        let source = root.join(name);
        if source.exists() {
            fs::rename(source, rollback.join(name))?;
        }
    }
    Ok(())
}

fn install_payload(root: &Path, payload: &Path) -> Result<(), GuiError> {
    for name in [ROOT_LAUNCHER, ROOT_CLI, APP_DIR] {
        fs::rename(payload.join(name), root.join(name))?;
    }
    Ok(())
}

fn restore_rollback(root: &Path, rollback: &Path) -> Result<(), GuiError> {
    for name in [ROOT_LAUNCHER, ROOT_CLI, APP_DIR, LEGACY_SIDECAR_DIR] {
        let target = root.join(name);
        if target.exists() {
            remove_path(&target)?;
        }
        let source = rollback.join(name);
        if source.exists() {
            fs::rename(source, target)?;
        }
    }
    Ok(())
}

fn remove_path(path: &Path) -> Result<(), GuiError> {
    if path.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn current_release_label(root: &Path) -> String {
    let path = root.join(APP_DIR).join(RELEASE_JSON);
    let Ok(text) = fs::read_to_string(path) else {
        return "unknown".to_string();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return "unknown".to_string();
    };
    value
        .get("version")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("unknown")
        .to_string()
}

fn now_unique_stamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| format!("{}-{:09}", value.as_secs(), value.subsec_nanos()))
        .unwrap_or_else(|_| "0-000000000".to_string())
}
