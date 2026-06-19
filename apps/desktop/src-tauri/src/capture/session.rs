fn start_rust_capture_session(
    state: &State<'_, AppState>,
    locale: &str,
    mode: CaptureMode,
    output_raw: Option<String>,
    known_record_ids: Vec<String>,
) -> Result<CaptureStatus, ApiError> {
    let pid = find_process_pid("HTGame.exe")
        .map_err(api_error)?
        .ok_or_else(|| api_error_message("capture_environment", "HTGame.exe not found"))?;
    let ports = candidate_ports(pid).map_err(api_error)?;
    if ports.is_empty() {
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
        bpf: ports
            .iter()
            .map(|port| format!("port {port}"))
            .collect::<Vec<_>>()
            .join(" or "),
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
    let snapshots = Arc::new(Mutex::new(Vec::<AutomationRecordSnapshot>::new()));
    let progress_snapshots = mode.auto_page().then(|| Arc::clone(&snapshots));
    let page_tracker = Arc::new(Mutex::new(CapturePageTracker::default()));
    let progress_page_tracker = mode.auto_page().then(|| Arc::clone(&page_tracker));
    let callback = capture_progress_callback(
        Arc::clone(&runtime),
        locale.to_string(),
        progress_snapshots,
        progress_page_tracker,
    );
    let runtime_for_thread = Arc::clone(&runtime);
    let stop_for_thread = Arc::clone(&stop);
    let handle = std::thread::spawn(move || {
        if mode.auto_page() {
            run_auto_page_capture_thread(AutoPageCaptureThread {
                runtime: runtime_for_thread,
                pid,
                ports,
                raw_out,
                locale: locale_for_thread,
                stop: stop_for_thread,
                callback,
                mode,
                known_record_ids,
                snapshots,
                page_tracker,
            });
        } else {
            let result = capture_live(
                CaptureOptions {
                    pid,
                    exe: "HTGame.exe".to_string(),
                    ports,
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
    raw_out: Option<PathBuf>,
    locale: String,
    stop: Arc<AtomicBool>,
    callback: Arc<dyn Fn(nte_capture::CaptureProgress) + Send + Sync + 'static>,
    mode: CaptureMode,
    known_record_ids: Vec<String>,
    snapshots: Arc<Mutex<Vec<AutomationRecordSnapshot>>>,
    page_tracker: Arc<Mutex<CapturePageTracker>>,
}

fn run_auto_page_capture_thread(context: AutoPageCaptureThread) {
    let AutoPageCaptureThread {
        runtime,
        pid,
        ports,
        raw_out,
        locale,
        stop,
        callback,
        mode,
        known_record_ids,
        snapshots,
        page_tracker,
    } = context;
    let capture_stop = Arc::clone(&stop);
    let capture_handle = std::thread::spawn(move || {
        capture_live(
            CaptureOptions {
                pid,
                exe: "HTGame.exe".to_string(),
                ports,
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
    let snapshot_callback = {
        let snapshots = Arc::clone(&snapshots);
        Arc::new(move || {
            snapshots
                .lock()
                .map(|records| records.clone())
                .unwrap_or_default()
        })
    };
    let decoded_page_count = {
        let page_tracker = Arc::clone(&page_tracker);
        Arc::new(move |pool: &str| {
            page_tracker
                .lock()
                .map(|tracker| tracker.count(pool))
                .unwrap_or_default()
        })
    };
    let mut options = AutomationOptions::new(pid, Arc::clone(&stop));
    options.full_update = mode.full_update();
    options.non_interactive = true;
    options.known_record_ids = known_record_ids;
    options.record_snapshot = Some(snapshot_callback);
    options.decoded_page_count = Some(decoded_page_count);
    options.on_status = Some(auto_status_callback);
    let auto_result = run_auto_page(options);
    let drain_error = auto_result
        .succeeded()
        .then(|| wait_for_capture_drain(&runtime, &page_tracker, &auto_result, &stop))
        .flatten();
    stop.store(true, Ordering::SeqCst);

    let capture_result = capture_handle
        .join()
        .map_err(|_| "capture worker panicked".to_string())
        .and_then(|result| result);
    let auto_page = Some(auto_page_result_value(&auto_result));
    let error = if auto_result.succeeded() {
        drain_error
    } else {
        Some(RuntimeError {
            code: "auto_page_failed".to_string(),
            message: auto_result.message.clone(),
        })
    };
    finish_capture_result(
        &runtime,
        capture_result,
        &locale,
        "pktmon-auto-page-capture",
        auto_page,
        error,
    );
}

