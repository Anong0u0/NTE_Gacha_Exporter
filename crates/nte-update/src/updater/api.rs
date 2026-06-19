pub fn check_update_manifest(
    manifest: UpdateManifest,
    current_version: &str,
    requested_channel: UpdateChannel,
) -> Result<UpdateCheckReport, GuiError> {
    validate_manifest(&manifest)?;
    let current = parse_version(current_version)?;
    let next = parse_version(&manifest.version)?;
    let available = is_manifest_allowed(&manifest, requested_channel) && next > current;
    Ok(UpdateCheckReport {
        current_version: current_version.to_string(),
        channel: requested_channel,
        available,
        package: available.then(|| package_from_manifest(manifest)),
    })
}

pub fn update_status(
    root: impl AsRef<Path>,
    current_version: &str,
) -> Result<UpdateStatus, GuiError> {
    let root = root.as_ref();
    Ok(UpdateStatus {
        portable_root: root.to_string_lossy().to_string(),
        current_version: current_version.to_string(),
        supported_layout: is_supported_portable_layout(root),
        staged_version: latest_child_name(&root.join(UPDATE_DIR).join("staging"))?,
        rollback_version: latest_child_name(&root.join(UPDATE_DIR).join("rollback"))?,
    })
}

pub fn stage_update_archive(
    root: impl AsRef<Path>,
    package: &UpdatePackage,
    archive_path: impl AsRef<Path>,
) -> Result<UpdateStageReport, GuiError> {
    validate_package(package)?;
    let root = root.as_ref();
    let archive_path = archive_path.as_ref();
    if !archive_path.is_file() {
        return Err(GuiError::InvalidUpdate(format!(
            "update archive not found: {}",
            archive_path.to_string_lossy()
        )));
    }
    let actual_size = fs::metadata(archive_path)?.len();
    if actual_size != package.size {
        return Err(GuiError::InvalidUpdate(format!(
            "update archive size mismatch: expected {}, got {}",
            package.size, actual_size
        )));
    }
    let actual_hash = sha256_file(archive_path)?;
    if !actual_hash.eq_ignore_ascii_case(&package.sha256) {
        return Err(GuiError::InvalidUpdate(format!(
            "update archive sha256 mismatch: expected {}, got {}",
            package.sha256, actual_hash
        )));
    }

    let staging = root.join(UPDATE_DIR).join("staging").join(&package.version);
    clear_scoped_dir(&staging)?;
    let extract_root = staging.join("extract");
    let payload = staging.join("payload");
    fs::create_dir_all(&extract_root)?;
    extract_zip_checked(archive_path, &extract_root)?;
    let source_root = normalized_payload_root(&extract_root)?;
    validate_payload_layout(&source_root)?;
    validate_payload_release(&source_root, &package.version)?;
    validate_no_payload_data_files(&source_root)?;
    fs::rename(&source_root, &payload)?;

    let staged_helper = staging.join(UPDATER_EXE);
    fs::copy(payload.join(APP_DIR).join(UPDATER_EXE), staged_helper)?;
    let _ = fs::remove_dir(&extract_root);

    Ok(UpdateStageReport {
        package: package.clone(),
        archive_path: archive_path.to_string_lossy().to_string(),
        staging_path: staging.to_string_lossy().to_string(),
    })
}

pub fn prepare_update_install(
    root: impl AsRef<Path>,
    version: &str,
) -> Result<UpdateInstallPlan, GuiError> {
    let root = root.as_ref();
    let version = validate_version_string(version)?;
    let staging = root.join(UPDATE_DIR).join("staging").join(&version);
    let payload = staging.join("payload");
    validate_payload_layout(&payload)?;
    validate_payload_release(&payload, &version)?;
    let helper = staging.join(UPDATER_EXE);
    if !helper.is_file() {
        return Err(GuiError::InvalidUpdate(format!(
            "staged updater helper missing: {}",
            helper.to_string_lossy()
        )));
    }
    Ok(UpdateInstallPlan {
        root: root.to_string_lossy().to_string(),
        version,
        staging_path: staging.to_string_lossy().to_string(),
        helper_path: helper.to_string_lossy().to_string(),
    })
}

pub fn apply_staged_update(root: impl AsRef<Path>, version: &str) -> Result<(), GuiError> {
    let root = root.as_ref();
    let version = validate_version_string(version)?;
    let staging = root.join(UPDATE_DIR).join("staging").join(&version);
    let payload = staging.join("payload");
    validate_payload_layout(&payload)?;
    validate_payload_release(&payload, &version)?;
    validate_existing_data_preserved(root)?;

    let rollback = root.join(UPDATE_DIR).join("rollback").join(format!(
        "{}-{}",
        current_release_label(root),
        now_unique_stamp()
    ));
    fs::create_dir_all(&rollback)?;

    let apply_result =
        backup_current_release(root, &rollback).and_then(|()| install_payload(root, &payload));
    match apply_result {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = restore_rollback(root, &rollback);
            Err(error)
        }
    }
}
