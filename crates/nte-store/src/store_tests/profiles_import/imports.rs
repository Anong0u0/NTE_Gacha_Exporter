#[test]
fn duplicate_import_is_skipped_and_internal_records_omit_display_fields() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let r1 = record(
        "r1",
        "CardPool_Character",
        "fork_dustbin",
        "2026-01-01 10:00:00",
    );
    let r2 = record(
        "r2",
        "CardPool_Character",
        "Fashion_vehicle_1010_V008",
        "2026-01-01 10:01:00",
    );
    let expected_ids = expected_record_ids(&[r1.clone(), r2.clone()]);
    let document = public_document(vec![r1, r2]);

    let first = store
        .import_public_document("default", &document, "json", Some("sample.json"))
        .unwrap();
    let second = store
        .import_public_document("default", &document, "json", Some("sample.json"))
        .unwrap();
    let stored =
        std::fs::read_to_string(tmp.path().join("data/profiles/default/records.json")).unwrap();

    assert_eq!(first.records_inserted, 2);
    assert_eq!(first.records_skipped, 0);
    assert_eq!(second.records_inserted, 0);
    assert_eq!(second.records_skipped, 2);
    assert!(!stored.contains("display must be ignored"));
    assert!(!stored.contains("pool_name"));
    assert!(!stored.contains("item_name"));
    assert!(
        tmp.path()
            .join("data/profiles/default/last-run.json")
            .exists()
    );
    assert_eq!(store.profile_record_ids("default").unwrap(), expected_ids);
}

#[test]
fn duplicate_import_uses_semantic_multiset_counts() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let duplicate = record(
        "old-a",
        "CardPool_Character",
        "fork_dustbin",
        "2026-01-01 10:00:00",
    );
    let old_document = public_document(vec![
        duplicate.clone(),
        record(
            "old-b",
            "CardPool_Character",
            "fork_dustbin",
            "2026-01-01 10:00:00",
        ),
    ]);
    let incoming_document = public_document(vec![
        record(
            "new-a",
            "CardPool_Character",
            "fork_dustbin",
            "2026-01-01 10:00:00",
        ),
        record(
            "new-b",
            "CardPool_Character",
            "fork_dustbin",
            "2026-01-01 10:00:00",
        ),
        record(
            "new-c",
            "CardPool_Character",
            "fork_dustbin",
            "2026-01-01 10:00:00",
        ),
    ]);

    let first = store
        .import_public_document("default", &old_document, "json", None)
        .unwrap();
    let second = store
        .import_public_document("default", &incoming_document, "json", None)
        .unwrap();

    assert_eq!(first.records_inserted, 2);
    assert_eq!(first.records_skipped, 0);
    assert_eq!(second.records_inserted, 1);
    assert_eq!(second.records_skipped, 2);
    assert_eq!(
        store.profile_record_ids("default").unwrap(),
        vec![
            expected_record_id(&duplicate),
            expected_record_id_with_occurrence(&duplicate, 1),
            expected_record_id_with_occurrence(&duplicate, 2),
        ]
    );
}

#[test]
fn import_canonicalizes_case_folded_item_ids_before_duplicate_merge() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let time = "2026-05-10 18:15:39";
    let old = record_with_options(
        "old",
        "ForkLottery_Nanali",
        "fork_Wushoutieyu",
        Some(time),
        None,
    );
    let canonical = record_with_options(
        "new",
        "ForkLottery_Nanali",
        "fork_wushoutieyu",
        Some(time),
        None,
    );
    let expected_id = expected_record_id(&canonical);

    let first = store
        .import_public_document("default", &public_document(vec![old]), "json", None)
        .unwrap();
    let second = store
        .import_public_document(
            "default",
            &public_document(vec![canonical]),
            "json",
            None,
        )
        .unwrap();
    let stored =
        std::fs::read_to_string(tmp.path().join("data/profiles/default/records.json")).unwrap();
    let list = store
        .list_records("default", "zh-Hant", &RecordFilter::default())
        .unwrap();

    assert_eq!(first.records_inserted, 1);
    assert_eq!(first.records_skipped, 0);
    assert_eq!(second.records_inserted, 0);
    assert_eq!(second.records_skipped, 1);
    assert_eq!(store.profile_record_ids("default").unwrap(), vec![expected_id]);
    assert!(stored.contains("fork_wushoutieyu"));
    assert!(!stored.contains("fork_Wushoutieyu"));
    assert_eq!(list.records[0].item_id, "fork_wushoutieyu");
    assert_eq!(list.records[0].item_name, "弧盤·焰魂狂飆");
    assert_eq!(list.records[0].rarity, Some(5));
}

#[test]
fn agent_smoke_raw_replay_clean_profile_imports_only_four_incremental_records_when_available() {
    let run1 = std::path::Path::new(
        "target/agent-smoke/app-current/data/runs/raw-1781891304-042719100-1.jsonl",
    );
    let run2 = std::path::Path::new(
        "target/agent-smoke/app-current/data/runs/raw-1782553392-632835900-0.jsonl",
    );
    if !run1.exists() || !run2.exists() {
        eprintln!("agent-smoke raw captures unavailable; skipping local replay regression");
        return;
    }

    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let run1_document = capture_document_text(run1);
    let run2_document = capture_document_text(run2);

    let first = store
        .import_public_document("default", &run1_document, "raw_replay", Some("run1"))
        .unwrap();
    let second = store
        .import_public_document("default", &run2_document, "raw_replay", Some("run2"))
        .unwrap();

    assert_eq!(first.records_inserted, 646);
    assert_eq!(first.records_skipped, 0);
    assert_eq!(second.records_inserted, 4);
    assert_eq!(second.records_skipped, 266);
    assert_eq!(store.profile_record_ids("default").unwrap().len(), 650);
}

fn capture_document_text(path: &std::path::Path) -> String {
    let raw = nte_capture::read_raw_capture(path).unwrap();
    let document = nte_capture::build_capture_document(&raw.rows, "zh-Hant").unwrap();
    serde_json::to_string(&document).unwrap()
}

#[test]
fn unknown_pool_id_rejects_entire_import_without_partial_write() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let original = public_document(vec![record(
        "r1",
        "CardPool_Character",
        "fork_dustbin",
        "2026-01-01 10:00:00",
    )]);
    store
        .import_public_document("default", &original, "json", None)
        .unwrap();

    let before =
        std::fs::read_to_string(tmp.path().join("data/profiles/default/records.json")).unwrap();
    let bad = public_document(vec![
        record(
            "r2",
            "CardPool_Character",
            "Fashion_vehicle_1010_V008",
            "2026-01-01 10:01:00",
        ),
        record("bad", "UnknownPool", "fork_dustbin", "2026-01-01 10:02:00"),
    ]);

    assert!(
        store
            .import_public_document("default", &bad, "json", None)
            .is_err()
    );
    let after =
        std::fs::read_to_string(tmp.path().join("data/profiles/default/records.json")).unwrap();
    assert_eq!(after, before);
}

#[test]
fn fork_pool_missing_from_maps_imports_as_synthetic_banner() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let document = public_document(vec![record(
        "synthetic-fork",
        "ForkLottery_NotInMaps",
        "fork_dustbin",
        "2026-01-01 10:00:00",
    )]);

    store
        .import_public_document("default", &document, "json", None)
        .unwrap();

    let list = store
        .list_records("default", "zh-Hant", &RecordFilter::default())
        .unwrap();
    assert_eq!(list.total, 1);
    assert_eq!(
        list.records[0].banner.resolution_issue,
        Some(nte_core::BannerResolutionIssue::UnknownPool)
    );
    assert_eq!(
        list.records[0].derived.banner_id.as_deref(),
        Some("ForkLottery_NotInMaps")
    );
    assert_eq!(list.records[0].banner.title.as_deref(), Some("NotInMaps"));
}
