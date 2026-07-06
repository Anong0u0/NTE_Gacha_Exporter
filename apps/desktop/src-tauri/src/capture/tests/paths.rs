#[test]
fn capture_raw_output_path_uses_generated_run_jsonl() {
    let tmp = tempfile::tempdir().unwrap();
    let store = nte_store::JsonStore::open(tmp.path()).unwrap();

    let paths = (0..3)
        .map(|_| capture_raw_output_path(&store))
        .collect::<Vec<_>>();

    assert_eq!(paths.len(), 3);
    assert!(paths.iter().all(|path| {
        let path = Path::new(path);
        path.parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            == Some("runs")
            && path
                .parent()
                .and_then(Path::parent)
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
                == Some("data")
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("raw-") && name.ends_with(".jsonl"))
    }));
    assert_eq!(
        paths.iter().collect::<std::collections::BTreeSet<_>>().len(),
        paths.len()
    );
}

#[test]
fn latest_records_read_capture_document_nte_list() {
    let records = (0..12)
        .map(|index| json!({ "record_id": format!("r{index}") }))
        .collect::<Vec<_>>();
    let document = json!({ "nte": { "list": records } });

    let latest = latest_records_from_capture_document(&document);

    assert_eq!(latest.len(), 10);
    assert_eq!(latest[0]["record_id"], "r11");
    assert_eq!(latest[9]["record_id"], "r2");
}

#[test]
fn latest_records_missing_capture_document_list_returns_empty() {
    assert!(latest_records_from_capture_document(&json!({ "records": [] })).is_empty());
}

#[test]
fn capture_pool_uses_auto_page_workflow_pool_keys() {
    assert_eq!(
        capture_pool("monopoly", Some("CardPool_Character")),
        Some("limited")
    );
    assert_eq!(
        capture_pool("monopoly", Some("CardPool_NewRole")),
        Some("standard")
    );
    assert_eq!(capture_pool("fork", Some("ForkLottery_AnHunQu")), Some("fork"));
    assert_eq!(capture_pool("monopoly", Some("CardPool_Weapon")), None);
}
