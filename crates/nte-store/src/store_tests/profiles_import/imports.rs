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
fn capture_unknown_and_mismatched_lottery_ids_survive_import_and_export() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let rows = vec![
        captured_monopoly_row(0, "2026-08-17T10:00:00.000000", "Lottery_Future"),
        captured_monopoly_row(1, "2026-08-17T10:01:00.000000", "Lottery_Permanent"),
    ];
    let document = nte_capture::build_capture_document(&rows, "zh-Hant").unwrap();

    assert_eq!(
        document["info"]["schema_version"],
        nte_core::PUBLIC_JSON_SCHEMA_VERSION
    );
    let report = store
        .import_public_document("default", &document.to_string(), "capture", None)
        .unwrap();
    assert_eq!(report.records_inserted, 2);

    let records = store
        .list_records("default", "zh-Hant", &RecordFilter::default())
        .unwrap();
    let future = records
        .records
        .iter()
        .find(|record| record.lottery_id.as_deref() == Some("Lottery_Future"))
        .unwrap();
    assert_eq!(
        future.banner_source.resolution_issue,
        Some(nte_core::BannerResolutionIssue::UnknownLotteryId)
    );
    let mismatch = records
        .records
        .iter()
        .find(|record| record.lottery_id.as_deref() == Some("Lottery_Permanent"))
        .unwrap();
    assert_eq!(
        mismatch.banner_source.resolution_issue,
        Some(nte_core::BannerResolutionIssue::LotteryPoolMismatch)
    );

    let export_path = tmp.path().join("capture-export.json");
    store
        .export_public_json("default", "zh-Hant", &export_path)
        .unwrap();
    let exported: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(export_path).unwrap()).unwrap();
    let exported_records = exported["nte"]["list"].as_array().unwrap();
    for lottery_id in ["Lottery_Future", "Lottery_Permanent"] {
        let record = exported_records
            .iter()
            .find(|record| record["lottery_id"] == lottery_id)
            .unwrap();
        assert!(record.get("banner_id").is_none());
    }
}

fn captured_monopoly_row(
    row_index: u32,
    time: &str,
    lottery_id: &str,
) -> nte_capture::ParsedRow {
    nte_capture::ParsedRow {
        record_type: nte_capture::RecordType::Monopoly,
        ticks: 0,
        time: Some(time.to_string()),
        pool_id: Some("CardPool_Character".to_string()),
        lottery_id: Some(lottery_id.to_string()),
        item_id: "fork_dustbin".to_string(),
        count: 1,
        roll_points: Some(1),
        roll_label_id: None,
        secondary_item_id: None,
        secondary_count: None,
        source: nte_capture::SourceRef {
            session: 0,
            line: 0,
            packet_index: 0,
            view: "test".to_string(),
            row_index,
            offset: 0,
            stream_key: Some("monopoly".to_string()),
            page_index: Some(0),
            query_high: Some(true),
            segment_index: Some(0),
            generation_index: Some(0),
        },
    }
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
        list.records[0].banner_source.resolution_issue,
        Some(nte_core::BannerResolutionIssue::UnknownPool)
    );
    assert_eq!(
        list.records[0].derived.banner_source_id.as_deref(),
        Some("ForkLottery_NotInMaps")
    );
    assert_eq!(list.records[0].banner_source.title.as_deref(), Some("NotInMaps"));
}

#[test]
fn duplicate_merge_enriches_and_preserves_lottery_id() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let incoming = record_with_lottery_id(
        "incoming",
        "CardPool_Character",
        "Lottery_Canhong",
        "fork_dustbin",
        None,
    );
    let expected_id = expected_record_id(&incoming);
    let mut unresolved = incoming.clone();
    unresolved.as_object_mut().unwrap().remove("lottery_id");
    unresolved["record_id"] = json!(expected_id);
    unresolved["source_order"] = json!(0);
    std::fs::write(
        tmp.path().join("data/profiles/default/records.json"),
        serde_json::to_vec_pretty(&json!({
            "schema_version": 2,
            "records": [unresolved.clone()]
        }))
        .unwrap(),
    )
    .unwrap();

    let enriched = store
        .import_public_document(
            "default",
            &public_document_v21(vec![incoming.clone()]),
            "json",
            None,
        )
        .unwrap();
    assert_eq!(enriched.records_inserted, 0);
    assert_eq!(enriched.records_skipped, 1);

    let legacy_without_id = store
        .import_public_document(
            "default",
            &public_document(vec![unresolved]),
            "json",
            None,
        )
        .unwrap();
    assert_eq!(legacy_without_id.records_inserted, 0);
    assert_eq!(legacy_without_id.records_skipped, 1);

    let same_id = store
        .import_public_document(
            "default",
            &public_document_v21(vec![incoming]),
            "json",
            None,
        )
        .unwrap();
    assert_eq!(same_id.records_inserted, 0);
    assert_eq!(same_id.records_skipped, 1);

    let stored: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(tmp.path().join("data/profiles/default/records.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(stored["records"].as_array().unwrap().len(), 1);
    assert_eq!(stored["records"][0]["record_id"], expected_id);
    assert_eq!(stored["records"][0]["lottery_id"], "Lottery_Canhong");
}

#[test]
fn lottery_id_conflict_rejects_entire_import_without_enrichment_write() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let enrich_incoming = record_with_lottery_id(
        "enrich",
        "CardPool_Character",
        "Lottery_Canhong",
        "fork_dustbin",
        None,
    );
    let conflict_existing = record_with_lottery_id(
        "conflict-existing",
        "CardPool_Character",
        "Lottery_Canhong",
        "1036",
        None,
    );
    let mut unresolved_existing = enrich_incoming.clone();
    unresolved_existing
        .as_object_mut()
        .unwrap()
        .remove("lottery_id");
    unresolved_existing["record_id"] = json!(expected_record_id(&enrich_incoming));
    unresolved_existing["source_order"] = json!(0);
    let mut conflict_existing = conflict_existing;
    conflict_existing["record_id"] = json!(expected_record_id(&conflict_existing));
    conflict_existing["source_order"] = json!(1);
    let records_path = tmp.path().join("data/profiles/default/records.json");
    std::fs::write(
        &records_path,
        serde_json::to_vec_pretty(&json!({
            "schema_version": 2,
            "records": [unresolved_existing, conflict_existing]
        }))
        .unwrap(),
    )
    .unwrap();
    let before = std::fs::read_to_string(&records_path).unwrap();
    let conflict_incoming = record_with_lottery_id(
        "conflict-incoming",
        "CardPool_Character",
        "Lottery_Nanali",
        "1036",
        None,
    );

    let error = store
        .import_public_document(
            "default",
            &public_document_v21(vec![enrich_incoming, conflict_incoming]),
            "json",
            None,
        )
        .unwrap_err();

    assert!(error.to_string().contains("lottery_id conflict"));
    assert_eq!(std::fs::read_to_string(&records_path).unwrap(), before);
}

#[test]
fn duplicate_merge_matches_occurrences_by_source_order() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let mut first = record_with_lottery_id(
        "first",
        "CardPool_Character",
        "Lottery_Canhong",
        "fork_dustbin",
        None,
    );
    first["source_order"] = json!(0);
    let mut second = record_with_lottery_id(
        "second",
        "CardPool_Character",
        "Lottery_Nanali",
        "fork_dustbin",
        None,
    );
    second["source_order"] = json!(1);
    let key_record = first.clone();
    first["record_id"] = json!(expected_record_id_with_occurrence(&key_record, 0));
    second["record_id"] = json!(expected_record_id_with_occurrence(&key_record, 1));
    std::fs::write(
        tmp.path().join("data/profiles/default/records.json"),
        serde_json::to_vec_pretty(&json!({
            "schema_version": 2,
            "records": [first.clone(), second.clone()]
        }))
        .unwrap(),
    )
    .unwrap();

    let report = store
        .import_public_document(
            "default",
            &public_document_v21(vec![second, first]),
            "json",
            None,
        )
        .unwrap();

    assert_eq!(report.records_inserted, 0);
    assert_eq!(report.records_skipped, 2);
}
