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
        assert!(error.support_path.unwrap().contains("data/support"));
        assert!(error.support_image_path.is_none());
    }
}
