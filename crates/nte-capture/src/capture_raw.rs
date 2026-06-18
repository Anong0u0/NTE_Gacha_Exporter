#![cfg_attr(not(windows), allow(dead_code))]

use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::capture_protocol::{ParseWarning, ParsedRow, ProtocolAssembler, parse_payload_blocks};

#[derive(Debug, Clone, Copy)]
pub enum PacketKind {
    Unknown,
    Ethernet,
    Ip,
    Tcp,
    Udp,
    L4Payload,
}

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

#[derive(Debug, Clone, Serialize)]
pub struct CaptureStartRecord {
    #[serde(rename = "type")]
    typ: &'static str,
    schema_version: u32,
    pid: u32,
    iface: &'static str,
    ports: Vec<u16>,
    bpf: String,
}

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

pub struct RawWriter {
    writer: BufWriter<File>,
}

impl RawWriter {
    pub fn open(path: &Path, pid: u32, ports: &[u16]) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
        let mut writer = Self {
            writer: BufWriter::new(file),
        };
        writer.write_json(&CaptureStartRecord {
            typ: "capture_start",
            schema_version: 1,
            pid,
            iface: "pktmon",
            ports: ports.to_vec(),
            bpf: ports
                .iter()
                .map(|port| format!("port {port}"))
                .collect::<Vec<_>>()
                .join(" or "),
        })?;
        Ok(writer)
    }

    pub fn write_packet(&mut self, record: &RawPacketRecord) -> Result<()> {
        self.write_json(record)
    }

    pub fn write_stop(
        &mut self,
        seen: u64,
        decoded_packets: u64,
        dropped: u64,
        duplicate_packets: u64,
    ) -> Result<()> {
        self.write_json(&CaptureStopRecord {
            typ: "capture_stop",
            schema_version: 1,
            seen,
            decoded_packets,
            dropped,
            duplicate_packets,
        })?;
        self.writer.flush().context("flush raw writer")
    }

    fn write_json(&mut self, value: &impl Serialize) -> Result<()> {
        serde_json::to_writer(&mut self.writer, value)?;
        self.writer.write_all(b"\n")?;
        Ok(())
    }
}

pub fn read_raw_capture(path: &Path) -> Result<RawReadResult> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut assembler = ProtocolAssembler::default();
    let mut warnings = Vec::new();
    let mut session_index: i64 = -1;
    let mut packet_index = 0_u64;
    let mut in_session = false;
    let mut saw_session = false;

    for (line_index, line) in BufReader::new(file).lines().enumerate() {
        let line_no = line_index as u64 + 1;
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let value: serde_json::Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(error) => {
                warnings.push(ParseWarning::new(
                    "bad_jsonl",
                    format!("line {line_no}: {error}"),
                ));
                continue;
            }
        };
        let Some(object) = value.as_object() else {
            warnings.push(ParseWarning::new(
                "bad_jsonl",
                format!("line {line_no}: record is not an object"),
            ));
            continue;
        };
        match object.get("type").and_then(serde_json::Value::as_str) {
            Some("capture_start") => {
                saw_session = true;
                in_session = true;
                session_index += 1;
                packet_index = 0;
            }
            Some("capture_stop") => in_session = false,
            Some("packet") if in_session => {
                let payload = object
                    .get("payload_b64")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|text| base64::engine::general_purpose::STANDARD.decode(text).ok());
                let Some(payload) = payload else {
                    warnings.push(ParseWarning::new("bad_packet", "invalid payload_b64"));
                    packet_index += 1;
                    continue;
                };
                let (blocks, found_warnings) =
                    parse_payload_blocks(&payload, session_index as u64, line_no, packet_index);
                packet_index += 1;
                warnings.extend(found_warnings);
                assembler.add_blocks(blocks);
            }
            _ => {}
        }
    }

    if !saw_session {
        anyhow::bail!("raw capture has no capture_start records");
    }
    let mut rows = assembler.rows();
    warnings.extend(assembler.warnings);
    rows.shrink_to_fit();
    Ok(RawReadResult { rows, warnings })
}

pub(crate) fn raw_record_from_parsed_packet(
    parsed: &ParsedNetworkPacket,
    capture_index: u64,
    captured_at: f64,
) -> RawPacketRecord {
    RawPacketRecord {
        typ: "packet".to_string(),
        schema_version: 1,
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

pub(crate) fn parse_packet_bytes(bytes: &[u8], kind: PacketKind) -> Option<ParsedNetworkPacket> {
    parse_network_packet(bytes, kind)
}

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
        _ => None,
    }
}

fn parse_raw_ipv4_offsets(bytes: &[u8]) -> Option<ParsedNetworkPacket> {
    [14_usize, 0]
        .into_iter()
        .find_map(|offset| parse_ip_at(bytes, offset, "pktmon-raw-ip"))
}

fn parse_ip(bytes: &[u8], parser: &str) -> Option<ParsedNetworkPacket> {
    parse_ip_at(bytes, 0, parser)
}

fn parse_ip_at(bytes: &[u8], offset: usize, parser: &str) -> Option<ParsedNetworkPacket> {
    let version = bytes.get(offset)? >> 4;
    match version {
        4 => parse_ipv4_at(bytes, offset, parser),
        6 => parse_ipv6_at(bytes, offset, parser),
        _ => None,
    }
}

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

fn parse_udp_l4(bytes: &[u8]) -> Option<ParsedNetworkPacket> {
    parse_udp_at(bytes, 0, bytes.len(), "pktmon-udp")
}

fn parse_tcp_l4(bytes: &[u8]) -> Option<ParsedNetworkPacket> {
    parse_tcp_at(bytes, 0, bytes.len(), "pktmon-tcp")
}

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
}
