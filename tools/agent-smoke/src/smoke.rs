use std::{collections::BTreeSet, fs, path::Path, thread, time::Duration};

use anyhow::{Result, bail};
use serde_json::{Value, json};

use crate::{
    api::{action, assert_agent_ids, click_nav, eval_js, wait_agent, wait_health, wait_text},
    cli::{APP_TITLE, DEFAULT_KEEP_RUNS, DEFAULT_SAMPLE, SmokeOptions},
    report::{CleanupReport, ProcessReport, Report, ScreenshotReport, StepReport},
    runtime::{
        default_agent_app_root, ensure_addr_available, launch_app, new_run_dir,
        remove_portable_copy, rotate_run_dirs, stage_portable,
    },
    util::{ensure_file, unix_secs, write_json, write_png},
    window::{
        WindowInfo, capture_window, close_window, find_window, image_metrics, require_windows,
        visible_nte_windows,
    },
};

pub fn run_smoke(options: SmokeOptions) -> Result<()> {
    require_windows()?;

    let release_root_input = default_agent_app_root();
    let sample_input = options
        .sample
        .as_ref()
        .cloned()
        .unwrap_or_else(|| std::path::PathBuf::from(DEFAULT_SAMPLE));
    let release_root = release_root_input.canonicalize().map_err(|error| {
        anyhow::anyhow!(
            "release root not found: {}: {error}",
            release_root_input.display()
        )
    })?;
    let sample = sample_input.canonicalize().map_err(|error| {
        anyhow::anyhow!("sample not found: {}: {error}", sample_input.display())
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

    ensure_addr_available(&options.addr)?;

    let before_windows = visible_nte_windows()?
        .into_iter()
        .map(|window| window.hwnd)
        .collect::<BTreeSet<_>>();
    let mut child = launch_app(&launcher, &portable_root, &options.addr)?;
    report.process.launcher_pid = Some(child.id());
    let mut launched_window: Option<WindowInfo> = None;

    let result = run_smoke_steps(
        &options,
        &sample,
        &logs,
        &screenshots,
        &before_windows,
        &mut report,
        &mut launched_window,
    );

    if let Err(error) = result {
        report.errors.push(error.to_string());
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
        Ok(())
    } else {
        bail!("agent smoke failed: {}", report.errors.join("; "))
    }
}

fn run_smoke_steps(
    options: &SmokeOptions,
    sample: &Path,
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
    assert_agent_ids(
        &snapshot,
        &[
            "nav-dashboard",
            "nav-records",
            "nav-import_export",
            "nav-settings",
        ],
    )?;
    push_step(report, "initial_snapshot", None);
    capture_step(report, screenshots, "dashboard_initial", &window)?;

    run_import_sample(&options.addr, sample, screenshots, report, &window)?;
    capture_navigation_views(&options.addr, screenshots, report, &window)?;
    capture_final_snapshot(&options.addr, logs, report)
}

fn run_import_sample(
    addr: &str,
    sample: &Path,
    screenshots: &Path,
    report: &mut Report,
    window: &WindowInfo,
) -> Result<()> {
    click_nav(addr, "import_export")?;
    wait_agent(addr, "view-import-export", Duration::from_secs(10))?;
    capture_step(report, screenshots, "import_export_before", window)?;

    action(
        addr,
        "set_input_agent",
        "import-mode",
        Value::String("raw".to_string()),
        5000,
    )?;
    action(
        addr,
        "set_input_agent",
        "import-path",
        Value::String(sample.display().to_string()),
        5000,
    )?;
    action(addr, "click_agent", "import-run", Value::Null, 5000)?;
    wait_text(addr, "Last import", Duration::from_secs(30))?;
    push_step(
        report,
        "import_sample",
        Some(json!({ "sample": sample.display().to_string() })),
    );
    capture_step(report, screenshots, "import_export_after", window)
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
        capture_step(report, screenshots, &format!("{view}_after"), window)?;
        push_step(report, format!("view_{view}"), None);
    }
    Ok(())
}

fn capture_final_snapshot(addr: &str, logs: &Path, report: &mut Report) -> Result<()> {
    let final_snapshot = action(addr, "snapshot", "", Value::Null, 5000)?;
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
