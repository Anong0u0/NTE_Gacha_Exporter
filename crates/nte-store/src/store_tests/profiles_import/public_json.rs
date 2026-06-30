#[test]
fn public_json_accepts_v2_only_and_rejects_other_major_versions() {
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
    v2_record["banner_id"] = serde_json::json!("ignored_banner");
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

    for version in ["1.7", "3.0", "4.0", "5.0", "6.0"] {
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
        list.records[0].derived.banner_id.as_deref(),
        Some("monopoly_limited_Nanali")
    );
    assert_eq!(list.records[0].rarity, Some(3));
}
