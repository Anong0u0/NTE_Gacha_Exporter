pub fn read_raw_capture(path: &Path) -> Result<RawReadResult> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut assembler = ProtocolAssembler::default();
    let mut warnings = Vec::new();
    let mut session_index: i64 = -1;
    let mut packet_index = 0_u64;
    let mut in_session = false;
    let mut saw_session = false;

    for (line_index, line) in BufReader::new(file).lines().enumerate() {
        let line_no = line_index as u64 + 1;
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let value: serde_json::Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(error) => {
                warnings.push(ParseWarning::new(
                    "bad_jsonl",
                    format!("line {line_no}: {error}"),
                ));
                continue;
            }
        };
        let Some(object) = value.as_object() else {
            warnings.push(ParseWarning::new(
                "bad_jsonl",
                format!("line {line_no}: record is not an object"),
            ));
            continue;
        };
        match object.get("type").and_then(serde_json::Value::as_str) {
            Some("capture_start") => {
                saw_session = true;
                in_session = true;
                session_index += 1;
                packet_index = 0;
            }
            Some("capture_stop") => in_session = false,
            Some("packet") if in_session => {
                let payload = object
                    .get("payload_b64")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|text| base64::engine::general_purpose::STANDARD.decode(text).ok());
                let Some(payload) = payload else {
                    warnings.push(ParseWarning::new("bad_packet", "invalid payload_b64"));
                    packet_index += 1;
                    continue;
                };
                let (blocks, found_warnings) =
                    parse_payload_blocks(&payload, session_index as u64, line_no, packet_index);
                packet_index += 1;
                warnings.extend(found_warnings);
                assembler.add_blocks(blocks);
            }
            _ => {}
        }
    }

    if !saw_session {
        anyhow::bail!("raw capture has no capture_start records");
    }
    let mut rows = assembler.rows();
    warnings.extend(assembler.warnings);
    rows.shrink_to_fit();
    Ok(RawReadResult { rows, warnings })
}
