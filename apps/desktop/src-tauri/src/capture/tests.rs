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
}
