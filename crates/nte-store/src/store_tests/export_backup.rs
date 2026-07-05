use super::*;

#[test]
fn export_public_json_and_csv_from_store() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let input = record(
        "c1",
        "CardPool_Character",
        "Fashion_vehicle_1010_V008",
        "2026-05-13 05:59:00",
    );
    let expected_id = expected_record_id(&input);
    let document = public_document(vec![input]);
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
    assert_eq!(first["record_id"], expected_id);
    assert_eq!(first["source_order"], 0);
    assert_eq!(first["banner_id"], "monopoly_limited_Nanali");
    assert_eq!(first["pool_name"], "王牌一代目");
    assert_eq!(first["item_name"], "改裝件·萌虎來襲-塗裝");
    assert_eq!(first["rarity"], 5);
    let expected_keys = vec![
        "record_id",
        "source_order",
        "record_type",
        "time",
        "pool_id",
        "pool_name",
        "banner_id",
        "item_id",
        "item_name",
        "rarity",
        "count",
        "roll_points",
        "roll_label",
    ];
    assert_eq!(
        first
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        expected_keys
    );
    assert!(
        first
            .as_object()
            .expect("exported record should be an object")
            .keys()
            .all(|key| !key.contains("resolution"))
    );
    for key in derived_export_keys() {
        assert!(first.get(key).is_none(), "{key} should not be exported");
    }

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
fn export_synthetic_banner_omits_public_banner_id_and_blanks_csv_name() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let document = public_document(vec![record(
        "unknown-fork",
        "ForkLottery_KaesiNew",
        "fork_dustbin",
        "2026-08-01 10:00:00",
    )]);
    store
        .import_public_document("default", &document, "json", None)
        .unwrap();

    let json_path = tmp.path().join("exports/synthetic.json");
    let csv_path = tmp.path().join("exports/synthetic.csv");
    store
        .export_public_json("default", "zh-Hant", &json_path)
        .unwrap();
    store.export_csv("default", "zh-Hant", &csv_path).unwrap();

    let exported_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(json_path).unwrap()).unwrap();
    let first = exported_json["nte"]["list"][0].as_object().unwrap();
    assert!(!first.contains_key("banner_id"));

    let csv = std::fs::read_to_string(csv_path).unwrap();
    let row = csv.lines().nth(1).unwrap().split(',').collect::<Vec<_>>();
    assert_eq!(row[9], "ForkLottery_KaesiNew");
    assert_eq!(row[10], "");
}

#[test]
fn export_preserves_source_order_inside_same_timestamp_and_writes_roll_labels() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let mut normal = record(
        "normal",
        "ForkLottery_AnHunQu",
        "DiceNormal",
        "2026-01-01 10:00:00",
    );
    normal["source_order"] = serde_json::json!(2);
    let mut gift = record_with_options(
        "gift",
        "ForkLottery_AnHunQu",
        "fork_dustbin",
        Some("2026-01-01 10:00:00"),
        Some(0),
    );
    gift["source_order"] = serde_json::json!(1);
    let mut sleep = record_with_options(
        "sleep",
        "ForkLottery_AnHunQu",
        "fork_jiaojuan",
        Some("2026-01-01 09:59:59"),
        Some(4_294_967_295),
    );
    sleep["source_order"] = serde_json::json!(0);
    let expected_ids = expected_record_ids(&[normal.clone(), gift.clone(), sleep.clone()]);
    let document = public_document(vec![normal, gift, sleep]);
    store
        .import_public_document("default", &document, "json", None)
        .unwrap();

    let json_path = tmp.path().join("exports/order.json");
    let csv_path = tmp.path().join("exports/order.csv");
    store
        .export_public_json("default", "zh-Hant", &json_path)
        .unwrap();
    store.export_csv("default", "zh-Hant", &csv_path).unwrap();

    let exported_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(json_path).unwrap()).unwrap();
    let records = exported_json["nte"]["list"].as_array().unwrap();
    assert_eq!(
        records
            .iter()
            .map(|record| record["record_id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        expected_ids
    );
    assert_eq!(
        records[1]["roll_label_id"],
        "BPUI_LotteryResult_jidianzengli"
    );
    assert_eq!(records[1]["roll_label"], "集點贈禮");
    for key in derived_export_keys() {
        assert!(
            records[1].get(key).is_none(),
            "{key} should not be exported"
        );
    }
    assert!(records[1].get("roll_points").is_none());
    assert_eq!(records[2]["roll_label_id"], "BPUI_LotteryResult_chenmiandi");
    assert_eq!(records[2]["roll_label"], "沉眠地");
    for key in derived_export_keys() {
        assert!(
            records[2].get(key).is_none(),
            "{key} should not be exported"
        );
    }
    assert!(records[2].get("roll_points").is_none());

    let csv = std::fs::read_to_string(csv_path).unwrap();
    let lines = csv.lines().collect::<Vec<_>>();
    assert!(lines[2].contains("集點贈禮"));
    assert!(lines[3].contains("沉眠地"));
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

fn derived_export_keys() -> [&'static str; 21] {
    [
        "banner_name",
        "banner_type",
        "banner_version",
        "counts_as_pull",
        "global_pull_no",
        "guarantee_5_after",
        "guarantee_5_before",
        "hit_rarity",
        "pity_5_after",
        "pity_5_before",
        "pity_badge",
        "pool_kind",
        "pull_no_in_banner",
        "pull_no_in_pool_kind",
        "rate_up_result",
        "rule_id",
        "ten_pull_progress_after",
        "ten_pull_progress_before",
        concat!("pity_", "4_before"),
        concat!("pity_", "4_after"),
        "item_kind",
    ]
}

#[test]
fn generated_run_and_backup_paths_are_unique() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();

    assert_ne!(store.default_run_raw_path(), store.default_run_raw_path());

    let first = store.create_data_backup().unwrap();
    let second = store.create_data_backup().unwrap();

    assert_ne!(first.path, second.path);
    assert!(!first.path.exists());
    assert!(second.path.exists());
}

#[test]
fn cleanup_generated_backups_keeps_latest_direct_file_only() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let dir = tmp.path().join("data/backups");
    let oldest = dir.join("backup-1000000000-000000000-0.zip");
    let middle = dir.join("backup-1000000000-000000001-0.zip");
    let latest = dir.join("backup-1000000000-000000002-0.zip");
    let custom = dir.join("backup-custom.zip");
    let unknown = dir.join("notes.txt");
    let matching_dir = dir.join("backup-1000000000-000000003-0.zip");
    std::fs::write(&oldest, b"oldest").unwrap();
    std::fs::write(&middle, b"middle").unwrap();
    std::fs::write(&latest, b"latest").unwrap();
    std::fs::write(&custom, b"custom").unwrap();
    std::fs::write(&unknown, b"unknown").unwrap();
    std::fs::create_dir(&matching_dir).unwrap();

    store.cleanup_generated_backups_keep_latest().unwrap();

    assert!(!oldest.exists());
    assert!(!middle.exists());
    assert!(latest.exists());
    assert!(custom.exists());
    assert!(unknown.exists());
    assert!(matching_dir.exists());
}

#[test]
fn cleanup_generated_raw_runs_keeps_latest_direct_file_only() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let dir = tmp.path().join("data/runs");
    let oldest = dir.join("raw-1000000000-000000000-0.jsonl");
    let latest = dir.join("raw-1000000000-000000001-0.jsonl");
    let custom = dir.join("raw-custom.jsonl");
    let matching_dir = dir.join("raw-1000000000-000000002-0.jsonl");
    std::fs::write(&oldest, b"oldest").unwrap();
    std::fs::write(&latest, b"latest").unwrap();
    std::fs::write(&custom, b"custom").unwrap();
    std::fs::create_dir(&matching_dir).unwrap();

    store.cleanup_generated_raw_runs_keep_latest().unwrap();

    assert!(!oldest.exists());
    assert!(latest.exists());
    assert!(custom.exists());
    assert!(matching_dir.exists());
}

#[cfg(unix)]
#[test]
fn cleanup_generated_backups_skips_symlink_without_deleting_target() {
    use std::os::unix::fs::symlink;

    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let dir = tmp.path().join("data/backups");
    let outside = tmp.path().join("outside.zip");
    let link = dir.join("backup-1000000000-000000000-0.zip");
    let latest = dir.join("backup-1000000000-000000001-0.zip");
    std::fs::write(&outside, b"keep").unwrap();
    symlink(&outside, &link).unwrap();
    std::fs::write(&latest, b"latest").unwrap();

    store.cleanup_generated_backups_keep_latest().unwrap();

    assert_eq!(std::fs::read(&outside).unwrap(), b"keep");
    assert!(
        std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(latest.exists());
}

#[cfg(unix)]
#[test]
fn cleanup_generated_raw_runs_skips_symlink_without_deleting_target() {
    use std::os::unix::fs::symlink;

    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let dir = tmp.path().join("data/runs");
    let outside = tmp.path().join("outside.jsonl");
    let link = dir.join("raw-1000000000-000000000-0.jsonl");
    let latest = dir.join("raw-1000000000-000000001-0.jsonl");
    std::fs::write(&outside, b"keep").unwrap();
    symlink(&outside, &link).unwrap();
    std::fs::write(&latest, b"latest").unwrap();

    store.cleanup_generated_raw_runs_keep_latest().unwrap();

    assert_eq!(std::fs::read(&outside).unwrap(), b"keep");
    assert!(
        std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(latest.exists());
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
            skipped_update_version: Some("9.9.9".to_string()),
            capture_auto_page_enabled: Some(true),
            capture_full_update_enabled: Some(true),
            capture_windivert_backend_enabled: Some(true),
        })
        .unwrap();
    let same = record(
        "same",
        "CardPool_Character",
        "fork_dustbin",
        "2026-01-01 10:00:00",
    );
    let new = record(
        "new",
        "CardPool_Character",
        "Fashion_vehicle_1010_V008",
        "2026-01-01 10:01:00",
    );
    let expected_default_ids = expected_record_ids(&[same.clone(), new.clone()]);
    let default_doc = public_document(vec![same.clone(), new]);
    source
        .import_public_document("default", &default_doc, "json", None)
        .unwrap();
    let extra = record(
        "extra",
        "ForkLottery_AnHunQu",
        "fork_Rose",
        "2026-01-02 10:00:00",
    );
    let expected_extra_id = expected_record_id(&extra);
    let extra_doc = public_document(vec![extra]);
    source
        .import_public_document("Extra", &extra_doc, "json", None)
        .unwrap();
    let backup_path = tmp.path().join("snapshot.zip");
    source.create_data_backup_at(&backup_path).unwrap();

    let target = JsonStore::open(tmp.path().join("target")).unwrap();
    target
        .import_public_document("default", &public_document(vec![same]), "json", None)
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
    assert_eq!(default_ids, expected_default_ids);
    assert_eq!(extra_ids, vec![expected_extra_id]);
    assert_eq!(settings.active_profile, "Extra");
    assert_eq!(settings.locale, "en");
    assert_eq!(settings.ui_locale, "zh-Hant");
    assert_eq!(settings.update_channel, "beta");
    assert!(settings.check_updates_on_startup);
    assert_eq!(settings.skipped_update_version.as_deref(), Some("9.9.9"));
    assert!(settings.capture_auto_page_enabled);
    assert!(settings.capture_full_update_enabled);
    assert!(settings.capture_windivert_backend_enabled);
}
