use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use nte_core::GuiError;
use nte_store::JsonStore;
use tauri::State;

use crate::admin::{PendingAdminCapture, PendingAdminDiagnostic};
use crate::capture::{CaptureRuntimeSession, CaptureSessionMeta};
use crate::diagnostic::DiagnosticRuntimeSession;
use crate::error::{ApiError, api_error, api_error_message};

pub(crate) struct AppState {
    pub(crate) store: Mutex<JsonStore>,
    pub(crate) capture_sessions: Mutex<HashMap<String, Arc<CaptureRuntimeSession>>>,
    pub(crate) captures: Mutex<HashMap<String, CaptureSessionMeta>>,
    pub(crate) pending_admin_capture: Mutex<Option<PendingAdminCapture>>,
    pub(crate) diagnostic_sessions: Mutex<HashMap<String, Arc<DiagnosticRuntimeSession>>>,
    pub(crate) pending_admin_diagnostic: Mutex<Option<PendingAdminDiagnostic>>,
}

impl AppState {
    pub(crate) fn new(
        store: JsonStore,
        pending_admin_capture: Option<PendingAdminCapture>,
        pending_admin_diagnostic: Option<PendingAdminDiagnostic>,
    ) -> Self {
        Self {
            store: Mutex::new(store),
            capture_sessions: Mutex::new(HashMap::new()),
            captures: Mutex::new(HashMap::new()),
            pending_admin_capture: Mutex::new(pending_admin_capture),
            diagnostic_sessions: Mutex::new(HashMap::new()),
            pending_admin_diagnostic: Mutex::new(pending_admin_diagnostic),
        }
    }
}

pub(crate) fn with_store<T>(
    state: &State<'_, AppState>,
    f: impl FnOnce(&JsonStore) -> Result<T, GuiError>,
) -> Result<T, ApiError> {
    let store = state
        .store
        .lock()
        .map_err(|_| api_error_message("store_lock_poisoned", "store lock poisoned"))?;
    f(&store).map_err(api_error)
}

pub(crate) fn portable_root() -> Result<PathBuf, std::io::Error> {
    if let Ok(root) = env::var("NTE_GACHA_EXPORTER_PORTABLE_ROOT") {
        if !root.trim().is_empty() {
            return Ok(PathBuf::from(root));
        }
    }
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(PathBuf::from))
        .or_else(|| std::env::current_dir().ok())
        .ok_or_else(|| {
            std::io::Error::other("cannot resolve current executable or current directory")
        })
}

pub(crate) fn now_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs_f64())
        .unwrap_or_default()
}

pub(crate) fn new_session_id() -> String {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or_default();
    format!("rust-capture-{}-{stamp}", std::process::id())
}

pub(crate) fn new_named_session_id(prefix: &str) -> String {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or_default();
    format!("{prefix}-{}-{stamp}", std::process::id())
}
