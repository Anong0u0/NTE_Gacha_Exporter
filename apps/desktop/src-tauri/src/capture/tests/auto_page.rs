#[test]
fn auto_page_coordinator_skips_at_six_known_records_in_safe_prefix() {
    let known = (1..=6)
        .map(|index| format!("old-{index}"))
        .collect::<Vec<_>>();
    let coordinator = AutoPageCoordinator::new(false, &known);
    let mut records = capture_page(
        "standard",
        "monopoly:standard",
        0,
        0,
        ["new-1", "new-2", "new-3", "new-4", "old-1"],
    );
    records.extend(capture_page(
        "standard",
        "monopoly:standard",
        0,
        1,
        ["old-2", "old-3", "old-4", "old-5", "old-6"],
    ));

    coordinator.add_progress(
        &progress_with_rows(vec![
            parsed_row("CardPool_NewRole", 0),
            parsed_row("CardPool_NewRole", 1),
        ]),
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
fn auto_page_coordinator_waits_for_third_page_after_five_new_records() {
    let known = (1..=10)
        .map(|index| format!("old-{index}"))
        .collect::<Vec<_>>();
    let coordinator = AutoPageCoordinator::new(false, &known);
    let mut records = capture_page(
        "standard",
        "monopoly:standard",
        0,
        0,
        ["new-1", "new-2", "new-3", "new-4", "new-5"],
    );
    records.extend(capture_page(
        "standard",
        "monopoly:standard",
        0,
        1,
        known.iter().take(5),
    ));
    coordinator.add_progress(&progress_with_rows(Vec::new()), Some(&records));

    assert_eq!(
        coordinator.decision(control_context("standard", 2)),
        AutoPageControlDecision::Continue
    );

    records.extend(capture_page(
        "standard",
        "monopoly:standard",
        0,
        2,
        known.iter().skip(5),
    ));
    coordinator.add_progress(&progress_with_rows(Vec::new()), Some(&records));

    assert_eq!(
        coordinator.decision(control_context("standard", 3)),
        AutoPageControlDecision::SkipPool {
            duplicate_records: 10,
        }
    );
}

#[test]
fn auto_page_coordinator_skips_when_first_two_pages_are_known() {
    let known = (1..=10)
        .map(|index| format!("old-{index}"))
        .collect::<Vec<_>>();
    let coordinator = AutoPageCoordinator::new(false, &known);
    let mut records = capture_page(
        "standard",
        "monopoly:standard",
        0,
        0,
        known.iter().take(5),
    );
    records.extend(capture_page(
        "standard",
        "monopoly:standard",
        0,
        1,
        known.iter().skip(5),
    ));
    coordinator.add_progress(&progress_with_rows(Vec::new()), Some(&records));

    assert_eq!(
        coordinator.decision(control_context("standard", 2)),
        AutoPageControlDecision::SkipPool {
            duplicate_records: 10,
        }
    );
}

#[test]
fn auto_page_coordinator_ignores_known_pages_before_segment_zero() {
    let known = (1..=10)
        .map(|index| format!("old-{index}"))
        .collect::<Vec<_>>();
    let coordinator = AutoPageCoordinator::new(false, &known);
    let mut records = capture_page(
        "standard",
        "monopoly:standard",
        0,
        4,
        known.iter().take(5),
    );
    records.extend(capture_page(
        "standard",
        "monopoly:standard",
        0,
        5,
        known.iter().skip(5),
    ));
    coordinator.add_progress(&progress_with_rows(Vec::new()), Some(&records));

    assert_eq!(
        coordinator.decision(control_context("standard", 6)),
        AutoPageControlDecision::Continue
    );
}

#[test]
fn auto_page_coordinator_excludes_records_after_gap_until_filled() {
    let known = (1..=15)
        .map(|index| format!("old-{index}"))
        .collect::<Vec<_>>();
    let coordinator = AutoPageCoordinator::new(false, &known);
    let mut records = capture_page(
        "standard",
        "monopoly:standard",
        0,
        0,
        known.iter().take(5),
    );
    records.extend(capture_page(
        "standard",
        "monopoly:standard",
        0,
        2,
        known.iter().skip(10),
    ));
    coordinator.add_progress(&progress_with_rows(Vec::new()), Some(&records));

    assert_eq!(
        coordinator.decision(control_context("standard", 3)),
        AutoPageControlDecision::Continue
    );

    records.splice(
        5..5,
        capture_page(
            "standard",
            "monopoly:standard",
            0,
            1,
            known.iter().skip(5).take(5),
        ),
    );
    coordinator.add_progress(&progress_with_rows(Vec::new()), Some(&records));

    assert_eq!(
        coordinator.decision(control_context("standard", 3)),
        AutoPageControlDecision::SkipPool {
            duplicate_records: 15,
        }
    );
}

#[test]
fn auto_page_coordinator_waits_for_segment_zero_before_counting() {
    let known = (1..=5)
        .map(|index| format!("old-{index}"))
        .collect::<Vec<_>>();
    let coordinator = AutoPageCoordinator::new(false, &known);
    let page_two = capture_page(
        "standard",
        "monopoly:standard",
        0,
        1,
        &known,
    );
    coordinator.add_progress(&progress_with_rows(Vec::new()), Some(&page_two));

    assert_eq!(
        coordinator.decision(control_context("standard", 2)),
        AutoPageControlDecision::Continue
    );

    let mut records = capture_page(
        "standard",
        "monopoly:standard",
        0,
        0,
        ["new-1", "new-2", "new-3", "new-4", "new-5"],
    );
    records.extend(page_two);
    coordinator.add_progress(&progress_with_rows(Vec::new()), Some(&records));

    assert_eq!(
        coordinator.decision(control_context("standard", 2)),
        AutoPageControlDecision::Continue
    );
}

#[test]
fn auto_page_coordinator_uses_only_latest_generation() {
    let known = (1..=15)
        .map(|index| format!("old-{index}"))
        .collect::<Vec<_>>();
    let coordinator = AutoPageCoordinator::new(false, &known);
    let mut records = capture_page(
        "standard",
        "monopoly:standard",
        0,
        0,
        known.iter().take(5),
    );
    records.extend(capture_page(
        "standard",
        "monopoly:standard",
        0,
        1,
        known.iter().skip(5).take(5),
    ));
    records.extend(capture_page(
        "standard",
        "monopoly:standard",
        1,
        2,
        known.iter().skip(10),
    ));
    coordinator.add_progress(&progress_with_rows(Vec::new()), Some(&records));

    assert_eq!(
        coordinator.decision(control_context("standard", 3)),
        AutoPageControlDecision::Continue
    );

    records.truncate(10);
    records.extend(capture_page(
        "standard",
        "monopoly:standard",
        1,
        0,
        ["new-1", "new-2", "new-3", "new-4", "old-5"],
    ));
    records.extend(capture_page(
        "standard",
        "monopoly:standard",
        1,
        1,
        ["old-6", "old-7", "old-8", "old-9", "old-10"],
    ));
    records.extend(capture_page(
        "standard",
        "monopoly:standard",
        1,
        2,
        known.iter().skip(10),
    ));
    coordinator.add_progress(&progress_with_rows(Vec::new()), Some(&records));

    assert_eq!(
        coordinator.decision(control_context("standard", 3)),
        AutoPageControlDecision::SkipPool {
            duplicate_records: 11,
        }
    );
}

#[test]
fn auto_page_coordinator_does_not_join_known_runs_across_streams() {
    let known = (1..=10)
        .map(|index| format!("old-{index}"))
        .collect::<Vec<_>>();
    let coordinator = AutoPageCoordinator::new(false, &known);
    let mut records = capture_page(
        "standard",
        "monopoly:standard:a",
        0,
        0,
        known.iter().take(5),
    );
    records.extend(capture_page(
        "standard",
        "monopoly:standard:b",
        0,
        0,
        known.iter().skip(5),
    ));
    coordinator.add_progress(&progress_with_rows(Vec::new()), Some(&records));

    assert_eq!(
        coordinator.decision(control_context("standard", 2)),
        AutoPageControlDecision::Continue
    );
}

#[test]
fn auto_page_coordinator_never_early_skips_full_update() {
    let known = (1..=10)
        .map(|index| format!("old-{index}"))
        .collect::<Vec<_>>();
    let coordinator = AutoPageCoordinator::new(true, &known);
    let mut records = capture_page(
        "standard",
        "monopoly:standard",
        0,
        0,
        known.iter().take(5),
    );
    records.extend(capture_page(
        "standard",
        "monopoly:standard",
        0,
        1,
        known.iter().skip(5),
    ));
    coordinator.add_progress(&progress_with_rows(Vec::new()), Some(&records));

    assert_eq!(
        coordinator.decision(control_context("standard", 2)),
        AutoPageControlDecision::Continue
    );
}

#[test]
fn capture_record_snapshot_preserves_missing_source_metadata() {
    let record = nte_capture::CapturePublicRecord {
        record_id: "record-id".to_string(),
        record_key: "record-key".to_string(),
        record_type: "monopoly".to_string(),
        pool_id: Some("CardPool_NewRole".to_string()),
        value: json!({}),
    };
    let mut row = parsed_row("CardPool_NewRole", 0);
    row.source.stream_key = None;

    let snapshot = capture_record_snapshot(&record, &row).expect("supported pool");
    assert_eq!(snapshot.stream_key, None);

    row.source.stream_key = Some("monopoly:standard".to_string());
    row.source.generation_index = None;
    let snapshot = capture_record_snapshot(&record, &row).expect("supported pool");
    assert_eq!(snapshot.generation_index, None);

    row.source.generation_index = Some(0);
    row.source.segment_index = None;
    let snapshot = capture_record_snapshot(&record, &row).expect("supported pool");
    assert_eq!(snapshot.segment_index, None);
}

#[test]
fn legacy_record_is_retained_as_unsafe_metadata() {
    let records = vec![
        nte_capture::CapturePublicRecord {
            record_id: "valid-id".to_string(),
            record_key: "valid-key".to_string(),
            record_type: "monopoly".to_string(),
            pool_id: Some("CardPool_NewRole".to_string()),
            value: json!({}),
        },
        nte_capture::CapturePublicRecord {
            record_id: "legacy-id".to_string(),
            record_key: "legacy-key".to_string(),
            record_type: "monopoly".to_string(),
            pool_id: Some("CardPool_NewRole".to_string()),
            value: json!({}),
        },
    ];
    let valid = parsed_row("CardPool_NewRole", 0);
    let mut legacy = parsed_row("CardPool_NewRole", 1);
    legacy.source.stream_key = None;
    legacy.source.generation_index = None;
    legacy.source.segment_index = None;

    let snapshots = capture_record_snapshots(&records, &[valid, legacy]);

    assert_eq!(snapshots.len(), 2);
    assert_eq!(snapshots[1].stream_key, None);
    assert_eq!(snapshots[1].generation_index, None);
    assert_eq!(snapshots[1].segment_index, None);
}

#[test]
fn auto_page_coordinator_rejects_pool_with_missing_metadata() {
    let known = (1..=6)
        .map(|index| format!("old-{index}"))
        .collect::<Vec<_>>();
    let coordinator = AutoPageCoordinator::new(false, &known);
    let mut records = capture_page(
        "standard",
        "monopoly:standard",
        0,
        0,
        known.iter().take(5),
    );
    records.extend(capture_page(
        "standard",
        "monopoly:standard",
        0,
        1,
        known.iter().skip(5),
    ));
    records.push(CaptureRecordSnapshot {
        record_key: "legacy".to_string(),
        pool: "standard".to_string(),
        stream_key: None,
        generation_index: None,
        segment_index: None,
    });
    coordinator.add_progress(&progress_with_rows(Vec::new()), Some(&records));

    assert_eq!(
        coordinator.decision(control_context("standard", 2)),
        AutoPageControlDecision::Continue
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
fn auto_page_coordinator_requires_exact_count_when_pool_completes() {
    let coordinator = AutoPageCoordinator::new(true, &[]);
    coordinator.add_progress(
        &progress_with_rows(
            (0..55)
                .map(|index| parsed_row("CardPool_NewRole", index))
                .collect(),
        ),
        None,
    );

    assert_eq!(
        coordinator.decision(pool_complete_context("standard", 59)),
        AutoPageControlDecision::WaitCapture {
            decoded_pages: 55,
            max_visited_pages: 55,
        }
    );
    assert_eq!(
        coordinator.decision(pool_complete_context("standard", 55)),
        AutoPageControlDecision::Continue
    );
}

#[test]
fn auto_page_coordinator_does_not_skip_completed_pool() {
    let known = (1..=6)
        .map(|index| format!("old-{index}"))
        .collect::<Vec<_>>();
    let coordinator = AutoPageCoordinator::new(false, &known);
    let mut records = capture_page(
        "standard",
        "monopoly:standard",
        0,
        0,
        known.iter().take(5),
    );
    records.extend(capture_page(
        "standard",
        "monopoly:standard",
        0,
        1,
        known.iter().skip(5),
    ));
    coordinator.add_progress(
        &progress_with_rows(vec![parsed_row("CardPool_NewRole", 0)]),
        Some(&records),
    );

    assert_eq!(
        coordinator.decision(pool_complete_context("standard", 1)),
        AutoPageControlDecision::Continue
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
fn pool_complete_stall_reuses_capture_window_error_code() {
    let error = auto_page_runtime_error(
        "capture window stalled: pool=standard phase=pool_complete visited_pages=59 decoded_pages=55 max_visited_pages=55",
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
    let auto_result = AutoPageRunResult {
        outcome: AutoPageOutcome::Completed,
        message: "auto page completed".to_string(),
        reason: None,
        completed_pools: Vec::new(),
        skipped_pools: vec!["standard".to_string()],
        visited_pages_by_pool: visited,
        last_page_by_pool: last,
        diagnostics: AutoPageDiagnostics::default(),
    };

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
