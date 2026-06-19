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

