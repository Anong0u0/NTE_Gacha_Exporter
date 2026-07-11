use std::{path::Path, thread, time::Duration};

use anyhow::{Result, bail};
use serde_json::{Value, json};

use crate::{
    api::{action, click_nav, eval_js, wait_agent},
    report::Report,
    util::write_json,
    window::WindowInfo,
};

use super::{
    dashboard::{
        audit_dashboard_expanded_layout, audit_dashboard_five_wall_contract,
        audit_dashboard_five_wall_data, audit_dashboard_scroll_regions,
        ensure_dashboard_five_star_items_visible,
    },
    reporting::{capture_step, push_step},
};

pub(super) fn reload_dashboard(addr: &str) -> Result<()> {
    eval_js(addr, "window.location.reload(); return true;", 5000)?;
    wait_agent(addr, "view-dashboard", Duration::from_secs(10))?;
    wait_dashboard_five_wall_records(addr)?;
    Ok(())
}

fn wait_dashboard_five_wall_records(addr: &str) -> Result<()> {
    let mut last_error = None;
    for _ in 0..40 {
        let count = match eval_js(
            addr,
            r#"
            return document.querySelectorAll(".latest-five-wall .five-wall-grid .five-wall-item").length;
            "#,
            5000,
        ) {
            Ok(value) => value.as_u64().unwrap_or(0),
            Err(error) => {
                last_error = Some(error);
                0
            }
        };
        if count > 0 {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(250));
    }
    if let Some(error) = last_error {
        bail!("dashboard five wall records not visible after seed import: {error}")
    } else {
        bail!("dashboard five wall records not visible after seed import")
    }
}

pub(super) fn capture_failure_snapshot(addr: &str, logs: &Path) -> Result<()> {
    let snapshot = action(addr, "snapshot", "", Value::Null, 5000)?;
    write_json(logs.join("snapshot-failure.json"), &snapshot)
}

pub(super) fn capture_navigation_views(
    addr: &str,
    screenshots: &Path,
    report: &mut Report,
    window: &WindowInfo,
    expected_dashboard: &Value,
) -> Result<()> {
    for view in ["dashboard", "records", "settings"] {
        click_nav(addr, view)?;
        wait_agent(addr, &format!("view-{view}"), Duration::from_secs(10))?;
        let layout = audit_layout(addr, view)?;
        push_step(report, format!("layout_{view}"), Some(layout));
        if view == "dashboard" {
            let regions = audit_dashboard_scroll_regions(addr, "initial")?;
            push_step(report, "layout_dashboard_scroll_regions", Some(regions));
            let five_wall_contract = audit_dashboard_five_wall_contract(addr)?;
            push_step(
                report,
                "layout_dashboard_five_wall_contract",
                Some(five_wall_contract),
            );
            let five_wall_item_toggle = ensure_dashboard_five_star_items_visible(addr)?;
            push_step(
                report,
                "dashboard_five_wall_show_items",
                Some(five_wall_item_toggle),
            );
            let five_wall_data = audit_dashboard_five_wall_data(addr, expected_dashboard)?;
            push_step(report, "dashboard_five_wall_data", Some(five_wall_data));
            let dialog = audit_status_dialog(addr)?;
            push_step(report, "layout_status_dialog", Some(dialog));
            let expanded = audit_dashboard_expanded_layout(addr)?;
            push_step(report, "layout_dashboard_expanded", Some(expanded));
        } else if view == "records" {
            let badges = audit_record_rate_up_badges(addr)?;
            push_step(report, "records_rate_up_badges", Some(badges));
        }
        capture_step(report, screenshots, &format!("{view}_after"), window)?;
        push_step(report, format!("view_{view}"), None);
    }
    Ok(())
}

fn audit_record_rate_up_badges(addr: &str) -> Result<Value> {
    let standard = audit_record_badge(
        addr,
        "monopoly_standard",
        "1023",
        "character",
        5,
        "up",
        None,
    )?;
    let limited = audit_record_badge(
        addr,
        "monopoly_limited",
        "1004",
        "character",
        5,
        "up",
        Some("UP"),
    )?;
    select_record_pool(addr, "all")?;
    wait_record_item(addr, "1023")?;
    Ok(json!({
        "standard": standard,
        "limited": limited,
    }))
}

fn audit_record_badge(
    addr: &str,
    pool_kind: &str,
    item_id: &str,
    expected_item_kind: &str,
    expected_rarity: u8,
    expected_rate_up_result: &str,
    expected_badge: Option<&str>,
) -> Result<Value> {
    select_record_pool(addr, pool_kind)?;
    wait_record_item(addr, item_id)?;
    let script = format!(
        r#"
        const fail = (message, detail = {{}}) => {{
          throw new Error(`${{message}} ${{JSON.stringify(detail)}}`);
        }};
        const itemId = {item_id_json};
        const expected = {{
          itemId,
          poolKind: {pool_kind_json},
          itemKind: {item_kind_json},
          rarity: {rarity},
          rateUpResult: {rate_up_result_json},
          badges: {badges_json},
        }};
        const row = document.querySelector(`.history-line[data-item-id="${{itemId}}"]`);
        if (!row) fail("seeded record row missing", {{ itemId, expected }});
        const actual = {{
          itemId: row.dataset.itemId ?? "",
          poolKind: row.dataset.poolKind ?? "",
          itemKind: row.dataset.itemKind ?? "",
          rarity: Number(row.dataset.rarity),
          rateUpResult: row.dataset.rateUpResult ?? "",
          badges: [...row.querySelectorAll(".derived-chip")]
            .map((badge) => badge.textContent?.trim() ?? ""),
        }};
        if (JSON.stringify(actual) !== JSON.stringify(expected)) {{
          fail("record rate-up badge mismatch", {{ recordId: row.dataset.recordId, actual, expected }});
        }}
        return {{ recordId: row.dataset.recordId ?? "", ...actual }};
        "#,
        item_id_json = serde_json::to_string(item_id)?,
        pool_kind_json = serde_json::to_string(pool_kind)?,
        item_kind_json = serde_json::to_string(expected_item_kind)?,
        rarity = expected_rarity,
        rate_up_result_json = serde_json::to_string(expected_rate_up_result)?,
        badges_json = serde_json::to_string(&expected_badge.into_iter().collect::<Vec<_>>())?,
    );
    eval_js(addr, &script, 5000)
}

fn select_record_pool(addr: &str, pool_kind: &str) -> Result<()> {
    let script = format!(
        r#"
        const poolKind = {pool_kind_json};
        const button = document.querySelector(`[data-record-pool-kind="${{poolKind}}"]`);
        if (!button) throw new Error(`record pool button missing ${{poolKind}}`);
        if (!button.classList.contains("active")) button.click();
        return true;
        "#,
        pool_kind_json = serde_json::to_string(pool_kind)?,
    );
    eval_js(addr, &script, 5000)?;
    for _ in 0..30 {
        thread::sleep(Duration::from_millis(100));
        let selected = eval_js(
            addr,
            &format!(
                r#"
                const poolKind = {pool_kind_json};
                return document.querySelector(`[data-record-pool-kind="${{poolKind}}"]`)?.classList.contains("active") ?? false;
                "#,
                pool_kind_json = serde_json::to_string(pool_kind)?,
            ),
            5000,
        )?;
        if selected.as_bool().unwrap_or(false) {
            return Ok(());
        }
    }
    bail!("record pool did not become active: {pool_kind}")
}

fn wait_record_item(addr: &str, item_id: &str) -> Result<()> {
    let script = format!(
        r#"
        const itemId = {item_id_json};
        return document.querySelector(`.history-line[data-item-id="${{itemId}}"]`) !== null;
        "#,
        item_id_json = serde_json::to_string(item_id)?,
    );
    for _ in 0..30 {
        thread::sleep(Duration::from_millis(100));
        if eval_js(addr, &script, 5000)?.as_bool().unwrap_or(false) {
            return Ok(());
        }
    }
    bail!("seeded record item did not become visible: {item_id}")
}

fn audit_layout(addr: &str, view: &str) -> Result<Value> {
    let script = format!(
        r#"
        const view = {view_json};
        const fail = (message, detail = {{}}) => {{
          throw new Error(`${{message}} ${{JSON.stringify(detail)}}`);
        }};
        const round = (value) => Math.round(value * 1000) / 1000;
        const metricsFor = (el) => {{
          if (!el) return null;
          const rect = el.getBoundingClientRect();
            return {{
              left: round(rect.left),
              top: round(rect.top),
              right: round(rect.right),
              bottom: round(rect.bottom),
              width: round(rect.width),
              height: round(rect.height),
              clientWidth: el.clientWidth,
              scrollWidth: el.scrollWidth,
              clientHeight: el.clientHeight,
              scrollHeight: el.scrollHeight,
            }};
          }};

        const doc = document.documentElement;
        const body = document.body;
        const workspace = document.querySelector(".workspace");
        const workbench = document.querySelector(`[data-agent-id="view-${{view}}"]`);
        if (!workspace || !workbench) fail("layout root missing", {{ view }});

        const viewportWidth = window.innerWidth;
        const viewportHeight = window.innerHeight;
        const docOverflow = Math.max(doc.scrollWidth, body.scrollWidth) - viewportWidth;
        const workspaceOverflow = workspace.scrollWidth - workspace.clientWidth;
        const workspaceVerticalOverflow = workspace.scrollHeight - workspace.clientHeight;
        if (docOverflow > 1) fail("document horizontal overflow", {{ view, docOverflow }});
        if (workspaceOverflow > 1) fail("workspace horizontal overflow", {{ view, workspaceOverflow }});

        const workspaceRect = workspace.getBoundingClientRect();
        const workbenchRect = workbench.getBoundingClientRect();
        const trailingBlank = Math.max(0, workspaceRect.right - workbenchRect.right);
        const trailingBlankRatio = trailingBlank / Math.max(1, workspaceRect.width);
        if (trailingBlankRatio > 0.05) {{
          fail("workbench trailing blank too large", {{
            view,
            trailingBlank: round(trailingBlank),
            trailingBlankRatio: round(trailingBlankRatio),
          }});
        }}

        const clippedControls = Array.from(workbench.querySelectorAll("button, input, select"))
          .filter((el) => !el.closest(".record-table, .banner-thumb-rail"))
          .map((el) => {{
            const rect = el.getBoundingClientRect();
            const style = window.getComputedStyle(el);
            return {{
              tag: el.tagName.toLowerCase(),
              text: (el.innerText || el.value || el.getAttribute("aria-label") || "").trim().slice(0, 80),
              display: style.display,
              visibility: style.visibility,
              width: rect.width,
              height: rect.height,
              left: rect.left,
              right: rect.right,
            }};
          }})
          .filter((item) => item.display !== "none" && item.visibility !== "hidden" && item.width > 1 && item.height > 1)
          .filter((item) => item.left < workspaceRect.left - 1 || item.right > workspaceRect.right + 1);
        if (clippedControls.length) {{
          fail("visible control clipped horizontally", {{
            view,
            controls: clippedControls.slice(0, 6).map((item) => ({{
              tag: item.tag,
              text: item.text,
              left: round(item.left),
              right: round(item.right),
            }})),
          }});
        }}

        const result = {{
          view,
          viewportWidth,
          viewportHeight,
          document: {{ scrollWidth: doc.scrollWidth, overflow: round(docOverflow) }},
          workspace: {{ ...metricsFor(workspace), verticalOverflow: round(workspaceVerticalOverflow) }},
          workbench: metricsFor(workbench),
          trailingBlank: round(trailingBlank),
          trailingBlankRatio: round(trailingBlankRatio),
        }};

        if (view === "dashboard") {{
          const detailBody = document.querySelector(".selected-detail-body");
          const detailPanel = document.querySelector(".selected-detail-panel");
          if (!detailBody || !detailPanel) fail("dashboard detail missing", {{ view }});

          const detailBodyOverflow = detailBody.scrollHeight - detailBody.clientHeight;
          const detailPanelOverflow = detailPanel.scrollHeight - detailPanel.clientHeight;
          const detailBodyStyle = window.getComputedStyle(detailBody);
          if (detailBodyOverflow > 1) {{
            fail("dashboard detail body has inner vertical overflow", {{
              detailBodyOverflow: round(detailBodyOverflow),
              detailBody: metricsFor(detailBody),
            }});
          }}
          if (["auto", "scroll"].includes(detailBodyStyle.overflowY)) {{
            fail("dashboard detail body is configured as inner scroll region", {{
              overflowY: detailBodyStyle.overflowY,
            }});
          }}

          result.dashboardDetail = {{
            body: metricsFor(detailBody),
            panel: metricsFor(detailPanel),
            bodyOverflow: round(detailBodyOverflow),
            panelOverflow: round(detailPanelOverflow),
            bodyOverflowY: detailBodyStyle.overflowY,
          }};
        }}

        if (view === "records") {{
          const table = document.querySelector(".history-table");
          const header = document.querySelector(".history-header");
          if (!table || !header) fail("records table missing", {{ view }});
          const tableOverflow = table.scrollWidth - table.clientWidth;
          if (tableOverflow > 1) fail("records table horizontal overflow", {{ tableOverflow }});
          const tableRect = table.getBoundingClientRect();
          const tableBottomOverflow = tableRect.bottom - workspaceRect.bottom;
          if (tableBottomOverflow > 1) fail("records table exceeds workspace", {{ tableBottomOverflow: round(tableBottomOverflow) }});

          const headerCells = Array.from(header.children);
          const bannerRect = headerCells[2]?.getBoundingClientRect();
          const itemRect = headerCells[3]?.getBoundingClientRect();
          if (!bannerRect || !itemRect) fail("records table header cells missing", {{ view }});

          const itemRatio = itemRect.width / Math.max(1, tableRect.width);
          const itemToBannerRatio = itemRect.width / Math.max(1, bannerRect.width);
          if (itemRatio > 0.34 || itemToBannerRatio > 1.35) {{
            fail("records item column too wide", {{
              itemRatio: round(itemRatio),
              itemToBannerRatio: round(itemToBannerRatio),
            }});
          }}

          result.recordsTable = {{
            table: metricsFor(table),
            overflow: round(tableOverflow),
            bottomOverflow: round(tableBottomOverflow),
            bannerColumnWidth: round(bannerRect.width),
            itemColumnWidth: round(itemRect.width),
            itemRatio: round(itemRatio),
            itemToBannerRatio: round(itemToBannerRatio),
          }};
        }}

        return result;
        "#,
        view_json = serde_json::to_string(view)?,
    );
    eval_js(addr, &script, 5000)
}

fn audit_status_dialog(addr: &str) -> Result<Value> {
    eval_js(
        addr,
        r#"
        const status = document.querySelector('[data-agent-id="topbar-status"]');
        if (!status) throw new Error("topbar status trigger missing");
        status.click();
        return { clicked: true };
        "#,
        5000,
    )?;
    thread::sleep(Duration::from_millis(250));
    let result = eval_js(
        addr,
        r#"
        const fail = (message, detail = {}) => {
          throw new Error(`${message} ${JSON.stringify(detail)}`);
        };
        const round = (value) => Math.round(value * 1000) / 1000;
        const metricsFor = (el) => {
          const rect = el.getBoundingClientRect();
          return {
            left: round(rect.left),
            top: round(rect.top),
            right: round(rect.right),
            bottom: round(rect.bottom),
            width: round(rect.width),
            height: round(rect.height),
          };
        };
        const backdrop = document.querySelector(".status-dialog-backdrop");
        const dialog = document.querySelector(".status-dialog");
        if (!backdrop || !dialog) fail("status dialog missing");

        const backdropRect = backdrop.getBoundingClientRect();
        const dialogRect = dialog.getBoundingClientRect();
        const viewport = { width: window.innerWidth, height: window.innerHeight };
        const tolerances = {
          left: Math.abs(backdropRect.left),
          top: Math.abs(backdropRect.top),
          right: Math.abs(backdropRect.right - viewport.width),
          bottom: Math.abs(backdropRect.bottom - viewport.height),
        };
        if (Object.values(tolerances).some((value) => value > 1)) {
          fail("status dialog backdrop does not cover viewport", {
            viewport,
            backdrop: metricsFor(backdrop),
            tolerances: Object.fromEntries(Object.entries(tolerances).map(([key, value]) => [key, round(value)])),
          });
        }
        if (dialogRect.top < 0 || dialogRect.bottom > viewport.height || dialogRect.left < 0 || dialogRect.right > viewport.width) {
          fail("status dialog outside viewport", {
            viewport,
            dialog: metricsFor(dialog),
          });
        }

        return {
          viewport,
          backdrop: metricsFor(backdrop),
          dialog: metricsFor(dialog),
          parentTag: backdrop.parentElement?.tagName.toLowerCase() ?? null,
        };
        "#,
        5000,
    )?;
    let _ = eval_js(
        addr,
        r#"
        const backdrop = document.querySelector(".status-dialog-backdrop");
        if (backdrop) backdrop.dispatchEvent(new MouseEvent("click", { bubbles: true }));
        return { closed: true };
        "#,
        5000,
    );
    thread::sleep(Duration::from_millis(100));
    Ok(result)
}
