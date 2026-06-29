struct LiveProgressState {
    locale: String,
    builder: Option<CaptureRecordBuilder>,
    records: Vec<Value>,
}

impl LiveProgressState {
    fn new(locale: &str) -> Self {
        Self {
            locale: locale.to_string(),
            builder: CaptureRecordBuilder::new(locale).ok(),
            records: Vec::new(),
        }
    }

    fn apply(&mut self, progress: &nte_capture::CaptureProgress) -> LiveProgressUpdate {
        if let Some(builder) = self.builder.as_mut() {
            let delta = progress
                .new_rows
                .iter()
                .map(|row| builder.build_record(row))
                .collect::<Vec<_>>();
            self.records
                .extend(delta.iter().map(|record| preview_record(record.value.clone())));
        }
        LiveProgressUpdate {
            records_count: if self.records.is_empty() {
                progress.row_count as u64
            } else {
                self.records.len() as u64
            },
            latest: latest_records(&self.records),
            automation_snapshot: self.automation_snapshot(progress),
        }
    }

    fn automation_snapshot(
        &self,
        progress: &nte_capture::CaptureProgress,
    ) -> Option<Vec<AutomationRecordSnapshot>> {
        if !progress.rows_snapshot.is_empty() || progress.row_count == 0 {
            let mut builder = CaptureRecordBuilder::new(&self.locale).ok()?;
            let records = builder.build_records(&progress.rows_snapshot);
            return Some(records.iter().filter_map(automation_snapshot).collect());
        }
        if progress.new_rows.is_empty() {
            return None;
        }
        let mut builder = CaptureRecordBuilder::new(&self.locale).ok()?;
        let records = builder.build_records(&progress.new_rows);
        Some(records.iter().filter_map(automation_snapshot).collect())
    }
}

struct LiveProgressUpdate {
    records_count: u64,
    latest: Vec<Value>,
    automation_snapshot: Option<Vec<AutomationRecordSnapshot>>,
}

fn finish_capture_result(
    runtime: &Arc<CaptureRuntimeSession>,
    result: Result<nte_capture::CaptureResult, String>,
    locale: &str,
    source_kind: &str,
    auto_page: Option<Value>,
    auto_error: Option<RuntimeError>,
    auto_result: Option<&AutoPageRunResult>,
) {
    let mut final_status = runtime.status.lock().expect("capture status lock");
    let now = now_seconds();
    match result {
        Ok(result) => match build_capture_document(&result.rows, locale) {
            Ok(document) => {
                let latest = latest_records_from_capture_document(&document);
                final_status.records_count = result.rows.len() as u64;
                final_status.latest_records = latest;
                final_status.counters = CaptureCounters::from(result.counters);
                final_status.target = serde_json::to_value(result.target).ok();
                final_status.state = if auto_error.is_some() {
                    "failed".to_string()
                } else {
                    "completed".to_string()
                };
                final_status.auto_page = auto_page;
                final_status.error = auto_error;
                final_status.document = Some(document);
                final_status.import_report = None;
            }
            Err(error) => {
                final_status.state = "failed".to_string();
                final_status.error = Some(runtime_error("capture_document_failed", error.to_string()));
            }
        },
        Err(message) => {
            final_status.state = "failed".to_string();
            final_status.error = Some(runtime_error(source_kind, message));
            final_status.auto_page = auto_page;
        }
    }
    final_status.updated_at = now;
    if final_status.state == "failed" {
        let root = portable_root();
        match root {
            Ok(root) => attach_capture_support(&root, &mut final_status, source_kind, auto_result),
            Err(error) => {
                if let Some(runtime_error) = final_status.error.as_mut() {
                    runtime_error.message =
                        format!("{}; support_failed: {error}", runtime_error.message);
                }
            }
        }
    }
}

fn automation_snapshot(
    record: &nte_capture::CapturePublicRecord,
) -> Option<AutomationRecordSnapshot> {
    Some(AutomationRecordSnapshot {
        record_id: record.record_id.clone(),
        record_key: record.record_key.clone(),
        record_type: record.record_type.clone(),
        pool_id: record.pool_id.clone()?,
    })
}

fn latest_records(records: &[Value]) -> Vec<Value> {
    records.iter().rev().take(10).cloned().collect::<Vec<_>>()
}

fn preview_record(mut record: Value) -> Value {
    if let Some(object) = record.as_object_mut() {
        object.remove("source_order");
    }
    record
}

fn latest_records_from_capture_document(document: &Value) -> Vec<Value> {
    document
        .get("nte")
        .and_then(|value| value.get("list"))
        .and_then(Value::as_array)
        .map(|records| latest_records(records))
        .unwrap_or_default()
}

fn capture_pool(record_type: &str, pool_id: Option<&str>) -> Option<&'static str> {
    if record_type == "monopoly" && pool_id == Some("CardPool_Character") {
        return Some("limited");
    }
    if record_type == "monopoly" && pool_id == Some("CardPool_NewRole") {
        return Some("standard");
    }
    if record_type == "fork" || pool_id.is_some_and(|pool_id| pool_id.starts_with("ForkLottery_")) {
        return Some("fork");
    }
    None
}

fn auto_page_status_value(status: &AutomationStatus, state: &str) -> Value {
    json!({
        "state": state,
        "message": status.message,
        "kind": status.kind,
        "step": status.step,
        "pool": status.pool,
        "current_page": status.current_page,
        "total_pages": status.total_pages,
        "technical_detail": status.technical_detail,
        "elapsed_seconds": status.elapsed_seconds,
    })
}

fn auto_page_result_value(result: &AutoPageRunResult) -> Value {
    json!({
        "state": if result.succeeded() { "completed" } else { "failed" },
        "message": result.message,
        "completed_pools": result.completed_pools,
        "skipped_pools": result.skipped_pools,
        "visited_pages_by_pool": result.visited_pages_by_pool,
        "last_page_by_pool": result.last_page_by_pool,
    })
}
