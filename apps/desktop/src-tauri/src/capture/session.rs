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
    let filter_mode = CaptureFilterMode::for_pppoe_detection(&pppoe_detection);
    if ports.is_empty() && filter_mode == CaptureFilterMode::PortFiltered {
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
        interface: "pktmon".to_string(),
        ports: ports.clone(),
        bpf: match filter_mode {
            CaptureFilterMode::PortFiltered => ports
                .iter()
                .map(|port| format!("port {port}"))
                .collect::<Vec<_>>()
                .join(" or "),
            CaptureFilterMode::NoFilterPppoe => "none (pppoe detected)".to_string(),
        },
        filter_mode: filter_mode.as_str().to_string(),
        pppoe_detection: pppoe_detection.clone(),
    };
    let initial_status = CaptureStatus {
        session_id: session_id.clone(),
        state: "starting".to_string(),
        mode: mode.as_str().to_string(),
        records_count: 0,
        latest_records: Vec::new(),
        counters: CaptureCounters::default(),
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
        handle: Mutex::new(None),
    });
    state
        .capture_sessions
        .lock()
        .map_err(|_| api_error_message("capture_lock_poisoned", "capture lock poisoned"))?
        .insert(session_id.clone(), Arc::clone(&runtime));

    let raw_out = output_raw.map(PathBuf::from);
    let locale_for_thread = locale.to_string();
    let coordinator = Arc::new(AutoPageCoordinator::new(
        mode.full_update(),
        &known_record_keys,
    ));
    let progress_coordinator = mode.auto_page().then(|| Arc::clone(&coordinator));
    let callback = capture_progress_callback(
        Arc::clone(&runtime),
        locale.to_string(),
        progress_coordinator,
    );
    let runtime_for_thread = Arc::clone(&runtime);
    let stop_for_thread = Arc::clone(&stop);
    let handle = std::thread::spawn(move || {
        if mode.auto_page() {
            run_auto_page_capture_thread(AutoPageCaptureThread {
                runtime: runtime_for_thread,
                pid,
                ports,
                pppoe_detection,
                raw_out,
                locale: locale_for_thread,
                stop: stop_for_thread,
                callback,
                mode,
                start_options,
                coordinator,
                app,
            });
        } else {
            let result = capture_live(
                CaptureOptions {
                    pid,
                    exe: "HTGame.exe".to_string(),
                    ports,
                    pppoe_detection: Some(pppoe_detection),
                    raw_out,
                    max_packets: 0,
                    max_decoded: 0,
                    on_progress: Some(callback),
                },
                stop_for_thread,
            );
            finish_capture_result(
                &runtime_for_thread,
                result.map_err(|error| error.to_string()),
                &locale_for_thread,
                "pktmon-live-capture",
                None,
                None,
                None,
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
    raw_out: Option<PathBuf>,
    locale: String,
    stop: Arc<AtomicBool>,
    callback: Arc<dyn Fn(nte_capture::CaptureProgress) + Send + Sync + 'static>,
    mode: CaptureMode,
    start_options: CaptureStartOptions,
    coordinator: Arc<AutoPageCoordinator>,
    app: AppHandle<Wry>,
}

fn run_auto_page_capture_thread(context: AutoPageCaptureThread) {
    let AutoPageCaptureThread {
        runtime,
        pid,
        ports,
        pppoe_detection,
        raw_out,
        locale,
        stop,
        callback,
        mode,
        start_options,
        coordinator,
        app,
    } = context;
    let capture_stop = Arc::clone(&stop);
    let capture_handle = std::thread::spawn(move || {
        capture_live(
            CaptureOptions {
                pid,
                exe: "HTGame.exe".to_string(),
                ports,
                pppoe_detection: Some(pppoe_detection),
                raw_out,
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
            if capture_status.state != "stopping" {
                capture_status.state = "running".to_string();
            }
            capture_status.updated_at = now_seconds();
        }
    });
    let control = {
        let coordinator = Arc::clone(&coordinator);
        Arc::new(move |context: AutoPageControlContext| coordinator.decision(context))
    };
    let mut options = AutomationOptions::new(pid, Arc::clone(&stop));
    options.full_update = mode.full_update();
    options.non_interactive = true;
    apply_capture_start_options(&mut options, &start_options);
    options.control = Some(control);
    options.on_status = Some(auto_status_callback);
    let auto_result = run_auto_page(options);
    let drain_error = auto_result
        .succeeded()
        .then(|| wait_for_capture_drain(&runtime, &coordinator, &auto_result, &stop))
        .flatten();
    let stopped_by_user = stop.load(Ordering::SeqCst);
    stop.store(true, Ordering::SeqCst);

    let capture_result = capture_handle
        .join()
        .map_err(|_| "capture worker panicked".to_string())
        .and_then(|result| result);
    let auto_page = Some(auto_page_result_value(&auto_result));
    let error = if auto_result.succeeded() {
        drain_error
    } else {
        Some(auto_page_runtime_error(&auto_result.message))
    };
    finish_capture_result(
        &runtime,
        capture_result,
        &locale,
        "pktmon-auto-page-capture",
        auto_page,
        error,
        Some(&auto_result),
    );
    finish_auto_page_terminal(pid, stopped_by_user, &app);
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
