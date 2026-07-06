use crate::live::{CaptureCounters, CaptureOptions, CaptureProgress, CaptureTarget};
use crate::protocol::ParsedRow;

pub(super) struct ProgressPayload<'a> {
    pub new_rows: &'a [ParsedRow],
    pub rows_snapshot: &'a [ParsedRow],
    pub row_count: usize,
    pub warning_count: usize,
}

pub(super) fn emit_progress(
    options: &CaptureOptions,
    target: &CaptureTarget,
    counters: &CaptureCounters,
    payload: ProgressPayload<'_>,
) {
    let Some(callback) = options.on_progress.as_ref() else {
        return;
    };
    callback(CaptureProgress {
        target: target.clone(),
        counters: counters.clone(),
        new_rows: payload.new_rows.to_vec(),
        rows_snapshot: payload.rows_snapshot.to_vec(),
        row_count: payload.row_count,
        warning_count: payload.warning_count,
    });
}
