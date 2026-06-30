fn run_external_pktmon_capture(
    paths: &SupportPaths,
    ports: &[u16],
    duration: Duration,
    stop: Arc<AtomicBool>,
) -> ExternalCaptureReport {
    let mut commands = Vec::new();
    let mut stdout_log = Vec::new();
    let mut stderr_log = Vec::new();
    let mut ok = true;
    let mut error = None;

    run_pktmon(
        &["filter", "remove"],
        &mut commands,
        &mut stdout_log,
        &mut stderr_log,
        &mut ok,
        &mut error,
    );
    for port in ports {
        run_pktmon(
            &[
                "filter",
                "add",
                &format!("NTE_DIAG_UDP_{port}"),
                "-t",
                "UDP",
                "-p",
                &port.to_string(),
            ],
            &mut commands,
            &mut stdout_log,
            &mut stderr_log,
            &mut ok,
            &mut error,
        );
        run_pktmon(
            &[
                "filter",
                "add",
                &format!("NTE_DIAG_TCP_{port}"),
                "-t",
                "TCP",
                "-p",
                &port.to_string(),
            ],
            &mut commands,
            &mut stdout_log,
            &mut stderr_log,
            &mut ok,
            &mut error,
        );
    }
    let external_etl = paths.external_etl.to_string_lossy().to_string();
    let external_pcapng = paths.external_pcapng.to_string_lossy().to_string();
    run_pktmon(
        &[
            "start",
            "--capture",
            "--pkt-size",
            "0",
            "--file-name",
            &external_etl,
        ],
        &mut commands,
        &mut stdout_log,
        &mut stderr_log,
        &mut ok,
        &mut error,
    );

    let started = Instant::now();
    while started.elapsed() < duration && !stop.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(250));
    }

    let counters_json = run_pktmon_capture_output(
        &["counters", "--json"],
        &paths.counters_json,
        &mut commands,
        &mut stdout_log,
        &mut stderr_log,
    );
    ok &= counters_json;
    if !counters_json && error.is_none() {
        error = Some("pktmon.exe counters --json failed".to_string());
    }
    let counters_txt = run_pktmon_capture_output(
        &["counters"],
        &paths.counters_txt,
        &mut commands,
        &mut stdout_log,
        &mut stderr_log,
    );
    ok &= counters_txt;
    if !counters_txt && error.is_none() {
        error = Some("pktmon.exe counters failed".to_string());
    }
    run_pktmon(
        &["stop"],
        &mut commands,
        &mut stdout_log,
        &mut stderr_log,
        &mut ok,
        &mut error,
    );
    run_pktmon(
        &["pcapng", &external_etl, "-o", &external_pcapng],
        &mut commands,
        &mut stdout_log,
        &mut stderr_log,
        &mut ok,
        &mut error,
    );
    run_pktmon(
        &["filter", "remove"],
        &mut commands,
        &mut stdout_log,
        &mut stderr_log,
        &mut ok,
        &mut error,
    );

    if let Err(write_error) = fs::write(&paths.external_stdout, stdout_log) {
        ok = false;
        error = Some(format!("write external stdout log failed: {write_error}"));
    }
    if let Err(write_error) = fs::write(&paths.external_stderr, stderr_log) {
        ok = false;
        error = Some(format!("write external stderr log failed: {write_error}"));
    }
    if let Err(write_error) = fs::write(
        &paths.external_commands,
        serde_json::to_vec_pretty(&commands).unwrap_or_default(),
    ) {
        ok = false;
        error = Some(format!("write external command log failed: {write_error}"));
    }

    ExternalCaptureReport {
        attempted: true,
        ok,
        error,
        etl_path: Some(paths.external_etl.to_string_lossy().to_string()),
        pcapng_path: Some(paths.external_pcapng.to_string_lossy().to_string()),
        stdout_log_path: Some(paths.external_stdout.to_string_lossy().to_string()),
        stderr_log_path: Some(paths.external_stderr.to_string_lossy().to_string()),
        counters_json_path: Some(paths.counters_json.to_string_lossy().to_string()),
        counters_txt_path: Some(paths.counters_txt.to_string_lossy().to_string()),
        command_log_path: Some(paths.external_commands.to_string_lossy().to_string()),
        commands,
    }
}

#[allow(clippy::too_many_arguments)]
fn run_pktmon(
    args: &[&str],
    commands: &mut Vec<ExternalCommandLog>,
    stdout_log: &mut Vec<u8>,
    stderr_log: &mut Vec<u8>,
    ok: &mut bool,
    error: &mut Option<String>,
) {
    let success = run_command("pktmon.exe", args, None, commands, stdout_log, stderr_log);
    if !success {
        *ok = false;
        if error.is_none() {
            *error = Some(format!("pktmon.exe {} failed", args.join(" ")));
        }
    }
}

fn run_pktmon_capture_output(
    args: &[&str],
    output_path: &Path,
    commands: &mut Vec<ExternalCommandLog>,
    stdout_log: &mut Vec<u8>,
    stderr_log: &mut Vec<u8>,
) -> bool {
    run_command(
        "pktmon.exe",
        args,
        Some(output_path),
        commands,
        stdout_log,
        stderr_log,
    )
}

fn run_command(
    program: &str,
    args: &[&str],
    stdout_file: Option<&Path>,
    commands: &mut Vec<ExternalCommandLog>,
    stdout_log: &mut Vec<u8>,
    stderr_log: &mut Vec<u8>,
) -> bool {
    let args_vec = args
        .iter()
        .map(|arg| (*arg).to_string())
        .collect::<Vec<_>>();
    match Command::new(program).args(args).output() {
        Ok(output) => {
            let _ = writeln!(stdout_log, "\n> {program} {}", args.join(" "));
            stdout_log.extend_from_slice(&output.stdout);
            let _ = writeln!(stderr_log, "\n> {program} {}", args.join(" "));
            stderr_log.extend_from_slice(&output.stderr);
            if let Some(path) = stdout_file {
                let _ = fs::write(path, &output.stdout);
            }
            let success = output.status.success();
            commands.push(ExternalCommandLog {
                program: program.to_string(),
                args: args_vec,
                exit_code: output.status.code(),
                success,
                stdout_bytes: output.stdout.len(),
                stderr_bytes: output.stderr.len(),
                error: None,
            });
            success
        }
        Err(error) => {
            let _ = writeln!(stderr_log, "\n> {program} {}", args.join(" "));
            let _ = writeln!(stderr_log, "{error}");
            commands.push(ExternalCommandLog {
                program: program.to_string(),
                args: args_vec,
                exit_code: None,
                success: false,
                stdout_bytes: 0,
                stderr_bytes: error.to_string().len(),
                error: Some(error.to_string()),
            });
            false
        }
    }
}
