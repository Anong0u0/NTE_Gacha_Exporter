const EXTERNAL_PKTMON_STAGING_DIR_FALLBACK: &str = r"C:\Windows\Temp";

fn run_external_pktmon_capture(
    paths: &SupportPaths,
    ports: &[u16],
    pppoe_detection: PppoeDetection,
    filter_mode: CaptureFilterMode,
    duration: Duration,
    stop: Arc<AtomicBool>,
) -> ExternalCaptureReport {
    let mut runner = PktmonRunner::new(paths, pppoe_detection, filter_mode);
    let staging_dir = external_pktmon_staging_dir();
    let staging_paths = external_staging_paths(paths, &staging_dir);

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
    if filter_mode == CaptureFilterMode::PortFiltered {
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
    runner.run(&["stop"], "pktmon.exe stop failed");
    runner.run(
        &["pcapng", &external_etl, "-o", &external_pcapng],
        "pktmon.exe pcapng failed",
    );
    runner.run(&["filter", "remove"], "pktmon.exe filter remove failed");
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
    filter_mode: CaptureFilterMode,
}

impl<'a> PktmonRunner<'a> {
    fn new(
        paths: &'a SupportPaths,
        pppoe_detection: PppoeDetection,
        filter_mode: CaptureFilterMode,
    ) -> Self {
        Self {
            paths,
            commands: Vec::new(),
            stdout_log: Vec::new(),
            stderr_log: Vec::new(),
            ok: true,
            error: None,
            pppoe_detection,
            filter_mode,
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
        let execution = run_command("pktmon.exe", args);
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
        let execution = run_command("pktmon.exe", args);
        let success = execution.log.success;
        self.record_command(execution);
        success
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
            filter_mode: self.filter_mode.as_str().to_string(),
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

fn run_command(program: &str, args: &[&str]) -> CommandExecution {
    let args_vec = args
        .iter()
        .map(|arg| (*arg).to_string())
        .collect::<Vec<_>>();
    match Command::new(program).args(args).output() {
        Ok(output) => {
            let success = output.status.success();
            CommandExecution {
                log: ExternalCommandLog {
                    program: program.to_string(),
                    args: args_vec,
                    exit_code: output.status.code(),
                    success,
                    stdout_bytes: output.stdout.len(),
                    stderr_bytes: output.stderr.len(),
                    error: None,
                },
                stdout: output.stdout,
                stderr: output.stderr,
            }
        }
        Err(error) => {
            let stderr = format!("{error}\n").into_bytes();
            CommandExecution {
                log: ExternalCommandLog {
                    program: program.to_string(),
                    args: args_vec,
                    exit_code: None,
                    success: false,
                    stdout_bytes: 0,
                    stderr_bytes: stderr.len(),
                    error: Some(error.to_string()),
                },
                stdout: Vec::new(),
                stderr,
            }
        }
    }
}
