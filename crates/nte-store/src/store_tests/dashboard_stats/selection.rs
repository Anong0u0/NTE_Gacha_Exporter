use super::super::*;

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
        .find(|hit| hit.record.item_id == "1052")
        .unwrap();
    let pool_anhunqu_up = pool
        .five_star_wall_history
        .iter()
        .find(|hit| hit.record.item_id == "1004")
        .unwrap();
    let banner_anhunqu_up = banner
        .five_star_wall_history
        .iter()
        .find(|hit| hit.record.item_id == "1004")
        .unwrap();

    assert_eq!(xun_up.focused_distance, Some(9));
    assert_eq!(pool_anhunqu_up.focused_distance, Some(44));
    assert_eq!(banner_anhunqu_up.five_star_distance, 44);
    assert_eq!(banner_anhunqu_up.focused_distance, Some(44));
}

#[test]
fn fork_banner_scope_keeps_no_hit_pulls_from_pool_kind_history() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let mut records = Vec::new();
    for index in 1..=40 {
        records.push(record(
            &format!("old-filler-{index}"),
            "ForkLottery_AnHunQu",
            "fork_dustbin",
            &format!("2026-01-01 00:{:02}:00", index - 1),
        ));
    }
    for index in 1..=19 {
        records.push(record(
            &format!("new-filler-{index}"),
            "ForkLottery_Zhenhong",
            "fork_dustbin",
            &format!("2026-01-02 00:{:02}:00", index - 1),
        ));
    }
    records.push(record(
        "new-up",
        "ForkLottery_Zhenhong",
        "fork_LunarPhase",
        "2026-01-02 00:19:00",
    ));
    store
        .import_public_document("default", &public_document(records), "json", None)
        .unwrap();

    let detail = store
        .dashboard_selection_detail(
            "default",
            "zh-Hant",
            &DashboardSelection::Banner {
                pool_kind: PoolKind::ForkLottery,
                banner_id: "ForkLottery_Zhenhong".to_string(),
            },
        )
        .unwrap();

    assert_eq!(detail.summary.total_pulls, 20);
    assert_eq!(detail.five_star_wall_history.len(), 1);
    let hit = &detail.five_star_wall_history[0];
    assert_eq!(hit.record.item_id, "fork_LunarPhase");
    assert_eq!(hit.record.derived.pull_no_in_pool_kind, Some(60));
    assert_eq!(hit.record.derived.pull_no_in_banner, Some(20));
    assert_eq!(hit.pity_distance, 60);
    assert_eq!(hit.five_star_distance, 60);
    assert_eq!(hit.focused_distance, Some(60));
}

#[test]
fn fork_banner_scope_keeps_off_rate_and_paid_boundary_history() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let mut records = Vec::new();
    for index in 1..=34 {
        records.push(record(
            &format!("old-filler-{index}"),
            "ForkLottery_AnHunQu",
            "fork_dustbin",
            &format!("2026-01-01 00:{:02}:00", index - 1),
        ));
    }
    records.push(record(
        "old-off-rate",
        "ForkLottery_AnHunQu",
        "fork_Arachne",
        "2026-01-01 00:34:00",
    ));
    for index in 36..=40 {
        records.push(record(
            &format!("old-post-{index}"),
            "ForkLottery_AnHunQu",
            "fork_dustbin",
            &format!("2026-01-01 00:{:02}:00", index - 1),
        ));
    }
    for index in 1..=19 {
        records.push(record(
            &format!("new-filler-{index}"),
            "ForkLottery_Zhenhong",
            "fork_dustbin",
            &format!("2026-01-02 00:{:02}:00", index - 1),
        ));
    }
    records.push(record(
        "new-up",
        "ForkLottery_Zhenhong",
        "fork_LunarPhase",
        "2026-01-02 00:19:00",
    ));
    store
        .import_public_document("default", &public_document(records), "json", None)
        .unwrap();

    let detail = store
        .dashboard_selection_detail(
            "default",
            "zh-Hant",
            &DashboardSelection::Banner {
                pool_kind: PoolKind::ForkLottery,
                banner_id: "ForkLottery_Zhenhong".to_string(),
            },
        )
        .unwrap();

    assert_eq!(detail.summary.total_pulls, 20);
    assert_eq!(detail.five_star_wall_history.len(), 1);
    let hit = &detail.five_star_wall_history[0];
    assert_eq!(hit.record.item_id, "fork_LunarPhase");
    assert_eq!(hit.pity_distance, 25);
    assert_eq!(hit.five_star_distance, 25);
    assert_eq!(hit.focused_distance, Some(60));
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
                    hit.record.item_id.as_str(),
                    hit.five_star_distance,
                    hit.focused_distance,
                    hit.result.clone(),
                )
            })
            .collect::<Vec<_>>(),
        vec![
            ("1004", 1, Some(1), FiveStarResult::Up),
            ("1010", 1, Some(2), FiveStarResult::OffRate,),
            (
                "Fashion_vehicle_1010_V008",
                1,
                None,
                FiveStarResult::NotApplicable,
            ),
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
    assert_eq!(view.record_page.records[0].item_id, "fork_Rose");
    assert_eq!(
        view.record_page.records[0].fork_result_mark,
        Some(ForkResultMark::Win)
    );
    assert_eq!(view.record_page.records[0].item_kind, ItemKind::Fork);
    assert_eq!(view.record_page.records[0].roll_bucket, RollBucket::One);
    assert!(
        view.selected_detail
            .five_star_history
            .iter()
            .any(|hit| hit.record.item_id == "fork_Arachne"
                && hit.record.fork_result_mark == Some(ForkResultMark::Lose))
    );
}
