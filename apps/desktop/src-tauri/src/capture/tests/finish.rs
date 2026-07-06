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
