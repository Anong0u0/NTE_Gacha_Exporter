#[test]
fn windivert_backend_setting_selects_windivert_target_fields() {
    let detection = nte_capture::PppoeDetection::default();
    let backend = capture_backend_for_start(true, None);
    let strategy = capture_strategy_for_start(&detection, backend);

    assert_eq!(backend, CaptureBackend::WinDivert);
    assert_eq!(capture_interface(backend), "windivert");
    assert_eq!(
        capture_bpf(backend, CaptureStrategyKind::PortFiltered, &[30031]),
        "ip"
    );
    assert_eq!(capture_source_kind(backend, false), "windivert-live-capture");
    assert_eq!(
        capture_source_kind(backend, true),
        "windivert-auto-page-capture"
    );
    assert_eq!(strategy.kind, CaptureStrategyKind::NoFilter);
    assert_eq!(strategy.reason, CaptureStrategyReason::WinDivertBackend);
}

#[test]
fn pktmon_backend_setting_keeps_existing_strategy_rules() {
    let detection = nte_capture::PppoeDetection::default();
    let backend = capture_backend_for_start(false, None);
    let strategy = capture_strategy_for_start(&detection, backend);

    assert_eq!(backend, CaptureBackend::Pktmon);
    assert_eq!(capture_interface(backend), "pktmon");
    assert_eq!(
        capture_bpf(backend, CaptureStrategyKind::PortFiltered, &[30031, 30230]),
        "port 30031 or port 30230"
    );
    assert_eq!(capture_source_kind(backend, false), "pktmon-live-capture");
    assert_eq!(strategy.kind, CaptureStrategyKind::PortFiltered);
}

#[test]
fn pppoe_detection_keeps_pktmon_no_filter_fast_path() {
    let detection = nte_capture::PppoeDetection {
        detected: true,
        ..Default::default()
    };
    let backend = capture_backend_for_start(false, None);
    let strategy = capture_strategy_for_start(&detection, backend);

    assert_eq!(backend, CaptureBackend::Pktmon);
    assert_eq!(strategy.kind, CaptureStrategyKind::NoFilter);
    assert_eq!(strategy.reason, CaptureStrategyReason::PppoeFastPath);
}

#[test]
fn capture_backend_start_override_wins_over_setting() {
    assert_eq!(
        capture_backend_for_start(true, Some(CaptureBackendOverride::Pktmon)),
        CaptureBackend::Pktmon
    );
    assert_eq!(
        capture_backend_for_start(false, Some(CaptureBackendOverride::WinDivert)),
        CaptureBackend::WinDivert
    );
}

#[test]
fn windivert_unavailable_error_uses_specific_error_code() {
    let error = capture_backend_runtime_error(
        "windivert-live-capture",
        "windivert_unavailable: failed to load WinDivert.dll",
    );

    assert_eq!(error.code, "windivert_unavailable");
    assert_eq!(error.message, "failed to load WinDivert.dll");
}
