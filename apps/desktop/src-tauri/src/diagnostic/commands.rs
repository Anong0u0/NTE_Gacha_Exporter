#[tauri::command]
pub(crate) fn diagnostic_start(
    state: State<'_, AppState>,
    duration_seconds: Option<u64>,
) -> Result<DiagnosticStatus, ApiError> {
    if admin_relaunch_required()? {
        return Err(api_error_message(
            "admin_required",
            "pktmon diagnostic requires administrator permission",
        ));
    }
    start_diagnostic_session(&state, duration_seconds)
}

#[tauri::command]
pub(crate) fn diagnostic_status(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<DiagnosticStatus, ApiError> {
    diagnostic_status_inner(&state, &session_id)
}

#[tauri::command]
pub(crate) fn diagnostic_cancel(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<DiagnosticStatus, ApiError> {
    let session = diagnostic_runtime_session(&state, &session_id)?;
    session.stop.store(true, Ordering::SeqCst);
    {
        let mut status = session.status.lock().map_err(|_| {
            api_error_message("diagnostic_lock_poisoned", "diagnostic lock poisoned")
        })?;
        if matches!(status.state.as_str(), "starting" | "running") {
            status.state = "stopping".to_string();
            status.stage = "stopping".to_string();
            status.updated_at = now_seconds();
        }
    }
    diagnostic_status_inner(&state, &session_id)
}

pub(crate) fn start_diagnostic_session(
    state: &State<'_, AppState>,
    duration_seconds: Option<u64>,
) -> Result<DiagnosticStatus, ApiError> {
    let duration_seconds = duration_seconds
        .unwrap_or(DEFAULT_DIAGNOSTIC_DURATION_SECONDS)
        .clamp(
            MIN_DIAGNOSTIC_DURATION_SECONDS,
            MAX_DIAGNOSTIC_DURATION_SECONDS,
        );
    let session_id = new_named_session_id("diagnostic");
    let now = now_seconds();
    let initial_status = DiagnosticStatus {
        session_id: session_id.clone(),
        state: "starting".to_string(),
        started_at: now,
        updated_at: now,
        duration_seconds,
        elapsed_seconds: 0.0,
        stage: "preparing".to_string(),
        progress: 0.0,
        support_zip_path: None,
        error: None,
        summary: None,
    };
    let stop = Arc::new(AtomicBool::new(false));
    let runtime = Arc::new(DiagnosticRuntimeSession {
        status: Mutex::new(initial_status.clone()),
        stop: Arc::clone(&stop),
        handle: Mutex::new(None),
    });
    state
        .diagnostic_sessions
        .lock()
        .map_err(|_| api_error_message("diagnostic_lock_poisoned", "diagnostic lock poisoned"))?
        .insert(session_id.clone(), Arc::clone(&runtime));

    let runtime_for_thread = Arc::clone(&runtime);
    let handle = std::thread::spawn(move || {
        run_diagnostic_thread(runtime_for_thread, session_id, duration_seconds);
    });
    *runtime
        .handle
        .lock()
        .map_err(|_| api_error_message("diagnostic_lock_poisoned", "diagnostic lock poisoned"))? =
        Some(handle);
    Ok(initial_status)
}
