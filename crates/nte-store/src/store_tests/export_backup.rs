#[test]
fn export_public_json_and_csv_from_store() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let document = public_document(vec![record(
        "c1",
        "CardPool_Character",
        "Fashion_vehicle_1010_V008",
        "2026-05-13 05:59:00",
    )]);
    store
        .import_public_document("default", &document, "json", None)
        .unwrap();

    let json_path = tmp.path().join("exports/history.json");
    let csv_path = tmp.path().join("exports/history.csv");
    store
        .export_public_json("default", "zh-Hant", &json_path)
        .unwrap();
    store.export_csv("default", "zh-Hant", &csv_path).unwrap();

    let exported_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(json_path).unwrap()).unwrap();
    let first = &exported_json["nte"]["list"][0];
    assert_eq!(exported_json["info"]["schema_version"], "2.0");
    assert_eq!(first["record_id"], "c1");
    assert_eq!(first["rarity"], 5);
    assert_eq!(first["banner_resolution_status"], "matched");
    assert_eq!(first["pool_kind"], "monopoly_limited");
    assert_eq!(first["banner_id"], "monopoly_limited_Nanali");
    assert_eq!(first["banner_name"], "王牌一代目");
    assert_eq!(first["banner_type"], "limited");
    assert_eq!(first["banner_phase"], "limited_2026_05_13");
    assert_eq!(first["banner_source_confidence"], "curated");
    assert_eq!(first["pull_no_in_pool_kind"], 1);
    assert_eq!(first["pull_no_in_banner"], 1);
    assert_eq!(first["pity_5_before"], 0);
    assert_eq!(first["pity_5_after"], 0);
    assert_eq!(first["pity_4_before"], 0);
    assert_eq!(first["pity_4_after"], 1);
    assert_eq!(first["hit_rarity"], 5);
    assert_eq!(first["rate_up_result"], "not_applicable");
    assert_eq!(first["result_confidence"], "curated");
    assert_eq!(first["rule_id"], "monopoly_limited");
    assert_eq!(first["rule_resolution_status"], "matched");
    assert_eq!(first["rule_source_confidence"], "curated");
    assert!(first.get("derived").is_none());

    let csv = std::fs::read_to_string(csv_path).unwrap();
    assert!(csv.contains("獲得時間"));
    assert!(csv.contains("改裝件·萌虎來襲-塗裝"));
    assert!(csv.contains("banner_id"));
    assert!(csv.contains("pull_no"));
    assert!(csv.contains("pity_5_before"));
    assert!(csv.contains("rate_up_result"));
    assert!(csv.contains("monopoly_limited_Nanali"));
    assert!(csv.contains(",not_applicable,"));
}

#[test]
fn data_backup_zip_contains_manifest_and_profile_files() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let document = public_document(vec![record(
        "c1",
        "CardPool_Character",
        "Fashion_vehicle_1010_V008",
        "2026-01-01 10:00:00",
    )]);
    store
        .import_public_document("default", &document, "json", None)
        .unwrap();

    let backup = store.create_data_backup().unwrap();
    let file = std::fs::File::open(&backup.path).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();
    let names = (0..zip.len())
        .map(|index| zip.by_index(index).unwrap().name().to_string())
        .collect::<Vec<_>>();

    assert!(backup.path.starts_with(tmp.path().join("data/backups")));
    assert!(names.contains(&"manifest.json".to_string()));
    assert!(names.contains(&"settings.json".to_string()));
    assert!(names.contains(&"profiles/default/profile.json".to_string()));
    assert!(names.contains(&"profiles/default/records.json".to_string()));
    assert!(names.contains(&"profiles/default/last-run.json".to_string()));
}

#[test]
fn generated_run_and_backup_paths_are_unique() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();

    assert_ne!(store.default_run_raw_path(), store.default_run_raw_path());

    let first = store.create_data_backup().unwrap();
    let second = store.create_data_backup().unwrap();

    assert_ne!(first.path, second.path);
    assert!(first.path.exists());
    assert!(second.path.exists());
}

#[test]
fn backup_can_write_selected_path_and_report_counts() {
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
            "ForkLottery_AnHunQu",
            "fork_Rose",
            "2026-01-02 10:00:00",
        ),
    ]);
    store
        .import_public_document("default", &document, "json", None)
        .unwrap();
    let path = tmp.path().join("selected-backup.zip");

    let report = store.create_data_backup_report(Some(&path)).unwrap();

    assert_eq!(report.path, path.to_string_lossy());
    assert_eq!(report.profile_count, 1);
    assert_eq!(report.record_count, 2);
    assert!(path.exists());
}

#[test]
fn restore_backup_merges_existing_profile_creates_new_profile_and_overwrites_settings() {
    let tmp = tempfile::tempdir().unwrap();
    let source = JsonStore::open(tmp.path().join("source")).unwrap();
    source.create_profile("Extra").unwrap();
    source
        .update_settings(SettingsPatch {
            active_profile: Some("Extra".to_string()),
            locale: Some("en".to_string()),
            ui_locale: Some("zh-Hant".to_string()),
            update_channel: Some("beta".to_string()),
            check_updates_on_startup: Some(true),
        })
        .unwrap();
    let default_doc = public_document(vec![
        record(
            "same",
            "CardPool_Character",
            "fork_dustbin",
            "2026-01-01 10:00:00",
        ),
        record(
            "new",
            "CardPool_Character",
            "Fashion_vehicle_1010_V008",
            "2026-01-01 10:01:00",
        ),
    ]);
    source
        .import_public_document("default", &default_doc, "json", None)
        .unwrap();
    let extra_doc = public_document(vec![record(
        "extra",
        "ForkLottery_AnHunQu",
        "fork_Rose",
        "2026-01-02 10:00:00",
    )]);
    source
        .import_public_document("Extra", &extra_doc, "json", None)
        .unwrap();
    let backup_path = tmp.path().join("snapshot.zip");
    source.create_data_backup_at(&backup_path).unwrap();

    let target = JsonStore::open(tmp.path().join("target")).unwrap();
    target
        .import_public_document(
            "default",
            &public_document(vec![record(
                "same",
                "CardPool_Character",
                "fork_dustbin",
                "2026-01-01 10:00:00",
            )]),
            "json",
            None,
        )
        .unwrap();

    let report = target.restore_data_backup_report(&backup_path).unwrap();
    let default_ids = target.profile_record_ids("default").unwrap();
    let extra_ids = target.profile_record_ids("Extra").unwrap();
    let settings = target.settings().unwrap();

    assert_eq!(report.profiles_seen, 2);
    assert_eq!(report.profiles_created, 1);
    assert_eq!(report.profiles_merged, 1);
    assert_eq!(report.records_seen, 3);
    assert_eq!(report.records_inserted, 2);
    assert_eq!(report.records_skipped, 1);
    assert!(report.settings_restored);
    assert_eq!(default_ids, vec!["same".to_string(), "new".to_string()]);
    assert_eq!(extra_ids, vec!["extra".to_string()]);
    assert_eq!(settings.active_profile, "Extra");
    assert_eq!(settings.locale, "en");
    assert_eq!(settings.ui_locale, "zh-Hant");
    assert_eq!(settings.update_channel, "beta");
    assert!(settings.check_updates_on_startup);
}
