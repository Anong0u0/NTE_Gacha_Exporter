use std::{
    collections::{BTreeSet, HashSet},
    fs,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand};
use image::RgbImage;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const DEFAULT_ADDR: &str = "127.0.0.1:17365";
const DEFAULT_OUT_DIR: &str = "target/agent-smoke";
const DEFAULT_SAMPLE: &str = "fixtures/sample.raw.jsonl";
const DEFAULT_SMOKE_RELEASE_ROOT: &str = "target/agent-smoke/smoke-input-current";
const DEFAULT_KEEP_RUNS: usize = 1;
const APP_TITLE: &str = "NTE Gacha Exporter";

#[derive(Debug, Parser)]
#[command(name = "nte-agent-smoke")]
#[command(about = "Agent-operated release smoke runner for NTE Gacha Exporter")]
struct Cli {
    #[command(subcommand)]
    command: CommandKind,
}

#[derive(Debug, Subcommand)]
enum CommandKind {
    Smoke {
        #[arg(long)]
        release_root: Option<PathBuf>,
        #[arg(long)]
        sample: Option<PathBuf>,
        #[arg(long, default_value = DEFAULT_OUT_DIR)]
        out_dir: PathBuf,
        #[arg(long, default_value = DEFAULT_ADDR)]
        addr: String,
        #[arg(long, default_value_t = 30)]
        timeout_secs: u64,
        #[arg(long)]
        keep_app: bool,
        #[arg(long, default_value_t = DEFAULT_KEEP_RUNS)]
        keep_runs: usize,
        #[arg(long)]
        keep_portable: bool,
    },
    Health {
        #[arg(long, default_value = DEFAULT_ADDR)]
        addr: String,
    },
    Snapshot {
        #[arg(long, default_value = DEFAULT_ADDR)]
        addr: String,
    },
    Ids {
        #[arg(long, default_value = DEFAULT_ADDR)]
        addr: String,
        #[arg(long)]
        plain: bool,
    },
    Inspect {
        #[arg(long, default_value = DEFAULT_ADDR)]
        addr: String,
        #[arg(long)]
        agent_id: String,
        #[arg(long)]
        plain: bool,
    },
    Wait {
        #[arg(long, default_value = DEFAULT_ADDR)]
        addr: String,
        #[arg(long)]
        agent_id: String,
        #[arg(long, default_value_t = 10)]
        timeout_secs: u64,
    },
    ExpectText {
        text: String,
        #[arg(long, default_value = DEFAULT_ADDR)]
        addr: String,
        #[arg(long, default_value_t = 10)]
        timeout_secs: u64,
    },
    Click {
        #[arg(long, default_value = DEFAULT_ADDR)]
        addr: String,
        #[arg(long)]
        agent_id: String,
    },
    Set {
        #[arg(long, default_value = DEFAULT_ADDR)]
        addr: String,
        #[arg(long)]
        agent_id: String,
        #[arg(long)]
        value: String,
    },
    Eval {
        #[arg(long, default_value = DEFAULT_ADDR)]
        addr: String,
        #[arg(long)]
        script: String,
        #[arg(long, default_value_t = 5000)]
        timeout_ms: u64,
    },
    Screenshot {
        #[arg(long)]
        pid: Option<u32>,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Debug, Deserialize)]
struct ApiResponse {
    ok: bool,
    result: Option<Value>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    schema_version: u32,
    release_root: String,
    staged_portable_root: String,
    run_dir: String,
    addr: String,
    process: ProcessReport,
    steps: Vec<StepReport>,
    screenshots: Vec<ScreenshotReport>,
    errors: Vec<String>,
    final_snapshot_summary: Option<Value>,
    cleanup: CleanupReport,
    ok: bool,
    finished_at: u64,
}

#[derive(Debug, Default, Serialize)]
struct ProcessReport {
    launcher_pid: Option<u32>,
    window_pid: Option<u32>,
}

#[derive(Debug, Serialize)]
struct StepReport {
    name: String,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<Value>,
}

#[derive(Debug, Serialize)]
struct ScreenshotReport {
    name: String,
    path: String,
    window: WindowInfo,
    metrics: ImageMetrics,
}

#[derive(Debug, Clone, Serialize)]
struct WindowInfo {
    hwnd: usize,
    pid: u32,
    title: String,
    rect: [i32; 4],
}

#[derive(Debug, Serialize)]
struct ImageMetrics {
    width: u32,
    height: u32,
    mean: [f64; 3],
    extrema: [[u8; 2]; 3],
    variance_score: f64,
    is_flat: bool,
}

#[derive(Debug, Clone, Serialize)]
struct AgentElement {
    id: String,
    tag: String,
    text: String,
    disabled: bool,
}

#[derive(Debug, Serialize)]
struct AgentIdsOutput {
    ids: Vec<AgentElement>,
}

#[derive(Debug, Serialize)]
struct ExpectTextOutput {
    text: String,
    matched: bool,
    elapsed_ms: u128,
}

#[derive(Debug, Default, Serialize)]
struct CleanupReport {
    keep_runs: usize,
    keep_portable: bool,
    portable_removed: bool,
    removed_run_dirs: Vec<String>,
    warnings: Vec<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        CommandKind::Smoke {
            release_root,
            sample,
            out_dir,
            addr,
            timeout_secs,
            keep_app,
            keep_runs,
            keep_portable,
        } => run_smoke(SmokeOptions {
            release_root,
            sample,
            out_dir,
            addr,
            timeout: Duration::from_secs(timeout_secs),
            keep_app,
            keep_runs,
            keep_portable,
        }),
        CommandKind::Health { addr } => {
            print_json(&health(&addr)?)?;
            Ok(())
        }
        CommandKind::Snapshot { addr } => {
            print_json(&snapshot(&addr)?)?;
            Ok(())
        }
        CommandKind::Ids { addr, plain } => {
            let ids = agent_elements(&snapshot(&addr)?);
            if plain {
                print_agent_elements_plain(&ids);
            } else {
                print_json(&AgentIdsOutput { ids })?;
            }
            Ok(())
        }
        CommandKind::Inspect {
            addr,
            agent_id,
            plain,
        } => {
            let element = find_agent_element(&snapshot(&addr)?, &agent_id)
                .ok_or_else(|| anyhow!("agent id not visible: {agent_id}"))?;
            if plain {
                print_agent_element_plain(&element);
            } else {
                print_json(&element)?;
            }
            Ok(())
        }
        CommandKind::Wait {
            addr,
            agent_id,
            timeout_secs,
        } => {
            print_json(&wait_agent_element(
                &addr,
                &agent_id,
                Duration::from_secs(timeout_secs),
            )?)?;
            Ok(())
        }
        CommandKind::ExpectText {
            addr,
            text,
            timeout_secs,
        } => {
            print_json(&expect_text(
                &addr,
                &text,
                Duration::from_secs(timeout_secs),
            )?)?;
            Ok(())
        }
        CommandKind::Click { addr, agent_id } => {
            print_json(&action(&addr, "click_agent", &agent_id, Value::Null, 5000)?)?;
            Ok(())
        }
        CommandKind::Set {
            addr,
            agent_id,
            value,
        } => {
            print_json(&action(
                &addr,
                "set_input_agent",
                &agent_id,
                Value::String(value),
                5000,
            )?)?;
            Ok(())
        }
        CommandKind::Eval {
            addr,
            script,
            timeout_ms,
        } => {
            print_json(&eval_js(&addr, &script, timeout_ms)?)?;
            Ok(())
        }
        CommandKind::Screenshot { pid, title, out } => {
            let window = find_window(pid, title.as_deref(), None, Duration::from_secs(10))?;
            let image = capture_window(&window)?;
            write_png(&out, &image)?;
            let record = ScreenshotReport {
                name: "screenshot".to_string(),
                path: out.display().to_string(),
                window,
                metrics: image_metrics(&image),
            };
            print_json(&record)?;
            Ok(())
        }
    }
}

struct SmokeOptions {
    release_root: Option<PathBuf>,
    sample: Option<PathBuf>,
    out_dir: PathBuf,
    addr: String,
    timeout: Duration,
    keep_app: bool,
    keep_runs: usize,
    keep_portable: bool,
}

fn run_smoke(options: SmokeOptions) -> Result<()> {
    require_windows()?;
    if options.keep_runs == 0 {
        bail!("--keep-runs must be at least 1");
    }
    let keep_portable = options.keep_portable || options.keep_app;

    let release_root_input = options.release_root.unwrap_or(default_release_root()?);
    let sample_input = options
        .sample
        .unwrap_or_else(|| PathBuf::from(DEFAULT_SAMPLE));
    let release_root = release_root_input
        .canonicalize()
        .with_context(|| format!("release root not found: {}", release_root_input.display()))?;
    let sample = sample_input
        .canonicalize()
        .with_context(|| format!("sample not found: {}", sample_input.display()))?;
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
            keep_runs: options.keep_runs,
            keep_portable,
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

    let result = (|| -> Result<()> {
        let window = find_window(
            None,
            Some(APP_TITLE),
            Some(&before_windows),
            options.timeout,
        )?;
        report.process.window_pid = Some(window.pid);
        launched_window = Some(window.clone());
        thread::sleep(Duration::from_secs(1));

        let health_result = wait_health(&options.addr, options.timeout)?;
        push_step(&mut report, "health", Some(health_result));

        let eval_result = eval_js(
            &options.addr,
            "return { title: document.title, href: String(location.href) };",
            5000,
        )?;
        write_json(logs.join("eval-smoke.json"), &eval_result)?;
        push_step(&mut report, "eval", Some(eval_result));

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
        push_step(&mut report, "initial_snapshot", None);
        capture_step(&mut report, &screenshots, "dashboard_initial", &window)?;

        click_nav(&options.addr, "import_export")?;
        wait_agent(&options.addr, "view-import-export", Duration::from_secs(10))?;
        capture_step(&mut report, &screenshots, "import_export_before", &window)?;

        action(
            &options.addr,
            "set_input_agent",
            "import-mode",
            Value::String("raw".to_string()),
            5000,
        )?;
        action(
            &options.addr,
            "set_input_agent",
            "import-path",
            Value::String(sample.display().to_string()),
            5000,
        )?;
        action(
            &options.addr,
            "click_agent",
            "import-run",
            Value::Null,
            5000,
        )?;
        wait_text(&options.addr, "Last import", Duration::from_secs(30))?;
        push_step(
            &mut report,
            "import_sample",
            Some(json!({ "sample": sample.display().to_string() })),
        );
        capture_step(&mut report, &screenshots, "import_export_after", &window)?;

        for view in ["dashboard", "records", "settings"] {
            click_nav(&options.addr, view)?;
            wait_agent(
                &options.addr,
                &format!("view-{view}"),
                Duration::from_secs(10),
            )?;
            capture_step(&mut report, &screenshots, &format!("{view}_after"), &window)?;
            push_step(&mut report, format!("view_{view}"), None);
        }

        let final_snapshot = action(&options.addr, "snapshot", "", Value::Null, 5000)?;
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
    })();

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

    if !options.keep_app {
        if let Some(window) = launched_window.as_ref() {
            let _ = close_window(window);
        }
        let _ = child.kill();
    } else {
        let _ = child.try_wait();
    }

    if !keep_portable {
        match remove_portable_copy(&run_dir, &portable_root) {
            Ok(removed) => report.cleanup.portable_removed = removed,
            Err(error) => report.cleanup.warnings.push(error.to_string()),
        }
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

    match rotate_run_dirs(&options.out_dir, &run_dir, options.keep_runs) {
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

fn push_step(report: &mut Report, name: impl Into<String>, detail: Option<Value>) {
    report.steps.push(StepReport {
        name: name.into(),
        ok: true,
        detail,
    });
}

fn capture_step(
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

fn health(addr: &str) -> Result<Value> {
    request("GET", addr, "/health", None, Duration::from_secs(5))
}

fn snapshot(addr: &str) -> Result<Value> {
    action(addr, "snapshot", "", Value::Null, 5000)
}

fn wait_health(addr: &str, timeout: Duration) -> Result<Value> {
    let deadline = Instant::now() + timeout;
    let mut last_error = None;
    while Instant::now() < deadline {
        match health(addr) {
            Ok(value) => return Ok(value),
            Err(error) => last_error = Some(error),
        }
        thread::sleep(Duration::from_millis(250));
    }
    match last_error {
        Some(error) => Err(error).context("agent smoke health did not become ready"),
        None => bail!("agent smoke health did not become ready"),
    }
}

fn eval_js(addr: &str, script: &str, timeout_ms: u64) -> Result<Value> {
    request(
        "POST",
        addr,
        "/eval",
        Some(json!({ "script": script, "timeout_ms": timeout_ms })),
        Duration::from_millis(timeout_ms).saturating_add(Duration::from_secs(2)),
    )
}

fn action(
    addr: &str,
    action_name: &str,
    agent_id: &str,
    value: Value,
    timeout_ms: u64,
) -> Result<Value> {
    request(
        "POST",
        addr,
        "/action",
        Some(json!({
            "action": action_name,
            "agent_id": agent_id,
            "value": value,
            "timeout_ms": timeout_ms,
        })),
        Duration::from_millis(timeout_ms).saturating_add(Duration::from_secs(2)),
    )
}

fn request(
    method: &str,
    addr: &str,
    path: &str,
    body: Option<Value>,
    timeout: Duration,
) -> Result<Value> {
    let url = format!("http://{addr}{path}");
    let agent = ureq::AgentBuilder::new().timeout(timeout).build();
    let response = match (method, body) {
        ("GET", None) => agent.get(&url).call(),
        ("POST", Some(body)) => agent
            .post(&url)
            .set("Content-Type", "application/json; charset=utf-8")
            .send_string(&body.to_string()),
        _ => bail!("unsupported request shape: {method} {path}"),
    }
    .map_err(|error| anyhow!("{error}"))?;
    let text = response
        .into_string()
        .map_err(|error| anyhow!("invalid API response body: {error}"))?;
    let envelope: ApiResponse = serde_json::from_str(&text)
        .map_err(|error| anyhow!("invalid API json response: {error}; body={text}"))?;
    if envelope.ok {
        Ok(envelope.result.unwrap_or(Value::Null))
    } else {
        bail!(
            envelope
                .error
                .unwrap_or_else(|| "agent API failed".to_string())
        )
    }
}

fn click_nav(addr: &str, view: &str) -> Result<()> {
    action(
        addr,
        "click_agent",
        &format!("nav-{view}"),
        Value::Null,
        5000,
    )?;
    thread::sleep(Duration::from_millis(500));
    Ok(())
}

fn agent_elements(snapshot: &Value) -> Vec<AgentElement> {
    snapshot
        .get("agentIds")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            Some(AgentElement {
                id: item.get("id")?.as_str()?.to_string(),
                tag: item
                    .get("tag")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                text: item
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                disabled: item
                    .get("disabled")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            })
        })
        .collect()
}

fn find_agent_element(snapshot: &Value, agent_id: &str) -> Option<AgentElement> {
    agent_elements(snapshot)
        .into_iter()
        .find(|element| element.id == agent_id)
}

fn wait_agent_element(addr: &str, agent_id: &str, timeout: Duration) -> Result<AgentElement> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let snapshot = snapshot(addr)?;
        if let Some(element) = find_agent_element(&snapshot, agent_id) {
            return Ok(element);
        }
        thread::sleep(Duration::from_millis(250));
    }
    bail!("agent id not visible before timeout: {agent_id}")
}

fn expect_text(addr: &str, text: &str, timeout: Duration) -> Result<ExpectTextOutput> {
    let started_at = Instant::now();
    let deadline = started_at + timeout;
    while Instant::now() < deadline {
        let snapshot = snapshot(addr)?;
        if snapshot
            .get("bodyText")
            .and_then(Value::as_str)
            .is_some_and(|body| body.contains(text))
        {
            return Ok(ExpectTextOutput {
                text: text.to_string(),
                matched: true,
                elapsed_ms: started_at.elapsed().as_millis(),
            });
        }
        thread::sleep(Duration::from_millis(250));
    }
    bail!("text not visible before timeout: {text}")
}

fn print_agent_elements_plain(elements: &[AgentElement]) {
    for element in elements {
        print_agent_element_plain(element);
    }
}

fn print_agent_element_plain(element: &AgentElement) {
    let state = if element.disabled {
        "disabled"
    } else {
        "enabled"
    };
    println!(
        "{}\t{}\t{}\t{}",
        element.id,
        element.tag,
        state,
        plain_cell(&element.text)
    );
}

fn plain_cell(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if matches!(character, '\t' | '\r' | '\n') {
                ' '
            } else {
                character
            }
        })
        .collect()
}

fn wait_agent(addr: &str, agent_id: &str, timeout: Duration) -> Result<Value> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let snapshot = action(addr, "snapshot", "", Value::Null, 5000)?;
        let ids = snapshot
            .get("agentIds")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|item| item.get("id").and_then(Value::as_str))
            .collect::<BTreeSet<_>>();
        if ids.contains(agent_id) {
            return Ok(snapshot);
        }
        thread::sleep(Duration::from_millis(250));
    }
    bail!("agent id not visible: {agent_id}")
}

fn wait_text(addr: &str, text: &str, timeout: Duration) -> Result<Value> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let snapshot = action(addr, "snapshot", "", Value::Null, 5000)?;
        if snapshot
            .get("bodyText")
            .and_then(Value::as_str)
            .is_some_and(|body| body.contains(text))
        {
            return Ok(snapshot);
        }
        thread::sleep(Duration::from_millis(500));
    }
    bail!("text not visible: {text}")
}

fn assert_agent_ids(snapshot: &Value, expected: &[&str]) -> Result<()> {
    let ids = snapshot
        .get("agentIds")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("id").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    let missing = expected
        .iter()
        .copied()
        .filter(|id| !ids.contains(id))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        bail!("missing agent ids: {}", missing.join(", "))
    }
}

fn stage_portable(source: &Path, destination: &Path) -> Result<()> {
    if destination.exists() {
        bail!(
            "portable destination already exists: {}",
            destination.display()
        );
    }
    for entry in walk_files(source)? {
        let relative = entry.strip_prefix(source)?;
        if relative
            .components()
            .next()
            .and_then(|component| component.as_os_str().to_str())
            .is_some_and(|name| name == "data" || name == "update")
        {
            continue;
        }
        let target = destination.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&target)?;
        } else if entry.is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&entry, &target)
                .with_context(|| format!("copy {} -> {}", entry.display(), target.display()))?;
        }
    }
    Ok(())
}

fn default_release_root() -> Result<PathBuf> {
    let smoke_input = PathBuf::from(DEFAULT_SMOKE_RELEASE_ROOT);
    if smoke_input.is_dir() {
        return Ok(smoke_input);
    }

    let versioned = PathBuf::from(format!(
        "dist/nte-gacha-exporter-{}",
        env!("CARGO_PKG_VERSION")
    ));
    if versioned.is_dir() {
        return Ok(versioned);
    }

    let dist = Path::new("dist");
    if !dist.is_dir() {
        return Ok(versioned);
    }

    let mut candidates = fs::read_dir(dist)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.is_dir()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("nte-gacha-exporter-"))
        })
        .collect::<Vec<_>>();
    candidates.sort();
    match candidates.as_slice() {
        [single] => Ok(single.clone()),
        [] => Ok(versioned),
        _ => bail!("multiple release roots in dist; pass --release-root"),
    }
}

fn walk_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(path) = stack.pop() {
        out.push(path.clone());
        if path.is_dir() {
            for entry in fs::read_dir(&path)? {
                stack.push(entry?.path());
            }
        }
    }
    Ok(out)
}

fn launch_app(launcher: &Path, portable_root: &Path, addr: &str) -> Result<Child> {
    Command::new(launcher)
        .current_dir(portable_root)
        .env("NTE_AGENT_SMOKE", "1")
        .env("NTE_AGENT_SMOKE_ADDR", addr)
        .env("NTE_GACHA_EXPORTER_PORTABLE_ROOT", portable_root)
        .env("NTE_GACHA_EXPORTER_ROOT", portable_root)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("launch failed: {}", launcher.display()))
}

fn ensure_addr_available(addr: &str) -> Result<()> {
    let listener = TcpListener::bind(addr)
        .with_context(|| format!("agent smoke addr is already in use or invalid: {addr}"))?;
    drop(listener);
    Ok(())
}

fn new_run_dir(base: &Path) -> Result<PathBuf> {
    fs::create_dir_all(base)?;
    let run_dir = base.join(format!("run-{}-{}", unix_secs(), std::process::id()));
    fs::create_dir(&run_dir)
        .with_context(|| format!("create run dir failed: {}", run_dir.display()))?;
    Ok(run_dir)
}

fn remove_portable_copy(run_dir: &Path, portable_root: &Path) -> Result<bool> {
    if portable_root.file_name().and_then(|name| name.to_str()) != Some("portable")
        || portable_root.parent() != Some(run_dir)
    {
        bail!(
            "refusing to remove unexpected portable path: {}",
            portable_root.display()
        );
    }
    if !portable_root.exists() {
        return Ok(false);
    }
    fs::remove_dir_all(portable_root)
        .with_context(|| format!("remove portable copy failed: {}", portable_root.display()))?;
    Ok(true)
}

fn rotate_run_dirs(base: &Path, current_run_dir: &Path, keep_runs: usize) -> Result<Vec<PathBuf>> {
    if keep_runs == 0 {
        bail!("keep_runs must be at least 1");
    }
    if !base.is_dir() {
        return Ok(Vec::new());
    }

    let current = canonical_or_self(current_run_dir);
    let mut runs = Vec::new();
    for entry in fs::read_dir(base)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(timestamp) = run_dir_timestamp(name) else {
            continue;
        };
        runs.push((timestamp, name.to_string(), path));
    }

    runs.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1)));

    let mut keep = HashSet::from([current]);
    for (_, _, path) in &runs {
        if keep.len() >= keep_runs {
            break;
        }
        keep.insert(canonical_or_self(path));
    }

    let mut removed = Vec::new();
    for (_, _, path) in runs {
        if keep.contains(&canonical_or_self(&path)) {
            continue;
        }
        fs::remove_dir_all(&path)
            .with_context(|| format!("remove old smoke run failed: {}", path.display()))?;
        removed.push(path);
    }
    Ok(removed)
}

fn run_dir_timestamp(name: &str) -> Option<u64> {
    let suffix = name.strip_prefix("run-")?;
    let (timestamp, pid) = suffix.split_once('-')?;
    if pid.is_empty() || !pid.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    timestamp.parse().ok()
}

fn canonical_or_self(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn ensure_file(path: &Path) -> Result<()> {
    if path.is_file() {
        Ok(())
    } else {
        bail!("missing file: {}", path.display())
    }
}

fn write_json(path: impl AsRef<Path>, value: &impl Serialize) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

fn print_json(value: &impl Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn write_png(path: &Path, image: &RgbImage) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    image.save(path)?;
    Ok(())
}

fn image_metrics(image: &RgbImage) -> ImageMetrics {
    let width = image.width();
    let height = image.height();
    let count = (width as f64 * height as f64).max(1.0);
    let mut sum = [0_u64; 3];
    let mut min = [u8::MAX; 3];
    let mut max = [0_u8; 3];
    for pixel in image.pixels() {
        for channel in 0..3 {
            let value = pixel[channel];
            sum[channel] += value as u64;
            min[channel] = min[channel].min(value);
            max[channel] = max[channel].max(value);
        }
    }
    let mean = [
        sum[0] as f64 / count,
        sum[1] as f64 / count,
        sum[2] as f64 / count,
    ];
    let mut deviation_sum = [0_f64; 3];
    for pixel in image.pixels() {
        for channel in 0..3 {
            deviation_sum[channel] += (pixel[channel] as f64 - mean[channel]).abs();
        }
    }
    let variance_score = deviation_sum.iter().map(|value| value / count).sum::<f64>();
    ImageMetrics {
        width,
        height,
        mean,
        extrema: [[min[0], max[0]], [min[1], max[1]], [min[2], max[2]]],
        variance_score,
        is_flat: variance_score < 2.0,
    }
}

#[cfg(not(windows))]
fn require_windows() -> Result<()> {
    bail!("agent smoke launch and screenshot require a Windows host")
}

#[cfg(windows)]
fn require_windows() -> Result<()> {
    Ok(())
}

#[cfg(not(windows))]
fn visible_nte_windows() -> Result<Vec<WindowInfo>> {
    require_windows()?;
    unreachable!()
}

#[cfg(not(windows))]
fn find_window(
    _pid: Option<u32>,
    _title: Option<&str>,
    _exclude_hwnds: Option<&BTreeSet<usize>>,
    _timeout: Duration,
) -> Result<WindowInfo> {
    require_windows()?;
    unreachable!()
}

#[cfg(not(windows))]
fn capture_window(_window: &WindowInfo) -> Result<RgbImage> {
    require_windows()?;
    unreachable!()
}

#[cfg(not(windows))]
fn close_window(_window: &WindowInfo) -> Result<()> {
    require_windows()
}

#[cfg(windows)]
fn visible_nte_windows() -> Result<Vec<WindowInfo>> {
    windows_impl::visible_windows(None, Some(APP_TITLE))
}

#[cfg(windows)]
fn find_window(
    pid: Option<u32>,
    title: Option<&str>,
    exclude_hwnds: Option<&BTreeSet<usize>>,
    timeout: Duration,
) -> Result<WindowInfo> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let mut windows = windows_impl::visible_windows(pid, title)?;
        if let Some(excluded) = exclude_hwnds {
            windows.retain(|window| !excluded.contains(&window.hwnd));
        }
        if let Some(window) = windows.into_iter().next() {
            return Ok(window);
        }
        thread::sleep(Duration::from_millis(250));
    }
    bail!("window not found: pid={pid:?} title={title:?}")
}

#[cfg(windows)]
fn capture_window(window: &WindowInfo) -> Result<RgbImage> {
    windows_impl::capture_window(window)
}

#[cfg(windows)]
fn close_window(window: &WindowInfo) -> Result<()> {
    windows_impl::close_window(window)
}

#[cfg(windows)]
mod windows_impl {
    use super::*;
    use std::mem;

    use windows_sys::Win32::{
        Foundation::{CloseHandle, HWND, LPARAM, RECT},
        Graphics::Gdi::{
            BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleBitmap, CreateCompatibleDC,
            DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDIBits, GetWindowDC, HGDIOBJ, ReleaseDC,
            SelectObject,
        },
        Storage::Xps::PrintWindow,
        System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess},
        UI::WindowsAndMessaging::{
            EnumWindows, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
            GetWindowThreadProcessId, IsIconic, IsWindowVisible, PostMessageW, SW_RESTORE, SW_SHOW,
            SetForegroundWindow, ShowWindow, WM_CLOSE,
        },
    };
    use windows_sys::core::BOOL;

    pub fn visible_windows(pid: Option<u32>, title: Option<&str>) -> Result<Vec<WindowInfo>> {
        struct Context {
            pid: Option<u32>,
            title: Option<String>,
            windows: Vec<WindowInfo>,
        }

        unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let context = unsafe { &mut *(lparam as *mut Context) };
            if unsafe { IsWindowVisible(hwnd) } == 0 || unsafe { IsIconic(hwnd) } != 0 {
                return 1;
            }
            let mut window_pid = 0_u32;
            unsafe {
                GetWindowThreadProcessId(hwnd, &mut window_pid);
            }
            if context.pid.is_some_and(|pid| pid != window_pid) {
                return 1;
            }
            let Ok(window_title) = window_title(hwnd) else {
                return 1;
            };
            if context
                .title
                .as_deref()
                .is_some_and(|needle| !window_title.contains(needle))
            {
                return 1;
            }
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            if unsafe { GetWindowRect(hwnd, &mut rect) } == 0 {
                return 1;
            }
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;
            if width < 200 || height < 200 {
                return 1;
            }
            context.windows.push(WindowInfo {
                hwnd: hwnd as usize,
                pid: window_pid,
                title: window_title,
                rect: [rect.left, rect.top, width, height],
            });
            1
        }

        let mut context = Context {
            pid,
            title: title.map(str::to_string),
            windows: Vec::new(),
        };
        unsafe {
            EnumWindows(Some(enum_proc), &mut context as *mut Context as LPARAM);
        }
        context.windows.sort_by_key(|window| window.hwnd);
        Ok(context.windows)
    }

    pub fn capture_window(window: &WindowInfo) -> Result<RgbImage> {
        let hwnd = window.hwnd as HWND;
        unsafe {
            if IsIconic(hwnd) != 0 {
                ShowWindow(hwnd, SW_RESTORE);
            } else {
                ShowWindow(hwnd, SW_SHOW);
            }
            let _ = SetForegroundWindow(hwnd);
        }
        thread::sleep(Duration::from_millis(200));

        let width = window.rect[2];
        let height = window.rect[3];
        if width <= 0 || height <= 0 {
            bail!("invalid window rect: {:?}", window.rect);
        }

        unsafe {
            let window_dc = GetWindowDC(hwnd);
            if window_dc.is_null() {
                bail!("GetWindowDC failed");
            }
            let memory_dc = CreateCompatibleDC(window_dc);
            if memory_dc.is_null() {
                ReleaseDC(hwnd, window_dc);
                bail!("CreateCompatibleDC failed");
            }
            let bitmap = CreateCompatibleBitmap(window_dc, width, height);
            if bitmap.is_null() {
                DeleteDC(memory_dc);
                ReleaseDC(hwnd, window_dc);
                bail!("CreateCompatibleBitmap failed");
            }
            let old_bitmap = SelectObject(memory_dc, bitmap as HGDIOBJ);
            let result = (|| -> Result<RgbImage> {
                if PrintWindow(hwnd, memory_dc, 2) == 0 && PrintWindow(hwnd, memory_dc, 0) == 0 {
                    bail!("PrintWindow failed");
                }
                let mut info = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: width,
                        biHeight: -height,
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB,
                        biSizeImage: 0,
                        biXPelsPerMeter: 0,
                        biYPelsPerMeter: 0,
                        biClrUsed: 0,
                        biClrImportant: 0,
                    },
                    bmiColors: [Default::default()],
                };
                let mut bgra = vec![0_u8; width as usize * height as usize * 4];
                let lines = GetDIBits(
                    memory_dc,
                    bitmap,
                    0,
                    height as u32,
                    bgra.as_mut_ptr().cast(),
                    &mut info,
                    DIB_RGB_COLORS,
                );
                if lines != height {
                    bail!("GetDIBits returned {lines}, expected {height}");
                }
                let mut rgb = Vec::with_capacity(width as usize * height as usize * 3);
                for pixel in bgra.chunks_exact(4) {
                    rgb.push(pixel[2]);
                    rgb.push(pixel[1]);
                    rgb.push(pixel[0]);
                }
                RgbImage::from_raw(width as u32, height as u32, rgb)
                    .ok_or_else(|| anyhow!("invalid image buffer"))
            })();
            if !old_bitmap.is_null() {
                SelectObject(memory_dc, old_bitmap);
            }
            DeleteObject(bitmap as HGDIOBJ);
            DeleteDC(memory_dc);
            ReleaseDC(hwnd, window_dc);
            result
        }
    }

    pub fn close_window(window: &WindowInfo) -> Result<()> {
        let hwnd = window.hwnd as HWND;
        unsafe {
            let _ = PostMessageW(hwnd, WM_CLOSE, 0, 0);
        }
        thread::sleep(Duration::from_secs(2));
        if visible_windows(Some(window.pid), Some(&window.title))?.is_empty() {
            return Ok(());
        }
        unsafe {
            let process = OpenProcess(PROCESS_TERMINATE, 0, window.pid);
            if process.is_null() {
                bail!(
                    "OpenProcess(PROCESS_TERMINATE) failed for pid={}",
                    window.pid
                );
            }
            if TerminateProcess(process, 1) == 0 {
                CloseHandle(process);
                bail!("TerminateProcess failed for pid={}", window.pid);
            }
            CloseHandle(process);
        }
        Ok(())
    }

    fn window_title(hwnd: HWND) -> Result<String> {
        let length = unsafe { GetWindowTextLengthW(hwnd) };
        if length <= 0 {
            return Ok(String::new());
        }
        let mut buffer = vec![0_u16; length as usize + 1];
        let written = unsafe { GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
        if written < 0 {
            bail!("GetWindowTextW failed");
        }
        Ok(String::from_utf16_lossy(&buffer[..written as usize]))
    }
}

#[cfg(test)]
mod tests {
    use std::{env, fs, path::PathBuf};

    use super::*;

    #[test]
    fn rotate_run_dirs_keeps_current_and_latest_only() {
        let temp = temp_dir("rotate");
        let base = temp.join("agent-smoke");
        let old = base.join("run-100-1");
        let middle = base.join("run-200-1");
        let current = base.join("run-300-1");
        fs::create_dir_all(old.join("logs")).unwrap();
        fs::create_dir_all(middle.join("logs")).unwrap();
        fs::create_dir_all(current.join("logs")).unwrap();
        fs::create_dir_all(base.join("smoke-input-current")).unwrap();
        fs::write(base.join("latest-report.json"), "{}").unwrap();

        let removed = rotate_run_dirs(&base, &current, 1).unwrap();

        assert_eq!(removed.len(), 2);
        assert!(!old.exists());
        assert!(!middle.exists());
        assert!(current.exists());
        assert!(base.join("smoke-input-current").exists());
        assert!(base.join("latest-report.json").exists());

        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn rotate_run_dirs_ignores_invalid_run_names() {
        let temp = temp_dir("invalid");
        let base = temp.join("agent-smoke");
        let current = base.join("run-300-1");
        let invalid = base.join("run-not-a-timestamp");
        fs::create_dir_all(&current).unwrap();
        fs::create_dir_all(&invalid).unwrap();

        let removed = rotate_run_dirs(&base, &current, 1).unwrap();

        assert!(removed.is_empty());
        assert!(current.exists());
        assert!(invalid.exists());

        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn rotate_run_dirs_counts_current_against_keep_limit() {
        let temp = temp_dir("current-limit");
        let base = temp.join("agent-smoke");
        let current = base.join("run-100-1");
        let newer = base.join("run-300-1");
        fs::create_dir_all(&current).unwrap();
        fs::create_dir_all(&newer).unwrap();

        let removed = rotate_run_dirs(&base, &current, 1).unwrap();

        assert_eq!(removed, vec![newer]);
        assert!(current.exists());
        assert!(!base.join("run-300-1").exists());

        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn remove_portable_copy_only_removes_run_portable_dir() {
        let temp = temp_dir("portable");
        let run_dir = temp.join("run-100-1");
        let portable = run_dir.join("portable");
        fs::create_dir_all(&portable).unwrap();
        fs::write(portable.join("nte-gacha-exporter.exe"), "").unwrap();

        assert!(remove_portable_copy(&run_dir, &portable).unwrap());
        assert!(!portable.exists());
        assert!(!remove_portable_copy(&run_dir, &portable).unwrap());

        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn remove_portable_copy_rejects_unexpected_path() {
        let temp = temp_dir("reject");
        let run_dir = temp.join("run-100-1");
        let unexpected = temp.join("portable");
        fs::create_dir_all(&unexpected).unwrap();

        assert!(remove_portable_copy(&run_dir, &unexpected).is_err());
        assert!(unexpected.exists());

        let _ = fs::remove_dir_all(temp);
    }

    fn temp_dir(name: &str) -> PathBuf {
        let path = env::temp_dir().join(format!(
            "nte-agent-smoke-{name}-{}-{}",
            std::process::id(),
            unix_secs()
        ));
        if path.exists() {
            let _ = fs::remove_dir_all(&path);
        }
        fs::create_dir_all(&path).unwrap();
        path
    }
}
