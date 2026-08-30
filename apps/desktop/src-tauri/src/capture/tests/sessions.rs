#[test]
fn prune_capture_session_maps_keeps_active_and_preserved_sessions() {
    let mut sessions = HashMap::from([
        (
            "active".to_string(),
            test_session(test_status("active", "running", 1.0)),
        ),
        (
            "preserve".to_string(),
            test_session(test_status("preserve", "completed", 1.0)),
        ),
        (
            "old".to_string(),
            test_session(test_status("old", "failed", 1.0)),
        ),
    ]);
    let mut captures = HashMap::from([
        ("active".to_string(), test_meta()),
        ("preserve".to_string(), test_meta()),
        ("old".to_string(), test_meta()),
    ]);

    prune_capture_session_maps(&mut sessions, &mut captures, "preserve", 2_000.0);

    assert!(sessions.contains_key("active"));
    assert!(sessions.contains_key("preserve"));
    assert!(!sessions.contains_key("old"));
    assert!(!captures.contains_key("old"));
}

#[test]
fn prune_capture_session_maps_retains_latest_terminal_limit() {
    let mut sessions = HashMap::new();
    let mut captures = HashMap::new();
    for index in 0..25 {
        let session_id = format!("s{index:02}");
        sessions.insert(
            session_id.clone(),
            test_session(test_status(&session_id, "completed", f64::from(index))),
        );
        captures.insert(session_id, test_meta());
    }

    prune_capture_session_maps(&mut sessions, &mut captures, "s24", 100.0);

    assert_eq!(sessions.len(), 21);
    assert!(sessions.contains_key("s24"));
    assert!(!sessions.contains_key("s00"));
    assert!(!sessions.contains_key("s03"));
    assert!(sessions.contains_key("s04"));
}

#[test]
fn prune_capture_session_maps_treats_cancelled_as_terminal() {
    let mut sessions = HashMap::from([(
        "cancelled".to_string(),
        test_session(test_status(
            "cancelled",
            crate::lifecycle::STATE_CANCELLED,
            1.0,
        )),
    )]);
    let mut captures = HashMap::from([("cancelled".to_string(), test_meta())]);

    prune_capture_session_maps(&mut sessions, &mut captures, "other", 2_000.0);

    assert!(!sessions.contains_key("cancelled"));
    assert!(!captures.contains_key("cancelled"));
}

#[test]
fn prune_capture_session_maps_treats_partial_as_terminal() {
    let mut sessions = HashMap::from([(
        "partial".to_string(),
        test_session(test_status(
            "partial",
            crate::lifecycle::STATE_PARTIAL,
            1.0,
        )),
    )]);
    let mut captures = HashMap::from([("partial".to_string(), test_meta())]);

    prune_capture_session_maps(&mut sessions, &mut captures, "other", 2_000.0);

    assert!(!sessions.contains_key("partial"));
    assert!(!captures.contains_key("partial"));
}

#[test]
fn merge_gate_accepts_completed_and_partial_only() {
    assert!(capture_state_can_merge(crate::lifecycle::STATE_COMPLETED));
    assert!(capture_state_can_merge(crate::lifecycle::STATE_PARTIAL));
    assert!(!capture_state_can_merge(crate::lifecycle::STATE_FAILED));
    assert!(!capture_state_can_merge(crate::lifecycle::STATE_CANCELLED));
}

#[test]
fn capture_import_failure_is_terminal_without_saved_document() {
    let mut status = test_status("partial-import-failure", crate::lifecycle::STATE_PARTIAL, 1.0);
    status.records_count = 155;
    status.latest_records = vec![json!({ "record_id": "latest" })];
    status.counters.decoded_packets = 155;
    status.target = Some(json!({ "interface": "pktmon" }));
    status.auto_page = Some(json!({ "state": "partial" }));
    status.document = Some(json!({ "nte": { "list": [] } }));

    apply_capture_import_failure(
        &mut status,
        &api_error_message("internal_error", "records write failed"),
    );

    assert_eq!(status.state, crate::lifecycle::STATE_FAILED);
    assert_eq!(
        status.error.as_ref().map(|error| error.code.as_str()),
        Some("capture_import_failed")
    );
    assert_eq!(
        status.error.as_ref().map(|error| error.message.as_str()),
        Some("records write failed")
    );
    assert!(status.document.is_none());
    assert!(status.import_report.is_none());
    assert!(status.latest_records.is_empty());
    assert_eq!(status.records_count, 155);
    assert_eq!(status.counters.decoded_packets, 155);
    assert_eq!(status.target, Some(json!({ "interface": "pktmon" })));
    assert_eq!(status.auto_page, Some(json!({ "state": "partial" })));
}

#[test]
fn partial_merge_imports_only_safe_pool_records_and_is_idempotent() {
    let tmp = tempfile::tempdir().unwrap();
    let store = nte_store::JsonStore::open(tmp.path()).unwrap();
    let mut existing = capture_result("test", "port_filtered", "default", 1, 12);
    existing.rows = vec![parsed_row("CardPool_NewRole", 100)];
    let existing_text =
        serde_json::to_string(&build_capture_document(&existing.rows, "en").unwrap()).unwrap();
    store
        .import_public_document("default", &existing_text, "test", None)
        .unwrap();

    let mut safe_row = parsed_row("CardPool_Character", 10);
    safe_row.lottery_id = Some("Lottery_Unknown".to_string());
    let unsafe_rows = (0..6)
        .map(|index| parsed_row("CardPool_NewRole", index))
        .collect::<Vec<_>>();
    let mut partial_rows = vec![safe_row.clone()];
    partial_rows.extend(unsafe_rows.clone());
    let mut auto_result = auto_page_result(AutoPageOutcome::Partial);
    auto_result.completed_pools = vec!["limited".to_string()];
    let mergeable_rows = mergeable_auto_page_rows(&partial_rows, &auto_result);
    assert_eq!(mergeable_rows.len(), 1);
    let partial_text =
        serde_json::to_string(&build_capture_document(&mergeable_rows, "en").unwrap()).unwrap();
    let mut status = test_status("partial-merge", crate::lifecycle::STATE_PARTIAL, 1.0);
    let mut meta = test_meta();

    merge_capture_document_with_store(&store, &mut status, &mut meta, &partial_text).unwrap();

    let report = status.import_report.as_ref().unwrap();
    assert_eq!(report.records_inserted, 1);
    assert_eq!(report.records_skipped, 0);
    assert_eq!(meta.import_report.as_ref().unwrap().records_inserted, 1);
    assert_eq!(
        store
            .list_records("default", "en", &nte_core::RecordFilter::default())
            .unwrap()
            .total,
        2
    );

    let mut builder = CaptureRecordBuilder::new("en").unwrap();
    let safe_record = builder.build_record(&safe_row);
    let unsafe_records = builder.build_records(&unsafe_rows);
    let known_keys = store.profile_record_keys("default").unwrap();
    assert!(known_keys.contains(&safe_record.record_key));
    for record in &unsafe_records {
        assert!(!known_keys.contains(&record.record_key));
    }

    let snapshots = unsafe_records
        .iter()
        .enumerate()
        .map(|(index, record)| {
            capture_record(
                &record.record_key,
                "standard",
                "monopoly:CardPool_NewRole",
                0,
                index as u32,
            )
        })
        .collect::<Vec<_>>();
    let coordinator = AutoPageCoordinator::new(false, &known_keys);
    coordinator.add_progress(&progress_with_rows(Vec::new()), Some(&snapshots));
    assert_eq!(
        coordinator.decision(control_context("standard", 5)),
        AutoPageControlDecision::Continue
    );

    status.import_report = None;
    merge_capture_document_with_store(&store, &mut status, &mut meta, &partial_text).unwrap();

    assert_eq!(status.import_report.as_ref().unwrap().records_inserted, 1);
    assert_eq!(
        store
            .list_records("default", "en", &nte_core::RecordFilter::default())
            .unwrap()
            .total,
        2
    );
}

#[test]
fn full_update_partial_merge_keeps_existing_records_and_creates_backup() {
    let tmp = tempfile::tempdir().unwrap();
    let store = nte_store::JsonStore::open(tmp.path()).unwrap();
    let existing = capture_result("test", "port_filtered", "default", 1, 12);
    let existing_text = serde_json::to_string(&build_capture_document(&existing.rows, "en").unwrap()).unwrap();
    store
        .import_public_document("default", &existing_text, "test", None)
        .unwrap();

    let partial = capture_result("test", "port_filtered", "default", 2, 12);
    let partial_text = serde_json::to_string(&build_capture_document(&partial.rows, "en").unwrap()).unwrap();
    let mut status = test_status("full-partial-merge", crate::lifecycle::STATE_PARTIAL, 1.0);
    let mut meta = test_meta();
    meta.full_update = true;

    merge_capture_document_with_store(&store, &mut status, &mut meta, &partial_text).unwrap();

    let report = status.import_report.as_ref().unwrap();
    assert_eq!(report.records_inserted, 1);
    assert_eq!(report.records_skipped, 1);
    assert_eq!(
        store
            .list_records("default", "en", &nte_core::RecordFilter::default())
            .unwrap()
            .total,
        2
    );
    let backup_count = std::fs::read_dir(tmp.path().join("data/backups"))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "zip"))
        .count();
    assert_eq!(backup_count, 1);
}

#[test]
fn request_capture_stop_sets_session_and_attempt_flags() {
    let session = test_session(test_status("active", crate::lifecycle::STATE_RUNNING, 1.0));
    let attempt_stop = Arc::new(AtomicBool::new(false));
    *session.attempt_stop.lock().unwrap() = Some(Arc::clone(&attempt_stop));

    request_capture_stop(&session);

    assert!(session.stop.load(Ordering::SeqCst));
    assert!(attempt_stop.load(Ordering::SeqCst));
    assert_eq!(
        session.status.lock().unwrap().state,
        crate::lifecycle::STATE_STOPPING
    );
}

#[test]
fn set_attempt_stop_inherits_existing_session_stop() {
    let session = test_session(test_status("active", crate::lifecycle::STATE_RUNNING, 1.0));
    session.stop.store(true, Ordering::SeqCst);
    let attempt_stop = Arc::new(AtomicBool::new(false));

    set_attempt_stop(&session, Some(Arc::clone(&attempt_stop)));

    assert!(attempt_stop.load(Ordering::SeqCst));
}

#[test]
fn join_capture_shutdown_consumes_worker_handle() {
    let session = test_session(test_status("active", crate::lifecycle::STATE_RUNNING, 1.0));
    *session.handle.lock().unwrap() = Some(std::thread::spawn(|| {}));

    join_capture_shutdown(std::slice::from_ref(&session));

    assert!(session.handle.lock().unwrap().is_none());
}
