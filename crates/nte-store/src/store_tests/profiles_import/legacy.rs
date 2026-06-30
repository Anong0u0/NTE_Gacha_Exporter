#[test]
fn legacy_composite_record_ids_are_normalized_to_current_hash_ids() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let expected_monopoly = json!({
        "record_id": "expected",
        "record_type": "monopoly",
        "time": "2026-06-09 05:22:09",
        "pool_id": "CardPool_Character",
        "item_id": "fork_yuren",
        "count": 1,
        "roll_points": 0
    });
    let expected_fork = json!({
        "record_id": "expected",
        "record_type": "fork",
        "time": "2026-06-03 17:15:58",
        "pool_id": "ForkLottery_AnHunQu",
        "item_id": "fork_Rose",
        "count": 1
    });
    let expected_monopoly_id = expected_record_id(&expected_monopoly);
    let expected_fork_id = expected_record_id(&expected_fork);
    let legacy = public_document(vec![
        json!({
            "record_id": "monopoly:639165793295740000:CardPool_Character:0:0:fork_yuren:1:fork_yuren:1",
            "record_type": "monopoly",
            "time": "2026-06-09 05:22:09",
            "pool_id": "CardPool_Character",
            "item_id": "fork_yuren",
            "count": 1,
            "roll_points": 0,
            "roll_label": "display must be ignored"
        }),
        json!({
            "record_id": "fork:639161037582960000:ForkLottery_AnHunQu:0::fork_Rose:1::",
            "record_type": "fork",
            "time": "2026-06-03 17:15:58",
            "pool_id": "ForkLottery_AnHunQu",
            "item_id": "fork_Rose",
            "count": 1
        }),
    ]);

    let first = store
        .import_public_document("default", &legacy, "json", None)
        .unwrap();
    let equivalent_current = public_document(vec![
        json!({
            "record_id": expected_monopoly_id,
            "source_order": 0,
            "record_type": "monopoly",
            "time": "2026-06-09 05:22:09",
            "pool_id": "CardPool_Character",
            "item_id": "fork_yuren",
            "count": 1,
            "roll_label_id": "BPUI_LotteryResult_jidianzengli",
            "pool_name": "display must be ignored",
            "item_name": "display must be ignored",
            "roll_label": "display must be ignored"
        }),
        json!({
            "record_id": expected_fork_id,
            "source_order": 1,
            "record_type": "fork",
            "time": "2026-06-03 17:15:58",
            "pool_id": "ForkLottery_AnHunQu",
            "item_id": "fork_Rose",
            "count": 1,
            "pool_name": "display must be ignored",
            "item_name": "display must be ignored"
        }),
    ]);
    let second = store
        .import_public_document("default", &equivalent_current, "json", None)
        .unwrap();

    assert_eq!(first.records_inserted, 2);
    assert_eq!(second.records_inserted, 0);
    assert_eq!(second.records_skipped, 2);
    assert_eq!(
        store.profile_record_ids("default").unwrap(),
        vec![expected_fork_id, expected_monopoly_id]
    );
}

#[test]
fn public_json_requires_source_order() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let document = serde_json::json!({
        "info": {
            "schema": nte_core::PUBLIC_JSON_SCHEMA,
            "schema_version": "2.0"
        },
        "nte": {
            "list": [{
                "record_id": "missing-source-order",
                "record_type": "fork",
                "time": "2026-01-01 10:00:00",
                "pool_id": "ForkLottery_AnHunQu",
                "item_id": "fork_dustbin",
                "count": 1
            }]
        }
    })
    .to_string();

    let error = store
        .import_public_document("default", &document, "json", None)
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("record missing u64 field: source_order")
    );
}
