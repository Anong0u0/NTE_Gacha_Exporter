use serde_json::json;

use std::io::Write;

use crate::{
    JsonStore, PoolKind, RateUpResult, RecordFilter, RecordSortKey, SettingsPatch, SortDirection,
};

fn public_document(records: Vec<serde_json::Value>) -> String {
    json!({
        "info": {
            "schema": "nte-gacha-export",
            "schema_version": "1.0"
        },
        "nte": {
            "list": records
        }
    })
    .to_string()
}

fn record(record_id: &str, pool_id: &str, item_id: &str, time: &str) -> serde_json::Value {
    json!({
        "record_id": record_id,
        "record_type": if pool_id.starts_with("ForkLottery_") { "fork" } else { "monopoly" },
        "time": time,
        "pool_id": pool_id,
        "pool_name": "display must be ignored",
        "item_id": item_id,
        "item_name": "display must be ignored",
        "count": 1,
        "roll_points": 1,
        "roll_label": "display must be ignored"
    })
}

fn record_with_options(
    record_id: &str,
    pool_id: &str,
    item_id: &str,
    time: Option<&str>,
    roll_points: Option<i64>,
) -> serde_json::Value {
    json!({
        "record_id": record_id,
        "record_type": if pool_id.starts_with("ForkLottery_") { "fork" } else { "monopoly" },
        "time": time,
        "pool_id": pool_id,
        "pool_name": "display must be ignored",
        "item_id": item_id,
        "item_name": "display must be ignored",
        "count": 1,
        "roll_points": roll_points,
        "roll_label": "display must be ignored"
    })
}

fn write_backup_zip(path: &std::path::Path, files: &[(&str, String)]) {
    let file = std::fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::FileOptions::default();
    let names = files
        .iter()
        .map(|(name, _)| (*name).to_string())
        .collect::<Vec<_>>();
    for (name, text) in files {
        zip.start_file(*name, options).unwrap();
        zip.write_all(text.as_bytes()).unwrap();
    }
    zip.start_file("manifest.json", options).unwrap();
    zip.write_all(
        serde_json::to_string(&json!({
            "schema": "nte-gacha-data-backup",
            "schema_version": 1,
            "created_at": "1",
            "files": names,
        }))
        .unwrap()
        .as_bytes(),
    )
    .unwrap();
    zip.finish().unwrap();
}

#[test]
fn store_bootstraps_default_profile_and_files() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();

    let settings = store.settings().unwrap();
    let profiles = store.list_profiles().unwrap();

    assert_eq!(settings.active_profile, "default");
    assert_eq!(settings.update_channel, "stable");
    assert!(!settings.check_updates_on_startup);
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].name, "default");
    assert!(tmp.path().join("data/settings.json").exists());
    assert!(tmp
        .path()
        .join("data/profiles/default/records.json")
        .exists());
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
            update_channel: Some("beta".to_string()),
            check_updates_on_startup: Some(true),
        })
        .unwrap();

    assert_eq!(settings.active_profile, "Player_1");
    assert_eq!(settings.locale, "en");
    assert_eq!(settings.update_channel, "beta");
    assert!(settings.check_updates_on_startup);
    assert!(store
        .update_settings(SettingsPatch {
            locale: Some("missing-locale".to_string()),
            ..SettingsPatch::default()
        })
        .is_err());
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
    assert!(tmp
        .path()
        .join("data/profiles/default/last-run.json")
        .exists());
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

    assert!(store
        .import_public_document("default", &bad, "json", None)
        .is_err());
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

    assert!(store
        .import_public_document("default", &bad, "json", None)
        .is_err());
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
            "schema": "nte-gacha-export",
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
            "schema": "nte-gacha-export",
            "schema_version": "2.0"
        },
        "nte": {
            "list": [v2_record]
        }
    })
    .to_string();
    let v3_major = serde_json::json!({
        "info": {
            "schema": "nte-gacha-export",
            "schema_version": "3.0"
        },
        "nte": {
            "list": [
                record("r3", "CardPool_Character", "fork_dustbin", "2026-01-01 10:02:00")
            ]
        }
    })
    .to_string();

    assert!(store
        .import_public_document("default", &v1_minor, "json", None)
        .is_ok());
    assert!(store
        .import_public_document("default", &v2_major, "json", None)
        .is_ok());
    assert!(store
        .import_public_document("default", &v3_major, "json", None)
        .is_err());
}

#[test]
fn analysis_computes_pity_distribution_and_fork_guarantee() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let document = public_document(vec![
        record(
            "c1",
            "CardPool_Character",
            "fork_dustbin",
            "2026-01-01 10:00:00",
        ),
        record(
            "c2",
            "CardPool_Character",
            "Fashion_vehicle_1010_V008",
            "2026-01-01 10:01:00",
        ),
        record(
            "c3",
            "CardPool_Character",
            "fork_dustbin",
            "2026-01-01 10:02:00",
        ),
        record(
            "f1",
            "ForkLottery_AnHunQu",
            "fork_Arachne",
            "2026-01-02 10:00:00",
        ),
        record(
            "f2",
            "ForkLottery_AnHunQu",
            "fork_Rose",
            "2026-01-02 10:01:00",
        ),
    ]);
    store
        .import_public_document("default", &document, "json", None)
        .unwrap();

    let overview = store.dashboard_overview("default", "zh-Hant").unwrap();
    let limited = overview
        .pool_kinds
        .iter()
        .find(|summary| summary.pool_kind == PoolKind::MonopolyLimited)
        .unwrap();
    let fork = store
        .pool_kind_detail("default", "zh-Hant", PoolKind::ForkLottery)
        .unwrap();

    assert_eq!(overview.total_records, 5);
    assert_eq!(overview.pool_kinds.len(), 3);
    assert_eq!(overview.resource.total_roll_points, 5);
    assert_eq!(overview.resource.known_roll_point_records, 5);
    assert_eq!(overview.resource.missing_roll_point_records, 0);
    assert_eq!(limited.total_pulls, 3);
    assert_eq!(limited.roll_points_total, 3);
    assert_eq!(limited.hit_count, 1);
    assert_eq!(limited.current_pity, 1);
    assert_eq!(limited.average_5star_pity, Some(2.0));
    assert_eq!(limited.average_roll_points_to_5star, Some(2.0));
    assert_eq!(limited.roll_point_cost_samples_5star, 1);
    assert_eq!(limited.early_hit_count, 1);
    assert_eq!(fork.summary.hit_count, 2);
    assert_eq!(fork.summary.roll_points_total, 2);
    assert_eq!(fork.summary.off_rate_count, 1);
    assert_eq!(fork.summary.up_count, 1);
    assert_eq!(fork.summary.early_hit_count, 2);
    assert_eq!(fork.five_star_history[0].guarantee_after, Some(true));
    assert_eq!(fork.five_star_history[1].guarantee_before, Some(true));
    assert_eq!(fork.five_star_history[1].guarantee_after, Some(false));
    assert_eq!(
        overview.latest_records[0].derived.banner_id.as_deref(),
        Some("ForkLottery_AnHunQu")
    );
    assert_eq!(overview.latest_records[0].derived.hit_rarity, Some(5));
    assert!(overview
        .rarity_distribution
        .iter()
        .any(|bucket| bucket.rarity == 5 && bucket.count == 3));
}

#[test]
fn dashboard_overview_empty_profile_returns_empty_stats() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();

    let overview = store.dashboard_overview("default", "zh-Hant").unwrap();
    let limited = overview
        .pool_kinds
        .iter()
        .find(|summary| summary.pool_kind == PoolKind::MonopolyLimited)
        .unwrap();

    assert_eq!(overview.total_records, 0);
    assert_eq!(overview.pool_kinds.len(), 3);
    assert_eq!(limited.total_pulls, 0);
    assert_eq!(limited.roll_points_total, 0);
    assert!(overview.banners.is_empty());
    assert_eq!(overview.resource.total_roll_points, 0);
    assert_eq!(overview.resource.known_roll_point_records, 0);
    assert_eq!(overview.resource.missing_roll_point_records, 0);
    assert_eq!(overview.resource.by_pool_kind.len(), 3);
    assert!(overview.time_stats.monthly.is_empty());
    assert!(overview.time_stats.daily.is_empty());
    assert!(overview.time_stats.phases.is_empty());
    assert_eq!(overview.time_stats.missing_time_records, 0);
}

#[test]
fn dashboard_overview_includes_banner_resource_and_time_stats() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let document = public_document(vec![
        record_with_options(
            "nanali",
            "CardPool_Character",
            "Fashion_vehicle_1010_V008",
            Some("2026-05-13 05:59:00"),
            Some(10),
        ),
        record_with_options(
            "xun",
            "CardPool_Character",
            "Fashion_vehicle_1052_V024",
            Some("2026-05-13 05:59:01"),
            Some(20),
        ),
        record_with_options(
            "fork-4",
            "ForkLottery_AnHunQu",
            "fork_jiaojuan",
            Some("2026-05-14 10:00:00"),
            Some(2),
        ),
        record_with_options(
            "fork-5",
            "ForkLottery_AnHunQu",
            "fork_Rose",
            Some("2026-05-14 10:01:00"),
            Some(8),
        ),
    ]);
    store
        .import_public_document("default", &document, "json", None)
        .unwrap();

    let overview = store.dashboard_overview("default", "zh-Hant").unwrap();
    let fork_banner = overview
        .banners
        .iter()
        .find(|banner| banner.banner_id == "ForkLottery_AnHunQu")
        .unwrap();
    let limited = overview
        .resource
        .by_pool_kind
        .iter()
        .find(|summary| summary.pool_kind == PoolKind::MonopolyLimited)
        .unwrap();
    let fork = overview
        .resource
        .by_pool_kind
        .iter()
        .find(|summary| summary.pool_kind == PoolKind::ForkLottery)
        .unwrap();
    let month = overview
        .time_stats
        .monthly
        .iter()
        .find(|bucket| bucket.bucket == "2026-05")
        .unwrap();
    let day = overview
        .time_stats
        .daily
        .iter()
        .find(|bucket| bucket.bucket == "2026-05-14")
        .unwrap();

    assert!(overview
        .banners
        .iter()
        .any(|banner| banner.banner_id == "monopoly_limited_Nanali"));
    assert!(overview
        .banners
        .iter()
        .any(|banner| banner.banner_id == "monopoly_limited_Xun"));
    assert_eq!(fork_banner.total_pulls, 2);
    assert_eq!(fork_banner.five_star_count, 1);
    assert_eq!(fork_banner.four_star_count, 1);
    assert_eq!(fork_banner.current_5star_pity, 0);
    assert_eq!(fork_banner.current_4star_pity, 1);
    assert_eq!(fork_banner.rate_up_5_count, 1);
    assert_eq!(fork_banner.rate_up_4_count, 0);
    assert_eq!(fork_banner.unknown_rate_up_4_count, 1);
    assert_eq!(fork_banner.roll_points_total, 10);
    assert!(fork_banner.asset_refs.contains_key("icon"));
    assert_eq!(fork_banner.average_roll_points_to_5star, Some(10.0));
    assert_eq!(fork_banner.average_roll_points_to_4star, Some(2.0));
    assert_eq!(fork_banner.roll_point_cost_samples_5star, 1);
    assert_eq!(fork_banner.roll_point_cost_samples_4star, 1);
    assert_eq!(fork_banner.latest_hit.as_ref().unwrap().record_id, "fork-5");
    assert_eq!(overview.resource.total_roll_points, 40);
    assert_eq!(overview.resource.known_roll_point_records, 4);
    assert_eq!(limited.roll_points_total, 30);
    assert_eq!(fork.roll_points_total, 10);
    assert_eq!(month.total_pulls, 4);
    assert_eq!(month.five_star_count, 3);
    assert_eq!(month.four_star_count, 1);
    assert_eq!(month.roll_points_total, 40);
    assert_eq!(day.total_pulls, 2);
    assert_eq!(day.roll_points_total, 10);
    assert_eq!(overview.time_stats.missing_time_records, 0);
}

#[test]
fn stats_skip_unknown_banner_for_banner_summary() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let document = public_document(vec![record_with_options(
        "after",
        "CardPool_Character",
        "fork_dustbin",
        Some("2026-07-08 05:59:01"),
        Some(6),
    )]);
    store
        .import_public_document("default", &document, "json", None)
        .unwrap();

    let overview = store.dashboard_overview("default", "zh-Hant").unwrap();
    let limited = overview
        .pool_kinds
        .iter()
        .find(|summary| summary.pool_kind == PoolKind::MonopolyLimited)
        .unwrap();

    assert!(overview.banners.is_empty());
    assert!(overview.time_stats.phases.is_empty());
    assert_eq!(limited.total_pulls, 1);
    assert_eq!(overview.resource.total_roll_points, 6);
    assert!(overview
        .time_stats
        .monthly
        .iter()
        .any(|bucket| bucket.bucket == "2026-07" && bucket.total_pulls == 1));
}

#[test]
fn stats_track_missing_roll_points_and_missing_time() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let document = public_document(vec![
        record_with_options(
            "known",
            "ForkLottery_AnHunQu",
            "DiceNormal",
            Some("2026-01-02 10:00:00"),
            Some(5),
        ),
        record_with_options("missing", "ForkLottery_AnHunQu", "fork_Rose", None, None),
    ]);
    store
        .import_public_document("default", &document, "json", None)
        .unwrap();

    let overview = store.dashboard_overview("default", "zh-Hant").unwrap();
    let fork = overview
        .pool_kinds
        .iter()
        .find(|summary| summary.pool_kind == PoolKind::ForkLottery)
        .unwrap();

    assert_eq!(overview.resource.total_roll_points, 5);
    assert_eq!(overview.resource.known_roll_point_records, 1);
    assert_eq!(overview.resource.missing_roll_point_records, 1);
    assert_eq!(fork.roll_points_total, 5);
    assert_eq!(fork.known_roll_point_records, 1);
    assert_eq!(fork.missing_roll_point_records, 1);
    assert_eq!(fork.average_roll_points_to_5star, Some(5.0));
    assert_eq!(overview.time_stats.missing_time_records, 1);
    assert_eq!(overview.time_stats.monthly.len(), 1);
    assert_eq!(overview.time_stats.daily.len(), 1);
}

#[test]
fn stats_normalize_roll_point_sentinels_from_public_import() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let document = public_document(vec![
        record_with_options(
            "zero",
            "CardPool_Character",
            "fork_dustbin",
            Some("2026-06-01 10:00:00"),
            Some(0),
        ),
        record_with_options(
            "huge",
            "CardPool_Character",
            "Dice_ticket_01",
            Some("2026-06-01 10:01:00"),
            Some(4_294_967_295),
        ),
        record_with_options(
            "normal",
            "CardPool_Character",
            "fork_vine",
            Some("2026-06-01 10:02:00"),
            Some(6),
        ),
    ]);
    store
        .import_public_document("default", &document, "json", None)
        .unwrap();

    let overview = store.dashboard_overview("default", "zh-Hant").unwrap();
    let limited = overview
        .pool_kinds
        .iter()
        .find(|summary| summary.pool_kind == PoolKind::MonopolyLimited)
        .unwrap();

    assert_eq!(overview.total_records, 3);
    assert_eq!(overview.resource.total_roll_points, 6);
    assert_eq!(overview.resource.known_roll_point_records, 1);
    assert_eq!(overview.resource.missing_roll_point_records, 2);
    assert_eq!(limited.roll_points_total, 6);
    assert_eq!(limited.known_roll_point_records, 1);
    assert_eq!(limited.missing_roll_point_records, 2);
}

#[test]
fn stats_group_phase_from_derived_banner() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let document = public_document(vec![
        record(
            "nanali",
            "CardPool_Character",
            "Fashion_vehicle_1010_V008",
            "2026-05-13 05:59:00",
        ),
        record(
            "xun",
            "CardPool_Character",
            "Fashion_vehicle_1052_V024",
            "2026-05-13 05:59:01",
        ),
    ]);
    store
        .import_public_document("default", &document, "json", None)
        .unwrap();

    let overview = store.dashboard_overview("default", "zh-Hant").unwrap();
    let nanali_phase = overview
        .time_stats
        .phases
        .iter()
        .find(|summary| summary.phase.as_deref() == Some("limited_2026_05_13"))
        .unwrap();
    let xun_phase = overview
        .time_stats
        .phases
        .iter()
        .find(|summary| summary.phase.as_deref() == Some("limited_2026_06_03"))
        .unwrap();

    assert_eq!(nanali_phase.total_pulls, 1);
    assert_eq!(nanali_phase.five_star_count, 1);
    assert_eq!(nanali_phase.roll_points_total, 1);
    assert_eq!(nanali_phase.banner_count, 1);
    assert_eq!(nanali_phase.average_5star_pity, Some(1.0));
    assert_eq!(xun_phase.total_pulls, 1);
    assert_eq!(xun_phase.five_star_count, 1);
    assert_eq!(xun_phase.roll_points_total, 1);
    assert_eq!(xun_phase.banner_count, 1);
    assert_eq!(xun_phase.average_5star_pity, Some(1.0));
}

#[test]
fn missing_rarity_records_are_retained_but_excluded_from_distribution_denominator() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let document = public_document(vec![
        record(
            "known",
            "CardPool_Character",
            "fork_dustbin",
            "2026-01-01 10:00:00",
        ),
        record(
            "missing",
            "CardPool_Character",
            "UnknownItem",
            "2026-01-01 10:01:00",
        ),
    ]);
    store
        .import_public_document("default", &document, "json", None)
        .unwrap();

    let overview = store.dashboard_overview("default", "zh-Hant").unwrap();
    let list = store
        .list_records("default", "zh-Hant", &RecordFilter::default())
        .unwrap();
    let known_count = overview
        .rarity_distribution
        .iter()
        .map(|bucket| bucket.count)
        .sum::<u64>();

    assert_eq!(overview.total_records, 2);
    assert_eq!(list.total, 2);
    assert_eq!(known_count, 1);
    assert!(list
        .records
        .iter()
        .any(|record| record.item_id == "UnknownItem" && record.rarity.is_none()));
    let missing = list
        .records
        .iter()
        .find(|record| record.item_id == "UnknownItem")
        .unwrap();
    assert_eq!(missing.derived.hit_rarity, None);
    assert_eq!(missing.derived.pity_5_after, 2);
    assert!(missing.item_asset_refs.is_empty());
}

#[test]
fn records_list_filters_by_search_and_pool_kind() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let document = public_document(vec![
        record(
            "c1",
            "CardPool_Character",
            "Fashion_vehicle_1010_V008",
            "2026-01-01 10:00:00",
        ),
        record(
            "f1",
            "ForkLottery_AnHunQu",
            "fork_Rose",
            "2026-01-02 10:00:00",
        ),
    ]);
    store
        .import_public_document("default", &document, "json", None)
        .unwrap();

    let list = store
        .list_records(
            "default",
            "zh-Hant",
            &RecordFilter {
                pool_kind: Some(PoolKind::ForkLottery),
                search: Some("玫瑰".to_string()),
                ..RecordFilter::default()
            },
        )
        .unwrap();

    assert_eq!(list.total, 1);
    assert_eq!(list.records[0].item_id, "fork_Rose");
    assert!(list.records[0].item_name.contains("玫瑰"));
    assert_eq!(
        list.records[0].banner.banner_id.as_deref(),
        Some("ForkLottery_AnHunQu")
    );
    assert_eq!(
        list.records[0].banner.source_confidence.as_deref(),
        Some("exact")
    );
    assert!(list.records[0].banner.asset_refs.contains_key("icon"));
    assert_eq!(list.records[0].derived.pull_no_in_banner, Some(1));
}

#[test]
fn records_list_resolves_limited_banner_boundaries() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let document = public_document(vec![
        record(
            "nanali",
            "CardPool_Character",
            "Fashion_vehicle_1010_V008",
            "2026-05-13 05:59:00",
        ),
        record(
            "xun",
            "CardPool_Character",
            "Fashion_vehicle_1052_V024",
            "2026-05-13 05:59:01",
        ),
        record(
            "after",
            "CardPool_Character",
            "fork_dustbin",
            "2026-07-08 05:59:01",
        ),
    ]);
    store
        .import_public_document("default", &document, "json", None)
        .unwrap();

    let list = store
        .list_records(
            "default",
            "zh-Hant",
            &RecordFilter {
                sort_key: Some(RecordSortKey::Time),
                sort_direction: Some(SortDirection::Asc),
                ..RecordFilter::default()
            },
        )
        .unwrap();

    assert_eq!(
        list.records[0].banner.banner_id.as_deref(),
        Some("monopoly_limited_Nanali")
    );
    assert_eq!(
        list.records[0].derived.banner_id.as_deref(),
        Some("monopoly_limited_Nanali")
    );
    assert_eq!(list.records[0].derived.pull_no_in_banner, Some(1));
    assert!(list.records[0].item_asset_refs.contains_key("icon"));
    assert_eq!(
        list.records[1].banner.banner_id.as_deref(),
        Some("monopoly_limited_Xun")
    );
    assert_eq!(
        list.records[1].derived.banner_id.as_deref(),
        Some("monopoly_limited_Xun")
    );
    assert_eq!(list.records[1].derived.pull_no_in_banner, Some(1));
    assert_eq!(
        list.records[2].banner.status,
        crate::model::BannerResolutionStatus::OutsideKnownWindows
    );
    assert_eq!(list.records[2].derived.banner_id, None);
    assert_eq!(list.records[2].pool_label, "限定棋盤");
}

#[test]
fn records_list_filters_by_derived_fields() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let document = public_document(vec![
        record(
            "r1",
            "ForkLottery_AnHunQu",
            "fork_dustbin",
            "2026-01-01 10:00:00",
        ),
        record(
            "r2",
            "ForkLottery_AnHunQu",
            "fork_Arachne",
            "2026-01-01 10:01:00",
        ),
        record(
            "r3",
            "ForkLottery_AnHunQu",
            "fork_Rose",
            "2026-01-01 10:02:00",
        ),
    ]);
    store
        .import_public_document("default", &document, "json", None)
        .unwrap();

    let banner = store
        .list_records(
            "default",
            "zh-Hant",
            &RecordFilter {
                banner_id: Some("ForkLottery_AnHunQu".to_string()),
                hit_rarity: Some(5),
                sort_key: Some(RecordSortKey::PullNo),
                sort_direction: Some(SortDirection::Asc),
                ..RecordFilter::default()
            },
        )
        .unwrap();
    let off_rate = store
        .list_records(
            "default",
            "zh-Hant",
            &RecordFilter {
                rate_up_result: Some(RateUpResult::OffRate),
                pity_5_min: Some(1),
                pity_5_max: Some(1),
                ..RecordFilter::default()
            },
        )
        .unwrap();

    assert_eq!(banner.total, 2);
    assert_eq!(banner.records[0].record_id, "r2");
    assert_eq!(
        banner.records[0].derived.rate_up_result,
        RateUpResult::OffRate
    );
    assert_eq!(banner.records[1].record_id, "r3");
    assert_eq!(banner.records[1].derived.guarantee_5_before, Some(true));
    assert_eq!(off_rate.total, 1);
    assert_eq!(off_rate.records[0].record_id, "r2");
}

#[test]
fn records_list_filters_by_type_date_sort_and_page() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let document = public_document(vec![
        record(
            "c1",
            "CardPool_Character",
            "Fashion_vehicle_1010_V008",
            "2026-01-01 10:00:00",
        ),
        record(
            "f1",
            "ForkLottery_AnHunQu",
            "fork_Rose",
            "2026-01-02 10:00:00",
        ),
        record(
            "f2",
            "ForkLottery_AnHunQu",
            "DiceNormal",
            "2026-01-03 10:00:00",
        ),
    ]);
    store
        .import_public_document("default", &document, "json", None)
        .unwrap();

    let list = store
        .list_records(
            "default",
            "zh-Hant",
            &RecordFilter {
                record_type: Some("fork".to_string()),
                date_from: Some("2026-01-02".to_string()),
                date_to: Some("2026-01-03".to_string()),
                sort_key: Some(RecordSortKey::Item),
                sort_direction: Some(SortDirection::Asc),
                limit: Some(1),
                offset: Some(0),
                ..RecordFilter::default()
            },
        )
        .unwrap();

    assert_eq!(list.total, 2);
    assert_eq!(list.records.len(), 1);
    assert_eq!(list.records[0].record_type, "fork");
}

#[test]
fn record_filter_options_count_pools_and_record_types() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let document = public_document(vec![
        record(
            "c1",
            "CardPool_Character",
            "fork_dustbin",
            "2026-01-01 10:00:00",
        ),
        record(
            "c2",
            "CardPool_Character",
            "fork_dustbin",
            "2026-01-01 10:01:00",
        ),
        record(
            "f1",
            "ForkLottery_AnHunQu",
            "fork_Rose",
            "2026-01-02 10:00:00",
        ),
    ]);
    store
        .import_public_document("default", &document, "json", None)
        .unwrap();

    let options = store.record_filter_options("default", "zh-Hant").unwrap();

    assert!(options
        .pools
        .iter()
        .any(|pool| pool.pool_id == "CardPool_Character" && pool.count == 2));
    assert!(options
        .record_types
        .iter()
        .any(|record_type| record_type.record_type == "monopoly" && record_type.count == 2));
    assert!(options
        .banners
        .iter()
        .any(|banner| banner.banner_id == "ForkLottery_AnHunQu" && banner.count == 1));
}

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
    assert_eq!(settings.update_channel, "beta");
    assert!(settings.check_updates_on_startup);
}

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

    assert!(store
        .import_public_document_with_backup("default", &bad, "auto_page_full", None)
        .is_err());

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
