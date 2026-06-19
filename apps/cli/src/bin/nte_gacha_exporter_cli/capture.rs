fn replay(args: ReplayArgs) -> CliResult<()> {
    let defaults = DefaultPaths::new();
    let json = args.output.json.clone().unwrap_or(defaults.json);
    let csv = args.output.csv.clone().unwrap_or(defaults.csv);
    export_raw_replay(&args.raw_jsonl, &args.output.locale, &json, Some(&csv))?;
    println!("records={}", count_public_records(&json)?);
    print_paths(&json, Some(&csv), None);
    Ok(())
}

fn capture(args: CaptureArgs) -> CliResult<()> {
    if relaunch_capture_as_admin()? {
        return Ok(());
    }
    let locale = args.output.locale.clone();
    let verbose = args.verbose;
    let pid = match args.pid {
        Some(pid) => pid,
        None => find_process_pid(EXE_NAME)
            .map_err(CliError::from_error)?
            .ok_or_else(|| CliError::new(3, format!("{EXE_NAME} not found")))?,
    };
    let ports = candidate_ports(pid).map_err(CliError::from_error)?;
    if ports.is_empty() {
        return Err(CliError::new(
            3,
            format!("no candidate ports found for pid={pid}"),
        ));
    }

    let defaults = DefaultPaths::new();
    let json = args.output.json.clone().unwrap_or(defaults.json);
    let csv = args.output.csv.clone().unwrap_or(defaults.csv);
    let output_raw = args.output_raw.map(|path| {
        if path.as_os_str().is_empty() {
            defaults.raw
        } else {
            path
        }
    });
    let stop = Arc::new(AtomicBool::new(false));
    install_ctrlc(Arc::clone(&stop))?;
    let q_listener = start_q_listener(Arc::clone(&stop))?;
    let has_q_listener = q_listener.is_some();
    let _q_listener = q_listener;

    if args.auto_page {
        run_auto_capture(AutoCaptureContext {
            pid,
            ports,
            output_raw,
            json,
            csv,
            locale,
            verbose,
            stop,
            has_q_listener,
        })
    } else {
        if has_q_listener {
            println!("Press q or Ctrl+C to stop.");
        } else {
            println!("Press Ctrl+C to stop.");
        }
        let result = capture_live(
            CaptureOptions {
                pid,
                exe: EXE_NAME.to_string(),
                ports,
                raw_out: output_raw.clone(),
                max_packets: 0,
                max_decoded: 0,
                on_progress: Some(progress_callback(&locale, verbose)),
            },
            stop,
        )
        .map_err(capture_error)?;
        export_capture_rows(&result.rows, &locale, &json, Some(&csv))?;
        println!("records={}", result.rows.len());
        print_paths(&json, Some(&csv), output_raw.as_deref());
        Ok(())
    }
}

struct AutoCaptureContext {
    pid: u32,
    ports: Vec<u16>,
    output_raw: Option<PathBuf>,
    json: PathBuf,
    csv: PathBuf,
    locale: String,
    verbose: bool,
    stop: Arc<AtomicBool>,
    has_q_listener: bool,
}

fn run_auto_capture(context: AutoCaptureContext) -> CliResult<()> {
    let AutoCaptureContext {
        pid,
        ports,
        output_raw,
        json,
        csv,
        locale,
        verbose,
        stop,
        has_q_listener,
    } = context;

    if has_q_listener {
        println!("Press Esc to stop auto. Press q or Ctrl+C to stop capture.");
    } else {
        println!("Press Esc to stop auto. Press Ctrl+C to stop capture.");
    }
    let capture_stop = Arc::clone(&stop);
    let capture_raw = output_raw.clone();
    let progress = progress_callback(&locale, verbose);
    let handle = thread::spawn(move || {
        capture_live(
            CaptureOptions {
                pid,
                exe: EXE_NAME.to_string(),
                ports,
                raw_out: capture_raw,
                max_packets: 0,
                max_decoded: 0,
                on_progress: Some(progress),
            },
            capture_stop,
        )
    });

    let mut options = AutoPageOptions::new(pid, Arc::clone(&stop));
    options.full_update = true;
    options.non_interactive = true;
    options.tooltip = false;
    options.on_status = Some(Arc::new(print_auto_status));
    let auto_result = run_auto_page(options);
    println!(
        "auto_page={} message={}",
        auto_result.status, auto_result.message
    );
    if auto_result.succeeded() {
        stop.store(true, Ordering::SeqCst);
    }

    let capture_result = handle
        .join()
        .map_err(|_| CliError::new(2, "capture worker panicked"))?
        .map_err(capture_error)?;
    export_capture_rows(&capture_result.rows, &locale, &json, Some(&csv))?;
    println!("records={}", capture_result.rows.len());
    print_paths(&json, Some(&csv), output_raw.as_deref());
    if auto_result.succeeded() {
        Ok(())
    } else {
        Err(CliError::new(2, auto_result.message))
    }
}

