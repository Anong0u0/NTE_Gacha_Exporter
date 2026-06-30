#[test]
fn store_bootstraps_default_profile_and_files() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();

    let settings = store.settings().unwrap();
    let profiles = store.list_profiles().unwrap();

    assert_eq!(settings.active_profile, "default");
    assert_eq!(settings.ui_locale, "en");
    assert_eq!(settings.update_channel, "stable");
    assert!(settings.check_updates_on_startup);
    assert_eq!(settings.skipped_update_version, None);
    assert!(settings.capture_auto_page_enabled);
    assert!(!settings.capture_full_update_enabled);
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
fn store_migrates_missing_check_updates_on_startup_to_enabled() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("data")).unwrap();
    std::fs::write(
        tmp.path().join("data/settings.json"),
        json!({
            "schema_version": 1,
            "active_profile": "default",
            "locale": "en",
            "ui_locale": "en",
            "update_channel": "stable"
        })
        .to_string(),
    )
    .unwrap();

    let store = JsonStore::open(tmp.path()).unwrap();

    let settings = store.settings().unwrap();
    let settings_text = std::fs::read_to_string(tmp.path().join("data/settings.json")).unwrap();
    assert!(settings.check_updates_on_startup);
    assert!(settings_text.contains("\"check_updates_on_startup\": true"));
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
    assert!(settings.capture_auto_page_enabled);
    assert!(!settings.capture_full_update_enabled);
    assert!(settings_text.contains("\"ui_locale\""));
    assert!(settings_text.contains("\"capture_auto_page_enabled\""));
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
            skipped_update_version: Some("1.2.3".to_string()),
            capture_auto_page_enabled: Some(true),
            capture_full_update_enabled: Some(true),
        })
        .unwrap();

    assert_eq!(settings.active_profile, "Player_1");
    assert_eq!(settings.locale, "en");
    assert_eq!(settings.ui_locale, "zh-Hant");
    assert_eq!(settings.update_channel, "beta");
    assert!(settings.check_updates_on_startup);
    assert_eq!(settings.skipped_update_version.as_deref(), Some("1.2.3"));
    assert!(settings.capture_auto_page_enabled);
    assert!(settings.capture_full_update_enabled);
    let settings = store
        .update_settings(SettingsPatch {
            capture_auto_page_enabled: Some(false),
            ..SettingsPatch::default()
        })
        .unwrap();
    assert!(!settings.capture_auto_page_enabled);
    assert!(!settings.capture_full_update_enabled);
    assert!(
        store
            .update_settings(SettingsPatch {
                locale: Some("missing-locale".to_string()),
                ..SettingsPatch::default()
            })
            .is_err()
    );
    assert!(
        store
            .update_settings(SettingsPatch {
                ui_locale: Some("missing-ui-locale".to_string()),
                ..SettingsPatch::default()
            })
            .is_err()
    );
}

#[test]
fn store_normalizes_saved_unsupported_ui_locale() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("data/profiles/default")).unwrap();
    std::fs::write(
        tmp.path().join("data/settings.json"),
        json!({
            "schema_version": 1,
            "active_profile": "default",
            "locale": "en",
            "ui_locale": "ja",
            "update_channel": "stable",
            "check_updates_on_startup": false
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(
        tmp.path().join("data/profiles/default/profile.json"),
        json!({
            "schema_version": 1,
            "name": "default",
            "created_at": "1",
            "updated_at": "1"
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(
        tmp.path().join("data/profiles/default/records.json"),
        json!({
            "schema_version": 1,
            "records": []
        })
        .to_string(),
    )
    .unwrap();

    let store = JsonStore::open(tmp.path()).unwrap();
    let settings_text = std::fs::read_to_string(tmp.path().join("data/settings.json")).unwrap();
    let settings_json: serde_json::Value = serde_json::from_str(&settings_text).unwrap();
    assert_eq!(store.settings().unwrap().ui_locale, "en");
    assert_eq!(settings_json["ui_locale"], "en");
}
