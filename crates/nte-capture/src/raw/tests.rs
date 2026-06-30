#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ipv4_udp_payload() {
        let payload = b"hello";
        let udp_len = 8 + payload.len();
        let total_len = 20 + udp_len;
        let mut packet = vec![0_u8; total_len];
        packet[0] = 0x45;
        packet[2..4].copy_from_slice(&(total_len as u16).to_be_bytes());
        packet[9] = 17;
        packet[20..22].copy_from_slice(&30230_u16.to_be_bytes());
        packet[22..24].copy_from_slice(&49310_u16.to_be_bytes());
        packet[24..26].copy_from_slice(&(udp_len as u16).to_be_bytes());
        packet[28..].copy_from_slice(payload);

        let parsed = parse_packet_bytes(&packet, PacketKind::Ip).unwrap();
        let record = raw_record_from_parsed_packet(&parsed, 1, 1.0);

        assert_eq!(record.proto, "udp");
        assert_eq!(record.sport, Some(30230));
        assert_eq!(record.dport, Some(49310));
        assert_eq!(record.size, 5);
    }

    #[test]
    fn parses_pppoe_ipv4_udp_payload() {
        let payload = b"hello";
        let packet = pppoe_ipv4_udp_packet(64208, 30138, payload, 0x0021);

        let parsed = parse_packet_bytes(&packet, PacketKind::Ethernet).unwrap();

        assert_eq!(parsed.proto, "udp");
        assert_eq!(parsed.sport, Some(64208));
        assert_eq!(parsed.dport, Some(30138));
        assert_eq!(parsed.parser, "pktmon-pppoe");
        assert_eq!(parsed.payload, payload);
    }

    #[test]
    fn parses_pppoe_ipv6_udp_payload() {
        let payload = b"hello-v6";
        let packet = pppoe_ipv6_udp_packet(64208, 30138, payload);

        let parsed = parse_packet_bytes(&packet, PacketKind::Ethernet).unwrap();

        assert_eq!(parsed.proto, "udp");
        assert_eq!(parsed.sport, Some(64208));
        assert_eq!(parsed.dport, Some(30138));
        assert_eq!(parsed.parser, "pktmon-pppoe");
        assert_eq!(parsed.payload, payload);
    }

    #[test]
    fn rejects_unsupported_pppoe_protocol() {
        let packet = pppoe_ipv4_udp_packet(64208, 30138, b"hello", 0x0059);

        assert!(parse_packet_bytes(&packet, PacketKind::Ethernet).is_none());
    }

    #[test]
    fn rejects_malformed_pppoe_length() {
        let mut packet = pppoe_ipv4_udp_packet(64208, 30138, b"hello", 0x0021);
        packet[18..20].copy_from_slice(&1_u16.to_be_bytes());

        assert!(parse_packet_bytes(&packet, PacketKind::Ethernet).is_none());
    }

    #[test]
    fn packet_kind_variants_are_covered() {
        let empty = [];
        for kind in [
            PacketKind::Unknown,
            PacketKind::Ethernet,
            PacketKind::Tcp,
            PacketKind::Udp,
            PacketKind::L4Payload,
        ] {
            assert!(parse_packet_bytes(&empty, kind).is_none());
        }
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

    fn pppoe_ipv6_udp_packet(sport: u16, dport: u16, payload: &[u8]) -> Vec<u8> {
        let udp_len = 8 + payload.len();
        let ipv6_payload_len = udp_len;
        let pppoe_len = 2 + 40 + udp_len;
        let ip_offset = 14 + 6 + 2;
        let mut packet = vec![0_u8; ip_offset + 40 + udp_len];
        packet[12..14].copy_from_slice(&0x8864_u16.to_be_bytes());
        packet[14] = 0x11;
        packet[15] = 0;
        packet[16..18].copy_from_slice(&1_u16.to_be_bytes());
        packet[18..20].copy_from_slice(&(pppoe_len as u16).to_be_bytes());
        packet[20..22].copy_from_slice(&0x0057_u16.to_be_bytes());
        packet[ip_offset] = 0x60;
        packet[ip_offset + 4..ip_offset + 6].copy_from_slice(&(ipv6_payload_len as u16).to_be_bytes());
        packet[ip_offset + 6] = 17;
        let udp_offset = ip_offset + 40;
        packet[udp_offset..udp_offset + 2].copy_from_slice(&sport.to_be_bytes());
        packet[udp_offset + 2..udp_offset + 4].copy_from_slice(&dport.to_be_bytes());
        packet[udp_offset + 4..udp_offset + 6].copy_from_slice(&(udp_len as u16).to_be_bytes());
        packet[udp_offset + 8..].copy_from_slice(payload);
        packet
    }
}
