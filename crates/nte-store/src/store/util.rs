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

fn validate_ui_locale(locale: &str) -> Result<String, GuiError> {
    let locale = locale.trim();
    if locale.is_empty() {
        return Err(GuiError::LocaleNotFound(locale.to_string()));
    }
    if nte_core::is_ui_locale(locale) {
        return Ok(locale.to_string());
    }
    Err(GuiError::LocaleNotFound(locale.to_string()))
}

fn normalize_ui_locale_or_default(locale: &str, fallback: &str) -> Result<String, GuiError> {
    validate_ui_locale(locale).or_else(|_| validate_ui_locale(fallback))
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

fn validate_update_version(version: &str) -> Result<String, GuiError> {
    let version = version.trim();
    semver::Version::parse(version).map_err(|_| {
        GuiError::InvalidDocument(format!("invalid skipped_update_version: {version}"))
    })?;
    Ok(version.to_string())
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
    for record in records.iter_mut() {
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

impl JsonStore {
    pub fn cleanup_generated_backups_keep_latest(&self) -> Result<(), GuiError> {
        prune_generated_artifacts_keep_latest(&self.root.join("data/backups"), "backup-", ".zip")
    }

    pub fn cleanup_generated_raw_runs_keep_latest(&self) -> Result<(), GuiError> {
        prune_generated_artifacts_keep_latest(&self.root.join("data/runs"), "raw-", ".jsonl")
    }
}

fn prune_generated_artifacts_keep_latest(
    dir: &Path,
    prefix: &str,
    suffix: &str,
) -> Result<(), GuiError> {
    let metadata = match fs::symlink_metadata(dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() || has_reparse_point(&metadata) {
        return Ok(());
    }

    let mut candidates = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };
        if !is_generated_artifact_file_name(file_name, prefix, suffix) {
            continue;
        }

        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || has_reparse_point(&metadata)
        {
            continue;
        }
        if !is_direct_child_file_path(dir, &path, file_name) {
            continue;
        }
        candidates.push((file_name.to_string(), path));
    }

    candidates.sort_by(|left, right| right.0.cmp(&left.0));
    for (_, path) in candidates.into_iter().skip(1) {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn is_generated_artifact_file_name(name: &str, prefix: &str, suffix: &str) -> bool {
    let Some(stamp) = name
        .strip_prefix(prefix)
        .and_then(|value| value.strip_suffix(suffix))
    else {
        return false;
    };
    is_unique_stamp(stamp)
}

fn is_unique_stamp(stamp: &str) -> bool {
    let mut parts = stamp.split('-');
    let Some(seconds) = parts.next() else {
        return false;
    };
    let Some(nanos) = parts.next() else {
        return false;
    };
    let Some(sequence) = parts.next() else {
        return false;
    };
    parts.next().is_none()
        && !seconds.is_empty()
        && !sequence.is_empty()
        && nanos.len() == 9
        && seconds.bytes().all(|byte| byte.is_ascii_digit())
        && nanos.bytes().all(|byte| byte.is_ascii_digit())
        && sequence.bytes().all(|byte| byte.is_ascii_digit())
}

fn is_direct_child_file_path(parent: &Path, path: &Path, file_name: &str) -> bool {
    if file_name.is_empty() || file_name == "." || file_name == ".." {
        return false;
    }
    if file_name.contains(['/', '\\', ':']) {
        return false;
    }
    let mut components = Path::new(file_name).components();
    if !matches!(components.next(), Some(std::path::Component::Normal(_)))
        || components.next().is_some()
    {
        return false;
    }
    path.parent().is_some_and(|value| value == parent)
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
    records.sort_by(compare_records_chronological);
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

#[cfg(windows)]
fn has_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn has_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

fn default_update_channel() -> String {
    DEFAULT_UPDATE_CHANNEL.to_string()
}

fn default_check_updates_on_startup() -> bool {
    DEFAULT_CHECK_UPDATES_ON_STARTUP
}

fn default_capture_auto_page_enabled() -> bool {
    true
}

fn default_locale() -> String {
    DEFAULT_LOCALE.to_string()
}

fn default_ui_locale() -> String {
    String::new()
}
