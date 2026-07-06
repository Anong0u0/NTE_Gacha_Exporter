#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_no_packets_seen() {
        let result = classify_capture_result(
            &diagnostic_result(DiagnosticCaptureCounters::default()),
            Vec::new(),
        );
        assert_eq!(result.verdict, "no_packets_seen");
    }

    #[test]
    fn classifies_idle_packets_without_markers() {
        let counters = DiagnosticCaptureCounters {
            packets_seen: 10,
            raw_packets_written: 10,
            ..Default::default()
        };
        let summary = DiagnosticCaptureSummary {
            small_parsed_payload_packets: 9,
            ..Default::default()
        };
        let result = classify_capture_result(&diagnostic_result((counters, summary)), Vec::new());
        assert_eq!(result.verdict, "only_idle_packets");
    }

    #[test]
    fn classifies_high_parser_drop_before_idle() {
        let counters = DiagnosticCaptureCounters {
            packets_seen: 10,
            dropped_packets: 6,
            ..Default::default()
        };
        let result = classify_capture_result(&diagnostic_result(counters), Vec::new());
        assert_eq!(result.verdict, "high_parser_drop");
    }

    #[test]
    fn keeps_high_parser_drop_verdict_with_dropped_evidence_findings() {
        let counters = DiagnosticCaptureCounters {
            packets_seen: 10,
            dropped_packets: 6,
            dropped_full_samples_written: 3,
            ..Default::default()
        };
        let mut summary = DiagnosticCaptureSummary::default();
        summary
            .dropped_evidence
            .encapsulation_counts
            .insert("pppoe_session".to_string(), 4);
        summary
            .dropped_evidence
            .ppp_protocol_counts
            .insert("0x0059".to_string(), 4);
        summary
            .dropped_evidence
            .failure_reason_counts
            .insert("unsupported_ppp_protocol".to_string(), 4);

        let result = classify_capture_result(&diagnostic_result((counters, summary)), Vec::new());

        assert_eq!(result.verdict, "high_parser_drop");
        assert!(
            result
                .findings
                .iter()
                .any(|finding| finding.contains("PPPoE"))
        );
        assert!(result.findings.iter().any(|finding| {
            finding.contains("unsupported_ppp_protocol") && finding.contains("count=4")
        }));
        assert!(
            result
                .findings
                .iter()
                .any(|finding| finding == "dropped full samples included: 3")
        );
    }

    #[test]
    fn classifies_decoded_ok() {
        let counters = DiagnosticCaptureCounters {
            packets_seen: 10,
            decoded_packets: 1,
            ..Default::default()
        };
        let result = classify_capture_result(&diagnostic_result(counters), Vec::new());
        assert_eq!(result.verdict, "decoded_ok");
    }

    #[test]
    fn classifies_no_filter_zero_decode_as_undecodable_path() {
        let counters = DiagnosticCaptureCounters {
            packets_seen: 10,
            ..Default::default()
        };
        let mut result = diagnostic_result(counters);
        result.target.capture_strategy = "no_filter".to_string();

        let result = classify_capture_result(&result, Vec::new());

        assert_eq!(result.verdict, "undecodable_path");
    }

    #[test]
    fn records_windivert_success_finding() {
        let counters = DiagnosticCaptureCounters {
            packets_seen: 10,
            decoded_packets: 1,
            ..Default::default()
        };
        let mut result = diagnostic_result(counters);
        result.target.interface = "windivert".to_string();

        let result = classify_capture_result(&result, Vec::new());

        assert_eq!(result.verdict, "decoded_ok");
        assert!(
            result
                .findings
                .iter()
                .any(|finding| finding == "WinDivert capture decoded records")
        );
    }

    #[test]
    fn classifies_windivert_zero_decode_as_windivert_no_decode() {
        let counters = DiagnosticCaptureCounters {
            packets_seen: 10,
            ..Default::default()
        };
        let mut result = diagnostic_result(counters);
        result.target.interface = "windivert".to_string();

        let result = classify_capture_result(&result, Vec::new());

        assert_eq!(result.verdict, "windivert_no_decode");
    }

    #[test]
    fn cancelled_diagnostic_status_has_no_bundle_summary_or_error() {
        let runtime = diagnostic_session(diagnostic_status("diagnostic-cancelled", "running", 1.0));

        mark_diagnostic_cancelled(&runtime);

        let status = runtime.status.lock().unwrap();
        assert_eq!(status.state, crate::lifecycle::STATE_CANCELLED);
        assert_eq!(status.stage, crate::lifecycle::STATE_CANCELLED);
        assert!(status.support_zip_path.is_none());
        assert!(status.summary.is_none());
        assert!(status.error.is_none());
    }

    #[test]
    fn prune_diagnostic_session_map_treats_cancelled_as_terminal() {
        let mut sessions = std::collections::HashMap::from([(
            "cancelled".to_string(),
            diagnostic_session(diagnostic_status(
                "cancelled",
                crate::lifecycle::STATE_CANCELLED,
                1.0,
            )),
        )]);

        prune_diagnostic_session_map(&mut sessions, "other", 2_000.0);

        assert!(!sessions.contains_key("cancelled"));
    }

    #[test]
    fn cleanup_diagnostic_staging_removes_only_session_artifacts() {
        let temp = tempfile::tempdir().unwrap();
        let paths = support_paths(temp.path(), "cleanup");
        std::fs::create_dir_all(&paths.support_dir).unwrap();
        for (_, path) in artifact_specs(&paths) {
            std::fs::write(path, b"session").unwrap();
        }
        std::fs::write(&paths.zip_path, b"zip").unwrap();
        let unrelated = paths.support_dir.join("diagnostic-other.zip");
        std::fs::write(&unrelated, b"keep").unwrap();

        cleanup_diagnostic_staging(&paths);

        for (_, path) in artifact_specs(&paths) {
            assert!(
                !path.exists(),
                "session artifact still exists: {}",
                path.display()
            );
        }
        assert!(!paths.zip_path.exists());
        assert_eq!(std::fs::read(unrelated).unwrap(), b"keep");
    }

    #[test]
    fn windivert_timer_completion_stops_capture_without_marking_cancelled() {
        let cancel_requested = Arc::new(AtomicBool::new(false));
        let capture_stop = Arc::new(AtomicBool::new(false));

        let timer = spawn_windivert_diagnostic_timer(
            Duration::ZERO,
            Arc::clone(&cancel_requested),
            Arc::clone(&capture_stop),
        );
        timer.join().unwrap();

        assert!(capture_stop.load(Ordering::SeqCst));
        assert!(!cancel_requested.load(Ordering::SeqCst));
    }

    #[test]
    fn windivert_timer_cancel_stops_capture_and_preserves_cancel_flag() {
        let cancel_requested = Arc::new(AtomicBool::new(true));
        let capture_stop = Arc::new(AtomicBool::new(false));

        let timer = spawn_windivert_diagnostic_timer(
            Duration::from_secs(60),
            Arc::clone(&cancel_requested),
            Arc::clone(&capture_stop),
        );
        timer.join().unwrap();

        assert!(capture_stop.load(Ordering::SeqCst));
        assert!(cancel_requested.load(Ordering::SeqCst));
    }

    trait IntoDiagnosticResultParts {
        fn into_parts(self) -> (DiagnosticCaptureCounters, DiagnosticCaptureSummary);
    }

    impl IntoDiagnosticResultParts for DiagnosticCaptureCounters {
        fn into_parts(self) -> (DiagnosticCaptureCounters, DiagnosticCaptureSummary) {
            (self, DiagnosticCaptureSummary::default())
        }
    }

    impl IntoDiagnosticResultParts for (DiagnosticCaptureCounters, DiagnosticCaptureSummary) {
        fn into_parts(self) -> (DiagnosticCaptureCounters, DiagnosticCaptureSummary) {
            self
        }
    }

    fn diagnostic_result(value: impl IntoDiagnosticResultParts) -> DiagnosticCaptureResult {
        let (counters, summary) = value.into_parts();
        DiagnosticCaptureResult {
            target: nte_capture::CaptureTarget {
                pid: 1,
                exe: "HTGame.exe".to_string(),
                interface: "pktmon".to_string(),
                ports: Vec::new(),
                bpf: String::new(),
                capture_strategy: "port_filtered".to_string(),
                strategy_reason: "default".to_string(),
                pppoe_detection: PppoeDetection::default(),
                attempts: Vec::new(),
            },
            counters,
            summary,
            warnings: Vec::new(),
            elapsed_seconds: 0.0,
        }
    }

    fn diagnostic_status(session_id: &str, state: &str, updated_at: f64) -> DiagnosticStatus {
        DiagnosticStatus {
            session_id: session_id.to_string(),
            mode: DiagnosticMode::Pktmon.as_str().to_string(),
            state: state.to_string(),
            started_at: updated_at,
            updated_at,
            duration_seconds: 20,
            elapsed_seconds: 0.0,
            stage: state.to_string(),
            progress: 0.0,
            support_zip_path: Some("support.zip".to_string()),
            error: Some(RuntimeError {
                code: "diagnostic_failed".to_string(),
                message: "failed".to_string(),
                support_path: None,
                support_image_path: None,
            }),
            summary: Some(DiagnosticStatusSummary {
                verdict: "failed".to_string(),
                findings: Vec::new(),
                packets_seen: 0,
                decoded_packets: 0,
                dropped_packets: 0,
                duplicate_packets: 0,
                rows_count: 0,
                external_ok: false,
            }),
        }
    }

    fn diagnostic_session(status: DiagnosticStatus) -> Arc<DiagnosticRuntimeSession> {
        Arc::new(DiagnosticRuntimeSession {
            status: Mutex::new(status),
            cancel_requested: Arc::new(AtomicBool::new(false)),
            handle: Mutex::new(None),
        })
    }
}
