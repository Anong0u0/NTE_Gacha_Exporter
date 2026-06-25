use std::sync::{Arc, atomic::AtomicBool};

use anyhow::Result;
use serde::Serialize;

#[cfg(windows)]
use crate::net;
use crate::protocol::{ParseWarning, ParsedRow};
#[cfg(windows)]
use crate::protocol::{ProtocolAssembler, parse_payload_blocks};
#[cfg(windows)]
use crate::raw::{
    PacketKind, ParsedNetworkPacket, RawWriter, parse_packet_bytes, raw_record_from_parsed_packet,
};

#[cfg(windows)]
use std::sync::atomic::Ordering;
#[cfg(windows)]
use std::time::Duration;
#[cfg(windows)]
use std::time::{SystemTime, UNIX_EPOCH};

pub struct CaptureOptions {
    pub pid: u32,
    pub exe: String,
    pub ports: Vec<u16>,
    pub raw_out: Option<std::path::PathBuf>,
    pub max_packets: u64,
    pub max_decoded: u64,
    pub on_progress: Option<CaptureProgressCallback>,
}

pub type CaptureProgressCallback = Arc<dyn Fn(CaptureProgress) + Send + Sync + 'static>;

#[derive(Debug, Clone, Serialize)]
pub struct CaptureTarget {
    pub pid: u32,
    pub exe: String,
    pub interface: String,
    pub ports: Vec<u16>,
    pub bpf: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CaptureCounters {
    pub packets_seen: u64,
    pub decoded_packets: u64,
    pub dropped_packets: u64,
    pub duplicate_packets: u64,
    pub filter_restarts: u64,
}

#[derive(Debug)]
pub struct CaptureResult {
    pub target: CaptureTarget,
    pub counters: CaptureCounters,
    pub rows: Vec<ParsedRow>,
    pub warnings: Vec<ParseWarning>,
}

#[derive(Debug, Clone)]
pub struct CaptureProgress {
    pub target: CaptureTarget,
    pub counters: CaptureCounters,
    pub new_rows: Vec<ParsedRow>,
    pub row_count: usize,
    pub warning_count: usize,
}

#[cfg(windows)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct RawSignature {
    proto: String,
    sport: Option<u16>,
    dport: Option<u16>,
    payload: Vec<u8>,
}

#[cfg(windows)]
#[derive(Debug, Clone)]
struct RecentPacket {
    signature: RawSignature,
    captured_at: f64,
}

#[cfg(windows)]
const PKTMON_DUPLICATE_WINDOW_SECONDS: f64 = 0.250;

#[cfg(not(windows))]
pub fn capture_live(_options: CaptureOptions, _stop: Arc<AtomicBool>) -> Result<CaptureResult> {
    anyhow::bail!("pktmon capture requires Windows")
}

#[cfg(windows)]
pub fn capture_live(options: CaptureOptions, stop: Arc<AtomicBool>) -> Result<CaptureResult> {
    use pktmon::filter::{PktMonFilter, TransportProtocol};

    let mut ports = net::limited_filter_ports(&options.ports);
    let mut target = CaptureTarget {
        pid: options.pid,
        exe: options.exe.clone(),
        interface: "pktmon".to_string(),
        ports: ports.clone(),
        bpf: bpf(&ports),
    };
    let mut raw_writer = match options.raw_out.as_ref() {
        Some(path) => Some(RawWriter::open(path, options.pid, &ports)?),
        None => None,
    };
    let mut assembler = ProtocolAssembler::default();
    let mut warnings = Vec::new();
    let mut counters = CaptureCounters::default();
    let mut last_packet: Option<RecentPacket> = None;
    let mut rows_snapshot: Vec<ParsedRow> = Vec::new();
    let mut last_progress_seen = 0_u64;
    emit_progress(&options, &target, &counters, &[], 0, 0);

    while !stop.load(Ordering::SeqCst) {
        let mut capture = pktmon::Capture::new()?;
        for port in &ports {
            capture.add_filter(PktMonFilter {
                name: format!("NTE UDP {port}"),
                transport_protocol: Some(TransportProtocol::UDP),
                port: (*port).into(),
                ..Default::default()
            })?;
            capture.add_filter(PktMonFilter {
                name: format!("NTE TCP {port}"),
                transport_protocol: Some(TransportProtocol::TCP),
                port: (*port).into(),
                ..Default::default()
            })?;
        }
        capture.start()?;

        let mut restart_for_ports = false;
        let mut idle_ticks = 0_u32;
        loop {
            if stop.load(Ordering::SeqCst) {
                break;
            }
            if options.max_packets > 0 && counters.packets_seen >= options.max_packets {
                stop.store(true, Ordering::SeqCst);
                break;
            }
            if options.max_decoded > 0 && counters.decoded_packets >= options.max_decoded {
                stop.store(true, Ordering::SeqCst);
                break;
            }

            match capture.next_packet_timeout(Duration::from_secs(1)) {
                Ok(packet) => {
                    idle_ticks = 0;
                    counters.packets_seen += 1;
                    let kind = packet_kind(&packet.payload);
                    let bytes = packet.payload.to_vec();
                    let Some(parsed_packet) = parse_packet_bytes(bytes, kind) else {
                        counters.dropped_packets += 1;
                        continue;
                    };
                    let captured_at = now_seconds();
                    if last_packet
                        .as_ref()
                        .is_some_and(|last| last.matches(&parsed_packet, captured_at))
                    {
                        counters.duplicate_packets += 1;
                        if counters.packets_seen.saturating_sub(last_progress_seen) >= 250 {
                            emit_progress(
                                &options,
                                &target,
                                &counters,
                                &[],
                                rows_snapshot.len(),
                                warnings.len(),
                            );
                            last_progress_seen = counters.packets_seen;
                        }
                        continue;
                    }
                    let signature = RawSignature::from_packet(&parsed_packet);
                    last_packet = Some(RecentPacket {
                        signature,
                        captured_at,
                    });
                    let record = raw_record_from_parsed_packet(
                        &parsed_packet,
                        counters.packets_seen,
                        captured_at,
                    );
                    if let Some(writer) = raw_writer.as_mut() {
                        writer.write_packet(&record)?;
                    }
                    let (blocks, found_warnings) = parse_payload_blocks(
                        &parsed_packet.payload,
                        0,
                        counters.packets_seen,
                        counters.packets_seen - 1,
                    );
                    warnings.extend(found_warnings);
                    if blocks.is_empty() {
                        if counters.packets_seen.saturating_sub(last_progress_seen) >= 250 {
                            emit_progress(
                                &options,
                                &target,
                                &counters,
                                &[],
                                rows_snapshot.len(),
                                warnings.len(),
                            );
                            last_progress_seen = counters.packets_seen;
                        }
                        continue;
                    }
                    counters.decoded_packets += 1;
                    let update = assembler.add_blocks_with_update(blocks);
                    if let Some(rows) = update.rows {
                        rows_snapshot = rows;
                    }
                    emit_progress(
                        &options,
                        &target,
                        &counters,
                        &update.new_rows,
                        rows_snapshot.len(),
                        warnings.len(),
                    );
                    last_progress_seen = counters.packets_seen;
                }
                Err(error) if is_timeout(&error) => {
                    idle_ticks += 1;
                    if idle_ticks >= 3 {
                        idle_ticks = 0;
                        let latest = net::limited_filter_ports(&net::candidate_ports(options.pid)?);
                        if latest.iter().any(|port| !ports.contains(port)) {
                            ports = latest;
                            target.ports = ports.clone();
                            target.bpf = bpf(&ports);
                            counters.filter_restarts += 1;
                            restart_for_ports = true;
                            break;
                        }
                    }
                }
                Err(error) => return Err(error.into()),
            }
        }
        let _ = capture.stop();
        let _ = capture.unload();
        if !restart_for_ports {
            break;
        }
    }

    if let Some(writer) = raw_writer.as_mut() {
        writer.write_stop(
            counters.packets_seen,
            counters.decoded_packets,
            counters.dropped_packets,
            counters.duplicate_packets,
        )?;
    }
    let mut rows = assembler.rows();
    warnings.extend(assembler.warnings);
    rows.shrink_to_fit();
    emit_progress(
        &options,
        &target,
        &counters,
        &[],
        rows.len(),
        warnings.len(),
    );
    Ok(CaptureResult {
        target,
        counters,
        rows,
        warnings,
    })
}

#[cfg(windows)]
impl RawSignature {
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
impl RecentPacket {
    fn matches(&self, packet: &ParsedNetworkPacket, captured_at: f64) -> bool {
        self.signature.proto.as_str() == packet.proto.as_str()
            && self.signature.sport == packet.sport
            && self.signature.dport == packet.dport
            && self.signature.payload.as_slice() == packet.payload.as_slice()
            && captured_at - self.captured_at <= PKTMON_DUPLICATE_WINDOW_SECONDS
    }
}

#[cfg(windows)]
fn emit_progress(
    options: &CaptureOptions,
    target: &CaptureTarget,
    counters: &CaptureCounters,
    new_rows: &[ParsedRow],
    row_count: usize,
    warning_count: usize,
) {
    let Some(callback) = options.on_progress.as_ref() else {
        return;
    };
    callback(CaptureProgress {
        target: target.clone(),
        counters: counters.clone(),
        new_rows: new_rows.to_vec(),
        row_count,
        warning_count,
    });
}

#[cfg(windows)]
fn packet_kind(payload: &pktmon::PacketPayload) -> PacketKind {
    match payload {
        pktmon::PacketPayload::Ethernet(_) | pktmon::PacketPayload::WiFi(_) => PacketKind::Ethernet,
        pktmon::PacketPayload::IP(_) => PacketKind::Ip,
        pktmon::PacketPayload::TCP(_) => PacketKind::Tcp,
        pktmon::PacketPayload::UDP(_) => PacketKind::Udp,
        pktmon::PacketPayload::L4Payload(_) => PacketKind::L4Payload,
        _ => PacketKind::Unknown,
    }
}

#[cfg(windows)]
fn is_timeout(error: &impl std::fmt::Display) -> bool {
    format!("{error}")
        .to_ascii_lowercase()
        .contains("timed out")
}

#[cfg(windows)]
fn bpf(ports: &[u16]) -> String {
    ports
        .iter()
        .map(|port| format!("port {port}"))
        .collect::<Vec<_>>()
        .join(" or ")
}

#[cfg(windows)]
fn now_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs_f64())
        .unwrap_or_default()
}
