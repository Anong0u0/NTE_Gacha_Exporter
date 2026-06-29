use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::Duration;

use anyhow::Result;
use serde::Serialize;

use crate::live::CaptureTarget;
use crate::protocol::ParseWarning;

#[cfg(windows)]
use crate::net;
#[cfg(windows)]
use crate::protocol::{ProtocolAssembler, RecordType, parse_payload_blocks};
#[cfg(windows)]
use crate::raw::{
    PacketKind, ParsedNetworkPacket, RawWriter, parse_packet_bytes, raw_record_from_parsed_packet,
};

#[cfg(windows)]
use std::fs::File;
#[cfg(windows)]
use std::io::{BufWriter, Write};
#[cfg(windows)]
use std::sync::atomic::Ordering;
#[cfg(windows)]
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[cfg(windows)]
use base64::Engine;

pub struct DiagnosticCaptureOptions {
    pub pid: u32,
    pub exe: String,
    pub ports: Vec<u16>,
    pub raw_out: Option<PathBuf>,
    pub dropped_samples_out: Option<PathBuf>,
    pub duration: Duration,
    pub max_dropped_samples: usize,
    pub on_progress: Option<DiagnosticCaptureProgressCallback>,
}

pub type DiagnosticCaptureProgressCallback =
    Arc<dyn Fn(DiagnosticCaptureProgress) + Send + Sync + 'static>;

#[derive(Debug, Clone, Default, Serialize)]
pub struct DiagnosticCaptureCounters {
    pub packets_seen: u64,
    pub decoded_packets: u64,
    pub dropped_packets: u64,
    pub duplicate_packets: u64,
    pub filter_restarts: u64,
    pub raw_packets_written: u64,
    pub dropped_samples_written: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DiagnosticMarkerHits {
    pub monopoly_blocks: u64,
    pub fork_blocks: u64,
    pub monopoly_rows: u64,
    pub fork_rows: u64,
    pub monopoly_parse_warnings: u64,
    pub fork_parse_warnings: u64,
}

impl DiagnosticMarkerHits {
    pub fn any(&self) -> bool {
        self.monopoly_blocks > 0
            || self.fork_blocks > 0
            || self.monopoly_parse_warnings > 0
            || self.fork_parse_warnings > 0
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DiagnosticCaptureSummary {
    pub rows_count: u64,
    pub warning_count: u64,
    pub packet_kind_counts: BTreeMap<String, u64>,
    pub parser_counts: BTreeMap<String, u64>,
    pub proto_counts: BTreeMap<String, u64>,
    pub port_pair_counts: BTreeMap<String, u64>,
    pub parsed_payload_size_buckets: BTreeMap<String, u64>,
    pub dropped_packet_size_buckets: BTreeMap<String, u64>,
    pub small_parsed_payload_packets: u64,
    pub marker_hits: DiagnosticMarkerHits,
    pub warning_code_counts: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticCaptureProgress {
    pub target: CaptureTarget,
    pub counters: DiagnosticCaptureCounters,
    pub elapsed_seconds: f64,
    pub rows_count: u64,
    pub warning_count: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticCaptureResult {
    pub target: CaptureTarget,
    pub counters: DiagnosticCaptureCounters,
    pub summary: DiagnosticCaptureSummary,
    pub warnings: Vec<ParseWarning>,
    pub elapsed_seconds: f64,
}

#[cfg(not(windows))]
pub fn run_diagnostic_capture(
    _options: DiagnosticCaptureOptions,
    _stop: Arc<AtomicBool>,
) -> Result<DiagnosticCaptureResult> {
    anyhow::bail!("pktmon diagnostic capture requires Windows")
}

#[cfg(windows)]
pub fn run_diagnostic_capture(
    options: DiagnosticCaptureOptions,
    stop: Arc<AtomicBool>,
) -> Result<DiagnosticCaptureResult> {
    use pktmon::filter::{PktMonFilter, TransportProtocol};

    let started_at = Instant::now();
    let duration = if options.duration.is_zero() {
        Duration::from_secs(30)
    } else {
        options.duration
    };
    let mut ports = net::limited_filter_ports(&options.ports);
    let mut target = CaptureTarget {
        pid: options.pid,
        exe: options.exe.clone(),
        interface: "pktmon".to_string(),
        ports: ports.clone(),
        bpf: diagnostic_bpf(&ports),
    };
    let mut raw_writer = match options.raw_out.as_ref() {
        Some(path) => Some(RawWriter::open(path, options.pid, &ports)?),
        None => None,
    };
    let mut dropped_writer = match options.dropped_samples_out.as_ref() {
        Some(path) => Some(DroppedSampleWriter::open(path, options.pid, &ports)?),
        None => None,
    };
    let mut assembler = ProtocolAssembler::default();
    let mut warnings = Vec::new();
    let mut counters = DiagnosticCaptureCounters::default();
    let mut summary = DiagnosticCaptureSummary::default();
    let mut last_packet: Option<DiagnosticRecentPacket> = None;
    let mut last_progress_seen = 0_u64;
    emit_progress(
        &options,
        &target,
        &counters,
        started_at,
        assembler.rows().len() as u64,
        warnings.len() as u64,
    );

    while !stop.load(Ordering::SeqCst) && started_at.elapsed() < duration {
        let mut capture = pktmon::Capture::new()?;
        for port in &ports {
            capture.add_filter(PktMonFilter {
                name: format!("NTE diagnostic UDP {port}"),
                transport_protocol: Some(TransportProtocol::UDP),
                port: (*port).into(),
                ..Default::default()
            })?;
            capture.add_filter(PktMonFilter {
                name: format!("NTE diagnostic TCP {port}"),
                transport_protocol: Some(TransportProtocol::TCP),
                port: (*port).into(),
                ..Default::default()
            })?;
        }
        capture.start()?;

        let mut restart_for_ports = false;
        let mut idle_ticks = 0_u32;
        loop {
            if stop.load(Ordering::SeqCst) || started_at.elapsed() >= duration {
                stop.store(true, Ordering::SeqCst);
                break;
            }

            match capture.next_packet_timeout(Duration::from_secs(1)) {
                Ok(packet) => {
                    idle_ticks = 0;
                    counters.packets_seen += 1;
                    let kind = packet_kind(&packet.payload);
                    increment(&mut summary.packet_kind_counts, packet_kind_name(kind));
                    let bytes = packet.payload.to_vec();
                    let Some(parsed_packet) = parse_packet_bytes(&bytes, kind) else {
                        counters.dropped_packets += 1;
                        increment(
                            &mut summary.dropped_packet_size_buckets,
                            size_bucket(bytes.len()),
                        );
                        if counters.dropped_samples_written < options.max_dropped_samples as u64 {
                            if let Some(writer) = dropped_writer.as_mut() {
                                writer.write_sample(&DroppedPacketSample {
                                    typ: "dropped_packet_sample",
                                    schema_version: 1,
                                    captured_at: now_seconds(),
                                    capture_index: counters.packets_seen,
                                    packet_kind: packet_kind_name(kind).to_string(),
                                    size: bytes.len(),
                                    payload_prefix_b64: payload_prefix_b64(&bytes),
                                    payload_truncated: bytes.len() > DROPPED_SAMPLE_PREFIX_BYTES,
                                })?;
                                counters.dropped_samples_written += 1;
                            }
                        }
                        continue;
                    };

                    let captured_at = now_seconds();
                    if last_packet
                        .as_ref()
                        .is_some_and(|last| last.matches(&parsed_packet, captured_at))
                    {
                        counters.duplicate_packets += 1;
                        maybe_emit_progress(
                            &options,
                            &target,
                            &counters,
                            started_at,
                            &mut assembler,
                            &warnings,
                            &mut last_progress_seen,
                        );
                        continue;
                    }
                    last_packet = Some(DiagnosticRecentPacket {
                        signature: DiagnosticRawSignature::from_packet(&parsed_packet),
                        captured_at,
                    });

                    add_parsed_summary(&mut summary, &parsed_packet);
                    let record = raw_record_from_parsed_packet(
                        &parsed_packet,
                        counters.packets_seen,
                        captured_at,
                    );
                    if let Some(writer) = raw_writer.as_mut() {
                        writer.write_packet(&record)?;
                        counters.raw_packets_written += 1;
                    }

                    let (blocks, found_warnings) = parse_payload_blocks(
                        &parsed_packet.payload,
                        0,
                        counters.packets_seen,
                        counters.packets_seen - 1,
                    );
                    add_block_summary(&mut summary, &blocks);
                    add_warning_summary(&mut summary, &found_warnings);
                    warnings.extend(found_warnings);
                    if blocks.is_empty() {
                        maybe_emit_progress(
                            &options,
                            &target,
                            &counters,
                            started_at,
                            &mut assembler,
                            &warnings,
                            &mut last_progress_seen,
                        );
                        continue;
                    }
                    counters.decoded_packets += 1;
                    let _ = assembler.add_blocks_with_update(blocks);
                    maybe_emit_progress(
                        &options,
                        &target,
                        &counters,
                        started_at,
                        &mut assembler,
                        &warnings,
                        &mut last_progress_seen,
                    );
                }
                Err(error) if is_timeout(&error) => {
                    idle_ticks += 1;
                    emit_progress(
                        &options,
                        &target,
                        &counters,
                        started_at,
                        assembler.rows().len() as u64,
                        warnings.len() as u64,
                    );
                    if idle_ticks >= 3 {
                        idle_ticks = 0;
                        let latest = net::limited_filter_ports(&net::candidate_ports(options.pid)?);
                        if latest.iter().any(|port| !ports.contains(port)) {
                            ports = latest;
                            target.ports = ports.clone();
                            target.bpf = diagnostic_bpf(&ports);
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
    if let Some(writer) = dropped_writer.as_mut() {
        writer.write_stop(&counters)?;
    }
    let assembler_warnings = std::mem::take(&mut assembler.warnings);
    add_warning_summary(&mut summary, &assembler_warnings);
    warnings.extend(assembler_warnings);
    summary.rows_count = assembler.rows().len() as u64;
    summary.warning_count = warnings.len() as u64;
    emit_progress(
        &options,
        &target,
        &counters,
        started_at,
        summary.rows_count,
        summary.warning_count,
    );
    Ok(DiagnosticCaptureResult {
        target,
        counters,
        summary,
        warnings,
        elapsed_seconds: started_at.elapsed().as_secs_f64(),
    })
}

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

#[cfg(windows)]
#[derive(Debug, Clone, Serialize)]
struct DroppedPacketSample {
    #[serde(rename = "type")]
    typ: &'static str,
    schema_version: u32,
    captured_at: f64,
    capture_index: u64,
    packet_kind: String,
    size: usize,
    payload_prefix_b64: String,
    payload_truncated: bool,
}

#[cfg(windows)]
#[derive(Debug, Clone, Serialize)]
struct DroppedStartRecord {
    #[serde(rename = "type")]
    typ: &'static str,
    schema_version: u32,
    pid: u32,
    ports: Vec<u16>,
}

#[cfg(windows)]
#[derive(Debug, Clone, Serialize)]
struct DroppedStopRecord<'a> {
    #[serde(rename = "type")]
    typ: &'static str,
    schema_version: u32,
    counters: &'a DiagnosticCaptureCounters,
}

#[cfg(windows)]
struct DroppedSampleWriter {
    writer: BufWriter<File>,
}

#[cfg(windows)]
impl DroppedSampleWriter {
    fn open(path: &std::path::Path, pid: u32, ports: &[u16]) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = File::create(path)?;
        let mut writer = Self {
            writer: BufWriter::new(file),
        };
        writer.write_json(&DroppedStartRecord {
            typ: "dropped_capture_start",
            schema_version: 1,
            pid,
            ports: ports.to_vec(),
        })?;
        Ok(writer)
    }

    fn write_sample(&mut self, sample: &DroppedPacketSample) -> Result<()> {
        self.write_json(sample)
    }

    fn write_stop(&mut self, counters: &DiagnosticCaptureCounters) -> Result<()> {
        self.write_json(&DroppedStopRecord {
            typ: "dropped_capture_stop",
            schema_version: 1,
            counters,
        })?;
        self.writer.flush()?;
        Ok(())
    }

    fn write_json(&mut self, value: &impl Serialize) -> Result<()> {
        serde_json::to_writer(&mut self.writer, value)?;
        self.writer.write_all(b"\n")?;
        Ok(())
    }
}

#[cfg(windows)]
const DIAGNOSTIC_DUPLICATE_WINDOW_SECONDS: f64 = 0.250;
#[cfg(windows)]
const DROPPED_SAMPLE_PREFIX_BYTES: usize = 512;

#[cfg(windows)]
fn maybe_emit_progress(
    options: &DiagnosticCaptureOptions,
    target: &CaptureTarget,
    counters: &DiagnosticCaptureCounters,
    started_at: Instant,
    assembler: &mut ProtocolAssembler,
    warnings: &[ParseWarning],
    last_progress_seen: &mut u64,
) {
    if counters.packets_seen.saturating_sub(*last_progress_seen) < 250 {
        return;
    }
    *last_progress_seen = counters.packets_seen;
    emit_progress(
        options,
        target,
        counters,
        started_at,
        assembler.rows().len() as u64,
        warnings.len() as u64,
    );
}

#[cfg(windows)]
fn emit_progress(
    options: &DiagnosticCaptureOptions,
    target: &CaptureTarget,
    counters: &DiagnosticCaptureCounters,
    started_at: Instant,
    rows_count: u64,
    warning_count: u64,
) {
    let Some(callback) = options.on_progress.as_ref() else {
        return;
    };
    callback(DiagnosticCaptureProgress {
        target: target.clone(),
        counters: counters.clone(),
        elapsed_seconds: started_at.elapsed().as_secs_f64(),
        rows_count,
        warning_count,
    });
}

#[cfg(windows)]
fn add_parsed_summary(summary: &mut DiagnosticCaptureSummary, packet: &ParsedNetworkPacket) {
    increment(&mut summary.parser_counts, &packet.parser);
    increment(&mut summary.proto_counts, &packet.proto);
    increment(
        &mut summary.parsed_payload_size_buckets,
        size_bucket(packet.payload.len()),
    );
    if packet.payload.len() <= 64 {
        summary.small_parsed_payload_packets += 1;
    }
    increment(&mut summary.port_pair_counts, port_pair(packet));
}

#[cfg(windows)]
fn add_block_summary(
    summary: &mut DiagnosticCaptureSummary,
    blocks: &[crate::protocol::ParsedBlock],
) {
    for block in blocks {
        match block.record_type {
            RecordType::Monopoly => {
                summary.marker_hits.monopoly_blocks += 1;
                summary.marker_hits.monopoly_rows += block.rows.len() as u64;
            }
            RecordType::Fork => {
                summary.marker_hits.fork_blocks += 1;
                summary.marker_hits.fork_rows += block.rows.len() as u64;
            }
        }
    }
}

#[cfg(windows)]
fn add_warning_summary(summary: &mut DiagnosticCaptureSummary, warnings: &[ParseWarning]) {
    for warning in warnings {
        increment(&mut summary.warning_code_counts, &warning.code);
        if warning.message.contains("FMonopolyLotteryRecordData") {
            summary.marker_hits.monopoly_parse_warnings += 1;
        }
        if warning.message.contains("FForkLotteryRecordData") {
            summary.marker_hits.fork_parse_warnings += 1;
        }
    }
}

#[cfg(windows)]
fn port_pair(packet: &ParsedNetworkPacket) -> String {
    match (packet.sport, packet.dport) {
        (Some(sport), Some(dport)) => format!("{sport}->{dport}"),
        (Some(sport), None) => format!("{sport}->?"),
        (None, Some(dport)) => format!("?->{dport}"),
        (None, None) => "unknown".to_string(),
    }
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
fn packet_kind_name(kind: PacketKind) -> &'static str {
    match kind {
        PacketKind::Unknown => "unknown",
        PacketKind::Ethernet => "ethernet",
        PacketKind::Ip => "ip",
        PacketKind::Tcp => "tcp",
        PacketKind::Udp => "udp",
        PacketKind::L4Payload => "l4_payload",
    }
}

#[cfg(windows)]
fn increment(map: &mut BTreeMap<String, u64>, key: impl AsRef<str>) {
    *map.entry(key.as_ref().to_string()).or_default() += 1;
}

#[cfg(windows)]
fn size_bucket(size: usize) -> &'static str {
    match size {
        0..=31 => "0-31",
        32..=63 => "32-63",
        64..=127 => "64-127",
        128..=511 => "128-511",
        512..=1499 => "512-1499",
        _ => "1500+",
    }
}

#[cfg(windows)]
fn diagnostic_bpf(ports: &[u16]) -> String {
    ports
        .iter()
        .map(|port| format!("port {port}"))
        .collect::<Vec<_>>()
        .join(" or ")
}

#[cfg(windows)]
fn payload_prefix_b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD
        .encode(bytes.get(..DROPPED_SAMPLE_PREFIX_BYTES).unwrap_or(bytes))
}

#[cfg(windows)]
fn is_timeout(error: &impl std::fmt::Display) -> bool {
    format!("{error}")
        .to_ascii_lowercase()
        .contains("timed out")
}

#[cfg(windows)]
fn now_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs_f64())
        .unwrap_or_default()
}
