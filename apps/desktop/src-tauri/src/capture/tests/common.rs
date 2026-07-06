fn test_status(session_id: &str, state: &str, updated_at: f64) -> CaptureStatus {
    CaptureStatus {
        session_id: session_id.to_string(),
        state: state.to_string(),
        mode: "live_only".to_string(),
        records_count: 0,
        latest_records: Vec::new(),
        counters: CaptureCounters::default(),
        attempts: Vec::new(),
        started_at: updated_at,
        updated_at,
        target: None,
        auto_page: None,
        raw_path: None,
        error: None,
        document: None,
        import_report: None,
    }
}

fn failed_status(session_id: &str) -> CaptureStatus {
    let mut status = test_status(session_id, "failed", 1.0);
    status.mode = "auto_page_full".to_string();
    status.records_count = 1;
    status.latest_records = vec![json!({ "record_id": "private-record" })];
    status.raw_path = Some("data/runs/raw-private.jsonl".to_string());
    status.error = Some(runtime_error("auto_page_failed", "cannot read page number"));
    status
}

fn test_session(status: CaptureStatus) -> Arc<CaptureRuntimeSession> {
    Arc::new(CaptureRuntimeSession {
        status: Mutex::new(status),
        stop: Arc::new(AtomicBool::new(false)),
        attempt_stop: Mutex::new(None),
        handle: Mutex::new(None),
    })
}

fn test_meta() -> CaptureSessionMeta {
    CaptureSessionMeta {
        profile_name: "default".to_string(),
        source_kind: "test".to_string(),
        source_path: None,
        full_update: false,
        import_report: None,
    }
}

fn automation_record(
    record_key: &str,
    pool_id: &str,
    record_type: &str,
) -> AutomationRecordSnapshot {
    AutomationRecordSnapshot {
        record_id: record_key.to_string(),
        record_key: record_key.to_string(),
        pool_id: pool_id.to_string(),
        record_type: record_type.to_string(),
    }
}

fn control_context(pool: &str, visited_pages: u32) -> AutoPageControlContext {
    AutoPageControlContext {
        pool: pool.to_string(),
        step: "test".to_string(),
        current_page: visited_pages,
        total_pages: 50,
        visited_pages,
    }
}

fn progress_with_rows(rows: Vec<nte_capture::ParsedRow>) -> nte_capture::CaptureProgress {
    nte_capture::CaptureProgress {
        target: CaptureTarget {
            pid: 1,
            exe: "HTGame.exe".to_string(),
            interface: "test".to_string(),
            ports: Vec::new(),
            bpf: String::new(),
            capture_strategy: "port_filtered".to_string(),
            strategy_reason: "default".to_string(),
            pppoe_detection: nte_capture::PppoeDetection::default(),
            attempts: Vec::new(),
        },
        counters: nte_capture::CaptureCounters::default(),
        new_rows: rows.clone(),
        rows_snapshot: rows.clone(),
        row_count: rows.len(),
        warning_count: 0,
    }
}

fn capture_result(
    interface: &str,
    strategy: &str,
    reason: &str,
    rows_count: usize,
    packets_seen: u64,
) -> nte_capture::CaptureResult {
    let attempts = vec![nte_capture::CaptureAttemptSummary {
        attempt_index: 0,
        capture_strategy: strategy.to_string(),
        strategy_reason: reason.to_string(),
        started_at: 1.0,
        ended_at: 2.0,
        counters: nte_capture::CaptureCounters {
            packets_seen,
            ..Default::default()
        },
    }];
    nte_capture::CaptureResult {
        target: CaptureTarget {
            pid: 1,
            exe: "HTGame.exe".to_string(),
            interface: interface.to_string(),
            ports: Vec::new(),
            bpf: String::new(),
            capture_strategy: strategy.to_string(),
            strategy_reason: reason.to_string(),
            pppoe_detection: nte_capture::PppoeDetection::default(),
            attempts: attempts.clone(),
        },
        counters: nte_capture::CaptureCounters {
            packets_seen,
            ..Default::default()
        },
        attempts,
        rows: (0..rows_count)
            .map(|index| parsed_row("CardPool_NewRole", index as u32))
            .collect(),
        warnings: Vec::new(),
    }
}

fn parsed_row(pool_id: &str, segment_index: u32) -> nte_capture::ParsedRow {
    nte_capture::ParsedRow {
        record_type: nte_capture::RecordType::Monopoly,
        ticks: 639_175_144_000_000_000,
        time: Some("2026-06-20T00:00:00.000000".to_string()),
        pool_id: Some(pool_id.to_string()),
        item_id: format!("item-{segment_index}"),
        count: 1,
        roll_points: Some(segment_index),
        roll_label_id: None,
        secondary_item_id: None,
        secondary_count: None,
        source: nte_capture::SourceRef {
            session: 0,
            line: 1,
            packet_index: u64::from(segment_index),
            view: "test".to_string(),
            row_index: 0,
            offset: 0,
            stream_key: Some(format!("monopoly:{pool_id}")),
            page_index: Some(segment_index),
            query_high: Some(true),
            segment_index: Some(segment_index),
            generation_index: Some(0),
        },
    }
}
