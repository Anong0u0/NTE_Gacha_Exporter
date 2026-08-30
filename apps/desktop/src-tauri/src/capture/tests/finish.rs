#[test]
fn pktmon_zero_decode_packets_reports_vpn_proxy_suspected() {
    let result = capture_result("pktmon", "port_filtered", "default", 0, 12);

    let error = zero_decode_runtime_error(&result).unwrap();

    assert_eq!(error.code, "vpn_proxy_suspected");
    assert!(error.message.contains("pktmon"));
}

#[test]
fn zero_packet_capture_reports_no_packets_seen() {
    let result = capture_result("pktmon", "port_filtered", "default", 0, 0);

    let error = zero_decode_runtime_error(&result).unwrap();

    assert_eq!(error.code, "no_packets_seen");
}

#[test]
fn windivert_zero_decode_packets_reports_windivert_no_decode() {
    let result = capture_result("windivert", "no_filter", "windivert_backend", 0, 12);

    let error = zero_decode_runtime_error(&result).unwrap();

    assert_eq!(error.code, "windivert_no_decode");
    assert!(error.message.contains("WinDivert"));
}

#[test]
fn auto_page_cancelled_zero_decode_does_not_report_recovery_error() {
    let finish = classify_capture_finish(
        Ok(capture_result("pktmon", "port_filtered", "default", 0, 12)),
        "en",
        "pktmon-auto-page-capture",
        None,
        None,
        true,
    );
    let mut status = test_status("cancelled-zero-decode", "running", 1.0);

    apply_capture_finish_status(&mut status, finish, None);

    assert_eq!(status.state, crate::lifecycle::STATE_CANCELLED);
    assert!(status.error.is_none());
    assert!(status.document.is_none());
    assert!(status.import_report.is_none());
}

#[test]
fn auto_page_cancelled_backend_error_prefers_cancelled_state() {
    let finish = classify_capture_finish(
        Err("pktmon exited with code 1".to_string()),
        "en",
        "pktmon-auto-page-capture",
        None,
        None,
        true,
    );
    let mut status = test_status("cancelled-backend-error", "running", 1.0);

    apply_capture_finish_status(&mut status, finish, None);

    assert_eq!(status.state, crate::lifecycle::STATE_CANCELLED);
    assert!(status.error.is_none());
    assert!(status.document.is_none());
    assert!(status.import_report.is_none());
}

#[test]
fn successful_result_completes_even_when_stop_was_requested() {
    let finish = classify_capture_finish(
        Ok(capture_result("pktmon", "port_filtered", "default", 1, 12)),
        "en",
        "pktmon-live-capture",
        None,
        None,
        true,
    );
    let mut status = test_status("completed-stop", "running", 1.0);

    apply_capture_finish_status(&mut status, finish, None);

    assert_eq!(status.state, crate::lifecycle::STATE_COMPLETED);
    assert!(status.error.is_none());
    assert!(status.document.is_some());
}

#[test]
fn partial_auto_page_with_records_is_partial() {
    let mut auto_result = auto_page_result(AutoPageOutcome::Partial);
    auto_result.completed_pools = vec!["standard".to_string()];

    let error = auto_page_terminal_error(&auto_result, None, true);
    let result = capture_result("pktmon", "port_filtered", "default", 1, 12);
    let finish = classify_capture_finish(
        Ok(result),
        "en",
        "pktmon-auto-page-capture",
        error,
        Some(&auto_result),
        false,
    );
    let mut status = test_status("partial-with-records", "running", 1.0);

    apply_capture_finish_status(
        &mut status,
        finish,
        Some(auto_page_result_value(
            &auto_result,
            &BTreeMap::from([("limited".to_string(), 54)]),
            None,
        )),
    );

    assert_eq!(status.state, crate::lifecycle::STATE_PARTIAL);
    assert!(status.error.is_none());
    assert!(status.document.is_some());
    let auto_page = status.auto_page.as_ref().unwrap();
    assert_eq!(auto_page.get("state"), Some(&json!("partial")));
    assert_eq!(auto_page.get("visited_pages"), Some(&json!(55)));
    assert_eq!(
        auto_page
            .get("visited_pages_by_pool")
            .and_then(|value| value.get("limited")),
        Some(&json!(55))
    );
    assert_eq!(
        auto_page
            .get("last_page_by_pool")
            .and_then(|value| value.get("limited")),
        Some(&json!(55))
    );
    assert!(auto_page.get("completed_pools").unwrap().is_array());
    assert!(auto_page.get("skipped_pools").unwrap().is_array());
    assert!(auto_page.get("reason").is_some());
    assert_eq!(auto_page.get("capture_drain_warning"), Some(&Value::Null));
}

#[test]
fn partial_auto_page_drain_timeout_preserves_observed_records() {
    let mut auto_result = auto_page_result(AutoPageOutcome::Partial);
    auto_result.completed_pools = vec!["standard".to_string()];
    let drain_error = runtime_error("capture_incomplete", "drain timed out");
    let error = auto_page_terminal_error(&auto_result, Some(drain_error.clone()), true);
    let result = capture_result("pktmon", "port_filtered", "default", 1, 12);
    let finish = classify_capture_finish(
        Ok(result),
        "en",
        "pktmon-auto-page-capture",
        error,
        Some(&auto_result),
        false,
    );
    let mut status = test_status("partial-drain-timeout", "running", 1.0);

    apply_capture_finish_status(
        &mut status,
        finish,
        Some(auto_page_result_value(
            &auto_result,
            &BTreeMap::from([("limited".to_string(), 54)]),
            Some(&drain_error),
        )),
    );

    assert_eq!(status.state, crate::lifecycle::STATE_PARTIAL);
    assert_eq!(status.records_count, 1);
    assert!(status.error.is_none());
    assert!(status.document.is_some());
    assert!(status.import_report.is_none());
    let auto_page = status.auto_page.as_ref().unwrap();
    assert_eq!(auto_page.get("state"), Some(&json!("partial")));
    assert_eq!(
        auto_page
            .get("capture_drain_warning")
            .and_then(|value| value.get("code")),
        Some(&json!("capture_incomplete"))
    );
}


#[test]
fn partial_auto_page_without_records_preserves_drain_error() {
    let auto_result = auto_page_result(AutoPageOutcome::Partial);

    let error = auto_page_terminal_error(
        &auto_result,
        Some(runtime_error("capture_incomplete", "drain timed out")),
        false,
    )
    .unwrap();

    assert_eq!(error.code, "capture_incomplete");
    assert_eq!(error.message, "drain timed out");
}

#[test]
fn stop_during_auto_page_with_only_unfinished_rows_is_cancelled() {
    let auto_result = auto_page_result(AutoPageOutcome::Cancelled);
    let finish = classify_capture_finish(
        Ok(capture_result("pktmon", "port_filtered", "default", 1, 12)),
        "en",
        "pktmon-auto-page-capture",
        None,
        Some(&auto_result),
        true,
    );
    let mut status = test_status("cancelled-user-stop", "running", 1.0);

    apply_capture_finish_status(
        &mut status,
        finish,
        Some(auto_page_result_value(
            &auto_result,
            &BTreeMap::new(),
            None,
        )),
    );

    assert_eq!(status.state, crate::lifecycle::STATE_CANCELLED);
    assert!(status.document.is_none());
    assert!(status.import_report.is_none());
    assert!(status.error.is_none());
}

#[test]
fn mid_pool_partial_with_only_unfinished_rows_is_failed() {
    let auto_result = auto_page_result(AutoPageOutcome::Partial);
    let finish = classify_capture_finish(
        Ok(capture_result("pktmon", "port_filtered", "default", 5, 12)),
        "en",
        "pktmon-auto-page-capture",
        None,
        Some(&auto_result),
        false,
    );
    let mut status = test_status("mid-pool-failed", "running", 1.0);

    apply_capture_finish_status(&mut status, finish, None);

    assert_eq!(status.state, crate::lifecycle::STATE_FAILED);
    assert_eq!(
        status.error.as_ref().map(|error| error.code.as_str()),
        Some(AUTO_PAGE_FAILED_CODE)
    );
    assert!(status.document.is_none());
    assert!(status.import_report.is_none());
}

#[test]
fn partial_auto_page_merges_completed_pool_only() {
    let mut auto_result = auto_page_result(AutoPageOutcome::Partial);
    auto_result.completed_pools = vec!["limited".to_string()];
    let mut result = capture_result("pktmon", "port_filtered", "default", 2, 12);
    let mut safe_row = parsed_row("CardPool_Character", 0);
    safe_row.lottery_id = Some("Lottery_Unknown".to_string());
    result.rows = vec![safe_row, parsed_row("CardPool_NewRole", 1)];
    let finish = classify_capture_finish(
        Ok(result),
        "en",
        "pktmon-auto-page-capture",
        None,
        Some(&auto_result),
        false,
    );
    let mut status = test_status("partial-safe-pool", "running", 1.0);

    apply_capture_finish_status(
        &mut status,
        finish,
        Some(auto_page_result_value(
            &auto_result,
            &BTreeMap::from([("limited".to_string(), 55)]),
            None,
        )),
    );

    assert_eq!(status.state, crate::lifecycle::STATE_PARTIAL);
    assert_eq!(status.records_count, 2);
    let records = status
        .document
        .as_ref()
        .and_then(|document| document.pointer("/nte/list"))
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(records.len(), 1);
}

#[test]
fn partial_auto_page_merges_skipped_pool_rows() {
    let mut auto_result = auto_page_result(AutoPageOutcome::Partial);
    auto_result.skipped_pools = vec!["standard".to_string()];
    let finish = classify_capture_finish(
        Ok(capture_result("pktmon", "port_filtered", "default", 1, 12)),
        "en",
        "pktmon-auto-page-capture",
        None,
        Some(&auto_result),
        false,
    );
    let mut status = test_status("partial-skipped-pool", "running", 1.0);

    apply_capture_finish_status(&mut status, finish, None);

    assert_eq!(status.state, crate::lifecycle::STATE_PARTIAL);
    assert!(status.document.is_some());
}

#[test]
fn stop_during_auto_page_after_completed_pool_saves_safe_rows() {
    let mut auto_result = auto_page_result(AutoPageOutcome::Cancelled);
    auto_result.completed_pools = vec!["limited".to_string()];
    let mut result = capture_result("pktmon", "port_filtered", "default", 2, 12);
    let mut safe_row = parsed_row("CardPool_Character", 0);
    safe_row.lottery_id = Some("Lottery_Unknown".to_string());
    result.rows = vec![safe_row, parsed_row("CardPool_NewRole", 1)];
    let finish = classify_capture_finish(
        Ok(result),
        "en",
        "pktmon-auto-page-capture",
        None,
        Some(&auto_result),
        true,
    );
    let mut status = test_status("partial-user-stop-safe", "running", 1.0);

    apply_capture_finish_status(
        &mut status,
        finish,
        Some(auto_page_result_value(&auto_result, &BTreeMap::new(), None)),
    );

    assert_eq!(status.state, crate::lifecycle::STATE_PARTIAL);
    assert_eq!(
        status.auto_page.as_ref().and_then(|value| value.pointer("/reason/code")),
        Some(&json!(USER_STOPPED_CODE))
    );
    let records = status
        .document
        .as_ref()
        .and_then(|document| document.pointer("/nte/list"))
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(records.len(), 1);
}

#[test]
fn partial_auto_page_without_records_is_failed() {
    let auto_result = auto_page_result(AutoPageOutcome::Partial);

    let error = auto_page_terminal_error(&auto_result, None, false);

    assert_eq!(error.unwrap().code, AUTO_PAGE_FAILED_CODE);
}

#[test]
fn completed_auto_page_preserves_drain_failure() {
    let auto_result = auto_page_result(AutoPageOutcome::Completed);

    let error = auto_page_terminal_error(
        &auto_result,
        Some(runtime_error("capture_incomplete", "drain timed out")),
        true,
    )
    .unwrap();

    assert_eq!(error.code, "capture_incomplete");
    assert_eq!(error.message, "drain timed out");
}

#[test]
fn stop_during_completed_auto_page_drain_preserves_records_as_partial() {
    let mut auto_result = auto_page_result(AutoPageOutcome::Completed);
    auto_result.visited_pages_by_pool = BTreeMap::from([
        ("limited".to_string(), 55),
        ("standard".to_string(), 60),
        ("fork".to_string(), 45),
    ]);
    auto_result.last_page_by_pool = auto_result.visited_pages_by_pool.clone();
    let drain_error = runtime_error(CAPTURE_STOPPED_CODE, "drain stopped");
    let result = capture_result("pktmon", "port_filtered", "default", 155, 12);
    let finish = classify_capture_finish(
        Ok(result),
        "en",
        "pktmon-auto-page-capture",
        Some(drain_error.clone()),
        Some(&auto_result),
        true,
    );
    let mut status = test_status("partial-drain-stop", "running", 1.0);
    let decoded_pages_by_pool = BTreeMap::from([
        ("limited".to_string(), 53),
        ("standard".to_string(), 59),
        ("fork".to_string(), 43),
    ]);

    apply_capture_finish_status(
        &mut status,
        finish,
        Some(auto_page_result_value(
            &auto_result,
            &decoded_pages_by_pool,
            capture_drain_warning_for_result(true, Some(&drain_error)),
        )),
    );

    assert_eq!(status.state, crate::lifecycle::STATE_PARTIAL);
    assert_eq!(status.records_count, 155);
    assert!(status.document.is_some());
    assert!(status.error.is_none());
    let auto_page = status.auto_page.as_ref().unwrap();
    assert_eq!(
        auto_page.pointer("/reason/code"),
        Some(&json!(CAPTURE_STOPPED_DURING_DRAIN_CODE))
    );
    assert_eq!(
        auto_page.pointer("/capture_drain_warning/code"),
        Some(&json!(CAPTURE_STOPPED_CODE))
    );
    assert_eq!(
        auto_page.pointer("/required_pages_by_pool/limited"),
        Some(&json!(55))
    );
    assert_eq!(
        auto_page.pointer("/decoded_pages_by_pool/standard"),
        Some(&json!(59))
    );
    assert_eq!(
        auto_page.pointer("/missing_pages_by_pool/fork"),
        Some(&json!(2))
    );
}

#[test]
fn partial_page_metrics_preserve_required_decoded_and_missing_counts() {
    let mut auto_result = auto_page_result(AutoPageOutcome::Completed);
    auto_result.visited_pages_by_pool = BTreeMap::from([
        ("limited".to_string(), 55),
        ("standard".to_string(), 60),
        ("fork".to_string(), 45),
    ]);
    let value = auto_page_result_value(
        &auto_result,
        &BTreeMap::from([
            ("limited".to_string(), 53),
            ("standard".to_string(), 59),
            ("fork".to_string(), 43),
        ]),
        None,
    );

    assert_eq!(
        value.pointer("/required_pages_by_pool/limited"),
        Some(&json!(55))
    );
    assert_eq!(
        value.pointer("/decoded_pages_by_pool/standard"),
        Some(&json!(59))
    );
    assert_eq!(
        value.pointer("/missing_pages_by_pool/fork"),
        Some(&json!(2))
    );
    assert_eq!(
        value
            .get("decoded_pages_by_pool")
            .and_then(Value::as_object)
            .unwrap()
            .values()
            .filter_map(Value::as_u64)
            .sum::<u64>(),
        155
    );
}

#[test]
fn capture_window_stall_skips_second_drain_wait() {
    let mut auto_result = auto_page_result(AutoPageOutcome::Partial);
    auto_result.reason = Some(nte_automation::AutoPageStopReason {
        code: AUTOMATION_INTERRUPTED_CODE.to_string(),
        message: "capture window stalled: pool=standard phase=pool_complete".to_string(),
    });

    assert!(!should_drain_auto_page_result(&auto_result));
}

#[test]
fn capture_document_failure_is_fatal_even_after_user_stop() {
    let finish = classify_capture_finish(
        Ok(capture_result("pktmon", "port_filtered", "default", 1, 12)),
        "missing-locale",
        "pktmon-auto-page-capture",
        Some(runtime_error(CAPTURE_STOPPED_CODE, "drain stopped")),
        Some(&auto_page_result(AutoPageOutcome::Completed)),
        true,
    );
    let mut status = test_status("document-failed", "running", 1.0);

    apply_capture_finish_status(&mut status, finish, None);

    assert_eq!(status.state, crate::lifecycle::STATE_FAILED);
    assert_eq!(
        status.error.as_ref().map(|error| error.code.as_str()),
        Some("capture_document_failed")
    );
    assert!(status.document.is_none());
}
fn auto_page_result(outcome: AutoPageOutcome) -> AutoPageRunResult {
    let completed_pools = if outcome == AutoPageOutcome::Completed {
        vec![
            "limited".to_string(),
            "standard".to_string(),
            "fork".to_string(),
        ]
    } else {
        Vec::new()
    };
    AutoPageRunResult {
        outcome,
        message: "auto page ended early".to_string(),
        reason: None,
        completed_pools,
        skipped_pools: Vec::new(),
        visited_pages_by_pool: BTreeMap::from([("limited".to_string(), 55)]),
        last_page_by_pool: BTreeMap::from([("limited".to_string(), 55)]),
        diagnostics: AutoPageDiagnostics::default(),
    }
}

#[test]
fn windivert_success_enables_windivert_persistence_check() {
    let result: Result<_, String> = Ok(capture_result(
        "windivert",
        "no_filter",
        "windivert_backend",
        1,
        12,
    ));

    assert!(windivert_capture_succeeded(&result));
}

#[test]
fn pktmon_success_does_not_enable_windivert_persistence() {
    let result: Result<_, String> =
        Ok(capture_result("pktmon", "port_filtered", "default", 1, 12));

    assert!(!windivert_capture_succeeded(&result));
}
