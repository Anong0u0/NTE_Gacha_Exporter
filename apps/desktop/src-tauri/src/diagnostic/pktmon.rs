const EXTERNAL_PKTMON_STAGING_DIR_FALLBACK: &str = r"C:\Windows\Temp";
const PKTMON_COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
const PKTMON_CLEANUP_TIMEOUT: Duration = Duration::from_secs(5);
const PKTMON_COMMAND_POLL_INTERVAL: Duration = Duration::from_millis(50);


fn run_external_pktmon_capture(
    paths: &SupportPaths,
    ports: &[u16],
    pppoe_detection: PppoeDetection,
    strategy: CaptureStrategy,
    duration: Duration,
    stop: Arc<AtomicBool>,
) -> ExternalCaptureReport {
    let mut runner = PktmonRunner::new(paths, pppoe_detection, strategy, Arc::clone(&stop));
    let staging_dir = external_pktmon_staging_dir();
    let staging_paths = external_staging_paths(paths, &staging_dir);

    if stop.load(Ordering::SeqCst) {
        return runner.finish();
    }

    if let Err(create_error) = fs::create_dir_all(&staging_dir) {
        runner.write_stderr_line(format!(
            "create external pktmon staging dir {} failed: {create_error}",
            staging_dir.display()
        ));
        runner.mark_error(format!(
            "create external pktmon staging dir failed: {create_error}"
        ));
        return runner.finish();
    }

    runner.run(&["filter", "remove"], "pktmon.exe filter remove failed");
    if strategy.kind == CaptureStrategyKind::PortFiltered {
        for port in ports {
            runner.add_port_filters(*port);
        }
    }

    let external_etl = staging_paths.etl.to_string_lossy().to_string();
    let external_pcapng = staging_paths.pcapng.to_string_lossy().to_string();
    runner.run(
        &[
            "start",
            "--capture",
            "--pkt-size",
            "0",
            "--file-name",
            &external_etl,
        ],
        "pktmon.exe start failed",
    );

    let started = Instant::now();
    while started.elapsed() < duration && !stop.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(250));
    }

    if stop.load(Ordering::SeqCst) {
        runner.run_cleanup(&["stop"], "pktmon.exe stop failed");
        runner.run_cleanup(
            &["filter", "remove"],
            "pktmon.exe filter remove failed",
        );
        runner.cleanup_staging_files(&staging_paths);
        return runner.finish();
    }

    runner.capture_output(
        &["counters", "--json"],
        &paths.counters_json,
        "pktmon.exe counters --json failed",
    );
    runner.capture_output(
        &["counters"],
        &paths.counters_txt,
        "pktmon.exe counters failed",
    );
    runner.run_cleanup(&["stop"], "pktmon.exe stop failed");
    runner.run(
        &["pcapng", &external_etl, "-o", &external_pcapng],
        "pktmon.exe pcapng failed",
    );
    runner.run_cleanup(
        &["filter", "remove"],
        "pktmon.exe filter remove failed",
    );
    runner.copy_staging_artifact(&staging_paths.etl, &paths.external_etl, "ETL");
    runner.copy_staging_artifact(
        &staging_paths.pcapng,
        &paths.external_pcapng,
        "PCAPNG",
    );
    runner.cleanup_staging_files(&staging_paths);
    runner.finish()
}

struct PktmonRunner<'a> {
    paths: &'a SupportPaths,
    commands: Vec<ExternalCommandLog>,
    stdout_log: Vec<u8>,
    stderr_log: Vec<u8>,
    ok: bool,
    error: Option<String>,
    pppoe_detection: PppoeDetection,
    strategy: CaptureStrategy,
    cancel: Arc<AtomicBool>,
}

impl<'a> PktmonRunner<'a> {
    fn new(
        paths: &'a SupportPaths,
        pppoe_detection: PppoeDetection,
        strategy: CaptureStrategy,
        cancel: Arc<AtomicBool>,
    ) -> Self {
        Self {
            paths,
            commands: Vec::new(),
            stdout_log: Vec::new(),
            stderr_log: Vec::new(),
            ok: true,
            error: None,
            pppoe_detection,
            strategy,
            cancel,
        }
    }

    fn add_port_filters(&mut self, port: u16) {
        let udp_name = format!("NTE_DIAG_UDP_{port}");
        let tcp_name = format!("NTE_DIAG_TCP_{port}");
        let port_arg = port.to_string();

        self.run(
            &["filter", "add", &udp_name, "-t", "UDP", "-p", &port_arg],
            format!("pktmon.exe add UDP filter for port {port} failed"),
        );
        self.run(
            &["filter", "add", &tcp_name, "-t", "TCP", "-p", &port_arg],
            format!("pktmon.exe add TCP filter for port {port} failed"),
        );
    }

    fn run(&mut self, args: &[&str], error_message: impl Into<String>) {
        if !self.run_command(args) {
            self.mark_error(error_message);
        }
    }

    fn capture_output(
        &mut self,
        args: &[&str],
        output_path: &Path,
        error_message: impl Into<String>,
    ) {
        let execution = run_command(
            "pktmon.exe",
            args,
            Some(self.cancel.as_ref()),
            PKTMON_COMMAND_TIMEOUT,
        );
        let success = execution.log.success;
        if let Err(write_error) = fs::write(output_path, &execution.stdout) {
            self.mark_error(format!("write pktmon output failed: {write_error}"));
        }
        self.record_command(execution);
        if !success {
            self.mark_error(error_message);
        }
    }

    fn run_command(&mut self, args: &[&str]) -> bool {
        let execution = run_command(
            "pktmon.exe",
            args,
            Some(self.cancel.as_ref()),
            PKTMON_COMMAND_TIMEOUT,
        );
        let success = execution.log.success;
        self.record_command(execution);
        success
    }

    fn run_cleanup(&mut self, args: &[&str], error_message: impl Into<String>) {
        let execution = run_command("pktmon.exe", args, None, PKTMON_CLEANUP_TIMEOUT);
        let success = execution.log.success;
        self.record_command(execution);
        if !success {
            self.mark_error(error_message);
        }
    }

    fn record_command(&mut self, execution: CommandExecution) {
        let command_line = execution.log.args.join(" ");
        let _ = writeln!(
            self.stdout_log,
            "\n> {} {}",
            execution.log.program, command_line
        );
        self.stdout_log.extend_from_slice(&execution.stdout);
        let _ = writeln!(
            self.stderr_log,
            "\n> {} {}",
            execution.log.program, command_line
        );
        self.stderr_log.extend_from_slice(&execution.stderr);
        self.commands.push(execution.log);
    }

    fn copy_staging_artifact(&mut self, staging_path: &Path, artifact_path: &Path, label: &str) {
        if !staging_path.is_file() {
            self.mark_error(format!("external pktmon {label} missing after capture"));
            return;
        }
        if let Err(copy_error) = fs::copy(staging_path, artifact_path) {
            self.mark_error(format!("copy external pktmon {label} failed: {copy_error}"));
        }
    }

    fn cleanup_staging_files(&mut self, paths: &ExternalStagingPaths) {
        let _ = fs::remove_file(&paths.etl);
        let _ = fs::remove_file(&paths.pcapng);
    }

    fn write_stderr_line(&mut self, message: String) {
        let _ = writeln!(self.stderr_log, "{message}");
    }

    fn mark_error(&mut self, message: impl Into<String>) {
        self.ok = false;
        if self.error.is_none() {
            self.error = Some(message.into());
        }
    }

    fn finish(mut self) -> ExternalCaptureReport {
        if let Err(write_error) = fs::write(&self.paths.external_stdout, &self.stdout_log) {
            self.mark_error(format!("write external stdout log failed: {write_error}"));
        }
        if let Err(write_error) = fs::write(&self.paths.external_stderr, &self.stderr_log) {
            self.mark_error(format!("write external stderr log failed: {write_error}"));
        }
        if let Err(write_error) = fs::write(
            &self.paths.external_commands,
            serde_json::to_vec_pretty(&self.commands).unwrap_or_default(),
        ) {
            self.mark_error(format!("write external command log failed: {write_error}"));
        }

        ExternalCaptureReport {
            attempted: true,
            ok: self.ok,
            error: self.error,
            capture_strategy: self.strategy.kind.as_str().to_string(),
            strategy_reason: self.strategy.reason.as_str().to_string(),
            pppoe_detection: self.pppoe_detection,
            etl_path: Some(self.paths.external_etl.to_string_lossy().to_string()),
            pcapng_path: Some(self.paths.external_pcapng.to_string_lossy().to_string()),
            stdout_log_path: Some(self.paths.external_stdout.to_string_lossy().to_string()),
            stderr_log_path: Some(self.paths.external_stderr.to_string_lossy().to_string()),
            counters_json_path: Some(self.paths.counters_json.to_string_lossy().to_string()),
            counters_txt_path: Some(self.paths.counters_txt.to_string_lossy().to_string()),
            command_log_path: Some(self.paths.external_commands.to_string_lossy().to_string()),
            commands: self.commands,
        }
    }
}

struct ExternalStagingPaths {
    etl: PathBuf,
    pcapng: PathBuf,
}

fn external_pktmon_staging_dir() -> PathBuf {
    std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .map(|path| path.join("Temp"))
        .unwrap_or_else(|| PathBuf::from(EXTERNAL_PKTMON_STAGING_DIR_FALLBACK))
}

fn external_staging_paths(paths: &SupportPaths, staging_dir: &Path) -> ExternalStagingPaths {
    ExternalStagingPaths {
        etl: external_staging_path_for(
            staging_dir,
            &paths.external_etl,
            "nte-gacha-exporter.external.etl",
        ),
        pcapng: external_staging_path_for(
            staging_dir,
            &paths.external_pcapng,
            "nte-gacha-exporter.external.pcapng",
        ),
    }
}

fn external_staging_path_for(staging_dir: &Path, path: &Path, fallback_name: &str) -> PathBuf {
    staging_dir.join(
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(fallback_name),
    )
}

struct CommandExecution {
    log: ExternalCommandLog,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

struct CommandOutputReaders {
    stdout: JoinHandle<std::io::Result<Vec<u8>>>,
    stderr: JoinHandle<std::io::Result<Vec<u8>>>,
}

fn spawn_command_output_reader(
    mut reader: impl Read + Send + 'static,
) -> JoinHandle<std::io::Result<Vec<u8>>> {
    std::thread::spawn(move || {
        let mut output = Vec::new();
        reader.read_to_end(&mut output)?;
        Ok(output)
    })
}

fn run_command(
    program: &str,
    args: &[&str],
    cancel: Option<&AtomicBool>,
    timeout: Duration,
) -> CommandExecution {
    let args_vec = args
        .iter()
        .map(|arg| (*arg).to_string())
        .collect::<Vec<_>>();
    let command_line = if args.is_empty() {
        program.to_string()
    } else {
        format!("{program} {}", args.join(" "))
    };

    if cancel.is_some_and(|cancel| cancel.load(Ordering::SeqCst)) {
        return command_failure(
            program,
            args_vec,
            format!("command cancelled: {command_line}"),
        );
    }

    let mut child = match Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => return command_failure(program, args_vec, error.to_string()),
    };
    let readers = CommandOutputReaders {
        stdout: spawn_command_output_reader(
            child.stdout.take().expect("piped stdout must be available"),
        ),
        stderr: spawn_command_output_reader(
            child.stderr.take().expect("piped stderr must be available"),
        ),
    };
    let started = Instant::now();

    loop {
        let abort_error = if cancel.is_some_and(|cancel| cancel.load(Ordering::SeqCst)) {
            Some(format!("command cancelled: {command_line}"))
        } else if started.elapsed() >= timeout {
            Some(format!(
                "command timed out after {}ms: {command_line}",
                timeout.as_millis()
            ))
        } else {
            None
        };
        if let Some(error) = abort_error {
            return terminate_command(child, readers, program, args_vec, error);
        }

        match child.try_wait() {
            Ok(Some(_)) => {
                return collect_command_output(child, readers, program, args_vec, None);
            }
            Ok(None) => {
                let remaining = timeout.saturating_sub(started.elapsed());
                std::thread::sleep(PKTMON_COMMAND_POLL_INTERVAL.min(remaining));
            }
            Err(error) => {
                return terminate_command(
                    child,
                    readers,
                    program,
                    args_vec,
                    format!("failed to wait for command {command_line}: {error}"),
                );
            }
        }
    }
}

fn terminate_command(
    mut child: std::process::Child,
    readers: CommandOutputReaders,
    program: &str,
    args: Vec<String>,
    error: String,
) -> CommandExecution {
    if let Err(kill_error) = child.kill() {
        return match child.try_wait() {
            Ok(Some(_)) => collect_command_output(child, readers, program, args, Some(error)),
            Ok(None) => command_failure(
                program,
                args,
                format!("{error}; failed to terminate command: {kill_error}"),
            ),
            Err(wait_error) => command_failure(
                program,
                args,
                format!(
                    "{error}; failed to terminate command: {kill_error}; failed to poll command: {wait_error}"
                ),
            ),
        };
    }
    collect_command_output(child, readers, program, args, Some(error))
}

fn collect_command_output(
    mut child: std::process::Child,
    readers: CommandOutputReaders,
    program: &str,
    args: Vec<String>,
    error: Option<String>,
) -> CommandExecution {
    let status = match child.wait() {
        Ok(status) => status,
        Err(wait_error) => {
            return command_failure(
                program,
                args,
                match error {
                    Some(error) => {
                        format!("{error}; failed to collect command output: {wait_error}")
                    }
                    None => wait_error.to_string(),
                },
            );
        }
    };
    let stdout = join_command_output_reader(readers.stdout, "stdout");
    let stderr = join_command_output_reader(readers.stderr, "stderr");
    let (stdout, mut stderr) = match (stdout, stderr) {
        (Ok(stdout), Ok(stderr)) => (stdout, stderr),
        (stdout, stderr) => {
            let mut errors = Vec::with_capacity(2);
            if let Err(error) = stdout {
                errors.push(error);
            }
            if let Err(error) = stderr {
                errors.push(error);
            }
            let reader_error = errors.join("; ");
            let error = if let Some(error) = error {
                format!("{error}; {reader_error}")
            } else {
                reader_error
            };
            return command_failure(program, args, error);
        }
    };
    if let Some(error) = error.as_ref() {
        stderr.extend_from_slice(error.as_bytes());
        stderr.push(b'\n');
    }
    CommandExecution {
        log: ExternalCommandLog {
            program: program.to_string(),
            args,
            exit_code: status.code(),
            success: error.is_none() && status.success(),
            stdout_bytes: stdout.len(),
            stderr_bytes: stderr.len(),
            error,
        },
        stdout,
        stderr,
    }
}

fn join_command_output_reader(
    reader: JoinHandle<std::io::Result<Vec<u8>>>,
    stream: &'static str,
) -> Result<Vec<u8>, String> {
    match reader.join() {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(error)) => Err(format!("failed to read command {stream}: {error}")),
        Err(_) => Err(format!("command {stream} reader panicked")),
    }
}

fn command_failure(program: &str, args: Vec<String>, error: String) -> CommandExecution {
    let stderr = format!("{error}\n").into_bytes();
    CommandExecution {
        log: ExternalCommandLog {
            program: program.to_string(),
            args,
            exit_code: None,
            success: false,
            stdout_bytes: 0,
            stderr_bytes: stderr.len(),
            error: Some(error),
        },
        stdout: Vec::new(),
        stderr,
    }
}
