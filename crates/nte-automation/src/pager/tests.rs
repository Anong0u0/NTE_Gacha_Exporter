#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_pool_maps_capture_records_to_workflow_pools() {
        assert_eq!(
            record_pool(&RecordSnapshot {
                record_id: "a".to_string(),
                record_key: "a".to_string(),
                pool_id: "CardPool_Character".to_string(),
                record_type: "monopoly".to_string(),
            }),
            Some("limited".to_string())
        );
        assert_eq!(
            record_pool(&RecordSnapshot {
                record_id: "b".to_string(),
                record_key: "b".to_string(),
                pool_id: "CardPool_NewRole".to_string(),
                record_type: "monopoly".to_string(),
            }),
            Some("standard".to_string())
        );
        assert_eq!(
            record_pool(&RecordSnapshot {
                record_id: "c".to_string(),
                record_key: "c".to_string(),
                pool_id: "ForkLottery_AnHunQu".to_string(),
                record_type: "fork".to_string(),
            }),
            Some("fork".to_string())
        );
    }

    #[test]
    fn consecutive_known_record_count_only_counts_latest_run() {
        let records = vec![
            snapshot("new", "CardPool_Character", "monopoly"),
            snapshot("old-1", "CardPool_Character", "monopoly"),
            snapshot("old-2", "CardPool_Character", "monopoly"),
        ];
        let known_counts = record_key_counts(&["old-1".to_string(), "old-2".to_string()]);

        assert_eq!(consecutive_known_record_count(&records, &known_counts), 2);
    }

    #[test]
    fn consecutive_known_record_count_stops_at_latest_unknown() {
        let records = vec![
            snapshot("old-1", "CardPool_Character", "monopoly"),
            snapshot("new", "CardPool_Character", "monopoly"),
        ];
        let known_counts = record_key_counts(&["old-1".to_string()]);

        assert_eq!(consecutive_known_record_count(&records, &known_counts), 0);
    }

    #[test]
    fn consecutive_known_record_count_respects_duplicate_key_counts() {
        let records = vec![
            snapshot_with_key("id-a", "same", "CardPool_Character", "monopoly"),
            snapshot_with_key("id-b", "same", "CardPool_Character", "monopoly"),
        ];
        let one_known = record_key_counts(&["same".to_string()]);
        let two_known = record_key_counts(&["same".to_string(), "same".to_string()]);

        assert_eq!(consecutive_known_record_count(&records, &one_known), 1);
        assert_eq!(consecutive_known_record_count(&records, &two_known), 2);
    }

    #[test]
    fn auto_page_result_serialization_excludes_png_bytes() {
        let mut result = AutoPageResult::failed("failed", Vec::new(), Vec::new());
        result.diagnostics.page_context_png = Some(vec![1, 2, 3, 4]);
        result.diagnostics.failure_kind = Some("fresh_page_number_unreadable".to_string());

        let text = serde_json::to_string(&result).unwrap();

        assert!(text.contains("fresh_page_number_unreadable"));
        assert!(!text.contains("page_context_png"));
        assert!(!text.contains("[1,2,3,4]"));
    }

    fn snapshot(record_id: &str, pool_id: &str, record_type: &str) -> RecordSnapshot {
        snapshot_with_key(record_id, record_id, pool_id, record_type)
    }

    fn snapshot_with_key(
        record_id: &str,
        record_key: &str,
        pool_id: &str,
        record_type: &str,
    ) -> RecordSnapshot {
        RecordSnapshot {
            record_id: record_id.to_string(),
            record_key: record_key.to_string(),
            pool_id: pool_id.to_string(),
            record_type: record_type.to_string(),
        }
    }
}
