#[test]
fn restore_backup_rejects_invalid_pool_and_keeps_existing_data() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    store
        .import_public_document(
            "default",
            &public_document(vec![record(
                "keep",
                "CardPool_Character",
                "fork_dustbin",
                "2026-01-01 10:00:00",
            )]),
            "json",
            None,
        )
        .unwrap();
    let before_records =
        std::fs::read_to_string(tmp.path().join("data/profiles/default/records.json")).unwrap();
    let backup_path = tmp.path().join("bad.zip");
    write_backup_zip(
        &backup_path,
        &[
            (
                "settings.json",
                json!({
                    "schema_version": 1,
                    "active_profile": "default",
                    "locale": "zh-Hant",
                    "update_channel": "stable",
                    "check_updates_on_startup": false
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
                    "records": [{
                        "record_id": "bad",
                        "record_type": "monopoly",
                        "time": "2026-01-01 10:00:00",
                        "pool_id": "UnknownPool",
                        "item_id": "fork_dustbin"
                    }]
                })
                .to_string(),
            ),
        ],
    );

    assert!(store.restore_data_backup_report(&backup_path).is_err());
    let after_records =
        std::fs::read_to_string(tmp.path().join("data/profiles/default/records.json")).unwrap();
    assert_eq!(after_records, before_records);
    assert_eq!(
        store.profile_record_ids("default").unwrap(),
        vec!["keep".to_string()]
    );
}

#[test]
fn restore_backup_maps_active_profile_to_existing_profile_casing() {
    let tmp = tempfile::tempdir().unwrap();
    let source = JsonStore::open(tmp.path().join("source")).unwrap();
    source.create_profile("extra").unwrap();
    source
        .update_settings(SettingsPatch {
            active_profile: Some("extra".to_string()),
            ..SettingsPatch::default()
        })
        .unwrap();
    source
        .import_public_document(
            "extra",
            &public_document(vec![record(
                "r1",
                "CardPool_Character",
                "fork_dustbin",
                "2026-01-01 10:00:00",
            )]),
            "json",
            None,
        )
        .unwrap();
    let backup_path = tmp.path().join("case.zip");
    source.create_data_backup_at(&backup_path).unwrap();

    let target = JsonStore::open(tmp.path().join("target")).unwrap();
    target.create_profile("Extra").unwrap();
    target.restore_data_backup_report(&backup_path).unwrap();

    assert_eq!(target.settings().unwrap().active_profile, "Extra");
    assert_eq!(
        target.profile_record_ids("Extra").unwrap(),
        vec!["r1".to_string()]
    );
}

#[test]
fn restore_backup_rejects_unsafe_zip_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let backup_path = tmp.path().join("unsafe.zip");
    write_backup_zip(
        &backup_path,
        &[(
            "../settings.json",
            json!({
                "schema_version": 1,
                "active_profile": "default",
                "locale": "zh-Hant",
                "update_channel": "stable",
                "check_updates_on_startup": false
            })
            .to_string(),
        )],
    );

    assert!(store.restore_data_backup_report(&backup_path).is_err());
}

#[test]
fn full_update_import_creates_backup_and_restores_on_failure() {
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
    let bad = public_document(vec![record(
        "bad",
        "UnknownPool",
        "fork_dustbin",
        "2026-01-01 10:01:00",
    )]);

    assert!(
        store
            .import_public_document_with_backup("default", &bad, "auto_page_full", None)
            .is_err()
    );

    let after =
        std::fs::read_to_string(tmp.path().join("data/profiles/default/records.json")).unwrap();
    let backups = std::fs::read_dir(tmp.path().join("data/backups"))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "zip"))
        .count();

    assert_eq!(after, before);
    assert_eq!(backups, 1);
}

#[test]
fn full_update_import_prunes_generated_backups_after_success() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let old_backup = tmp
        .path()
        .join("data/backups/backup-0000000000-000000000-0.zip");
    std::fs::write(&old_backup, b"old").unwrap();
    let document = public_document(vec![record(
        "r1",
        "CardPool_Character",
        "fork_dustbin",
        "2026-01-01 10:00:00",
    )]);

    store
        .import_public_document_with_backup("default", &document, "auto_page_full", None)
        .unwrap();

    let backups = std::fs::read_dir(tmp.path().join("data/backups"))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "zip"))
        .count();

    assert!(!old_backup.exists());
    assert_eq!(backups, 1);
}
