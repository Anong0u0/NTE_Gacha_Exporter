#[cfg(windows)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct DiagnosticRawSignature {
    proto: String,
    sport: Option<u16>,
    dport: Option<u16>,
    payload: Vec<u8>,
}

#[cfg(windows)]
#[derive(Debug, Clone)]
struct DiagnosticRecentPacket {
    signature: DiagnosticRawSignature,
    captured_at: f64,
}

#[cfg(windows)]
impl DiagnosticRawSignature {
    fn from_packet(packet: &ParsedNetworkPacket) -> Self {
        Self {
            proto: packet.proto.clone(),
            sport: packet.sport,
            dport: packet.dport,
            payload: packet.payload.clone(),
        }
    }
}

#[cfg(windows)]
impl DiagnosticRecentPacket {
    fn matches(&self, packet: &ParsedNetworkPacket, captured_at: f64) -> bool {
        self.signature.proto.as_str() == packet.proto.as_str()
            && self.signature.sport == packet.sport
            && self.signature.dport == packet.dport
            && self.signature.payload.as_slice() == packet.payload.as_slice()
            && captured_at - self.captured_at <= DIAGNOSTIC_DUPLICATE_WINDOW_SECONDS
    }
}
