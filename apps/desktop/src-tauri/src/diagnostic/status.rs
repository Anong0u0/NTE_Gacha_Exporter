fn update_status(
    runtime: &Arc<DiagnosticRuntimeSession>,
    state: &str,
    stage: &str,
    progress: f64,
    support_zip_path: Option<String>,
    summary: Option<DiagnosticStatusSummary>,
) {
    let mut status = runtime.status.lock().expect("diagnostic status lock");
    status.state = state.to_string();
    status.stage = stage.to_string();
    status.progress = progress.clamp(0.0, 1.0);
    status.updated_at = now_seconds();
    status.elapsed_seconds = status.updated_at - status.started_at;
    if support_zip_path.is_some() {
        status.support_zip_path = support_zip_path;
    }
    if summary.is_some() {
        status.summary = summary;
    }
}

fn diagnostic_status_inner(
    state: &State<'_, AppState>,
    session_id: &str,
) -> Result<DiagnosticStatus, ApiError> {
    let session = diagnostic_runtime_session(state, session_id)?;
    let status = session
        .status
        .lock()
        .map_err(|_| api_error_message("diagnostic_lock_poisoned", "diagnostic lock poisoned"))?
        .clone();
    cleanup_terminal_diagnostic_session(state, session_id, &session, &status)?;
    Ok(status)
}

fn diagnostic_runtime_session(
    state: &State<'_, AppState>,
    session_id: &str,
) -> Result<Arc<DiagnosticRuntimeSession>, ApiError> {
    state
        .diagnostic_sessions
        .lock()
        .map_err(|_| api_error_message("diagnostic_lock_poisoned", "diagnostic lock poisoned"))?
        .get(session_id)
        .cloned()
        .ok_or_else(|| api_error_message("diagnostic_not_found", "diagnostic session not found"))
}

fn cleanup_terminal_diagnostic_session(
    state: &State<'_, AppState>,
    session_id: &str,
    session: &DiagnosticRuntimeSession,
    status: &DiagnosticStatus,
) -> Result<(), ApiError> {
    if !diagnostic_status_is_terminal(status) {
        return Ok(());
    }
    let _ = try_join_finished_diagnostic_thread(session);
    let mut sessions = state
        .diagnostic_sessions
        .lock()
        .map_err(|_| api_error_message("diagnostic_lock_poisoned", "diagnostic lock poisoned"))?;
    prune_diagnostic_session_map(&mut sessions, session_id, now_seconds());
    Ok(())
}

fn prune_diagnostic_session_map(
    sessions: &mut HashMap<String, Arc<DiagnosticRuntimeSession>>,
    preserve_session_id: &str,
    now: f64,
) {
    for session in sessions.values() {
        let is_terminal = session
            .status
            .lock()
            .map(|status| diagnostic_status_is_terminal(&status))
            .unwrap_or(false);
        if is_terminal {
            let _ = try_join_finished_diagnostic_thread(session);
        }
    }

    let removable = sessions
        .iter()
        .filter_map(|(session_id, session)| {
            if session_id == preserve_session_id || !diagnostic_handle_joined(session) {
                return None;
            }
            let status = session.status.lock().ok()?;
            diagnostic_status_is_terminal(&status).then(|| (session_id.clone(), status.updated_at))
        })
        .collect::<Vec<_>>();

    let mut to_remove = BTreeSet::new();
    for (session_id, updated_at) in &removable {
        if now - *updated_at >= DIAGNOSTIC_SESSION_RETENTION_SECONDS {
            to_remove.insert(session_id.clone());
        }
    }

    let recent = removable
        .into_iter()
        .filter(|(session_id, _)| !to_remove.contains(session_id))
        .collect::<Vec<_>>();
    if recent.len() > DIAGNOSTIC_TERMINAL_SESSION_LIMIT {
        let extra = recent.len() - DIAGNOSTIC_TERMINAL_SESSION_LIMIT;
        let mut recent = recent;
        recent.sort_by(|left, right| {
            left.1
                .total_cmp(&right.1)
                .then_with(|| left.0.cmp(&right.0))
        });
        for (session_id, _) in recent.into_iter().take(extra) {
            to_remove.insert(session_id);
        }
    }

    for session_id in to_remove {
        sessions.remove(&session_id);
    }
}

fn diagnostic_status_is_terminal(status: &DiagnosticStatus) -> bool {
    matches!(status.state.as_str(), "completed" | "failed")
}

fn diagnostic_handle_joined(session: &DiagnosticRuntimeSession) -> bool {
    session
        .handle
        .lock()
        .map(|handle| handle.is_none())
        .unwrap_or(false)
}

fn try_join_finished_diagnostic_thread(session: &DiagnosticRuntimeSession) -> bool {
    let handle = session.handle.lock().ok().and_then(|mut guard| {
        guard
            .as_ref()
            .is_some_and(std::thread::JoinHandle::is_finished)
            .then(|| guard.take())
            .flatten()
    });
    let Some(handle) = handle else {
        return false;
    };
    handle.join().is_ok()
}
