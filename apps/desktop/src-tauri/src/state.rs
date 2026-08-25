use std::collections::HashMap;
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex, MutexGuard,
    atomic::{AtomicBool, Ordering},
};
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
    shutdown_started: AtomicBool,
    session_registration: Mutex<()>,
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
            shutdown_started: AtomicBool::new(false),
            session_registration: Mutex::new(()),
        }
    }

    pub(crate) fn session_registration(&self) -> Option<MutexGuard<'_, ()>> {
        let guard = self
            .session_registration
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if self.shutdown_started.load(Ordering::SeqCst) {
            None
        } else {
            Some(guard)
        }
    }

    pub(crate) fn shutdown(&self) {
        if !crate::lifecycle::begin_shutdown(&self.shutdown_started) {
            return;
        }
        let registration = self
            .session_registration
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let capture_sessions = crate::capture::request_capture_shutdown(self);
        let diagnostic_sessions = crate::diagnostic::request_diagnostic_shutdown(self);
        drop(registration);
        crate::capture::join_capture_shutdown(&capture_sessions);
        crate::diagnostic::join_diagnostic_shutdown(&diagnostic_sessions);
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
    resolve_portable_root(
        env::var_os("NTE_GACHA_EXPORTER_PORTABLE_ROOT"),
        env::var_os("NTE_GACHA_EXPORTER_ROOT"),
        env::current_exe().ok(),
        env::current_dir().ok(),
    )
    .ok_or_else(|| std::io::Error::other("cannot resolve current executable or current directory"))
}

fn resolve_portable_root(
    portable_root: Option<OsString>,
    root: Option<OsString>,
    current_exe: Option<PathBuf>,
    current_dir: Option<PathBuf>,
) -> Option<PathBuf> {
    non_empty_path(portable_root)
        .or_else(|| non_empty_path(root))
        .or_else(|| current_exe.as_deref().and_then(portable_root_from_exe))
        .or(current_dir)
}

fn non_empty_path(value: Option<OsString>) -> Option<PathBuf> {
    value.filter(|value| !value.is_empty()).map(PathBuf::from)
}

fn portable_root_from_exe(executable: &Path) -> Option<PathBuf> {
    let executable_dir = executable.parent()?;
    if executable_dir
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("app"))
    {
        let packaged_root = executable_dir.parent()?;
        if packaged_root.join(launcher_exe_name()).is_file() {
            return Some(packaged_root.to_path_buf());
        }
    }
    Some(executable_dir.to_path_buf())
}

fn launcher_exe_name() -> &'static str {
    if cfg!(windows) {
        "nte-gacha-exporter.exe"
    } else {
        "nte-gacha-exporter"
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn blocked_worker(
        release: Arc<(Mutex<bool>, std::sync::Condvar)>,
    ) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            let (released, wake) = &*release;
            let released = released
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            drop(
                wake.wait_while(released, |released| !*released)
                    .unwrap_or_else(|poisoned| poisoned.into_inner()),
            );
        })
    }

    fn test_state(root: &Path) -> Arc<AppState> {
        let store =
            JsonStore::open_with_defaults(root, crate::system_commands::store_defaults()).unwrap();
        Arc::new(AppState::new(store, None, None))
    }

    #[test]
    fn portable_root_prefers_explicit_sources() {
        let root = resolve_portable_root(
            Some(OsString::from("portable")),
            Some(OsString::from("root")),
            Some(PathBuf::from("fallback/app.exe")),
            Some(PathBuf::from("cwd")),
        );
        assert_eq!(root, Some(PathBuf::from("portable")));

        let root = resolve_portable_root(
            Some(OsString::new()),
            Some(OsString::from("root")),
            Some(PathBuf::from("fallback/app.exe")),
            Some(PathBuf::from("cwd")),
        );
        assert_eq!(root, Some(PathBuf::from("root")));
    }

    #[test]
    fn packaged_desktop_resolves_parent_portable_root() {
        let temp = tempfile::tempdir().unwrap();
        let app_dir = temp.path().join("app");
        std::fs::create_dir(&app_dir).unwrap();
        std::fs::write(temp.path().join(launcher_exe_name()), b"").unwrap();

        let root = portable_root_from_exe(&app_dir.join("nte-gacha-exporter-desktop.exe"));

        assert_eq!(root.as_deref(), Some(temp.path()));
    }

    #[test]
    fn development_executable_uses_its_parent() {
        let executable = Path::new("target/debug/nte-gacha-exporter-desktop");

        assert_eq!(
            portable_root_from_exe(executable),
            Some(PathBuf::from("target/debug"))
        );
    }

    #[test]
    fn shutdown_waits_for_session_registration_and_rejects_new_sessions() {
        let temp = tempfile::tempdir().unwrap();
        let state = test_state(temp.path());
        let registration = state.session_registration().unwrap();
        let shutdown_state = Arc::clone(&state);
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            shutdown_state.shutdown();
            done_tx.send(()).unwrap();
        });

        started_rx.recv().unwrap();
        assert!(
            done_rx
                .recv_timeout(std::time::Duration::from_millis(50))
                .is_err()
        );
        drop(registration);
        done_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .unwrap();
        handle.join().unwrap();

        assert!(state.session_registration().is_none());
        state.shutdown();
    }

    #[test]
    fn shutdown_requests_all_sessions_before_joining_workers() {
        let temp = tempfile::tempdir().unwrap();
        let state = test_state(temp.path());
        let release = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
        let (capture_a, capture_a_flags) = crate::capture::tests::shutdown_session_probe(
            "capture-a",
            blocked_worker(Arc::clone(&release)),
        );
        let (capture_b, capture_b_flags) = crate::capture::tests::shutdown_session_probe(
            "capture-b",
            blocked_worker(Arc::clone(&release)),
        );
        let (diagnostic_a, diagnostic_a_cancel) = crate::diagnostic::tests::shutdown_session_probe(
            "diagnostic-a",
            blocked_worker(Arc::clone(&release)),
        );
        let (diagnostic_b, diagnostic_b_cancel) = crate::diagnostic::tests::shutdown_session_probe(
            "diagnostic-b",
            blocked_worker(Arc::clone(&release)),
        );
        {
            let mut sessions = state.capture_sessions.lock().unwrap();
            sessions.insert("capture-a".to_string(), Arc::clone(&capture_a));
            sessions.insert("capture-b".to_string(), Arc::clone(&capture_b));
        }
        {
            let mut sessions = state.diagnostic_sessions.lock().unwrap();
            sessions.insert("diagnostic-a".to_string(), Arc::clone(&diagnostic_a));
            sessions.insert("diagnostic-b".to_string(), Arc::clone(&diagnostic_b));
        }

        let shutdown_state = Arc::clone(&state);
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let shutdown = std::thread::spawn(move || {
            shutdown_state.shutdown();
            done_tx.send(()).unwrap();
        });
        let signals = [
            Arc::clone(&capture_a_flags[0]),
            Arc::clone(&capture_a_flags[1]),
            Arc::clone(&capture_b_flags[0]),
            Arc::clone(&capture_b_flags[1]),
            diagnostic_a_cancel,
            diagnostic_b_cancel,
        ];
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let all_requested = loop {
            let requested = signals.iter().all(|signal| signal.load(Ordering::SeqCst));
            if requested || std::time::Instant::now() >= deadline {
                break requested;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        };
        let waiting_for_workers = done_rx
            .recv_timeout(std::time::Duration::from_millis(50))
            .is_err();
        {
            let (released, wake) = &*release;
            *released.lock().unwrap() = true;
            wake.notify_all();
        }
        done_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .unwrap();
        shutdown.join().unwrap();

        assert!(all_requested);
        assert!(waiting_for_workers);
        state.shutdown();
    }
}
