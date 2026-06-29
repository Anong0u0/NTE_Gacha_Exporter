use std::collections::{BTreeSet, HashMap};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use nte_capture::{
    DiagnosticCaptureCounters, DiagnosticCaptureOptions, DiagnosticCaptureResult,
    DiagnosticCaptureSummary, candidate_ports, find_process_pids, is_admin, run_diagnostic_capture,
};
use serde::Serialize;
use tauri::State;
use zip::{CompressionMethod, ZipWriter, write::FileOptions};

use crate::admin::admin_relaunch_required;
use crate::error::{ApiError, RuntimeError, api_error_message};
use crate::state::{AppState, new_named_session_id, now_seconds, portable_root};

const DEFAULT_DIAGNOSTIC_DURATION_SECONDS: u64 = 30;
const MIN_DIAGNOSTIC_DURATION_SECONDS: u64 = 5;
const MAX_DIAGNOSTIC_DURATION_SECONDS: u64 = 120;
const DIAGNOSTIC_SESSION_RETENTION_SECONDS: f64 = 30.0 * 60.0;
const DIAGNOSTIC_TERMINAL_SESSION_LIMIT: usize = 10;
const DROPPED_SAMPLE_LIMIT: usize = 1_000;

pub(crate) struct DiagnosticRuntimeSession {
    status: Mutex<DiagnosticStatus>,
    stop: Arc<AtomicBool>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DiagnosticStatus {
    session_id: String,
    state: String,
    started_at: f64,
    updated_at: f64,
    duration_seconds: u64,
    elapsed_seconds: f64,
    stage: String,
    progress: f64,
    support_zip_path: Option<String>,
    error: Option<RuntimeError>,
    summary: Option<DiagnosticStatusSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DiagnosticStatusSummary {
    verdict: String,
    findings: Vec<String>,
    packets_seen: u64,
    decoded_packets: u64,
    dropped_packets: u64,
    duplicate_packets: u64,
    rows_count: u64,
    external_ok: bool,
}

#[derive(Debug, Clone, Serialize)]
struct DiagnosticDocument {
    schema_version: u32,
    app_version: String,
    session_id: String,
    created_at: f64,
    duration_seconds: u64,
    environment: DiagnosticEnvironment,
    target: DiagnosticTargetDiscovery,
    internal: InternalDiagnosticReport,
    external: ExternalCaptureReport,
    artifacts: Vec<DiagnosticArtifact>,
    verdict: DiagnosticClassification,
}

#[derive(Debug, Clone, Serialize)]
struct DiagnosticEnvironment {
    windows: bool,
    admin: bool,
    portable_root: String,
    current_exe: Option<String>,
    current_dir: Option<String>,
    process_id: u32,
}

#[derive(Debug, Clone, Serialize)]
struct DiagnosticTargetDiscovery {
    exe: String,
    selected_pid: Option<u32>,
    selected_ports: Vec<u16>,
    candidates: Vec<ProcessCandidate>,
    warnings: Vec<String>,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ProcessCandidate {
    pid: u32,
    ports: Vec<u16>,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct InternalDiagnosticReport {
    attempted: bool,
    error: Option<String>,
    result: Option<DiagnosticCaptureResult>,
}

#[derive(Debug, Clone, Serialize)]
struct ExternalCaptureReport {
    attempted: bool,
    ok: bool,
    error: Option<String>,
    etl_path: Option<String>,
    pcapng_path: Option<String>,
    stdout_log_path: Option<String>,
    stderr_log_path: Option<String>,
    counters_json_path: Option<String>,
    counters_txt_path: Option<String>,
    command_log_path: Option<String>,
    commands: Vec<ExternalCommandLog>,
}

#[derive(Debug, Clone, Serialize)]
struct ExternalCommandLog {
    program: String,
    args: Vec<String>,
    exit_code: Option<i32>,
    success: bool,
    stdout_bytes: usize,
    stderr_bytes: usize,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct DiagnosticArtifact {
    name: String,
    path: Option<String>,
    exists: bool,
    size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
struct DiagnosticClassification {
    verdict: String,
    findings: Vec<String>,
}

struct SupportPaths {
    support_dir: PathBuf,
    zip_path: PathBuf,
    diagnostic_json: PathBuf,
    internal_raw: PathBuf,
    dropped_samples: PathBuf,
    external_etl: PathBuf,
    external_pcapng: PathBuf,
    external_stdout: PathBuf,
    external_stderr: PathBuf,
    counters_json: PathBuf,
    counters_txt: PathBuf,
    external_commands: PathBuf,
}

#[tauri::command]
pub(crate) fn diagnostic_start(
    state: State<'_, AppState>,
    duration_seconds: Option<u64>,
) -> Result<DiagnosticStatus, ApiError> {
    if admin_relaunch_required()? {
        return Err(api_error_message(
            "admin_required",
            "pktmon diagnostic requires administrator permission",
        ));
    }
    start_diagnostic_session(&state, duration_seconds)
}

#[tauri::command]
pub(crate) fn diagnostic_status(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<DiagnosticStatus, ApiError> {
    diagnostic_status_inner(&state, &session_id)
}

#[tauri::command]
pub(crate) fn diagnostic_cancel(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<DiagnosticStatus, ApiError> {
    let session = diagnostic_runtime_session(&state, &session_id)?;
    session.stop.store(true, Ordering::SeqCst);
    {
        let mut status = session.status.lock().map_err(|_| {
            api_error_message("diagnostic_lock_poisoned", "diagnostic lock poisoned")
        })?;
        if matches!(status.state.as_str(), "starting" | "running") {
            status.state = "stopping".to_string();
            status.stage = "stopping".to_string();
            status.updated_at = now_seconds();
        }
    }
    diagnostic_status_inner(&state, &session_id)
}

pub(crate) fn start_diagnostic_session(
    state: &State<'_, AppState>,
    duration_seconds: Option<u64>,
) -> Result<DiagnosticStatus, ApiError> {
    let duration_seconds = duration_seconds
        .unwrap_or(DEFAULT_DIAGNOSTIC_DURATION_SECONDS)
        .clamp(
            MIN_DIAGNOSTIC_DURATION_SECONDS,
            MAX_DIAGNOSTIC_DURATION_SECONDS,
        );
    let session_id = new_named_session_id("diagnostic");
    let now = now_seconds();
    let initial_status = DiagnosticStatus {
        session_id: session_id.clone(),
        state: "starting".to_string(),
        started_at: now,
        updated_at: now,
        duration_seconds,
        elapsed_seconds: 0.0,
        stage: "preparing".to_string(),
        progress: 0.0,
        support_zip_path: None,
        error: None,
        summary: None,
    };
    let stop = Arc::new(AtomicBool::new(false));
    let runtime = Arc::new(DiagnosticRuntimeSession {
        status: Mutex::new(initial_status.clone()),
        stop: Arc::clone(&stop),
        handle: Mutex::new(None),
    });
    state
        .diagnostic_sessions
        .lock()
        .map_err(|_| api_error_message("diagnostic_lock_poisoned", "diagnostic lock poisoned"))?
        .insert(session_id.clone(), Arc::clone(&runtime));

    let runtime_for_thread = Arc::clone(&runtime);
    let handle = std::thread::spawn(move || {
        run_diagnostic_thread(runtime_for_thread, session_id, duration_seconds);
    });
    *runtime
        .handle
        .lock()
        .map_err(|_| api_error_message("diagnostic_lock_poisoned", "diagnostic lock poisoned"))? =
        Some(handle);
    Ok(initial_status)
}

fn run_diagnostic_thread(
    runtime: Arc<DiagnosticRuntimeSession>,
    session_id: String,
    duration_seconds: u64,
) {
    update_status(&runtime, "running", "resolving_target", 0.04, None, None);
    let result = build_support_bundle(&runtime, &session_id, duration_seconds);
    match result {
        Ok((document, zip_path)) => {
            let summary = status_summary(&document);
            update_status(
                &runtime,
                "completed",
                "completed",
                1.0,
                Some(zip_path.to_string_lossy().to_string()),
                Some(summary),
            );
        }
        Err(error) => {
            let runtime_error = RuntimeError {
                code: "diagnostic_failed".to_string(),
                message: error.to_string(),
                support_path: None,
                support_image_path: None,
            };
            let mut status = runtime.status.lock().expect("diagnostic status lock");
            status.state = "failed".to_string();
            status.stage = "failed".to_string();
            status.updated_at = now_seconds();
            status.elapsed_seconds = status.updated_at - status.started_at;
            status.error = Some(runtime_error);
        }
    }
}

fn build_support_bundle(
    runtime: &Arc<DiagnosticRuntimeSession>,
    session_id: &str,
    duration_seconds: u64,
) -> anyhow::Result<(DiagnosticDocument, PathBuf)> {
    let root = portable_root()?;
    let paths = support_paths(&root, session_id);
    fs::create_dir_all(&paths.support_dir)?;
    let environment = DiagnosticEnvironment {
        windows: cfg!(windows),
        admin: is_admin(),
        portable_root: root.to_string_lossy().to_string(),
        current_exe: std::env::current_exe()
            .ok()
            .map(|path| path.to_string_lossy().to_string()),
        current_dir: std::env::current_dir()
            .ok()
            .map(|path| path.to_string_lossy().to_string()),
        process_id: std::process::id(),
    };
    let target = discover_target();
    update_status(runtime, "running", "capturing", 0.08, None, None);

    let duration = Duration::from_secs(duration_seconds);
    let external_handle = start_external_capture_thread(
        &paths,
        &target.selected_ports,
        duration,
        Arc::clone(&runtime.stop),
    );
    let internal = run_internal_capture(runtime, &paths, &target, &environment, duration);
    update_status(runtime, "running", "external_capture", 0.90, None, None);
    let external = external_handle
        .map(|handle| {
            handle.join().unwrap_or_else(|_| ExternalCaptureReport {
                attempted: true,
                ok: false,
                error: Some("external pktmon worker panicked".to_string()),
                etl_path: Some(paths.external_etl.to_string_lossy().to_string()),
                pcapng_path: Some(paths.external_pcapng.to_string_lossy().to_string()),
                stdout_log_path: Some(paths.external_stdout.to_string_lossy().to_string()),
                stderr_log_path: Some(paths.external_stderr.to_string_lossy().to_string()),
                counters_json_path: Some(paths.counters_json.to_string_lossy().to_string()),
                counters_txt_path: Some(paths.counters_txt.to_string_lossy().to_string()),
                command_log_path: Some(paths.external_commands.to_string_lossy().to_string()),
                commands: Vec::new(),
            })
        })
        .unwrap_or_else(|| ExternalCaptureReport {
            attempted: false,
            ok: false,
            error: Some("external pktmon skipped: no selected ports".to_string()),
            etl_path: None,
            pcapng_path: None,
            stdout_log_path: None,
            stderr_log_path: None,
            counters_json_path: None,
            counters_txt_path: None,
            command_log_path: None,
            commands: Vec::new(),
        });

    update_status(runtime, "running", "packing", 0.95, None, None);
    let classification = classify_diagnostic(&environment, &target, &internal, &external);
    let mut document = DiagnosticDocument {
        schema_version: 1,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        session_id: session_id.to_string(),
        created_at: now_seconds(),
        duration_seconds,
        environment,
        target,
        internal,
        external,
        artifacts: Vec::new(),
        verdict: classification,
    };
    fs::write(
        &paths.diagnostic_json,
        serde_json::to_vec_pretty(&document)?,
    )?;
    document.artifacts = collect_artifacts(&paths);
    fs::write(
        &paths.diagnostic_json,
        serde_json::to_vec_pretty(&document)?,
    )?;
    write_support_zip(&paths)?;
    rotate_support_zips(&paths.support_dir, &paths.zip_path)?;
    cleanup_artifact_files(&paths);
    Ok((document, paths.zip_path))
}

fn run_internal_capture(
    runtime: &Arc<DiagnosticRuntimeSession>,
    paths: &SupportPaths,
    target: &DiagnosticTargetDiscovery,
    environment: &DiagnosticEnvironment,
    duration: Duration,
) -> InternalDiagnosticReport {
    let Some(pid) = target.selected_pid else {
        return InternalDiagnosticReport {
            attempted: false,
            error: Some("internal capture skipped: HTGame.exe not found".to_string()),
            result: None,
        };
    };
    if target.selected_ports.is_empty() {
        return InternalDiagnosticReport {
            attempted: false,
            error: Some("internal capture skipped: no candidate ports".to_string()),
            result: None,
        };
    }
    if !environment.windows {
        return InternalDiagnosticReport {
            attempted: false,
            error: Some("internal capture skipped: pktmon requires Windows".to_string()),
            result: None,
        };
    }
    if !environment.admin {
        return InternalDiagnosticReport {
            attempted: false,
            error: Some("internal capture skipped: administrator permission required".to_string()),
            result: None,
        };
    }

    let started_at = Instant::now();
    let status_runtime = Arc::clone(runtime);
    let duration_seconds = duration.as_secs_f64().max(1.0);
    let progress = Arc::new(move |progress: nte_capture::DiagnosticCaptureProgress| {
        let elapsed = progress
            .elapsed_seconds
            .max(started_at.elapsed().as_secs_f64());
        let capture_progress = (elapsed / duration_seconds).clamp(0.0, 1.0);
        let mut status = status_runtime
            .status
            .lock()
            .expect("diagnostic status lock");
        if matches!(status.state.as_str(), "running" | "starting") {
            status.state = "running".to_string();
            status.stage = "capturing".to_string();
            status.progress = 0.08 + capture_progress * 0.80;
            status.elapsed_seconds = elapsed;
            status.updated_at = now_seconds();
        }
    });

    let result = run_diagnostic_capture(
        DiagnosticCaptureOptions {
            pid,
            exe: "HTGame.exe".to_string(),
            ports: target.selected_ports.clone(),
            raw_out: Some(paths.internal_raw.clone()),
            dropped_samples_out: Some(paths.dropped_samples.clone()),
            duration,
            max_dropped_samples: DROPPED_SAMPLE_LIMIT,
            on_progress: Some(progress),
        },
        Arc::clone(&runtime.stop),
    );
    match result {
        Ok(result) => InternalDiagnosticReport {
            attempted: true,
            error: None,
            result: Some(result),
        },
        Err(error) => InternalDiagnosticReport {
            attempted: true,
            error: Some(error.to_string()),
            result: None,
        },
    }
}

fn start_external_capture_thread(
    paths: &SupportPaths,
    ports: &[u16],
    duration: Duration,
    stop: Arc<AtomicBool>,
) -> Option<JoinHandle<ExternalCaptureReport>> {
    if ports.is_empty() {
        return None;
    }
    let paths = paths.clone_for_thread();
    let ports = ports.to_vec();
    Some(std::thread::spawn(move || {
        run_external_pktmon_capture(&paths, &ports, duration, stop)
    }))
}

fn run_external_pktmon_capture(
    paths: &SupportPaths,
    ports: &[u16],
    duration: Duration,
    stop: Arc<AtomicBool>,
) -> ExternalCaptureReport {
    let mut commands = Vec::new();
    let mut stdout_log = Vec::new();
    let mut stderr_log = Vec::new();
    let mut ok = true;
    let mut error = None;

    run_pktmon(
        &["filter", "remove"],
        &mut commands,
        &mut stdout_log,
        &mut stderr_log,
        &mut ok,
        &mut error,
    );
    for port in ports {
        run_pktmon(
            &[
                "filter",
                "add",
                &format!("NTE_DIAG_UDP_{port}"),
                "-t",
                "UDP",
                "-p",
                &port.to_string(),
            ],
            &mut commands,
            &mut stdout_log,
            &mut stderr_log,
            &mut ok,
            &mut error,
        );
        run_pktmon(
            &[
                "filter",
                "add",
                &format!("NTE_DIAG_TCP_{port}"),
                "-t",
                "TCP",
                "-p",
                &port.to_string(),
            ],
            &mut commands,
            &mut stdout_log,
            &mut stderr_log,
            &mut ok,
            &mut error,
        );
    }
    let external_etl = paths.external_etl.to_string_lossy().to_string();
    let external_pcapng = paths.external_pcapng.to_string_lossy().to_string();
    run_pktmon(
        &[
            "start",
            "--capture",
            "--pkt-size",
            "0",
            "--file-name",
            &external_etl,
        ],
        &mut commands,
        &mut stdout_log,
        &mut stderr_log,
        &mut ok,
        &mut error,
    );

    let started = Instant::now();
    while started.elapsed() < duration && !stop.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(250));
    }

    let counters_json = run_pktmon_capture_output(
        &["counters", "--json"],
        &paths.counters_json,
        &mut commands,
        &mut stdout_log,
        &mut stderr_log,
    );
    ok &= counters_json;
    if !counters_json && error.is_none() {
        error = Some("pktmon.exe counters --json failed".to_string());
    }
    let counters_txt = run_pktmon_capture_output(
        &["counters"],
        &paths.counters_txt,
        &mut commands,
        &mut stdout_log,
        &mut stderr_log,
    );
    ok &= counters_txt;
    if !counters_txt && error.is_none() {
        error = Some("pktmon.exe counters failed".to_string());
    }
    run_pktmon(
        &["stop"],
        &mut commands,
        &mut stdout_log,
        &mut stderr_log,
        &mut ok,
        &mut error,
    );
    run_pktmon(
        &["pcapng", &external_etl, "-o", &external_pcapng],
        &mut commands,
        &mut stdout_log,
        &mut stderr_log,
        &mut ok,
        &mut error,
    );
    run_pktmon(
        &["filter", "remove"],
        &mut commands,
        &mut stdout_log,
        &mut stderr_log,
        &mut ok,
        &mut error,
    );

    if let Err(write_error) = fs::write(&paths.external_stdout, stdout_log) {
        ok = false;
        error = Some(format!("write external stdout log failed: {write_error}"));
    }
    if let Err(write_error) = fs::write(&paths.external_stderr, stderr_log) {
        ok = false;
        error = Some(format!("write external stderr log failed: {write_error}"));
    }
    if let Err(write_error) = fs::write(
        &paths.external_commands,
        serde_json::to_vec_pretty(&commands).unwrap_or_default(),
    ) {
        ok = false;
        error = Some(format!("write external command log failed: {write_error}"));
    }

    ExternalCaptureReport {
        attempted: true,
        ok,
        error,
        etl_path: Some(paths.external_etl.to_string_lossy().to_string()),
        pcapng_path: Some(paths.external_pcapng.to_string_lossy().to_string()),
        stdout_log_path: Some(paths.external_stdout.to_string_lossy().to_string()),
        stderr_log_path: Some(paths.external_stderr.to_string_lossy().to_string()),
        counters_json_path: Some(paths.counters_json.to_string_lossy().to_string()),
        counters_txt_path: Some(paths.counters_txt.to_string_lossy().to_string()),
        command_log_path: Some(paths.external_commands.to_string_lossy().to_string()),
        commands,
    }
}

#[allow(clippy::too_many_arguments)]
fn run_pktmon(
    args: &[&str],
    commands: &mut Vec<ExternalCommandLog>,
    stdout_log: &mut Vec<u8>,
    stderr_log: &mut Vec<u8>,
    ok: &mut bool,
    error: &mut Option<String>,
) {
    let success = run_command("pktmon.exe", args, None, commands, stdout_log, stderr_log);
    if !success {
        *ok = false;
        if error.is_none() {
            *error = Some(format!("pktmon.exe {} failed", args.join(" ")));
        }
    }
}

fn run_pktmon_capture_output(
    args: &[&str],
    output_path: &Path,
    commands: &mut Vec<ExternalCommandLog>,
    stdout_log: &mut Vec<u8>,
    stderr_log: &mut Vec<u8>,
) -> bool {
    run_command(
        "pktmon.exe",
        args,
        Some(output_path),
        commands,
        stdout_log,
        stderr_log,
    )
}

fn run_command(
    program: &str,
    args: &[&str],
    stdout_file: Option<&Path>,
    commands: &mut Vec<ExternalCommandLog>,
    stdout_log: &mut Vec<u8>,
    stderr_log: &mut Vec<u8>,
) -> bool {
    let args_vec = args
        .iter()
        .map(|arg| (*arg).to_string())
        .collect::<Vec<_>>();
    match Command::new(program).args(args).output() {
        Ok(output) => {
            let _ = writeln!(stdout_log, "\n> {program} {}", args.join(" "));
            stdout_log.extend_from_slice(&output.stdout);
            let _ = writeln!(stderr_log, "\n> {program} {}", args.join(" "));
            stderr_log.extend_from_slice(&output.stderr);
            if let Some(path) = stdout_file {
                let _ = fs::write(path, &output.stdout);
            }
            let success = output.status.success();
            commands.push(ExternalCommandLog {
                program: program.to_string(),
                args: args_vec,
                exit_code: output.status.code(),
                success,
                stdout_bytes: output.stdout.len(),
                stderr_bytes: output.stderr.len(),
                error: None,
            });
            success
        }
        Err(error) => {
            let _ = writeln!(stderr_log, "\n> {program} {}", args.join(" "));
            let _ = writeln!(stderr_log, "{error}");
            commands.push(ExternalCommandLog {
                program: program.to_string(),
                args: args_vec,
                exit_code: None,
                success: false,
                stdout_bytes: 0,
                stderr_bytes: error.to_string().len(),
                error: Some(error.to_string()),
            });
            false
        }
    }
}

fn discover_target() -> DiagnosticTargetDiscovery {
    let mut warnings = Vec::new();
    let mut error = None;
    let pids = match find_process_pids("HTGame.exe") {
        Ok(pids) => pids,
        Err(err) => {
            error = Some(err.to_string());
            Vec::new()
        }
    };
    if pids.len() > 1 {
        warnings.push(format!(
            "multiple HTGame.exe processes found: {}",
            pids.len()
        ));
    }
    let mut candidates = Vec::new();
    for pid in pids {
        match candidate_ports(pid) {
            Ok(ports) => candidates.push(ProcessCandidate {
                pid,
                ports,
                error: None,
            }),
            Err(err) => candidates.push(ProcessCandidate {
                pid,
                ports: Vec::new(),
                error: Some(err.to_string()),
            }),
        }
    }
    let selected = candidates
        .iter()
        .find(|candidate| !candidate.ports.is_empty())
        .or_else(|| candidates.first());
    DiagnosticTargetDiscovery {
        exe: "HTGame.exe".to_string(),
        selected_pid: selected.map(|candidate| candidate.pid),
        selected_ports: selected
            .map(|candidate| candidate.ports.clone())
            .unwrap_or_default(),
        candidates,
        warnings,
        error,
    }
}

fn classify_diagnostic(
    environment: &DiagnosticEnvironment,
    target: &DiagnosticTargetDiscovery,
    internal: &InternalDiagnosticReport,
    external: &ExternalCaptureReport,
) -> DiagnosticClassification {
    let mut findings = target.warnings.clone();
    if let Some(error) = &target.error {
        findings.push(format!("process discovery error: {error}"));
    }
    if external.attempted && !external.ok {
        findings.push(format!(
            "external pktmon failed: {}",
            external.error.as_deref().unwrap_or("unknown error")
        ));
    }
    if !environment.windows {
        return classification("non_windows", findings);
    }
    if !environment.admin {
        return classification("admin_required", findings);
    }
    if target.selected_pid.is_none() {
        return classification("game_not_found", findings);
    }
    if target.selected_ports.is_empty() {
        return classification("no_candidate_ports", findings);
    }
    if let Some(error) = &internal.error {
        findings.push(format!("internal capture error: {error}"));
        if internal.result.is_none() {
            return classification("internal_capture_failed", findings);
        }
    }
    let Some(result) = &internal.result else {
        return classification("internal_capture_missing", findings);
    };
    classify_capture_result(&result.counters, &result.summary, findings)
}

fn classify_capture_result(
    counters: &DiagnosticCaptureCounters,
    summary: &DiagnosticCaptureSummary,
    findings: Vec<String>,
) -> DiagnosticClassification {
    if counters.packets_seen == 0 {
        return classification("no_packets_seen", findings);
    }
    if summary.rows_count > 0 || counters.decoded_packets > 0 {
        return classification("decoded_ok", findings);
    }
    let dropped_ratio = counters.dropped_packets as f64 / counters.packets_seen.max(1) as f64;
    if dropped_ratio >= 0.50 {
        return classification("high_parser_drop", findings);
    }
    if !summary.marker_hits.any() {
        let parsed = counters
            .packets_seen
            .saturating_sub(counters.dropped_packets)
            .saturating_sub(counters.duplicate_packets)
            .max(1);
        if summary.small_parsed_payload_packets as f64 / parsed as f64 >= 0.80 {
            return classification("only_idle_packets", findings);
        }
        return classification("no_decoder_marker", findings);
    }
    classification("marker_found_no_rows", findings)
}

fn classification(verdict: &str, findings: Vec<String>) -> DiagnosticClassification {
    DiagnosticClassification {
        verdict: verdict.to_string(),
        findings,
    }
}

fn status_summary(document: &DiagnosticDocument) -> DiagnosticStatusSummary {
    let counters = document
        .internal
        .result
        .as_ref()
        .map(|result| result.counters.clone())
        .unwrap_or_default();
    let rows_count = document
        .internal
        .result
        .as_ref()
        .map(|result| result.summary.rows_count)
        .unwrap_or_default();
    DiagnosticStatusSummary {
        verdict: document.verdict.verdict.clone(),
        findings: document.verdict.findings.clone(),
        packets_seen: counters.packets_seen,
        decoded_packets: counters.decoded_packets,
        dropped_packets: counters.dropped_packets,
        duplicate_packets: counters.duplicate_packets,
        rows_count,
        external_ok: document.external.ok,
    }
}

fn support_paths(root: &Path, session_id: &str) -> SupportPaths {
    let support_dir = root.join("data").join("support");
    let prefix = format!("diagnostic-{session_id}");
    SupportPaths {
        support_dir: support_dir.clone(),
        zip_path: support_dir.join(format!("{prefix}.zip")),
        diagnostic_json: support_dir.join(format!("{prefix}.diagnostic.json")),
        internal_raw: support_dir.join(format!("{prefix}.internal.raw.jsonl")),
        dropped_samples: support_dir.join(format!("{prefix}.internal-dropped-samples.jsonl")),
        external_etl: support_dir.join(format!("{prefix}.external.etl")),
        external_pcapng: support_dir.join(format!("{prefix}.external.pcapng")),
        external_stdout: support_dir.join(format!("{prefix}.external.stdout.txt")),
        external_stderr: support_dir.join(format!("{prefix}.external.stderr.txt")),
        counters_json: support_dir.join(format!("{prefix}.pktmon-counters.json")),
        counters_txt: support_dir.join(format!("{prefix}.pktmon-counters.txt")),
        external_commands: support_dir.join(format!("{prefix}.external-commands.json")),
    }
}

impl SupportPaths {
    fn clone_for_thread(&self) -> Self {
        Self {
            support_dir: self.support_dir.clone(),
            zip_path: self.zip_path.clone(),
            diagnostic_json: self.diagnostic_json.clone(),
            internal_raw: self.internal_raw.clone(),
            dropped_samples: self.dropped_samples.clone(),
            external_etl: self.external_etl.clone(),
            external_pcapng: self.external_pcapng.clone(),
            external_stdout: self.external_stdout.clone(),
            external_stderr: self.external_stderr.clone(),
            counters_json: self.counters_json.clone(),
            counters_txt: self.counters_txt.clone(),
            external_commands: self.external_commands.clone(),
        }
    }
}

fn collect_artifacts(paths: &SupportPaths) -> Vec<DiagnosticArtifact> {
    artifact_specs(paths)
        .into_iter()
        .map(|(name, path)| artifact(name, path))
        .collect()
}

fn artifact(name: &str, path: &Path) -> DiagnosticArtifact {
    let metadata = fs::metadata(path).ok();
    DiagnosticArtifact {
        name: name.to_string(),
        path: Some(path.to_string_lossy().to_string()),
        exists: metadata.as_ref().is_some_and(|metadata| metadata.is_file()),
        size_bytes: metadata.map(|metadata| metadata.len()),
    }
}

fn artifact_specs(paths: &SupportPaths) -> Vec<(&'static str, &Path)> {
    vec![
        ("diagnostic.json", paths.diagnostic_json.as_path()),
        ("internal.raw.jsonl", paths.internal_raw.as_path()),
        (
            "internal-dropped-samples.jsonl",
            paths.dropped_samples.as_path(),
        ),
        ("external.etl", paths.external_etl.as_path()),
        ("external.pcapng", paths.external_pcapng.as_path()),
        ("pktmon-counters.json", paths.counters_json.as_path()),
        ("pktmon-counters.txt", paths.counters_txt.as_path()),
        ("external.stdout.txt", paths.external_stdout.as_path()),
        ("external.stderr.txt", paths.external_stderr.as_path()),
        ("external-commands.json", paths.external_commands.as_path()),
    ]
}

fn write_support_zip(paths: &SupportPaths) -> anyhow::Result<()> {
    let file = File::create(&paths.zip_path)?;
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::default().compression_method(CompressionMethod::Stored);
    for (name, path) in artifact_specs(paths) {
        if !path.is_file() {
            continue;
        }
        zip.start_file(name, options)?;
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        zip.write_all(&buffer)?;
    }
    zip.finish()?;
    Ok(())
}

fn rotate_support_zips(support_dir: &Path, preserve: &Path) -> anyhow::Result<()> {
    let mut zips = fs::read_dir(support_dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("diagnostic-") && name.ends_with(".zip"))
        })
        .collect::<Vec<_>>();
    zips.sort();
    for path in zips {
        if path != preserve {
            let _ = fs::remove_file(path);
        }
    }
    Ok(())
}

fn cleanup_artifact_files(paths: &SupportPaths) {
    for (_, path) in artifact_specs(paths) {
        let _ = fs::remove_file(path);
    }
}

fn update_status(
    runtime: &Arc<DiagnosticRuntimeSession>,
    state: &str,
    stage: &str,
    progress: f64,
    support_zip_path: Option<String>,
    summary: Option<DiagnosticStatusSummary>,
) {
    let mut status = runtime.status.lock().expect("diagnostic status lock");
    status.state = state.to_string();
    status.stage = stage.to_string();
    status.progress = progress.clamp(0.0, 1.0);
    status.updated_at = now_seconds();
    status.elapsed_seconds = status.updated_at - status.started_at;
    if support_zip_path.is_some() {
        status.support_zip_path = support_zip_path;
    }
    if summary.is_some() {
        status.summary = summary;
    }
}

fn diagnostic_status_inner(
    state: &State<'_, AppState>,
    session_id: &str,
) -> Result<DiagnosticStatus, ApiError> {
    let session = diagnostic_runtime_session(state, session_id)?;
    let status = session
        .status
        .lock()
        .map_err(|_| api_error_message("diagnostic_lock_poisoned", "diagnostic lock poisoned"))?
        .clone();
    cleanup_terminal_diagnostic_session(state, session_id, &session, &status)?;
    Ok(status)
}

fn diagnostic_runtime_session(
    state: &State<'_, AppState>,
    session_id: &str,
) -> Result<Arc<DiagnosticRuntimeSession>, ApiError> {
    state
        .diagnostic_sessions
        .lock()
        .map_err(|_| api_error_message("diagnostic_lock_poisoned", "diagnostic lock poisoned"))?
        .get(session_id)
        .cloned()
        .ok_or_else(|| api_error_message("diagnostic_not_found", "diagnostic session not found"))
}

fn cleanup_terminal_diagnostic_session(
    state: &State<'_, AppState>,
    session_id: &str,
    session: &DiagnosticRuntimeSession,
    status: &DiagnosticStatus,
) -> Result<(), ApiError> {
    if !diagnostic_status_is_terminal(status) {
        return Ok(());
    }
    let _ = try_join_finished_diagnostic_thread(session);
    let mut sessions = state
        .diagnostic_sessions
        .lock()
        .map_err(|_| api_error_message("diagnostic_lock_poisoned", "diagnostic lock poisoned"))?;
    prune_diagnostic_session_map(&mut sessions, session_id, now_seconds());
    Ok(())
}

fn prune_diagnostic_session_map(
    sessions: &mut HashMap<String, Arc<DiagnosticRuntimeSession>>,
    preserve_session_id: &str,
    now: f64,
) {
    for session in sessions.values() {
        let is_terminal = session
            .status
            .lock()
            .map(|status| diagnostic_status_is_terminal(&status))
            .unwrap_or(false);
        if is_terminal {
            let _ = try_join_finished_diagnostic_thread(session);
        }
    }

    let removable = sessions
        .iter()
        .filter_map(|(session_id, session)| {
            if session_id == preserve_session_id || !diagnostic_handle_joined(session) {
                return None;
            }
            let status = session.status.lock().ok()?;
            diagnostic_status_is_terminal(&status).then(|| (session_id.clone(), status.updated_at))
        })
        .collect::<Vec<_>>();

    let mut to_remove = BTreeSet::new();
    for (session_id, updated_at) in &removable {
        if now - *updated_at >= DIAGNOSTIC_SESSION_RETENTION_SECONDS {
            to_remove.insert(session_id.clone());
        }
    }

    let recent = removable
        .into_iter()
        .filter(|(session_id, _)| !to_remove.contains(session_id))
        .collect::<Vec<_>>();
    if recent.len() > DIAGNOSTIC_TERMINAL_SESSION_LIMIT {
        let extra = recent.len() - DIAGNOSTIC_TERMINAL_SESSION_LIMIT;
        let mut recent = recent;
        recent.sort_by(|left, right| {
            left.1
                .total_cmp(&right.1)
                .then_with(|| left.0.cmp(&right.0))
        });
        for (session_id, _) in recent.into_iter().take(extra) {
            to_remove.insert(session_id);
        }
    }

    for session_id in to_remove {
        sessions.remove(&session_id);
    }
}

fn diagnostic_status_is_terminal(status: &DiagnosticStatus) -> bool {
    matches!(status.state.as_str(), "completed" | "failed")
}

fn diagnostic_handle_joined(session: &DiagnosticRuntimeSession) -> bool {
    session
        .handle
        .lock()
        .map(|handle| handle.is_none())
        .unwrap_or(false)
}

fn try_join_finished_diagnostic_thread(session: &DiagnosticRuntimeSession) -> bool {
    let handle = session.handle.lock().ok().and_then(|mut guard| {
        guard
            .as_ref()
            .is_some_and(std::thread::JoinHandle::is_finished)
            .then(|| guard.take())
            .flatten()
    });
    let Some(handle) = handle else {
        return false;
    };
    handle.join().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_no_packets_seen() {
        let result = classify_capture_result(
            &DiagnosticCaptureCounters::default(),
            &DiagnosticCaptureSummary::default(),
            Vec::new(),
        );
        assert_eq!(result.verdict, "no_packets_seen");
    }

    #[test]
    fn classifies_idle_packets_without_markers() {
        let counters = DiagnosticCaptureCounters {
            packets_seen: 10,
            raw_packets_written: 10,
            ..Default::default()
        };
        let summary = DiagnosticCaptureSummary {
            small_parsed_payload_packets: 9,
            ..Default::default()
        };
        let result = classify_capture_result(&counters, &summary, Vec::new());
        assert_eq!(result.verdict, "only_idle_packets");
    }

    #[test]
    fn classifies_high_parser_drop_before_idle() {
        let counters = DiagnosticCaptureCounters {
            packets_seen: 10,
            dropped_packets: 6,
            ..Default::default()
        };
        let result =
            classify_capture_result(&counters, &DiagnosticCaptureSummary::default(), Vec::new());
        assert_eq!(result.verdict, "high_parser_drop");
    }

    #[test]
    fn classifies_decoded_ok() {
        let counters = DiagnosticCaptureCounters {
            packets_seen: 10,
            decoded_packets: 1,
            ..Default::default()
        };
        let result =
            classify_capture_result(&counters, &DiagnosticCaptureSummary::default(), Vec::new());
        assert_eq!(result.verdict, "decoded_ok");
    }
}
