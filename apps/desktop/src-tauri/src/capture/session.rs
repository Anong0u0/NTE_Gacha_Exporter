fn start_rust_capture_session(
    app: AppHandle<Wry>,
    state: &State<'_, AppState>,
    locale: &str,
    mode: CaptureMode,
    output_raw: Option<String>,
    known_record_keys: Vec<String>,
    start_options: CaptureStartOptions,
) -> Result<CaptureStatus, ApiError> {
    let pid = find_process_pid("HTGame.exe")
        .map_err(api_error)?
        .ok_or_else(|| api_error_message("capture_environment", "HTGame.exe not found"))?;
    let ports = candidate_ports(pid).map_err(api_error)?;
    let pppoe_detection = detect_pppoe();
    let settings = with_store(state, |store| store.settings())?;
    let backend = capture_backend_for_start(
        settings.capture_windivert_backend_enabled,
        start_options.capture_backend,
    );
    let strategy = capture_strategy_for_start(&pppoe_detection, backend);
    if backend == CaptureBackend::Pktmon
        && ports.is_empty()
        && strategy.kind == CaptureStrategyKind::PortFiltered
    {
        return Err(api_error_message(
            "capture_environment",
            "no HTGame.exe candidate ports",
        ));
    }
    let session_id = new_session_id();
    let now = now_seconds();
    let target = CaptureTarget {
        pid,
        exe: "HTGame.exe".to_string(),
        interface: capture_interface(backend).to_string(),
        ports: ports.clone(),
        bpf: capture_bpf(backend, strategy.kind, &ports),
        capture_strategy: strategy.kind.as_str().to_string(),
        strategy_reason: strategy.reason.as_str().to_string(),
        pppoe_detection: pppoe_detection.clone(),
        attempts: Vec::new(),
    };
    let initial_status = CaptureStatus {
        session_id: session_id.clone(),
        state: crate::lifecycle::STATE_STARTING.to_string(),
        mode: mode.as_str().to_string(),
        records_count: 0,
        latest_records: Vec::new(),
        counters: CaptureCounters::default(),
        attempts: Vec::new(),
        started_at: now,
        updated_at: now,
        target: Some(serde_json::to_value(&target).map_err(api_error)?),
        auto_page: None,
        raw_path: output_raw.clone(),
        error: None,
        document: None,
        import_report: None,
    };
    let stop = Arc::new(AtomicBool::new(false));
    let runtime = Arc::new(CaptureRuntimeSession {
        status: Mutex::new(initial_status.clone()),
        stop: Arc::clone(&stop),
        attempt_stop: Mutex::new(None),
        handle: Mutex::new(None),
    });
    state
        .capture_sessions
        .lock()
        .map_err(|_| api_error_message("capture_lock_poisoned", "capture lock poisoned"))?
        .insert(session_id.clone(), Arc::clone(&runtime));

    let raw_out = output_raw.map(PathBuf::from);
    let locale_for_thread = locale.to_string();
    let runtime_for_thread = Arc::clone(&runtime);
    let stop_for_thread = Arc::clone(&stop);
    let callback = (!mode.auto_page()).then(|| {
        capture_progress_callback(Arc::clone(&runtime), locale.to_string(), None)
    });
    let handle = std::thread::spawn(move || {
        if mode.auto_page() {
            run_auto_page_capture_thread(AutoPageCaptureThread {
                runtime: runtime_for_thread,
                pid,
                ports,
                pppoe_detection,
                strategy,
                backend,
                raw_out,
                locale: locale_for_thread,
                stop: stop_for_thread,
                mode,
                start_options,
                known_record_keys,
                app,
            });
        } else {
            let result = capture_live(
                CaptureOptions {
                    pid,
                    exe: "HTGame.exe".to_string(),
                    ports,
                    pppoe_detection: Some(pppoe_detection),
                    backend,
                    strategy: Some(strategy),
                    raw_out,
                    raw_append: false,
                    windivert_dir: windivert_dir_for_backend(backend),
                    max_packets: 0,
                    max_decoded: 0,
                    on_progress: callback,
                },
                stop_for_thread,
            );
            let should_persist_windivert = windivert_capture_succeeded(&result, None);
            if should_persist_windivert {
                enable_windivert_backend_from_app(&app);
            }
            finish_capture_result(
                &runtime_for_thread,
                FinishCaptureInput {
                    result: result.map_err(|error| error.to_string()),
                    locale: &locale_for_thread,
                    source_kind: capture_source_kind(backend, false),
                    auto_page: None,
                    auto_error: None,
                    auto_result: None,
                    cancel_requested: false,
                },
            );
        }
    });
    *runtime
        .handle
        .lock()
        .map_err(|_| api_error_message("capture_lock_poisoned", "capture lock poisoned"))? =
        Some(handle);
    Ok(initial_status)
}

struct AutoPageCaptureThread {
    runtime: Arc<CaptureRuntimeSession>,
    pid: u32,
    ports: Vec<u16>,
    pppoe_detection: nte_capture::PppoeDetection,
    strategy: CaptureStrategy,
    backend: CaptureBackend,
    raw_out: Option<PathBuf>,
    locale: String,
    stop: Arc<AtomicBool>,
    mode: CaptureMode,
    start_options: CaptureStartOptions,
    known_record_keys: Vec<String>,
    app: AppHandle<Wry>,
}

fn run_auto_page_capture_thread(context: AutoPageCaptureThread) {
    let AutoPageCaptureThread {
        runtime,
        pid,
        ports,
        pppoe_detection,
        strategy,
        backend,
        raw_out,
        locale,
        stop,
        mode,
        start_options,
        known_record_keys,
        app,
    } = context;
    let mut final_capture_result = Err("capture attempt did not run".to_string());
    let mut final_auto_result = None;
    let mut final_auto_page = None;
    let mut final_error = None;
    let mut attempts = Vec::new();

    if !stop.load(Ordering::SeqCst) {
        let attempt_stop = Arc::new(AtomicBool::new(false));
        set_attempt_stop(&runtime, Some(Arc::clone(&attempt_stop)));
        let coordinator = Arc::new(AutoPageCoordinator::new(
            mode.full_update(),
            &known_record_keys,
        ));
        let callback = capture_progress_callback(
            Arc::clone(&runtime),
            locale.clone(),
            Some(Arc::clone(&coordinator)),
        );
        let capture_stop = Arc::clone(&attempt_stop);
        let capture_ports = ports.clone();
        let capture_detection = pppoe_detection.clone();
        let capture_raw_out = raw_out.clone();
        let capture_handle = std::thread::spawn(move || {
            capture_live(
                CaptureOptions {
                    pid,
                    exe: "HTGame.exe".to_string(),
                    ports: capture_ports,
                    pppoe_detection: Some(capture_detection),
                    backend,
                    strategy: Some(strategy),
                    raw_out: capture_raw_out,
                    raw_append: false,
                    windivert_dir: windivert_dir_for_backend(backend),
                    max_packets: 0,
                    max_decoded: 0,
                    on_progress: Some(callback),
                },
                capture_stop,
            )
            .map_err(|error| error.to_string())
        });

        let auto_runtime = Arc::clone(&runtime);
        let auto_status_callback = Arc::new(move |status: AutomationStatus| {
            if let Ok(mut capture_status) = auto_runtime.status.lock() {
                capture_status.auto_page = Some(auto_page_status_value(&status, "running"));
                if capture_status.state != crate::lifecycle::STATE_STOPPING {
                    capture_status.state = crate::lifecycle::STATE_RUNNING.to_string();
                }
                capture_status.updated_at = now_seconds();
            }
        });
        let control = {
            let coordinator = Arc::clone(&coordinator);
            Arc::new(move |context: AutoPageControlContext| coordinator.decision(context))
        };
        let mut options = AutomationOptions::new(pid, Arc::clone(&attempt_stop));
        options.full_update = mode.full_update();
        options.non_interactive = true;
        apply_capture_start_options(&mut options, &start_options);
        options.control = Some(control);
        options.on_status = Some(auto_status_callback);
        let auto_result = run_auto_page(options);
        let drain_error = auto_result
            .succeeded()
            .then(|| {
                wait_for_capture_drain(
                    &runtime,
                    &coordinator,
                    &auto_result,
                    &stop,
                    &attempt_stop,
                )
            })
            .flatten();
        attempt_stop.store(true, Ordering::SeqCst);

        let capture_result = capture_handle
            .join()
            .map_err(|_| "capture worker panicked".to_string())
            .and_then(|result| result);
        extend_attempts_from_result(&mut attempts, &capture_result);
        final_auto_page = Some(auto_page_result_value(&auto_result));
        final_error = if auto_result.succeeded() {
            drain_error
        } else {
            Some(auto_page_runtime_error(&auto_result.message))
        };
        final_capture_result = apply_attempts_to_result(capture_result, &attempts);
        final_auto_result = Some(auto_result);
    }

    set_attempt_stop(&runtime, None);
    let stopped_by_user = stop.load(Ordering::SeqCst);
    stop.store(true, Ordering::SeqCst);
    if windivert_capture_succeeded(&final_capture_result, final_error.as_ref()) {
        enable_windivert_backend_from_app(&app);
    }
    finish_capture_result(
        &runtime,
        FinishCaptureInput {
            result: final_capture_result,
            locale: &locale,
            source_kind: capture_source_kind(backend, true),
            auto_page: final_auto_page,
            auto_error: final_error,
            auto_result: final_auto_result.as_ref(),
            cancel_requested: stopped_by_user,
        },
    );
    finish_auto_page_terminal(pid, stopped_by_user, &app);
}

fn capture_strategy_for_start(
    detection: &nte_capture::PppoeDetection,
    backend: CaptureBackend,
) -> CaptureStrategy {
    if backend == CaptureBackend::WinDivert {
        CaptureStrategy::no_filter(CaptureStrategyReason::WinDivertBackend)
    } else if detection.detected {
        CaptureStrategy::no_filter(CaptureStrategyReason::PppoeFastPath)
    } else {
        CaptureStrategy::port_filtered()
    }
}

fn capture_backend_for_start(
    enabled: bool,
    backend_override: Option<CaptureBackendOverride>,
) -> CaptureBackend {
    match backend_override {
        Some(CaptureBackendOverride::Pktmon) => CaptureBackend::Pktmon,
        Some(CaptureBackendOverride::WinDivert) => CaptureBackend::WinDivert,
        None if enabled => CaptureBackend::WinDivert,
        None => CaptureBackend::Pktmon,
    }
}

fn windivert_dir_for_backend(backend: CaptureBackend) -> Option<PathBuf> {
    if backend == CaptureBackend::WinDivert {
        return portable_root()
            .ok()
            .map(|root| nte_capture::windivert::windivert_install_dir(&root));
    }
    None
}

fn enable_windivert_backend_from_app(app: &AppHandle<Wry>) {
    let state = app.state::<AppState>();
    let _ = with_store(&state, |store| {
        store.update_settings(SettingsPatch {
            capture_windivert_backend_enabled: Some(true),
            ..SettingsPatch::default()
        })?;
        Ok(())
    });
}

fn windivert_capture_succeeded(
    result: &Result<nte_capture::CaptureResult, impl std::fmt::Display>,
    error: Option<&RuntimeError>,
) -> bool {
    error.is_none()
        && result.as_ref().is_ok_and(|result| {
            (!result.rows.is_empty() || result.counters.decoded_packets > 0)
                && result.target.interface == CaptureBackend::WinDivert.as_str()
        })
}

fn capture_interface(backend: CaptureBackend) -> &'static str {
    match backend {
        CaptureBackend::Pktmon => "pktmon",
        CaptureBackend::WinDivert => "windivert",
    }
}

fn capture_source_kind(backend: CaptureBackend, auto_page: bool) -> &'static str {
    match (backend, auto_page) {
        (CaptureBackend::Pktmon, false) => "pktmon-live-capture",
        (CaptureBackend::Pktmon, true) => "pktmon-auto-page-capture",
        (CaptureBackend::WinDivert, false) => "windivert-live-capture",
        (CaptureBackend::WinDivert, true) => "windivert-auto-page-capture",
    }
}

fn capture_bpf(backend: CaptureBackend, strategy: CaptureStrategyKind, ports: &[u16]) -> String {
    if backend == CaptureBackend::WinDivert {
        return "ip".to_string();
    }
    match strategy {
        CaptureStrategyKind::PortFiltered => ports
            .iter()
            .map(|port| format!("port {port}"))
            .collect::<Vec<_>>()
            .join(" or "),
        CaptureStrategyKind::NoFilter => "none".to_string(),
    }
}

fn set_attempt_stop(runtime: &Arc<CaptureRuntimeSession>, stop: Option<Arc<AtomicBool>>) {
    if let Ok(mut guard) = runtime.attempt_stop.lock() {
        *guard = stop;
    }
}

fn extend_attempts_from_result(
    attempts: &mut Vec<CaptureAttemptSummary>,
    result: &Result<nte_capture::CaptureResult, String>,
) {
    let Ok(result) = result else {
        return;
    };
    for attempt in &result.attempts {
        let mut attempt = attempt.clone();
        attempt.attempt_index = attempts.len() as u32;
        attempts.push(attempt);
    }
}

fn apply_attempts_to_result(
    result: Result<nte_capture::CaptureResult, String>,
    attempts: &[CaptureAttemptSummary],
) -> Result<nte_capture::CaptureResult, String> {
    result.map(|mut result| {
        result.attempts = attempts.to_vec();
        result.target.attempts = attempts.to_vec();
        result
    })
}

fn finish_auto_page_terminal(pid: u32, stopped_by_user: bool, app: &AppHandle<Wry>) {
    if !stopped_by_user {
        let _ = nte_automation::restore_game_home(pid);
    }
    wake_main_window(app);
}

fn auto_page_runtime_error(message: &str) -> RuntimeError {
    let code = if message.starts_with("capture window stalled:") {
        AUTO_PAGE_CAPTURE_WINDOW_STALLED_CODE
    } else {
        AUTO_PAGE_FAILED_CODE
    };
    runtime_error(code, message)
}

fn apply_capture_start_options(options: &mut AutomationOptions, start_options: &CaptureStartOptions) {
    if let Some(page_record_min_wait_ms) = start_options.page_record_min_wait_ms {
        options.page_record_min_wait = Duration::from_millis(page_record_min_wait_ms.clamp(
            AUTO_PAGE_PAGE_RECORD_MIN_WAIT_MS,
            AUTO_PAGE_MAX_PAGE_RECORD_MIN_WAIT_MS,
        ));
    }
}
