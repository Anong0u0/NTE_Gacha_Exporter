use super::super::*;
use super::fork::push_fork_dustbin_records;

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
    assert!(
        list.records
            .iter()
            .any(|record| record.item_id == "UnknownItem" && record.rarity.is_none())
    );
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
    assert_eq!(banner.records[0].item_id, "fork_Rose");
    assert_eq!(banner.records[0].derived.guarantee_5_before, Some(true));
    assert_eq!(off_rate.total, 1);
    assert_eq!(off_rate.records[0].item_id, "fork_Arachne");
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
            .map(|record| record.item_id.as_str())
            .collect::<Vec<_>>(),
        vec!["1003", "1010"]
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
            .map(|record| record.fork_result_mark)
            .collect::<Vec<_>>(),
        vec![Some(ForkResultMark::Win), Some(ForkResultMark::Guaranteed)]
    );
    assert_eq!(wins.records[0].derived.fork_up_pity_before, Some(1));
    assert_eq!(wins.records[1].derived.fork_up_pity_before, Some(79));
    assert_eq!(
        wins.records[1].derived.pity_badge.clone(),
        Some(PityBadge::ForkUpGuarantee)
    );
    assert_eq!(losses.total, 1);
    assert_eq!(
        losses.records[0].fork_result_mark,
        Some(ForkResultMark::Lose)
    );
}
