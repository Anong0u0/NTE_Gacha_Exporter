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
        nte_core::BannerResolutionStatus::OutsideKnownWindows
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
