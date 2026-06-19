#[tauri::command]
pub(crate) fn capture_start(
    state: State<'_, AppState>,
    profile_name: String,
    locale: Option<String>,
    mode: Option<CaptureMode>,
) -> Result<CaptureStatus, ApiError> {
    let mode = mode.unwrap_or(CaptureMode::LiveOnly);
    if admin_relaunch_required()? {
        return Err(api_error_message(
            "admin_required",
            "pktmon capture requires administrator permission",
        ));
    }
    let locale = with_store(&state, |store| {
        let locale = load_locale_or_settings(store, locale)?;
        store.dashboard_overview(&profile_name, &locale)?;
        Ok(locale)
    })?;
    let output_raw = with_store(&state, |store| {
        let raw_path = if mode.auto_page() {
            Some(store.default_run_raw_path().to_string_lossy().to_string())
        } else {
            None
        };
        Ok(raw_path)
    })?;
    let known_record_ids = if mode == CaptureMode::AutoPageIncremental {
        with_store(&state, |store| store.profile_record_ids(&profile_name))?
    } else {
        Vec::new()
    };
    let mut status =
        start_rust_capture_session(&state, &locale, mode, output_raw.clone(), known_record_ids)?;
    status.import_report = None;
    let source_path = status.raw_path.clone().or(output_raw);
    state
        .captures
        .lock()
        .map_err(|_| api_error_message("capture_lock_poisoned", "capture lock poisoned"))?
        .insert(
            status.session_id.clone(),
            CaptureSessionMeta {
                profile_name,
                source_kind: mode.source_kind().to_string(),
                source_path,
                full_update: mode.full_update(),
                import_report: None,
            },
        );
    if status.state == "completed" {
        return capture_status_with_merge(&state, &status.session_id);
    }
    Ok(status)
}

#[tauri::command]
pub(crate) fn capture_status(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<CaptureStatus, ApiError> {
    capture_status_with_merge(&state, &session_id)
}

#[tauri::command]
pub(crate) fn capture_stop(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<CaptureStatus, ApiError> {
    let session = capture_runtime_session(&state, &session_id)?;
    session.stop.store(true, Ordering::SeqCst);
    {
        let mut status = session
            .status
            .lock()
            .map_err(|_| api_error_message("capture_lock_poisoned", "capture lock poisoned"))?;
        if matches!(status.state.as_str(), "starting" | "running") {
            status.state = "stopping".to_string();
            status.updated_at = now_seconds();
        }
    }
    capture_status_with_merge(&state, &session_id)
}

