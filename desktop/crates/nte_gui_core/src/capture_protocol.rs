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
    for (view_name, data) in packet_views(payload) {
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
                    view: view_name.clone(),
                };
                let parsed = match kind {
                    RecordType::Monopoly => parse_monopoly_block(&data, marker_pos, &ctx),
                    RecordType::Fork => parse_fork_block(&data, marker_pos, &ctx),
                };
                match parsed {
                    Ok(block) => blocks.push(block),
                    Err(error) => warnings.push(ParseWarning::at(
                        "parse_error",
                        format!("{}: {error}", String::from_utf8_lossy(marker)),
                        session,
                        line,
                        packet_index,
                        view_name.clone(),
                    )),
                }
                pos = marker_pos + marker.len();
            }
        }
    }
    (blocks, warnings)
}

impl ProtocolAssembler {
    pub fn add_blocks(&mut self, blocks: impl IntoIterator<Item = ParsedBlock>) -> Vec<ParsedRow> {
        let before = self.rows();
        for block in blocks {
            self.add_block(block);
        }
        let after = self.rows();
        new_prefix_rows(&before, &after)
    }

    pub fn add_block(&mut self, block: ParsedBlock) {
        let Some(envelope) = block.envelope.clone() else {
            self.add_legacy_block(block);
            return;
        };

        if !self.streams.contains_key(&envelope.stream_key) {
            self.streams.insert(
                envelope.stream_key.clone(),
                StreamState {
                    generations: Vec::new(),
                },
            );
            self.order.push(envelope.stream_key.clone());
        }

        let stream = self
            .streams
            .get_mut(&envelope.stream_key)
            .expect("stream exists");
        if stream.generations.is_empty() {
            stream.start_generation();
        }
        let signature = block_signature(block.record_type, &block.rows);
        let segment = Segment {
            index: envelope.segment_index,
            rows: block.rows,
            signature,
        };
        let current = stream.generations.last_mut().expect("generation exists");
        if let Some(existing) = current.segments.get(&segment.index) {
            if existing.signature == segment.signature {
                return;
            }
            stream.start_generation();
        }
        stream
            .generations
            .last_mut()
            .expect("generation exists")
            .segments
            .insert(segment.index, segment);
    }

    pub fn rows(&mut self) -> Vec<ParsedRow> {
        let mut rows = Vec::new();
        for key in self.order.clone() {
            if key == "__legacy__" {
                rows.extend(self.legacy_rows.clone());
                continue;
            }
            if let Some(stream) = self.streams.get(&key).cloned() {
                rows.extend(self.assemble_stream(&key, &stream));
            }
        }
        rows
    }

    fn add_legacy_block(&mut self, block: ParsedBlock) {
        let signature = block_signature(block.record_type, &block.rows);
        if self.legacy_blocks.contains(&signature) {
            return;
        }
        if self.legacy_rows.is_empty() {
            self.order.push("__legacy__".to_string());
        }
        self.legacy_blocks.insert(signature);
        self.legacy_rows.extend(block.rows);
    }

    fn assemble_stream(&mut self, key: &str, stream: &StreamState) -> Vec<ParsedRow> {
        let mut result_rows = Vec::new();
        let mut result_max_segment: Option<u32> = None;

        for generation in &stream.generations {
            if generation.segments.is_empty() {
                continue;
            }
            let generation_rows = rows_with_generation(generation);
            let generation_min = *generation.segments.keys().next().expect("nonempty");
            let generation_max = *generation.segments.keys().next_back().expect("nonempty");

            if result_rows.is_empty() {
                result_rows = generation_rows;
                result_max_segment = Some(generation_max);
                continue;
            }

            if generation_min == 0 {
                if result_max_segment.is_none_or(|max| generation_max >= max) {
                    result_rows = generation_rows;
                    result_max_segment = Some(generation_max);
                    continue;
                }
                if let Some(merged) = partial_snapshot_merge(&generation_rows, &result_rows) {
                    result_rows = merged;
                } else {
                    self.warn_generation(
                        "ambiguous_snapshot_merge",
                        format!("{key}: partial snapshot cannot be merged safely"),
                        generation,
                    );
                }
                continue;
            }

            if result_max_segment.is_some_and(|max| generation_min > max) {
                result_rows.extend(generation_rows);
                result_max_segment = Some(generation_max);
                continue;
            }

            self.warn_generation(
                "ambiguous_snapshot_merge",
                format!("{key}: non-zero snapshot reset cannot be merged safely"),
                generation,
            );
        }

        result_rows
    }

    fn warn_generation(&mut self, code: &str, message: String, generation: &Generation) {
        let Some(row) = generation
            .segments
            .values()
            .next()
            .and_then(|segment| segment.rows.first())
        else {
            return;
        };
        let key = (
            code.to_string(),
            row.source.stream_key.clone().unwrap_or_default(),
            generation.index,
        );
        if self.warning_keys.contains(&key) {
            return;
        }
        self.warning_keys.insert(key);
        self.warnings.push(ParseWarning {
            code: code.to_string(),
            message,
            session: Some(row.source.session),
            line: Some(row.source.line),
            packet_index: Some(row.source.packet_index),
            view: Some(row.source.view.clone()),
        });
    }
}

impl StreamState {
    fn start_generation(&mut self) {
        self.generations.push(Generation {
            index: self.generations.len() as u32,
            segments: BTreeMap::new(),
        });
    }
}

fn parse_monopoly_block(
    data: &[u8],
    marker_pos: usize,
    ctx: &ParseContext,
) -> Result<ParsedBlock, ParseError> {
    let envelope = parse_protocol_envelope(RecordType::Monopoly, data, marker_pos, &ctx.view)?;
    let mut pos = marker_pos + MONOPOLY_MARKER.len();
    if data.get(pos) == Some(&0) {
        pos += 1;
    }
    let declared_size = u32_at(data, pos + 4)?;
    let row_count = u32_at(data, pos + 8)?;
    pos += 12;
    if row_count > MAX_ROWS_PER_BLOCK {
        return Err(message(format!("row_count too large: {row_count}")));
    }

    let mut reader = Reader { data, pos };
    let mut rows = Vec::new();
    for index in 0..row_count {
        let row_start = reader.pos;
        let raw_roll_points = reader.u32()?;
        let item_spec = reader.string()?;
        let _zero = reader.u32()?;
        let secondary_count = reader.u32()?;
        let secondary_item = reader.string()?;
        let result_or_pool = reader.string()?;

        let pool_start = reader.pos;
        let pool_id = match reader.try_string() {
            Some(pool) if pool.starts_with("CardPool_") => Some(pool),
            _ => {
                reader.pos = pool_start;
                result_or_pool
                    .starts_with("CardPool_")
                    .then_some(result_or_pool.clone())
            }
        };

        let ticks = reader.u64()?;
        let (item_id, count) = parse_item_spec(&item_spec);
        rows.push(ParsedRow {
            record_type: RecordType::Monopoly,
            ticks,
            time: dotnet_ticks_to_iso(ticks),
            pool_id,
            item_id,
            count,
            roll_points: roll_points_value(raw_roll_points),
            roll_label_id: roll_label_id(raw_roll_points).map(ToOwned::to_owned),
            secondary_item_id: (!secondary_item.is_empty()).then_some(secondary_item),
            secondary_count: Some(secondary_count),
            source: source_with_envelope(ctx, envelope.as_ref(), index, row_start),
        });
    }

    Ok(ParsedBlock {
        record_type: RecordType::Monopoly,
        marker_offset: marker_pos,
        declared_size,
        row_count,
        rows,
        envelope,
    })
}

fn parse_fork_block(
    data: &[u8],
    marker_pos: usize,
    ctx: &ParseContext,
) -> Result<ParsedBlock, ParseError> {
    let envelope = parse_protocol_envelope(RecordType::Fork, data, marker_pos, &ctx.view)?;
    let mut pos = marker_pos + FORK_MARKER.len();
    if data.get(pos) == Some(&0) {
        pos += 1;
    }
    let declared_size = u32_at(data, pos + 4)?;
    let row_count = u32_at(data, pos + 8)?;
    pos += 12;
    if row_count > MAX_ROWS_PER_BLOCK {
        return Err(message(format!("row_count too large: {row_count}")));
    }

    let mut reader = Reader { data, pos };
    let mut rows = Vec::new();
    for index in 0..row_count {
        let row_start = reader.pos;
        let item_spec = reader.string()?;
        let pool_id = reader.string()?;
        let ticks = reader.u64()?;
        let (item_id, count) = parse_item_spec(&item_spec);
        rows.push(ParsedRow {
            record_type: RecordType::Fork,
            ticks,
            time: dotnet_ticks_to_iso(ticks),
            pool_id: Some(pool_id),
            item_id,
            count,
            roll_points: None,
            roll_label_id: None,
            secondary_item_id: None,
            secondary_count: None,
            source: source_with_envelope(ctx, envelope.as_ref(), index, row_start),
        });
    }

    Ok(ParsedBlock {
        record_type: RecordType::Fork,
        marker_offset: marker_pos,
        declared_size,
        row_count,
        rows,
        envelope,
    })
}

fn parse_protocol_envelope(
    record_type: RecordType,
    data: &[u8],
    marker_pos: usize,
    view: &str,
) -> Result<Option<ProtocolEnvelope>, ParseError> {
    if marker_pos == 0 {
        return Ok(None);
    }
    match record_type {
        RecordType::Monopoly => {
            if !view.starts_with("shift8:") {
                return Err(message("invalid monopoly protocol view"));
            }
            if marker_pos < 26 {
                return Err(message("invalid monopoly protocol envelope"));
            }
            let protocol_constant = relative_u32(data, marker_pos, -26)?;
            let query_raw = relative_u32(data, marker_pos, -22)?;
            let page_raw = relative_u32(data, marker_pos, -18)?;
            let block_kind = relative_u32(data, marker_pos, -14)?;
            let pool_token = relative_u32(data, marker_pos, -10)?;
            let footer = relative_u32(data, marker_pos, -6)?;
            if protocol_constant != PROTOCOL_CONSTANT
                || block_kind != MONOPOLY_BLOCK_KIND
                || footer != 1_774_080
            {
                return Err(message("invalid monopoly protocol constants"));
            }
            let page_index = page_raw & 0x7fffffff;
            let query_high = (query_raw & 0x80000000) != 0;
            Ok(Some(ProtocolEnvelope {
                record_type,
                stream_key: format!("monopoly:{pool_token}"),
                page_index,
                query_high,
                segment_index: segment_index(page_index, query_high)?,
            }))
        }
        RecordType::Fork => {
            if !view.starts_with("shift8:") {
                return Err(message("invalid fork protocol view"));
            }
            if marker_pos < 17 {
                return Err(message("invalid fork protocol envelope"));
            }
            let protocol_constant = relative_u32(data, marker_pos, -17)?;
            let query_raw = relative_u32(data, marker_pos, -13)?;
            let page_raw = relative_u32(data, marker_pos, -9)?;
            let block_kind = relative_u32(data, marker_pos, -5)?;
            if protocol_constant != PROTOCOL_CONSTANT || block_kind != FORK_BLOCK_KIND {
                return Err(message("invalid fork protocol constants"));
            }
            let page_index = page_raw & 0x7fffffff;
            let query_high = (query_raw & 0x80000000) != 0;
            Ok(Some(ProtocolEnvelope {
                record_type,
                stream_key: "fork".to_string(),
                page_index,
                query_high,
                segment_index: segment_index(page_index, query_high)?,
            }))
        }
    }
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn u32(&mut self) -> Result<u32, ParseError> {
        let value = u32_at(self.data, self.pos)?;
        self.pos += 4;
        Ok(value)
    }

    fn u64(&mut self) -> Result<u64, ParseError> {
        let value = u64_at(self.data, self.pos)?;
        self.pos += 8;
        Ok(value)
    }

    fn string(&mut self) -> Result<String, ParseError> {
        let len_pos = self.pos;
        let length = self.u32()? as usize;
        if length == 0 || length > 256 {
            return Err(message(format!(
                "invalid string length {length} at {len_pos}"
            )));
        }
        let end = self
            .pos
            .checked_add(length)
            .filter(|end| *end <= self.data.len())
            .ok_or_else(|| message("string out of payload range"))?;
        let mut raw = &self.data[self.pos..end];
        self.pos = end;
        if raw.ends_with(&[0]) {
            raw = &raw[..raw.len() - 1];
        }
        Ok(String::from_utf8_lossy(raw).to_string())
    }

    fn try_string(&mut self) -> Option<String> {
        let start = self.pos;
        match self.string() {
            Ok(value) => Some(value),
            Err(_) => {
                self.pos = start;
                None
            }
        }
    }
}

fn packet_views(data: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut views = vec![("raw".to_string(), data.to_vec())];
    for shift in 1..8 {
        views.push((
            format!("shift8:{shift}"),
            decode_shifted_bytes(data, 8, shift, data.len().saturating_sub(8)),
        ));
    }
    views
}

fn decode_shifted_bytes(data: &[u8], byte_off: usize, bit_shift: usize, count: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let bit_pos = (byte_off + i) * 8 + bit_shift;
        let b_off = bit_pos / 8;
        let b_shift = bit_pos % 8;
        let Some(first) = data.get(b_off) else {
            break;
        };
        let mut value = first >> b_shift;
        if b_shift != 0 {
            if let Some(next) = data.get(b_off + 1) {
                value |= next << (8 - b_shift);
            }
        }
        out.push(value);
    }
    out
}

fn source_with_envelope(
    ctx: &ParseContext,
    envelope: Option<&ProtocolEnvelope>,
    row_index: u32,
    offset: usize,
) -> SourceRef {
    SourceRef {
        session: ctx.session,
        line: ctx.line,
        packet_index: ctx.packet_index,
        view: ctx.view.clone(),
        row_index,
        offset,
        stream_key: envelope.map(|value| value.stream_key.clone()),
        page_index: envelope.map(|value| value.page_index),
        query_high: envelope.map(|value| value.query_high),
        segment_index: envelope.map(|value| value.segment_index),
        generation_index: None,
    }
}

fn row_signature(row: &ParsedRow) -> RowSignature {
    RowSignature(
        row.record_type,
        row.ticks,
        row.pool_id.clone(),
        row.item_id.clone(),
        row.count,
        row.roll_points,
        row.roll_label_id.clone(),
        row.secondary_item_id.clone(),
        row.secondary_count,
    )
}

fn block_signature(record_type: RecordType, rows: &[ParsedRow]) -> BlockSignature {
    BlockSignature(record_type, rows.iter().map(row_signature).collect())
}

fn row_signatures(rows: &[ParsedRow]) -> Vec<RowSignature> {
    rows.iter().map(row_signature).collect()
}

fn rows_with_generation(generation: &Generation) -> Vec<ParsedRow> {
    let mut rows = Vec::new();
    for segment in generation.segments.values() {
        for row in &segment.rows {
            let mut row = row.clone();
            row.source.generation_index = Some(generation.index);
            rows.push(row);
        }
    }
    rows
}

fn partial_snapshot_merge(
    new_rows: &[ParsedRow],
    old_rows: &[ParsedRow],
) -> Option<Vec<ParsedRow>> {
    if new_rows.is_empty() {
        return Some(old_rows.to_vec());
    }
    if old_rows.is_empty() {
        return Some(new_rows.to_vec());
    }
    let new_signatures = row_signatures(new_rows);
    let old_signatures = row_signatures(old_rows);
    let max_overlap = new_signatures.len().min(old_signatures.len());
    let mut matches = Vec::new();

    for overlap in (1..=max_overlap).rev() {
        let suffix = &new_signatures[new_signatures.len() - overlap..];
        for position in 0..=old_signatures.len() - overlap {
            if &old_signatures[position..position + overlap] == suffix {
                matches.push((overlap, position));
            }
        }
        if !matches.is_empty() {
            break;
        }
    }

    if matches.len() != 1 {
        return None;
    }
    let (overlap, position) = matches[0];
    let mut rows = new_rows.to_vec();
    rows.extend_from_slice(&old_rows[position + overlap..]);
    Some(rows)
}

fn new_prefix_rows(before: &[ParsedRow], after: &[ParsedRow]) -> Vec<ParsedRow> {
    if after.is_empty() {
        return Vec::new();
    }
    if before.is_empty() {
        return after.to_vec();
    }
    let before_signatures = row_signatures(before);
    let after_signatures = row_signatures(after);
    if before_signatures == after_signatures {
        return Vec::new();
    }

    let mut matches = Vec::new();
    for position in 0..after_signatures.len() {
        let overlap = before_signatures
            .len()
            .min(after_signatures.len() - position);
        if overlap == 0 {
            continue;
        }
        if after_signatures[position..position + overlap] == before_signatures[..overlap] {
            matches.push((overlap, position));
        }
    }
    if matches.is_empty() {
        return Vec::new();
    }
    let best_overlap = matches
        .iter()
        .map(|(overlap, _)| *overlap)
        .max()
        .unwrap_or(0);
    let best_positions = matches
        .into_iter()
        .filter_map(|(overlap, position)| (overlap == best_overlap).then_some(position))
        .collect::<Vec<_>>();
    if best_positions.len() != 1 {
        return Vec::new();
    }
    after[..best_positions[0]].to_vec()
}

fn parse_item_spec(value: &str) -> (String, u32) {
    let Some((item_id, amount)) = value.rsplit_once(',') else {
        return (value.to_string(), 1);
    };
    match amount.parse::<u32>() {
        Ok(value) if value > 0 => (item_id.to_string(), value),
        _ => (value.to_string(), 1),
    }
}

fn dotnet_ticks_to_iso(ticks: u64) -> Option<String> {
    let ticks = i64::try_from(ticks).ok()?;
    let seconds = (ticks - DOTNET_EPOCH_TICKS) / TICKS_PER_SECOND;
    if !(1_500_000_000..=4_102_444_800).contains(&seconds) {
        return None;
    }
    let micros_since_unix = (ticks - DOTNET_EPOCH_TICKS) / 10;
    let secs = micros_since_unix.div_euclid(1_000_000);
    let micros = micros_since_unix.rem_euclid(1_000_000) as u32;
    let dt: DateTime<Utc> = DateTime::from_timestamp(secs, micros * 1_000)?;
    Some(dt.naive_utc().format("%Y-%m-%dT%H:%M:%S%.6f").to_string())
}

fn public_time(value: Option<&str>) -> Option<String> {
    Some(value?.replace('T', " ").chars().take(19).collect())
}

pub fn row_public_time(row: &ParsedRow) -> Option<String> {
    public_time(row.time.as_deref())
}

fn roll_label_id(value: u32) -> Option<&'static str> {
    match value {
        0 => Some("BPUI_LotteryResult_jidianzengli"),
        0xffffffff => Some("BPUI_LotteryResult_chenmiandi"),
        _ => None,
    }
}

fn roll_points_value(value: u32) -> Option<u32> {
    roll_label_id(value).is_none().then_some(value)
}

fn segment_index(page_index: u32, query_high: bool) -> Result<u32, ParseError> {
    if query_high {
        Ok(page_index.saturating_mul(2))
    } else if page_index > 0 {
        Ok(page_index * 2 - 1)
    } else {
        Err(message("invalid protocol segment index"))
    }
}

fn relative_u32(data: &[u8], marker_pos: usize, relative_pos: isize) -> Result<u32, ParseError> {
    let pos = marker_pos
        .checked_add_signed(relative_pos)
        .ok_or_else(|| message("protocol envelope out of range"))?;
    u32_at(data, pos)
}

fn u32_at(data: &[u8], pos: usize) -> Result<u32, ParseError> {
    let bytes = data
        .get(pos..pos + 4)
        .ok_or_else(|| message("u32 out of range"))?;
    Ok(u32::from_le_bytes(bytes.try_into().expect("4 bytes")))
}

fn u64_at(data: &[u8], pos: usize) -> Result<u64, ParseError> {
    let bytes = data
        .get(pos..pos + 8)
        .ok_or_else(|| message("u64 out of range"))?;
    Ok(u64::from_le_bytes(bytes.try_into().expect("8 bytes")))
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|candidate| candidate == needle)
}

fn message(value: impl Into<String>) -> ParseError {
    ParseError::Message(value.into())
}

#[cfg(test)]
mod tests {
    use base64::Engine;

    use super::*;

    fn fstring(value: &str) -> Vec<u8> {
        let mut raw = value.as_bytes().to_vec();
        raw.push(0);
        let mut out = (raw.len() as u32).to_le_bytes().to_vec();
        out.extend(raw);
        out
    }

    #[test]
    fn decodes_monopoly_record() {
        let mut row = Vec::new();
        row.extend(2_u32.to_le_bytes());
        row.extend(fstring("Fashion_vehicle_1010_V008"));
        row.extend(0_u32.to_le_bytes());
        row.extend(1_u32.to_le_bytes());
        row.extend(fstring("Fashion_vehicle_1010_V008"));
        row.extend(fstring("Fashion_vehicle_1010_V008"));
        row.extend(fstring("CardPool_Character"));
        row.extend(639_131_653_353_040_000_u64.to_le_bytes());
        let mut payload = MONOPOLY_MARKER.to_vec();
        payload.push(0);
        payload.extend(0_u32.to_le_bytes());
        payload.extend((row.len() as u32).to_le_bytes());
        payload.extend(1_u32.to_le_bytes());
        payload.extend(row);

        let (blocks, warnings) = parse_payload_blocks(&payload, 0, 1, 0);

        assert!(warnings.is_empty());
        assert_eq!(blocks[0].rows[0].record_type, RecordType::Monopoly);
        assert_eq!(
            blocks[0].rows[0].pool_id.as_deref(),
            Some("CardPool_Character")
        );
        assert_eq!(blocks[0].rows[0].roll_points, Some(2));
    }

    #[test]
    fn decodes_sample_fixture_payload() {
        let payload = base64::engine::general_purpose::STANDARD
            .decode("RkZvcmtMb3R0ZXJ5UmVjb3JkRGF0YQAAAAAAMQAAAAEAAAANAAAAZm9ya19kdXN0YmluABQAAABGb3JrTG90dGVyeV9Bbkh1blF1AIAFZ8eTwd4I")
            .unwrap();
        let (blocks, warnings) = parse_payload_blocks(&payload, 0, 2, 1);
        assert!(warnings.is_empty());
        assert_eq!(blocks[0].rows[0].record_type, RecordType::Fork);
        assert_eq!(blocks[0].rows[0].item_id, "fork_dustbin");
        assert_eq!(
            row_public_time(&blocks[0].rows[0]).as_deref(),
            Some("2026-06-03 17:15:58")
        );
    }
}
