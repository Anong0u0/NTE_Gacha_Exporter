fn run_internal_capture(
    runtime: &Arc<DiagnosticRuntimeSession>,
    paths: &SupportPaths,
    target: &DiagnosticTargetDiscovery,
    environment: &DiagnosticEnvironment,
    duration: Duration,
    mode: DiagnosticMode,
    root: &Path,
) -> InternalDiagnosticReport {
    let Some(pid) = target.selected_pid else {
        return InternalDiagnosticReport {
            attempted: false,
            error: Some("internal capture skipped: HTGame.exe not found".to_string()),
            result: None,
        };
    };
    let strategy = diagnostic_strategy(mode);
    if mode == DiagnosticMode::Pktmon
        && target.selected_ports.is_empty()
        && strategy.kind == CaptureStrategyKind::PortFiltered
    {
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
        if matches!(
            status.state.as_str(),
            crate::lifecycle::STATE_RUNNING | crate::lifecycle::STATE_STARTING
        ) {
            status.state = crate::lifecycle::STATE_RUNNING.to_string();
            status.stage = "capturing".to_string();
            status.progress = 0.08 + capture_progress * 0.80;
            status.elapsed_seconds = elapsed;
            status.updated_at = now_seconds();
        }
    });

    let result = match mode {
        DiagnosticMode::Pktmon => run_diagnostic_capture(
            DiagnosticCaptureOptions {
                pid,
                exe: "HTGame.exe".to_string(),
                ports: target.selected_ports.clone(),
                pppoe_detection: Some(target.pppoe_detection.clone()),
                strategy: Some(strategy),
                raw_out: Some(paths.internal_raw.clone()),
                raw_append: false,
                dropped_samples_out: Some(paths.dropped_samples.clone()),
                duration,
                max_dropped_samples: DROPPED_SAMPLE_LIMIT,
                max_full_dropped_samples: 32,
                on_progress: Some(progress),
            },
            Arc::clone(&runtime.stop),
        ),
        DiagnosticMode::WinDivert => run_windivert_diagnostic_capture(
            pid,
            target,
            paths,
            root,
            duration,
            Arc::clone(&runtime.stop),
            progress,
        ),
    };
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

fn diagnostic_strategy(mode: DiagnosticMode) -> CaptureStrategy {
    match mode {
        DiagnosticMode::Pktmon => CaptureStrategy::no_filter(
            nte_capture::CaptureStrategyReason::DiagnosticNoFilter,
        ),
        DiagnosticMode::WinDivert => CaptureStrategy::no_filter(
            nte_capture::CaptureStrategyReason::WinDivertBackend,
        ),
    }
}

fn run_windivert_diagnostic_capture(
    pid: u32,
    target: &DiagnosticTargetDiscovery,
    paths: &SupportPaths,
    root: &Path,
    duration: Duration,
    stop: Arc<AtomicBool>,
    progress: Arc<dyn Fn(nte_capture::DiagnosticCaptureProgress) + Send + Sync + 'static>,
) -> anyhow::Result<DiagnosticCaptureResult> {
    let capture_stop = Arc::clone(&stop);
    let timer = std::thread::spawn(move || {
        let started = Instant::now();
        while started.elapsed() < duration && !capture_stop.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(100));
        }
        capture_stop.store(true, Ordering::SeqCst);
    });
    let result = capture_live(
        CaptureOptions {
            pid,
            exe: "HTGame.exe".to_string(),
            ports: target.selected_ports.clone(),
            pppoe_detection: Some(target.pppoe_detection.clone()),
            backend: CaptureBackend::WinDivert,
            strategy: Some(CaptureStrategy::no_filter(
                nte_capture::CaptureStrategyReason::WinDivertBackend,
            )),
            raw_out: Some(paths.internal_raw.clone()),
            raw_append: false,
            windivert_dir: Some(nte_capture::windivert::windivert_install_dir(root)),
            max_packets: 0,
            max_decoded: 0,
            on_progress: Some(Arc::new(move |capture_progress| {
                progress(nte_capture::DiagnosticCaptureProgress {
                    target: capture_progress.target,
                    counters: diagnostic_counters_from_live(&capture_progress.counters),
                    elapsed_seconds: 0.0,
                    rows_count: capture_progress.row_count as u64,
                    warning_count: capture_progress.warning_count as u64,
                });
            })),
        },
        stop,
    )?;
    let _ = timer.join();
    Ok(diagnostic_result_from_live(result, duration))
}

fn diagnostic_result_from_live(
    result: nte_capture::CaptureResult,
    duration: Duration,
) -> DiagnosticCaptureResult {
    let mut summary = DiagnosticCaptureSummary {
        rows_count: result.rows.len() as u64,
        warning_count: result.warnings.len() as u64,
        ..DiagnosticCaptureSummary::default()
    };
    for warning in &result.warnings {
        *summary
            .warning_code_counts
            .entry(warning.code.clone())
            .or_default() += 1;
    }
    DiagnosticCaptureResult {
        target: result.target,
        counters: diagnostic_counters_from_live(&result.counters),
        summary,
        warnings: result.warnings,
        elapsed_seconds: duration.as_secs_f64(),
    }
}

fn diagnostic_counters_from_live(counters: &nte_capture::CaptureCounters) -> DiagnosticCaptureCounters {
    DiagnosticCaptureCounters {
        packets_seen: counters.packets_seen,
        decoded_packets: counters.decoded_packets,
        dropped_packets: counters.dropped_packets,
        duplicate_packets: counters.duplicate_packets,
        filter_restarts: counters.filter_restarts,
        ..DiagnosticCaptureCounters::default()
    }
}

fn start_external_capture_thread(
    paths: &SupportPaths,
    ports: &[u16],
    pppoe_detection: &PppoeDetection,
    strategy: CaptureStrategy,
    duration: Duration,
    stop: Arc<AtomicBool>,
) -> Option<JoinHandle<ExternalCaptureReport>> {
    if ports.is_empty() && strategy.kind == CaptureStrategyKind::PortFiltered {
        return None;
    }
    let paths = paths.clone_for_thread();
    let ports = ports.to_vec();
    let pppoe_detection = pppoe_detection.clone();
    Some(std::thread::spawn(move || {
        run_external_pktmon_capture(
            &paths,
            &ports,
            pppoe_detection,
            strategy,
            duration,
            stop,
        )
    }))
}
