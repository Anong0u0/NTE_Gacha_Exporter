#![cfg_attr(not(windows), allow(dead_code))]

use std::sync::{atomic::AtomicBool, Arc};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use serde::Serialize;

#[cfg(windows)]
use crate::capture_net;
#[cfg(windows)]
use crate::capture_protocol::{parse_payload_blocks, ProtocolAssembler};
use crate::capture_protocol::{ParseWarning, ParsedRow};
#[cfg(windows)]
use crate::capture_raw::{raw_record_from_packet_bytes, PacketKind, RawWriter};

#[cfg(windows)]
use std::sync::atomic::Ordering;
#[cfg(windows)]
use std::time::Duration;

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
    pub rows: Vec<ParsedRow>,
    pub warnings: Vec<ParseWarning>,
}

#[cfg(windows)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct RawSignature {
    proto: String,
    sport: Option<u16>,
    dport: Option<u16>,
    payload_b64: String,
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

    let mut ports = capture_net::limited_filter_ports(&options.ports);
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
    emit_progress(&options, &target, &counters, &rows_snapshot, &warnings);

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
        eprintln!(
            "capture started pid={} ports={} max_packets={} max_decoded={}",
            options.pid,
            ports
                .iter()
                .map(u16::to_string)
                .collect::<Vec<_>>()
                .join(","),
            options.max_packets,
            options.max_decoded
        );

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
                    let bytes = packet.payload.to_vec().clone();
                    let Some(record) = raw_record_from_packet_bytes(
                        &bytes,
                        kind,
                        counters.packets_seen,
                        now_seconds(),
                    ) else {
                        counters.dropped_packets += 1;
                        continue;
                    };
                    let signature = RawSignature::from_record(&record);
                    if last_packet.as_ref().is_some_and(|last| {
                        last.signature == signature
                            && record.captured_at - last.captured_at
                                <= PKTMON_DUPLICATE_WINDOW_SECONDS
                    }) {
                        counters.duplicate_packets += 1;
                        if counters.packets_seen.saturating_sub(last_progress_seen) >= 250 {
                            emit_progress(&options, &target, &counters, &rows_snapshot, &warnings);
                            last_progress_seen = counters.packets_seen;
                        }
                        continue;
                    }
                    last_packet = Some(RecentPacket {
                        signature,
                        captured_at: record.captured_at,
                    });
                    if let Some(writer) = raw_writer.as_mut() {
                        writer.write_packet(&record)?;
                    }
                    let payload = base64::Engine::decode(
                        &base64::engine::general_purpose::STANDARD,
                        &record.payload_b64,
                    )?;
                    let (blocks, found_warnings) = parse_payload_blocks(
                        &payload,
                        0,
                        counters.packets_seen,
                        counters.packets_seen - 1,
                    );
                    warnings.extend(found_warnings);
                    if blocks.is_empty() {
                        if counters.packets_seen.saturating_sub(last_progress_seen) >= 250 {
                            emit_progress(&options, &target, &counters, &rows_snapshot, &warnings);
                            last_progress_seen = counters.packets_seen;
                        }
                        continue;
                    }
                    counters.decoded_packets += 1;
                    let new_rows = assembler.add_blocks(blocks);
                    rows_snapshot = assembler.rows();
                    emit_progress(&options, &target, &counters, &rows_snapshot, &warnings);
                    last_progress_seen = counters.packets_seen;
                    if !new_rows.is_empty() {
                        eprintln!(
                            "decoded rows total={} packet={} decoded_packets={}",
                            rows_snapshot.len(),
                            counters.packets_seen,
                            counters.decoded_packets
                        );
                    }
                }
                Err(error) if is_timeout(&error) => {
                    idle_ticks += 1;
                    if idle_ticks >= 3 {
                        idle_ticks = 0;
                        let latest = capture_net::limited_filter_ports(
                            &capture_net::candidate_ports(options.pid)?,
                        );
                        if latest.iter().any(|port| !ports.contains(port)) {
                            ports = latest;
                            target.ports = ports.clone();
                            target.bpf = bpf(&ports);
                            counters.filter_restarts += 1;
                            restart_for_ports = true;
                            eprintln!(
                                "ports changed; restarting filters: {}",
                                ports
                                    .iter()
                                    .map(u16::to_string)
                                    .collect::<Vec<_>>()
                                    .join(",")
                            );
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
    emit_progress(&options, &target, &counters, &rows, &warnings);
    Ok(CaptureResult {
        target,
        counters,
        rows,
        warnings,
    })
}

#[cfg(windows)]
impl RawSignature {
    fn from_record(record: &crate::capture_raw::RawPacketRecord) -> Self {
        Self {
            proto: record.proto.clone(),
            sport: record.sport,
            dport: record.dport,
            payload_b64: record.payload_b64.clone(),
        }
    }
}

#[cfg(windows)]
fn emit_progress(
    options: &CaptureOptions,
    target: &CaptureTarget,
    counters: &CaptureCounters,
    rows: &[ParsedRow],
    warnings: &[ParseWarning],
) {
    let Some(callback) = options.on_progress.as_ref() else {
        return;
    };
    callback(CaptureProgress {
        target: target.clone(),
        counters: counters.clone(),
        rows: rows.to_vec(),
        warnings: warnings.to_vec(),
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

fn bpf(ports: &[u16]) -> String {
    ports
        .iter()
        .map(|port| format!("port {port}"))
        .collect::<Vec<_>>()
        .join(" or ")
}

fn now_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs_f64())
        .unwrap_or_default()
}
