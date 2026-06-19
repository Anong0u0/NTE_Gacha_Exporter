use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use nte_automation::{
    AutoPageOptions as AutomationOptions, AutoPageResult as AutoPageRunResult,
    AutoPageStatus as AutomationStatus, RecordSnapshot as AutomationRecordSnapshot, run_auto_page,
};
use nte_capture::{
    CaptureOptions, CaptureRecordBuilder, CaptureTarget, build_capture_document, candidate_ports,
    capture_live, find_process_pid,
};
use nte_core::ImportReport;
use nte_store::load_locale_or_settings;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::State;

use crate::admin::admin_relaunch_required;
use crate::error::{ApiError, RuntimeError, api_error, api_error_message};
use crate::state::{AppState, new_session_id, now_seconds, with_store};

const CAPTURE_DRAIN_TIMEOUT: Duration = Duration::from_secs(20);
const CAPTURE_DRAIN_POLL_INTERVAL: Duration = Duration::from_millis(100);
const CAPTURE_SESSION_RETENTION_SECONDS: f64 = 30.0 * 60.0;
const CAPTURE_TERMINAL_SESSION_LIMIT: usize = 20;

#[derive(Debug, Clone)]
pub(crate) struct CaptureSessionMeta {
    profile_name: String,
    source_kind: String,
    source_path: Option<String>,
    full_update: bool,
    import_report: Option<ImportReport>,
}

pub(crate) struct CaptureRuntimeSession {
    status: Mutex<CaptureStatus>,
    stop: Arc<AtomicBool>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CaptureMode {
    LiveOnly,
    AutoPageIncremental,
    AutoPageFull,
}

impl CaptureMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::LiveOnly => "live_only",
            Self::AutoPageIncremental => "auto_page_incremental",
            Self::AutoPageFull => "auto_page_full",
        }
    }

    fn auto_page(self) -> bool {
        matches!(self, Self::AutoPageIncremental | Self::AutoPageFull)
    }

    fn full_update(self) -> bool {
        matches!(self, Self::AutoPageFull)
    }

    fn source_kind(self) -> &'static str {
        match self {
            Self::LiveOnly => "live_capture",
            Self::AutoPageIncremental => "auto_page_capture",
            Self::AutoPageFull => "auto_page_full",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct CaptureCounters {
    packets_seen: u64,
    decoded_packets: u64,
    dropped_packets: u64,
    #[serde(default)]
    duplicate_packets: u64,
    #[serde(default)]
    filter_restarts: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct CapturePageKey {
    stream_key: String,
    generation_index: u32,
    page_index: u32,
}

#[derive(Debug, Default)]
struct CapturePageTracker {
    pages_by_pool: BTreeMap<String, BTreeSet<CapturePageKey>>,
    last_decoded_at: Option<f64>,
}

impl CapturePageTracker {
    fn add_progress(&mut self, progress: &nte_capture::CaptureProgress) {
        let mut changed = false;
        for row in &progress.new_rows {
            let Some(pool) = capture_pool(row.record_type.as_str(), row.pool_id.as_deref()) else {
                continue;
            };
            let Some(page_index) = row.source.segment_index.or(row.source.page_index) else {
                continue;
            };
            let stream_key = row.source.stream_key.clone().unwrap_or_else(|| {
                format!(
                    "{}:{}",
                    row.record_type.as_str(),
                    row.pool_id.as_deref().unwrap_or_default()
                )
            });
            let key = CapturePageKey {
                stream_key,
                generation_index: row.source.generation_index.unwrap_or_default(),
                page_index,
            };
            changed |= self
                .pages_by_pool
                .entry(pool.to_string())
                .or_default()
                .insert(key);
        }
        if changed {
            self.last_decoded_at = Some(now_seconds());
        }
    }

    fn count(&self, pool: &str) -> usize {
        self.pages_by_pool
            .get(pool)
            .map(BTreeSet::len)
            .unwrap_or_default()
    }

    fn counts(&self) -> BTreeMap<String, usize> {
        self.pages_by_pool
            .iter()
            .map(|(pool, pages)| (pool.clone(), pages.len()))
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CaptureStatus {
    session_id: String,
    state: String,
    mode: String,
    records_count: u64,
    latest_records: Vec<Value>,
    counters: CaptureCounters,
    started_at: f64,
    updated_at: f64,
    target: Option<Value>,
    auto_page: Option<Value>,
    raw_path: Option<String>,
    error: Option<RuntimeError>,
    #[serde(default, skip_serializing)]
    document: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    import_report: Option<ImportReport>,
}

impl From<nte_capture::CaptureCounters> for CaptureCounters {
    fn from(value: nte_capture::CaptureCounters) -> Self {
        Self {
            packets_seen: value.packets_seen,
            decoded_packets: value.decoded_packets,
            dropped_packets: value.dropped_packets,
            duplicate_packets: value.duplicate_packets,
            filter_restarts: value.filter_restarts,
        }
    }
}

