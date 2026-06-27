use super::super::*;

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
                roll_buckets: vec![RollBucket::Gift, RollBucket::Six, RollBucket::NotApplicable],
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

    assert!(
        options
            .banners
            .iter()
            .any(|banner| banner.banner_id == "ForkLottery_AnHunQu" && banner.count == 1)
    );
    assert!(
        options
            .roll_buckets
            .iter()
            .any(|bucket| bucket.bucket == RollBucket::One && bucket.count == 3)
    );
    assert!(
        options
            .item_kinds
            .iter()
            .any(|item_kind| item_kind.item_kind == ItemKind::Fork && item_kind.count == 3)
    );
}
