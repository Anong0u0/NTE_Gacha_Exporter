pub fn load_locale_or_settings(
    store: &JsonStore,
    locale: Option<String>,
) -> Result<String, GuiError> {
    match locale {
        Some(locale) if !locale.trim().is_empty() => Ok(locale),
        _ => Ok(store.settings()?.locale),
    }
}

fn validate_profile_name(name: &str) -> Result<String, GuiError> {
    let name = name.trim();
    if name.is_empty() || name.len() > 40 {
        return Err(GuiError::InvalidProfile(
            "profile name length must be 1..40".to_string(),
        ));
    }
    if !name
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(GuiError::InvalidProfile(
            "profile name must use ASCII letters, digits, _ or -".to_string(),
        ));
    }
    if is_reserved_windows_name(name) {
        return Err(GuiError::InvalidProfile(
            "profile name must not use a reserved Windows device name".to_string(),
        ));
    }
    Ok(name.to_string())
}

fn is_reserved_windows_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || upper
            .strip_prefix("COM")
            .and_then(|tail| tail.parse::<u8>().ok())
            .is_some_and(|value| (1..=9).contains(&value))
        || upper
            .strip_prefix("LPT")
            .and_then(|tail| tail.parse::<u8>().ok())
            .is_some_and(|value| (1..=9).contains(&value))
}

fn validate_locale(locale: &str) -> Result<(), GuiError> {
    load_map(locale).map(|_| ())
}

fn validate_update_channel(channel: &str) -> Result<String, GuiError> {
    let channel = channel.trim();
    if channel.is_empty() || channel.len() > 32 {
        return Err(GuiError::InvalidDocument(
            "update_channel length must be 1..32".to_string(),
        ));
    }
    if !channel
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(GuiError::InvalidDocument(
            "update_channel must use ASCII letters, digits, _ or -".to_string(),
        ));
    }
    Ok(channel.to_string())
}

fn validate_records_against_map(records: &[InternalRecord], map: &MapData) -> Result<(), GuiError> {
    for record in records {
        if !map.has_pool_id(&record.pool_id) {
            return Err(GuiError::UnknownPoolId(record.pool_id.clone()));
        }
    }
    Ok(())
}

fn normalize_records(records: &mut [InternalRecord]) {
    for record in records {
        if record
            .roll_points
            .is_some_and(|value| matches!(value, 0 | 4_294_967_295))
        {
            record.roll_points = None;
        }
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, GuiError> {
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn read_zip_json<T: for<'de> Deserialize<'de>>(
    zip: &mut ZipArchive<fs::File>,
    name: &str,
) -> Result<T, GuiError> {
    let mut entry = zip
        .by_name(name)
        .map_err(|_| GuiError::InvalidBackup(format!("backup missing required file: {name}")))?;
    if !entry.is_file() {
        return Err(GuiError::InvalidBackup(format!(
            "backup entry must be a file: {name}"
        )));
    }
    let mut text = String::new();
    entry.read_to_string(&mut text)?;
    Ok(serde_json::from_str(&text)?)
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), GuiError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = AtomicWriteFile::open(path)?;
    let bytes = serde_json::to_vec_pretty(value)?;
    file.write_all(&bytes)?;
    file.write_all(b"\n")?;
    file.commit()?;
    Ok(())
}

fn validate_backup_files(files: &HashSet<String>) -> Result<(), GuiError> {
    for name in files {
        if name.is_empty()
            || name == "."
            || name.contains('\\')
            || name.contains("..")
            || name.starts_with('/')
        {
            return Err(GuiError::InvalidBackup(format!(
                "invalid backup path: {name}"
            )));
        }
        if name != "settings.json"
            && !is_supported_profile_backup_path(name)
            && name != "manifest.json"
        {
            return Err(GuiError::InvalidBackup(format!(
                "unsupported backup path: {name}"
            )));
        }
    }
    Ok(())
}

fn backup_profile_names(files: &HashSet<String>) -> Result<Vec<String>, GuiError> {
    let mut names = Vec::new();
    for name in files {
        let Some(rest) = name.strip_prefix("profiles/") else {
            continue;
        };
        let mut parts = rest.split('/');
        let Some(profile_name) = parts.next() else {
            continue;
        };
        if profile_name.is_empty() {
            return Err(GuiError::InvalidBackup(format!(
                "invalid backup path: {name}"
            )));
        }
        validate_profile_name(profile_name)?;
        if parts.next().is_none() {
            return Err(GuiError::InvalidBackup(format!(
                "profile path missing filename: {name}"
            )));
        }
        if !names
            .iter()
            .any(|existing: &String| existing == profile_name)
        {
            names.push(profile_name.to_string());
        }
    }
    names.sort();
    Ok(names)
}

fn is_supported_profile_backup_path(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("profiles/") else {
        return false;
    };
    let mut parts = rest.split('/');
    let Some(profile_name) = parts.next() else {
        return false;
    };
    let Some(file_name) = parts.next() else {
        return false;
    };
    parts.next().is_none()
        && !profile_name.is_empty()
        && matches!(file_name, "profile.json" | "records.json" | "last-run.json")
}

fn remove_profile_dir_known_files(path: PathBuf) -> Result<(), GuiError> {
    for file_name in ["profile.json", "records.json", "last-run.json"] {
        let file = path.join(file_name);
        if file.exists() {
            fs::remove_file(file)?;
        }
    }
    let _ = fs::remove_dir(path);
    Ok(())
}

fn remove_profile_dir_known_files_strict(path: PathBuf) -> Result<(), GuiError> {
    for entry in fs::read_dir(&path)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if !entry.file_type()?.is_file()
            || !matches!(
                file_name.as_ref(),
                "profile.json" | "records.json" | "last-run.json"
            )
        {
            return Err(GuiError::InvalidProfile(format!(
                "profile directory contains unsupported path: {}",
                entry.path().display()
            )));
        }
    }
    for file_name in ["profile.json", "records.json", "last-run.json"] {
        let file = path.join(file_name);
        if file.exists() {
            fs::remove_file(file)?;
        }
    }
    fs::remove_dir(path)?;
    Ok(())
}

fn backup_entry_names(zip: &mut ZipArchive<fs::File>) -> Result<HashSet<String>, GuiError> {
    let mut names = HashSet::new();
    for index in 0..zip.len() {
        let entry = zip.by_index(index)?;
        if entry.is_file() {
            names.insert(entry.name().to_string());
        }
    }
    Ok(names)
}

fn sort_records(records: &mut [InternalRecord]) {
    records.sort_by(|left, right| {
        left.time
            .cmp(&right.time)
            .then_with(|| left.record_id.cmp(&right.record_id))
    });
}

fn now_stamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn now_unique_stamp() -> String {
    let sequence = UNIQUE_STAMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| format!("{}-{:09}-{sequence}", value.as_secs(), value.subsec_nanos()))
        .unwrap_or_else(|_| format!("0-000000000-{sequence}"))
}

fn default_update_channel() -> String {
    DEFAULT_UPDATE_CHANNEL.to_string()
}
