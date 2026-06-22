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
                hit_rarities: vec![5],
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
                hit_rarities: vec![3, 5],
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
fn records_list_keeps_source_order_inside_same_timestamp_for_time_sort() {
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
        vec!["source-1", "source-2"]
    );
    assert_eq!(
        chronological
            .records
            .iter()
            .map(|record| record.record_id.as_str())
            .collect::<Vec<_>>(),
        vec!["source-1", "source-2"]
    );
    assert_eq!(chronological.records[0].derived.pity_5_before, 1);
    assert_eq!(chronological.records[1].derived.pity_5_before, 0);
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
