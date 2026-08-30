fn capture_status_with_merge(
    state: &State<'_, AppState>,
    session_id: &str,
) -> Result<CaptureStatus, ApiError> {
    let session = capture_runtime_session(state, session_id)?;
    let mut status = session
        .status
        .lock()
        .map_err(|_| api_error_message("capture_lock_poisoned", "capture lock poisoned"))?
        .clone();
    if !capture_state_can_merge(&status.state) {
        cleanup_terminal_capture_session(state, session_id, &session, &status)?;
        return Ok(status);
    }
    let merge_error = {
        let mut captures = state
            .captures
            .lock()
            .map_err(|_| api_error_message("capture_lock_poisoned", "capture lock poisoned"))?;
        captures
            .get_mut(session_id)
            .and_then(|meta| merge_capture(state, &mut status, meta).err())
    };
    if let Some(error) = merge_error {
        let mut runtime_status = session
            .status
            .lock()
            .map_err(|_| api_error_message("capture_lock_poisoned", "capture lock poisoned"))?;
        apply_capture_import_failure(&mut runtime_status, &error);
        status = runtime_status.clone();
        drop(runtime_status);
        cleanup_terminal_capture_session(state, session_id, &session, &status)?;
        return Ok(status);
    }
    cleanup_terminal_capture_session(state, session_id, &session, &status)?;
    Ok(status)
}
fn merge_capture(
    state: &State<'_, AppState>,
    status: &mut CaptureStatus,
    meta: &mut CaptureSessionMeta,
) -> Result<(), ApiError> {
    if let Some(report) = &meta.import_report {
        status.import_report = Some(report.clone());
        return Ok(());
    }
    let Some(document) = status.document.as_ref() else {
        return Ok(());
    };
    let document_text = serde_json::to_string(document).map_err(api_error)?;
    with_store(state, |store| {
        merge_capture_document_with_store(store, status, meta, &document_text)
    })
}

fn apply_capture_import_failure(status: &mut CaptureStatus, error: &ApiError) {
    status.state = crate::lifecycle::STATE_FAILED.to_string();
    status.updated_at = now_seconds();
    status.latest_records.clear();
    status.error = Some(runtime_error("capture_import_failed", error.message().to_string()));
    status.document = None;
    status.import_report = None;
}

fn capture_state_can_merge(state: &str) -> bool {
    matches!(
        state,
        crate::lifecycle::STATE_COMPLETED | crate::lifecycle::STATE_PARTIAL
    )
}

fn capture_runtime_session(
    state: &State<'_, AppState>,
    session_id: &str,
) -> Result<Arc<CaptureRuntimeSession>, ApiError> {
    state
        .capture_sessions
        .lock()
        .map_err(|_| api_error_message("capture_lock_poisoned", "capture lock poisoned"))?
        .get(session_id)
        .cloned()
        .ok_or_else(|| api_error_message("capture_not_found", "capture session not found"))
}


fn merge_capture_document_with_store(
    store: &nte_store::JsonStore,
    status: &mut CaptureStatus,
    meta: &mut CaptureSessionMeta,
    document_text: &str,
) -> Result<(), nte_core::GuiError> {
    if let Some(report) = &meta.import_report {
        status.import_report = Some(report.clone());
        return Ok(());
    }
    let report = if meta.full_update {
        store.import_public_document_with_backup(
            &meta.profile_name,
            document_text,
            &meta.source_kind,
            meta.source_path.as_deref(),
        )?
    } else {
        store.import_public_document(
            &meta.profile_name,
            document_text,
            &meta.source_kind,
            meta.source_path.as_deref(),
        )?
    };
    meta.import_report = Some(report.clone());
    if meta.source_path.is_some() {
        store.cleanup_generated_raw_runs_keep_latest()?;
    }
    status.import_report = Some(report);
    Ok(())
}

fn cleanup_terminal_capture_session(
    state: &State<'_, AppState>,
    session_id: &str,
    session: &CaptureRuntimeSession,
    status: &CaptureStatus,
) -> Result<(), ApiError> {
    if !capture_status_is_terminal(status) {
        return Ok(());
    }
    let _ = try_join_finished_capture_thread(session);
    let mut sessions = state
        .capture_sessions
        .lock()
        .map_err(|_| api_error_message("capture_lock_poisoned", "capture lock poisoned"))?;
    let mut captures = state
        .captures
        .lock()
        .map_err(|_| api_error_message("capture_lock_poisoned", "capture lock poisoned"))?;
    prune_capture_session_maps(&mut sessions, &mut captures, session_id, now_seconds());
    Ok(())
}

fn prune_capture_session_maps(
    sessions: &mut HashMap<String, Arc<CaptureRuntimeSession>>,
    captures: &mut HashMap<String, CaptureSessionMeta>,
    preserve_session_id: &str,
    now: f64,
) {
    for session in sessions.values() {
        let is_terminal = session
            .status
            .lock()
            .map(|status| capture_status_is_terminal(&status))
            .unwrap_or(false);
        if is_terminal {
            let _ = try_join_finished_capture_thread(session);
        }
    }

    let removable = sessions
        .iter()
        .filter_map(|(session_id, session)| {
            if session_id == preserve_session_id || !capture_handle_joined(session) {
                return None;
            }
            let status = session.status.lock().ok()?;
            capture_status_is_terminal(&status).then(|| (session_id.clone(), status.updated_at))
        })
        .collect::<Vec<_>>();

    let mut to_remove = BTreeSet::new();
    for (session_id, updated_at) in &removable {
        if now - *updated_at >= CAPTURE_SESSION_RETENTION_SECONDS {
            to_remove.insert(session_id.clone());
        }
    }

    let recent = removable
        .into_iter()
        .filter(|(session_id, _)| !to_remove.contains(session_id))
        .collect::<Vec<_>>();
    if recent.len() > CAPTURE_TERMINAL_SESSION_LIMIT {
        let extra = recent.len() - CAPTURE_TERMINAL_SESSION_LIMIT;
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
        captures.remove(&session_id);
    }
}

fn capture_status_is_terminal(status: &CaptureStatus) -> bool {
    crate::lifecycle::is_terminal_state(&status.state)
}

fn capture_handle_joined(session: &CaptureRuntimeSession) -> bool {
    crate::lifecycle::handle_joined(&session.handle)
}

fn try_join_finished_capture_thread(session: &CaptureRuntimeSession) -> bool {
    crate::lifecycle::try_join_finished_thread(&session.handle)
}
