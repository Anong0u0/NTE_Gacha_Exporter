use std::{collections::BTreeSet, fs, path::Path, thread, time::Duration};

use anyhow::{Result, bail};
use serde_json::Value;

use crate::{
    api::{action, assert_agent_ids, eval_js, wait_health},
    cli::{APP_TITLE, DEFAULT_KEEP_RUNS, SmokeOptions},
    report::{CleanupReport, ProcessReport, Report},
    runtime::{
        default_agent_app_root, ensure_agent_app_fresh, launch_app, new_run_dir,
        prepare_agent_addr, read_agent_build_manifest, remove_portable_copy, rotate_run_dirs,
        stage_portable,
    },
    util::{ensure_file, unix_secs, write_json},
    window::{WindowInfo, close_window, find_window, require_windows, visible_nte_windows},
};

mod dashboard;
mod data;
mod navigation;
mod reporting;

use data::seed_dashboard_five_wall_data;
use navigation::{capture_failure_snapshot, capture_navigation_views, reload_dashboard};
use reporting::{capture_final_snapshot, capture_step, push_step};

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
    push_step(
        report,
        "seed_dashboard_five_wall_data",
        Some(seeded.import_report),
    );
    reload_dashboard(&options.addr)?;
    capture_step(report, screenshots, "dashboard_initial", &window)?;

    capture_navigation_views(
        &options.addr,
        screenshots,
        report,
        &window,
        &seeded.expected_dashboard,
    )?;
    capture_final_snapshot(&options.addr, logs, report)
}
