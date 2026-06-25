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
        list.records[2].banner.resolution_issue,
        Some(nte_core::BannerResolutionIssue::OutsideKnownWindows)
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
                banner_ids: vec!["ForkLottery_AnHunQu".to_string()],
                focused_rarities: vec![5],
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
                rate_up_results: vec![RateUpResult::OffRate],
                ..RecordFilter::default()
            },
        )
        .unwrap();

    assert_eq!(banner.total, 1);
    assert_eq!(banner.records[0].record_id, "r3");
    assert_eq!(banner.records[0].derived.guarantee_5_before, Some(true));
    assert_eq!(off_rate.total, 1);
    assert_eq!(off_rate.records[0].record_id, "r2");
}

#[test]
fn records_list_focused_five_star_filter_uses_pool_kind_wall_semantics() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let document = public_document(vec![
        record(
            "standard-character",
            "CardPool_NewRole",
            "1003",
            "2026-01-01 10:00:00",
        ),
        record(
            "standard-fork",
            "CardPool_NewRole",
            "fork_rishi",
            "2026-01-01 10:01:00",
        ),
        record(
            "limited-character",
            "CardPool_Character",
            "1010",
            "2026-06-04 10:00:00",
        ),
        record(
            "limited-item",
            "CardPool_Character",
            "Fashion_vehicle_1010_V008",
            "2026-01-02 10:01:00",
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
                focused_rarities: vec![5],
                sort_direction: Some(SortDirection::Asc),
                ..RecordFilter::default()
            },
        )
        .unwrap();

    assert_eq!(
        list.records
            .iter()
            .map(|record| record.record_id.as_str())
            .collect::<Vec<_>>(),
        vec!["standard-character", "limited-character"]
    );
}

#[test]
fn records_list_filters_by_fork_result_marks() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let mut records = vec![
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
    ];
    push_fork_dustbin_records(&mut records, "guarantee-pull", 11, 79);
    records.push(record(
        "guaranteed",
        "ForkLottery_AnHunQu",
        "fork_Rose",
        "2026-01-01 12:19:00",
    ));
    store
        .import_public_document("default", &public_document(records), "json", None)
        .unwrap();

    let wins = store
        .list_records(
            "default",
            "zh-Hant",
            &RecordFilter {
                fork_result_marks: vec![ForkResultMark::Win, ForkResultMark::Guaranteed],
                sort_direction: Some(SortDirection::Asc),
                ..RecordFilter::default()
            },
        )
        .unwrap();
    let losses = store
        .list_records(
            "default",
            "zh-Hant",
            &RecordFilter {
                fork_result_marks: vec![ForkResultMark::Lose],
                sort_direction: Some(SortDirection::Asc),
                ..RecordFilter::default()
            },
        )
        .unwrap();

    assert_eq!(
        wins.records
            .iter()
            .map(|record| record.record_id.as_str())
            .collect::<Vec<_>>(),
        vec!["win", "guaranteed"]
    );
    assert_eq!(
        wins.records[0].derived.fork_up_pity_before,
        Some(1)
    );
    assert_eq!(
        wins.records[1].derived.fork_up_pity_before,
        Some(79)
    );
    assert_eq!(
        wins.records[1].derived.pity_badge.clone(),
        Some(PityBadge::ForkUpGuarantee)
    );
    assert_eq!(losses.total, 1);
    assert_eq!(losses.records[0].record_id, "lose");
}

#[test]
fn records_list_filters_by_fork_pity_badges() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let mut records = Vec::new();
    push_fork_dustbin_records(&mut records, "up-pull", 10, 79);
    records.push(record(
        "up-guaranteed",
        "ForkLottery_AnHunQu",
        "fork_Rose",
        "2026-01-01 11:19:00",
    ));
    push_fork_dustbin_records(&mut records, "five-pull", 12, 59);
    records.push(record(
        "five-guaranteed",
        "ForkLottery_AnHunQu",
        "fork_Arachne",
        "2026-01-01 12:59:00",
    ));
    push_fork_dustbin_records(&mut records, "four-pull", 13, 9);
    records.push(record(
        "four-guaranteed",
        "ForkLottery_AnHunQu",
        "fork_jiaojuan",
        "2026-01-01 13:09:00",
    ));
    store
        .import_public_document("default", &public_document(records), "json", None)
        .unwrap();

    let up_and_four = store
        .list_records(
            "default",
            "zh-Hant",
            &RecordFilter {
                fork_pity_badges: vec![
                    PityBadge::ForkUpGuarantee,
                    PityBadge::ForkFourStarGuarantee,
                ],
                sort_direction: Some(SortDirection::Asc),
                ..RecordFilter::default()
            },
        )
        .unwrap();
    let five = store
        .list_records(
            "default",
            "zh-Hant",
            &RecordFilter {
                fork_pity_badges: vec![PityBadge::ForkFiveStarGuarantee],
                sort_direction: Some(SortDirection::Asc),
                ..RecordFilter::default()
            },
        )
        .unwrap();

    assert_eq!(
        up_and_four
            .records
            .iter()
            .map(|record| record.record_id.as_str())
            .collect::<Vec<_>>(),
        vec!["up-guaranteed", "four-guaranteed"]
    );
    assert_eq!(
        up_and_four.records[0].derived.pity_badge.clone(),
        Some(PityBadge::ForkUpGuarantee)
    );
    assert_eq!(
        up_and_four.records[1].derived.pity_badge.clone(),
        Some(PityBadge::ForkFourStarGuarantee)
    );
    assert_eq!(five.total, 1);
    assert_eq!(five.records[0].record_id, "five-guaranteed");
    assert_eq!(
        five.records[0].derived.pity_badge.clone(),
        Some(PityBadge::ForkFiveStarGuarantee)
    );
}

#[test]
fn records_list_accepts_frontend_fork_pity_badge_filter_values() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let mut records = Vec::new();
    push_fork_dustbin_records(&mut records, "five-pull", 10, 59);
    records.push(record(
        "five-guaranteed",
        "ForkLottery_AnHunQu",
        "fork_Arachne",
        "2026-01-01 10:59:00",
    ));
    push_fork_dustbin_records(&mut records, "four-pull", 12, 9);
    records.push(record(
        "four-guaranteed",
        "ForkLottery_AnHunQu",
        "fork_jiaojuan",
        "2026-01-01 12:09:00",
    ));
    store
        .import_public_document("default", &public_document(records), "json", None)
        .unwrap();

    let filter: RecordFilter = serde_json::from_value(json!({
        "fork_pity_badges": ["fork_5star_guarantee", "fork_4star_guarantee"],
        "sort_direction": "asc"
    }))
    .unwrap();
    let list = store.list_records("default", "zh-Hant", &filter).unwrap();

    assert_eq!(
        serde_json::to_value(PityBadge::ForkFiveStarGuarantee).unwrap(),
        "fork_5star_guarantee"
    );
    assert_eq!(
        serde_json::to_value(PityBadge::ForkFourStarGuarantee).unwrap(),
        "fork_4star_guarantee"
    );
    assert_eq!(
        list.records
            .iter()
            .map(|record| record.record_id.as_str())
            .collect::<Vec<_>>(),
        vec!["five-guaranteed", "four-guaranteed"]
    );
}

#[test]
fn records_list_exposes_global_pull_no_and_filters_three_star_hits() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let mut sentinel = record(
        "gift",
        "ForkLottery_AnHunQu",
        "fork_Prokaryon",
        "2026-01-01 10:01:00",
    );
    sentinel["roll_label_id"] = serde_json::json!("BPUI_LotteryResult_jidianzengli");
    sentinel["roll_points"] = serde_json::Value::Null;
    let document = public_document(vec![
        record(
            "three",
            "ForkLottery_AnHunQu",
            "fork_dustbin",
            "2026-01-01 10:00:00",
        ),
        sentinel,
        record(
            "five",
            "ForkLottery_AnHunQu",
            "fork_Rose",
            "2026-01-01 10:02:00",
        ),
    ]);
    store
        .import_public_document("default", &document, "json", None)
        .unwrap();

    let chronological = store
        .list_records(
            "default",
            "zh-Hant",
            &RecordFilter {
                sort_direction: Some(SortDirection::Asc),
                ..RecordFilter::default()
            },
        )
        .unwrap();
    let three_star = store
        .list_records(
            "default",
            "zh-Hant",
            &RecordFilter {
                focused_rarities: vec![3, 5],
                sort_direction: Some(SortDirection::Asc),
                ..RecordFilter::default()
            },
        )
        .unwrap();

    assert_eq!(
        chronological
            .records
            .iter()
            .map(|record| record.derived.global_pull_no)
            .collect::<Vec<_>>(),
        vec![Some(1), None, Some(2)]
    );
    assert_eq!(chronological.records[1].record_id, "gift");
    assert!(!chronological.records[1].derived.counts_as_pull);
    assert_eq!(three_star.total, 2);
    assert_eq!(three_star.records[0].record_id, "three");
    assert_eq!(three_star.records[0].derived.hit_rarity, Some(3));
    assert_eq!(three_star.records[1].record_id, "five");
    assert_eq!(three_star.records[1].derived.hit_rarity, Some(5));
}

fn push_fork_dustbin_records(
    records: &mut Vec<serde_json::Value>,
    prefix: &str,
    start_hour: usize,
    count: usize,
) {
    for index in 0..count {
        records.push(record(
            &format!("{prefix}-{index}"),
            "ForkLottery_AnHunQu",
            "fork_dustbin",
            &format!(
                "2026-01-01 {:02}:{:02}:00",
                start_hour + index / 60,
                index % 60
            ),
        ));
    }
}

#[test]
fn records_list_filters_by_roll_buckets_and_item_kinds() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let mut six = record(
        "six",
        "ForkLottery_AnHunQu",
        "fork_Rose",
        "2026-01-01 10:01:00",
    );
    six["roll_points"] = serde_json::json!(6);
    let mut gift = record(
        "gift",
        "ForkLottery_AnHunQu",
        "fork_dustbin",
        "2026-01-01 10:02:00",
    );
    gift["roll_label_id"] = serde_json::json!("BPUI_LotteryResult_jidianzengli");
    gift["roll_points"] = serde_json::Value::Null;
    let mut sleep = record(
        "sleep",
        "ForkLottery_AnHunQu",
        "fork_dustbin",
        "2026-01-01 10:03:00",
    );
    sleep["roll_label_id"] = serde_json::json!("BPUI_LotteryResult_chenmiandi");
    sleep["roll_points"] = serde_json::Value::Null;
    let mut other = record(
        "other-roll",
        "ForkLottery_AnHunQu",
        "fork_dustbin",
        "2026-01-01 10:04:00",
    );
    other["roll_points"] = serde_json::json!(9);
    let document = public_document(vec![
        record("one", "CardPool_Character", "1010", "2026-06-01 10:00:00"),
        six,
        gift,
        sleep,
        other,
        record(
            "appearance",
            "CardPool_Character",
            "Fashion_Glide_1010",
            "2026-06-01 10:05:00",
        ),
        record(
            "vehicle",
            "CardPool_Character",
            "Fashion_vehicle_1010_V008",
            "2026-06-01 10:06:00",
        ),
        record(
            "unknown",
            "CardPool_Character",
            "UnknownItem",
            "2026-06-01 10:07:00",
        ),
    ]);
    store
        .import_public_document("default", &document, "json", None)
        .unwrap();

    let roll_filtered = store
        .list_records(
            "default",
            "zh-Hant",
            &RecordFilter {
                roll_buckets: vec![
                    RollBucket::Gift,
                    RollBucket::Six,
                    RollBucket::NotApplicable,
                ],
                sort_direction: Some(SortDirection::Asc),
                ..RecordFilter::default()
            },
        )
        .unwrap();
    let item_filtered = store
        .list_records(
            "default",
            "zh-Hant",
            &RecordFilter {
                item_kinds: vec![ItemKind::Character, ItemKind::Appearance],
                sort_direction: Some(SortDirection::Asc),
                ..RecordFilter::default()
            },
        )
        .unwrap();
    let rarity_filtered = store
        .list_records(
            "default",
            "zh-Hant",
            &RecordFilter {
                rarities: vec![4, 5],
                sort_direction: Some(SortDirection::Asc),
                ..RecordFilter::default()
            },
        )
        .unwrap();

    assert_eq!(
        roll_filtered
            .records
            .iter()
            .map(|record| record.record_id.as_str())
            .collect::<Vec<_>>(),
        vec!["six", "gift", "other-roll"]
    );
    assert_eq!(
        item_filtered
            .records
            .iter()
            .map(|record| record.record_id.as_str())
            .collect::<Vec<_>>(),
        vec!["one", "appearance"]
    );
    assert_eq!(
        rarity_filtered
            .records
            .iter()
            .map(|record| record.record_id.as_str())
            .collect::<Vec<_>>(),
        vec!["six", "one", "appearance", "vehicle"]
    );
}

#[test]
fn records_list_filters_by_date_sort_and_page() {
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
                date_from: Some("2026-01-02".to_string()),
                date_to: Some("2026-01-03".to_string()),
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
fn records_list_oldest_first_is_exact_newest_first_reverse() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let mut first = record(
        "source-2",
        "ForkLottery_AnHunQu",
        "DiceNormal",
        "2026-01-01 10:00:00",
    );
    first["source_order"] = serde_json::json!(2);
    let mut second = record(
        "source-1",
        "ForkLottery_AnHunQu",
        "fork_dustbin",
        "2026-01-01 10:00:00",
    );
    second["source_order"] = serde_json::json!(1);
    let document = public_document(vec![first, second]);
    store
        .import_public_document("default", &document, "json", None)
        .unwrap();

    let newest_first = store
        .list_records(
            "default",
            "zh-Hant",
            &RecordFilter {
                sort_direction: Some(SortDirection::Desc),
                ..RecordFilter::default()
            },
        )
        .unwrap();
    let chronological = store
        .list_records(
            "default",
            "zh-Hant",
            &RecordFilter {
                sort_direction: Some(SortDirection::Asc),
                ..RecordFilter::default()
            },
        )
        .unwrap();

    assert_eq!(
        newest_first
            .records
            .iter()
            .map(|record| record.record_id.as_str())
            .collect::<Vec<_>>(),
        vec!["source-2", "source-1"]
    );
    assert_eq!(
        chronological
            .records
            .iter()
            .map(|record| record.record_id.as_str())
            .collect::<Vec<_>>(),
        vec!["source-1", "source-2"]
    );
    assert_eq!(chronological.records[0].derived.pity_5_before, 0);
    assert_eq!(chronological.records[1].derived.pity_5_before, 1);
}

#[test]
fn record_filter_options_count_banners_roll_buckets_and_item_kinds() {
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
        .banners
        .iter()
        .any(|banner| banner.banner_id == "ForkLottery_AnHunQu" && banner.count == 1));
    assert!(options
        .roll_buckets
        .iter()
        .any(|bucket| bucket.bucket == RollBucket::One && bucket.count == 3));
    assert!(options
        .item_kinds
        .iter()
        .any(|item_kind| item_kind.item_kind == ItemKind::Fork && item_kind.count == 3));
}
