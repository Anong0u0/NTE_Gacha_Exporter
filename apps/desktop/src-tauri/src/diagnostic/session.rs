fn run_diagnostic_thread(
    runtime: Arc<DiagnosticRuntimeSession>,
    session_id: String,
    duration_seconds: u64,
) {
    update_status(&runtime, "running", "resolving_target", 0.04, None, None);
    let result = build_support_bundle(&runtime, &session_id, duration_seconds);
    match result {
        Ok((document, zip_path)) => {
            let summary = status_summary(&document);
            update_status(
                &runtime,
                "completed",
                "completed",
                1.0,
                Some(zip_path.to_string_lossy().to_string()),
                Some(summary),
            );
        }
        Err(error) => {
            let runtime_error = RuntimeError {
                code: "diagnostic_failed".to_string(),
                message: error.to_string(),
                support_path: None,
                support_image_path: None,
            };
            let mut status = runtime.status.lock().expect("diagnostic status lock");
            status.state = "failed".to_string();
            status.stage = "failed".to_string();
            status.updated_at = now_seconds();
            status.elapsed_seconds = status.updated_at - status.started_at;
            status.error = Some(runtime_error);
        }
    }
}

fn build_support_bundle(
    runtime: &Arc<DiagnosticRuntimeSession>,
    session_id: &str,
    duration_seconds: u64,
) -> anyhow::Result<(DiagnosticDocument, PathBuf)> {
    let root = portable_root()?;
    let paths = support_paths(&root, session_id);
    fs::create_dir_all(&paths.support_dir)?;
    let environment = DiagnosticEnvironment {
        windows: cfg!(windows),
        admin: is_admin(),
        portable_root: root.to_string_lossy().to_string(),
        current_exe: std::env::current_exe()
            .ok()
            .map(|path| path.to_string_lossy().to_string()),
        current_dir: std::env::current_dir()
            .ok()
            .map(|path| path.to_string_lossy().to_string()),
        process_id: std::process::id(),
    };
    let target = discover_target(detect_pppoe());
    update_status(runtime, "running", "capturing", 0.08, None, None);

    let duration = Duration::from_secs(duration_seconds);
    let filter_mode = CaptureFilterMode::for_pppoe_detection(&target.pppoe_detection);
    let external_handle = start_external_capture_thread(
        &paths,
        &target.selected_ports,
        &target.pppoe_detection,
        filter_mode,
        duration,
        Arc::clone(&runtime.stop),
    );
    let internal = run_internal_capture(runtime, &paths, &target, &environment, duration);
    update_status(runtime, "running", "external_capture", 0.90, None, None);
    let external = external_handle
        .map(|handle| {
            handle.join().unwrap_or_else(|_| ExternalCaptureReport {
                attempted: true,
                ok: false,
                error: Some("external pktmon worker panicked".to_string()),
                filter_mode: filter_mode.as_str().to_string(),
                pppoe_detection: target.pppoe_detection.clone(),
                etl_path: Some(paths.external_etl.to_string_lossy().to_string()),
                pcapng_path: Some(paths.external_pcapng.to_string_lossy().to_string()),
                stdout_log_path: Some(paths.external_stdout.to_string_lossy().to_string()),
                stderr_log_path: Some(paths.external_stderr.to_string_lossy().to_string()),
                counters_json_path: Some(paths.counters_json.to_string_lossy().to_string()),
                counters_txt_path: Some(paths.counters_txt.to_string_lossy().to_string()),
                command_log_path: Some(paths.external_commands.to_string_lossy().to_string()),
                commands: Vec::new(),
            })
        })
        .unwrap_or_else(|| ExternalCaptureReport {
            attempted: false,
            ok: false,
            error: Some("external pktmon skipped: no selected ports".to_string()),
            filter_mode: filter_mode.as_str().to_string(),
            pppoe_detection: target.pppoe_detection.clone(),
            etl_path: None,
            pcapng_path: None,
            stdout_log_path: None,
            stderr_log_path: None,
            counters_json_path: None,
            counters_txt_path: None,
            command_log_path: None,
            commands: Vec::new(),
        });

    update_status(runtime, "running", "packing", 0.95, None, None);
    let classification = classify_diagnostic(&environment, &target, &internal, &external);
    let mut document = DiagnosticDocument {
        schema_version: 1,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        session_id: session_id.to_string(),
        created_at: now_seconds(),
        duration_seconds,
        environment,
        target,
        internal,
        external,
        artifacts: Vec::new(),
        verdict: classification,
    };
    fs::write(
        &paths.diagnostic_json,
        serde_json::to_vec_pretty(&document)?,
    )?;
    document.artifacts = collect_artifacts(&paths);
    fs::write(
        &paths.diagnostic_json,
        serde_json::to_vec_pretty(&document)?,
    )?;
    write_support_zip(&paths)?;
    rotate_support_zips(&paths.support_dir, &paths.zip_path)?;
    cleanup_artifact_files(&paths);
    Ok((document, paths.zip_path))
}
