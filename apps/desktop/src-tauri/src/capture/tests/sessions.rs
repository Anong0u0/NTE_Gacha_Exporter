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
fn prune_capture_session_maps_treats_cancelled_as_terminal() {
    let mut sessions = HashMap::from([(
        "cancelled".to_string(),
        test_session(test_status("cancelled", crate::lifecycle::STATE_CANCELLED, 1.0)),
    )]);
    let mut captures = HashMap::from([("cancelled".to_string(), test_meta())]);

    prune_capture_session_maps(&mut sessions, &mut captures, "other", 2_000.0);

    assert!(!sessions.contains_key("cancelled"));
    assert!(!captures.contains_key("cancelled"));
}
