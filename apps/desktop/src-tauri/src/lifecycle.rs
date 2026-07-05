use std::sync::Mutex;
use std::thread::JoinHandle;

pub(crate) const STATE_STARTING: &str = "starting";
pub(crate) const STATE_RUNNING: &str = "running";
pub(crate) const STATE_STOPPING: &str = "stopping";
pub(crate) const STATE_COMPLETED: &str = "completed";
pub(crate) const STATE_FAILED: &str = "failed";
pub(crate) const STATE_CANCELLED: &str = "cancelled";

pub(crate) fn is_active_state(state: &str) -> bool {
    matches!(state, STATE_STARTING | STATE_RUNNING | STATE_STOPPING)
}

pub(crate) fn is_terminal_state(state: &str) -> bool {
    matches!(state, STATE_COMPLETED | STATE_FAILED | STATE_CANCELLED)
}

pub(crate) fn set_stopping(state: &mut String) {
    if matches!(state.as_str(), STATE_STARTING | STATE_RUNNING) {
        *state = STATE_STOPPING.to_string();
    }
}

pub(crate) fn handle_joined(handle: &Mutex<Option<JoinHandle<()>>>) -> bool {
    handle
        .lock()
        .map(|handle| handle.is_none())
        .unwrap_or(false)
}

pub(crate) fn try_join_finished_thread(handle: &Mutex<Option<JoinHandle<()>>>) -> bool {
    let handle = handle.lock().ok().and_then(|mut guard| {
        guard
            .as_ref()
            .is_some_and(JoinHandle::is_finished)
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
    use std::thread;

    #[test]
    fn terminal_states_include_cancelled() {
        assert!(is_terminal_state(STATE_COMPLETED));
        assert!(is_terminal_state(STATE_FAILED));
        assert!(is_terminal_state(STATE_CANCELLED));
        assert!(!is_terminal_state(STATE_RUNNING));
    }

    #[test]
    fn set_stopping_only_changes_active_non_stopping_states() {
        let mut starting = STATE_STARTING.to_string();
        set_stopping(&mut starting);
        assert_eq!(starting, STATE_STOPPING);

        let mut failed = STATE_FAILED.to_string();
        set_stopping(&mut failed);
        assert_eq!(failed, STATE_FAILED);
    }

    #[test]
    fn try_join_finished_thread_joins_only_finished_handle() {
        let handle = Mutex::new(Some(thread::spawn(|| {})));
        while !handle.lock().unwrap().as_ref().unwrap().is_finished() {
            thread::yield_now();
        }

        assert!(try_join_finished_thread(&handle));
        assert!(handle.lock().unwrap().is_none());
        assert!(handle_joined(&handle));
    }
}
