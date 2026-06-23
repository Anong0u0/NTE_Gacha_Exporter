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
    capture_step(report, screenshots, "dashboard_initial", &window)?;

    capture_navigation_views(&options.addr, screenshots, report, &window)?;
    capture_final_snapshot(&options.addr, logs, report)
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
