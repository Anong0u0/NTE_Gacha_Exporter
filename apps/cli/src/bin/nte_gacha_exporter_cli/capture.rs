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
    if args.install_windivert && !args.windivert {
        return Err(CliError::new(
            2,
            "--install-windivert must be used with --windivert",
        ));
    }
    if relaunch_capture_as_admin()? {
        return Ok(());
    }
    let locale = args.output.locale.clone();
    let verbose = args.verbose;
    let backend = if args.windivert {
        CaptureBackend::WinDivert
    } else {
        CaptureBackend::Pktmon
    };
    if args.install_windivert {
        install_windivert_runtime()?;
    } else if backend == CaptureBackend::WinDivert && !windivert_installed() {
        return Err(CliError::new(
            3,
            "WinDivert is not installed; rerun with --windivert --install-windivert",
        ));
    }
    let pid = match args.pid {
        Some(pid) => pid,
        None => find_process_pid(EXE_NAME)
            .map_err(CliError::from_error)?
            .ok_or_else(|| CliError::new(3, format!("{EXE_NAME} not found")))?,
    };
    let ports = candidate_ports(pid).map_err(CliError::from_error)?;
    let pppoe_detection = detect_pppoe();
    let strategy = if backend == CaptureBackend::WinDivert {
        CaptureStrategy::no_filter(nte_capture::CaptureStrategyReason::WinDivertBackend)
    } else {
        CaptureStrategy::for_pppoe_detection(&pppoe_detection)
    };
    if backend == CaptureBackend::Pktmon
        && ports.is_empty()
        && strategy.kind == CaptureStrategyKind::PortFiltered
    {
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
            pppoe_detection,
            backend,
            strategy,
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
                pppoe_detection: Some(pppoe_detection),
                backend,
                strategy: Some(strategy),
                raw_out: output_raw.clone(),
                raw_append: false,
                windivert_dir: windivert_dir_for_backend(backend),
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

fn windivert_dir_for_backend(backend: CaptureBackend) -> Option<PathBuf> {
    if backend == CaptureBackend::WinDivert {
        return Some(nte_capture::windivert::windivert_install_dir(
            &portable_root(),
        ));
    }
    None
}

fn windivert_installed() -> bool {
    nte_capture::windivert::windivert_status(&portable_root(), false).installed
}

fn install_windivert_runtime() -> CliResult<()> {
    let report =
        nte_capture::windivert::install_windivert(&portable_root()).map_err(|message| {
            CliError::new(
                3,
                format!("WinDivert install failed: {message}"),
            )
        })?;
    println!("windivert_installed={}", report.status.install_dir);
    println!("windivert_sha256={}", report.verified_sha256);
    Ok(())
}

fn portable_root() -> PathBuf {
    std::env::var_os("NTE_GACHA_EXPORTER_PORTABLE_ROOT")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|path| path.parent().map(PathBuf::from))
        })
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

struct AutoCaptureContext {
    pid: u32,
    ports: Vec<u16>,
    pppoe_detection: nte_capture::PppoeDetection,
    backend: CaptureBackend,
    strategy: CaptureStrategy,
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
        pppoe_detection,
        backend,
        strategy,
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
                pppoe_detection: Some(pppoe_detection),
                backend,
                strategy: Some(strategy),
                raw_out: capture_raw,
                raw_append: false,
                windivert_dir: windivert_dir_for_backend(backend),
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
