use crate::live::CaptureStrategyKind;
use crate::raw::ParsedNetworkPacket;

pub(crate) fn should_write_raw_packet(
    packet: &ParsedNetworkPacket,
    ports: &[u16],
    decoded: bool,
    strategy: CaptureStrategyKind,
) -> bool {
    if strategy == CaptureStrategyKind::PortFiltered {
        return true;
    }
    decoded || packet_matches_ports(packet, ports)
}

fn packet_matches_ports(packet: &ParsedNetworkPacket, ports: &[u16]) -> bool {
    packet.sport.is_some_and(|port| ports.contains(&port))
        || packet.dport.is_some_and(|port| ports.contains(&port))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_filter_raw_guard_keeps_candidate_ports_and_decode_hits() {
        let packet = parsed_packet(Some(64208), Some(30138));
        assert!(should_write_raw_packet(
            &packet,
            &[30138],
            false,
            CaptureStrategyKind::NoFilter
        ));
        assert!(should_write_raw_packet(
            &packet,
            &[],
            true,
            CaptureStrategyKind::NoFilter
        ));
    }

    #[test]
    fn no_filter_raw_guard_drops_unmatched_non_decode_packets() {
        let packet = parsed_packet(Some(64208), Some(30138));
        assert!(!should_write_raw_packet(
            &packet,
            &[30031],
            false,
            CaptureStrategyKind::NoFilter
        ));
    }

    fn parsed_packet(sport: Option<u16>, dport: Option<u16>) -> ParsedNetworkPacket {
        ParsedNetworkPacket {
            proto: "udp".to_string(),
            sport,
            dport,
            seq: None,
            ack: None,
            flags: None,
            payload: b"hello".to_vec(),
            parser: "test".to_string(),
        }
    }
}
