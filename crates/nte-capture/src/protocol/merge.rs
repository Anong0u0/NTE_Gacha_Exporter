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

    if after_signatures.len() >= before_signatures.len() {
        let full_matches = (0..=after_signatures.len() - before_signatures.len())
            .filter(|position| {
                after_signatures[*position..*position + before_signatures.len()]
                    == before_signatures
            })
            .collect::<Vec<_>>();
        if full_matches.len() == 1 {
            let position = full_matches[0];
            let mut rows = after[..position].to_vec();
            rows.extend_from_slice(&after[position + before_signatures.len()..]);
            return rows;
        }
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

