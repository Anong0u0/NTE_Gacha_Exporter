fn wait_for_capture_drain(
    runtime: &Arc<CaptureRuntimeSession>,
    coordinator: &Arc<AutoPageCoordinator>,
    auto_result: &AutoPageRunResult,
    stop: &Arc<AtomicBool>,
) -> Option<RuntimeError> {
    let mut required = auto_result.visited_pages_by_pool.clone();
    for pool in &auto_result.skipped_pools {
        required.remove(pool);
    }
    if required.is_empty() {
        return None;
    }

    let started = Instant::now();
    loop {
        if stop.load(Ordering::SeqCst) {
            return Some(runtime_error(
                "capture_stopped",
                "capture stopped before packet drain completed",
            ));
        }

        let decoded = coordinator.counts();
        let last_decoded_at = coordinator.last_decoded_at();
        let missing = missing_capture_pages(&required, &decoded);
        if missing.is_empty() {
            return None;
        }

        if started.elapsed() >= CAPTURE_DRAIN_TIMEOUT {
            return Some(runtime_error(
                "capture_incomplete",
                format!(
                    "capture drain timed out: required={required:?} decoded={decoded:?}"
                ),
            ));
        }

        if let Ok(mut status) = runtime.status.lock() {
            if status.state != "stopping" {
                status.state = "running".to_string();
            }
            status.auto_page = Some(json!({
                "state": "draining",
                "message": "waiting for capture drain",
                "kind": "draining",
                "required_pages_by_pool": required.clone(),
                "decoded_pages_by_pool": decoded.clone(),
                "missing_pages_by_pool": missing.clone(),
                "last_decoded_at": last_decoded_at,
            }));
            status.updated_at = now_seconds();
        }
        std::thread::sleep(CAPTURE_DRAIN_POLL_INTERVAL);
    }
}

fn missing_capture_pages(
    required: &BTreeMap<String, u32>,
    decoded: &BTreeMap<String, usize>,
) -> BTreeMap<String, u32> {
    required
        .iter()
        .filter_map(|(pool, required_count)| {
            let decoded_count = decoded.get(pool).copied().unwrap_or_default() as u32;
            (decoded_count < *required_count)
                .then(|| (pool.clone(), required_count - decoded_count))
        })
        .collect()
}

fn capture_progress_callback(
    runtime: Arc<CaptureRuntimeSession>,
    locale: String,
    coordinator: Option<Arc<AutoPageCoordinator>>,
) -> Arc<dyn Fn(nte_capture::CaptureProgress) + Send + Sync + 'static> {
    let progress_state = Arc::new(Mutex::new(LiveProgressState::new(&locale)));
    Arc::new(move |progress: nte_capture::CaptureProgress| {
        let update = progress_state
            .lock()
            .map(|mut state| state.apply(&progress))
            .unwrap_or_else(|_| LiveProgressUpdate {
                records_count: progress.row_count as u64,
                latest: Vec::new(),
                automation_snapshot: None,
            });
        if let Some(coordinator) = &coordinator {
            coordinator.add_progress(&progress, update.automation_snapshot.as_deref());
        }
        if let Ok(mut status) = runtime.status.lock() {
            status.state = if status.state == "stopping" {
                "stopping".to_string()
            } else {
                "running".to_string()
            };
            status.records_count = update.records_count;
            status.latest_records = update.latest;
            status.counters = CaptureCounters::from(progress.counters);
            status.target = serde_json::to_value(progress.target).ok();
            status.updated_at = now_seconds();
        }
    })
}
