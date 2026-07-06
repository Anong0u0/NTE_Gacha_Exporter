#[test]
fn support_json_excludes_record_payloads_and_raw_contents() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("data/runs")).unwrap();
    std::fs::write(tmp.path().join("data/runs/raw-private.jsonl"), b"private raw").unwrap();
    let status = failed_status("session/private");

    let result = write_capture_support(SupportRequest {
        root: tmp.path(),
        status: &status,
        source_kind: "pktmon-auto-page-capture",
        auto_result: None,
    });

    let path = result.json_path.unwrap();
    let text = std::fs::read_to_string(path).unwrap();
    assert!(text.contains("nte-gacha-capture-support"));
    assert!(text.contains("\"schema_version\": 2"));
    assert!(text.contains("raw_path_exists"));
    assert!(text.contains("\"raw_path_exists\": true"));
    assert!(!text.contains("private-record"));
    assert!(!text.contains("raw-private.jsonl"));
    assert!(!text.contains("private raw"));
    assert!(result.image_path.is_none());
}

#[test]
fn support_writer_saves_auto_page_context_image_when_available() {
    let tmp = tempfile::tempdir().unwrap();
    let status = failed_status("session-image");
    let mut auto_result =
        AutoPageRunResult::failed("cannot read page number", Vec::new(), Vec::new());
    auto_result.diagnostics.context_png = Some(vec![137, 80, 78, 71]);
    auto_result.diagnostics.raw_page_png = Some(vec![137, 80, 78, 71, 2]);
    auto_result.diagnostics.input.mouse_buttons_swapped = Some(true);
    auto_result.diagnostics.input.last_click = Some(nte_automation::MouseClickDiagnostics {
        point: nte_automation::Point { x: 450, y: 1000 },
        physical_button: nte_automation::MouseButton::Right,
        mouse_buttons_swapped: true,
    });

    let result = write_capture_support(SupportRequest {
        root: tmp.path(),
        status: &status,
        source_kind: "pktmon-auto-page-capture",
        auto_result: Some(&auto_result),
    });

    let image_path = result.image_path.unwrap();
    assert_eq!(std::fs::read(&image_path).unwrap(), vec![137, 80, 78, 71]);
    assert!(image_path.ends_with("capture-session-image-context.png"));
    let text = std::fs::read_to_string(result.json_path.unwrap()).unwrap();
    assert!(text.contains("support_image_path"));
    assert!(text.contains("support_raw_page_image_path"));
    assert!(text.contains("\"input\""));
    assert!(text.contains("\"physical_button\": \"right\""));
    let raw_path = image_path.with_file_name("capture-session-image-page-number-raw.png");
    assert_eq!(std::fs::read(&raw_path).unwrap(), vec![137, 80, 78, 71, 2]);
}

#[test]
fn attach_capture_support_adds_paths_to_runtime_error() {
    let tmp = tempfile::tempdir().unwrap();
    let mut status = failed_status("session-paths");

    attach_capture_support(tmp.path(), &mut status, "pktmon-auto-page-capture", None);

    let error = status.error.unwrap();
    assert_eq!(error.code, "auto_page_failed");
    let support_path = Path::new(error.support_path.as_deref().unwrap());
    assert_eq!(support_path.parent().unwrap().file_name().unwrap(), "support");
    assert_eq!(
        support_path
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .file_name()
            .unwrap(),
        "data"
    );
    assert!(error.support_image_path.is_none());
}

#[test]
fn support_rotate_keeps_latest_three_bundles() {
    let tmp = tempfile::tempdir().unwrap();
    let support_dir = tmp.path().join("data/support");
    std::fs::create_dir_all(&support_dir).unwrap();
    for index in 0..5 {
        write_support_bundle(&support_dir, &format!("capture-s{index}"));
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    rotate_capture_support_files(tmp.path(), 3, Some("capture-s4")).unwrap();

    assert_support_bundle_exists(&support_dir, "capture-s4");
    assert_support_bundle_exists(&support_dir, "capture-s3");
    assert_support_bundle_exists(&support_dir, "capture-s2");
    assert_support_bundle_missing(&support_dir, "capture-s1");
    assert_support_bundle_missing(&support_dir, "capture-s0");
}

#[test]
fn support_rotate_removes_orphan_images_but_skips_unrelated_files() {
    let tmp = tempfile::tempdir().unwrap();
    let support_dir = tmp.path().join("data/support");
    std::fs::create_dir_all(&support_dir).unwrap();
    write_support_bundle(&support_dir, "capture-current");
    std::fs::write(support_dir.join("capture-orphan-context.png"), b"orphan").unwrap();
    std::fs::write(support_dir.join("capture-legacy-page-number.png"), b"legacy").unwrap();
    std::fs::write(support_dir.join("notes.txt"), b"keep").unwrap();
    std::fs::create_dir(support_dir.join("capture-dir.json")).unwrap();

    rotate_capture_support_files(tmp.path(), 3, Some("capture-current")).unwrap();

    assert_support_bundle_exists(&support_dir, "capture-current");
    assert!(!support_dir.join("capture-orphan-context.png").exists());
    assert!(!support_dir.join("capture-legacy-page-number.png").exists());
    assert_eq!(std::fs::read(support_dir.join("notes.txt")).unwrap(), b"keep");
    assert!(support_dir.join("capture-dir.json").is_dir());
}

#[test]
fn support_rotate_reports_symlink_support_dir_without_deleting_target() {
    let tmp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();
    if create_dir_symlink(outside.path(), data_dir.join("support")).is_err() {
        return;
    }
    std::fs::write(outside.path().join("capture-outside.json"), b"outside").unwrap();

    let error = rotate_capture_support_files(tmp.path(), 3, None).unwrap_err();

    assert!(error.to_string().contains("support path is symlink"));
    assert_eq!(
        std::fs::read(outside.path().join("capture-outside.json")).unwrap(),
        b"outside"
    );
}

#[test]
fn support_writer_rejects_symlink_support_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();
    if create_dir_symlink(outside.path(), data_dir.join("support")).is_err() {
        return;
    }
    let status = failed_status("session-symlink");

    let result = write_capture_support(SupportRequest {
        root: tmp.path(),
        status: &status,
        source_kind: "pktmon-auto-page-capture",
        auto_result: None,
    });

    assert!(result.json_path.is_none());
    assert!(result.error.unwrap().contains("support path is symlink"));
    assert!(std::fs::read_dir(outside.path()).unwrap().next().is_none());
}

#[test]
fn support_writer_sanitizes_session_id_and_stays_in_support_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let status = failed_status("../../escape");

    let result = write_capture_support(SupportRequest {
        root: tmp.path(),
        status: &status,
        source_kind: "pktmon-auto-page-capture",
        auto_result: None,
    });

    let path = result.json_path.unwrap();
    assert_eq!(path.parent().unwrap(), tmp.path().join("data/support"));
    assert_eq!(path.file_name().unwrap(), "capture-------escape.json");
    assert!(!tmp.path().join("escape.json").exists());
}

fn write_support_bundle(support_dir: &Path, base: &str) {
    std::fs::write(support_dir.join(format!("{base}.json")), b"json").unwrap();
    std::fs::write(support_dir.join(format!("{base}-context.png")), b"png").unwrap();
}

fn assert_support_bundle_exists(support_dir: &Path, base: &str) {
    assert!(support_dir.join(format!("{base}.json")).is_file());
    assert!(support_dir.join(format!("{base}-context.png")).is_file());
}

fn assert_support_bundle_missing(support_dir: &Path, base: &str) {
    assert!(!support_dir.join(format!("{base}.json")).exists());
    assert!(!support_dir.join(format!("{base}-context.png")).exists());
}

#[cfg(unix)]
fn create_dir_symlink(
    target: impl AsRef<Path>,
    link: impl AsRef<Path>,
) -> Result<(), std::io::Error> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn create_dir_symlink(
    target: impl AsRef<Path>,
    link: impl AsRef<Path>,
) -> Result<(), std::io::Error> {
    std::os::windows::fs::symlink_dir(target, link)
}
