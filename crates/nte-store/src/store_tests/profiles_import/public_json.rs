#[test]
fn public_json_accepts_exact_supported_versions() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let mut v2_record = record(
        "r2",
        "CardPool_Character",
        "fork_dustbin",
        "2026-01-01 10:01:00",
    );
    v2_record["pool_kind"] = serde_json::json!("monopoly_limited");
    v2_record["pity_5_before"] = serde_json::json!(99);
    v2_record["banner_id"] = serde_json::json!("monopoly_limited_Nanali");
    v2_record["rarity"] = serde_json::json!(1);
    v2_record["source_order"] = serde_json::json!(0);
    let v2_major = serde_json::json!({
        "info": {
            "schema": nte_core::PUBLIC_JSON_SCHEMA,
            "schema_version": "2.0"
        },
        "nte": {
            "list": [v2_record]
        }
    })
    .to_string();

    assert!(
        store
            .import_public_document("default", &v2_major, "json", None)
            .is_ok()
    );

    for version in ["1.7", "2.2", "3.0", "4.0", "5.0", "6.0"] {
        let document = serde_json::json!({
            "info": {
                "schema": nte_core::PUBLIC_JSON_SCHEMA,
                "schema_version": version
            },
            "nte": {
                "list": [
                    record(
                        &format!("r-{version}"),
                        "CardPool_Character",
                        "fork_dustbin",
                        "2026-01-01 10:05:00"
                    )
                ]
            }
        })
        .to_string();
        assert!(
            store
                .import_public_document("default", &document, "json", None)
                .is_err(),
            "{version} should be rejected"
        );
    }

    let list = store
        .list_records("default", "zh-Hant", &RecordFilter::default())
        .unwrap();
    assert_eq!(list.records.len(), 1);
    assert_eq!(list.records[0].record_id, expected_record_id(&v2_record));
    assert_eq!(
        list.records[0].derived.banner_source_id.as_deref(),
        Some("Lottery_Nanali")
    );
    assert_eq!(list.records[0].rarity, Some(3));
}

#[test]
fn public_json_v20_prefers_explicit_banner_id_over_time_fallback() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let mut value = record(
        "conflict",
        "CardPool_Character",
        "fork_dustbin",
        "2026-01-01 10:01:00",
    );
    value["banner_id"] = serde_json::json!("monopoly_limited_Xun");

    store
        .import_public_document("default", &public_document(vec![value]), "json", None)
        .unwrap();

    let records = store
        .list_records("default", "zh-Hant", &RecordFilter::default())
        .unwrap();
    assert_eq!(records.records.len(), 1);
    assert_eq!(records.records[0].lottery_id.as_deref(), Some("Lottery_Xun"));
}

#[test]
fn public_json_v21_requires_valid_monopoly_lottery_id() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let valid = record_with_lottery_id(
        "valid",
        "CardPool_Character",
        "Lottery_Canhong",
        "1036",
        None,
    );
    let fork = record_with_options(
        "fork",
        "ForkLottery_AnHunQu",
        "fork_dustbin",
        None,
        None,
    );

    let report = store
        .import_public_document(
            "default",
            &public_document_v21(vec![valid, fork]),
            "json",
            None,
        )
        .unwrap();
    assert_eq!(report.records_inserted, 2);

    let missing = record_with_options(
        "missing",
        "CardPool_Character",
        "fork_dustbin",
        None,
        Some(1),
    );
    let missing_error = store
        .import_public_document(
            "default",
            &public_document_v21(vec![missing]),
            "json",
            None,
        )
        .unwrap_err();
    assert!(missing_error.to_string().contains("record missing string field: lottery_id"));

    let invalid = record_with_lottery_id(
        "invalid",
        "CardPool_Character",
        "Lottery_",
        "fork_dustbin",
        None,
    );
    let invalid_error = store
        .import_public_document(
            "default",
            &public_document_v21(vec![invalid]),
            "json",
            None,
        )
        .unwrap_err();
    assert!(invalid_error.to_string().contains("invalid lottery_id: Lottery_"));

    let mut spoofed_fork = record_with_options(
        "spoofed",
        "CardPool_Character",
        "fork_dustbin",
        None,
        Some(1),
    );
    spoofed_fork["record_type"] = serde_json::json!("fork");
    let spoofed_error = store
        .import_public_document(
            "default",
            &public_document_v21(vec![spoofed_fork]),
            "json",
            None,
        )
        .unwrap_err();
    assert!(
        spoofed_error
            .to_string()
            .contains("record_type fork does not match pool_id CardPool_Character")
    );
}
