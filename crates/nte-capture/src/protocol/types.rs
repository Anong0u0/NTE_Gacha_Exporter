use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const DOTNET_EPOCH_TICKS: i64 = 621_355_968_000_000_000;
const TICKS_PER_SECOND: i64 = 10_000_000;
const MONOPOLY_MARKER: &[u8] = b"FMonopolyLotteryRecordData";
const FORK_MARKER: &[u8] = b"FForkLotteryRecordData";
const MAX_ROWS_PER_BLOCK: u32 = 100;
const PROTOCOL_CONSTANT: u32 = 0x03000000;
const MONOPOLY_BLOCK_KIND: u32 = 527;
const FORK_BLOCK_KIND: u32 = 5906;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordType {
    Monopoly,
    Fork,
}

impl RecordType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Monopoly => "monopoly",
            Self::Fork => "fork",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SourceRef {
    pub session: u64,
    pub line: u64,
    pub packet_index: u64,
    pub view: String,
    pub row_index: u32,
    pub offset: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_index: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_high: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segment_index: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_index: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolEnvelope {
    pub record_type: RecordType,
    pub stream_key: String,
    pub page_index: u32,
    pub query_high: bool,
    pub segment_index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ParsedRow {
    pub record_type: RecordType,
    pub ticks: u64,
    pub time: Option<String>,
    pub pool_id: Option<String>,
    pub item_id: String,
    pub count: u32,
    pub roll_points: Option<u32>,
    pub roll_label_id: Option<String>,
    pub secondary_item_id: Option<String>,
    pub secondary_count: Option<u32>,
    pub source: SourceRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedBlock {
    pub record_type: RecordType,
    pub marker_offset: usize,
    pub declared_size: u32,
    pub row_count: u32,
    pub rows: Vec<ParsedRow>,
    pub envelope: Option<ProtocolEnvelope>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParseWarning {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packet_index: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
}

impl ParseWarning {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            session: None,
            line: None,
            packet_index: None,
            view: None,
        }
    }

    fn at(
        code: impl Into<String>,
        message: impl Into<String>,
        session: u64,
        line: u64,
        packet_index: u64,
        view: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            session: Some(session),
            line: Some(line),
            packet_index: Some(packet_index),
            view: Some(view.into()),
        }
    }
}

#[derive(Debug, Error)]
enum ParseError {
    #[error("{0}")]
    Message(String),
}

#[derive(Debug, Clone)]
struct ParseContext {
    session: u64,
    line: u64,
    packet_index: u64,
    view: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RowSignature(
    RecordType,
    u64,
    Option<String>,
    String,
    u32,
    Option<u32>,
    Option<String>,
    Option<String>,
    Option<u32>,
);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct BlockSignature(RecordType, Vec<RowSignature>);

#[derive(Debug, Clone)]
struct Segment {
    index: u32,
    rows: Vec<ParsedRow>,
    signature: BlockSignature,
}

#[derive(Debug, Clone)]
struct Generation {
    index: u32,
    segments: BTreeMap<u32, Segment>,
}

#[derive(Debug, Clone)]
struct StreamState {
    generations: Vec<Generation>,
}

#[derive(Debug, Default)]
pub struct ProtocolAssembler {
    order: Vec<String>,
    streams: BTreeMap<String, StreamState>,
    legacy_rows: Vec<ParsedRow>,
    legacy_blocks: BTreeSet<BlockSignature>,
    rows_cache: Vec<ParsedRow>,
    rows_dirty: bool,
    pub warnings: Vec<ParseWarning>,
    warning_keys: BTreeSet<(String, String, u32)>,
}

pub fn parse_payload_blocks(
    payload: &[u8],
    session: u64,
    line: u64,
    packet_index: u64,
) -> (Vec<ParsedBlock>, Vec<ParseWarning>) {
    let mut blocks = Vec::new();
    let mut warnings = Vec::new();
    parse_payload_view(
        "raw",
        payload,
        session,
        line,
        packet_index,
        &mut blocks,
        &mut warnings,
    );
    for shift in 1..8 {
        if !shifted_view_contains_marker(payload, 8, shift, payload.len().saturating_sub(8)) {
            continue;
        }
        let data = decode_shifted_bytes(payload, 8, shift, payload.len().saturating_sub(8));
        parse_payload_view(
            &format!("shift8:{shift}"),
            &data,
            session,
            line,
            packet_index,
            &mut blocks,
            &mut warnings,
        );
    }
    (blocks, warnings)
}

fn parse_payload_view(
    view_name: &str,
    data: &[u8],
    session: u64,
    line: u64,
    packet_index: u64,
    blocks: &mut Vec<ParsedBlock>,
    warnings: &mut Vec<ParseWarning>,
) {
    for (marker, kind) in [
        (MONOPOLY_MARKER, RecordType::Monopoly),
        (FORK_MARKER, RecordType::Fork),
    ] {
        let mut pos = 0;
        while let Some(found) = find_bytes(&data[pos..], marker) {
            let marker_pos = pos + found;
            let ctx = ParseContext {
                session,
                line,
                packet_index,
                view: view_name.to_string(),
            };
            let parsed = match kind {
                RecordType::Monopoly => parse_monopoly_block(data, marker_pos, &ctx),
                RecordType::Fork => parse_fork_block(data, marker_pos, &ctx),
            };
            match parsed {
                Ok(block) => blocks.push(block),
                Err(error) => warnings.push(ParseWarning::at(
                    "parse_error",
                    format!("{}: {error}", String::from_utf8_lossy(marker)),
                    session,
                    line,
                    packet_index,
                    view_name,
                )),
            }
            pos = marker_pos + marker.len();
        }
    }
}

