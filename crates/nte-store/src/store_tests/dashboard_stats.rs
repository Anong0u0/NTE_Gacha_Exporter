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
    assert_eq!(
        overview
            .pool_kinds
            .iter()
            .map(|summary| summary.roll_points_total)
            .sum::<i64>(),
        5
    );
    assert_eq!(limited.total_pulls, 3);
    assert_eq!(limited.roll_points_total, 3);
    assert_eq!(limited.known_roll_point_records, 3);
    assert_eq!(limited.missing_roll_point_records, 0);
    assert_eq!(limited.hit_count, 0);
    assert_eq!(limited.current_pity, 3);
    assert_eq!(limited.average_5star_pity, None);
    assert_eq!(limited.average_roll_points_to_5star, None);
    assert_eq!(limited.roll_point_cost_samples_5star, 0);
    assert_eq!(limited.early_hit_count, 0);
    assert_eq!(fork.summary.hit_count, 2);
    assert_eq!(fork.summary.roll_points_total, 2);
    assert_eq!(fork.summary.known_roll_point_records, 2);
    assert_eq!(fork.summary.missing_roll_point_records, 0);
    assert_eq!(fork.summary.off_rate_count, 1);
    assert_eq!(fork.summary.up_count, 1);
    assert_eq!(fork.summary.early_hit_count, 2);
    assert_eq!(fork.five_star_history[0].guarantee_after, Some(true));
    assert_eq!(fork.five_star_history[1].guarantee_before, Some(true));
    assert_eq!(fork.five_star_history[1].guarantee_after, Some(false));
    assert_eq!(
        fork.five_star_history[1].record.derived.banner_id.as_deref(),
        Some("ForkLottery_AnHunQu")
    );
    assert_eq!(fork.five_star_history[1].record.derived.hit_rarity, Some(5));
    assert!(
        overview
            .rarity_distribution
            .iter()
            .any(|bucket| bucket.rarity == 5 && bucket.count == 3)
    );
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
    assert_eq!(
        overview
            .pool_kinds
            .iter()
            .map(|summary| summary.roll_points_total)
            .sum::<i64>(),
        0
    );
    assert!(overview.time_stats.monthly.is_empty());
    assert!(overview.time_stats.daily.is_empty());
    assert_eq!(overview.time_stats.missing_time_records, 0);
}

#[test]
fn dashboard_overview_includes_banner_roll_points_and_time_stats() {
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
        .pool_kinds
        .iter()
        .find(|summary| summary.pool_kind == PoolKind::MonopolyLimited)
        .unwrap();
    let fork = overview
        .pool_kinds
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

    assert!(
        overview
            .banners
            .iter()
            .any(|banner| banner.banner_id == "monopoly_limited_Nanali")
    );
    assert!(
        overview
            .banners
            .iter()
            .any(|banner| banner.banner_id == "monopoly_limited_Xun")
    );
    assert_eq!(fork_banner.total_pulls, 2);
    assert_eq!(fork_banner.five_star_count, 1);
    assert_eq!(fork_banner.four_star_count, 1);
    assert_eq!(fork_banner.current_5star_pity, 0);
    assert_eq!(fork_banner.rate_up_5_count, 1);
    assert_eq!(fork_banner.rate_up_4_count, 0);
    assert_eq!(fork_banner.unknown_rate_up_4_count, 1);
    assert_eq!(fork_banner.roll_points_total, 10);
    assert!(fork_banner.asset_refs.contains_key("icon"));
    assert_eq!(fork_banner.average_roll_points_to_5star, Some(10.0));
    assert_eq!(fork_banner.roll_point_cost_samples_5star, 1);
    assert_eq!(fork_banner.latest_hit.as_ref().unwrap().record_id, "fork-5");
    assert_eq!(
        overview
            .pool_kinds
            .iter()
            .map(|summary| summary.roll_points_total)
            .sum::<i64>(),
        40
    );
    assert_eq!(
        overview
            .pool_kinds
            .iter()
            .map(|summary| summary.known_roll_point_records)
            .sum::<u64>(),
        4
    );
    assert_eq!(limited.roll_points_total, 30);
    assert_eq!(fork.roll_points_total, 10);
    assert_eq!(month.total_pulls, 4);
    assert_eq!(month.five_star_count, 1);
    assert_eq!(month.four_star_count, 1);
    assert_eq!(month.roll_points_total, 40);
    assert_eq!(day.total_pulls, 2);
    assert_eq!(day.roll_points_total, 10);
    assert_eq!(overview.time_stats.missing_time_records, 0);
}

#[test]
fn fork_stats_separate_twenty_five_seventy_five_wins_losses_and_forced_up() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let mut records = Vec::new();
    records.push(record(
        "r001-loss",
        "ForkLottery_AnHunQu",
        "fork_Arachne",
        "2026-01-01 00:01:00",
    ));
    for index in 2..60 {
        records.push(record(
            &format!("r{index:03}"),
            "ForkLottery_AnHunQu",
            "fork_dustbin",
            &format!("2026-01-01 00:{index:02}:00"),
        ));
    }
    records.push(record(
        "r060-loss",
        "ForkLottery_AnHunQu",
        "fork_Arachne",
        "2026-01-01 01:00:00",
    ));
    for index in 61..80 {
        records.push(record(
            &format!("r{index:03}"),
            "ForkLottery_AnHunQu",
            "fork_dustbin",
            &format!("2026-01-01 01:{:02}:00", index - 60),
        ));
    }
    records.push(record(
        "r080-forced",
        "ForkLottery_AnHunQu",
        "fork_Rose",
        "2026-01-01 02:00:00",
    ));
    let document = public_document(records);
    store
        .import_public_document("default", &document, "json", None)
        .unwrap();

    let overview = store.dashboard_overview("default", "zh-Hant").unwrap();
    let fork = overview
        .pool_kinds
        .iter()
        .find(|summary| summary.pool_kind == PoolKind::ForkLottery)
        .unwrap();
    let banner = overview
        .banners
        .iter()
        .find(|banner| banner.banner_id == "ForkLottery_AnHunQu")
        .unwrap();
    let detail = store
        .pool_kind_detail("default", "zh-Hant", PoolKind::ForkLottery)
        .unwrap();
    let forced = detail
        .five_star_history
        .iter()
        .find(|hit| hit.record.record_id == "r080-forced")
        .unwrap();

    assert_eq!(fork.hit_count, 3);
    assert_eq!(fork.hard_pity, 60);
    assert_eq!(fork.fork_win_count, 0);
    assert_eq!(fork.fork_loss_count, 2);
    assert_eq!(fork.fork_forced_up_count, 1);
    assert_eq!(fork.fork_observed_25_75_win_rate, Some(0.0));
    assert_eq!(banner.fork_loss_count, 2);
    assert_eq!(banner.fork_forced_up_count, 1);
    assert_eq!(forced.record.derived.fork_up_pity_before, Some(79));
    assert_eq!(forced.record.derived.fork_forced_up, Some(true));
}

#[test]
fn dashboard_selection_detail_switches_between_pool_and_banner_scope() {
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

    let pool = store
        .dashboard_selection_detail(
            "default",
            "zh-Hant",
            &DashboardSelection::PoolKind {
                pool_kind: PoolKind::MonopolyLimited,
            },
        )
        .unwrap();
    let banner = store
        .dashboard_selection_detail(
            "default",
            "zh-Hant",
            &DashboardSelection::Banner {
                pool_kind: PoolKind::MonopolyLimited,
                banner_id: "monopoly_limited_Nanali".to_string(),
            },
        )
        .unwrap();

    assert_eq!(pool.summary.label, "限定棋盤");
    assert_eq!(pool.summary.total_pulls, 2);
    assert_eq!(pool.five_star_history.len(), 0);
    assert_eq!(pool.rarity_distribution[0].count, 2);
    assert_eq!(pool.item_ranking.len(), 2);

    assert_eq!(banner.summary.label, "王牌一代目");
    assert_eq!(banner.summary.total_pulls, 1);
    assert_eq!(banner.summary.roll_points_total, 10);
    assert_eq!(banner.five_star_history.len(), 0);
    assert_eq!(banner.rarity_distribution[0].count, 1);
    assert_eq!(banner.item_ranking[0].item_id, "Fashion_vehicle_1010_V008");
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
