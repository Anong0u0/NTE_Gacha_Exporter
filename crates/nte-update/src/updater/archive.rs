fn extract_zip_checked(archive_path: &Path, target: &Path) -> Result<(), GuiError> {
    let file = fs::File::open(archive_path)?;
    let mut zip = ZipArchive::new(file)?;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index)?;
        let Some(safe_name) = safe_zip_path(entry.name())? else {
            continue;
        };
        if safe_name.components().next().is_some_and(
            |component| matches!(component, Component::Normal(name) if name == DATA_DIR),
        ) && entry.is_file()
        {
            return Err(GuiError::InvalidUpdate(format!(
                "update package must not contain data files: {}",
                entry.name()
            )));
        }
        let output = target.join(safe_name);
        if entry.is_dir() {
            fs::create_dir_all(output)?;
        } else {
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut output_file = fs::File::create(output)?;
            std::io::copy(&mut entry, &mut output_file)?;
            output_file.flush()?;
        }
    }
    Ok(())
}

fn safe_zip_path(name: &str) -> Result<Option<PathBuf>, GuiError> {
    if name.trim().is_empty() {
        return Ok(None);
    }
    if name.contains('\\') || name.starts_with('/') || name.contains(':') {
        return Err(GuiError::InvalidUpdate(format!(
            "invalid update zip path: {name}"
        )));
    }
    let path = PathBuf::from(name);
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            _ => {
                return Err(GuiError::InvalidUpdate(format!(
                    "invalid update zip path: {name}"
                )));
            }
        }
    }
    Ok((!clean.as_os_str().is_empty()).then_some(clean))
}

fn normalized_payload_root(extract_root: &Path) -> Result<PathBuf, GuiError> {
    if validate_payload_layout(extract_root).is_ok() {
        return Ok(extract_root.to_path_buf());
    }
    let entries = fs::read_dir(extract_root)?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    if entries.len() == 1 && validate_payload_layout(&entries[0]).is_ok() {
        return Ok(entries[0].clone());
    }
    Err(GuiError::InvalidUpdate(
        "update package payload layout is invalid".to_string(),
    ))
}
