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

#[derive(Clone, Copy)]
enum UpdateBucket {
    Downloads,
    Staging,
    Rollback,
}

impl UpdateBucket {
    fn name(self) -> &'static str {
        match self {
            Self::Downloads => "downloads",
            Self::Staging => "staging",
            Self::Rollback => "rollback",
        }
    }
}

fn bucket_dir(root: &Path, bucket: UpdateBucket) -> PathBuf {
    root.join(UPDATE_DIR).join(bucket.name())
}

fn scoped_bucket_child(root: &Path, bucket: UpdateBucket, child: &str) -> Result<PathBuf, GuiError> {
    let child = validate_direct_child_name(child)?;
    Ok(bucket_dir(root, bucket).join(child))
}

fn clear_staging_child(root: &Path, child: &str) -> Result<PathBuf, GuiError> {
    let path = scoped_bucket_child(root, UpdateBucket::Staging, child)?;
    if path.exists() {
        remove_update_artifact_dir(&path)?;
    }
    fs::create_dir_all(&path)?;
    Ok(path)
}

fn cleanup_update_artifacts(root: impl AsRef<Path>) -> Result<(), GuiError> {
    let root = validate_cleanup_root(root.as_ref())?;
    for bucket in [
        UpdateBucket::Downloads,
        UpdateBucket::Staging,
        UpdateBucket::Rollback,
    ] {
        cleanup_bucket(&root, bucket)?;
    }
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

fn cleanup_bucket(root: &Path, bucket: UpdateBucket) -> Result<(), GuiError> {
    let bucket_path = bucket_dir(root, bucket);
    if !bucket_path.exists() {
        return Ok(());
    }
    validate_update_artifact_dir(&bucket_path)?;
    for entry in fs::read_dir(&bucket_path)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink() || has_reparse_point(&metadata) {
            return Err(GuiError::InvalidUpdate(format!(
                "refusing to delete unsafe update artifact path: {}",
                entry.path().to_string_lossy()
            )));
        }
        if !metadata.is_dir() {
            continue;
        }
        let file_name = entry.file_name();
        let child = file_name.to_string_lossy();
        let target = scoped_bucket_child(root, bucket, child.as_ref())?;
        if let Err(error) = remove_update_artifact_dir(&target) {
            if is_transient_cleanup_error(&error) {
                continue;
            }
            return Err(error);
        }
    }
    Ok(())
}

fn validate_cleanup_root(root: &Path) -> Result<PathBuf, GuiError> {
    if !is_supported_portable_layout(root) {
        return Err(GuiError::InvalidUpdate(format!(
            "refusing to cleanup unsupported portable root: {}",
            root.to_string_lossy()
        )));
    }
    validate_update_artifact_dir(root)?;
    Ok(fs::canonicalize(root)?)
}

fn validate_direct_child_name(value: &str) -> Result<&str, GuiError> {
    let value = value.trim();
    if value.is_empty() || value == "." || value == ".." || value.contains(['/', '\\', ':']) {
        return Err(GuiError::InvalidUpdate(format!(
            "invalid update artifact name: {value}"
        )));
    }
    let path = Path::new(value);
    if path.components().count() != 1
        || !matches!(path.components().next(), Some(Component::Normal(_)))
    {
        return Err(GuiError::InvalidUpdate(format!(
            "invalid update artifact name: {value}"
        )));
    }
    Ok(value)
}

fn validate_update_artifact_dir(path: &Path) -> Result<(), GuiError> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() || has_reparse_point(&metadata) {
        return Err(GuiError::InvalidUpdate(format!(
            "refusing to delete unsafe update artifact path: {}",
            path.to_string_lossy()
        )));
    }
    Ok(())
}

fn remove_update_artifact_dir(path: &Path) -> Result<(), GuiError> {
    validate_update_artifact_dir(path)?;
    remove_update_artifact_children(path)?;
    fs::remove_dir(path)?;
    Ok(())
}

fn remove_update_artifact_children(path: &Path) -> Result<(), GuiError> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let child = entry.path();
        let metadata = fs::symlink_metadata(&child)?;
        if metadata.file_type().is_symlink() || has_reparse_point(&metadata) {
            return Err(GuiError::InvalidUpdate(format!(
                "refusing to delete unsafe update artifact path: {}",
                child.to_string_lossy()
            )));
        }
        if metadata.is_dir() {
            remove_update_artifact_children(&child)?;
            fs::remove_dir(&child)?;
        } else if metadata.is_file() {
            fs::remove_file(&child)?;
        } else {
            return Err(GuiError::InvalidUpdate(format!(
                "refusing to delete unsupported update artifact path: {}",
                child.to_string_lossy()
            )));
        }
    }
    Ok(())
}

fn is_transient_cleanup_error(error: &GuiError) -> bool {
    match error {
        GuiError::Io(error) => matches!(
            error.kind(),
            std::io::ErrorKind::PermissionDenied
                | std::io::ErrorKind::NotFound
                | std::io::ErrorKind::DirectoryNotEmpty
        ),
        _ => false,
    }
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
