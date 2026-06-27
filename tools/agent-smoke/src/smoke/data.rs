use anyhow::Result;
use nte_core::{DashboardSelection, PoolKind};
use nte_store::JsonStore;
use serde_json::{Value, json};

use crate::api::action;
use crate::util::unix_secs;

pub(super) struct DashboardFiveWallSeed {
    pub import_report: Value,
    pub expected_dashboard: Value,
}

pub(super) fn seed_dashboard_five_wall_data(addr: &str) -> Result<DashboardFiveWallSeed> {
    let document = smoke_public_document();
    let expected_dashboard = expected_dashboard_five_wall(&document)?;
    let import_report = action(
        addr,
        "import_public_json_text",
        "",
        Value::String(document.to_string()),
        10000,
    )?;
    Ok(DashboardFiveWallSeed {
        import_report,
        expected_dashboard,
    })
}

fn smoke_public_document() -> Value {
    let mut records = Vec::new();
    for index in 1..=8 {
        records.push(json!({
            "record_id": format!("agent-smoke-limited-pad-{index}"),
            "source_order": 300 + index,
            "record_type": "monopoly",
            "time": format!("2026-04-01 00:00:{index:02}"),
            "pool_id": "CardPool_Character",
            "item_id": "fork_dustbin",
            "count": 1,
            "roll_points": 1
        }));
    }
    records.push(json!({
        "record_id": "agent-smoke-limited-old-dice",
        "source_order": 216,
        "record_type": "monopoly",
        "time": "2026-04-30 17:02:07",
        "pool_id": "CardPool_Character",
        "item_id": "Dicelimite",
        "count": 1,
        "roll_points": 6
    }));
    records.push(json!({
        "record_id": "agent-smoke-limited-old-character",
        "source_order": 209,
        "record_type": "monopoly",
        "time": "2026-04-30 17:02:07",
        "pool_id": "CardPool_Character",
        "item_id": "1010",
        "count": 1,
        "roll_points": 1
    }));
    records.push(json!({
        "record_id": "agent-smoke-limited-late-ticket",
        "source_order": 125,
        "record_type": "monopoly",
        "time": "2026-06-03 16:42:17",
        "pool_id": "CardPool_Character",
        "item_id": "Dice_ticket_01",
        "count": 30,
        "roll_points": null,
        "roll_label_id": "BPUI_LotteryResult_chenmiandi"
    }));
    records.push(json!({
        "record_id": "agent-smoke-limited-late-dice",
        "source_order": 129,
        "record_type": "monopoly",
        "time": "2026-06-03 16:42:17",
        "pool_id": "CardPool_Character",
        "item_id": "Dicelimite",
        "count": 1,
        "roll_points": null,
        "roll_label_id": "BPUI_LotteryResult_chenmiandi"
    }));
    for source_order in (1..=10).rev() {
        records.push(json!({
            "record_id": format!("agent-smoke-limited-{source_order}"),
            "source_order": source_order,
            "record_type": "monopoly",
            "time": "2026-06-09 05:22:09",
            "pool_id": "CardPool_Character",
            "item_id": if source_order == 2 { "1004" } else { "fork_dustbin" },
            "count": 1,
            "roll_points": 1
        }));
    }
    records.push(json!({
        "record_id": "agent-smoke-standard-old-ticket",
        "source_order": 402,
        "record_type": "monopoly",
        "time": "2026-06-09 00:00:00",
        "pool_id": "CardPool_NewRole",
        "item_id": "Dice_ticket_01",
        "count": 30,
        "roll_points": null,
        "roll_label_id": "BPUI_LotteryResult_chenmiandi"
    }));
    records.push(json!({
        "record_id": "agent-smoke-standard-guard",
        "source_order": 400,
        "record_type": "monopoly",
        "time": "2026-06-10 00:00:00",
        "pool_id": "CardPool_NewRole",
        "item_id": "1023",
        "count": 1,
        "roll_points": 1
    }));
    records.push(json!({
        "record_id": "agent-smoke-standard-new-ticket",
        "source_order": 401,
        "record_type": "monopoly",
        "time": "2026-06-11 00:00:00",
        "pool_id": "CardPool_NewRole",
        "item_id": "Dice_ticket_01",
        "count": 30,
        "roll_points": null,
        "roll_label_id": "BPUI_LotteryResult_chenmiandi"
    }));
    for source_order in (20..=29).rev() {
        records.push(json!({
            "record_id": format!("agent-smoke-fork-{source_order}"),
            "source_order": source_order,
            "record_type": "fork",
            "time": "2026-06-03 17:15:58",
            "pool_id": "ForkLottery_AnHunQu",
            "item_id": if source_order == 20 {
                "fork_Rose"
            } else if source_order == 21 {
                "fork_PaperPlane"
            } else {
                "fork_dustbin"
            },
            "count": 1,
            "roll_points": 1
        }));
    }
    records.push(json!({
        "record_id": "agent-smoke-fork-loss",
        "source_order": 1001,
        "record_type": "fork",
        "time": "2026-06-04 00:00:01",
        "pool_id": "ForkLottery_AnHunQu",
        "item_id": "fork_Arachne",
        "count": 1,
        "roll_points": 1
    }));
    for index in 1..=78 {
        let offset = index + 1;
        records.push(json!({
            "record_id": format!("agent-smoke-fork-guarantee-pad-{index:02}"),
            "source_order": 1001 + offset,
            "record_type": "fork",
            "time": format!("2026-06-04 00:{:02}:{:02}", offset / 60, offset % 60),
            "pool_id": "ForkLottery_AnHunQu",
            "item_id": "fork_dustbin",
            "count": 1,
            "roll_points": 1
        }));
    }
    records.push(json!({
        "record_id": "agent-smoke-fork-guaranteed",
        "source_order": 1080,
        "record_type": "fork",
        "time": "2026-06-04 00:01:20",
        "pool_id": "ForkLottery_AnHunQu",
        "item_id": "fork_Rose",
        "count": 1,
        "roll_points": 1
    }));

    json!({
        "info": {
            "schema": "nte-gacha-export",
            "schema_version": "2.0",
            "export_app": "agent-smoke",
            "export_app_version": env!("CARGO_PKG_VERSION"),
            "export_timestamp": unix_secs(),
            "locale": "zh-Hant",
            "name_source": "agent-smoke",
            "time_source": "fixed",
            "privacy": "synthetic"
        },
        "nte": {
            "list": records
        }
    })
}

fn expected_dashboard_five_wall(document: &Value) -> Result<Value> {
    let tempdir = tempfile::tempdir()?;
    let store = JsonStore::open(tempdir.path())?;
    let document_text = document.to_string();
    store.import_public_document("default", &document_text, "json", None)?;
    let overview = store.dashboard_overview("default", "zh-Hant")?;
    let pool_kinds = [
        PoolKind::MonopolyLimited,
        PoolKind::MonopolyStandard,
        PoolKind::ForkLottery,
    ];

    let mut pool_counters = serde_json::Map::new();
    let mut expected_by_pool = serde_json::Map::new();
    for pool_kind in pool_kinds {
        let key = pool_kind.as_str();
        let summary = overview
            .pool_kinds
            .iter()
            .find(|summary| summary.pool_kind == pool_kind)
            .ok_or_else(|| anyhow::anyhow!("expected dashboard pool summary missing: {key}"))?;
        pool_counters.insert(
            key.to_string(),
            json!({
                "currentPity": summary.current_pity.to_string(),
                "hardPity": summary.hard_pity.to_string(),
                "currentTenPullProgress": summary
                    .current_ten_pull_progress
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
            }),
        );

        let detail = store.dashboard_selection_detail(
            "default",
            "zh-Hant",
            &DashboardSelection::PoolKind { pool_kind },
        )?;
        let wall_items = detail
            .five_star_wall_history
            .iter()
            .map(|hit| {
                json!({
                    "recordId": hit.record.record_id,
                    "fiveWallDistance": hit.five_star_distance.to_string(),
                })
            })
            .collect::<Vec<_>>();
        expected_by_pool.insert(key.to_string(), Value::Array(wall_items));
    }

    Ok(json!({
        "poolCounters": pool_counters,
        "expectedByPool": expected_by_pool,
    }))
}
