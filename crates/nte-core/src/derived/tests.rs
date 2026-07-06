use super::*;
use crate::load_map;

fn record(record_id: &str, pool_id: &str, item_id: &str, time: &str) -> InternalRecord {
    InternalRecord {
        record_id: record_id.to_string(),
        source_order: 0,
        record_type: if pool_id.starts_with("ForkLottery_") {
            "fork".to_string()
        } else {
            "monopoly".to_string()
        },
        time: Some(time.to_string()),
        pool_id: pool_id.to_string(),
        item_id: item_id.to_string(),
        count: Some(1),
        roll_points: Some(1),
        roll_label_id: None,
        secondary_item_id: None,
        secondary_count: None,
    }
}

#[test]
fn fork_sequence_tracks_pull_pity_rate_up_and_guarantee() {
    let map = load_map("zh-Hant").expect("map should load");
    let records = vec![
        record(
            "r4",
            "ForkLottery_AnHunQu",
            "fork_Rose",
            "2026-01-01 00:03:00",
        ),
        record(
            "r1",
            "ForkLottery_AnHunQu",
            "fork_dustbin",
            "2026-01-01 00:00:00",
        ),
        record(
            "r2",
            "ForkLottery_AnHunQu",
            "fork_Arachne",
            "2026-01-01 00:01:00",
        ),
        record(
            "r3",
            "ForkLottery_AnHunQu",
            "fork_dustbin",
            "2026-01-01 00:02:00",
        ),
    ];

    let derived = derive_records(&records, &map).expect("records should derive");

    assert_eq!(
        derived
            .iter()
            .map(|record| record.record_id.as_str())
            .collect::<Vec<_>>(),
        vec!["r1", "r2", "r3", "r4"]
    );
    assert_eq!(derived[0].pull_no_in_pool_kind, Some(1));
    assert_eq!(derived[1].pull_no_in_pool_kind, Some(2));
    assert_eq!(derived[2].pull_no_in_pool_kind, Some(3));
    assert_eq!(derived[3].pull_no_in_pool_kind, Some(4));
    assert_eq!(derived[1].pity_5_before, 1);
    assert_eq!(derived[1].pity_5_after, 2);
    assert_eq!(derived[1].rate_up_result, RateUpResult::OffRate);
    assert_eq!(derived[1].guarantee_5_before, Some(false));
    assert_eq!(derived[1].guarantee_5_after, Some(true));
    assert_eq!(derived[3].pity_5_before, 1);
    assert_eq!(derived[3].rate_up_result, RateUpResult::Up);
    assert_eq!(derived[3].guarantee_5_before, Some(true));
    assert_eq!(derived[3].guarantee_5_after, Some(false));
}

#[test]
fn limited_rate_up_applies_to_character_domain_only() {
    let map = load_map("zh-Hant").expect("map should load");
    let records = vec![
        record("up", "CardPool_Character", "1010", "2026-05-13 05:57:00"),
        record("off", "CardPool_Character", "1003", "2026-05-13 05:58:00"),
        record(
            "vehicle",
            "CardPool_Character",
            "Fashion_vehicle_1010_V008",
            "2026-05-13 05:59:00",
        ),
    ];

    let derived = derive_records(&records, &map).expect("records should derive");

    assert_eq!(derived[0].rate_up_result, RateUpResult::Up);
    assert_eq!(derived[1].rate_up_result, RateUpResult::OffRate);
    assert_eq!(derived[2].rate_up_result, RateUpResult::NotApplicable);
    assert_eq!(derived[0].hit_rarity, Some(5));
    assert_eq!(derived[1].hit_rarity, Some(5));
    assert_eq!(derived[2].hit_rarity, None);
    assert_eq!(derived[2].pity_5_after, 1);
}

#[test]
fn four_star_hit_keeps_five_star_pity_progress() {
    let map = load_map("zh-Hant").expect("map should load");
    let records = vec![
        record(
            "r1",
            "ForkLottery_AnHunQu",
            "fork_dustbin",
            "2026-01-01 00:00:00",
        ),
        record(
            "r2",
            "ForkLottery_AnHunQu",
            "fork_jiaojuan",
            "2026-01-01 00:01:00",
        ),
    ];

    let derived = derive_records(&records, &map).expect("records should derive");

    assert_eq!(derived[1].hit_rarity, Some(4));
    assert_eq!(derived[1].pity_5_before, 1);
    assert_eq!(derived[1].pity_5_after, 2);
}

#[test]
fn fork_item_in_standard_five_pool_uses_source_quality_for_pity() {
    let map = load_map("zh-Hant").expect("map should load");
    let records = vec![
        record(
            "three",
            "ForkLottery_Nanali",
            "fork_dustbin",
            "2026-01-01 00:00:00",
        ),
        record(
            "forgotten",
            "ForkLottery_Nanali",
            "fork_wuhuakuang",
            "2026-01-01 00:01:00",
        ),
    ];

    let derived = derive_records(&records, &map).expect("records should derive");

    assert_eq!(derived[1].hit_rarity, Some(4));
    assert_eq!(derived[1].pity_5_before, 1);
    assert_eq!(derived[1].pity_5_after, 2);
    assert_eq!(derived[1].ten_pull_progress_after, Some(0));
}

#[test]
fn monopoly_ten_pull_progress_has_before_and_after_state() {
    let map = load_map("zh-Hant").expect("map should load");
    let limited_records = (0..11)
        .map(|index| {
            record(
                &format!("limited-r{index}"),
                "CardPool_Character",
                "1003",
                &format!("2026-05-13 05:{index:02}:00"),
            )
        })
        .collect::<Vec<_>>();
    let standard_records = (0..11)
        .map(|index| {
            record(
                &format!("standard-r{index}"),
                "CardPool_NewRole",
                "1003",
                &format!("2026-01-01 00:{index:02}:00"),
            )
        })
        .collect::<Vec<_>>();

    let limited = derive_records(&limited_records, &map).expect("records should derive");
    let standard = derive_records(&standard_records, &map).expect("records should derive");

    assert_eq!(limited[0].ten_pull_progress_before, Some(1));
    assert_eq!(limited[0].ten_pull_progress_after, Some(1));
    assert_eq!(limited[8].ten_pull_progress_before, Some(9));
    assert_eq!(limited[8].ten_pull_progress_after, Some(9));
    assert_eq!(limited[9].ten_pull_progress_before, Some(10));
    assert_eq!(limited[9].ten_pull_progress_after, Some(0));
    assert_eq!(limited[10].ten_pull_progress_before, Some(1));
    assert_eq!(limited[10].ten_pull_progress_after, Some(1));
    assert_eq!(
        standard
            .iter()
            .map(|record| (
                record.ten_pull_progress_before,
                record.ten_pull_progress_after
            ))
            .collect::<Vec<_>>(),
        limited
            .iter()
            .map(|record| (
                record.ten_pull_progress_before,
                record.ten_pull_progress_after
            ))
            .collect::<Vec<_>>()
    );
}

#[test]
fn fork_ten_pull_progress_resets_on_four_star() {
    let map = load_map("zh-Hant").expect("map should load");
    let mut records = Vec::new();
    for index in 0..9 {
        records.push(record(
            &format!("r{index}"),
            "ForkLottery_AnHunQu",
            "fork_dustbin",
            &format!("2026-01-01 00:{index:02}:00"),
        ));
    }
    records.push(record(
        "r9",
        "ForkLottery_AnHunQu",
        "fork_jiaojuan",
        "2026-01-01 00:09:00",
    ));

    let derived = derive_records(&records, &map).expect("records should derive");

    assert_eq!(derived[8].ten_pull_progress_before, Some(9));
    assert_eq!(derived[8].ten_pull_progress_after, Some(9));
    assert_eq!(derived[9].hit_rarity, Some(4));
    assert_eq!(derived[9].ten_pull_progress_before, Some(10));
    assert_eq!(derived[9].ten_pull_progress_after, Some(0));
    assert_eq!(
        derived[9].pity_badge,
        Some(PityBadge::ForkFourStarGuarantee)
    );
}

#[test]
fn fork_ten_pull_progress_resets_on_five_star() {
    let map = load_map("zh-Hant").expect("map should load");
    let records = vec![
        record(
            "r1",
            "ForkLottery_AnHunQu",
            "fork_dustbin",
            "2026-01-01 00:00:00",
        ),
        record(
            "r2",
            "ForkLottery_AnHunQu",
            "fork_Rose",
            "2026-01-01 00:01:00",
        ),
    ];

    let derived = derive_records(&records, &map).expect("records should derive");

    assert_eq!(derived[0].ten_pull_progress_before, Some(1));
    assert_eq!(derived[0].ten_pull_progress_after, Some(1));
    assert_eq!(derived[1].hit_rarity, Some(5));
    assert_eq!(derived[1].ten_pull_progress_before, Some(2));
    assert_eq!(derived[1].ten_pull_progress_after, Some(0));
}

#[test]
fn non_pull_rows_do_not_advance_ten_pull_progress() {
    let map = load_map("zh-Hant").expect("map should load");
    let mut records = Vec::new();
    for index in 0..11 {
        records.push(record(
            &format!("r{index}"),
            "ForkLottery_AnHunQu",
            "fork_dustbin",
            &format!("2026-01-01 00:{index:02}:00"),
        ));
    }
    records[5].roll_label_id = Some("BPUI_LotteryResult_jidianzengli".to_string());

    let derived = derive_records(&records, &map).expect("records should derive");

    assert_eq!(derived[4].ten_pull_progress_before, Some(5));
    assert_eq!(derived[4].ten_pull_progress_after, Some(5));
    assert_eq!(derived[5].ten_pull_progress_before, None);
    assert_eq!(derived[5].ten_pull_progress_after, None);
    assert_eq!(derived[10].ten_pull_progress_before, Some(10));
    assert_eq!(derived[10].ten_pull_progress_after, Some(9));
}

#[test]
fn fork_pity_badge_priority_prefers_up_then_five_then_four_star() {
    let map = load_map("zh-Hant").expect("map should load");
    let mut four_star_records = Vec::new();
    for index in 0..9 {
        four_star_records.push(record(
            &format!("four-r{index}"),
            "ForkLottery_AnHunQu",
            "fork_dustbin",
            &format!("2026-01-01 00:{index:02}:00"),
        ));
    }
    four_star_records.push(record(
        "four-hit",
        "ForkLottery_AnHunQu",
        "fork_jiaojuan",
        "2026-01-01 00:09:00",
    ));
    let four_star = derive_records(&four_star_records, &map).expect("records should derive");

    let mut five_star_records = Vec::new();
    for index in 0..59 {
        five_star_records.push(record(
            &format!("five-r{index}"),
            "ForkLottery_AnHunQu",
            "fork_dustbin",
            &format!("2026-01-01 01:{index:02}:00"),
        ));
    }
    five_star_records.push(record(
        "five-hit",
        "ForkLottery_AnHunQu",
        "fork_Arachne",
        "2026-01-01 02:00:00",
    ));
    let five_star = derive_records(&five_star_records, &map).expect("records should derive");

    let mut up_records = Vec::new();
    for index in 0..79 {
        up_records.push(record(
            &format!("up-r{index}"),
            "ForkLottery_AnHunQu",
            "fork_dustbin",
            &format!("2026-01-01 03:{:02}:00", index % 60),
        ));
    }
    up_records.push(record(
        "up-hit",
        "ForkLottery_AnHunQu",
        "fork_Rose",
        "2026-01-01 04:00:00",
    ));
    let up = derive_records(&up_records, &map).expect("records should derive");

    assert_eq!(
        four_star
            .last()
            .and_then(|record| record.pity_badge.clone()),
        Some(PityBadge::ForkFourStarGuarantee)
    );
    assert_eq!(
        five_star
            .last()
            .and_then(|record| record.pity_badge.clone()),
        Some(PityBadge::ForkFiveStarGuarantee)
    );
    assert_eq!(
        up.last().and_then(|record| record.pity_badge.clone()),
        Some(PityBadge::ForkUpGuarantee)
    );
}

#[test]
fn missing_rarity_record_stays_and_increments_pity() {
    let map = load_map("zh-Hant").expect("map should load");
    let records = vec![record(
        "missing",
        "CardPool_Character",
        "UnknownItem",
        "2026-05-13 05:59:00",
    )];

    let derived = derive_records(&records, &map).expect("records should derive");

    assert_eq!(derived.len(), 1);
    assert_eq!(derived[0].hit_rarity, None);
    assert_eq!(derived[0].rate_up_result, RateUpResult::Unknown);
    assert_eq!(derived[0].pity_5_after, 1);
}

#[test]
fn same_timestamp_uses_source_order_tiebreaker() {
    let map = load_map("zh-Hant").expect("map should load");
    let mut first = record(
        "same-b",
        "ForkLottery_AnHunQu",
        "DiceNormal",
        "2026-01-01 00:00:00",
    );
    first.source_order = 1;
    let mut second = record(
        "same-a",
        "ForkLottery_AnHunQu",
        "fork_dustbin",
        "2026-01-01 00:00:00",
    );
    second.source_order = 0;
    let records = vec![first, second];

    let derived = derive_records(&records, &map).expect("records should derive");

    assert_eq!(
        derived
            .iter()
            .map(|record| record.record_id.as_str())
            .collect::<Vec<_>>(),
        vec!["same-a", "same-b"]
    );
    assert_eq!(derived[0].pull_no_in_pool_kind, Some(1));
    assert_eq!(derived[1].pull_no_in_pool_kind, Some(2));
    assert_eq!(derived[1].pity_5_before, 1);
}

#[test]
fn limited_boundaries_have_independent_banner_pull_numbers() {
    let map = load_map("zh-Hant").expect("map should load");
    let records = vec![
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
    ];

    let derived = derive_records(&records, &map).expect("records should derive");

    assert_eq!(
        derived[0].banner_id.as_deref(),
        Some("monopoly_limited_Nanali")
    );
    assert_eq!(derived[0].pull_no_in_banner, Some(1));
    assert_eq!(
        derived[1].banner_id.as_deref(),
        Some("monopoly_limited_Xun")
    );
    assert_eq!(derived[1].pull_no_in_banner, Some(1));
}

#[test]
fn outside_known_limited_window_uses_synthetic_banner() {
    let map = load_map("zh-Hant").expect("map should load");
    let records = vec![record(
        "outside",
        "CardPool_Character",
        "fork_dustbin",
        "2026-08-19 05:59:01",
    )];

    let derived = derive_records(&records, &map).expect("records should derive");

    assert_eq!(derived[0].banner_id.as_deref(), Some("CardPool_Character"));
    assert_eq!(derived[0].pull_no_in_banner, Some(1));
    assert_eq!(derived[0].pull_no_in_pool_kind, Some(1));
    assert_eq!(derived[0].pity_5_after, 1);
}

#[test]
fn unknown_time_limited_window_uses_synthetic_banner() {
    let map = load_map("zh-Hant").expect("map should load");
    let mut record = record(
        "unknown-time",
        "CardPool_Character",
        "fork_dustbin",
        "not a time",
    );
    record.time = None;
    let derived = derive_records(&[record], &map).expect("records should derive");

    assert_eq!(derived[0].banner_id.as_deref(), Some("CardPool_Character"));
    assert_eq!(derived[0].pull_no_in_banner, Some(1));
    assert_eq!(derived[0].rate_up_result, RateUpResult::Unknown);
}

#[test]
fn sentinel_rows_stay_visible_but_do_not_advance_pull_state() {
    let map = load_map("zh-Hant").expect("map should load");
    let mut sentinel = record(
        "sentinel",
        "CardPool_Character",
        "1010",
        "2026-05-13 05:58:30",
    );
    sentinel.roll_label_id = Some("BPUI_LotteryResult_jidianzengli".to_string());
    sentinel.roll_points = None;
    let records = vec![
        record("first", "CardPool_Character", "1003", "2026-05-13 05:58:00"),
        sentinel,
        record("after", "CardPool_Character", "1004", "2026-05-13 05:59:00"),
    ];

    let derived = derive_records(&records, &map).expect("records should derive");

    assert_eq!(derived[0].record_id, "first");
    assert_eq!(derived[0].pull_no_in_pool_kind, Some(1));
    assert!(derived[0].counts_as_pull);
    assert_eq!(derived[1].record_id, "sentinel");
    assert!(!derived[1].counts_as_pull);
    assert_eq!(derived[1].pull_no_in_pool_kind, None);
    assert_eq!(derived[1].pull_no_in_banner, None);
    assert_eq!(derived[1].hit_rarity, None);
    assert_eq!(derived[1].pity_5_before, 0);
    assert_eq!(derived[1].pity_5_after, 0);
    assert_eq!(derived[1].ten_pull_progress_before, None);
    assert_eq!(derived[1].ten_pull_progress_after, None);
    assert_eq!(derived[2].pull_no_in_pool_kind, Some(2));
    assert_eq!(derived[2].pity_5_before, 0);
    assert_eq!(derived[2].ten_pull_progress_before, Some(2));
    assert_eq!(derived[2].ten_pull_progress_after, Some(2));
}

#[test]
fn monopoly_pity_resets_only_on_character_hits() {
    let map = load_map("zh-Hant").expect("map should load");
    let records = vec![
        record(
            "vehicle",
            "CardPool_NewRole",
            "Fashion_vehicle_1010_V008",
            "2026-01-01 00:00:00",
        ),
        record(
            "fork",
            "CardPool_NewRole",
            "fork_Rose",
            "2026-01-01 00:01:00",
        ),
        record(
            "character",
            "CardPool_NewRole",
            "1010",
            "2026-01-01 00:02:00",
        ),
    ];

    let derived = derive_records(&records, &map).expect("records should derive");

    assert_eq!(derived[0].hit_rarity, None);
    assert_eq!(derived[0].pity_5_after, 1);
    assert_eq!(derived[1].hit_rarity, None);
    assert_eq!(derived[1].pity_5_after, 2);
    assert_eq!(derived[2].hit_rarity, Some(5));
    assert_eq!(derived[2].pity_5_before, 2);
    assert_eq!(derived[2].pity_5_after, 3);
}

#[test]
fn fork_pity_resets_only_on_fork_hits() {
    let map = load_map("zh-Hant").expect("map should load");
    let records = vec![
        record(
            "character",
            "ForkLottery_AnHunQu",
            "1010",
            "2026-01-01 00:00:00",
        ),
        record(
            "fork",
            "ForkLottery_AnHunQu",
            "fork_Rose",
            "2026-01-01 00:01:00",
        ),
    ];

    let derived = derive_records(&records, &map).expect("records should derive");

    assert_eq!(derived[0].hit_rarity, None);
    assert_eq!(derived[0].pity_5_after, 1);
    assert_eq!(derived[1].hit_rarity, Some(5));
    assert_eq!(derived[1].pity_5_before, 1);
    assert_eq!(derived[1].pity_5_after, 2);
}
