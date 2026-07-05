#[cfg(any(windows, test))]
pub(crate) fn raw_record_from_parsed_packet(
    parsed: &ParsedNetworkPacket,
    capture_index: u64,
    captured_at: f64,
) -> RawPacketRecord {
    RawPacketRecord {
        typ: "packet".to_string(),
        schema_version: 2,
        captured_at,
        capture_index,
        proto: parsed.proto.clone(),
        sport: parsed.sport,
        dport: parsed.dport,
        seq: parsed.seq,
        ack: parsed.ack,
        flags: parsed.flags,
        parser: parsed.parser.clone(),
        size: parsed.payload.len(),
        payload_b64: base64::engine::general_purpose::STANDARD.encode(&parsed.payload),
    }
}

#[cfg(any(windows, test))]
pub(crate) fn parse_packet_bytes(bytes: &[u8], kind: PacketKind) -> Option<ParsedNetworkPacket> {
    parse_network_packet(bytes, kind)
}

#[cfg(any(windows, test))]
fn parse_network_packet(bytes: &[u8], kind: PacketKind) -> Option<ParsedNetworkPacket> {
    match kind {
        PacketKind::Ethernet => parse_ethernet(bytes),
        PacketKind::Ip => parse_ip(bytes, "pktmon-ip"),
        PacketKind::Udp => parse_udp_l4(bytes).or_else(|| assume_l4_payload(bytes, "udp")),
        PacketKind::Tcp => parse_tcp_l4(bytes).or_else(|| assume_l4_payload(bytes, "tcp")),
        PacketKind::L4Payload => assume_l4_payload(bytes, "l4"),
        PacketKind::Unknown => parse_ethernet(bytes)
            .or_else(|| parse_ip(bytes, "pktmon-ip"))
            .or_else(|| {
                parse_raw_ipv4_offsets(bytes).or_else(|| assume_l4_payload(bytes, "unknown"))
            }),
    }
    .or_else(|| parse_raw_ipv4_offsets(bytes))
}

#[cfg(any(windows, test))]
fn parse_ethernet(bytes: &[u8]) -> Option<ParsedNetworkPacket> {
    if bytes.len() < 14 {
        return None;
    }
    let mut ether_type = u16::from_be_bytes([bytes[12], bytes[13]]);
    let mut offset = 14;
    while matches!(ether_type, 0x8100 | 0x88a8 | 0x9100) {
        if bytes.len() < offset + 4 {
            return None;
        }
        ether_type = u16::from_be_bytes([bytes[offset + 2], bytes[offset + 3]]);
        offset += 4;
    }
    match ether_type {
        0x0800 | 0x86dd => parse_ip_at(bytes, offset, "pktmon-ethernet"),
        0x8864 => parse_pppoe_session(bytes, offset),
        _ => None,
    }
}

#[cfg(any(windows, test))]
fn parse_pppoe_session(bytes: &[u8], offset: usize) -> Option<ParsedNetworkPacket> {
    const PPPOE_HEADER_LEN: usize = 6;
    const PPP_PROTOCOL_LEN: usize = 2;
    const PPP_IPV4: u16 = 0x0021;
    const PPP_IPV6: u16 = 0x0057;

    if bytes.len() < offset + PPPOE_HEADER_LEN + PPP_PROTOCOL_LEN {
        return None;
    }
    let code = bytes[offset + 1];
    if code != 0 {
        return None;
    }
    let pppoe_len = u16::from_be_bytes([bytes[offset + 4], bytes[offset + 5]]) as usize;
    if pppoe_len < PPP_PROTOCOL_LEN {
        return None;
    }
    let ppp_offset = offset + PPPOE_HEADER_LEN;
    if bytes.len() < ppp_offset + pppoe_len {
        return None;
    }
    let protocol = u16::from_be_bytes([bytes[ppp_offset], bytes[ppp_offset + 1]]);
    let ip_offset = ppp_offset + PPP_PROTOCOL_LEN;
    match protocol {
        PPP_IPV4 | PPP_IPV6 => parse_ip_at(bytes, ip_offset, "pktmon-pppoe"),
        _ => None,
    }
}

#[cfg(any(windows, test))]
fn parse_raw_ipv4_offsets(bytes: &[u8]) -> Option<ParsedNetworkPacket> {
    [14_usize, 0]
        .into_iter()
        .find_map(|offset| parse_ip_at(bytes, offset, "pktmon-raw-ip"))
}

#[cfg(any(windows, test))]
fn parse_ip(bytes: &[u8], parser: &str) -> Option<ParsedNetworkPacket> {
    parse_ip_at(bytes, 0, parser)
}

#[cfg(any(windows, test))]
fn parse_ip_at(bytes: &[u8], offset: usize, parser: &str) -> Option<ParsedNetworkPacket> {
    let version = bytes.get(offset)? >> 4;
    match version {
        4 => parse_ipv4_at(bytes, offset, parser),
        6 => parse_ipv6_at(bytes, offset, parser),
        _ => None,
    }
}

#[cfg(any(windows, test))]
fn parse_ipv4_at(bytes: &[u8], ip_off: usize, parser: &str) -> Option<ParsedNetworkPacket> {
    if bytes.len() < ip_off + 20 {
        return None;
    }
    let ihl = ((bytes[ip_off] & 0x0f) as usize) * 4;
    if ihl < 20 || bytes.len() < ip_off + ihl {
        return None;
    }
    let total_len = u16::from_be_bytes([bytes[ip_off + 2], bytes[ip_off + 3]]) as usize;
    let ip_end = bytes.len().min(ip_off + total_len);
    let proto = bytes[ip_off + 9];
    let l4_off = ip_off + ihl;
    match proto {
        6 => parse_tcp_at(bytes, l4_off, ip_end, parser),
        17 => parse_udp_at(bytes, l4_off, ip_end, parser),
        _ => None,
    }
}

#[cfg(any(windows, test))]
fn parse_ipv6_at(bytes: &[u8], ip_off: usize, parser: &str) -> Option<ParsedNetworkPacket> {
    if bytes.len() < ip_off + 40 {
        return None;
    }
    let payload_len = u16::from_be_bytes([bytes[ip_off + 4], bytes[ip_off + 5]]) as usize;
    let ip_end = bytes.len().min(ip_off + 40 + payload_len);
    let mut next_header = bytes[ip_off + 6];
    let mut l4_off = ip_off + 40;
    loop {
        match next_header {
            0 | 43 | 60 => {
                if bytes.len() < l4_off + 2 {
                    return None;
                }
                next_header = bytes[l4_off];
                let len = (bytes[l4_off + 1] as usize + 1) * 8;
                l4_off += len;
            }
            44 => {
                if bytes.len() < l4_off + 8 {
                    return None;
                }
                next_header = bytes[l4_off];
                l4_off += 8;
            }
            6 => return parse_tcp_at(bytes, l4_off, ip_end, parser),
            17 => return parse_udp_at(bytes, l4_off, ip_end, parser),
            _ => return None,
        }
    }
}

#[cfg(any(windows, test))]
fn parse_udp_l4(bytes: &[u8]) -> Option<ParsedNetworkPacket> {
    parse_udp_at(bytes, 0, bytes.len(), "pktmon-udp")
}

#[cfg(any(windows, test))]
fn parse_tcp_l4(bytes: &[u8]) -> Option<ParsedNetworkPacket> {
    parse_tcp_at(bytes, 0, bytes.len(), "pktmon-tcp")
}

#[cfg(any(windows, test))]
fn parse_udp_at(
    bytes: &[u8],
    l4_off: usize,
    ip_end: usize,
    parser: &str,
) -> Option<ParsedNetworkPacket> {
    if ip_end < l4_off + 8 || bytes.len() < l4_off + 8 {
        return None;
    }
    let sport = u16::from_be_bytes([bytes[l4_off], bytes[l4_off + 1]]);
    let dport = u16::from_be_bytes([bytes[l4_off + 2], bytes[l4_off + 3]]);
    let udp_len = u16::from_be_bytes([bytes[l4_off + 4], bytes[l4_off + 5]]) as usize;
    if udp_len < 8 {
        return None;
    }
    let payload_end = ip_end.min(l4_off + udp_len).min(bytes.len());
    let payload = bytes.get(l4_off + 8..payload_end)?.to_vec();
    if payload.is_empty() {
        return None;
    }
    Some(ParsedNetworkPacket {
        proto: "udp".to_string(),
        sport: Some(sport),
        dport: Some(dport),
        seq: None,
        ack: None,
        flags: None,
        payload,
        parser: parser.to_string(),
    })
}

#[cfg(any(windows, test))]
fn parse_tcp_at(
    bytes: &[u8],
    l4_off: usize,
    ip_end: usize,
    parser: &str,
) -> Option<ParsedNetworkPacket> {
    if ip_end < l4_off + 20 || bytes.len() < l4_off + 20 {
        return None;
    }
    let sport = u16::from_be_bytes([bytes[l4_off], bytes[l4_off + 1]]);
    let dport = u16::from_be_bytes([bytes[l4_off + 2], bytes[l4_off + 3]]);
    let seq = u32::from_be_bytes([
        bytes[l4_off + 4],
        bytes[l4_off + 5],
        bytes[l4_off + 6],
        bytes[l4_off + 7],
    ]);
    let ack = u32::from_be_bytes([
        bytes[l4_off + 8],
        bytes[l4_off + 9],
        bytes[l4_off + 10],
        bytes[l4_off + 11],
    ]);
    let off_flags = u16::from_be_bytes([bytes[l4_off + 12], bytes[l4_off + 13]]);
    let tcp_header_len = ((off_flags >> 12) as usize) * 4;
    if tcp_header_len < 20 || ip_end < l4_off + tcp_header_len {
        return None;
    }
    let payload = bytes.get(l4_off + tcp_header_len..ip_end)?.to_vec();
    if payload.is_empty() {
        return None;
    }
    Some(ParsedNetworkPacket {
        proto: "tcp".to_string(),
        sport: Some(sport),
        dport: Some(dport),
        seq: Some(seq),
        ack: Some(ack),
        flags: Some(off_flags & 0x01ff),
        payload,
        parser: parser.to_string(),
    })
}

#[cfg(any(windows, test))]
fn assume_l4_payload(bytes: &[u8], parser: &str) -> Option<ParsedNetworkPacket> {
    if bytes.is_empty() {
        return None;
    }
    Some(ParsedNetworkPacket {
        proto: "payload".to_string(),
        sport: None,
        dport: None,
        seq: None,
        ack: None,
        flags: None,
        payload: bytes.to_vec(),
        parser: format!("pktmon-{parser}-payload"),
    })
}
