use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;

use nte_update::apply_staged_update;

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
    wait_for_pid(args.app_pid);
    apply_staged_update(&args.root, &args.version).map_err(|error| error.to_string())?;
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

fn wait_for_pid(pid: u32) {
    for _ in 0..120 {
        if !pid_is_alive(pid) {
            return;
        }
        thread::sleep(Duration::from_millis(500));
    }
}

#[cfg(windows)]
fn pid_is_alive(pid: u32) -> bool {
    let output = Command::new("cmd")
        .args(["/C", "tasklist", "/FI", &format!("PID eq {pid}")])
        .output();
    output.is_ok_and(|output| String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()))
}

#[cfg(not(windows))]
fn pid_is_alive(pid: u32) -> bool {
    PathBuf::from(format!("/proc/{pid}")).exists()
}

fn launcher_exe_name() -> &'static str {
    if cfg!(windows) {
        "nte-gacha-exporter.exe"
    } else {
        "nte-gacha-exporter"
    }
}
