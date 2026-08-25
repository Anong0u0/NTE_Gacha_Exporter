use std::env;
use std::path::PathBuf;
use std::process::Command;
#[cfg(not(windows))]
use std::thread;
use std::time::Duration;
#[cfg(not(windows))]
use std::time::Instant;

#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{CloseHandle, ERROR_INVALID_PARAMETER, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
};

use nte_update::{apply_staged_update, cleanup_update_artifacts_after_success};

const APP_EXIT_TIMEOUT: Duration = Duration::from_secs(60);

fn main() {
    let code = match run() {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    };
    std::process::exit(code);
}

fn run() -> Result<(), String> {
    let args = Args::parse(env::args().skip(1).collect())?;
    wait_for_pid(args.app_pid, APP_EXIT_TIMEOUT)?;
    apply_staged_update(&args.root, &args.version).map_err(|error| error.to_string())?;
    let _ = cleanup_update_artifacts_after_success(&args.root);
    if args.relaunch {
        let launcher = args.root.join(launcher_exe_name());
        if launcher.is_file() {
            Command::new(launcher)
                .current_dir(&args.root)
                .spawn()
                .map_err(|error| format!("failed to relaunch app: {error}"))?;
        }
    }
    Ok(())
}

struct Args {
    root: PathBuf,
    version: String,
    app_pid: u32,
    relaunch: bool,
}

impl Args {
    fn parse(raw: Vec<String>) -> Result<Self, String> {
        let mut root = None;
        let mut version = None;
        let mut app_pid = None;
        let mut relaunch = false;
        let mut index = 0;
        while index < raw.len() {
            match raw[index].as_str() {
                "--root" => {
                    index += 1;
                    root = raw.get(index).map(PathBuf::from);
                }
                "--version" => {
                    index += 1;
                    version = raw.get(index).cloned();
                }
                "--app-pid" => {
                    index += 1;
                    app_pid = raw.get(index).and_then(|value| value.parse::<u32>().ok());
                }
                "--relaunch" => {
                    relaunch = true;
                }
                other => return Err(format!("unknown argument: {other}")),
            }
            index += 1;
        }
        Ok(Self {
            root: root.ok_or_else(|| "--root is required".to_string())?,
            version: version.ok_or_else(|| "--version is required".to_string())?,
            app_pid: app_pid.ok_or_else(|| "--app-pid is required".to_string())?,
            relaunch,
        })
    }
}

#[cfg(windows)]
fn wait_for_pid(pid: u32, timeout: Duration) -> Result<(), String> {
    let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
    if handle.is_null() {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(ERROR_INVALID_PARAMETER as i32) {
            return Ok(());
        }
        return Err(format!(
            "failed to wait for application process {pid}: {error}"
        ));
    }

    let timeout_ms = timeout.as_millis().min(u128::from(u32::MAX)) as u32;
    let wait = unsafe { WaitForSingleObject(handle, timeout_ms) };
    let wait_error = (wait == WAIT_FAILED).then(std::io::Error::last_os_error);
    let _ = unsafe { CloseHandle(handle) };

    match wait {
        WAIT_OBJECT_0 => Ok(()),
        WAIT_TIMEOUT => Err(format!(
            "timed out waiting for application process {pid} to exit"
        )),
        WAIT_FAILED => Err(format!(
            "failed to wait for application process {pid}: {}",
            wait_error.expect("WAIT_FAILED must preserve its OS error")
        )),
        other => Err(format!(
            "failed to wait for application process {pid}: unexpected wait result {other}"
        )),
    }
}

#[cfg(not(windows))]
fn wait_for_pid(pid: u32, timeout: Duration) -> Result<(), String> {
    let started = Instant::now();
    loop {
        if !PathBuf::from(format!("/proc/{pid}")).exists() {
            return Ok(());
        }
        if started.elapsed() >= timeout {
            return Err(format!(
                "timed out waiting for application process {pid} to exit"
            ));
        }
        thread::sleep(Duration::from_millis(500).min(timeout.saturating_sub(started.elapsed())));
    }
}

fn launcher_exe_name() -> &'static str {
    if cfg!(windows) {
        "nte-gacha-exporter.exe"
    } else {
        "nte-gacha-exporter"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn updater_wait_child() {}

    #[test]
    fn wait_for_pid_times_out_while_process_is_alive() {
        let pid = std::process::id();

        let error = wait_for_pid(pid, Duration::ZERO).unwrap_err();

        assert_eq!(
            error,
            format!("timed out waiting for application process {pid} to exit")
        );
    }

    #[test]
    fn wait_for_pid_returns_after_process_exit() {
        let executable = std::env::current_exe().unwrap();
        let mut child = Command::new(executable)
            .args(["--ignored", "--exact", "tests::updater_wait_child"])
            .spawn()
            .unwrap();
        let pid = child.id();
        assert!(child.wait().unwrap().success());

        wait_for_pid(pid, Duration::from_secs(1)).unwrap();
    }
}
