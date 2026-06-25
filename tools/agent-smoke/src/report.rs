use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::window::{ImageMetrics, WindowInfo};

#[derive(Debug, Deserialize)]
pub struct ApiResponse {
    pub ok: bool,
    pub result: Option<Value>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub schema: &'static str,
    pub schema_version: u32,
    pub build: Option<AgentBuildManifest>,
    pub release_root: String,
    pub staged_portable_root: String,
    pub run_dir: String,
    pub addr: String,
    pub process: ProcessReport,
    pub steps: Vec<StepReport>,
    pub screenshots: Vec<ScreenshotReport>,
    pub errors: Vec<String>,
    pub final_snapshot_summary: Option<Value>,
    pub cleanup: CleanupReport,
    pub ok: bool,
    pub finished_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentBuildManifest {
    pub schema: String,
    pub schema_version: u32,
    pub version: String,
    pub fingerprint: String,
}

#[derive(Debug, Default, Serialize)]
pub struct ProcessReport {
    pub launcher_pid: Option<u32>,
    pub window_pid: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct StepReport {
    pub name: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct ScreenshotReport {
    pub name: String,
    pub path: String,
    pub window: WindowInfo,
    pub metrics: ImageMetrics,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentElement {
    pub id: String,
    pub tag: String,
    pub text: String,
    pub disabled: bool,
}

#[derive(Debug, Serialize)]
pub struct AgentIdsOutput {
    pub ids: Vec<AgentElement>,
}

#[derive(Debug, Serialize)]
pub struct ExpectTextOutput {
    pub text: String,
    pub matched: bool,
    pub elapsed_ms: u128,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LaunchOutput {
    pub launcher_pid: u32,
    pub app_pid: Option<u64>,
    pub addr: String,
    pub root: String,
    pub launcher: String,
    pub health: Value,
}

#[derive(Debug, Default, Serialize)]
pub struct CleanupReport {
    pub keep_runs: usize,
    pub portable_removed: bool,
    pub removed_run_dirs: Vec<String>,
    pub warnings: Vec<String>,
}
