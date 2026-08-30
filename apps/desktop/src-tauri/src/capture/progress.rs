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
    ) -> Option<Vec<CaptureRecordSnapshot>> {
        if !progress.rows_snapshot.is_empty() || progress.row_count == 0 {
            let mut builder = CaptureRecordBuilder::new(&self.locale).ok()?;
            let records = builder.build_records(&progress.rows_snapshot);
            return Some(capture_record_snapshots(
                &records,
                &progress.rows_snapshot,
            ));
        }
        if progress.new_rows.is_empty() {
            return None;
        }
        let mut builder = CaptureRecordBuilder::new(&self.locale).ok()?;
        let records = builder.build_records(&progress.new_rows);
        Some(capture_record_snapshots(&records, &progress.new_rows))
    }
}

struct LiveProgressUpdate {
    records_count: u64,
    latest: Vec<Value>,
    automation_snapshot: Option<Vec<CaptureRecordSnapshot>>,
}

fn finish_capture_result(
    runtime: &Arc<CaptureRuntimeSession>,
    input: FinishCaptureInput<'_>,
) {
    let mut final_status = runtime.status.lock().expect("capture status lock");
    let finish = classify_capture_finish(
        input.result,
        input.locale,
        input.source_kind,
        input.auto_error,
        input.auto_result,
        input.cancel_requested,
    );
    apply_capture_finish_status(&mut final_status, finish, input.auto_page);
    final_status.updated_at = now_seconds();
    if final_status.state == crate::lifecycle::STATE_FAILED
        || capture_partial_support_required(&final_status)
    {
        let root = portable_root();
        match root {
            Ok(root) => attach_capture_support(
                &root,
                &mut final_status,
                input.source_kind,
                input.auto_result,
            ),
            Err(error) => {
                if let Some(runtime_error) = final_status.error.as_mut() {
                    runtime_error.message =
                        format!("{}; support_failed: {error}", runtime_error.message);
                } else {
                    append_partial_support_error(&mut final_status, error.to_string());
                }
            }
        }
    }
}

struct FinishCaptureInput<'a> {
    result: Result<nte_capture::CaptureResult, String>,
    locale: &'a str,
    source_kind: &'a str,
    auto_page: Option<Value>,
    auto_error: Option<RuntimeError>,
    auto_result: Option<&'a AutoPageRunResult>,
    cancel_requested: bool,
}

enum CaptureFinish {
    Completed {
        result: nte_capture::CaptureResult,
        document: Value,
    },
    Partial {
        result: nte_capture::CaptureResult,
        document: Value,
        reason: CapturePartialReason,
    },
    Failed {
        result: Option<nte_capture::CaptureResult>,
        error: RuntimeError,
    },
    Cancelled {
        result: Option<nte_capture::CaptureResult>,
    },
}

fn classify_capture_finish(
    result: Result<nte_capture::CaptureResult, String>,
    locale: &str,
    source_kind: &str,
    auto_error: Option<RuntimeError>,
    auto_result: Option<&AutoPageRunResult>,
    cancel_requested: bool,
) -> CaptureFinish {
    match result {
        Ok(result) => {
            if result.rows.is_empty()
                && auto_result.is_some_and(|result| {
                    matches!(
                        result.outcome,
                        AutoPageOutcome::Cancelled
                            | AutoPageOutcome::Partial
                            | AutoPageOutcome::Failed
                    )
                })
            {
                return if cancel_requested {
                    CaptureFinish::Cancelled {
                        result: Some(result),
                    }
                } else {
                    let error = auto_error.unwrap_or_else(|| {
                        auto_page_runtime_error(
                            auto_result
                                .map(|result| result.message.as_str())
                                .unwrap_or_default(),
                        )
                    });
                    let error = auto_page_stalled_capture_path_error(&result, &error)
                        .unwrap_or(error);
                    CaptureFinish::Failed {
                        result: Some(result),
                        error,
                    }
                };
            }
            if let Some(error) = auto_error.as_ref().filter(|_| result.rows.is_empty()) {
                let error = auto_page_stalled_capture_path_error(&result, error)
                    .unwrap_or_else(|| error.clone());
                return if cancel_requested {
                    CaptureFinish::Cancelled {
                        result: Some(result),
                    }
                } else {
                    CaptureFinish::Failed {
                        result: Some(result),
                        error,
                    }
                };
            }
            if let Some(error) = zero_decode_runtime_error(&result) {
                return if cancel_requested {
                    CaptureFinish::Cancelled {
                        result: Some(result),
                    }
                } else {
                    CaptureFinish::Failed {
                        result: Some(result),
                        error,
                    }
                };
            }
            let partial_reason = auto_result.and_then(|auto_result| {
                capture_partial_reason(auto_result, auto_error.as_ref(), cancel_requested)
            });
            let document = if partial_reason.is_some() {
                let auto_result = auto_result.expect("partial reason requires auto-page result");
                let safe_rows = mergeable_auto_page_rows(&result.rows, auto_result);
                if safe_rows.is_empty() {
                    return if cancel_requested {
                        CaptureFinish::Cancelled {
                            result: Some(result),
                        }
                    } else {
                        let error = auto_error.unwrap_or_else(|| {
                            auto_page_runtime_error(auto_result.message.as_str())
                        });
                        let error = auto_page_stalled_capture_path_error(&result, &error)
                            .unwrap_or(error);
                        CaptureFinish::Failed {
                            result: Some(result),
                            error,
                        }
                    };
                }
                match build_capture_document(&safe_rows, locale) {
                    Ok(document) => document,
                    Err(error) => {
                        return CaptureFinish::Failed {
                            result: Some(result),
                            error: runtime_error("capture_document_failed", error.to_string()),
                        };
                    }
                }
            } else {
                match build_capture_document(&result.rows, locale) {
                    Ok(document) => document,
                    Err(error) => {
                        return CaptureFinish::Failed {
                            result: Some(result),
                            error: runtime_error("capture_document_failed", error.to_string()),
                        };
                    }
                }
            };
            if let Some(reason) = partial_reason {
                return CaptureFinish::Partial {
                    result,
                    document,
                    reason,
                };
            }
            CaptureFinish::Completed { result, document }
        }
        Err(_) if cancel_requested => CaptureFinish::Cancelled { result: None },
        Err(message) => CaptureFinish::Failed {
            result: None,
            error: capture_backend_runtime_error(source_kind, &message),
        },
    }
}

fn apply_capture_finish_status(
    final_status: &mut CaptureStatus,
    finish: CaptureFinish,
    auto_page: Option<Value>,
) {
    match finish {
        CaptureFinish::Completed { result, document } => {
            let latest = latest_records_from_capture_document(&document);
            final_status.records_count = result.rows.len() as u64;
            final_status.latest_records = latest;
            final_status.counters = CaptureCounters::from(result.counters);
            final_status.attempts = result.attempts.clone();
            final_status.target = serde_json::to_value(result.target).ok();
            final_status.state = crate::lifecycle::STATE_COMPLETED.to_string();
            final_status.auto_page = auto_page;
            final_status.error = None;
            final_status.document = Some(document);
            final_status.import_report = None;
        }
        CaptureFinish::Partial {
            result,
            document,
            reason,
        } => {
            let latest = latest_records_from_capture_document(&document);
            final_status.records_count = result.rows.len() as u64;
            final_status.latest_records = latest;
            final_status.counters = CaptureCounters::from(result.counters);
            final_status.attempts = result.attempts.clone();
            final_status.target = serde_json::to_value(result.target).ok();
            final_status.state = crate::lifecycle::STATE_PARTIAL.to_string();
            final_status.auto_page = auto_page_with_partial_reason(auto_page, &reason);
            final_status.error = None;
            final_status.document = Some(document);
            final_status.import_report = None;
        }
        CaptureFinish::Failed { result, error } => {
            if let Some(result) = result {
                final_status.records_count = result.rows.len() as u64;
                final_status.latest_records = Vec::new();
                final_status.counters = CaptureCounters::from(result.counters);
                final_status.attempts = result.attempts.clone();
                final_status.target = serde_json::to_value(result.target).ok();
            }
            final_status.state = crate::lifecycle::STATE_FAILED.to_string();
            final_status.auto_page = auto_page;
            final_status.error = Some(error);
            final_status.document = None;
            final_status.import_report = None;
        }
        CaptureFinish::Cancelled { result } => {
            if let Some(result) = result {
                final_status.records_count = result.rows.len() as u64;
                final_status.latest_records = Vec::new();
                final_status.counters = CaptureCounters::from(result.counters);
                final_status.attempts = result.attempts.clone();
                final_status.target = serde_json::to_value(result.target).ok();
            }
            final_status.state = crate::lifecycle::STATE_CANCELLED.to_string();
            final_status.auto_page = auto_page;
            final_status.error = None;
            final_status.document = None;
            final_status.import_report = None;
        }
    }
}

fn capture_partial_reason(
    auto_result: &AutoPageRunResult,
    auto_error: Option<&RuntimeError>,
    cancel_requested: bool,
) -> Option<CapturePartialReason> {
    if let Some(error) = auto_error {
        let (code, retryable) = match error.code.as_str() {
            CAPTURE_STOPPED_CODE => (CAPTURE_STOPPED_DURING_DRAIN_CODE, true),
            CAPTURE_INCOMPLETE_CODE => (CAPTURE_INCOMPLETE_CODE, true),
            AUTO_PAGE_CAPTURE_WINDOW_STALLED_CODE => {
                (AUTO_PAGE_CAPTURE_WINDOW_STALLED_CODE, true)
            }
            _ => (AUTOMATION_INTERRUPTED_CODE, false),
        };
        return Some(CapturePartialReason::new(
            code,
            error.message.clone(),
            retryable,
        ));
    }
    match auto_result.outcome {
        AutoPageOutcome::Completed => None,
        AutoPageOutcome::Cancelled => Some(CapturePartialReason::new(
            USER_STOPPED_CODE,
            auto_result
                .reason
                .as_ref()
                .map(|reason| reason.message.clone())
                .unwrap_or_else(|| "auto page stopped by user".to_string()),
            false,
        )),
        AutoPageOutcome::Partial => {
            let message = auto_result
                .reason
                .as_ref()
                .map(|reason| reason.message.clone())
                .unwrap_or_else(|| auto_result.message.clone());
            let stalled = message.starts_with("capture window stalled:");
            Some(CapturePartialReason::new(
                if stalled {
                    AUTO_PAGE_CAPTURE_WINDOW_STALLED_CODE
                } else {
                    AUTOMATION_INTERRUPTED_CODE
                },
                message,
                stalled,
            ))
        }
        AutoPageOutcome::Failed if cancel_requested => Some(CapturePartialReason::new(
            USER_STOPPED_CODE,
            auto_result.message.clone(),
            false,
        )),
        AutoPageOutcome::Failed => Some(CapturePartialReason::new(
            AUTOMATION_INTERRUPTED_CODE,
            auto_result.message.clone(),
            false,
        )),
    }
}

fn auto_page_with_partial_reason(
    auto_page: Option<Value>,
    reason: &CapturePartialReason,
) -> Option<Value> {
    let mut auto_page = auto_page.unwrap_or_else(|| json!({}));
    let object = auto_page.as_object_mut()?;
    object.insert("state".to_string(), json!("partial"));
    object.insert("message".to_string(), json!(reason.message));
    object.insert("reason".to_string(), serde_json::to_value(reason).ok()?);
    Some(auto_page)
}

fn capture_backend_runtime_error(source_kind: &str, message: &str) -> RuntimeError {
    const WINDIVERT_UNAVAILABLE: &str = "windivert_unavailable";
    if let Some(detail) = message.strip_prefix(WINDIVERT_UNAVAILABLE) {
        let detail = detail.trim_start_matches(':').trim();
        return runtime_error(
            WINDIVERT_UNAVAILABLE,
            if detail.is_empty() { message } else { detail }.to_string(),
        );
    }
    runtime_error(source_kind, message.to_string())
}

fn auto_page_stalled_capture_path_error(
    result: &nte_capture::CaptureResult,
    error: &RuntimeError,
) -> Option<RuntimeError> {
    if error.code != AUTO_PAGE_CAPTURE_WINDOW_STALLED_CODE
        || !result.rows.is_empty()
        || result.counters.decoded_packets > 0
    {
        return None;
    }
    if result.target.interface == "windivert" {
        if result.counters.packets_seen > 0 {
            return Some(runtime_error(
                "windivert_no_decode",
                format!(
                    "WinDivert saw packets but no decodable NTE records were found; auto_page_stalled: {}",
                    error.message
                ),
            ));
        }
        return None;
    }
    Some(runtime_error(
        "vpn_proxy_suspected",
        format!(
            "auto-page stalled with no decoded NTE records through pktmon; packets_seen={}; auto_page_stalled: {}",
            result.counters.packets_seen, error.message
        ),
    ))
}

fn zero_decode_runtime_error(result: &nte_capture::CaptureResult) -> Option<RuntimeError> {
    if !result.rows.is_empty() || result.counters.decoded_packets > 0 {
        return None;
    }
    if result.counters.packets_seen == 0 {
        return Some(runtime_error(
            "no_packets_seen",
            "no packets seen during capture",
        ));
    }
    if result.target.interface == "windivert" {
        return Some(runtime_error(
            "windivert_no_decode",
            "WinDivert saw packets but no decodable NTE records were found",
        ));
    }
    Some(runtime_error(
        "vpn_proxy_suspected",
        "packets seen but no decodable NTE records were found through pktmon",
    ))
}

fn capture_record_snapshot(
    record: &nte_capture::CapturePublicRecord,
    row: &nte_capture::ParsedRow,
) -> Option<CaptureRecordSnapshot> {
    Some(CaptureRecordSnapshot {
        record_key: record.record_key.clone(),
        pool: capture_pool(record.record_type.as_str(), record.pool_id.as_deref())?.to_string(),
        stream_key: row.source.stream_key.clone(),
        generation_index: row.source.generation_index,
        segment_index: row.source.segment_index,
    })
}

fn capture_record_snapshots(
    records: &[nte_capture::CapturePublicRecord],
    rows: &[nte_capture::ParsedRow],
) -> Vec<CaptureRecordSnapshot> {
    records
        .iter()
        .zip(rows)
        .filter_map(|(record, row)| capture_record_snapshot(record, row))
        .collect()
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
fn mergeable_auto_page_rows(
    rows: &[nte_capture::ParsedRow],
    auto_result: &AutoPageRunResult,
) -> Vec<nte_capture::ParsedRow> {
    rows.iter()
        .filter(|row| {
            let Some(pool) = capture_pool(row.record_type.as_str(), row.pool_id.as_deref()) else {
                return false;
            };
            auto_result
                .completed_pools
                .iter()
                .chain(&auto_result.skipped_pools)
                .any(|safe_pool| safe_pool == pool)
        })
        .cloned()
        .collect()
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

fn auto_page_result_value(
    result: &AutoPageRunResult,
    decoded_pages_by_pool: &BTreeMap<String, usize>,
    capture_drain_warning: Option<&RuntimeError>,
) -> Value {
    let required_pages_by_pool = required_capture_pages(result);
    let missing_pages_by_pool =
        missing_capture_pages(&required_pages_by_pool, decoded_pages_by_pool);
    json!({
        "state": result.outcome.as_str(),
        "message": result.message,
        "reason": result.reason,
        "capture_drain_warning": capture_drain_warning,
        "completed_pools": result.completed_pools,
        "skipped_pools": result.skipped_pools,
        "visited_pages_by_pool": result.visited_pages_by_pool,
        "required_pages_by_pool": required_pages_by_pool,
        "decoded_pages_by_pool": decoded_pages_by_pool,
        "missing_pages_by_pool": missing_pages_by_pool,
        "last_page_by_pool": result.last_page_by_pool,
        "visited_pages": result.visited_pages(),
    })
}
