#[test]
fn store_bootstraps_default_profile_and_files() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();

    let settings = store.settings().unwrap();
    let profiles = store.list_profiles().unwrap();

    assert_eq!(settings.active_profile, "default");
    assert_eq!(settings.ui_locale, "en");
    assert_eq!(settings.update_channel, "stable");
    assert!(!settings.check_updates_on_startup);
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].name, "default");
    assert!(tmp.path().join("data/settings.json").exists());
    assert!(
        tmp.path()
            .join("data/profiles/default/records.json")
            .exists()
    );
}

#[test]
fn store_migrates_missing_ui_locale_from_defaults() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("data")).unwrap();
    std::fs::write(
        tmp.path().join("data/settings.json"),
        json!({
            "schema_version": 1,
            "active_profile": "default",
            "locale": "en",
            "update_channel": "stable",
            "check_updates_on_startup": false
        })
        .to_string(),
    )
    .unwrap();

    let store = JsonStore::open_with_defaults(
        tmp.path(),
        StoreDefaults {
            locale: "en".to_string(),
            ui_locale: "zh-Hant".to_string(),
        },
    )
    .unwrap();

    let settings = store.settings().unwrap();
    let settings_text = std::fs::read_to_string(tmp.path().join("data/settings.json")).unwrap();
    assert_eq!(settings.ui_locale, "zh-Hant");
    assert!(settings_text.contains("\"ui_locale\""));
}

#[test]
fn settings_update_persists_locale_active_profile_and_update_flags() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    store.create_profile("Player_1").unwrap();

    let settings = store
        .update_settings(SettingsPatch {
            active_profile: Some("Player_1".to_string()),
            locale: Some("en".to_string()),
            ui_locale: Some("zh-Hant".to_string()),
            update_channel: Some("beta".to_string()),
            check_updates_on_startup: Some(true),
        })
        .unwrap();

    assert_eq!(settings.active_profile, "Player_1");
    assert_eq!(settings.locale, "en");
    assert_eq!(settings.ui_locale, "zh-Hant");
    assert_eq!(settings.update_channel, "beta");
    assert!(settings.check_updates_on_startup);
    assert!(
        store
            .update_settings(SettingsPatch {
                locale: Some("missing-locale".to_string()),
                ..SettingsPatch::default()
            })
            .is_err()
    );
}

#[test]
fn profile_name_validation_rejects_unsafe_and_duplicate_names() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();

    store.create_profile("Player_1").unwrap();

    assert!(store.create_profile("Player 1").is_err());
    assert!(store.create_profile("../bad").is_err());
    assert!(store.create_profile("player_1").is_err());
    assert!(store.create_profile("CON").is_err());
    assert!(store.create_profile("LPT1").is_err());
}

#[test]
fn profile_rename_updates_files_and_active_settings() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    store.create_profile("Player_1").unwrap();
    store.set_active_profile("Player_1").unwrap();

    let renamed = store.rename_profile("Player_1", "Player_2").unwrap();
    let settings = store.settings().unwrap();
    let profiles = store.list_profiles().unwrap();

    assert_eq!(renamed.name, "Player_2");
    assert!(renamed.active);
    assert_eq!(settings.active_profile, "Player_2");
    assert!(
        !tmp.path()
            .join("data/profiles/Player_1/profile.json")
            .exists()
    );
    assert!(
        tmp.path()
            .join("data/profiles/Player_2/profile.json")
            .exists()
    );
    assert!(profiles.iter().any(|profile| profile.name == "Player_2"));
    assert!(!profiles.iter().any(|profile| profile.name == "Player_1"));
}

#[test]
fn profile_rename_rejects_duplicates_case_insensitively() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    store.create_profile("Player_1").unwrap();
    store.create_profile("Player_2").unwrap();

    assert!(store.rename_profile("Player_1", "player_2").is_err());
    assert!(
        tmp.path()
            .join("data/profiles/Player_1/profile.json")
            .exists()
    );
    assert!(
        tmp.path()
            .join("data/profiles/Player_2/profile.json")
            .exists()
    );
}

#[test]
fn profile_delete_active_switches_to_remaining_profile() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    store.create_profile("Player_1").unwrap();
    store.set_active_profile("Player_1").unwrap();

    let settings = store.delete_profile("Player_1").unwrap();
    let profiles = store.list_profiles().unwrap();

    assert_eq!(settings.active_profile, "default");
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].name, "default");
    assert!(
        !tmp.path()
            .join("data/profiles/Player_1/profile.json")
            .exists()
    );
}

#[test]
fn profile_delete_rejects_last_profile_and_unknown_files() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();

    assert!(store.delete_profile("default").is_err());

    store.create_profile("Player_1").unwrap();
    std::fs::write(tmp.path().join("data/profiles/Player_1/extra.json"), "{}").unwrap();
    assert!(store.delete_profile("Player_1").is_err());
    assert!(
        tmp.path()
            .join("data/profiles/Player_1/profile.json")
            .exists()
    );
    assert!(
        tmp.path()
            .join("data/profiles/Player_1/extra.json")
            .exists()
    );
}

#[test]
fn duplicate_import_is_skipped_and_internal_records_omit_display_fields() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let document = public_document(vec![
        record(
            "r1",
            "CardPool_Character",
            "fork_dustbin",
            "2026-01-01 10:00:00",
        ),
        record(
            "r2",
            "CardPool_Character",
            "Fashion_vehicle_1010_V008",
            "2026-01-01 10:01:00",
        ),
    ]);

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
    assert_eq!(
        store.profile_record_ids("default").unwrap(),
        vec!["r1".to_string(), "r2".to_string()]
    );
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
fn fork_pool_missing_from_maps_rejects_entire_import() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let bad = public_document(vec![record(
        "bad",
        "ForkLottery_NotInMaps",
        "fork_dustbin",
        "2026-01-01 10:00:00",
    )]);

    assert!(
        store
            .import_public_document("default", &bad, "json", None)
            .is_err()
    );
    let stored =
        std::fs::read_to_string(tmp.path().join("data/profiles/default/records.json")).unwrap();
    assert!(!stored.contains("ForkLottery_NotInMaps"));
}

#[test]
fn public_json_accepts_v1_v2_and_rejects_unsupported_major_versions() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let v1_minor = serde_json::json!({
        "info": {
            "schema": "nte-gacha-exporter-export",
            "schema_version": "1.7"
        },
        "nte": {
            "list": [
                record("r1", "CardPool_Character", "fork_dustbin", "2026-01-01 10:00:00")
            ]
        }
    })
    .to_string();
    let mut v2_record = record(
        "r2",
        "CardPool_Character",
        "fork_dustbin",
        "2026-01-01 10:01:00",
    );
    v2_record["pool_kind"] = serde_json::json!("monopoly_limited");
    v2_record["pity_5_before"] = serde_json::json!(99);
    let v2_major = serde_json::json!({
        "info": {
            "schema": "nte-gacha-exporter-export",
            "schema_version": "2.0"
        },
        "nte": {
            "list": [v2_record]
        }
    })
    .to_string();
    let v3_major = serde_json::json!({
        "info": {
            "schema": "nte-gacha-exporter-export",
            "schema_version": "3.0"
        },
        "nte": {
            "list": [
                record("r3", "CardPool_Character", "fork_dustbin", "2026-01-01 10:02:00")
            ]
        }
    })
    .to_string();

    assert!(
        store
            .import_public_document("default", &v1_minor, "json", None)
            .is_ok()
    );
    assert!(
        store
            .import_public_document("default", &v2_major, "json", None)
            .is_ok()
    );
    assert!(
        store
            .import_public_document("default", &v3_major, "json", None)
            .is_err()
    );
}
