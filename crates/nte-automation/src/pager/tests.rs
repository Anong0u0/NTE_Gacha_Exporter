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
        result.diagnostics.context_png = Some(vec![1, 2, 3, 4]);
        result.diagnostics.raw_page_png = Some(vec![5, 6, 7, 8]);
        result.diagnostics.failure_kind = Some("fresh_page_number_unreadable".to_string());

        let text = serde_json::to_string(&result).unwrap();

        assert!(text.contains("fresh_page_number_unreadable"));
        assert!(!text.contains("context_png"));
        assert!(!text.contains("raw_page_png"));
        assert!(!text.contains("[1,2,3,4]"));
        assert!(!text.contains("[5,6,7,8]"));
    }

    #[test]
    fn status_message_uses_label_when_available() {
        let labels = BTreeMap::from([(
            "ui_forkshop_03".to_string(),
            "Arc Research".to_string(),
        )]);

        let status =
            StatusEvent::new("template verified", "template").step("arcResearch").to_status(
                1.0,
                &labels,
            );

        assert_eq!(status.message, "Arc Research");
        assert_eq!(status.technical_detail, "");
    }

    #[test]
    fn status_message_keeps_english_when_label_missing() {
        let status = StatusEvent::new("template verified", "template")
            .step("arcResearch")
            .to_status(1.0, &BTreeMap::new());

        assert_eq!(status.message, "template verified");
    }

    #[test]
    fn page_context_rect_expands_to_include_cursor_with_margin() {
        let page_rect = crate::model::Rect {
            x: 100,
            y: 100,
            width: 20,
            height: 10,
        };

        let rect = page_context_rect(
            page_rect,
            Size {
                width: 500,
                height: 400,
            },
            Some(Point { x: 300, y: 200 }),
        );

        assert_eq!(
            rect,
            crate::model::Rect {
                x: 80,
                y: 95,
                width: 269,
                height: 154,
            }
        );
        assert!(rect_contains_point(rect, Point { x: 100, y: 100 }));
        assert!(rect_contains_point(rect, Point { x: 119, y: 109 }));
        assert!(rect_contains_point(rect, Point { x: 300, y: 200 }));
        assert!(Point { x: 300, y: 200 }.x - rect.x >= CURSOR_CONTEXT_PADDING);
        assert!(Point { x: 300, y: 200 }.y - rect.y >= CURSOR_CONTEXT_PADDING);
        assert_eq!(
            rect.right() - Point { x: 300, y: 200 }.x - 1,
            CURSOR_CONTEXT_PADDING
        );
        assert_eq!(
            rect.bottom() - Point { x: 300, y: 200 }.y - 1,
            CURSOR_CONTEXT_PADDING
        );
    }

    #[test]
    fn page_context_rect_clamps_cursor_margin_at_client_edge() {
        let page_rect = crate::model::Rect {
            x: 100,
            y: 100,
            width: 20,
            height: 10,
        };

        let rect = page_context_rect(
            page_rect,
            Size {
                width: 320,
                height: 240,
            },
            Some(Point { x: 319, y: 239 }),
        );

        assert_eq!(
            rect,
            crate::model::Rect {
                x: 80,
                y: 95,
                width: 240,
                height: 145,
            }
        );
        assert!(rect_contains_point(rect, Point { x: 319, y: 239 }));
    }

    #[test]
    fn page_context_rect_ignores_cursor_outside_client_when_filtered() {
        let page_rect = crate::model::Rect {
            x: 100,
            y: 100,
            width: 20,
            height: 10,
        };

        let cursor = Point { x: -1, y: 120 };
        let cursor = point_in_size(
            cursor,
            Size {
                width: 320,
                height: 240,
            },
        )
        .then_some(cursor);
        let rect = page_context_rect(
            page_rect,
            Size {
                width: 320,
                height: 240,
            },
            cursor,
        );

        assert_eq!(
            rect,
            crate::model::Rect {
                x: 80,
                y: 95,
                width: 60,
                height: 20,
            }
        );
    }

    #[test]
    fn draw_cursor_marker_marks_center_and_edges_without_resizing() {
        let mut image = image::RgbaImage::new(24, 24);
        let color = image::Rgba([0, 220, 255, 255]);

        draw_cursor_marker(&mut image, Point { x: 4, y: 4 }, color);

        assert_eq!(*image.get_pixel(4, 4), color);
        assert_eq!(*image.get_pixel(0, 4), color);
        assert_eq!(*image.get_pixel(4, 0), color);
        assert_eq!(image.dimensions(), (24, 24));
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
