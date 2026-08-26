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
        true,
    );
    let mut status = test_status("completed-stop", "running", 1.0);

    apply_capture_finish_status(&mut status, finish, None);

    assert_eq!(status.state, crate::lifecycle::STATE_COMPLETED);
    assert!(status.error.is_none());
    assert!(status.document.is_some());
}

#[test]
fn partial_auto_page_with_records_and_successful_drain_completes() {
    let auto_result = auto_page_result(AutoPageOutcome::Partial);

    let error = auto_page_terminal_error(&auto_result, None, true);
    let finish = classify_capture_finish(
        Ok(capture_result("pktmon", "port_filtered", "default", 1, 12)),
        "en",
        "pktmon-auto-page-capture",
        error,
        false,
    );
    let mut status = test_status("partial-with-records", "running", 1.0);

    apply_capture_finish_status(
        &mut status,
        finish,
        Some(auto_page_result_value(&auto_result, None)),
    );

    assert_eq!(status.state, crate::lifecycle::STATE_COMPLETED);
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
    assert!(auto_page.get("capture_drain_warning").is_some());
}

#[test]
fn partial_auto_page_drain_timeout_preserves_observed_records() {
    let auto_result = auto_page_result(AutoPageOutcome::Partial);
    let drain_error = runtime_error("capture_incomplete", "drain timed out");
    let error = auto_page_terminal_error(&auto_result, Some(drain_error.clone()), true);
    let finish = classify_capture_finish(
        Ok(capture_result("pktmon", "port_filtered", "default", 1, 12)),
        "en",
        "pktmon-auto-page-capture",
        error,
        false,
    );
    let mut status = test_status("partial-drain-timeout", "running", 1.0);

    apply_capture_finish_status(
        &mut status,
        finish,
        Some(auto_page_result_value(
            &auto_result,
            Some(&drain_error),
        )),
    );

    assert_eq!(status.state, crate::lifecycle::STATE_COMPLETED);
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

fn auto_page_result(outcome: AutoPageOutcome) -> AutoPageRunResult {
    AutoPageRunResult {
        outcome,
        message: "auto page ended early".to_string(),
        reason: None,
        completed_pools: Vec::new(),
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

    assert!(windivert_capture_succeeded(&result, None));
}

#[test]
fn pktmon_success_does_not_enable_windivert_persistence() {
    let result: Result<_, String> =
        Ok(capture_result("pktmon", "port_filtered", "default", 1, 12));

    assert!(!windivert_capture_succeeded(&result, None));
}
