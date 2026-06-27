use super::super::*;

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

pub(super) fn push_fork_dustbin_records(
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
