use super::super::*;

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
    assert!(
        detail
            .rarity_distribution
            .iter()
            .any(|bucket| { bucket.rarity == 5 && bucket.count == 2 })
    );
    assert!(
        detail
            .hit_rarity_distribution
            .iter()
            .any(|bucket| { bucket.rarity == 5 && bucket.count == 1 && bucket.percent == 0.25 })
    );
    assert!(
        detail
            .hit_rarity_distribution
            .iter()
            .any(|bucket| { bucket.rarity == 4 && bucket.count == 1 && bucket.percent == 0.25 })
    );
    assert!(
        detail
            .hit_rarity_distribution
            .iter()
            .any(|bucket| { bucket.rarity == 3 && bucket.count == 2 && bucket.percent == 0.5 })
    );
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
    assert!(
        detail
            .rarity_distribution
            .iter()
            .any(|bucket| { bucket.rarity == 5 && bucket.count == 1 })
    );
    assert!(
        detail
            .rarity_distribution
            .iter()
            .any(|bucket| { bucket.rarity == 4 && bucket.count == 1 })
    );
    assert!(
        detail
            .hit_rarity_distribution
            .iter()
            .any(|bucket| { bucket.rarity == 5 && bucket.count == 1 })
    );
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
