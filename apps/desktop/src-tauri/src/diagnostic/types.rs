pub(crate) struct DiagnosticRuntimeSession {
    status: Mutex<DiagnosticStatus>,
    cancel_requested: Arc<AtomicBool>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) enum DiagnosticMode {
    #[serde(rename = "pktmon")]
    Pktmon,
    #[serde(rename = "windivert")]
    WinDivert,
}

impl DiagnosticMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pktmon => "pktmon",
            Self::WinDivert => "windivert",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DiagnosticStatus {
    session_id: String,
    mode: String,
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
    mode: String,
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
    pppoe_detection: PppoeDetection,
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
    capture_strategy: String,
    strategy_reason: String,
    pppoe_detection: PppoeDetection,
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
