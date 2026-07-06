#[cfg(any(windows, test))]
const DROPPED_EVIDENCE_PREFIX_BYTES: usize = 32;
#[cfg(any(windows, test))]
const DROPPED_EVIDENCE_EXAMPLE_LIMIT: usize = 20;
#[cfg(any(windows, test))]
const MAX_VLAN_TAGS: usize = 2;

#[cfg(any(windows, test))]
fn analyze_dropped_packet(
    bytes: &[u8],
    kind: crate::raw::PacketKind,
) -> DiagnosticDroppedPacketAnalysis {
    let mut analyzer = DroppedPacketAnalyzer::new(bytes, kind);
    analyzer.analyze();
    analyzer.finish()
}

#[cfg(any(windows, test))]
fn add_dropped_evidence_summary(
    summary: &mut DiagnosticCaptureSummary,
    analysis: &DiagnosticDroppedPacketAnalysis,
    capture_index: u64,
    size: usize,
) {
    increment(
        &mut summary.dropped_evidence.layer_chain_counts,
        analysis.layer_chain.join(" -> "),
    );
    increment(
        &mut summary.dropped_evidence.failure_reason_counts,
        &analysis.failure_reason,
    );
    for layer in &analysis.layer_chain {
        if matches!(
            layer.as_str(),
            "vlan" | "qinq" | "pppoe_discovery" | "pppoe_session" | "ppp_ipv4" | "ppp_ipv6"
        ) {
            increment(&mut summary.dropped_evidence.encapsulation_counts, layer);
        }
    }
    if let Some(ethertype) = &analysis.ethertype {
        increment(&mut summary.dropped_evidence.ethertype_counts, ethertype);
    }
    if let Some(ppp_protocol) = &analysis.ppp_protocol {
        increment(
            &mut summary.dropped_evidence.ppp_protocol_counts,
            ppp_protocol,
        );
    }
    if let Some(ip) = &analysis.ip {
        increment(&mut summary.dropped_evidence.ip_protocol_counts, &ip.protocol);
    }
    if summary.dropped_evidence.examples.len() < DROPPED_EVIDENCE_EXAMPLE_LIMIT {
        summary
            .dropped_evidence
            .examples
            .push(DiagnosticDroppedEvidenceExample {
                capture_index,
                packet_kind: analysis.packet_kind.clone(),
                size,
                layer_chain: analysis.layer_chain.clone(),
                failure_reason: analysis.failure_reason.clone(),
                offsets: analysis.offsets.clone(),
                ethertype: analysis.ethertype.clone(),
                ppp_protocol: analysis.ppp_protocol.clone(),
                ip_protocol: analysis.ip.as_ref().map(|ip| ip.protocol.clone()),
                prefix_hex: analysis.prefix_hex.clone(),
            });
    }
}

include!("drop_analysis/analyzer.rs");
include!("drop_analysis/format.rs");

#[cfg(test)]
include!("drop_analysis/tests.rs");
