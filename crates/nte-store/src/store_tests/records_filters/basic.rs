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
    let document = public_document_v21(vec![
        record_with_lottery_id(
            "nanali",
            "CardPool_Character",
            "Lottery_Nanali",
            "Fashion_vehicle_1010_V008",
            Some("2026-05-13 05:59:00"),
        ),
        record_with_lottery_id(
            "xun",
            "CardPool_Character",
            "Lottery_Xun",
            "Fashion_vehicle_1052_V024",
            Some("2026-05-13 05:59:01"),
        ),
        record_with_lottery_id(
            "after",
            "CardPool_Character",
            "Lottery_Canhong",
            "fork_dustbin",
            Some("2026-08-19 05:59:01"),
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
        list.records[0].banner_source.source_id.as_deref(),
        Some("Lottery_Nanali")
    );
    assert_eq!(
        list.records[0].derived.banner_source_id.as_deref(),
        Some("Lottery_Nanali")
    );
    assert_eq!(
        list.records[0].derived.pull_no_in_banner_occurrence,
        Some(1)
    );
    assert!(list.records[0].item_asset_refs.contains_key("icon"));
    assert_eq!(
        list.records[1].banner_source.source_id.as_deref(),
        Some("Lottery_Xun")
    );
    assert_eq!(
        list.records[1].derived.banner_source_id.as_deref(),
        Some("Lottery_Xun")
    );
    assert_eq!(
        list.records[1].derived.pull_no_in_banner_occurrence,
        Some(1)
    );
    assert_eq!(list.records[2].banner_source.resolution_issue, None);
    assert_eq!(
        list.records[2].derived.banner_source_id.as_deref(),
        Some("Lottery_Canhong")
    );
    assert_eq!(
        list.records[2].derived.pull_no_in_banner_occurrence,
        Some(1)
    );
}

#[test]
fn pool_mismatches_stay_visible_but_do_not_join_occurrence_stats_or_filters() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let document = public_document_v21(vec![
        record_with_lottery_id(
            "permanent",
            "CardPool_NewRole",
            "Lottery_Permanent",
            "fork_dustbin",
            Some("2026-08-13 06:00:00"),
        ),
        record_with_lottery_id(
            "limited-with-permanent-id",
            "CardPool_Character",
            "Lottery_Permanent",
            "fork_dustbin",
            Some("2026-08-13 06:01:00"),
        ),
        record_with_lottery_id(
            "standard-with-limited-id",
            "CardPool_NewRole",
            "Lottery_Nanali",
            "fork_dustbin",
            Some("2026-08-13 06:02:00"),
        ),
    ]);
    store
        .import_public_document("default", &document, "json", None)
        .unwrap();

    let list = store
        .list_records("default", "zh-Hant", &RecordFilter::default())
        .unwrap();
    let overview = store.dashboard_overview("default", "zh-Hant").unwrap();
    let options = store.record_filter_options("default", "zh-Hant").unwrap();
    let permanent = list
        .records
        .iter()
        .find(|record| {
            record.pool_id == "CardPool_NewRole"
                && record.lottery_id.as_deref() == Some("Lottery_Permanent")
        })
        .unwrap();
    assert_eq!(list.total, 3);
    assert_eq!(
        permanent.derived.banner_occurrence_id.as_deref(),
        Some("Lottery_Permanent@permanent")
    );

    let mismatches = list
        .records
        .iter()
        .filter(|record| {
            record.banner_source.resolution_issue
                == Some(nte_core::BannerResolutionIssue::LotteryPoolMismatch)
        })
        .collect::<Vec<_>>();
    assert_eq!(mismatches.len(), 2);
    assert!(mismatches.iter().all(|record| {
        record.derived.banner_occurrence_id.is_none()
            && record.banner_occurrence.occurrence_id.is_none()
    }));
    assert!(
        mismatches.iter().any(|record| {
            record.derived.banner_source_id.as_deref() == Some("Lottery_Permanent")
        })
    );
    assert!(
        mismatches
            .iter()
            .any(|record| { record.derived.banner_source_id.as_deref() == Some("Lottery_Nanali") })
    );

    assert_eq!(overview.banner_occurrences.len(), 1);
    assert_eq!(
        overview.banner_occurrences[0].occurrence_id,
        "Lottery_Permanent@permanent"
    );
    assert_eq!(overview.banner_occurrences[0].total_pulls, 1);
    assert_eq!(options.banner_occurrences.len(), 1);
    assert_eq!(
        options.banner_occurrences[0].occurrence_id,
        "Lottery_Permanent@permanent"
    );

    let filtered = store
        .list_records(
            "default",
            "zh-Hant",
            &RecordFilter {
                banner_occurrence_ids: vec!["Lottery_Permanent@permanent".to_string()],
                ..RecordFilter::default()
            },
        )
        .unwrap();
    assert_eq!(filtered.total, 1);
    assert_eq!(filtered.records[0].pool_id, "CardPool_NewRole");

    let limited = overview
        .pool_kinds
        .iter()
        .find(|summary| summary.pool_kind == PoolKind::MonopolyLimited)
        .unwrap();
    let standard = overview
        .pool_kinds
        .iter()
        .find(|summary| summary.pool_kind == PoolKind::MonopolyStandard)
        .unwrap();
    assert_eq!(limited.total_pulls, 1);
    assert_eq!(standard.total_pulls, 2);
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
                banner_occurrence_ids: vec![
                    "ForkLottery_AnHunQu@observed-20260101100000".to_string(),
                ],
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
fn records_list_standard_five_star_results_use_item_kind() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let document = public_document(vec![
        record(
            "standard-1023",
            "CardPool_NewRole",
            "1023",
            "2026-01-01 10:00:00",
        ),
        record(
            "standard-1054",
            "CardPool_NewRole",
            "1054",
            "2026-01-01 10:01:00",
        ),
        record(
            "standard-1055",
            "CardPool_NewRole",
            "1055",
            "2026-01-01 10:02:00",
        ),
        record(
            "standard-item",
            "CardPool_NewRole",
            "DiceNormal",
            "2026-01-01 10:03:00",
        ),
    ]);
    store
        .import_public_document("default", &document, "json", None)
        .unwrap();

    let filtered = |result| {
        store
            .list_records(
                "default",
                "zh-Hant",
                &RecordFilter {
                    pool_kind: Some(PoolKind::MonopolyStandard),
                    rate_up_results: vec![result],
                    sort_direction: Some(SortDirection::Asc),
                    ..RecordFilter::default()
                },
            )
            .unwrap()
    };
    let up = filtered(RateUpResult::Up);
    let off_rate = filtered(RateUpResult::OffRate);
    let not_applicable = filtered(RateUpResult::NotApplicable);
    let detail = store
        .dashboard_selection_detail(
            "default",
            "zh-Hant",
            &DashboardSelection::PoolKind {
                pool_kind: PoolKind::MonopolyStandard,
            },
        )
        .unwrap();

    assert_eq!(
        up.records
            .iter()
            .map(|record| record.item_id.as_str())
            .collect::<Vec<_>>(),
        vec!["1023", "1054", "1055"]
    );
    assert_eq!(off_rate.total, 0);
    assert_eq!(not_applicable.total, 1);
    assert_eq!(not_applicable.records[0].item_id, "DiceNormal");
    assert_eq!(detail.summary.up_count, 3);
    assert_eq!(detail.summary.off_rate_count, 0);
    assert_eq!(detail.summary.not_applicable_rate_up_count, 1);
    assert_eq!(detail.summary.unknown_rate_up_count, 0);
    assert_eq!(detail.summary.observed_up_rate, Some(1.0));
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
