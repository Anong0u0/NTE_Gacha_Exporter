fn wait_for_capture_drain(
    runtime: &Arc<CaptureRuntimeSession>,
    page_tracker: &Arc<Mutex<CapturePageTracker>>,
    auto_result: &AutoPageRunResult,
    stop: &Arc<AtomicBool>,
) -> Option<RuntimeError> {
    let required = auto_result.visited_pages_by_pool.clone();
    if required.is_empty() {
        return None;
    }

    let started = Instant::now();
    loop {
        if stop.load(Ordering::SeqCst) {
            return Some(RuntimeError {
                code: "capture_stopped".to_string(),
                message: "capture stopped before packet drain completed".to_string(),
            });
        }

        let (decoded, last_decoded_at) = page_tracker
            .lock()
            .map(|tracker| (tracker.counts(), tracker.last_decoded_at))
            .unwrap_or_default();
        let missing = missing_capture_pages(&required, &decoded);
        if missing.is_empty() {
            return None;
        }

        if started.elapsed() >= CAPTURE_DRAIN_TIMEOUT {
            return Some(RuntimeError {
                code: "capture_incomplete".to_string(),
                message: format!(
                    "capture drain timed out: required={required:?} decoded={decoded:?}"
                ),
            });
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
    snapshots: Option<Arc<Mutex<Vec<AutomationRecordSnapshot>>>>,
    page_tracker: Option<Arc<Mutex<CapturePageTracker>>>,
) -> Arc<dyn Fn(nte_capture::CaptureProgress) + Send + Sync + 'static> {
    let progress_state = Arc::new(Mutex::new(LiveProgressState::new(&locale)));
    Arc::new(move |progress: nte_capture::CaptureProgress| {
        if let Some(page_tracker) = &page_tracker {
            if let Ok(mut tracker) = page_tracker.lock() {
                tracker.add_progress(&progress);
            }
        }
        let (records_count, latest, snapshot_delta) = progress_state
            .lock()
            .map(|mut state| {
                let snapshot_delta = state.apply(&progress);
                let records_count = if state.records.is_empty() {
                    progress.row_count as u64
                } else {
                    state.records.len() as u64
                };
                (
                    records_count,
                    latest_records(&state.records),
                    snapshot_delta,
                )
            })
            .unwrap_or_else(|_| (progress.row_count as u64, Vec::new(), None));
        if let Some(snapshot_delta) = snapshot_delta {
            if let Some(snapshots) = &snapshots {
                if let Ok(mut snapshot_records) = snapshots.lock() {
                    snapshot_records.extend(snapshot_delta);
                }
            }
        }
        if let Ok(mut status) = runtime.status.lock() {
            status.state = if status.state == "stopping" {
                "stopping".to_string()
            } else {
                "running".to_string()
            };
            status.records_count = records_count;
            status.latest_records = latest;
            status.counters = CaptureCounters::from(progress.counters);
            status.target = serde_json::to_value(progress.target).ok();
            status.updated_at = now_seconds();
        }
    })
}

