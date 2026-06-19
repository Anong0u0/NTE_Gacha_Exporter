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

fn shifted_view_contains_marker(
    data: &[u8],
    byte_off: usize,
    bit_shift: usize,
    count: usize,
) -> bool {
    shifted_view_contains_bytes(data, byte_off, bit_shift, count, MONOPOLY_MARKER)
        || shifted_view_contains_bytes(data, byte_off, bit_shift, count, FORK_MARKER)
}

fn shifted_view_contains_bytes(
    data: &[u8],
    byte_off: usize,
    bit_shift: usize,
    count: usize,
    needle: &[u8],
) -> bool {
    if needle.is_empty() || count < needle.len() {
        return false;
    }
    for start in 0..=count - needle.len() {
        if needle.iter().enumerate().all(|(index, byte)| {
            shifted_byte(data, byte_off, bit_shift, start + index) == Some(*byte)
        }) {
            return true;
        }
    }
    false
}

fn shifted_byte(data: &[u8], byte_off: usize, bit_shift: usize, index: usize) -> Option<u8> {
    let bit_pos = (byte_off + index) * 8 + bit_shift;
    let b_off = bit_pos / 8;
    let b_shift = bit_pos % 8;
    let first = data.get(b_off)?;
    let mut value = first >> b_shift;
    if b_shift != 0 {
        if let Some(next) = data.get(b_off + 1) {
            value |= next << (8 - b_shift);
        }
    }
    Some(value)
}

