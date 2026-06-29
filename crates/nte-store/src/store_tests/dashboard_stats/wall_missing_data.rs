use super::super::*;

#[test]
fn dashboard_five_star_wall_history_is_newest_first_without_pull_number_sort() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let mut late_ticket = record_with_options(
        "late-ticket",
        "CardPool_Character",
        "Dice_ticket_01",
        Some("2026-06-03 16:42:17"),
        Some(4_294_967_295),
    );
    late_ticket["source_order"] = serde_json::json!(125);
    let mut same_time_sleep_ticket = record_with_options(
        "same-time-sleep-ticket",
        "CardPool_Character",
        "Dice_ticket_01",
        Some("2026-05-15 12:00:00"),
        Some(4_294_967_295),
    );
    same_time_sleep_ticket["source_order"] = serde_json::json!(300);
    let mut same_time_character = record(
        "same-time-character",
        "CardPool_Character",
        "1010",
        "2026-05-15 12:00:00",
    );
    same_time_character["source_order"] = serde_json::json!(301);
    let mut old_character = record(
        "old-character",
        "CardPool_Character",
        "1010",
        "2026-04-30 17:02:07",
    );
    old_character["source_order"] = serde_json::json!(209);
    let mut old_dice = record(
        "old-dice",
        "CardPool_Character",
        "Dicelimite",
        "2026-04-30 17:02:07",
    );
    old_dice["source_order"] = serde_json::json!(216);
    let document = public_document(vec![
        old_dice,
        same_time_character,
        late_ticket,
        old_character,
        same_time_sleep_ticket,
    ]);
    store
        .import_public_document("default", &document, "json", None)
        .unwrap();

    let detail = store
        .dashboard_selection_detail(
            "default",
            "zh-Hant",
            &DashboardSelection::PoolKind {
                pool_kind: PoolKind::MonopolyLimited,
            },
        )
        .unwrap();

    assert_eq!(
        detail
            .five_star_wall_history
            .iter()
            .map(|hit| (hit.record.item_id.as_str(), hit.record.time.as_deref()))
            .collect::<Vec<_>>(),
        vec![
            ("Dice_ticket_01", Some("2026-06-03 16:42:17")),
            ("1010", Some("2026-05-15 12:00:00")),
            ("Dice_ticket_01", Some("2026-05-15 12:00:00")),
            ("Dicelimite", Some("2026-04-30 17:02:07")),
            ("1010", Some("2026-04-30 17:02:07")),
        ]
    );
    assert_eq!(
        detail
            .five_star_wall_history
            .iter()
            .map(|hit| hit.record.source_order)
            .collect::<Vec<_>>(),
        vec![125, 301, 300, 216, 209]
    );
    assert_eq!(
        detail
            .five_star_wall_history
            .iter()
            .map(|hit| hit.five_star_distance)
            .collect::<Vec<_>>(),
        vec![1, 1, 1, 1, 1]
    );
    let same_time_sleep = detail
        .five_star_wall_history
        .iter()
        .find(|hit| {
            hit.record.item_id == "Dice_ticket_01"
                && hit.record.time.as_deref() == Some("2026-05-15 12:00:00")
        })
        .unwrap();
    assert_eq!(same_time_sleep.record.roll_label.as_deref(), Some("沉眠地"));
    assert!(!same_time_sleep.record.derived.counts_as_pull);
    assert_eq!(
        detail
            .summary
            .latest_5star_any
            .as_ref()
            .map(|record| (record.item_id.as_str(), record.time.as_deref())),
        Some(("Dice_ticket_01", Some("2026-06-03 16:42:17")))
    );
}

#[test]
fn dashboard_five_wall_distance_uses_effective_pull_intervals() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let mut first_ticket = record_with_options(
        "ticket-1",
        "CardPool_Character",
        "Dice_ticket_01",
        Some("2026-01-01 00:01:00"),
        None,
    );
    first_ticket["roll_label_id"] = serde_json::json!("BPUI_LotteryResult_chenmiandi");
    let mut first_dice = record(
        "dice-1",
        "CardPool_Character",
        "Dicelimite",
        "2026-01-01 00:02:00",
    );
    first_dice["source_order"] = serde_json::json!(20);
    let mut character = record(
        "character",
        "CardPool_Character",
        "1010",
        "2026-01-01 00:03:00",
    );
    character["source_order"] = serde_json::json!(30);
    let mut second_ticket = record_with_options(
        "ticket-2",
        "CardPool_Character",
        "Dice_ticket_01",
        Some("2026-01-01 00:04:00"),
        None,
    );
    second_ticket["source_order"] = serde_json::json!(40);
    second_ticket["roll_label_id"] = serde_json::json!("BPUI_LotteryResult_chenmiandi");
    let mut second_dice = record(
        "dice-2",
        "CardPool_Character",
        "Dicelimite",
        "2026-01-01 00:05:00",
    );
    second_dice["source_order"] = serde_json::json!(50);
    let document = public_document(vec![
        first_ticket,
        first_dice,
        character,
        second_ticket,
        second_dice,
    ]);
    store
        .import_public_document("default", &document, "json", None)
        .unwrap();

    let detail = store
        .dashboard_selection_detail(
            "default",
            "zh-Hant",
            &DashboardSelection::PoolKind {
                pool_kind: PoolKind::MonopolyLimited,
            },
        )
        .unwrap();

    assert_eq!(
        detail
            .five_star_wall_history
            .iter()
            .map(|hit| {
                (
                    hit.record.item_id.as_str(),
                    hit.five_star_distance,
                    hit.focused_distance,
                )
            })
            .collect::<Vec<_>>(),
        vec![
            ("Dicelimite", 1, None),
            ("Dice_ticket_01", 1, None),
            ("1010", 1, Some(2)),
            ("Dicelimite", 1, None),
            ("Dice_ticket_01", 1, None),
        ]
    );
    assert_eq!(detail.summary.current_pity, 1);
    assert_eq!(detail.summary.current_ten_pull_progress, Some(3));
}

#[test]
fn stats_include_synthetic_limited_banner_for_banner_summary() {
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

    let banner = overview
        .banners
        .iter()
        .find(|banner| banner.banner_id == "CardPool_Character")
        .unwrap();
    assert_eq!(
        banner.resolution_issue,
        Some(nte_core::BannerResolutionIssue::OutsideKnownWindows)
    );
    assert_eq!(banner.pool_kind, PoolKind::MonopolyLimited);
    assert!(banner.asset_refs.is_empty());
    assert_eq!(banner.total_pulls, 1);
    assert_eq!(limited.total_pulls, 1);
    assert_eq!(limited.roll_points_total, 6);
    assert!(
        overview
            .time_stats
            .monthly
            .iter()
            .any(|bucket| bucket.bucket == "2026-07" && bucket.total_pulls == 1)
    );
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

    assert_eq!(fork.roll_points_total, 5);
    assert_eq!(fork.known_roll_point_records, 1);
    assert_eq!(fork.missing_roll_point_records, 1);
    assert_eq!(fork.average_roll_points_to_5star, None);
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
    assert_eq!(limited.roll_points_total, 6);
    assert_eq!(limited.known_roll_point_records, 1);
    assert_eq!(limited.missing_roll_point_records, 0);
    assert_eq!(limited.total_pulls, 1);
}
