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
        assert_eq!(
            analysis.transport.as_ref().and_then(|value| value.dport),
            Some(30138)
        );
    }

    #[test]
    fn analyzes_pppoe_unsupported_protocol() {
        let packet = pppoe_ipv4_udp_packet(64208, 30138, b"hello", 0x0059);

        let analysis = analyze_dropped_packet(&packet, PacketKind::Ethernet);

        assert_eq!(analysis.failure_reason, "unsupported_ppp_protocol");
        assert_eq!(analysis.layer_chain, ["ethernet", "pppoe_session"]);
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
            [
                "ethernet",
                "vlan",
                "pppoe_session",
                "ppp_ipv4",
                "ipv4",
                "udp"
            ]
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
