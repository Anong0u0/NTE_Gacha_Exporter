fn run_internal_capture(
    runtime: &Arc<DiagnosticRuntimeSession>,
    paths: &SupportPaths,
    target: &DiagnosticTargetDiscovery,
    environment: &DiagnosticEnvironment,
    duration: Duration,
) -> InternalDiagnosticReport {
    let Some(pid) = target.selected_pid else {
        return InternalDiagnosticReport {
            attempted: false,
            error: Some("internal capture skipped: HTGame.exe not found".to_string()),
            result: None,
        };
    };
    if target.selected_ports.is_empty() && !target.pppoe_detection.detected {
        return InternalDiagnosticReport {
            attempted: false,
            error: Some("internal capture skipped: no candidate ports".to_string()),
            result: None,
        };
    }
    if !environment.windows {
        return InternalDiagnosticReport {
            attempted: false,
            error: Some("internal capture skipped: pktmon requires Windows".to_string()),
            result: None,
        };
    }
    if !environment.admin {
        return InternalDiagnosticReport {
            attempted: false,
            error: Some("internal capture skipped: administrator permission required".to_string()),
            result: None,
        };
    }

    let started_at = Instant::now();
    let status_runtime = Arc::clone(runtime);
    let duration_seconds = duration.as_secs_f64().max(1.0);
    let progress = Arc::new(move |progress: nte_capture::DiagnosticCaptureProgress| {
        let elapsed = progress
            .elapsed_seconds
            .max(started_at.elapsed().as_secs_f64());
        let capture_progress = (elapsed / duration_seconds).clamp(0.0, 1.0);
        let mut status = status_runtime
            .status
            .lock()
            .expect("diagnostic status lock");
        if matches!(status.state.as_str(), "running" | "starting") {
            status.state = "running".to_string();
            status.stage = "capturing".to_string();
            status.progress = 0.08 + capture_progress * 0.80;
            status.elapsed_seconds = elapsed;
            status.updated_at = now_seconds();
        }
    });

    let result = run_diagnostic_capture(
        DiagnosticCaptureOptions {
            pid,
            exe: "HTGame.exe".to_string(),
            ports: target.selected_ports.clone(),
            pppoe_detection: Some(target.pppoe_detection.clone()),
            raw_out: Some(paths.internal_raw.clone()),
            dropped_samples_out: Some(paths.dropped_samples.clone()),
            duration,
            max_dropped_samples: DROPPED_SAMPLE_LIMIT,
            max_full_dropped_samples: 32,
            on_progress: Some(progress),
        },
        Arc::clone(&runtime.stop),
    );
    match result {
        Ok(result) => InternalDiagnosticReport {
            attempted: true,
            error: None,
            result: Some(result),
        },
        Err(error) => InternalDiagnosticReport {
            attempted: true,
            error: Some(error.to_string()),
            result: None,
        },
    }
}

fn start_external_capture_thread(
    paths: &SupportPaths,
    ports: &[u16],
    pppoe_detection: &PppoeDetection,
    filter_mode: CaptureFilterMode,
    duration: Duration,
    stop: Arc<AtomicBool>,
) -> Option<JoinHandle<ExternalCaptureReport>> {
    if ports.is_empty() && filter_mode == CaptureFilterMode::PortFiltered {
        return None;
    }
    let paths = paths.clone_for_thread();
    let ports = ports.to_vec();
    let pppoe_detection = pppoe_detection.clone();
    Some(std::thread::spawn(move || {
        run_external_pktmon_capture(&paths, &ports, pppoe_detection, filter_mode, duration, stop)
    }))
}
