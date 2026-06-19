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
}
