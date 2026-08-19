#[test]
fn disk_v1_migrates_to_v2_once_with_pre_migration_backup() {
    use std::io::Read;

    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    JsonStore::open(root).unwrap();
    let records_path = root.join("data/profiles/default/records.json");
    std::fs::write(
        &records_path,
        serde_json::to_vec_pretty(&json!({
            "schema_version": 1,
            "records": [
                {
                    "record_id": "standard-id",
                    "source_order": 0,
                    "record_type": "monopoly",
                    "pool_id": "CardPool_NewRole",
                    "item_id": "fork_dustbin"
                },
                {
                    "record_id": "limited-id",
                    "source_order": 1,
                    "record_type": "monopoly",
                    "time": "2026-06-09 05:22:09",
                    "pool_id": "CardPool_Character",
                    "item_id": "fork_dustbin"
                },
                {
                    "record_id": "unresolved-id",
                    "source_order": 2,
                    "record_type": "monopoly",
                    "pool_id": "CardPool_Character",
                    "item_id": "fork_dustbin"
                }
            ]
        }))
        .unwrap(),
    )
    .unwrap();

    JsonStore::open(root).unwrap();

    let migrated: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&records_path).unwrap()).unwrap();
    assert_eq!(migrated["schema_version"], 2);
    let by_id = migrated["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|record| (record["record_id"].as_str().unwrap(), record))
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(by_id["standard-id"]["lottery_id"], "Lottery_Permanent");
    assert_eq!(by_id["limited-id"]["lottery_id"], "Lottery_AnHunQu");
    assert!(by_id["unresolved-id"]["lottery_id"].is_null());

    let backup_paths = std::fs::read_dir(root.join("data/backups"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "zip"))
        .collect::<Vec<_>>();
    assert_eq!(backup_paths.len(), 1);
    let mut zip = zip::ZipArchive::new(std::fs::File::open(&backup_paths[0]).unwrap()).unwrap();
    let mut original_records = String::new();
    zip.by_name("profiles/default/records.json")
        .unwrap()
        .read_to_string(&mut original_records)
        .unwrap();
    let original: serde_json::Value = serde_json::from_str(&original_records).unwrap();
    assert_eq!(original["schema_version"], 1);

    JsonStore::open(root).unwrap();
    let backup_count = std::fs::read_dir(root.join("data/backups"))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|extension| extension == "zip"))
        .count();
    assert_eq!(backup_count, 1);
}

#[test]
fn disk_migration_rejects_unknown_schema_version() {
    let tmp = tempfile::tempdir().unwrap();
    JsonStore::open(tmp.path()).unwrap();
    std::fs::write(
        tmp.path().join("data/profiles/default/records.json"),
        serde_json::to_vec_pretty(&json!({
            "schema_version": 3,
            "records": []
        }))
        .unwrap(),
    )
    .unwrap();

    let error = JsonStore::open(tmp.path()).err().unwrap();

    assert!(error.to_string().contains("unsupported records schema_version: 3"));
}

#[test]
fn v1_backup_restore_writes_v2_records() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let backup_path = tmp.path().join("legacy.zip");
    let mut legacy_record = json!({
        "record_id": "placeholder",
        "source_order": 0,
        "record_type": "monopoly",
        "time": "2026-07-09 00:00:00",
        "pool_id": "CardPool_Character",
        "item_id": "fork_dustbin"
    });
    let expected_id = expected_record_id(&legacy_record);
    legacy_record["record_id"] = json!(expected_id);
    write_backup_zip(
        &backup_path,
        &[
            (
                "settings.json",
                json!({
                    "schema_version": 1,
                    "active_profile": "default",
                    "locale": "zh-Hant",
                    "ui_locale": "zh-Hant",
                    "update_channel": "stable",
                    "check_updates_on_startup": true
                })
                .to_string(),
            ),
            (
                "profiles/default/profile.json",
                json!({
                    "schema_version": 1,
                    "name": "default",
                    "created_at": "1",
                    "updated_at": "1"
                })
                .to_string(),
            ),
            (
                "profiles/default/records.json",
                json!({
                    "schema_version": 1,
                    "records": [legacy_record]
                })
                .to_string(),
            ),
        ],
    );

    store.restore_data_backup_report(&backup_path).unwrap();

    let restored: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(tmp.path().join("data/profiles/default/records.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(restored["schema_version"], 2);
    assert_eq!(restored["records"][0]["record_id"], expected_id);
    assert_eq!(restored["records"][0]["lottery_id"], "Lottery_ZhenHong");
}
