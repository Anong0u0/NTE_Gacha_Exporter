use std::{
    collections::HashSet,
    fs,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow, bail};

use crate::{
    api::wait_health,
    cli::{AGENT_BUILD_SCRIPT, DEFAULT_AGENT_APP_ROOT, DEFAULT_OUT_DIR},
    report::{AgentBuildManifest, LaunchOutput},
    util::{canonical_or_self, ensure_file, unix_secs},
};

pub fn run_agent_build(force: bool) -> Result<()> {
    let mut command = Command::new("powershell.exe");
    command
        .arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-File")
        .arg(AGENT_BUILD_SCRIPT);
    if force {
        command.arg("-Force");
    }
    let status = command
        .status()
        .with_context(|| format!("failed to start {AGENT_BUILD_SCRIPT}"))?;
    if status.success() {
        Ok(())
    } else {
        bail!("agent app build failed with exit code {status}")
    }
}

pub fn ensure_agent_app_fresh() -> Result<()> {
    run_agent_build(false)
}

pub fn run_agent_launch(addr: &str, timeout: Duration) -> Result<LaunchOutput> {
    ensure_agent_app_fresh()?;
    let portable_root_input = default_agent_app_root();
    let portable_root = portable_root_input.canonicalize().with_context(|| {
        format!(
            "agent app root not found: {}; run `cargo agent build` first",
            portable_root_input.display()
        )
    })?;
    let launcher = portable_root.join("nte-gacha-exporter.exe");
    let desktop = portable_root
        .join("app")
        .join("nte-gacha-exporter-desktop.exe");
    ensure_file(&launcher)?;
    ensure_file(&desktop)?;

    prepare_agent_addr(addr, timeout)?;

    let child = launch_app(&launcher, &portable_root, addr)?;
    let health = wait_health(addr, timeout)?;
    let app_pid = health.get("pid").and_then(serde_json::Value::as_u64);
    Ok(LaunchOutput {
        launcher_pid: child.id(),
        app_pid,
        addr: addr.to_string(),
        root: portable_root.display().to_string(),
        launcher: launcher.display().to_string(),
        health,
    })
}

pub fn default_agent_app_root() -> PathBuf {
    PathBuf::from(DEFAULT_AGENT_APP_ROOT)
}

pub fn agent_build_manifest_path() -> PathBuf {
    PathBuf::from(DEFAULT_OUT_DIR).join("app-current.build.json")
}

pub fn read_agent_build_manifest() -> Result<AgentBuildManifest> {
    let path = agent_build_manifest_path();
    let text = fs::read_to_string(&path)
        .with_context(|| format!("agent build manifest missing: {}", path.display()))?;
    let manifest = serde_json::from_str::<AgentBuildManifest>(&text)
        .with_context(|| format!("invalid agent build manifest: {}", path.display()))?;
    if manifest.schema != "nte-agent-smoke-build" || manifest.schema_version != 1 {
        bail!("unsupported agent build manifest: {}", path.display());
    }
    Ok(manifest)
}

pub fn stage_portable(source: &Path, destination: &Path) -> Result<()> {
    if destination.exists() {
        bail!(
            "portable destination already exists: {}",
            destination.display()
        );
    }
    for entry in walk_files(source)? {
        let relative = entry.strip_prefix(source)?;
        if relative
            .components()
            .next()
            .and_then(|component| component.as_os_str().to_str())
            .is_some_and(|name| name == "data" || name == "update")
        {
            continue;
        }
        let target = destination.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&target)?;
        } else if entry.is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&entry, &target)
                .with_context(|| format!("copy {} -> {}", entry.display(), target.display()))?;
        }
    }
    Ok(())
}

pub fn launch_app(launcher: &Path, portable_root: &Path, addr: &str) -> Result<Child> {
    Command::new(launcher)
        .current_dir(portable_root)
        .env("NTE_AGENT_SMOKE", "1")
        .env("NTE_AGENT_SMOKE_ADDR", addr)
        .env("NTE_GACHA_EXPORTER_PORTABLE_ROOT", portable_root)
        .env("NTE_GACHA_EXPORTER_ROOT", portable_root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("launch failed: {}", launcher.display()))
}

pub fn ensure_addr_available(addr: &str) -> Result<()> {
    let listener = TcpListener::bind(addr)
        .with_context(|| format!("agent smoke addr is already in use or invalid: {addr}"))?;
    drop(listener);
    Ok(())
}

pub fn prepare_agent_addr(addr: &str, timeout: Duration) -> Result<()> {
    stop_addr_owner_if_nte(addr)?;
    wait_addr_available(addr, timeout)
}

pub fn remove_portable_copy(run_dir: &Path, portable_root: &Path) -> Result<bool> {
    if portable_root.file_name().and_then(|name| name.to_str()) != Some("portable")
        || portable_root.parent() != Some(run_dir)
    {
        bail!(
            "refusing to remove unexpected portable path: {}",
            portable_root.display()
        );
    }
    if !portable_root.exists() {
        return Ok(false);
    }
    fs::remove_dir_all(portable_root)
        .with_context(|| format!("remove portable copy failed: {}", portable_root.display()))?;
    Ok(true)
}

pub fn rotate_run_dirs(
    base: &Path,
    current_run_dir: &Path,
    keep_runs: usize,
) -> Result<Vec<PathBuf>> {
    if keep_runs == 0 {
        bail!("keep_runs must be at least 1");
    }
    if !base.is_dir() {
        return Ok(Vec::new());
    }

    let current = canonical_or_self(current_run_dir);
    let mut runs = Vec::new();
    for entry in fs::read_dir(base)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(timestamp) = run_dir_timestamp(name) else {
            continue;
        };
        runs.push((timestamp, name.to_string(), path));
    }

    runs.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1)));

    let mut keep = HashSet::from([current]);
    for (_, _, path) in &runs {
        if keep.len() >= keep_runs {
            break;
        }
        keep.insert(canonical_or_self(path));
    }

    let mut removed = Vec::new();
    for (_, _, path) in runs {
        if keep.contains(&canonical_or_self(&path)) {
            continue;
        }
        fs::remove_dir_all(&path)
            .with_context(|| format!("remove old smoke run failed: {}", path.display()))?;
        removed.push(path);
    }
    Ok(removed)
}

pub fn run_dir_timestamp(name: &str) -> Option<u64> {
    let suffix = name.strip_prefix("run-")?;
    let (timestamp, pid) = suffix.split_once('-')?;
    if pid.is_empty() || !pid.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    timestamp.parse().ok()
}

pub fn new_run_dir(base: &Path) -> Result<PathBuf> {
    fs::create_dir_all(base)?;
    let run_dir = base.join(format!("run-{}-{}", unix_secs(), std::process::id()));
    fs::create_dir(&run_dir)
        .with_context(|| format!("create run dir failed: {}", run_dir.display()))?;
    Ok(run_dir)
}

fn walk_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(path) = stack.pop() {
        out.push(path.clone());
        if path.is_dir() {
            for entry in fs::read_dir(&path)? {
                stack.push(entry?.path());
            }
        }
    }
    Ok(out)
}

fn wait_addr_available(addr: &str, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut last_error = None;
    while Instant::now() < deadline {
        match ensure_addr_available(addr) {
            Ok(()) => return Ok(()),
            Err(error) => last_error = Some(error),
        }
        thread::sleep(Duration::from_millis(250));
    }
    match last_error {
        Some(error) => Err(error).context("agent addr did not become available"),
        None => bail!("agent addr did not become available"),
    }
}

fn stop_addr_owner_if_nte(addr: &str) -> Result<()> {
    let port = addr
        .rsplit_once(':')
        .and_then(|(_, port)| port.parse::<u16>().ok())
        .ok_or_else(|| anyhow!("agent addr must include a numeric port: {addr}"))?;
    let script = r#"
$ErrorActionPreference = "Stop"
$Port = __PORT__
$conn = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1
if ($null -eq $conn) {
    exit 0
}
$process = Get-Process -Id $conn.OwningProcess -ErrorAction Stop
$name = $process.ProcessName
if ($name -ne "nte-gacha-exporter" -and $name -ne "nte-gacha-exporter-desktop") {
    throw "agent addr port $Port is owned by non-NTE process: pid=$($process.Id) name=$name"
}
if ($process.MainWindowHandle -ne 0) {
    [void]$process.CloseMainWindow()
    Start-Sleep -Milliseconds 1500
    $process.Refresh()
}
if (-not $process.HasExited) {
    Stop-Process -Id $process.Id -Force
}
exit 0
"#
    .replace("__PORT__", &port.to_string());
    let output = Command::new("powershell.exe")
        .arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-Command")
        .arg(script)
        .output()
        .context("failed to inspect agent addr owner")?;
    if output.status.success() {
        Ok(())
    } else {
        bail!(
            "failed to clear agent addr {addr}: stdout={}; stderr={}",
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        )
    }
}

#[cfg(test)]
mod tests {
    use std::{env, fs, path::PathBuf};

    use super::*;

    #[test]
    fn rotate_run_dirs_keeps_current_and_latest_only() {
        let temp = temp_dir("rotate");
        let base = temp.join("agent-smoke");
        let old = base.join("run-100-1");
        let middle = base.join("run-200-1");
        let current = base.join("run-300-1");
        fs::create_dir_all(old.join("logs")).unwrap();
        fs::create_dir_all(middle.join("logs")).unwrap();
        fs::create_dir_all(current.join("logs")).unwrap();
        fs::create_dir_all(base.join("app-current")).unwrap();
        fs::write(base.join("latest-report.json"), "{}").unwrap();

        let removed = rotate_run_dirs(&base, &current, 1).unwrap();

        assert_eq!(removed.len(), 2);
        assert!(!old.exists());
        assert!(!middle.exists());
        assert!(current.exists());
        assert!(base.join("app-current").exists());
        assert!(base.join("latest-report.json").exists());

        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn rotate_run_dirs_ignores_invalid_run_names() {
        let temp = temp_dir("invalid");
        let base = temp.join("agent-smoke");
        let current = base.join("run-300-1");
        let invalid = base.join("run-not-a-timestamp");
        fs::create_dir_all(&current).unwrap();
        fs::create_dir_all(&invalid).unwrap();

        let removed = rotate_run_dirs(&base, &current, 1).unwrap();

        assert!(removed.is_empty());
        assert!(current.exists());
        assert!(invalid.exists());

        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn rotate_run_dirs_counts_current_against_keep_limit() {
        let temp = temp_dir("current-limit");
        let base = temp.join("agent-smoke");
        let current = base.join("run-100-1");
        let newer = base.join("run-300-1");
        fs::create_dir_all(&current).unwrap();
        fs::create_dir_all(&newer).unwrap();

        let removed = rotate_run_dirs(&base, &current, 1).unwrap();

        assert_eq!(removed, vec![newer]);
        assert!(current.exists());
        assert!(!base.join("run-300-1").exists());

        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn remove_portable_copy_only_removes_run_portable_dir() {
        let temp = temp_dir("portable");
        let run_dir = temp.join("run-100-1");
        let portable = run_dir.join("portable");
        fs::create_dir_all(&portable).unwrap();
        fs::write(portable.join("nte-gacha-exporter.exe"), "").unwrap();

        assert!(remove_portable_copy(&run_dir, &portable).unwrap());
        assert!(!portable.exists());
        assert!(!remove_portable_copy(&run_dir, &portable).unwrap());

        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn remove_portable_copy_rejects_unexpected_path() {
        let temp = temp_dir("reject");
        let run_dir = temp.join("run-100-1");
        let unexpected = temp.join("portable");
        fs::create_dir_all(&unexpected).unwrap();

        assert!(remove_portable_copy(&run_dir, &unexpected).is_err());
        assert!(unexpected.exists());

        let _ = fs::remove_dir_all(temp);
    }

    fn temp_dir(name: &str) -> PathBuf {
        let path = env::temp_dir().join(format!(
            "nte-agent-smoke-{name}-{}-{}",
            std::process::id(),
            unix_secs()
        ));
        if path.exists() {
            let _ = fs::remove_dir_all(&path);
        }
        fs::create_dir_all(&path).unwrap();
        path
    }
}
