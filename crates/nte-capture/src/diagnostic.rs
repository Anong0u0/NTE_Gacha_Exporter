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

include!("diagnostic/run.rs");
include!("diagnostic/dedup.rs");
include!("diagnostic/dropped_samples.rs");
include!("diagnostic/summary.rs");
