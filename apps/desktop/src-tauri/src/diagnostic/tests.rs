#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_no_packets_seen() {
        let result = classify_capture_result(
            &DiagnosticCaptureCounters::default(),
            &DiagnosticCaptureSummary::default(),
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
        let result = classify_capture_result(&counters, &summary, Vec::new());
        assert_eq!(result.verdict, "only_idle_packets");
    }

    #[test]
    fn classifies_high_parser_drop_before_idle() {
        let counters = DiagnosticCaptureCounters {
            packets_seen: 10,
            dropped_packets: 6,
            ..Default::default()
        };
        let result =
            classify_capture_result(&counters, &DiagnosticCaptureSummary::default(), Vec::new());
        assert_eq!(result.verdict, "high_parser_drop");
    }

    #[test]
    fn classifies_decoded_ok() {
        let counters = DiagnosticCaptureCounters {
            packets_seen: 10,
            decoded_packets: 1,
            ..Default::default()
        };
        let result =
            classify_capture_result(&counters, &DiagnosticCaptureSummary::default(), Vec::new());
        assert_eq!(result.verdict, "decoded_ok");
    }
}
