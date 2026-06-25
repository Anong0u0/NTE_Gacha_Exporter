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

    fn row(item_id: &str, row_index: u32) -> ParsedRow {
        ParsedRow {
            record_type: RecordType::Monopoly,
            ticks: 639_131_653_353_040_000,
            time: None,
            pool_id: Some("CardPool_Character".to_string()),
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
