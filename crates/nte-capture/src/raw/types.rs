#[cfg(any(windows, test))]
#[derive(Debug, Clone, Copy)]
pub enum PacketKind {
    Unknown,
    Ethernet,
    Ip,
    Tcp,
    Udp,
    L4Payload,
}

#[cfg(any(windows, test))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawPacketRecord {
    #[serde(rename = "type")]
    pub typ: String,
    pub schema_version: u32,
    pub captured_at: f64,
    pub capture_index: u64,
    pub proto: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sport: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dport: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seq: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ack: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flags: Option<u16>,
    pub parser: String,
    pub size: usize,
    pub payload_b64: String,
}

#[cfg(windows)]
#[derive(Debug, Clone, Serialize)]
pub struct CaptureStartRecord {
    #[serde(rename = "type")]
    typ: &'static str,
    schema_version: u32,
    pid: u32,
    iface: &'static str,
    ports: Vec<u16>,
    bpf: String,
    filter_mode: String,
    pppoe_detection: crate::net::PppoeDetection,
}

#[cfg(windows)]
#[derive(Debug, Clone, Serialize)]
pub struct CaptureStopRecord {
    #[serde(rename = "type")]
    typ: &'static str,
    schema_version: u32,
    seen: u64,
    decoded_packets: u64,
    dropped: u64,
    duplicate_packets: u64,
}

#[derive(Debug, Clone)]
pub struct RawReadResult {
    pub rows: Vec<ParsedRow>,
    pub warnings: Vec<ParseWarning>,
}

#[cfg(any(windows, test))]
#[derive(Debug)]
pub(crate) struct ParsedNetworkPacket {
    pub(crate) proto: String,
    pub(crate) sport: Option<u16>,
    pub(crate) dport: Option<u16>,
    pub(crate) seq: Option<u32>,
    pub(crate) ack: Option<u32>,
    pub(crate) flags: Option<u16>,
    pub(crate) payload: Vec<u8>,
    pub(crate) parser: String,
}
