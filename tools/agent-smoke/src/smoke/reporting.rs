use std::path::Path;

use anyhow::{Result, bail};
use serde_json::{Value, json};

use crate::{
    api::{action, assert_agent_ids},
    report::{Report, ScreenshotReport, StepReport},
    util::{write_json, write_png},
    window::{WindowInfo, capture_window, image_metrics},
};

pub(super) fn capture_final_snapshot(addr: &str, logs: &Path, report: &mut Report) -> Result<()> {
    let final_snapshot = action(addr, "snapshot", "", Value::Null, 5000)?;
    assert_agent_ids(
        &final_snapshot,
        &[
            "settings-import-raw",
            "settings-import-public",
            "settings-export-json",
            "settings-export-csv",
            "settings-backup-create",
            "settings-backup-restore",
        ],
    )?;
    write_json(logs.join("snapshot-final.json"), &final_snapshot)?;
    report.final_snapshot_summary = Some(json!({
        "body_text_prefix": final_snapshot
            .get("bodyText")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .chars()
            .take(500)
            .collect::<String>(),
        "agent_id_count": final_snapshot
            .get("agentIds")
            .and_then(Value::as_array)
            .map_or(0, Vec::len),
    }));
    Ok(())
}

pub fn push_step(report: &mut Report, name: impl Into<String>, detail: Option<Value>) {
    report.steps.push(StepReport {
        name: name.into(),
        ok: true,
        detail,
    });
}

pub fn capture_step(
    report: &mut Report,
    screenshots: &Path,
    name: &str,
    window: &WindowInfo,
) -> Result<()> {
    let image = capture_window(window)?;
    let metrics = image_metrics(&image);
    if metrics.width < 200 || metrics.height < 200 {
        bail!("screenshot too small: {}x{}", metrics.width, metrics.height);
    }
    if metrics.is_flat {
        bail!("screenshot flat/blank: variance={}", metrics.variance_score);
    }
    let path = screenshots.join(format!("{name}.png"));
    write_png(&path, &image)?;
    report.screenshots.push(ScreenshotReport {
        name: name.to_string(),
        path: path.display().to_string(),
        window: window.clone(),
        metrics,
    });
    Ok(())
}
