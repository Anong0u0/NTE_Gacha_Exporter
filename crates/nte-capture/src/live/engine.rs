use anyhow::Result;

use super::progress::{ProgressPayload, emit_progress};
use super::raw_filter::should_write_raw_packet;
use crate::live::{CaptureCounters, CaptureOptions, CaptureTarget};
use crate::protocol::{ParseWarning, ParsedRow, ProtocolAssembler, parse_payload_blocks};
use crate::raw::{ParsedNetworkPacket, RawWriter, raw_record_from_parsed_packet};

const CAPTURE_DUPLICATE_WINDOW_SECONDS: f64 = 0.250;
const IDLE_PROGRESS_PACKET_INTERVAL: u64 = 250;

#[derive(Debug, Clone, PartialEq, Eq)]
struct RawSignature {
    proto: String,
    sport: Option<u16>,
    dport: Option<u16>,
    payload: Vec<u8>,
}

#[derive(Debug, Clone)]
struct RecentPacket {
    signature: RawSignature,
    captured_at: f64,
}

pub(super) struct CaptureLoopState {
    raw_writer: Option<RawWriter>,
    assembler: ProtocolAssembler,
    warnings: Vec<ParseWarning>,
    counters: CaptureCounters,
    last_packet: Option<RecentPacket>,
    rows_snapshot: Vec<ParsedRow>,
    last_progress_seen: u64,
}

pub(super) struct CaptureLoopFinish {
    pub counters: CaptureCounters,
    pub rows: Vec<ParsedRow>,
    pub warnings: Vec<ParseWarning>,
}

impl CaptureLoopState {
    pub fn new(raw_writer: Option<RawWriter>) -> Self {
        Self {
            raw_writer,
            assembler: ProtocolAssembler::default(),
            warnings: Vec::new(),
            counters: CaptureCounters::default(),
            last_packet: None,
            rows_snapshot: Vec::new(),
            last_progress_seen: 0,
        }
    }

    pub fn counters(&self) -> &CaptureCounters {
        &self.counters
    }

    pub fn counters_mut(&mut self) -> &mut CaptureCounters {
        &mut self.counters
    }

    pub fn emit_initial_progress(&self, options: &CaptureOptions, target: &CaptureTarget) {
        emit_progress(
            options,
            target,
            &self.counters,
            ProgressPayload {
                new_rows: &[],
                rows_snapshot: &[],
                row_count: 0,
                warning_count: 0,
            },
        );
    }

    pub fn record_parse_drop(&mut self) {
        self.counters.dropped_packets += 1;
    }

    pub fn handle_packet(
        &mut self,
        options: &CaptureOptions,
        target: &CaptureTarget,
        packet: ParsedNetworkPacket,
        ports: &[u16],
        strategy: crate::live::CaptureStrategyKind,
        captured_at: f64,
    ) -> Result<()> {
        if self
            .last_packet
            .as_ref()
            .is_some_and(|last| last.matches(&packet, captured_at))
        {
            self.counters.duplicate_packets += 1;
            self.emit_idle_progress(options, target);
            return Ok(());
        }
        self.last_packet = Some(RecentPacket {
            signature: RawSignature::from_packet(&packet),
            captured_at,
        });

        let (blocks, found_warnings) = parse_payload_blocks(
            &packet.payload,
            0,
            self.counters.packets_seen,
            self.counters.packets_seen - 1,
        );
        self.warnings.extend(found_warnings);
        if should_write_raw_packet(&packet, ports, !blocks.is_empty(), strategy) {
            let record =
                raw_record_from_parsed_packet(&packet, self.counters.packets_seen, captured_at);
            if let Some(writer) = self.raw_writer.as_mut() {
                writer.write_packet(&record)?;
            }
        }
        if blocks.is_empty() {
            self.emit_idle_progress(options, target);
            return Ok(());
        }

        self.counters.decoded_packets += 1;
        let update = self.assembler.add_blocks_with_update(blocks);
        if let Some(rows) = update.rows {
            self.rows_snapshot = rows;
        }
        emit_progress(
            options,
            target,
            &self.counters,
            ProgressPayload {
                new_rows: &update.new_rows,
                rows_snapshot: &self.rows_snapshot,
                row_count: self.rows_snapshot.len(),
                warning_count: self.warnings.len(),
            },
        );
        self.last_progress_seen = self.counters.packets_seen;
        Ok(())
    }

    pub fn finish(
        mut self,
        options: &CaptureOptions,
        target: &CaptureTarget,
    ) -> Result<CaptureLoopFinish> {
        if let Some(writer) = self.raw_writer.as_mut() {
            writer.write_stop(
                self.counters.packets_seen,
                self.counters.decoded_packets,
                self.counters.dropped_packets,
                self.counters.duplicate_packets,
            )?;
        }
        let mut rows = self.assembler.rows();
        self.warnings.extend(self.assembler.warnings);
        rows.shrink_to_fit();
        emit_progress(
            options,
            target,
            &self.counters,
            ProgressPayload {
                new_rows: &[],
                rows_snapshot: &rows,
                row_count: rows.len(),
                warning_count: self.warnings.len(),
            },
        );
        Ok(CaptureLoopFinish {
            counters: self.counters,
            rows,
            warnings: self.warnings,
        })
    }

    fn emit_idle_progress(&mut self, options: &CaptureOptions, target: &CaptureTarget) {
        if self
            .counters
            .packets_seen
            .saturating_sub(self.last_progress_seen)
            < IDLE_PROGRESS_PACKET_INTERVAL
        {
            return;
        }
        emit_progress(
            options,
            target,
            &self.counters,
            ProgressPayload {
                new_rows: &[],
                rows_snapshot: &self.rows_snapshot,
                row_count: self.rows_snapshot.len(),
                warning_count: self.warnings.len(),
            },
        );
        self.last_progress_seen = self.counters.packets_seen;
    }
}

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

impl RecentPacket {
    fn matches(&self, packet: &ParsedNetworkPacket, captured_at: f64) -> bool {
        self.signature.proto.as_str() == packet.proto.as_str()
            && self.signature.sport == packet.sport
            && self.signature.dport == packet.dport
            && self.signature.payload.as_slice() == packet.payload.as_slice()
            && captured_at - self.captured_at <= CAPTURE_DUPLICATE_WINDOW_SECONDS
    }
}

pub(super) fn now_seconds() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs_f64())
        .unwrap_or_default()
}

pub(super) fn counters_delta(start: &CaptureCounters, end: &CaptureCounters) -> CaptureCounters {
    CaptureCounters {
        packets_seen: end.packets_seen.saturating_sub(start.packets_seen),
        decoded_packets: end.decoded_packets.saturating_sub(start.decoded_packets),
        dropped_packets: end.dropped_packets.saturating_sub(start.dropped_packets),
        duplicate_packets: end
            .duplicate_packets
            .saturating_sub(start.duplicate_packets),
        filter_restarts: end.filter_restarts.saturating_sub(start.filter_restarts),
    }
}
