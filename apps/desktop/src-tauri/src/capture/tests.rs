#[cfg(test)]
mod tests {
    use super::*;

    fn test_status(session_id: &str, state: &str, updated_at: f64) -> CaptureStatus {
        CaptureStatus {
            session_id: session_id.to_string(),
            state: state.to_string(),
            mode: "live_only".to_string(),
            records_count: 0,
            latest_records: Vec::new(),
            counters: CaptureCounters::default(),
            started_at: updated_at,
            updated_at,
            target: None,
            auto_page: None,
            raw_path: None,
            error: None,
            document: None,
            import_report: None,
        }
    }

    fn failed_status(session_id: &str) -> CaptureStatus {
        let mut status = test_status(session_id, "failed", 1.0);
        status.mode = "auto_page_full".to_string();
        status.records_count = 1;
        status.latest_records = vec![json!({ "record_id": "private-record" })];
        status.raw_path = Some("data/runs/raw-private.jsonl".to_string());
        status.error = Some(runtime_error("auto_page_failed", "cannot read page number"));
        status
    }

    fn test_session(status: CaptureStatus) -> Arc<CaptureRuntimeSession> {
        Arc::new(CaptureRuntimeSession {
            status: Mutex::new(status),
            stop: Arc::new(AtomicBool::new(false)),
            handle: Mutex::new(None),
        })
    }

    fn test_meta() -> CaptureSessionMeta {
        CaptureSessionMeta {
            profile_name: "default".to_string(),
            source_kind: "test".to_string(),
            source_path: None,
            full_update: false,
            import_report: None,
        }
    }

    #[test]
    fn latest_records_read_capture_document_nte_list() {
        let records = (0..12)
            .map(|index| json!({ "record_id": format!("r{index}") }))
            .collect::<Vec<_>>();
        let document = json!({ "nte": { "list": records } });

        let latest = latest_records_from_capture_document(&document);

        assert_eq!(latest.len(), 10);
        assert_eq!(latest[0]["record_id"], "r11");
        assert_eq!(latest[9]["record_id"], "r2");
    }

    #[test]
    fn latest_records_missing_capture_document_list_returns_empty() {
        assert!(latest_records_from_capture_document(&json!({ "records": [] })).is_empty());
    }

    #[test]
    fn capture_pool_uses_auto_page_workflow_pool_keys() {
        assert_eq!(
            capture_pool("monopoly", Some("CardPool_Character")),
            Some("limited")
        );
        assert_eq!(
            capture_pool("monopoly", Some("CardPool_NewRole")),
            Some("standard")
        );
        assert_eq!(
            capture_pool("fork", Some("ForkLottery_AnHunQu")),
            Some("fork")
        );
        assert_eq!(
            capture_pool("monopoly", Some("CardPool_Weapon")),
            None
        );
    }

    #[test]
    fn prune_capture_session_maps_keeps_active_and_preserved_sessions() {
        let mut sessions = HashMap::from([
            (
                "active".to_string(),
                test_session(test_status("active", "running", 1.0)),
            ),
            (
                "preserve".to_string(),
                test_session(test_status("preserve", "completed", 1.0)),
            ),
            (
                "old".to_string(),
                test_session(test_status("old", "failed", 1.0)),
            ),
        ]);
        let mut captures = HashMap::from([
            ("active".to_string(), test_meta()),
            ("preserve".to_string(), test_meta()),
            ("old".to_string(), test_meta()),
        ]);

        prune_capture_session_maps(&mut sessions, &mut captures, "preserve", 2_000.0);

        assert!(sessions.contains_key("active"));
        assert!(sessions.contains_key("preserve"));
        assert!(!sessions.contains_key("old"));
        assert!(!captures.contains_key("old"));
    }

    #[test]
    fn prune_capture_session_maps_retains_latest_terminal_limit() {
        let mut sessions = HashMap::new();
        let mut captures = HashMap::new();
        for index in 0..25 {
            let session_id = format!("s{index:02}");
            sessions.insert(
                session_id.clone(),
                test_session(test_status(&session_id, "completed", f64::from(index))),
            );
            captures.insert(session_id, test_meta());
        }

        prune_capture_session_maps(&mut sessions, &mut captures, "s24", 100.0);

        assert_eq!(sessions.len(), 21);
        assert!(sessions.contains_key("s24"));
        assert!(!sessions.contains_key("s00"));
        assert!(!sessions.contains_key("s03"));
        assert!(sessions.contains_key("s04"));
    }

    #[test]
    fn support_json_excludes_record_payloads_and_raw_contents() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("data/runs")).unwrap();
        std::fs::write(tmp.path().join("data/runs/raw-private.jsonl"), b"private raw").unwrap();
        let status = failed_status("session/private");

        let result = write_capture_support(SupportRequest {
            root: tmp.path(),
            status: &status,
            source_kind: "pktmon-auto-page-capture",
            auto_result: None,
        });

        let path = result.json_path.unwrap();
        let text = std::fs::read_to_string(path).unwrap();
        assert!(text.contains("nte-gacha-capture-support"));
        assert!(text.contains("raw_path_exists"));
        assert!(text.contains("\"raw_path_exists\": true"));
        assert!(!text.contains("private-record"));
        assert!(!text.contains("raw-private.jsonl"));
        assert!(!text.contains("private raw"));
        assert!(result.image_path.is_none());
    }

    #[test]
    fn support_writer_saves_auto_page_context_image_when_available() {
        let tmp = tempfile::tempdir().unwrap();
        let status = failed_status("session-image");
        let mut auto_result =
            AutoPageRunResult::failed("cannot read page number", Vec::new(), Vec::new());
        auto_result.diagnostics.page_context_png = Some(vec![137, 80, 78, 71]);

        let result = write_capture_support(SupportRequest {
            root: tmp.path(),
            status: &status,
            source_kind: "pktmon-auto-page-capture",
            auto_result: Some(&auto_result),
        });

        let image_path = result.image_path.unwrap();
        assert_eq!(std::fs::read(image_path).unwrap(), vec![137, 80, 78, 71]);
        assert!(std::fs::read_to_string(result.json_path.unwrap())
            .unwrap()
            .contains("support_image_path"));
    }

    #[test]
    fn attach_capture_support_adds_paths_to_runtime_error() {
        let tmp = tempfile::tempdir().unwrap();
        let mut status = failed_status("session-paths");

        attach_capture_support(tmp.path(), &mut status, "pktmon-auto-page-capture", None);

        let error = status.error.unwrap();
        assert_eq!(error.code, "auto_page_failed");
        let support_path = Path::new(error.support_path.as_deref().unwrap());
        assert_eq!(support_path.parent().unwrap().file_name().unwrap(), "support");
        assert_eq!(
            support_path
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .file_name()
                .unwrap(),
            "data"
        );
        assert!(error.support_image_path.is_none());
    }

    #[test]
    fn support_rotate_keeps_latest_three_bundles() {
        let tmp = tempfile::tempdir().unwrap();
        let support_dir = tmp.path().join("data/support");
        std::fs::create_dir_all(&support_dir).unwrap();
        for index in 0..5 {
            write_support_bundle(&support_dir, &format!("capture-s{index}"));
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        rotate_capture_support_files(tmp.path(), 3, Some("capture-s4")).unwrap();

        assert_support_bundle_exists(&support_dir, "capture-s4");
        assert_support_bundle_exists(&support_dir, "capture-s3");
        assert_support_bundle_exists(&support_dir, "capture-s2");
        assert_support_bundle_missing(&support_dir, "capture-s1");
        assert_support_bundle_missing(&support_dir, "capture-s0");
    }

    #[test]
    fn support_rotate_removes_orphan_images_but_skips_unrelated_files() {
        let tmp = tempfile::tempdir().unwrap();
        let support_dir = tmp.path().join("data/support");
        std::fs::create_dir_all(&support_dir).unwrap();
        write_support_bundle(&support_dir, "capture-current");
        std::fs::write(support_dir.join("capture-orphan-page-number.png"), b"orphan").unwrap();
        std::fs::write(support_dir.join("notes.txt"), b"keep").unwrap();
        std::fs::create_dir(support_dir.join("capture-dir.json")).unwrap();

        rotate_capture_support_files(tmp.path(), 3, Some("capture-current")).unwrap();

        assert_support_bundle_exists(&support_dir, "capture-current");
        assert!(!support_dir
            .join("capture-orphan-page-number.png")
            .exists());
        assert_eq!(std::fs::read(support_dir.join("notes.txt")).unwrap(), b"keep");
        assert!(support_dir.join("capture-dir.json").is_dir());
    }

    #[test]
    fn support_rotate_reports_symlink_support_dir_without_deleting_target() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let data_dir = tmp.path().join("data");
        std::fs::create_dir_all(&data_dir).unwrap();
        if create_dir_symlink(outside.path(), data_dir.join("support")).is_err() {
            return;
        }
        std::fs::write(outside.path().join("capture-outside.json"), b"outside").unwrap();

        let error = rotate_capture_support_files(tmp.path(), 3, None).unwrap_err();

        assert!(error.to_string().contains("support path is symlink"));
        assert_eq!(std::fs::read(outside.path().join("capture-outside.json")).unwrap(), b"outside");
    }

    #[test]
    fn support_writer_rejects_symlink_support_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let data_dir = tmp.path().join("data");
        std::fs::create_dir_all(&data_dir).unwrap();
        if create_dir_symlink(outside.path(), data_dir.join("support")).is_err() {
            return;
        }
        let status = failed_status("session-symlink");

        let result = write_capture_support(SupportRequest {
            root: tmp.path(),
            status: &status,
            source_kind: "pktmon-auto-page-capture",
            auto_result: None,
        });

        assert!(result.json_path.is_none());
        assert!(result.error.unwrap().contains("support path is symlink"));
        assert!(std::fs::read_dir(outside.path()).unwrap().next().is_none());
    }

    #[test]
    fn support_writer_sanitizes_session_id_and_stays_in_support_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let status = failed_status("../../escape");

        let result = write_capture_support(SupportRequest {
            root: tmp.path(),
            status: &status,
            source_kind: "pktmon-auto-page-capture",
            auto_result: None,
        });

        let path = result.json_path.unwrap();
        assert_eq!(path.parent().unwrap(), tmp.path().join("data/support"));
        assert_eq!(path.file_name().unwrap(), "capture-------escape.json");
        assert!(!tmp.path().join("escape.json").exists());
    }

    #[test]
    fn auto_page_coordinator_skips_when_snapshot_has_known_run() {
        let known = (1..=6).map(|index| format!("old-{index}")).collect::<Vec<_>>();
        let coordinator = AutoPageCoordinator::new(false, &known);
        let records = vec![
            automation_record("new", "CardPool_NewRole", "monopoly"),
            automation_record("old-1", "CardPool_NewRole", "monopoly"),
            automation_record("old-2", "CardPool_NewRole", "monopoly"),
            automation_record("old-3", "CardPool_NewRole", "monopoly"),
            automation_record("old-4", "CardPool_NewRole", "monopoly"),
            automation_record("old-5", "CardPool_NewRole", "monopoly"),
            automation_record("old-6", "CardPool_NewRole", "monopoly"),
        ];

        coordinator.add_progress(&progress_with_rows(vec![parsed_row("CardPool_NewRole", 0)]), Some(&records));

        assert_eq!(
            coordinator.decision(control_context("standard", 2)),
            AutoPageControlDecision::SkipPool {
                duplicate_records: 6,
            }
        );
    }

    #[test]
    fn auto_page_coordinator_blocks_when_pager_exceeds_capture_window() {
        let coordinator = AutoPageCoordinator::new(true, &[]);
        coordinator.add_progress(
            &progress_with_rows(vec![parsed_row("CardPool_NewRole", 0)]),
            None,
        );

        assert_eq!(
            coordinator.decision(control_context("standard", 7)),
            AutoPageControlDecision::Continue
        );
        assert_eq!(
            coordinator.decision(control_context("standard", 8)),
            AutoPageControlDecision::WaitCapture {
                decoded_pages: 1,
                max_visited_pages: 7,
            }
        );
    }

    #[test]
    fn auto_page_capture_window_stalled_has_specific_error_code() {
        let error = auto_page_runtime_error(
            "capture window stalled: pool=standard visited_pages=8 decoded_pages=1 max_visited_pages=7",
        );

        assert_eq!(error.code, AUTO_PAGE_CAPTURE_WINDOW_STALLED_CODE);
    }

    #[test]
    fn other_auto_page_failures_keep_generic_error_code() {
        let error = auto_page_runtime_error("cannot read page number");

        assert_eq!(error.code, AUTO_PAGE_FAILED_CODE);
    }

    #[test]
    fn capture_start_options_override_page_record_wait() {
        let stop = Arc::new(AtomicBool::new(false));
        let mut options = AutomationOptions::new(123, stop);

        apply_capture_start_options(
            &mut options,
            &CaptureStartOptions {
                page_record_min_wait_ms: Some(500),
            },
        );

        assert_eq!(options.page_record_min_wait, Duration::from_millis(500));
    }

    #[test]
    fn capture_start_options_clamp_page_record_wait() {
        let stop = Arc::new(AtomicBool::new(false));
        let mut options = AutomationOptions::new(123, stop);

        apply_capture_start_options(
            &mut options,
            &CaptureStartOptions {
                page_record_min_wait_ms: Some(2000),
            },
        );

        assert_eq!(options.page_record_min_wait, Duration::from_millis(1500));
    }

    #[test]
    fn auto_page_coordinator_replaces_page_counts_from_full_snapshot() {
        let coordinator = AutoPageCoordinator::new(true, &[]);
        coordinator.add_progress(
            &progress_with_rows(vec![
                parsed_row("CardPool_NewRole", 0),
                parsed_row("CardPool_NewRole", 1),
            ]),
            None,
        );
        coordinator.add_progress(
            &progress_with_rows(vec![parsed_row("CardPool_NewRole", 0)]),
            None,
        );

        assert_eq!(coordinator.counts().get("standard"), Some(&1));
    }

    #[test]
    fn capture_drain_does_not_require_skipped_pool_pages() {
        let status = test_status("drain-skipped", "running", 1.0);
        let runtime = test_session(status);
        let coordinator = Arc::new(AutoPageCoordinator::new(false, &[]));
        let mut visited = BTreeMap::new();
        visited.insert("standard".to_string(), 10);
        let mut last = BTreeMap::new();
        last.insert("standard".to_string(), 10);
        let auto_result = AutoPageRunResult::completed_with_pages(
            Vec::new(),
            vec!["standard".to_string()],
            visited,
            last,
        );

        assert!(wait_for_capture_drain(
            &runtime,
            &coordinator,
            &auto_result,
            &runtime.stop,
        )
        .is_none());
    }

    fn write_support_bundle(support_dir: &Path, base: &str) {
        std::fs::write(support_dir.join(format!("{base}.json")), b"json").unwrap();
        std::fs::write(
            support_dir.join(format!("{base}-page-number.png")),
            b"png",
        )
        .unwrap();
    }

    fn automation_record(
        record_key: &str,
        pool_id: &str,
        record_type: &str,
    ) -> AutomationRecordSnapshot {
        AutomationRecordSnapshot {
            record_id: record_key.to_string(),
            record_key: record_key.to_string(),
            pool_id: pool_id.to_string(),
            record_type: record_type.to_string(),
        }
    }

    fn control_context(pool: &str, visited_pages: u32) -> AutoPageControlContext {
        AutoPageControlContext {
            pool: pool.to_string(),
            step: "test".to_string(),
            current_page: visited_pages,
            total_pages: 50,
            visited_pages,
        }
    }

    fn progress_with_rows(rows: Vec<nte_capture::ParsedRow>) -> nte_capture::CaptureProgress {
        nte_capture::CaptureProgress {
            target: CaptureTarget {
                pid: 1,
                exe: "HTGame.exe".to_string(),
                interface: "test".to_string(),
                ports: Vec::new(),
                bpf: String::new(),
            },
            counters: nte_capture::CaptureCounters::default(),
            new_rows: rows.clone(),
            rows_snapshot: rows.clone(),
            row_count: rows.len(),
            warning_count: 0,
        }
    }

    fn parsed_row(pool_id: &str, segment_index: u32) -> nte_capture::ParsedRow {
        nte_capture::ParsedRow {
            record_type: nte_capture::RecordType::Monopoly,
            ticks: 639_175_144_000_000_000,
            time: Some("2026-06-20T00:00:00.000000".to_string()),
            pool_id: Some(pool_id.to_string()),
            item_id: format!("item-{segment_index}"),
            count: 1,
            roll_points: Some(segment_index),
            roll_label_id: None,
            secondary_item_id: None,
            secondary_count: None,
            source: nte_capture::SourceRef {
                session: 0,
                line: 1,
                packet_index: u64::from(segment_index),
                view: "test".to_string(),
                row_index: 0,
                offset: 0,
                stream_key: Some(format!("monopoly:{pool_id}")),
                page_index: Some(segment_index),
                query_high: Some(true),
                segment_index: Some(segment_index),
                generation_index: Some(0),
            },
        }
    }

    fn assert_support_bundle_exists(support_dir: &Path, base: &str) {
        assert!(support_dir.join(format!("{base}.json")).is_file());
        assert!(support_dir
            .join(format!("{base}-page-number.png"))
            .is_file());
    }

    fn assert_support_bundle_missing(support_dir: &Path, base: &str) {
        assert!(!support_dir.join(format!("{base}.json")).exists());
        assert!(!support_dir
            .join(format!("{base}-page-number.png"))
            .exists());
    }

    #[cfg(unix)]
    fn create_dir_symlink(
        target: impl AsRef<Path>,
        link: impl AsRef<Path>,
    ) -> Result<(), std::io::Error> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(windows)]
    fn create_dir_symlink(
        target: impl AsRef<Path>,
        link: impl AsRef<Path>,
    ) -> Result<(), std::io::Error> {
        std::os::windows::fs::symlink_dir(target, link)
    }
}
