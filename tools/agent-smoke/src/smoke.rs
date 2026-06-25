use std::{collections::BTreeSet, fs, path::Path, thread, time::Duration};

use anyhow::{Result, bail};
use serde_json::{Value, json};

use crate::{
    api::{action, assert_agent_ids, click_nav, eval_js, wait_agent, wait_health},
    cli::{APP_TITLE, DEFAULT_KEEP_RUNS, SmokeOptions},
    report::{CleanupReport, ProcessReport, Report, ScreenshotReport, StepReport},
    runtime::{
        default_agent_app_root, ensure_agent_app_fresh, launch_app, new_run_dir,
        prepare_agent_addr, read_agent_build_manifest, remove_portable_copy, rotate_run_dirs,
        stage_portable,
    },
    util::{ensure_file, unix_secs, write_json, write_png},
    window::{
        WindowInfo, capture_window, close_window, find_window, image_metrics, require_windows,
        visible_nte_windows,
    },
};

pub fn run_smoke(options: SmokeOptions) -> Result<()> {
    require_windows()?;
    ensure_agent_app_fresh()?;
    let build = read_agent_build_manifest()?;

    let release_root_input = default_agent_app_root();
    let release_root = release_root_input.canonicalize().map_err(|error| {
        anyhow::anyhow!(
            "release root not found: {}: {error}",
            release_root_input.display()
        )
    })?;
    let run_dir = new_run_dir(&options.out_dir)?;
    let logs = run_dir.join("logs");
    let screenshots = run_dir.join("screenshots");
    fs::create_dir_all(&logs)?;
    fs::create_dir_all(&screenshots)?;

    let portable_root = run_dir.join("portable");
    stage_portable(&release_root, &portable_root)?;
    let launcher = portable_root.join("nte-gacha-exporter.exe");
    let desktop = portable_root
        .join("app")
        .join("nte-gacha-exporter-desktop.exe");
    ensure_file(&launcher)?;
    ensure_file(&desktop)?;

    let mut report = Report {
        schema: "nte-agent-smoke-report",
        schema_version: 1,
        build: Some(build),
        release_root: release_root.display().to_string(),
        staged_portable_root: portable_root.display().to_string(),
        run_dir: run_dir.display().to_string(),
        addr: options.addr.clone(),
        process: ProcessReport::default(),
        steps: Vec::new(),
        screenshots: Vec::new(),
        errors: Vec::new(),
        final_snapshot_summary: None,
        cleanup: CleanupReport {
            keep_runs: DEFAULT_KEEP_RUNS,
            ..CleanupReport::default()
        },
        ok: false,
        finished_at: 0,
    };

    prepare_agent_addr(&options.addr, options.timeout)?;

    let before_windows = visible_nte_windows()?
        .into_iter()
        .map(|window| window.hwnd)
        .collect::<BTreeSet<_>>();
    let mut child = launch_app(&launcher, &portable_root, &options.addr)?;
    report.process.launcher_pid = Some(child.id());
    let mut launched_window: Option<WindowInfo> = None;

    let result = run_smoke_steps(
        &options,
        &logs,
        &screenshots,
        &before_windows,
        &mut report,
        &mut launched_window,
    );

    if let Err(error) = result {
        report.errors.push(error.to_string());
        if let Err(snapshot_error) = capture_failure_snapshot(&options.addr, &logs) {
            report
                .errors
                .push(format!("failure snapshot failed: {snapshot_error}"));
        }
        if let Some(window) = launched_window.as_ref() {
            if let Err(screenshot_error) =
                capture_step(&mut report, &screenshots, "failure", window)
            {
                report
                    .errors
                    .push(format!("screenshot failed: {screenshot_error}"));
            }
        }
    }

    if let Some(window) = launched_window.as_ref() {
        let _ = close_window(window);
    }
    let _ = child.kill();

    match remove_portable_copy(&run_dir, &portable_root) {
        Ok(removed) => report.cleanup.portable_removed = removed,
        Err(error) => report.cleanup.warnings.push(error.to_string()),
    }

    report.ok = report.errors.is_empty();
    report.finished_at = unix_secs();
    write_json(run_dir.join("report.json"), &report)?;
    fs::create_dir_all(&options.out_dir)?;
    write_json(options.out_dir.join("latest-report.json"), &report)?;
    fs::write(
        options.out_dir.join("latest-run.txt"),
        format!("{}\n", run_dir.display()),
    )?;

    match rotate_run_dirs(&options.out_dir, &run_dir, DEFAULT_KEEP_RUNS) {
        Ok(removed) => {
            report.cleanup.removed_run_dirs = removed
                .into_iter()
                .map(|path| path.display().to_string())
                .collect();
        }
        Err(error) => report.cleanup.warnings.push(error.to_string()),
    }
    write_json(run_dir.join("report.json"), &report)?;
    write_json(options.out_dir.join("latest-report.json"), &report)?;

    if report.ok {
        println!(
            "OK agent smoke report: {}",
            options.out_dir.join("latest-report.json").display()
        );
        Ok(())
    } else {
        bail!("agent smoke failed: {}", report.errors.join("; "))
    }
}

fn run_smoke_steps(
    options: &SmokeOptions,
    logs: &Path,
    screenshots: &Path,
    before_windows: &BTreeSet<usize>,
    report: &mut Report,
    launched_window: &mut Option<WindowInfo>,
) -> Result<()> {
    let window = find_window(None, Some(APP_TITLE), Some(before_windows), options.timeout)?;
    report.process.window_pid = Some(window.pid);
    *launched_window = Some(window.clone());
    thread::sleep(Duration::from_secs(1));

    let health_result = wait_health(&options.addr, options.timeout)?;
    push_step(report, "health", Some(health_result));

    let eval_result = eval_js(
        &options.addr,
        "return { title: document.title, href: String(location.href) };",
        5000,
    )?;
    write_json(logs.join("eval-smoke.json"), &eval_result)?;
    push_step(report, "eval", Some(eval_result));

    let snapshot = action(&options.addr, "snapshot", "", Value::Null, 5000)?;
    write_json(logs.join("snapshot-initial.json"), &snapshot)?;
    assert_agent_ids(&snapshot, &["nav-dashboard", "nav-records", "nav-settings"])?;
    push_step(report, "initial_snapshot", None);
    let seeded = seed_dashboard_five_wall_data(&options.addr)?;
    push_step(report, "seed_dashboard_five_wall_data", Some(seeded));
    reload_dashboard(&options.addr)?;
    capture_step(report, screenshots, "dashboard_initial", &window)?;

    capture_navigation_views(&options.addr, screenshots, report, &window)?;
    capture_final_snapshot(&options.addr, logs, report)
}

fn seed_dashboard_five_wall_data(addr: &str) -> Result<Value> {
    action(
        addr,
        "import_public_json_text",
        "",
        Value::String(smoke_public_json()),
        10000,
    )
}

fn smoke_public_json() -> String {
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
    .to_string()
}

fn reload_dashboard(addr: &str) -> Result<()> {
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

fn capture_failure_snapshot(addr: &str, logs: &Path) -> Result<()> {
    let snapshot = action(addr, "snapshot", "", Value::Null, 5000)?;
    write_json(logs.join("snapshot-failure.json"), &snapshot)
}

fn capture_navigation_views(
    addr: &str,
    screenshots: &Path,
    report: &mut Report,
    window: &WindowInfo,
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
            let five_wall_data = audit_dashboard_five_wall_data(addr)?;
            push_step(report, "dashboard_five_wall_data", Some(five_wall_data));
            let dialog = audit_status_dialog(addr)?;
            push_step(report, "layout_status_dialog", Some(dialog));
            let expanded = audit_dashboard_expanded_layout(addr)?;
            push_step(report, "layout_dashboard_expanded", Some(expanded));
        }
        capture_step(report, screenshots, &format!("{view}_after"), window)?;
        push_step(report, format!("view_{view}"), None);
    }
    Ok(())
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

fn audit_dashboard_five_wall_contract(addr: &str) -> Result<Value> {
    eval_js(
        addr,
        r#"
        const fail = (message, detail = {}) => {
          throw new Error(`${message} ${JSON.stringify(detail)}`);
        };
        const round = (value) => Math.round(value * 1000) / 1000;
        const metricsFor = (el) => {
          const rect = el.getBoundingClientRect();
          return {
            width: round(rect.width),
            height: round(rect.height),
            clientHeight: el.clientHeight,
            scrollHeight: el.scrollHeight,
          };
        };
        const fixture = document.createElement("section");
        fixture.className = "panel latest-five-wall";
        fixture.style.cssText = [
          "position: fixed",
          "left: -10000px",
          "top: 0",
          "width: 420px",
          "visibility: hidden",
          "pointer-events: none",
        ].join(";");
        const items = Array.from({ length: 24 }, (_, index) => (
          `<div class="five-wall-item rarity-5"><span class="five-wall-thumb empty">${index}</span><span class="five-wall-pity">${index + 1}</span></div>`
        )).join("");
        fixture.innerHTML = `
          <div class="panel-head"><h2>contract</h2></div>
          <div class="five-wall-shell is-expanded" style="--five-wall-row-height: 74px">
            <div class="five-wall-grid">${items}</div>
            <div class="five-wall-toolbar"><button type="button">toggle</button></div>
          </div>
        `;
        document.body.appendChild(fixture);
        try {
          const shell = fixture.querySelector(".five-wall-shell");
          void shell.offsetHeight;

          const expandedOverflowY = window.getComputedStyle(shell).overflowY;
          const expandedOverflow = shell.scrollHeight - shell.clientHeight;
          if (["auto", "scroll"].includes(expandedOverflowY)) {
            fail("synthetic expanded five wall is configured as inner scroll region", {
              overflowY: expandedOverflowY,
            });
          }
          if (expandedOverflow > 1) {
            fail("synthetic expanded five wall has inner vertical overflow", {
              overflow: round(expandedOverflow),
              shell: metricsFor(shell),
            });
          }

          shell.classList.remove("is-expanded");
          shell.classList.add("is-collapsed");
          void shell.offsetHeight;

          const collapsedOverflowY = window.getComputedStyle(shell).overflowY;
          const collapsedOverflow = shell.scrollHeight - shell.clientHeight;
          if (collapsedOverflowY !== "hidden") {
            fail("synthetic collapsed five wall does not mask preview", {
              overflowY: collapsedOverflowY,
            });
          }
          if (collapsedOverflow <= 1) {
            fail("synthetic collapsed five wall did not create preview overflow", {
              overflow: round(collapsedOverflow),
              shell: metricsFor(shell),
            });
          }

          return {
            expanded: {
              overflow: round(expandedOverflow),
              overflowY: expandedOverflowY,
            },
            collapsed: {
              overflow: round(collapsedOverflow),
              overflowY: collapsedOverflowY,
            },
          };
        } finally {
          fixture.remove();
        }
        "#,
        5000,
    )
}

fn ensure_dashboard_five_star_items_visible(addr: &str) -> Result<Value> {
    let result = eval_js(
        addr,
        r#"
        const toggle = document.querySelector(".latest-item-toggle");
        if (!toggle) return { skipped: true, reason: "toggle not visible" };
        const pressedBefore = toggle.getAttribute("aria-pressed") === "true";
        if (!pressedBefore) toggle.click();
        return { skipped: false, pressedBefore, clicked: !pressedBefore };
        "#,
        5000,
    )?;
    thread::sleep(Duration::from_millis(250));
    Ok(result)
}

fn audit_dashboard_five_wall_data(addr: &str) -> Result<Value> {
    let mut audits = serde_json::Map::new();
    let mut pool_counters = Value::Null;
    for pool_kind in ["monopoly_limited", "monopoly_standard", "fork_lottery"] {
        select_dashboard_pool(addr, pool_kind)?;
        let audit = audit_selected_dashboard_five_wall_data(addr, pool_kind)?;
        if pool_counters.is_null() {
            pool_counters = audit.get("poolCounters").cloned().unwrap_or(Value::Null);
        }
        audits.insert(pool_kind.to_string(), audit);
    }
    Ok(json!({
        "skipped": false,
        "poolCounters": pool_counters,
        "audits": audits,
    }))
}

fn select_dashboard_pool(addr: &str, pool_kind: &str) -> Result<()> {
    let script = format!(
        r#"
        const poolKind = {pool_kind_json};
        const button = document.querySelector(`.pool-strip button[data-pool-kind="${{poolKind}}"]`);
        if (!button) {{
          throw new Error(`dashboard pool button missing ${{poolKind}}`);
        }}
        if (button.getAttribute("aria-pressed") !== "true") button.click();
        return true;
        "#,
        pool_kind_json = serde_json::to_string(pool_kind)?,
    );
    eval_js(addr, &script, 5000)?;
    for _ in 0..20 {
        thread::sleep(Duration::from_millis(100));
        let ready = eval_js(
            addr,
            r#"
            const selectedPoolKind = document.querySelector(".pool-strip button[aria-pressed='true']")?.dataset.poolKind ?? null;
            const itemPoolKinds = [...document.querySelectorAll(".latest-five-wall .five-wall-grid .five-wall-item")]
              .map((item) => item.dataset.poolKind ?? "");
            return { selectedPoolKind, itemPoolKinds };
            "#,
            5000,
        )?;
        let selected = ready
            .get("selectedPoolKind")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let all_items_match = ready
            .get("itemPoolKinds")
            .and_then(Value::as_array)
            .is_some_and(|items| {
                !items.is_empty() && items.iter().all(|item| item.as_str() == Some(pool_kind))
            });
        if selected == pool_kind && all_items_match {
            return Ok(());
        }
    }
    bail!("dashboard pool did not become ready: {pool_kind}")
}

fn audit_selected_dashboard_five_wall_data(addr: &str, expected_pool_kind: &str) -> Result<Value> {
    let expected_pool_kind_json = serde_json::to_string(expected_pool_kind)?;
    let script = r#"
        const fail = (message, detail = {}) => {
          throw new Error(`${message} ${JSON.stringify(detail)}`);
        };
        const expectedPoolKind = __EXPECTED_POOL_KIND__;
        const readSelectedPoolKind = () => document.querySelector(".pool-strip button[aria-pressed='true']")?.dataset.poolKind ?? null;
        const readItems = () => [...document.querySelectorAll(".latest-five-wall .five-wall-grid .five-wall-item")]
          .map((el, index) => ({
            index,
            recordId: el.dataset.recordId ?? "",
            sourceOrder: Number(el.dataset.sourceOrder),
            time: el.dataset.time || null,
            poolKind: el.dataset.poolKind ?? "",
            poolId: el.dataset.poolId ?? "",
            itemId: el.dataset.itemId ?? "",
            rarity: el.dataset.rarity ?? "",
            fiveWallDistance: el.dataset.fiveWallDistance ?? "",
          }));

        const expectedPoolCounters = {
          monopoly_limited: { currentPity: "1", hardPity: "90", currentTenPullProgress: "0" },
          monopoly_standard: { currentPity: "0", hardPity: "90", currentTenPullProgress: "1" },
          fork_lottery: { currentPity: "0", hardPity: "60", currentTenPullProgress: "0" },
        };
        const poolCounters = [...document.querySelectorAll(".pool-strip button[data-pool-kind]")]
          .reduce((acc, button) => {
            acc[button.dataset.poolKind] = {
              currentPity: button.dataset.currentPity ?? "",
              hardPity: button.dataset.hardPity ?? "",
              currentTenPullProgress: button.dataset.currentTenPullProgress ?? "",
            };
            return acc;
          }, {});
        const counterMismatches = Object.entries(expectedPoolCounters)
          .filter(([poolKind, expected]) => JSON.stringify(poolCounters[poolKind] ?? null) !== JSON.stringify(expected))
          .map(([poolKind, expected]) => ({ poolKind, actual: poolCounters[poolKind] ?? null, expected }));
        if (counterMismatches.length) {
          fail("dashboard seeded pity counters mismatch", { counters: counterMismatches });
        }

        const expectedByPool = {
          monopoly_limited: [
            { recordId: "agent-smoke-limited-2", fiveWallDistance: "9" },
            { recordId: "agent-smoke-limited-late-ticket", fiveWallDistance: "3" },
            { recordId: "agent-smoke-limited-late-dice", fiveWallDistance: "2" },
            { recordId: "agent-smoke-limited-old-character", fiveWallDistance: "10" },
            { recordId: "agent-smoke-limited-old-dice", fiveWallDistance: "2" },
          ],
          monopoly_standard: [
            { recordId: "agent-smoke-standard-new-ticket", fiveWallDistance: "2" },
            { recordId: "agent-smoke-standard-guard", fiveWallDistance: "1" },
            { recordId: "agent-smoke-standard-old-ticket", fiveWallDistance: "1" },
          ],
          fork_lottery: [
            { recordId: "agent-smoke-fork-guaranteed", fiveWallDistance: "80" },
            { recordId: "agent-smoke-fork-loss", fiveWallDistance: "1" },
            { recordId: "agent-smoke-fork-20", fiveWallDistance: "10" },
          ],
        };

        const selectedPoolKind = readSelectedPoolKind();
        if (selectedPoolKind !== expectedPoolKind) {
          fail("dashboard seeded pool kind mismatch", { selectedPoolKind, expected: expectedPoolKind });
        }
        const items = readItems();
        if (!items.length) {
          fail("dashboard five wall has no records", { selectedPoolKind });
        }

        const duplicateRecordIds = [...items.reduce((counts, item) => {
          counts.set(item.recordId, (counts.get(item.recordId) ?? 0) + 1);
          return counts;
        }, new Map()).entries()].filter(([, count]) => count > 1);
        if (duplicateRecordIds.length) {
          fail("dashboard five wall has duplicate record ids", {
            poolKind: expectedPoolKind,
            duplicates: duplicateRecordIds.map(([recordId, count]) => ({ recordId, count })),
          });
        }

        const badPoolItems = items.filter((item) => {
          if (item.poolKind !== selectedPoolKind) return true;
          if (item.poolKind === "monopoly_limited") return item.poolId !== "CardPool_Character";
          if (item.poolKind === "monopoly_standard") return item.poolId !== "CardPool_NewRole";
          if (item.poolKind === "fork_lottery") return !item.poolId.startsWith("ForkLottery_");
          return true;
        });
        if (badPoolItems.length) {
          fail("dashboard five wall item pool mismatch", {
            selectedPoolKind,
            items: badPoolItems.slice(0, 8),
          });
        }

        const invalidSourceOrder = items.filter((item) => !Number.isSafeInteger(item.sourceOrder));
        if (invalidSourceOrder.length) {
          fail("dashboard five wall item source_order missing or invalid", {
            poolKind: expectedPoolKind,
            items: invalidSourceOrder.slice(0, 8),
          });
        }

        const expected = [...items].sort((left, right) => {
          if (left.time !== null && right.time === null) return -1;
          if (left.time === null && right.time !== null) return 1;
          const timeOrder = String(right.time ?? "").localeCompare(String(left.time ?? ""));
          return timeOrder || left.sourceOrder - right.sourceOrder || left.recordId.localeCompare(right.recordId);
        });
        const mismatches = items
          .map((item, index) => ({ index, actual: item, expected: expected[index] }))
          .filter((entry) => entry.actual.recordId !== entry.expected.recordId);
        if (mismatches.length) {
          fail("dashboard five wall is not newest-first", {
            poolKind: expectedPoolKind,
            actual: items.slice(0, 12).map((item) => ({
              recordId: item.recordId,
              time: item.time,
              sourceOrder: item.sourceOrder,
              poolKind: item.poolKind,
              poolId: item.poolId,
              itemId: item.itemId,
            })),
            expected: expected.slice(0, 12).map((item) => ({
              recordId: item.recordId,
              time: item.time,
              sourceOrder: item.sourceOrder,
              poolKind: item.poolKind,
              poolId: item.poolId,
              itemId: item.itemId,
            })),
          });
        }

        const phantomItems = items.filter((item) => item.rarity !== "5");
        if (phantomItems.length) {
          fail("dashboard five wall contains non-5-star item", {
            poolKind: expectedPoolKind,
            items: phantomItems.slice(0, 8),
          });
        }

        const expectedSeedItems = expectedByPool[expectedPoolKind];
        const actualSeedItems = items.map((item) => ({
          recordId: item.recordId,
          fiveWallDistance: item.fiveWallDistance,
        }));
        if (JSON.stringify(actualSeedItems) !== JSON.stringify(expectedSeedItems)) {
          fail("dashboard five wall seeded records mismatch", {
            poolKind: expectedPoolKind,
            actual: actualSeedItems,
            expected: expectedSeedItems,
          });
        }

        return {
          selectedPoolKind,
          poolCounters,
          count: items.length,
          first: {
            recordId: items[0].recordId,
            time: items[0].time,
            sourceOrder: items[0].sourceOrder,
            poolKind: items[0].poolKind,
            poolId: items[0].poolId,
            itemId: items[0].itemId,
            fiveWallDistance: items[0].fiveWallDistance,
          },
          last: {
            recordId: items[items.length - 1].recordId,
            time: items[items.length - 1].time,
            sourceOrder: items[items.length - 1].sourceOrder,
            poolKind: items[items.length - 1].poolKind,
            poolId: items[items.length - 1].poolId,
            itemId: items[items.length - 1].itemId,
            fiveWallDistance: items[items.length - 1].fiveWallDistance,
          },
        };
        "#
    .replace("__EXPECTED_POOL_KIND__", &expected_pool_kind_json);
    eval_js(addr, &script, 5000)
}

fn audit_dashboard_scroll_regions(addr: &str, phase: &str) -> Result<Value> {
    let script = format!(
        "const phase = {};\n{}",
        serde_json::to_string(phase)?,
        r#"
        const fail = (message, detail = {}) => {
          throw new Error(`${message} ${JSON.stringify(detail)}`);
        };
        const round = (value) => Math.round(value * 1000) / 1000;
        const metricsFor = (el) => {
          if (!el) return null;
          const rect = el.getBoundingClientRect();
          return {
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
          };
        };

        const workspace = document.querySelector(".workspace");
        const detailBody = document.querySelector(".selected-detail-body");
        const detailPanel = document.querySelector(".selected-detail-panel");
        const fiveWallShell = document.querySelector(".five-wall-shell");
        if (!workspace || !detailBody || !detailPanel || !fiveWallShell) {
          fail("dashboard scroll region missing", {
            phase,
            hasWorkspace: Boolean(workspace),
            hasDetailBody: Boolean(detailBody),
            hasDetailPanel: Boolean(detailPanel),
            hasFiveWallShell: Boolean(fiveWallShell),
          });
        }

        const detailBodyStyle = window.getComputedStyle(detailBody);
        const fiveWallStyle = window.getComputedStyle(fiveWallShell);
        const detailBodyOverflow = detailBody.scrollHeight - detailBody.clientHeight;
        const detailPanelOverflow = detailPanel.scrollHeight - detailPanel.clientHeight;
        const fiveWallOverflow = fiveWallShell.scrollHeight - fiveWallShell.clientHeight;
        const fiveWallClasses = [...fiveWallShell.classList];
        const fiveWallCollapsed = fiveWallShell.classList.contains("is-collapsed");

        if (detailBodyOverflow > 1) {
          fail("dashboard detail body has inner vertical overflow", {
            phase,
            detailBodyOverflow: round(detailBodyOverflow),
            detailBody: metricsFor(detailBody),
          });
        }
        if (["auto", "scroll"].includes(detailBodyStyle.overflowY)) {
          fail("dashboard detail body is configured as inner scroll region", {
            phase,
            overflowY: detailBodyStyle.overflowY,
          });
        }
        if (["auto", "scroll"].includes(fiveWallStyle.overflowY)) {
          fail("dashboard five wall shell is configured as inner scroll region", {
            phase,
            overflowY: fiveWallStyle.overflowY,
            classes: fiveWallClasses,
          });
        }
        if (fiveWallCollapsed && fiveWallStyle.overflowY !== "hidden") {
          fail("dashboard collapsed five wall shell does not mask preview", {
            phase,
            overflowY: fiveWallStyle.overflowY,
            classes: fiveWallClasses,
          });
        }
        if (!fiveWallCollapsed && fiveWallOverflow > 1) {
          fail("dashboard five wall shell has inner vertical overflow", {
            phase,
            fiveWallOverflow: round(fiveWallOverflow),
            classes: fiveWallClasses,
            fiveWallShell: metricsFor(fiveWallShell),
          });
        }

        return {
          phase,
          workspace: {
            ...metricsFor(workspace),
            verticalOverflow: round(workspace.scrollHeight - workspace.clientHeight),
            overflowY: window.getComputedStyle(workspace).overflowY,
          },
          dashboardDetail: {
            body: metricsFor(detailBody),
            panel: metricsFor(detailPanel),
            bodyOverflow: round(detailBodyOverflow),
            panelOverflow: round(detailPanelOverflow),
            bodyOverflowY: detailBodyStyle.overflowY,
          },
          fiveWall: {
            shell: metricsFor(fiveWallShell),
            overflow: round(fiveWallOverflow),
            overflowY: fiveWallStyle.overflowY,
            classes: fiveWallClasses,
          },
        };
        "#,
    );
    eval_js(addr, &script, 5000)
}

fn audit_dashboard_expanded_layout(addr: &str) -> Result<Value> {
    let interaction = eval_js(
        addr,
        r#"
        const toggle = document.querySelector('[data-agent-id="dashboard-five-wall-toggle"]');
        if (!toggle) {
          return { skipped: true, reason: "five wall toggle not visible" };
        }
        const expandedBefore = toggle.getAttribute("aria-expanded") === "true";
        if (!expandedBefore) toggle.click();
        return { skipped: false, expandedBefore, clicked: !expandedBefore };
        "#,
        5000,
    )?;
    thread::sleep(Duration::from_millis(250));

    let skipped = interaction
        .get("skipped")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let expanded_state = eval_js(
        addr,
        r#"
        const toggle = document.querySelector('[data-agent-id="dashboard-five-wall-toggle"]');
        const shell = document.querySelector(".five-wall-shell");
        return {
          hasToggle: Boolean(toggle),
          ariaExpanded: toggle?.getAttribute("aria-expanded") ?? null,
          shellClasses: shell ? [...shell.classList] : [],
          shellExpanded: shell?.classList.contains("is-expanded") ?? false,
        };
        "#,
        5000,
    )?;
    if !skipped
        && !expanded_state
            .get("shellExpanded")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        bail!("dashboard five wall did not enter expanded state: {expanded_state}");
    }

    let regions = audit_dashboard_scroll_regions(
        addr,
        if skipped {
            "expanded-skipped"
        } else {
            "expanded"
        },
    )?;
    let restore = if skipped {
        json!({ "skipped": true })
    } else {
        eval_js(
            addr,
            r#"
            const toggle = document.querySelector('[data-agent-id="dashboard-five-wall-toggle"]');
            if (toggle?.getAttribute("aria-expanded") === "true") toggle.click();
            return { clicked: Boolean(toggle), ariaExpanded: toggle?.getAttribute("aria-expanded") ?? null };
            "#,
            5000,
        )?
    };
    thread::sleep(Duration::from_millis(100));

    Ok(json!({
        "interaction": interaction,
        "expandedState": expanded_state,
        "regions": regions,
        "restore": restore,
    }))
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

fn capture_final_snapshot(addr: &str, logs: &Path, report: &mut Report) -> Result<()> {
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
