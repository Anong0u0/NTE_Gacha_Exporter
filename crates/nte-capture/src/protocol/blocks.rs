fn parse_monopoly_block(
    data: &[u8],
    marker_pos: usize,
    ctx: &ParseContext,
) -> Result<ParsedBlock, ParseError> {
    let envelope = parse_protocol_envelope(RecordType::Monopoly, data, marker_pos, &ctx.view)?;
    let frame = parse_block_frame(data, marker_pos, MONOPOLY_MARKER)?;
    let rows = parse_monopoly_rows(
        data,
        frame.rows_start,
        frame.block_end,
        frame.row_count,
        ctx,
        envelope.as_ref(),
    )?;

    Ok(ParsedBlock {
        record_type: RecordType::Monopoly,
        marker_offset: marker_pos,
        declared_size: frame.declared_size,
        row_count: frame.row_count,
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
    let frame = parse_block_frame(data, marker_pos, FORK_MARKER)?;
    let rows = parse_fork_rows(
        data,
        frame.rows_start,
        frame.block_end,
        frame.row_count,
        ctx,
        envelope.as_ref(),
    )?;

    Ok(ParsedBlock {
        record_type: RecordType::Fork,
        marker_offset: marker_pos,
        declared_size: frame.declared_size,
        row_count: frame.row_count,
        rows,
        envelope,
    })
}

struct BlockFrame {
    declared_size: u32,
    row_count: u32,
    rows_start: usize,
    block_end: usize,
}

fn parse_block_frame(
    data: &[u8],
    marker_pos: usize,
    marker: &[u8],
) -> Result<BlockFrame, ParseError> {
    let mut pos = marker_pos + marker.len();
    if data.get(pos) == Some(&0) {
        pos += 1;
    }
    let declared_size = u32_at(data, pos + 4)?;
    let row_count = u32_at(data, pos + 8)?;
    if row_count > MAX_ROWS_PER_BLOCK {
        return Err(message(format!("row_count too large: {row_count}")));
    }
    let rows_start = pos + 12;
    let block_end = usize::try_from(declared_size)
        .ok()
        .and_then(|size| size.checked_sub(4))
        .and_then(|size| rows_start.checked_add(size))
        .filter(|end| *end >= rows_start && *end <= data.len())
        .ok_or_else(|| message("declared block size out of payload range"))?;
    Ok(BlockFrame {
        declared_size,
        row_count,
        rows_start,
        block_end,
    })
}

#[derive(Clone)]
struct MonopolyRowCore {
    row_start: usize,
    raw_roll_points: u32,
    item_spec: String,
    secondary_count: u32,
    secondary_item: String,
    pool_id: Option<String>,
    ticks: u64,
}

fn parse_monopoly_rows(
    data: &[u8],
    rows_start: usize,
    block_end: usize,
    row_count: u32,
    ctx: &ParseContext,
    envelope: Option<&ProtocolEnvelope>,
) -> Result<Vec<ParsedRow>, ParseError> {
    let mut states = BTreeMap::from([(rows_start, vec![Vec::new()])]);
    for index in 0..row_count {
        let mut next: BTreeMap<usize, Vec<Vec<ParsedRow>>> = BTreeMap::new();
        for (position, paths) in states {
            let Ok((core, core_end)) = parse_monopoly_row_core(data, position, block_end) else {
                continue;
            };
            let mut tails = vec![(core_end, None)];
            let mut tail_reader = Reader {
                data,
                pos: core_end,
                end: block_end,
            };
            if let Ok(lottery_id) = tail_reader.string()
                && is_valid_lottery_id(&lottery_id)
            {
                tails.push((tail_reader.pos, Some(lottery_id)));
            }
            for (next_pos, lottery_id) in tails {
                for path in &paths {
                    let mut rows = path.clone();
                    rows.push(monopoly_row(
                        &core,
                        lottery_id.clone(),
                        index,
                        ctx,
                        envelope,
                    ));
                    let candidates = next.entry(next_pos).or_default();
                    if candidates.len() < 2 {
                        candidates.push(rows);
                    }
                }
            }
        }
        states = next;
        if states.is_empty() {
            break;
        }
    }
    let mut solutions = states.remove(&block_end).unwrap_or_default();
    if solutions.len() != 1 {
        return Err(message(format!(
            "monopoly block has {} valid layouts",
            solutions.len()
        )));
    }
    Ok(solutions.pop().expect("one solution"))
}

fn parse_monopoly_row_core(
    data: &[u8],
    pos: usize,
    block_end: usize,
) -> Result<(MonopolyRowCore, usize), ParseError> {
    let mut reader = Reader {
        data,
        pos,
        end: block_end,
    };
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
                .then_some(result_or_pool)
        }
    };
    let ticks = reader.u64()?;
    Ok((
        MonopolyRowCore {
            row_start,
            raw_roll_points,
            item_spec,
            secondary_count,
            secondary_item,
            pool_id,
            ticks,
        },
        reader.pos,
    ))
}

fn monopoly_row(
    core: &MonopolyRowCore,
    lottery_id: Option<String>,
    index: u32,
    ctx: &ParseContext,
    envelope: Option<&ProtocolEnvelope>,
) -> ParsedRow {
    let (item_id, count) = parse_item_spec(&core.item_spec);
    ParsedRow {
        record_type: RecordType::Monopoly,
        ticks: core.ticks,
        time: dotnet_ticks_to_iso(core.ticks),
        pool_id: core.pool_id.clone(),
        lottery_id,
        item_id,
        count,
        roll_points: roll_points_value(core.raw_roll_points),
        roll_label_id: roll_label_id(core.raw_roll_points).map(ToOwned::to_owned),
        secondary_item_id: (!core.secondary_item.is_empty()).then(|| core.secondary_item.clone()),
        secondary_count: Some(core.secondary_count),
        source: source_with_envelope(ctx, envelope, index, core.row_start),
    }
}

fn parse_fork_rows(
    data: &[u8],
    rows_start: usize,
    block_end: usize,
    row_count: u32,
    ctx: &ParseContext,
    envelope: Option<&ProtocolEnvelope>,
) -> Result<Vec<ParsedRow>, ParseError> {
    let mut reader = Reader {
        data,
        pos: rows_start,
        end: block_end,
    };
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
            lottery_id: None,
            item_id,
            count,
            roll_points: None,
            roll_label_id: None,
            secondary_item_id: None,
            secondary_count: None,
            source: source_with_envelope(ctx, envelope, index, row_start),
        });
    }
    if reader.pos != block_end {
        return Err(message("fork rows do not consume declared block size"));
    }
    Ok(rows)
}
