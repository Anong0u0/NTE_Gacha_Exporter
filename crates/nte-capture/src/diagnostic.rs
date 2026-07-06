use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::Duration;

use anyhow::Result;
use serde::Serialize;

#[cfg(windows)]
use crate::live::{
    CaptureAttemptSummary, CaptureCounters, CaptureStrategyKind, bpf, should_write_raw_packet,
};
use crate::live::{CaptureStrategy, CaptureTarget};
use crate::protocol::ParseWarning;

#[cfg(windows)]
use crate::net;
#[cfg(windows)]
use crate::protocol::{ProtocolAssembler, RecordType, parse_payload_blocks};
#[cfg(any(windows, test))]
use crate::raw::PacketKind;
#[cfg(windows)]
use crate::raw::{
    ParsedNetworkPacket, RawWriter, parse_packet_bytes, raw_record_from_parsed_packet,
};

#[cfg(windows)]
use std::fs::File;
#[cfg(windows)]
use std::io::{BufWriter, Write};
#[cfg(any(windows, test))]
use std::sync::atomic::Ordering;
#[cfg(any(windows, test))]
use std::time::Instant;
#[cfg(windows)]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(any(windows, test))]
use base64::Engine;

pub struct DiagnosticCaptureOptions {
    pub pid: u32,
    pub exe: String,
    pub ports: Vec<u16>,
    pub pppoe_detection: Option<crate::net::PppoeDetection>,
    pub strategy: Option<CaptureStrategy>,
    pub raw_out: Option<PathBuf>,
    pub raw_append: bool,
    pub dropped_samples_out: Option<PathBuf>,
    pub duration: Duration,
    pub max_dropped_samples: usize,
    pub max_full_dropped_samples: usize,
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
    pub dropped_full_samples_written: u64,
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
    pub dropped_evidence: DiagnosticDroppedEvidenceSummary,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DiagnosticDroppedEvidenceSummary {
    pub layer_chain_counts: BTreeMap<String, u64>,
    pub failure_reason_counts: BTreeMap<String, u64>,
    pub encapsulation_counts: BTreeMap<String, u64>,
    pub ethertype_counts: BTreeMap<String, u64>,
    pub ppp_protocol_counts: BTreeMap<String, u64>,
    pub ip_protocol_counts: BTreeMap<String, u64>,
    pub examples: Vec<DiagnosticDroppedEvidenceExample>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticDroppedEvidenceExample {
    pub capture_index: u64,
    pub packet_kind: String,
    pub size: usize,
    pub layer_chain: Vec<String>,
    pub failure_reason: String,
    pub offsets: DiagnosticDroppedOffsets,
    pub ethertype: Option<String>,
    pub ppp_protocol: Option<String>,
    pub ip_protocol: Option<String>,
    pub prefix_hex: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DiagnosticDroppedOffsets {
    pub ethertype_offset: Option<usize>,
    pub vlan_offsets: Vec<usize>,
    pub pppoe_offset: Option<usize>,
    pub ppp_protocol_offset: Option<usize>,
    pub inner_ip_offset: Option<usize>,
    pub l4_offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticDroppedPacketAnalysis {
    pub packet_kind: String,
    pub layer_chain: Vec<String>,
    pub failure_reason: String,
    pub offsets: DiagnosticDroppedOffsets,
    pub ethertype: Option<String>,
    pub vlan_tags: Vec<DiagnosticVlanTagEvidence>,
    pub pppoe: Option<DiagnosticPppoeEvidence>,
    pub ppp_protocol: Option<String>,
    pub ip: Option<DiagnosticIpEvidence>,
    pub transport: Option<DiagnosticTransportEvidence>,
    pub prefix_hex: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticVlanTagEvidence {
    pub offset: usize,
    pub tpid: String,
    pub tci: String,
    pub vid: u16,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticPppoeEvidence {
    pub offset: usize,
    pub version: u8,
    #[serde(rename = "type")]
    pub typ: u8,
    pub code: u8,
    pub session_id: String,
    pub length: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticIpEvidence {
    pub version: u8,
    pub offset: usize,
    pub header_len: usize,
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticTransportEvidence {
    pub protocol: String,
    pub offset: usize,
    pub sport: Option<u16>,
    pub dport: Option<u16>,
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

include!("diagnostic/run.rs");
include!("diagnostic/dedup.rs");
include!("diagnostic/drop_analysis.rs");
include!("diagnostic/dropped_samples.rs");
include!("diagnostic/summary.rs");
