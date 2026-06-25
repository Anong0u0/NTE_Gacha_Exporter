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
            "c4",
            "CardPool_Character",
            "fork_jiaojuan",
            "2026-01-01 10:03:00",
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
    let limited_detail = store
        .dashboard_selection_detail(
            "default",
            "zh-Hant",
            &DashboardSelection::PoolKind {
                pool_kind: PoolKind::MonopolyLimited,
            },
        )
        .unwrap();

    assert_eq!(overview.total_records, 6);
    assert_eq!(overview.pool_kinds.len(), 3);
    assert_eq!(
        overview
            .pool_kinds
            .iter()
            .map(|summary| summary.roll_points_total)
            .sum::<i64>(),
        6
    );
    assert_eq!(limited.total_pulls, 4);
    assert_eq!(limited.roll_points_total, 4);
    assert_eq!(limited.known_roll_point_records, 4);
    assert_eq!(limited.missing_roll_point_records, 0);
    assert_eq!(limited.hit_count, 0);
    assert_eq!(limited.five_star_item_count, 1);
    assert_eq!(limited.current_pity, 4);
    assert_eq!(limited.average_5star_pity, None);
    assert_eq!(limited.latest_5star, None);
    assert_eq!(
        limited.latest_5star_any.as_ref().map(|record| record.record_id.as_str()),
        Some("c2")
    );
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
    assert_eq!(fork.five_star_wall_history.len(), 2);
    assert_eq!(limited_detail.five_star_history.len(), 0);
    assert_eq!(limited_detail.five_star_wall_history.len(), 1);
    assert_eq!(
        limited_detail.five_star_wall_history[0]
            .record
            .record_id
            .as_str(),
        "c2"
    );
    assert!(
        overview
            .rarity_distribution
            .iter()
            .any(|bucket| bucket.rarity == 5 && bucket.count == 3)
    );
    assert_eq!(
        limited_detail
            .pull_rarity_distribution
            .iter()
            .map(|bucket| bucket.count)
            .sum::<u64>(),
        limited_detail.summary.total_pulls
    );
    assert!(limited_detail.pull_rarity_distribution.iter().any(|bucket| {
        bucket.key == PullRarityBucketKey::FiveItem
            && bucket.rarity == Some(5)
            && bucket.count == 1
            && bucket.percent == 0.25
    }));
    assert!(limited_detail.pull_rarity_distribution.iter().any(|bucket| {
        bucket.key == PullRarityBucketKey::FourItem
            && bucket.rarity == Some(4)
            && bucket.count == 1
            && bucket.percent == 0.25
    }));
    assert!(limited_detail.pull_rarity_distribution.iter().any(|bucket| {
        bucket.key == PullRarityBucketKey::Three
            && bucket.rarity == Some(3)
            && bucket.count == 2
            && bucket.percent == 0.5
    }));
    assert!(
        limited_detail
            .pull_rarity_distribution
            .iter()
            .all(|bucket| bucket.key != PullRarityBucketKey::FiveUp)
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
fn item_ranking_splits_same_item_by_reward_quantity() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let mut dustbin_30_a = record(
        "dustbin-30-a",
        "CardPool_Character",
        "fork_dustbin",
        "2026-01-01 10:00:00",
    );
    let mut dustbin_30_b = record(
        "dustbin-30-b",
        "CardPool_Character",
        "fork_dustbin",
        "2026-01-01 10:01:00",
    );
    let mut dustbin_50 = record(
        "dustbin-50",
        "CardPool_Character",
        "fork_dustbin",
        "2026-01-01 10:02:00",
    );
    dustbin_30_a["count"] = json!(30);
    dustbin_30_b["count"] = json!(30);
    dustbin_50["count"] = json!(50);
    let document = public_document(vec![dustbin_30_a, dustbin_30_b, dustbin_50]);
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
    let dustbin_ranks = detail
        .item_ranking
        .iter()
        .filter(|item| item.item_id == "fork_dustbin")
        .collect::<Vec<_>>();
    let dustbin_30 = dustbin_ranks
        .iter()
        .find(|item| item.reward_count == 30)
        .unwrap();
    let dustbin_50 = dustbin_ranks
        .iter()
        .find(|item| item.reward_count == 50)
        .unwrap();

    assert_eq!(dustbin_ranks.len(), 2);
    assert_eq!(dustbin_30.count, 2);
    assert_eq!(dustbin_50.count, 1);
    assert_eq!(dustbin_30.item_name, dustbin_50.item_name);
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
    let forced_display = detail
        .five_star_wall_history
        .iter()
        .find(|hit| hit.record.record_id == "r080-forced")
        .unwrap();
    let loss_display = detail
        .five_star_wall_history
        .iter()
        .find(|hit| hit.record.record_id == "r060-loss")
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
    assert_eq!(forced.pity_distance, 20);
    assert_eq!(forced_display.five_star_distance, 20);
    assert_eq!(forced_display.focused_distance, Some(80));
    assert_eq!(loss_display.five_star_distance, loss_display.pity_distance);
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
fn banner_scope_keeps_focused_wall_distance_from_pool_kind_history() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let mut records = Vec::new();
    for index in 1..=8 {
        records.push(record(
            &format!("xun-filler-{index}"),
            "CardPool_Character",
            "fork_dustbin",
            &format!("2026-05-20 00:{index:02}:00"),
        ));
    }
    records.push(record(
        "xun-up",
        "CardPool_Character",
        "1052",
        "2026-05-20 00:09:00",
    ));
    for index in 10..=27 {
        records.push(record(
            &format!("xun-post-{index}"),
            "CardPool_Character",
            "fork_dustbin",
            &format!("2026-05-20 00:{index:02}:00"),
        ));
    }
    for index in 1..=25 {
        records.push(record(
            &format!("anhunqu-filler-{index}"),
            "CardPool_Character",
            "fork_dustbin",
            &format!("2026-06-04 00:{index:02}:00"),
        ));
    }
    records.push(record(
        "anhunqu-up",
        "CardPool_Character",
        "1004",
        "2026-06-04 00:26:00",
    ));
    store
        .import_public_document("default", &public_document(records), "json", None)
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
                banner_id: "monopoly_limited_AnHunQu".to_string(),
            },
        )
        .unwrap();
    let xun_up = pool
        .five_star_wall_history
        .iter()
        .find(|hit| hit.record.record_id == "xun-up")
        .unwrap();
    let pool_anhunqu_up = pool
        .five_star_wall_history
        .iter()
        .find(|hit| hit.record.record_id == "anhunqu-up")
        .unwrap();
    let banner_anhunqu_up = banner
        .five_star_wall_history
        .iter()
        .find(|hit| hit.record.record_id == "anhunqu-up")
        .unwrap();

    assert_eq!(xun_up.focused_distance, Some(9));
    assert_eq!(pool_anhunqu_up.focused_distance, Some(44));
    assert_eq!(banner_anhunqu_up.five_star_distance, 26);
    assert_eq!(banner_anhunqu_up.focused_distance, Some(44));
}

#[test]
fn limited_focused_wall_includes_non_up_character_and_all_wall_resets_on_items() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let document = public_document(vec![
        record(
            "limited-item",
            "CardPool_Character",
            "Fashion_vehicle_1010_V008",
            "2026-06-04 00:01:00",
        ),
        record(
            "limited-off-character",
            "CardPool_Character",
            "1010",
            "2026-06-04 00:02:00",
        ),
        record(
            "limited-up-character",
            "CardPool_Character",
            "1004",
            "2026-06-04 00:03:00",
        ),
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
                    hit.record.record_id.as_str(),
                    hit.five_star_distance,
                    hit.focused_distance,
                    hit.result.clone(),
                )
            })
            .collect::<Vec<_>>(),
        vec![
            ("limited-up-character", 1, Some(1), FiveStarResult::Up),
            (
                "limited-off-character",
                1,
                Some(2),
                FiveStarResult::OffRate,
            ),
            ("limited-item", 1, None, FiveStarResult::NotApplicable),
        ]
    );
}

#[test]
fn profile_analysis_view_matches_detail_options_and_record_page_contracts() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let document = public_document(vec![
        record(
            "lose",
            "ForkLottery_AnHunQu",
            "fork_Arachne",
            "2026-01-01 10:00:00",
        ),
        record(
            "win",
            "ForkLottery_AnHunQu",
            "fork_Rose",
            "2026-01-01 10:01:00",
        ),
        record(
            "limited",
            "CardPool_Character",
            "1010",
            "2026-01-02 10:00:00",
        ),
    ]);
    store
        .import_public_document("default", &document, "json", None)
        .unwrap();

    let selection = DashboardSelection::PoolKind {
        pool_kind: PoolKind::ForkLottery,
    };
    let record_filter = RecordFilter {
        pool_kind: Some(PoolKind::ForkLottery),
        focused_rarities: vec![5],
        fork_result_marks: vec![ForkResultMark::Win],
        sort_direction: Some(SortDirection::Asc),
        ..RecordFilter::default()
    };

    let view = store
        .profile_analysis_view("default", "zh-Hant", &selection, &record_filter)
        .unwrap();
    let detail = store
        .dashboard_scope_detail("default", "zh-Hant", &selection)
        .unwrap();
    let options = store.record_filter_options("default", "zh-Hant").unwrap();
    let page = store
        .record_page("default", "zh-Hant", &record_filter)
        .unwrap();

    assert_eq!(view.overview.total_records, 3);
    assert_eq!(view.selected_detail, detail);
    assert_eq!(view.record_filter_options, options);
    assert_eq!(view.record_page, page);
    assert_eq!(view.record_page.total, 1);
    assert_eq!(view.record_page.records[0].record_id, "win");
    assert_eq!(
        view.record_page.records[0].fork_result_mark,
        Some(ForkResultMark::Win)
    );
    assert_eq!(view.record_page.records[0].item_kind, ItemKind::Fork);
    assert_eq!(view.record_page.records[0].roll_bucket, RollBucket::One);
    assert!(view
        .selected_detail
        .five_star_history
        .iter()
        .any(|hit| hit.record.record_id == "lose"
            && hit.record.fork_result_mark == Some(ForkResultMark::Lose)));
}

#[test]
fn dashboard_selection_detail_reports_hit_distribution_and_average_four_star_pity() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let mut gift = record(
        "gift",
        "ForkLottery_AnHunQu",
        "fork_dustbin",
        "2026-06-01 00:01:00",
    );
    gift["roll_label_id"] = serde_json::json!("BPUI_LotteryResult_jidianzengli");
    gift["roll_points"] = serde_json::Value::Null;
    let mut sleep = record(
        "sleep",
        "ForkLottery_AnHunQu",
        "fork_dustbin",
        "2026-06-01 00:03:00",
    );
    sleep["roll_label_id"] = serde_json::json!("BPUI_LotteryResult_chenmiandi");
    sleep["roll_points"] = serde_json::Value::Null;
    let document = public_document(vec![
        record(
            "three-1",
            "ForkLottery_AnHunQu",
            "fork_dustbin",
            "2026-06-01 00:00:00",
        ),
        gift,
        record(
            "four",
            "ForkLottery_AnHunQu",
            "fork_jiaojuan",
            "2026-06-01 00:02:00",
        ),
        sleep,
        record(
            "three-2",
            "ForkLottery_AnHunQu",
            "fork_dustbin",
            "2026-06-01 00:04:00",
        ),
        record(
            "off-rate-five",
            "ForkLottery_AnHunQu",
            "fork_Arachne",
            "2026-06-01 00:05:00",
        ),
        record(
            "up-five",
            "ForkLottery_AnHunQu",
            "fork_Rose",
            "2026-06-01 00:06:00",
        ),
    ]);
    store
        .import_public_document("default", &document, "json", None)
        .unwrap();

    let detail = store
        .dashboard_selection_detail(
            "default",
            "zh-Hant",
            &DashboardSelection::PoolKind {
                pool_kind: PoolKind::ForkLottery,
            },
        )
        .unwrap();

    assert_eq!(detail.summary.total_pulls, 5);
    assert_eq!(detail.summary.hit_count, 2);
    assert_eq!(detail.summary.five_star_item_count, 2);
    assert_eq!(detail.summary.up_count, 1);
    assert_eq!(detail.summary.four_star_count, 1);
    assert_eq!(detail.summary.average_4star_pity, Some(5.0 / 3.0));
    assert!(detail.rarity_distribution.iter().any(|bucket| {
        bucket.rarity == 5 && bucket.count == 2
    }));
    assert!(detail.hit_rarity_distribution.iter().any(|bucket| {
        bucket.rarity == 5 && bucket.count == 1 && bucket.percent == 0.25
    }));
    assert!(detail.hit_rarity_distribution.iter().any(|bucket| {
        bucket.rarity == 4 && bucket.count == 1 && bucket.percent == 0.25
    }));
    assert!(detail.hit_rarity_distribution.iter().any(|bucket| {
        bucket.rarity == 3 && bucket.count == 2 && bucket.percent == 0.5
    }));
    assert_eq!(
        detail
            .pull_rarity_distribution
            .iter()
            .map(|bucket| bucket.count)
            .sum::<u64>(),
        detail.summary.total_pulls
    );
    assert!(detail.pull_rarity_distribution.iter().any(|bucket| {
        bucket.key == PullRarityBucketKey::FiveUp
            && bucket.rarity == Some(5)
            && bucket.count == 1
            && bucket.percent == 0.2
    }));
    assert!(detail.pull_rarity_distribution.iter().any(|bucket| {
        bucket.key == PullRarityBucketKey::FiveNonUp
            && bucket.rarity == Some(5)
            && bucket.count == 1
            && bucket.percent == 0.2
    }));
    assert!(detail.pull_rarity_distribution.iter().any(|bucket| {
        bucket.key == PullRarityBucketKey::FourHit
            && bucket.rarity == Some(4)
            && bucket.count == 1
            && bucket.percent == 0.2
    }));
    assert!(detail.pull_rarity_distribution.iter().any(|bucket| {
        bucket.key == PullRarityBucketKey::Three
            && bucket.rarity == Some(3)
            && bucket.count == 2
            && bucket.percent == 0.4
    }));
}

#[test]
fn standard_pool_uses_item_quality_for_display_rarity() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let mut gift = record(
        "standard-gift",
        "CardPool_NewRole",
        "1010",
        "2026-06-02 00:02:00",
    );
    gift["roll_label_id"] = serde_json::json!("BPUI_LotteryResult_jidianzengli");
    gift["roll_points"] = serde_json::Value::Null;
    let document = public_document(vec![
        record(
            "standard-character",
            "CardPool_NewRole",
            "1003",
            "2026-06-02 00:00:00",
        ),
        record(
            "standard-fork",
            "CardPool_NewRole",
            "fork_wuhuakuang",
            "2026-06-02 00:01:00",
        ),
        gift,
    ]);
    store
        .import_public_document("default", &document, "json", None)
        .unwrap();

    let detail = store
        .dashboard_selection_detail(
            "default",
            "zh-Hant",
            &DashboardSelection::PoolKind {
                pool_kind: PoolKind::MonopolyStandard,
            },
        )
        .unwrap();
    let overview = store.dashboard_overview("default", "zh-Hant").unwrap();
    let standard_banner = overview
        .banners
        .iter()
        .find(|banner| banner.banner_id == "monopoly_standard")
        .unwrap();
    let fork_rank = detail
        .item_ranking
        .iter()
        .find(|item| item.item_id == "fork_wuhuakuang")
        .unwrap();

    assert_eq!(detail.summary.total_pulls, 2);
    assert_eq!(detail.summary.hit_count, 1);
    assert_eq!(detail.summary.five_star_item_count, 1);
    assert_eq!(detail.summary.up_count, 1);
    assert_eq!(detail.summary.off_rate_count, 0);
    assert_eq!(detail.summary.not_applicable_rate_up_count, 0);
    assert_eq!(detail.summary.unknown_rate_up_count, 0);
    assert_eq!(detail.five_star_history.len(), 1);
    assert_eq!(
        detail.five_star_history[0].record.record_id,
        "standard-character"
    );
    assert!(detail.rarity_distribution.iter().any(|bucket| {
        bucket.rarity == 5 && bucket.count == 1
    }));
    assert!(detail.rarity_distribution.iter().any(|bucket| {
        bucket.rarity == 4 && bucket.count == 1
    }));
    assert!(detail.hit_rarity_distribution.iter().any(|bucket| {
        bucket.rarity == 5 && bucket.count == 1
    }));
    assert_eq!(
        detail
            .pull_rarity_distribution
            .iter()
            .map(|bucket| bucket.count)
            .sum::<u64>(),
        detail.summary.total_pulls
    );
    assert!(detail.pull_rarity_distribution.iter().any(|bucket| {
        bucket.key == PullRarityBucketKey::FiveCharacter
            && bucket.rarity == Some(5)
            && bucket.count == 1
            && bucket.percent == 0.5
    }));
    assert!(detail.pull_rarity_distribution.iter().any(|bucket| {
        bucket.key == PullRarityBucketKey::FourItem
            && bucket.rarity == Some(4)
            && bucket.count == 1
            && bucket.percent == 0.5
    }));
    assert_eq!(standard_banner.rate_up_5_count, 1);
    assert_eq!(fork_rank.rarity, Some(4));
    assert!(fork_rank.item_asset_refs.contains_key("portrait"));
    assert!(!fork_rank.item_asset_refs.contains_key("icon"));
}

#[test]
fn limited_and_standard_pull_rarity_distribution_split_character_and_items() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let document = public_document(vec![
        record(
            "limited-five-character",
            "CardPool_Character",
            "1010",
            "2026-06-02 00:00:00",
        ),
        record(
            "limited-five-item",
            "CardPool_Character",
            "Fashion_vehicle_1010_V008",
            "2026-06-02 00:01:00",
        ),
        record(
            "limited-four-character",
            "CardPool_Character",
            "1008",
            "2026-06-02 00:02:00",
        ),
        record(
            "limited-four-item",
            "CardPool_Character",
            "fork_jiaojuan",
            "2026-06-02 00:03:00",
        ),
        record(
            "standard-five-character",
            "CardPool_NewRole",
            "1003",
            "2026-06-02 00:04:00",
        ),
        record(
            "standard-five-item",
            "CardPool_NewRole",
            "fork_rishi",
            "2026-06-02 00:05:00",
        ),
        record(
            "standard-four-character",
            "CardPool_NewRole",
            "1008",
            "2026-06-02 00:06:00",
        ),
        record(
            "standard-four-item",
            "CardPool_NewRole",
            "fork_jiaojuan",
            "2026-06-02 00:07:00",
        ),
    ]);
    store
        .import_public_document("default", &document, "json", None)
        .unwrap();

    for pool_kind in [PoolKind::MonopolyLimited, PoolKind::MonopolyStandard] {
        let detail = store
            .dashboard_selection_detail(
                "default",
                "zh-Hant",
                &DashboardSelection::PoolKind { pool_kind },
            )
            .unwrap();
        let bucket_count = |key| {
            detail
                .pull_rarity_distribution
                .iter()
                .find(|bucket| bucket.key == key)
                .map(|bucket| bucket.count)
                .unwrap_or_default()
        };

        assert_eq!(detail.summary.total_pulls, 4);
        assert_eq!(bucket_count(PullRarityBucketKey::FiveCharacter), 1);
        assert_eq!(bucket_count(PullRarityBucketKey::FiveItem), 1);
        assert_eq!(bucket_count(PullRarityBucketKey::FourCharacter), 1);
        assert_eq!(bucket_count(PullRarityBucketKey::FourItem), 1);
        assert!(
            detail
                .pull_rarity_distribution
                .iter()
                .all(|bucket| bucket.key != PullRarityBucketKey::FiveUp
                    && bucket.key != PullRarityBucketKey::FourHit)
        );
    }
}

#[test]
fn dashboard_pity_keeps_existing_same_timestamp_analysis_order() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let mut records = Vec::new();
    for source_order in (1..=10).rev() {
        let item_id = if source_order == 2 {
            "1004"
        } else {
            "fork_dustbin"
        };
        let mut record = record(
            &format!("limited-{source_order}"),
            "CardPool_Character",
            item_id,
            "2026-06-09 05:22:09",
        );
        record["source_order"] = serde_json::json!(source_order);
        records.push(record);
    }
    for source_order in (20..=29).rev() {
        let item_id = if source_order == 20 {
            "fork_Rose"
        } else if source_order == 21 {
            "fork_PaperPlane"
        } else {
            "fork_dustbin"
        };
        let mut record = record(
            &format!("fork-{source_order}"),
            "ForkLottery_AnHunQu",
            item_id,
            "2026-06-03 17:15:58",
        );
        record["source_order"] = serde_json::json!(source_order);
        records.push(record);
    }
    let document = public_document(records);
    store
        .import_public_document("default", &document, "json", None)
        .unwrap();

    let overview = store.dashboard_overview("default", "zh-Hant").unwrap();
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

    assert_eq!(limited.current_pity, 8);
    assert_eq!(limited.current_ten_pull_progress, Some(0));
    assert_eq!(fork.current_pity, 9);
    assert_eq!(fork.current_ten_pull_progress, Some(8));
}

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
            .map(|hit| hit.record.record_id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "late-ticket",
            "same-time-character",
            "same-time-sleep-ticket",
            "old-dice",
            "old-character"
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
        .find(|hit| hit.record.record_id == "same-time-sleep-ticket")
        .unwrap();
    assert_eq!(same_time_sleep.record.roll_label.as_deref(), Some("沉眠地"));
    assert!(!same_time_sleep.record.derived.counts_as_pull);
    assert_eq!(
        detail
            .summary
            .latest_5star_any
            .as_ref()
            .map(|record| record.record_id.as_str()),
        Some("late-ticket")
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
                    hit.record.record_id.as_str(),
                    hit.five_star_distance,
                    hit.focused_distance,
                )
            })
            .collect::<Vec<_>>(),
        vec![
            ("dice-2", 1, None),
            ("ticket-2", 1, None),
            ("character", 1, Some(2)),
            ("dice-1", 1, None),
            ("ticket-1", 1, None),
        ]
    );
    assert_eq!(detail.summary.current_pity, 1);
    assert_eq!(detail.summary.current_ten_pull_progress, Some(3));
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
