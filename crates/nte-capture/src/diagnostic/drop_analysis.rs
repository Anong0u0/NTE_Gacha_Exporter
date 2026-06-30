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

#[cfg(any(windows, test))]
struct DroppedPacketAnalyzer<'a> {
    bytes: &'a [u8],
    kind: crate::raw::PacketKind,
    layer_chain: Vec<String>,
    failure_reason: Option<String>,
    offsets: DiagnosticDroppedOffsets,
    ethertype: Option<String>,
    vlan_tags: Vec<DiagnosticVlanTagEvidence>,
    pppoe: Option<DiagnosticPppoeEvidence>,
    ppp_protocol: Option<String>,
    ip: Option<DiagnosticIpEvidence>,
    transport: Option<DiagnosticTransportEvidence>,
}

#[cfg(any(windows, test))]
impl<'a> DroppedPacketAnalyzer<'a> {
    fn new(bytes: &'a [u8], kind: crate::raw::PacketKind) -> Self {
        Self {
            bytes,
            kind,
            layer_chain: vec![packet_kind_name(kind).to_string()],
            failure_reason: None,
            offsets: DiagnosticDroppedOffsets::default(),
            ethertype: None,
            vlan_tags: Vec::new(),
            pppoe: None,
            ppp_protocol: None,
            ip: None,
            transport: None,
        }
    }

    fn analyze(&mut self) {
        match self.kind {
            crate::raw::PacketKind::Ethernet => self.analyze_ethernet(),
            crate::raw::PacketKind::Ip => self.analyze_ip_at(0),
            crate::raw::PacketKind::Udp => self.analyze_udp_at(0, self.bytes.len()),
            crate::raw::PacketKind::Tcp => self.analyze_tcp_at(0, self.bytes.len()),
            crate::raw::PacketKind::L4Payload => self.fail("unknown_l4_payload"),
            crate::raw::PacketKind::Unknown => self.analyze_unknown(),
        }
    }

    fn analyze_unknown(&mut self) {
        if self.bytes.first().is_some_and(|byte| matches!(byte >> 4, 4 | 6)) {
            self.analyze_ip_at(0);
            return;
        }
        if self.bytes.len() >= 14 {
            self.analyze_ethernet();
            return;
        }
        self.fail("unknown_packet_format");
    }

    fn analyze_ethernet(&mut self) {
        self.push_layer("ethernet");
        if self.bytes.len() < 14 {
            self.fail("truncated_ethernet_header");
            return;
        }
        self.offsets.ethertype_offset = Some(12);
        let mut ethertype = read_u16(self.bytes, 12).expect("ethernet length checked");
        let mut offset = 14;
        let mut vlan_depth = 0;
        while is_vlan_ethertype(ethertype) {
            if vlan_depth >= MAX_VLAN_TAGS {
                self.ethertype = Some(hex_u16(ethertype));
                self.fail("unsupported_vlan_depth");
                return;
            }
            self.push_layer(if vlan_depth == 0 { "vlan" } else { "qinq" });
            if self.bytes.len() < offset + 4 {
                self.fail("truncated_vlan_header");
                return;
            }
            let tci = read_u16(self.bytes, offset).expect("vlan length checked");
            self.offsets.vlan_offsets.push(offset);
            self.vlan_tags.push(DiagnosticVlanTagEvidence {
                offset,
                tpid: hex_u16(ethertype),
                tci: hex_u16(tci),
                vid: tci & 0x0fff,
            });
            ethertype = read_u16(self.bytes, offset + 2).expect("vlan length checked");
            offset += 4;
            vlan_depth += 1;
        }
        self.ethertype = Some(hex_u16(ethertype));
        match ethertype {
            0x0800 | 0x86dd => self.analyze_ip_at(offset),
            0x8863 => self.analyze_pppoe_discovery_at(offset),
            0x8864 => self.analyze_pppoe_at(offset),
            _ => self.fail("unsupported_ethertype"),
        }
    }

    fn analyze_pppoe_discovery_at(&mut self, offset: usize) {
        self.push_layer("pppoe_discovery");
        self.offsets.pppoe_offset = Some(offset);
        if self.bytes.len() < offset + 6 {
            self.fail("truncated_pppoe_header");
            return;
        }
        let version_type = self.bytes[offset];
        let length = read_u16(self.bytes, offset + 4).expect("pppoe length checked") as usize;
        self.pppoe = Some(DiagnosticPppoeEvidence {
            offset,
            version: version_type >> 4,
            typ: version_type & 0x0f,
            code: self.bytes[offset + 1],
            session_id: hex_u16(read_u16(self.bytes, offset + 2).expect("pppoe length checked")),
            length,
        });
        self.fail("pppoe_discovery_frame");
    }

    fn analyze_pppoe_at(&mut self, offset: usize) {
        self.push_layer("pppoe_session");
        self.offsets.pppoe_offset = Some(offset);
        if self.bytes.len() < offset + 6 {
            self.fail("truncated_pppoe_header");
            return;
        }
        let version_type = self.bytes[offset];
        let code = self.bytes[offset + 1];
        let length = read_u16(self.bytes, offset + 4).expect("pppoe length checked") as usize;
        self.pppoe = Some(DiagnosticPppoeEvidence {
            offset,
            version: version_type >> 4,
            typ: version_type & 0x0f,
            code,
            session_id: hex_u16(read_u16(self.bytes, offset + 2).expect("pppoe length checked")),
            length,
        });
        if code != 0 {
            self.fail("non_session_pppoe_code");
            return;
        }
        if length < 2 {
            self.fail("malformed_pppoe_length");
            return;
        }
        let ppp_protocol_offset = offset + 6;
        self.offsets.ppp_protocol_offset = Some(ppp_protocol_offset);
        if self.bytes.len() < ppp_protocol_offset + 2 {
            self.fail("truncated_ppp_protocol");
            return;
        }
        if self.bytes.len() < ppp_protocol_offset + length {
            self.fail("truncated_pppoe_payload");
            return;
        }
        let protocol = read_u16(self.bytes, ppp_protocol_offset).expect("ppp length checked");
        self.ppp_protocol = Some(hex_u16(protocol));
        let ip_offset = ppp_protocol_offset + 2;
        self.offsets.inner_ip_offset = Some(ip_offset);
        match protocol {
            0x0021 => {
                self.push_layer("ppp_ipv4");
                self.analyze_ip_at(ip_offset);
            }
            0x0057 => {
                self.push_layer("ppp_ipv6");
                self.analyze_ip_at(ip_offset);
            }
            _ => self.fail("unsupported_ppp_protocol"),
        }
    }

    fn analyze_ip_at(&mut self, offset: usize) {
        let Some(first) = self.bytes.get(offset).copied() else {
            self.fail("truncated_ip_header");
            return;
        };
        match first >> 4 {
            4 => self.analyze_ipv4_at(offset),
            6 => self.analyze_ipv6_at(offset),
            _ => self.fail("unsupported_ip_version"),
        }
    }

    fn analyze_ipv4_at(&mut self, offset: usize) {
        self.push_layer("ipv4");
        if self.bytes.len() < offset + 20 {
            self.fail("truncated_ipv4_header");
            return;
        }
        let ihl = ((self.bytes[offset] & 0x0f) as usize) * 4;
        if ihl < 20 {
            self.fail("invalid_ipv4_ihl");
            return;
        }
        if self.bytes.len() < offset + ihl {
            self.fail("truncated_ipv4_options");
            return;
        }
        let total_len = read_u16(self.bytes, offset + 2).expect("ipv4 length checked") as usize;
        if total_len < ihl {
            self.fail("invalid_ipv4_total_length");
            return;
        }
        let protocol = self.bytes[offset + 9];
        let l4_offset = offset + ihl;
        let ip_end = self.bytes.len().min(offset + total_len);
        self.offsets.l4_offset = Some(l4_offset);
        self.ip = Some(DiagnosticIpEvidence {
            version: 4,
            offset,
            header_len: ihl,
            protocol: ip_protocol_name(protocol).to_string(),
        });
        match protocol {
            6 => self.analyze_tcp_at(l4_offset, ip_end),
            17 => self.analyze_udp_at(l4_offset, ip_end),
            _ => self.fail("unsupported_ip_protocol"),
        }
    }

    fn analyze_ipv6_at(&mut self, offset: usize) {
        self.push_layer("ipv6");
        if self.bytes.len() < offset + 40 {
            self.fail("truncated_ipv6_header");
            return;
        }
        let payload_len = read_u16(self.bytes, offset + 4).expect("ipv6 length checked") as usize;
        let mut next_header = self.bytes[offset + 6];
        let mut l4_offset = offset + 40;
        let ip_end = self.bytes.len().min(offset + 40 + payload_len);
        loop {
            match next_header {
                0 | 43 | 60 => {
                    if self.bytes.len() < l4_offset + 2 {
                        self.fail("truncated_ipv6_extension_header");
                        return;
                    }
                    next_header = self.bytes[l4_offset];
                    l4_offset += (self.bytes[l4_offset + 1] as usize + 1) * 8;
                    if self.bytes.len() < l4_offset {
                        self.fail("truncated_ipv6_extension_payload");
                        return;
                    }
                }
                44 => {
                    if self.bytes.len() < l4_offset + 8 {
                        self.fail("truncated_ipv6_fragment_header");
                        return;
                    }
                    next_header = self.bytes[l4_offset];
                    l4_offset += 8;
                }
                6 | 17 => {
                    self.offsets.l4_offset = Some(l4_offset);
                    self.ip = Some(DiagnosticIpEvidence {
                        version: 6,
                        offset,
                        header_len: 40,
                        protocol: ip_protocol_name(next_header).to_string(),
                    });
                    if next_header == 6 {
                        self.analyze_tcp_at(l4_offset, ip_end);
                    } else {
                        self.analyze_udp_at(l4_offset, ip_end);
                    }
                    return;
                }
                _ => {
                    self.ip = Some(DiagnosticIpEvidence {
                        version: 6,
                        offset,
                        header_len: 40,
                        protocol: ip_protocol_name(next_header).to_string(),
                    });
                    self.fail("unsupported_ip_protocol");
                    return;
                }
            }
        }
    }

    fn analyze_udp_at(&mut self, offset: usize, ip_end: usize) {
        self.push_layer("udp");
        if ip_end < offset + 8 || self.bytes.len() < offset + 8 {
            self.fail("truncated_udp_header");
            return;
        }
        let sport = read_u16(self.bytes, offset).expect("udp length checked");
        let dport = read_u16(self.bytes, offset + 2).expect("udp length checked");
        let udp_len = read_u16(self.bytes, offset + 4).expect("udp length checked") as usize;
        self.transport = Some(DiagnosticTransportEvidence {
            protocol: "udp".to_string(),
            offset,
            sport: Some(sport),
            dport: Some(dport),
        });
        if udp_len < 8 {
            self.fail("invalid_udp_length");
            return;
        }
        let payload_end = ip_end.min(offset + udp_len).min(self.bytes.len());
        if payload_end <= offset + 8 {
            self.fail("empty_udp_payload");
            return;
        }
        self.fail("parser_rejected_udp_payload");
    }

    fn analyze_tcp_at(&mut self, offset: usize, ip_end: usize) {
        self.push_layer("tcp");
        if ip_end < offset + 20 || self.bytes.len() < offset + 20 {
            self.fail("truncated_tcp_header");
            return;
        }
        let sport = read_u16(self.bytes, offset).expect("tcp length checked");
        let dport = read_u16(self.bytes, offset + 2).expect("tcp length checked");
        let off_flags = read_u16(self.bytes, offset + 12).expect("tcp length checked");
        let header_len = ((off_flags >> 12) as usize) * 4;
        self.transport = Some(DiagnosticTransportEvidence {
            protocol: "tcp".to_string(),
            offset,
            sport: Some(sport),
            dport: Some(dport),
        });
        if header_len < 20 {
            self.fail("invalid_tcp_header_length");
            return;
        }
        if ip_end < offset + header_len || self.bytes.len() < offset + header_len {
            self.fail("truncated_tcp_options");
            return;
        }
        if ip_end <= offset + header_len {
            self.fail("empty_tcp_payload");
            return;
        }
        self.fail("parser_rejected_tcp_payload");
    }

    fn finish(self) -> DiagnosticDroppedPacketAnalysis {
        DiagnosticDroppedPacketAnalysis {
            packet_kind: packet_kind_name(self.kind).to_string(),
            layer_chain: self.layer_chain,
            failure_reason: self
                .failure_reason
                .unwrap_or_else(|| "analysis_incomplete".to_string()),
            offsets: self.offsets,
            ethertype: self.ethertype,
            vlan_tags: self.vlan_tags,
            pppoe: self.pppoe,
            ppp_protocol: self.ppp_protocol,
            ip: self.ip,
            transport: self.transport,
            prefix_hex: prefix_hex(self.bytes, DROPPED_EVIDENCE_PREFIX_BYTES),
        }
    }

    fn push_layer(&mut self, layer: &str) {
        if self.layer_chain.last().is_none_or(|last| last != layer) {
            self.layer_chain.push(layer.to_string());
        }
    }

    fn fail(&mut self, reason: &str) {
        if self.failure_reason.is_none() {
            self.failure_reason = Some(reason.to_string());
        }
    }
}

#[cfg(any(windows, test))]
fn is_vlan_ethertype(value: u16) -> bool {
    matches!(value, 0x8100 | 0x88a8 | 0x9100)
}

#[cfg(any(windows, test))]
fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_be_bytes([
        *bytes.get(offset)?,
        *bytes.get(offset + 1)?,
    ]))
}

#[cfg(any(windows, test))]
fn hex_u16(value: u16) -> String {
    format!("0x{value:04x}")
}

#[cfg(any(windows, test))]
fn hex_u8(value: u8) -> String {
    format!("0x{value:02x}")
}

#[cfg(any(windows, test))]
fn ip_protocol_name(value: u8) -> String {
    match value {
        6 => "tcp".to_string(),
        17 => "udp".to_string(),
        _ => hex_u8(value),
    }
}

#[cfg(any(windows, test))]
fn prefix_hex(bytes: &[u8], limit: usize) -> String {
    bytes
        .iter()
        .take(limit)
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod dropped_analysis_tests {
    use super::*;
    use crate::raw::PacketKind;

    #[test]
    fn analyzes_pppoe_ipv4_udp_offsets() {
        let packet = pppoe_ipv4_udp_packet(64208, 30138, b"hello", 0x0021);

        let analysis = analyze_dropped_packet(&packet, PacketKind::Ethernet);

        assert_eq!(
            analysis.layer_chain,
            ["ethernet", "pppoe_session", "ppp_ipv4", "ipv4", "udp"]
        );
        assert_eq!(analysis.failure_reason, "parser_rejected_udp_payload");
        assert_eq!(analysis.ethertype.as_deref(), Some("0x8864"));
        assert_eq!(analysis.ppp_protocol.as_deref(), Some("0x0021"));
        assert_eq!(analysis.offsets.pppoe_offset, Some(14));
        assert_eq!(analysis.offsets.ppp_protocol_offset, Some(20));
        assert_eq!(analysis.offsets.inner_ip_offset, Some(22));
        assert_eq!(analysis.transport.as_ref().and_then(|value| value.dport), Some(30138));
    }

    #[test]
    fn analyzes_pppoe_unsupported_protocol() {
        let packet = pppoe_ipv4_udp_packet(64208, 30138, b"hello", 0x0059);

        let analysis = analyze_dropped_packet(&packet, PacketKind::Ethernet);

        assert_eq!(analysis.failure_reason, "unsupported_ppp_protocol");
        assert_eq!(
            analysis.layer_chain,
            ["ethernet", "pppoe_session"]
        );
        assert_eq!(analysis.ppp_protocol.as_deref(), Some("0x0059"));
    }

    #[test]
    fn analyzes_pppoe_discovery_as_evidence_only() {
        let mut packet = vec![0_u8; 20];
        packet[12..14].copy_from_slice(&0x8863_u16.to_be_bytes());
        packet[14] = 0x11;
        packet[15] = 0x09;

        let analysis = analyze_dropped_packet(&packet, PacketKind::Ethernet);

        assert_eq!(analysis.layer_chain, ["ethernet", "pppoe_discovery"]);
        assert_eq!(analysis.failure_reason, "pppoe_discovery_frame");
        assert_eq!(analysis.ethertype.as_deref(), Some("0x8863"));
        assert_eq!(analysis.pppoe.as_ref().map(|pppoe| pppoe.code), Some(0x09));
    }

    #[test]
    fn analyzes_malformed_pppoe_length() {
        let mut packet = pppoe_ipv4_udp_packet(64208, 30138, b"hello", 0x0021);
        packet[18..20].copy_from_slice(&1_u16.to_be_bytes());

        let analysis = analyze_dropped_packet(&packet, PacketKind::Ethernet);

        assert_eq!(analysis.failure_reason, "malformed_pppoe_length");
    }

    #[test]
    fn analyzes_vlan_pppoe_chain() {
        let inner = pppoe_ipv4_udp_packet(64208, 30138, b"hello", 0x0021);
        let mut packet = vec![0_u8; inner.len() + 4];
        packet[..12].copy_from_slice(&inner[..12]);
        packet[12..14].copy_from_slice(&0x8100_u16.to_be_bytes());
        packet[14..16].copy_from_slice(&7_u16.to_be_bytes());
        packet[16..18].copy_from_slice(&0x8864_u16.to_be_bytes());
        packet[18..].copy_from_slice(&inner[14..]);

        let analysis = analyze_dropped_packet(&packet, PacketKind::Ethernet);

        assert_eq!(
            analysis.layer_chain,
            ["ethernet", "vlan", "pppoe_session", "ppp_ipv4", "ipv4", "udp"]
        );
        assert_eq!(analysis.vlan_tags[0].vid, 7);
        assert_eq!(analysis.offsets.vlan_offsets, [14]);
        assert_eq!(analysis.offsets.inner_ip_offset, Some(26));
    }

    #[test]
    fn summarizes_dropped_evidence_examples() {
        let packet = pppoe_ipv4_udp_packet(64208, 30138, b"hello", 0x0059);
        let analysis = analyze_dropped_packet(&packet, PacketKind::Ethernet);
        let mut summary = DiagnosticCaptureSummary::default();

        add_dropped_evidence_summary(&mut summary, &analysis, 7, packet.len());

        assert_eq!(
            summary
                .dropped_evidence
                .failure_reason_counts
                .get("unsupported_ppp_protocol"),
            Some(&1)
        );
        assert_eq!(
            summary
                .dropped_evidence
                .encapsulation_counts
                .get("pppoe_session"),
            Some(&1)
        );
        assert_eq!(summary.dropped_evidence.examples[0].capture_index, 7);
    }

    fn pppoe_ipv4_udp_packet(sport: u16, dport: u16, payload: &[u8], ppp_protocol: u16) -> Vec<u8> {
        let udp_len = 8 + payload.len();
        let total_len = 20 + udp_len;
        let pppoe_len = 2 + total_len;
        let ip_offset = 14 + 6 + 2;
        let mut packet = vec![0_u8; ip_offset + total_len];
        packet[12..14].copy_from_slice(&0x8864_u16.to_be_bytes());
        packet[14] = 0x11;
        packet[15] = 0;
        packet[16..18].copy_from_slice(&1_u16.to_be_bytes());
        packet[18..20].copy_from_slice(&(pppoe_len as u16).to_be_bytes());
        packet[20..22].copy_from_slice(&ppp_protocol.to_be_bytes());
        packet[ip_offset] = 0x45;
        packet[ip_offset + 2..ip_offset + 4].copy_from_slice(&(total_len as u16).to_be_bytes());
        packet[ip_offset + 9] = 17;
        let udp_offset = ip_offset + 20;
        packet[udp_offset..udp_offset + 2].copy_from_slice(&sport.to_be_bytes());
        packet[udp_offset + 2..udp_offset + 4].copy_from_slice(&dport.to_be_bytes());
        packet[udp_offset + 4..udp_offset + 6].copy_from_slice(&(udp_len as u16).to_be_bytes());
        packet[udp_offset + 8..].copy_from_slice(payload);
        packet
    }
}
