#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    };

    use crate::{NumberCandidate, NumberObservation, ObservationState};

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
        result.diagnostics.visual.template_transition = Some(TemplateTransitionDiagnostics {
            source_template: "homeBoardFileIcon".to_string(),
            target_template: None,
            verification: "source_gone".to_string(),
            click_point: Some(Point { x: 448, y: 995 }),
            first_click_settle: 0.5,
            post_click_settle: 0.5,
            click_attempts: 2,
            checks: 4,
            verified: false,
        });

        let text = serde_json::to_string(&result).unwrap();

        assert!(text.contains("fresh_page_number_unreadable"));
        assert!(text.contains("homeBoardFileIcon"));
        assert!(text.contains("source_gone"));
        assert!(text.contains("\"click_attempts\":2"));
        assert!(text.contains("\"first_click_settle\":0.5"));
        assert!(!text.contains("context_png"));
        assert!(!text.contains("raw_page_png"));
        assert!(!text.contains("[1,2,3,4]"));
        assert!(!text.contains("[5,6,7,8]"));
    }

    #[test]
    fn capture_window_stalled_message_keeps_page_window_format() {
        assert_eq!(
            capture_window_stalled_message("standard", 8, 1, 7, false),
            "capture window stalled: pool=standard visited_pages=8 decoded_pages=1 max_visited_pages=7"
        );
    }

    #[test]
    fn capture_window_stalled_message_marks_pool_completion() {
        assert_eq!(
            capture_window_stalled_message("standard", 59, 55, 55, true),
            "capture window stalled: pool=standard phase=pool_complete visited_pages=59 decoded_pages=55 max_visited_pages=55"
        );
    }
    #[test]
    fn pool_completion_waits_for_capture_before_emitting_completed_status() {
        let (event_tx, event_rx) = mpsc::channel();
        let control_calls = Arc::new(AtomicUsize::new(0));
        let mut options = AutoPageOptions::new(42, Arc::new(AtomicBool::new(false)));
        options.click_poll_interval = 0.0;
        options.control = Some({
            let event_tx = event_tx.clone();
            let control_calls = Arc::clone(&control_calls);
            Arc::new(move |context| {
                assert!(context.pool_complete);
                assert_eq!(context.pool, "standard");
                assert_eq!(context.visited_pages, 59);
                let call = control_calls.fetch_add(1, Ordering::SeqCst);
                event_tx
                    .send(if call == 0 {
                        "control:wait"
                    } else {
                        "control:continue"
                    })
                    .unwrap();
                if call == 0 {
                    AutoPageControlDecision::WaitCapture {
                        decoded_pages: 55,
                        max_visited_pages: 55,
                    }
                } else {
                    AutoPageControlDecision::Continue
                }
            })
        });
        options.on_status = Some({
            let event_tx = event_tx.clone();
            Arc::new(move |status| {
                let event = match status.kind.as_str() {
                    "diagnostic" => "status:diagnostic",
                    "pool_completed" => "status:pool_completed",
                    other => panic!("unexpected status: {other}"),
                };
                event_tx.send(event).unwrap();
            })
        });
        let mut pager = control_test_pager(options);

        pager
            .complete_pool(
                "standard",
                "standardStep",
                &PageCursor {
                    current: 59,
                    total: Some(59),
                    fingerprint: fingerprint(&[0], &[0]),
                },
                59,
            )
            .unwrap();

        assert_eq!(
            event_rx.try_iter().collect::<Vec<_>>(),
            [
                "control:wait",
                "status:diagnostic",
                "control:continue",
                "status:pool_completed",
            ]
        );
    }

    #[test]
    fn template_match_center_uses_actual_matched_size() {
        let matched = TemplateMatch {
            name: "scaled".to_string(),
            matched: true,
            edge_score: 1.0,
            gray_score: 1.0,
            point: Point { x: 100, y: 200 },
            size: Size {
                width: 39,
                height: 45,
            },
            scale: 1.25,
            searched_rect: crate::model::Rect {
                x: 80,
                y: 180,
                width: 100,
                height: 100,
            },
            candidate_count: 1,
        };

        assert_eq!(template_match_center(&matched), Point { x: 119, y: 222 });
    }

    #[test]
    fn template_transition_accepts_target_without_extra_click() {
        let mut state = TemplateTransitionState::default();

        let decision = state.observe(true, true, TemplatePresence::Unavailable);

        assert_eq!(
            decision,
            TemplateTransitionDecision::Complete(TemplateTransitionVerification::Target)
        );
        assert_eq!(state.clicks, 0);
    }

    #[test]
    fn template_transition_requires_stable_source_absence() {
        let mut state = TemplateTransitionState::default();
        assert_eq!(
            state.observe(false, false, TemplatePresence::Present),
            TemplateTransitionDecision::ClickSource {
                first_attempt: true
            }
        );
        state.clicked();

        assert_eq!(
            state.observe(false, false, TemplatePresence::Absent),
            TemplateTransitionDecision::Wait
        );
        assert_eq!(
            state.observe(false, false, TemplatePresence::Absent),
            TemplateTransitionDecision::Complete(TemplateTransitionVerification::SourceGone)
        );
    }

    #[test]
    fn template_transition_retries_when_source_remains_visible() {
        let mut state = TemplateTransitionState::default();
        assert_eq!(
            state.observe(false, false, TemplatePresence::Present),
            TemplateTransitionDecision::ClickSource {
                first_attempt: true
            }
        );
        state.clicked();
        assert_eq!(
            state.observe(false, false, TemplatePresence::Absent),
            TemplateTransitionDecision::Wait
        );

        assert_eq!(
            state.observe(false, false, TemplatePresence::Present),
            TemplateTransitionDecision::ClickSource {
                first_attempt: false
            }
        );
        assert_eq!(state.source_absence_samples, 0);
    }

    #[test]
    fn template_transition_waits_for_required_target_when_source_disappears() {
        let mut state = TemplateTransitionState::default();
        assert_eq!(
            state.observe(true, false, TemplatePresence::Present),
            TemplateTransitionDecision::ClickSource {
                first_attempt: true
            }
        );
        state.clicked();

        assert_eq!(
            state.observe(true, false, TemplatePresence::Absent),
            TemplateTransitionDecision::Wait
        );
        assert_eq!(
            state.observe(true, true, TemplatePresence::Absent),
            TemplateTransitionDecision::Complete(TemplateTransitionVerification::Target)
        );
    }

    #[test]
    fn template_transition_unavailable_breaks_absence_stability() {
        let mut state = TemplateTransitionState::default();
        assert_eq!(
            state.observe(false, false, TemplatePresence::Present),
            TemplateTransitionDecision::ClickSource {
                first_attempt: true
            }
        );
        state.clicked();

        assert_eq!(
            state.observe(false, false, TemplatePresence::Absent),
            TemplateTransitionDecision::Wait
        );
        assert_eq!(state.source_absence_samples, 1);
        assert_eq!(
            state.observe(false, false, TemplatePresence::Unavailable),
            TemplateTransitionDecision::Wait
        );
        assert_eq!(state.source_absence_samples, 0);
        assert_eq!(
            state.observe(false, false, TemplatePresence::Absent),
            TemplateTransitionDecision::Wait
        );
        assert_eq!(
            state.observe(false, false, TemplatePresence::Absent),
            TemplateTransitionDecision::Complete(TemplateTransitionVerification::SourceGone)
        );
    }

    #[test]
    fn template_transition_retries_unavailable_observations_without_clicking() {
        let mut state = TemplateTransitionState::default();

        for _ in 0..3 {
            assert_eq!(
                state.observe(false, false, TemplatePresence::Unavailable),
                TemplateTransitionDecision::Wait
            );
        }

        assert_eq!(state.clicks, 0);
        assert_eq!(state.source_absence_samples, 0);
    }

    #[test]
    fn template_transition_accepts_target_after_unavailable_observation() {
        let mut state = TemplateTransitionState::default();
        assert_eq!(
            state.observe(true, false, TemplatePresence::Unavailable),
            TemplateTransitionDecision::Wait
        );

        assert_eq!(
            state.observe(true, true, TemplatePresence::Unavailable),
            TemplateTransitionDecision::Complete(TemplateTransitionVerification::Target)
        );
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

    fn control_test_pager(options: AutoPageOptions) -> AutoPager {
        let profile = load_profile().unwrap();
        let window = GameWindow {
            hwnd: 0,
            pid: options.pid,
            class_name: "test".to_string(),
            title: "test".to_string(),
            client_rect: crate::window::ClientRect {
                left: 0,
                top: 0,
                right: profile.base_client_size.width as i32,
                bottom: profile.base_client_size.height as i32,
            },
        };
        let capture = WindowCaptureClient::new(window.hwnd);
        let matcher = ImageTemplateMatcher::new(profile.clone());
        AutoPager {
            options,
            window,
            profile,
            capture,
            page_reader: PageNumberReader::default(),
            matcher,
            tooltip: AutomationTooltip::new(false),
            started_at: Instant::now(),
            diagnostics: AutoPageDiagnostics::default(),
            completed_pools: Vec::new(),
            skipped_pools: Vec::new(),
            visited_pages_by_pool: BTreeMap::new(),
            last_page_by_pool: BTreeMap::new(),
        }
    }

    #[test]
    fn stable_page_total_requires_consecutive_valid_samples() {
        let mut total = StablePageTotal::default();
        total.observe(Some(191), 1);
        assert_eq!(total.confirmed(), None);
        total.observe(Some(191), 1);
        assert_eq!(total.confirmed(), Some(191));

        total.observe(Some(190), 1);
        assert_eq!(total.confirmed(), None);
        total.observe(None, 1);
        total.observe(Some(191), 1);
        assert_eq!(total.confirmed(), None);
        total.observe(Some(55), 56);
        assert_eq!(total.confirmed(), None);
    }

    #[test]
    fn advancing_page_preserves_authoritative_total() {
        for total in [None, Some(191)] {
            let previous = PageCursor {
                current: 55,
                total,
                fingerprint: fingerprint(&[0], &[0]),
            };

            let next = previous.advance(56, fingerprint(&[1], &[0]));

            assert_eq!(next.current, 56);
            assert_eq!(next.total, total);
        }
    }

    #[test]
    fn unchanged_fingerprint_never_confirms_transition() {
        let baseline = fingerprint(&[0, 0, 0, 0], &[0, 0, 0, 0]);
        let mut tracker = PageTransitionTracker::new();

        assert_eq!(
            tracker.observe(&baseline, &baseline),
            PageTransitionProgress::Waiting
        );
        assert_eq!(
            tracker.observe(&baseline, &baseline),
            PageTransitionProgress::Waiting
        );
        assert_eq!(tracker.finish(), PageTransitionTerminal::Unchanged);
    }

    #[test]
    fn changed_fingerprint_requires_two_consecutive_stable_samples() {
        let baseline = fingerprint(&[0, 0, 0, 0], &[0, 0, 0, 0]);
        let changed = fingerprint(&[1, 1, 0, 0], &[0, 0, 0, 0]);
        let mut tracker = PageTransitionTracker::new();

        assert_eq!(
            tracker.observe(&baseline, &changed),
            PageTransitionProgress::Candidate
        );
        assert_eq!(
            tracker.observe(&baseline, &changed),
            PageTransitionProgress::Confirmed
        );
    }

    #[test]
    fn animation_frame_breaks_changed_stability_and_disables_retry() {
        let baseline = fingerprint(&[0, 0, 0, 0], &[0, 0, 0, 0]);
        let changed = fingerprint(&[1, 1, 0, 0], &[0, 0, 0, 0]);
        let animation = fingerprint(&[1, 0, 1, 0], &[1, 0, 0, 0]);
        let mut tracker = PageTransitionTracker::new();

        assert_eq!(
            tracker.observe(&baseline, &changed),
            PageTransitionProgress::Candidate
        );
        assert_eq!(
            tracker.observe(&baseline, &animation),
            PageTransitionProgress::Waiting
        );
        assert_eq!(
            tracker.observe(&baseline, &changed),
            PageTransitionProgress::Candidate
        );
        assert_eq!(tracker.finish(), PageTransitionTerminal::Indeterminate);
        assert_eq!(
            tracker.observe(&baseline, &changed),
            PageTransitionProgress::Confirmed
        );
    }

    #[test]
    fn suffix_change_never_confirms_page_transition() {
        let baseline = fingerprint(&[0, 0, 0, 0], &[0, 0, 0, 0]);
        let wrong_suffix = fingerprint(&[1, 1, 0, 0], &[1, 0, 0, 0]);
        let mut tracker = PageTransitionTracker::new();

        assert_eq!(
            tracker.observe(&baseline, &wrong_suffix),
            PageTransitionProgress::Waiting
        );
        assert_eq!(
            tracker.observe(&baseline, &wrong_suffix),
            PageTransitionProgress::Waiting
        );

        assert_eq!(tracker.finish(), PageTransitionTerminal::Indeterminate);
    }

    #[test]
    fn missing_fingerprint_disables_click_retry() {
        let baseline = fingerprint(&[0, 0, 0, 0], &[0, 0, 0, 0]);
        let changed = fingerprint(&[1, 1, 0, 0], &[0, 0, 0, 0]);
        let mut tracker = PageTransitionTracker::new();

        assert_eq!(
            tracker.observe(&baseline, &changed),
            PageTransitionProgress::Candidate
        );
        tracker.observe_missing();
        assert_eq!(
            tracker.observe(&baseline, &changed),
            PageTransitionProgress::Candidate
        );

        assert_eq!(tracker.finish(), PageTransitionTerminal::Indeterminate);
    }

    #[test]
    fn ocr_validation_never_supplies_navigation_state() {
        let expected = page_observation(
            56,
            191,
            ObservationState::Confident,
            ObservationState::Confident,
        );
        let wrong_current = page_observation(
            88,
            191,
            ObservationState::Confident,
            ObservationState::Confident,
        );
        let impossible_total = page_observation(
            56,
            50,
            ObservationState::Confident,
            ObservationState::Confident,
        );
        let ambiguous_current = page_observation(
            56,
            191,
            ObservationState::Ambiguous,
            ObservationState::Confident,
        );
        let ambiguous_total = page_observation(
            56,
            191,
            ObservationState::Confident,
            ObservationState::Ambiguous,
        );

        assert_eq!(
            page_ocr_validation(&expected, 56, Some(191)),
            PageOcrValidation::Matches
        );
        assert_eq!(
            page_ocr_validation(&wrong_current, 56, Some(191)),
            PageOcrValidation::Mismatch
        );
        assert_eq!(
            page_ocr_validation(&impossible_total, 56, None),
            PageOcrValidation::Mismatch
        );
        assert_eq!(
            page_ocr_validation(&expected, 56, Some(190)),
            PageOcrValidation::Mismatch
        );
        assert_eq!(
            page_ocr_validation(&ambiguous_current, 56, Some(191)),
            PageOcrValidation::Ambiguous
        );
        assert_eq!(
            page_ocr_validation(&ambiguous_total, 56, Some(191)),
            PageOcrValidation::Ambiguous
        );
    }

    fn fingerprint(current: &[u8], suffix: &[u8]) -> PageFingerprint {
        PageFingerprint {
            current_mask: current.to_vec(),
            suffix_mask: suffix.to_vec(),
        }
    }

    fn page_observation(
        current: u32,
        total: u32,
        current_state: ObservationState,
        total_state: ObservationState,
    ) -> PageObservation {
        PageObservation {
            slash: ObservationState::Confident,
            current: NumberObservation {
                state: current_state,
                candidates: vec![NumberCandidate {
                    value: current,
                    score: 0.9,
                }],
            },
            total: NumberObservation {
                state: total_state,
                candidates: vec![NumberCandidate {
                    value: total,
                    score: 0.9,
                }],
            },
            fingerprint: Some(fingerprint(&[0], &[0])),
        }
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
