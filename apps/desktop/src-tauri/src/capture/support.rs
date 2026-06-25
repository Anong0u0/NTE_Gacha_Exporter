
const SUPPORT_SCHEMA: &str = "nte-gacha-capture-support";
const SUPPORT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Default)]
struct SupportWriteResult {
    json_path: Option<PathBuf>,
    image_path: Option<PathBuf>,
    error: Option<String>,
}

#[derive(Debug, Clone)]
struct SupportRequest<'a> {
    root: &'a Path,
    status: &'a CaptureStatus,
    source_kind: &'a str,
    auto_result: Option<&'a AutoPageRunResult>,
}

fn attach_capture_support(
    root: &Path,
    status: &mut CaptureStatus,
    source_kind: &str,
    auto_result: Option<&AutoPageRunResult>,
) {
    if status.error.is_none() {
        return;
    }
    let result = write_capture_support(SupportRequest {
        root,
        status,
        source_kind,
        auto_result,
    });
    if let Some(error) = status.error.as_mut() {
        attach_support_paths(error, result);
    }
}

fn attach_support_paths(error: &mut RuntimeError, result: SupportWriteResult) {
    if let Some(path) = result.json_path {
        error.support_path = Some(path.to_string_lossy().to_string());
    }
    if let Some(path) = result.image_path {
        error.support_image_path = Some(path.to_string_lossy().to_string());
    }
    if let Some(support_error) = result.error {
        error.message = format!("{}; support_failed: {support_error}", error.message);
    }
}

fn write_capture_support(request: SupportRequest<'_>) -> SupportWriteResult {
    let support_dir = request.root.join("data/support");
    if let Err(error) = fs::create_dir_all(&support_dir) {
        return SupportWriteResult {
            error: Some(error.to_string()),
            ..SupportWriteResult::default()
        };
    }

    let base = sanitize_file_stem(&format!("capture-{}", request.status.session_id));
    let json_path = support_dir.join(format!("{base}.json"));
    let mut write_error = None;
    let image_path = write_support_image(&support_dir, &base, request.auto_result, &mut write_error);
    let report = support_report(&request, image_path.as_ref(), write_error.as_deref());
    if let Err(error) = write_support_json(&json_path, &report) {
        return SupportWriteResult {
            image_path,
            error: Some(error.to_string()),
            ..SupportWriteResult::default()
        };
    }
    SupportWriteResult {
        json_path: Some(json_path),
        image_path,
        error: write_error,
    }
}

fn write_support_image(
    support_dir: &Path,
    base: &str,
    auto_result: Option<&AutoPageRunResult>,
    write_error: &mut Option<String>,
) -> Option<PathBuf> {
    let bytes = auto_result
        .and_then(|result| result.diagnostics.page_context_png.as_ref())
        .filter(|bytes| !bytes.is_empty())?;
    let path = support_dir.join(format!("{base}-page-number.png"));
    if let Err(error) = fs::write(&path, bytes) {
        *write_error = Some(format!("write support image failed: {error}"));
        return None;
    }
    Some(path)
}

fn write_support_json(path: &Path, report: &SupportReport) -> Result<(), std::io::Error> {
    let text = serde_json::to_string_pretty(report).map_err(std::io::Error::other)?;
    fs::write(path, text)
}

fn support_report(
    request: &SupportRequest<'_>,
    image_path: Option<&PathBuf>,
    support_error: Option<&str>,
) -> SupportReport {
    SupportReport {
        schema: SUPPORT_SCHEMA,
        schema_version: SUPPORT_SCHEMA_VERSION,
        created_at: now_seconds(),
        app_version: env!("CARGO_PKG_VERSION"),
        session_id: request.status.session_id.clone(),
        failure: json!({
            "code": request.status.error.as_ref().map(|error| error.code.as_str()),
            "message": request.status.error.as_ref().map(|error| error.message.as_str()),
            "state": request.status.state,
            "source_kind": request.source_kind,
            "support_error": support_error,
        }),
        capture: json!({
            "mode": request.status.mode,
            "target": request.status.target,
            "counters": request.status.counters,
            "raw_path_exists": request.status.raw_path.as_ref().map(|path| {
                let path = Path::new(path);
                if path.is_absolute() {
                    path.is_file()
                } else {
                    request.root.join(path).is_file()
                }
            }),
        }),
        auto_page: request.auto_result.map(auto_page_summary),
        diagnostics: request.auto_result.map(|result| auto_page_diagnostics(&result.diagnostics)),
        support_image_path: image_path.map(|path| path.to_string_lossy().to_string()),
    }
}

fn auto_page_summary(result: &AutoPageRunResult) -> Value {
    json!({
        "status": result.status,
        "message": result.message,
        "completed_pools": result.completed_pools,
        "skipped_pools": result.skipped_pools,
        "visited_pages_by_pool": result.visited_pages_by_pool,
        "last_page_by_pool": result.last_page_by_pool,
    })
}

fn auto_page_diagnostics(diagnostics: &AutoPageDiagnostics) -> Value {
    json!({
        "failure_kind": diagnostics.failure_kind,
        "window": diagnostics.window,
        "visual": diagnostics.visual,
        "ocr": diagnostics.ocr,
    })
}

fn sanitize_file_stem(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

#[derive(Debug, Serialize)]
struct SupportReport {
    schema: &'static str,
    schema_version: u32,
    created_at: f64,
    app_version: &'static str,
    session_id: String,
    failure: Value,
    capture: Value,
    auto_page: Option<Value>,
    diagnostics: Option<Value>,
    support_image_path: Option<String>,
}
