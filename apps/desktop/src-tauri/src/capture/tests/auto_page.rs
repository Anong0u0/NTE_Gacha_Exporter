#[test]
fn auto_page_coordinator_skips_when_snapshot_has_known_run() {
    let known = (1..=6)
        .map(|index| format!("old-{index}"))
        .collect::<Vec<_>>();
    let coordinator = AutoPageCoordinator::new(false, &known);
    let records = vec![
        automation_record("new", "CardPool_NewRole", "monopoly"),
        automation_record("old-1", "CardPool_NewRole", "monopoly"),
        automation_record("old-2", "CardPool_NewRole", "monopoly"),
        automation_record("old-3", "CardPool_NewRole", "monopoly"),
        automation_record("old-4", "CardPool_NewRole", "monopoly"),
        automation_record("old-5", "CardPool_NewRole", "monopoly"),
        automation_record("old-6", "CardPool_NewRole", "monopoly"),
    ];

    coordinator.add_progress(
        &progress_with_rows(vec![parsed_row("CardPool_NewRole", 0)]),
        Some(&records),
    );

    assert_eq!(
        coordinator.decision(control_context("standard", 2)),
        AutoPageControlDecision::SkipPool {
            duplicate_records: 6,
        }
    );
}

#[test]
fn auto_page_coordinator_blocks_when_pager_exceeds_capture_window() {
    let coordinator = AutoPageCoordinator::new(true, &[]);
    coordinator.add_progress(
        &progress_with_rows(vec![parsed_row("CardPool_NewRole", 0)]),
        None,
    );

    assert_eq!(
        coordinator.decision(control_context("standard", 7)),
        AutoPageControlDecision::Continue
    );
    assert_eq!(
        coordinator.decision(control_context("standard", 8)),
        AutoPageControlDecision::WaitCapture {
            decoded_pages: 1,
            max_visited_pages: 7,
        }
    );
}

#[test]
fn auto_page_capture_window_stalled_has_specific_error_code() {
    let error = auto_page_runtime_error(
        "capture window stalled: pool=standard visited_pages=8 decoded_pages=1 max_visited_pages=7",
    );

    assert_eq!(error.code, AUTO_PAGE_CAPTURE_WINDOW_STALLED_CODE);
}

#[test]
fn other_auto_page_failures_keep_generic_error_code() {
    let error = auto_page_runtime_error("cannot read page number");

    assert_eq!(error.code, AUTO_PAGE_FAILED_CODE);
}

#[test]
fn capture_start_options_override_page_record_wait() {
    let stop = Arc::new(AtomicBool::new(false));
    let mut options = AutomationOptions::new(123, stop);

    apply_capture_start_options(
        &mut options,
        &CaptureStartOptions {
            page_record_min_wait_ms: Some(500),
            capture_backend: None,
        },
    );

    assert_eq!(options.page_record_min_wait, Duration::from_millis(500));
}

#[test]
fn capture_start_options_clamp_page_record_wait() {
    let stop = Arc::new(AtomicBool::new(false));
    let mut options = AutomationOptions::new(123, stop);

    apply_capture_start_options(
        &mut options,
        &CaptureStartOptions {
            page_record_min_wait_ms: Some(2000),
            capture_backend: None,
        },
    );

    assert_eq!(options.page_record_min_wait, Duration::from_millis(1500));
}

#[test]
fn auto_page_coordinator_replaces_page_counts_from_full_snapshot() {
    let coordinator = AutoPageCoordinator::new(true, &[]);
    coordinator.add_progress(
        &progress_with_rows(vec![
            parsed_row("CardPool_NewRole", 0),
            parsed_row("CardPool_NewRole", 1),
        ]),
        None,
    );
    coordinator.add_progress(
        &progress_with_rows(vec![parsed_row("CardPool_NewRole", 0)]),
        None,
    );

    assert_eq!(coordinator.counts().get("standard"), Some(&1));
}

#[test]
fn capture_drain_does_not_require_skipped_pool_pages() {
    let status = test_status("drain-skipped", "running", 1.0);
    let runtime = test_session(status);
    let coordinator = Arc::new(AutoPageCoordinator::new(false, &[]));
    let mut visited = BTreeMap::new();
    visited.insert("standard".to_string(), 10);
    let mut last = BTreeMap::new();
    last.insert("standard".to_string(), 10);
    let auto_result = AutoPageRunResult::completed_with_pages(
        Vec::new(),
        vec!["standard".to_string()],
        visited,
        last,
    );

    assert!(
        wait_for_capture_drain(
            &runtime,
            &coordinator,
            &auto_result,
            &runtime.stop,
            &Arc::new(AtomicBool::new(false)),
        )
        .is_none()
    );
}
