fn export_raw_replay(
    raw_jsonl: &Path,
    locale: &str,
    json: &Path,
    csv: Option<&Path>,
) -> CliResult<()> {
    let rows = read_raw_capture(raw_jsonl).map_err(CliError::from_error)?;
    let document = build_capture_document(&rows.rows, locale).map_err(CliError::from_error)?;
    export_document(document, locale, json, csv, Some(raw_jsonl))
}

fn export_capture_rows(
    rows: &[ParsedRow],
    locale: &str,
    json: &Path,
    csv: Option<&Path>,
) -> CliResult<()> {
    let document = build_capture_document(rows, locale).map_err(CliError::from_error)?;
    export_document(document, locale, json, csv, None)
}

fn export_document(
    document: serde_json::Value,
    locale: &str,
    json: &Path,
    csv: Option<&Path>,
    source_path: Option<&Path>,
) -> CliResult<()> {
    let temp = tempfile::tempdir().map_err(CliError::from_error)?;
    let store = JsonStore::open(temp.path()).map_err(CliError::from_error)?;
    let text = serde_json::to_string(&document).map_err(CliError::from_error)?;
    store
        .import_public_document(
            "default",
            &text,
            "raw_jsonl",
            source_path.map(|path| path.to_string_lossy()).as_deref(),
        )
        .map_err(CliError::from_error)?;
    store
        .export_public_json("default", locale, json)
        .map_err(CliError::from_error)?;
    if let Some(csv) = csv {
        store
            .export_csv("default", locale, csv)
            .map_err(CliError::from_error)?;
    }
    Ok(())
}

fn count_public_records(path: &Path) -> CliResult<usize> {
    let text = std::fs::read_to_string(path).map_err(CliError::from_error)?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(CliError::from_error)?;
    Ok(value
        .get("nte")
        .and_then(|nte| nte.get("list"))
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len))
}

fn print_paths(json: &Path, csv: Option<&Path>, raw: Option<&Path>) {
    println!("json={}", json.display());
    if let Some(csv) = csv {
        println!("csv={}", csv.display());
    }
    if let Some(raw) = raw {
        println!("private_raw={}", raw.display());
    }
}

fn install_ctrlc(stop: Arc<AtomicBool>) -> CliResult<()> {
    ctrlc::set_handler(move || {
        stop.store(true, Ordering::SeqCst);
    })
    .map_err(CliError::from_error)
}

struct QListener {
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Drop for QListener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        let _ = disable_raw_mode();
    }
}

fn start_q_listener(stop: Arc<AtomicBool>) -> CliResult<Option<QListener>> {
    if !std::io::stdin().is_terminal() {
        return Ok(None);
    }
    enable_raw_mode().map_err(CliError::from_error)?;
    let listener_stop = Arc::clone(&stop);
    let handle = thread::spawn(move || {
        while !listener_stop.load(Ordering::SeqCst) {
            match event::poll(Duration::from_millis(100)) {
                Ok(true) => match event::read() {
                    Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => match key.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') => {
                            listener_stop.store(true, Ordering::SeqCst);
                            return;
                        }
                        KeyCode::Char('c') | KeyCode::Char('C')
                            if key.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            listener_stop.store(true, Ordering::SeqCst);
                            return;
                        }
                        _ => {}
                    },
                    Ok(_) => {}
                    Err(_) => {
                        listener_stop.store(true, Ordering::SeqCst);
                        return;
                    }
                },
                Ok(false) => {}
                Err(_) => {
                    listener_stop.store(true, Ordering::SeqCst);
                    return;
                }
            }
        }
    });
    Ok(Some(QListener {
        stop,
        handle: Some(handle),
    }))
}

