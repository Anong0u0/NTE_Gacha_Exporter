use super::super::*;

#[test]
fn analysis_computes_pity_distribution_and_fork_guarantee() {
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
            "Fashion_vehicle_1010_V008",
            "2026-01-01 10:01:00",
        ),
        record(
            "c3",
            "CardPool_Character",
            "fork_dustbin",
            "2026-01-01 10:02:00",
        ),
        record(
            "c4",
            "CardPool_Character",
            "fork_jiaojuan",
            "2026-01-01 10:03:00",
        ),
        record(
            "f1",
            "ForkLottery_AnHunQu",
            "fork_Arachne",
            "2026-01-02 10:00:00",
        ),
        record(
            "f2",
            "ForkLottery_AnHunQu",
            "fork_Rose",
            "2026-01-02 10:01:00",
        ),
    ]);
    store
        .import_public_document("default", &document, "json", None)
        .unwrap();

    let overview = store.dashboard_overview("default", "zh-Hant").unwrap();
    let limited = overview
        .pool_kinds
        .iter()
        .find(|summary| summary.pool_kind == PoolKind::MonopolyLimited)
        .unwrap();
    let fork = store
        .pool_kind_detail("default", "zh-Hant", PoolKind::ForkLottery)
        .unwrap();
    let limited_detail = store
        .dashboard_selection_detail(
            "default",
            "zh-Hant",
            &DashboardSelection::PoolKind {
                pool_kind: PoolKind::MonopolyLimited,
            },
        )
        .unwrap();

    assert_eq!(overview.total_records, 6);
    assert_eq!(overview.pool_kinds.len(), 3);
    assert_eq!(
        overview
            .pool_kinds
            .iter()
            .map(|summary| summary.roll_points_total)
            .sum::<i64>(),
        6
    );
    assert_eq!(limited.total_pulls, 4);
    assert_eq!(limited.roll_points_total, 4);
    assert_eq!(limited.known_roll_point_records, 4);
    assert_eq!(limited.missing_roll_point_records, 0);
    assert_eq!(limited.hit_count, 0);
    assert_eq!(limited.five_star_item_count, 1);
    assert_eq!(limited.current_pity, 4);
    assert_eq!(limited.average_5star_pity, None);
    assert_eq!(limited.latest_5star, None);
    assert_eq!(
        limited
            .latest_5star_any
            .as_ref()
            .map(|record| record.record_id.as_str()),
        Some(
            expected_record_id_for(
                "CardPool_Character",
                "Fashion_vehicle_1010_V008",
                "2026-01-01 10:01:00"
            )
            .as_str()
        )
    );
    assert_eq!(limited.average_roll_points_to_5star, None);
    assert_eq!(limited.roll_point_cost_samples_5star, 0);
    assert_eq!(limited.early_hit_count, 0);
    assert_eq!(fork.summary.hit_count, 2);
    assert_eq!(fork.summary.roll_points_total, 2);
    assert_eq!(fork.summary.known_roll_point_records, 2);
    assert_eq!(fork.summary.missing_roll_point_records, 0);
    assert_eq!(fork.summary.off_rate_count, 1);
    assert_eq!(fork.summary.up_count, 1);
    assert_eq!(fork.summary.early_hit_count, 2);
    assert_eq!(fork.five_star_history[0].guarantee_after, Some(true));
    assert_eq!(fork.five_star_history[1].guarantee_before, Some(true));
    assert_eq!(fork.five_star_history[1].guarantee_after, Some(false));
    assert_eq!(
        fork.five_star_history[1]
            .record
            .derived
            .banner_id
            .as_deref(),
        Some("ForkLottery_AnHunQu")
    );
    assert_eq!(fork.five_star_history[1].record.derived.hit_rarity, Some(5));
    assert_eq!(fork.five_star_wall_history.len(), 2);
    assert_eq!(limited_detail.five_star_history.len(), 0);
    assert_eq!(limited_detail.five_star_wall_history.len(), 1);
    assert_eq!(
        limited_detail.five_star_wall_history[0]
            .record
            .record_id
            .as_str(),
        expected_record_id_for(
            "CardPool_Character",
            "Fashion_vehicle_1010_V008",
            "2026-01-01 10:01:00"
        )
        .as_str()
    );
    assert!(
        overview
            .rarity_distribution
            .iter()
            .any(|bucket| bucket.rarity == 5 && bucket.count == 3)
    );
    assert_eq!(
        limited_detail
            .pull_rarity_distribution
            .iter()
            .map(|bucket| bucket.count)
            .sum::<u64>(),
        limited_detail.summary.total_pulls
    );
    assert!(
        limited_detail
            .pull_rarity_distribution
            .iter()
            .any(|bucket| {
                bucket.key == PullRarityBucketKey::FiveItem
                    && bucket.rarity == Some(5)
                    && bucket.count == 1
                    && bucket.percent == 0.25
            })
    );
    assert!(
        limited_detail
            .pull_rarity_distribution
            .iter()
            .any(|bucket| {
                bucket.key == PullRarityBucketKey::FourItem
                    && bucket.rarity == Some(4)
                    && bucket.count == 1
                    && bucket.percent == 0.25
            })
    );
    assert!(
        limited_detail
            .pull_rarity_distribution
            .iter()
            .any(|bucket| {
                bucket.key == PullRarityBucketKey::Three
                    && bucket.rarity == Some(3)
                    && bucket.count == 2
                    && bucket.percent == 0.5
            })
    );
    assert!(
        limited_detail
            .pull_rarity_distribution
            .iter()
            .all(|bucket| bucket.key != PullRarityBucketKey::FiveUp)
    );
}

#[test]
fn dashboard_overview_empty_profile_returns_empty_stats() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();

    let overview = store.dashboard_overview("default", "zh-Hant").unwrap();
    let limited = overview
        .pool_kinds
        .iter()
        .find(|summary| summary.pool_kind == PoolKind::MonopolyLimited)
        .unwrap();

    assert_eq!(overview.total_records, 0);
    assert_eq!(overview.pool_kinds.len(), 3);
    assert_eq!(limited.total_pulls, 0);
    assert_eq!(limited.roll_points_total, 0);
    assert!(overview.banners.is_empty());
    assert_eq!(
        overview
            .pool_kinds
            .iter()
            .map(|summary| summary.roll_points_total)
            .sum::<i64>(),
        0
    );
    assert!(overview.time_stats.monthly.is_empty());
    assert!(overview.time_stats.daily.is_empty());
    assert_eq!(overview.time_stats.missing_time_records, 0);
}

#[test]
fn item_ranking_splits_same_item_by_reward_quantity() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let mut dustbin_30_a = record(
        "dustbin-30-a",
        "CardPool_Character",
        "fork_dustbin",
        "2026-01-01 10:00:00",
    );
    let mut dustbin_30_b = record(
        "dustbin-30-b",
        "CardPool_Character",
        "fork_dustbin",
        "2026-01-01 10:01:00",
    );
    let mut dustbin_50 = record(
        "dustbin-50",
        "CardPool_Character",
        "fork_dustbin",
        "2026-01-01 10:02:00",
    );
    dustbin_30_a["count"] = json!(30);
    dustbin_30_b["count"] = json!(30);
    dustbin_50["count"] = json!(50);
    let document = public_document(vec![dustbin_30_a, dustbin_30_b, dustbin_50]);
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
    let dustbin_ranks = detail
        .item_ranking
        .iter()
        .filter(|item| item.item_id == "fork_dustbin")
        .collect::<Vec<_>>();
    let dustbin_30 = dustbin_ranks
        .iter()
        .find(|item| item.reward_count == 30)
        .unwrap();
    let dustbin_50 = dustbin_ranks
        .iter()
        .find(|item| item.reward_count == 50)
        .unwrap();

    assert_eq!(dustbin_ranks.len(), 2);
    assert_eq!(dustbin_30.count, 2);
    assert_eq!(dustbin_50.count, 1);
    assert_eq!(dustbin_30.item_name, dustbin_50.item_name);
}

#[test]
fn item_ranking_returns_all_ranked_items() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let mut records = Vec::new();
    for index in 0..25 {
        records.push(record(
            &format!("rank-{index:02}"),
            "CardPool_Character",
            &format!("rank_item_{index:02}"),
            &format!("2026-01-01 10:{index:02}:00"),
        ));
    }
    records.push(record(
        "rank-repeat-a",
        "CardPool_Character",
        "rank_item_24",
        "2026-01-01 11:00:00",
    ));
    records.push(record(
        "rank-repeat-b",
        "CardPool_Character",
        "rank_item_24",
        "2026-01-01 11:01:00",
    ));
    let document = public_document(records);
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

    assert_eq!(detail.item_ranking.len(), 25);
    assert_eq!(detail.item_ranking[0].item_id, "rank_item_24");
    assert_eq!(detail.item_ranking[0].count, 3);
    assert!(
        detail
            .item_ranking
            .iter()
            .any(|item| item.item_id == "rank_item_00")
    );
}

#[test]
fn fork_stats_separate_twenty_five_seventy_five_wins_losses_and_forced_up() {
    let tmp = tempfile::tempdir().unwrap();
    let store = JsonStore::open(tmp.path()).unwrap();
    let mut records = Vec::new();
    records.push(record(
        "r001-loss",
        "ForkLottery_AnHunQu",
        "fork_Arachne",
        "2026-01-01 00:01:00",
    ));
    for index in 2..60 {
        records.push(record(
            &format!("r{index:03}"),
            "ForkLottery_AnHunQu",
            "fork_dustbin",
            &format!("2026-01-01 00:{index:02}:00"),
        ));
    }
    records.push(record(
        "r060-loss",
        "ForkLottery_AnHunQu",
        "fork_Arachne",
        "2026-01-01 01:00:00",
    ));
    for index in 61..80 {
        records.push(record(
            &format!("r{index:03}"),
            "ForkLottery_AnHunQu",
            "fork_dustbin",
            &format!("2026-01-01 01:{:02}:00", index - 60),
        ));
    }
    records.push(record(
        "r080-forced",
        "ForkLottery_AnHunQu",
        "fork_Rose",
        "2026-01-01 02:00:00",
    ));
    let document = public_document(records);
    store
        .import_public_document("default", &document, "json", None)
        .unwrap();

    let overview = store.dashboard_overview("default", "zh-Hant").unwrap();
    let fork = overview
        .pool_kinds
        .iter()
        .find(|summary| summary.pool_kind == PoolKind::ForkLottery)
        .unwrap();
    let banner = overview
        .banners
        .iter()
        .find(|banner| banner.banner_id == "ForkLottery_AnHunQu")
        .unwrap();
    let detail = store
        .pool_kind_detail("default", "zh-Hant", PoolKind::ForkLottery)
        .unwrap();
    let forced = detail
        .five_star_history
        .iter()
        .find(|hit| hit.record.item_id == "fork_Rose")
        .unwrap();
    let forced_display = detail
        .five_star_wall_history
        .iter()
        .find(|hit| hit.record.item_id == "fork_Rose")
        .unwrap();
    let loss_display = detail
        .five_star_wall_history
        .iter()
        .find(|hit| hit.record.time.as_deref() == Some("2026-01-01 01:00:00"))
        .unwrap();

    assert_eq!(fork.hit_count, 3);
    assert_eq!(fork.hard_pity, 60);
    assert_eq!(fork.fork_win_count, 0);
    assert_eq!(fork.fork_loss_count, 2);
    assert_eq!(fork.fork_forced_up_count, 1);
    assert_eq!(fork.fork_observed_25_75_win_rate, Some(0.0));
    assert_eq!(banner.fork_loss_count, 2);
    assert_eq!(banner.fork_forced_up_count, 1);
    assert_eq!(forced.record.derived.fork_up_pity_before, Some(79));
    assert_eq!(forced.record.derived.fork_forced_up, Some(true));
    assert_eq!(forced.pity_distance, 20);
    assert_eq!(forced_display.five_star_distance, 20);
    assert_eq!(forced_display.focused_distance, Some(80));
    assert_eq!(loss_display.five_star_distance, loss_display.pity_distance);
}
