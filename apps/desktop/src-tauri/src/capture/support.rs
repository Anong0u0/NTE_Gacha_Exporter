
const SUPPORT_SCHEMA: &str = "nte-gacha-capture-support";
const SUPPORT_SCHEMA_VERSION: u32 = 2;
const CAPTURE_SUPPORT_RETENTION_LIMIT: usize = 3;
const SUPPORT_DATA_DIR: &str = "data";
const SUPPORT_DIR: &str = "support";
const SUPPORT_PREFIX: &str = "capture-";
const SUPPORT_JSON_SUFFIX: &str = ".json";
const SUPPORT_IMAGE_SUFFIX: &str = "-context.png";
const SUPPORT_RAW_PAGE_IMAGE_SUFFIX: &str = "-page-number-raw.png";
const LEGACY_SUPPORT_IMAGE_SUFFIX: &str = "-page-number.png";

#[derive(Debug, Clone, Default)]
struct SupportWriteResult {
    json_path: Option<PathBuf>,
    image_path: Option<PathBuf>,
    raw_page_image_path: Option<PathBuf>,
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
    } else if let Some(path) = result.raw_page_image_path {
        error.support_image_path = Some(path.to_string_lossy().to_string());
    }
    if let Some(support_error) = result.error {
        error.message = format!("{}; support_failed: {support_error}", error.message);
    }
}

fn write_capture_support(request: SupportRequest<'_>) -> SupportWriteResult {
    let support_dir = match prepare_support_dir(request.root) {
        Ok(path) => path,
        Err(error) => {
            return SupportWriteResult {
                error: Some(error.to_string()),
                ..SupportWriteResult::default()
            };
        }
    };
    let base = sanitize_file_stem(&format!("capture-{}", request.status.session_id));
    let json_path = support_dir.join(format!("{base}.json"));
    let mut write_error = None;
    let images = write_support_images(&support_dir, &base, request.auto_result, &mut write_error);
    let report = support_report(
        &request,
        images.context_path.as_ref(),
        images.raw_page_path.as_ref(),
        write_error.as_deref(),
    );
    if let Err(error) = write_support_json(&json_path, &report) {
        return SupportWriteResult {
            image_path: images.context_path,
            raw_page_image_path: images.raw_page_path,
            error: Some(error.to_string()),
            ..SupportWriteResult::default()
        };
    }
    if let Err(error) = rotate_capture_support_files(
        request.root,
        CAPTURE_SUPPORT_RETENTION_LIMIT,
        Some(base.as_str()),
    ) {
        append_support_error(&mut write_error, format!("rotate support files failed: {error}"));
    }
    SupportWriteResult {
        json_path: Some(json_path),
        image_path: images.context_path,
        raw_page_image_path: images.raw_page_path,
        error: write_error,
    }
}

#[derive(Debug, Clone, Default)]
struct SupportImages {
    context_path: Option<PathBuf>,
    raw_page_path: Option<PathBuf>,
}

fn write_support_images(
    support_dir: &Path,
    base: &str,
    auto_result: Option<&AutoPageRunResult>,
    write_error: &mut Option<String>,
) -> SupportImages {
    let context_path = auto_result
        .and_then(|result| result.diagnostics.context_png.as_ref())
        .filter(|bytes| !bytes.is_empty())
        .and_then(|bytes| {
            let path = support_dir.join(format!("{base}{SUPPORT_IMAGE_SUFFIX}"));
            match fs::write(&path, bytes) {
                Ok(()) => Some(path),
                Err(error) => {
                    append_support_error(write_error, format!("write support image failed: {error}"));
                    None
                }
            }
        });
    let raw_page_path = auto_result
        .and_then(|result| result.diagnostics.raw_page_png.as_ref())
        .filter(|bytes| !bytes.is_empty())
        .and_then(|bytes| {
            let path = support_dir.join(format!("{base}{SUPPORT_RAW_PAGE_IMAGE_SUFFIX}"));
            match fs::write(&path, bytes) {
                Ok(()) => Some(path),
                Err(error) => {
                    append_support_error(
                        write_error,
                        format!("write raw page image failed: {error}"),
                    );
                    None
                }
            }
        });
    SupportImages {
        context_path,
        raw_page_path,
    }
}

fn write_support_json(path: &Path, report: &SupportReport) -> Result<(), std::io::Error> {
    let text = serde_json::to_string_pretty(report).map_err(std::io::Error::other)?;
    fs::write(path, text)
}

fn support_dir_path(root: &Path) -> PathBuf {
    root.join(SUPPORT_DATA_DIR).join(SUPPORT_DIR)
}

fn prepare_support_dir(root: &Path) -> Result<PathBuf, std::io::Error> {
    reject_existing_symlink_or_non_dir(root)?;
    let data_dir = root.join(SUPPORT_DATA_DIR);
    reject_existing_symlink_or_non_dir(&data_dir)?;
    let support_dir = support_dir_path(root);
    fs::create_dir_all(&support_dir)?;
    reject_existing_symlink_or_non_dir(&data_dir)?;
    reject_existing_symlink_or_non_dir(&support_dir)?;
    Ok(support_dir)
}

fn reject_existing_symlink_or_non_dir(path: &Path) -> Result<(), std::io::Error> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if metadata.file_type().is_symlink() {
        return Err(std::io::Error::other(format!(
            "support path is symlink: {}",
            path.display()
        )));
    }
    if !metadata.is_dir() {
        return Err(std::io::Error::other(format!(
            "support path is not directory: {}",
            path.display()
        )));
    }
    Ok(())
}

fn rotate_capture_support_files(
    root: &Path,
    keep: usize,
    protected_base: Option<&str>,
) -> Result<(), std::io::Error> {
    reject_existing_symlink_or_non_dir(root)?;
    let data_dir = root.join(SUPPORT_DATA_DIR);
    reject_existing_symlink_or_non_dir(&data_dir)?;
    let support_dir = support_dir_path(root);
    match fs::symlink_metadata(&support_dir) {
        Ok(_) => reject_existing_symlink_or_non_dir(&support_dir)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    }

    let mut anchors = support_json_anchors(&support_dir)?;
    anchors.sort_by(|left, right| {
        let left_protected = Some(left.base.as_str()) == protected_base;
        let right_protected = Some(right.base.as_str()) == protected_base;
        right_protected
            .cmp(&left_protected)
            .then_with(|| right.modified.cmp(&left.modified))
            .then_with(|| right.base.cmp(&left.base))
    });

    let keep = keep.max(1);
    let keep_bases = anchors
        .iter()
        .take(keep)
        .map(|anchor| anchor.base.clone())
        .collect::<BTreeSet<_>>();
    for anchor in anchors.iter().skip(keep) {
        remove_regular_file_if_exists(&anchor.path)?;
        remove_regular_file_if_exists(
            &support_dir.join(format!("{}{}", anchor.base, SUPPORT_IMAGE_SUFFIX)),
        )?;
        remove_regular_file_if_exists(
            &support_dir.join(format!("{}{}", anchor.base, SUPPORT_RAW_PAGE_IMAGE_SUFFIX)),
        )?;
        remove_regular_file_if_exists(
            &support_dir.join(format!("{}{}", anchor.base, LEGACY_SUPPORT_IMAGE_SUFFIX)),
        )?;
    }
    remove_orphan_support_images(&support_dir, &keep_bases)
}

#[derive(Debug)]
struct SupportJsonAnchor {
    base: String,
    path: PathBuf,
    modified: std::time::SystemTime,
}

fn support_json_anchors(support_dir: &Path) -> Result<Vec<SupportJsonAnchor>, std::io::Error> {
    let mut anchors = Vec::new();
    for entry in fs::read_dir(support_dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.file_type().is_file() {
            continue;
        }
        let Some(base) = path.file_name().and_then(support_json_base) else {
            continue;
        };
        anchors.push(SupportJsonAnchor {
            base,
            path,
            modified: metadata.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH),
        });
    }
    Ok(anchors)
}

fn remove_orphan_support_images(
    support_dir: &Path,
    keep_bases: &BTreeSet<String>,
) -> Result<(), std::io::Error> {
    for entry in fs::read_dir(support_dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.file_type().is_file() {
            continue;
        }
        let Some(base) = path.file_name().and_then(support_image_base) else {
            continue;
        };
        if !keep_bases.contains(&base) {
            remove_regular_file_if_exists(&path)?;
        }
    }
    Ok(())
}

fn remove_regular_file_if_exists(path: &Path) -> Result<(), std::io::Error> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if metadata.file_type().is_file() {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn support_json_base(file_name: &std::ffi::OsStr) -> Option<String> {
    let name = file_name.to_str()?;
    let base = name.strip_suffix(SUPPORT_JSON_SUFFIX)?;
    support_base_valid(base).then(|| base.to_string())
}

fn support_image_base(file_name: &std::ffi::OsStr) -> Option<String> {
    let name = file_name.to_str()?;
    let base = name
        .strip_suffix(SUPPORT_IMAGE_SUFFIX)
        .or_else(|| name.strip_suffix(SUPPORT_RAW_PAGE_IMAGE_SUFFIX))
        .or_else(|| name.strip_suffix(LEGACY_SUPPORT_IMAGE_SUFFIX))?;
    support_base_valid(base).then(|| base.to_string())
}

fn support_base_valid(base: &str) -> bool {
    base.len() > SUPPORT_PREFIX.len()
        && base.starts_with(SUPPORT_PREFIX)
        && base
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn append_support_error(target: &mut Option<String>, message: impl Into<String>) {
    let message = message.into();
    match target {
        Some(existing) if !existing.is_empty() => {
            existing.push_str("; ");
            existing.push_str(&message);
        }
        Some(existing) => *existing = message,
        None => *target = Some(message),
    }
}

fn support_report(
    request: &SupportRequest<'_>,
    image_path: Option<&PathBuf>,
    raw_page_image_path: Option<&PathBuf>,
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
        support_raw_page_image_path: raw_page_image_path
            .map(|path| path.to_string_lossy().to_string()),
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
        "input": diagnostics.input,
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
    support_raw_page_image_path: Option<String>,
}
