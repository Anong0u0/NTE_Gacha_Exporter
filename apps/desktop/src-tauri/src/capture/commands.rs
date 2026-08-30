#[tauri::command]
pub(crate) fn capture_start(
    app: AppHandle<Wry>,
    state: State<'_, AppState>,
    profile_name: String,
    locale: Option<String>,
    mode: Option<CaptureMode>,
    options: Option<CaptureStartOptions>,
) -> Result<CaptureStatus, ApiError> {
    let mode = mode.unwrap_or(CaptureMode::LiveOnly);
    if admin_relaunch_required()? {
        return Err(api_error_message(
            "admin_required",
            "capture requires administrator permission",
        ));
    }
    let locale = with_store(&state, |store| {
        let locale = load_locale_or_settings(store, locale)?;
        store.dashboard_overview(&profile_name, &locale)?;
        Ok(locale)
    })?;
    let output_raw = with_store(&state, |store| Ok(Some(capture_raw_output_path(store))))?;
    let mut status = start_rust_capture_session(
        app,
        &state,
        &profile_name,
        &locale,
        mode,
        output_raw.clone(),
        options.unwrap_or_default(),
    )?;
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
    if capture_state_can_merge(&status.state) {
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
    request_capture_stop(&session);
    capture_status_with_merge(&state, &session_id)
}

fn request_capture_stop(session: &CaptureRuntimeSession) {
    session.stop.store(true, Ordering::SeqCst);
    let guard = session
        .attempt_stop
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(stop) = guard.as_ref() {
        stop.store(true, Ordering::SeqCst);
    }
    drop(guard);
    {
        let mut status = session
            .status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if crate::lifecycle::is_active_state(&status.state) {
            crate::lifecycle::set_stopping(&mut status.state);
            status.updated_at = now_seconds();
        }
    }
}

pub(crate) fn request_capture_shutdown(state: &AppState) -> Vec<Arc<CaptureRuntimeSession>> {
    let sessions = state
        .capture_sessions
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .values()
        .cloned()
        .collect::<Vec<_>>();
    for session in &sessions {
        request_capture_stop(session);
    }
    sessions
}

pub(crate) fn join_capture_shutdown(sessions: &[Arc<CaptureRuntimeSession>]) {
    for session in sessions {
        crate::lifecycle::join_thread(&session.handle);
    }
}

fn capture_raw_output_path(store: &nte_store::JsonStore) -> String {
    store.default_run_raw_path().to_string_lossy().to_string()
}
