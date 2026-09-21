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

    fn monopoly_row_bytes(raw_roll_points: u32, item_id: &str) -> Vec<u8> {
        let mut row = Vec::new();
        row.extend(raw_roll_points.to_le_bytes());
        row.extend(fstring(item_id));
        row.extend(0_u32.to_le_bytes());
        row.extend(1_u32.to_le_bytes());
        row.extend(fstring(item_id));
        row.extend(fstring(item_id));
        row.extend(fstring("CardPool_Character"));
        row.extend(639_131_653_353_040_000_u64.to_le_bytes());
        row
    }

    fn block_payload(marker: &[u8], row_count: u32, body: &[u8], declared_size: u32) -> Vec<u8> {
        let mut payload = marker.to_vec();
        payload.push(0);
        payload.extend(0_u32.to_le_bytes());
        payload.extend(declared_size.to_le_bytes());
        payload.extend(row_count.to_le_bytes());
        payload.extend(body);
        payload
    }

    fn row(item_id: &str, row_index: u32) -> ParsedRow {
        ParsedRow {
            record_type: RecordType::Monopoly,
            ticks: 639_131_653_353_040_000,
            time: None,
            pool_id: Some("CardPool_Character".to_string()),
            lottery_id: None,
            item_id: item_id.to_string(),
            count: 1,
            roll_points: Some(row_index),
            roll_label_id: None,
            secondary_item_id: None,
            secondary_count: None,
            source: SourceRef {
                session: 0,
                line: 1,
                packet_index: 1,
                view: "test".to_string(),
                row_index,
                offset: row_index as usize,
                stream_key: Some("monopoly:256".to_string()),
                page_index: Some(row_index),
                query_high: Some(false),
                segment_index: Some(row_index),
                generation_index: Some(0),
            },
        }
    }

    fn legacy_block(rows: Vec<ParsedRow>) -> ParsedBlock {
        ParsedBlock {
            record_type: RecordType::Monopoly,
            marker_offset: 0,
            declared_size: 0,
            row_count: rows.len() as u32,
            rows,
            envelope: None,
        }
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
        payload.extend(((row.len() + 4) as u32).to_le_bytes());
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
        assert_eq!(blocks[0].rows[0].lottery_id, None);
    }

    #[test]
    fn decodes_monopoly_record_with_lottery_id() {
        let mut row = Vec::new();
        row.extend(2_u32.to_le_bytes());
        row.extend(fstring("1036"));
        row.extend(0_u32.to_le_bytes());
        row.extend(1_u32.to_le_bytes());
        row.extend(fstring("1036"));
        row.extend(fstring("1036"));
        row.extend(fstring("CardPool_Character"));
        row.extend(639_131_653_353_040_000_u64.to_le_bytes());
        row.extend(fstring("Lottery_Canhong"));
        let mut payload = MONOPOLY_MARKER.to_vec();
        payload.push(0);
        payload.extend(0_u32.to_le_bytes());
        payload.extend(((row.len() + 4) as u32).to_le_bytes());
        payload.extend(1_u32.to_le_bytes());
        payload.extend(row);

        let (blocks, warnings) = parse_payload_blocks(&payload, 0, 1, 0);

        assert!(warnings.is_empty());
        assert_eq!(
            blocks[0].rows[0].lottery_id.as_deref(),
            Some("Lottery_Canhong")
        );
    }

    #[test]
    fn decodes_byte_aligned_monopoly_envelope() {
        let body = monopoly_row_bytes(2, "1036");
        let mut payload = Vec::new();
        payload.extend(0x03000000_u32.to_le_bytes());
        payload.extend(2_u32.to_le_bytes());
        payload.extend(0x80000010_u32.to_le_bytes());
        payload.extend(527_u32.to_le_bytes());
        payload.extend(128_u32.to_le_bytes());
        payload.extend(1_774_080_u32.to_le_bytes());
        payload.extend([0_u8; 2]);
        payload.extend(block_payload(
            MONOPOLY_MARKER,
            1,
            &body,
            u32::try_from(body.len() + 4).unwrap(),
        ));

        let (blocks, warnings) = parse_payload_blocks(&payload, 0, 1, 0);

        assert!(warnings.is_empty());
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].rows.len(), 1);
        assert_eq!(blocks[0].rows[0].source.view, "raw");
        assert_eq!(blocks[0].rows[0].source.stream_key.as_deref(), Some("monopoly:128"));
        assert_eq!(blocks[0].rows[0].source.page_index, Some(16));
        assert_eq!(blocks[0].rows[0].source.segment_index, Some(31));
    }

    #[test]
    fn legacy_multi_row_roll_points_are_not_decoded_as_lottery_ids() {
        let body = (1..=6)
            .flat_map(|points| monopoly_row_bytes(points, &format!("item-{points}")))
            .collect::<Vec<_>>();
        let payload = block_payload(
            MONOPOLY_MARKER,
            6,
            &body,
            u32::try_from(body.len() + 4).unwrap(),
        );

        let (blocks, warnings) = parse_payload_blocks(&payload, 0, 1, 0);

        assert!(warnings.is_empty());
        assert_eq!(blocks[0].rows.len(), 6);
        assert_eq!(
            blocks[0]
                .rows
                .iter()
                .map(|row| row.roll_points)
                .collect::<Vec<_>>(),
            (1..=6).map(Some).collect::<Vec<_>>()
        );
        assert!(blocks[0].rows.iter().all(|row| row.lottery_id.is_none()));
    }

    #[test]
    fn monopoly_block_rejects_declared_size_mismatch() {
        let body = monopoly_row_bytes(2, "1036");
        let payload = block_payload(
            MONOPOLY_MARKER,
            1,
            &body,
            u32::try_from(body.len() + 5).unwrap(),
        );

        let (blocks, warnings) = parse_payload_blocks(&payload, 0, 1, 0);

        assert!(blocks.is_empty());
        assert!(warnings.iter().any(|warning| warning.code == "parse_error"));
    }

    #[test]
    fn monopoly_block_rejects_rows_only_declared_size() {
        let body = monopoly_row_bytes(2, "1036");
        let payload = block_payload(
            MONOPOLY_MARKER,
            1,
            &body,
            u32::try_from(body.len()).unwrap(),
        );

        let (blocks, warnings) = parse_payload_blocks(&payload, 0, 1, 0);

        assert!(blocks.is_empty());
        assert!(warnings.iter().any(|warning| warning.code == "parse_error"));
    }

    #[test]
    fn monopoly_block_rejects_invalid_lottery_tail() {
        let mut body = monopoly_row_bytes(2, "1036");
        body.extend(fstring("Lottery_"));
        let payload = block_payload(
            MONOPOLY_MARKER,
            1,
            &body,
            u32::try_from(body.len() + 4).unwrap(),
        );

        let (blocks, warnings) = parse_payload_blocks(&payload, 0, 1, 0);

        assert!(blocks.is_empty());
        assert!(warnings.iter().any(|warning| {
            warning.message.contains("monopoly block has 0 valid layouts")
        }));
    }

    #[test]
    fn fork_block_rejects_bytes_past_declared_rows() {
        let mut body = Vec::new();
        body.extend(fstring("fork_dustbin"));
        body.extend(fstring("ForkLottery_AnHunQu"));
        body.extend(639_131_653_353_040_000_u64.to_le_bytes());
        body.push(0xff);
        let payload = block_payload(
            FORK_MARKER,
            1,
            &body,
            u32::try_from(body.len() + 4).unwrap(),
        );

        let (blocks, warnings) = parse_payload_blocks(&payload, 0, 1, 0);

        assert!(blocks.is_empty());
        assert!(warnings.iter().any(|warning| {
            warning
                .message
                .contains("fork rows do not consume declared block size")
        }));
    }

    #[test]
    fn fork_block_rejects_rows_only_declared_size() {
        let mut body = Vec::new();
        body.extend(fstring("fork_dustbin"));
        body.extend(fstring("ForkLottery_AnHunQu"));
        body.extend(639_131_653_353_040_000_u64.to_le_bytes());
        let payload = block_payload(
            FORK_MARKER,
            1,
            &body,
            u32::try_from(body.len()).unwrap(),
        );

        let (blocks, warnings) = parse_payload_blocks(&payload, 0, 1, 0);

        assert!(blocks.is_empty());
        assert!(warnings.iter().any(|warning| warning.code == "parse_error"));
    }

    #[test]
    fn decodes_sample_fixture_payload() {
        let payload = base64::engine::general_purpose::STANDARD
            .decode("RkZvcmtMb3R0ZXJ5UmVjb3JkRGF0YQAAAAAANQAAAAEAAAANAAAAZm9ya19kdXN0YmluABQAAABGb3JrTG90dGVyeV9Bbkh1blF1AIAFZ8eTwd4I")
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

    #[test]
    fn new_rows_detects_appended_pages() {
        let before = vec![row("a", 0), row("b", 1)];
        let after = vec![row("a", 0), row("b", 1), row("c", 2), row("d", 3)];

        let delta = new_prefix_rows(&before, &after);

        assert_eq!(delta, vec![row("c", 2), row("d", 3)]);
    }

    #[test]
    fn new_rows_detects_prepended_pages() {
        let before = vec![row("c", 2), row("d", 3)];
        let after = vec![row("a", 0), row("b", 1), row("c", 2), row("d", 3)];

        let delta = new_prefix_rows(&before, &after);

        assert_eq!(delta, vec![row("a", 0), row("b", 1)]);
    }

    #[test]
    fn add_blocks_with_update_returns_no_full_rows_for_duplicate_block() {
        let mut assembler = ProtocolAssembler::default();
        let block = legacy_block(vec![row("a", 0)]);

        let first = assembler.add_blocks_with_update([block.clone()]);
        let second = assembler.add_blocks_with_update([block]);

        assert_eq!(first.rows.as_ref().map(Vec::len), Some(1));
        assert_eq!(first.new_rows, vec![row("a", 0)]);
        assert!(second.rows.is_none());
        assert!(second.new_rows.is_empty());
    }

    #[test]
    fn block_signature_distinguishes_lottery_id_payloads() {
        let mut assembler = ProtocolAssembler::default();
        let mut canhong = row("a", 0);
        canhong.lottery_id = Some("Lottery_Canhong".to_string());
        let mut permanent = canhong.clone();
        permanent.lottery_id = Some("Lottery_Permanent".to_string());

        let first = assembler.add_blocks_with_update([legacy_block(vec![canhong])]);
        let second = assembler.add_blocks_with_update([legacy_block(vec![permanent])]);

        assert_eq!(first.rows.as_ref().map(Vec::len), Some(1));
        assert_eq!(second.rows.as_ref().map(Vec::len), Some(2));
        assert_eq!(second.new_rows.len(), 1);
    }

    #[test]
    fn shifted_prefilter_skips_payload_without_markers() {
        let payload = vec![0x55_u8; 128];

        assert!(!shifted_view_contains_marker(
            &payload,
            8,
            1,
            payload.len().saturating_sub(8)
        ));
        let (blocks, warnings) = parse_payload_blocks(&payload, 0, 1, 0);
        assert!(blocks.is_empty());
        assert!(warnings.is_empty());
    }
}
