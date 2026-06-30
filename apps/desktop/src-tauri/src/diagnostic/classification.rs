fn classify_diagnostic(
    environment: &DiagnosticEnvironment,
    target: &DiagnosticTargetDiscovery,
    internal: &InternalDiagnosticReport,
    external: &ExternalCaptureReport,
) -> DiagnosticClassification {
    let mut findings = target.warnings.clone();
    if let Some(error) = &target.error {
        findings.push(format!("process discovery error: {error}"));
    }
    if external.attempted && !external.ok {
        findings.push(format!(
            "external pktmon failed: {}",
            external.error.as_deref().unwrap_or("unknown error")
        ));
    }
    if !environment.windows {
        return classification("non_windows", findings);
    }
    if !environment.admin {
        return classification("admin_required", findings);
    }
    if target.selected_pid.is_none() {
        return classification("game_not_found", findings);
    }
    if target.selected_ports.is_empty() {
        return classification("no_candidate_ports", findings);
    }
    if let Some(error) = &internal.error {
        findings.push(format!("internal capture error: {error}"));
        if internal.result.is_none() {
            return classification("internal_capture_failed", findings);
        }
    }
    let Some(result) = &internal.result else {
        return classification("internal_capture_missing", findings);
    };
    classify_capture_result(&result.counters, &result.summary, findings)
}

fn classify_capture_result(
    counters: &DiagnosticCaptureCounters,
    summary: &DiagnosticCaptureSummary,
    findings: Vec<String>,
) -> DiagnosticClassification {
    if counters.packets_seen == 0 {
        return classification("no_packets_seen", findings);
    }
    if summary.rows_count > 0 || counters.decoded_packets > 0 {
        return classification("decoded_ok", findings);
    }
    let dropped_ratio = counters.dropped_packets as f64 / counters.packets_seen.max(1) as f64;
    if dropped_ratio >= 0.50 {
        return classification("high_parser_drop", findings);
    }
    if !summary.marker_hits.any() {
        let parsed = counters
            .packets_seen
            .saturating_sub(counters.dropped_packets)
            .saturating_sub(counters.duplicate_packets)
            .max(1);
        if summary.small_parsed_payload_packets as f64 / parsed as f64 >= 0.80 {
            return classification("only_idle_packets", findings);
        }
        return classification("no_decoder_marker", findings);
    }
    classification("marker_found_no_rows", findings)
}

fn classification(verdict: &str, findings: Vec<String>) -> DiagnosticClassification {
    DiagnosticClassification {
        verdict: verdict.to_string(),
        findings,
    }
}

fn status_summary(document: &DiagnosticDocument) -> DiagnosticStatusSummary {
    let counters = document
        .internal
        .result
        .as_ref()
        .map(|result| result.counters.clone())
        .unwrap_or_default();
    let rows_count = document
        .internal
        .result
        .as_ref()
        .map(|result| result.summary.rows_count)
        .unwrap_or_default();
    DiagnosticStatusSummary {
        verdict: document.verdict.verdict.clone(),
        findings: document.verdict.findings.clone(),
        packets_seen: counters.packets_seen,
        decoded_packets: counters.decoded_packets,
        dropped_packets: counters.dropped_packets,
        duplicate_packets: counters.duplicate_packets,
        rows_count,
        external_ok: document.external.ok,
    }
}
