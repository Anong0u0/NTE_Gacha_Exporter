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

include!("diagnostic/types.rs");
include!("diagnostic/commands.rs");
include!("diagnostic/session.rs");
include!("diagnostic/internal_capture.rs");
include!("diagnostic/pktmon.rs");
include!("diagnostic/target.rs");
include!("diagnostic/classification.rs");
include!("diagnostic/artifacts.rs");
include!("diagnostic/status.rs");
include!("diagnostic/tests.rs");
