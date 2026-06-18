use std::env;
use std::path::Path;
use std::process::{Command, Stdio};

const ROOT_ENV: &str = "NTE_GACHA_EXPORTER_ROOT";
const PORTABLE_ROOT_ENV: &str = "NTE_GACHA_EXPORTER_PORTABLE_ROOT";
const LAUNCHER_ENV: &str = "NTE_GACHA_EXPORTER_LAUNCHER";

fn main() {
    let exit_code = run().unwrap_or_else(|error| {
        eprintln!("{error}");
        1
    });
    std::process::exit(exit_code);
}

fn run() -> Result<i32, String> {
    let self_path =
        env::current_exe().map_err(|error| format!("failed to resolve launcher: {error}"))?;
    let root = self_path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "failed to resolve launcher directory".to_string())?;
    let app = root.join("app").join(app_exe_name());
    if !app.is_file() {
        return Err(format!("desktop app not found: {}", app.display()));
    }

    let mut command = Command::new(app);
    command
        .args(env::args_os().skip(1))
        .current_dir(&root)
        .env(ROOT_ENV, &root)
        .env(PORTABLE_ROOT_ENV, &root)
        .env(LAUNCHER_ENV, &self_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
        .spawn()
        .map_err(|error| format!("failed to start desktop app: {error}"))?;
    Ok(0)
}

fn app_exe_name() -> &'static str {
    if cfg!(windows) {
        "nte-gacha-exporter-desktop.exe"
    } else {
        "nte-gacha-exporter-desktop"
    }
}
