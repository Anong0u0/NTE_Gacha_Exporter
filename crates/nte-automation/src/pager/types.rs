use std::collections::BTreeMap;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use crate::error::{AutomationError, AutomationResult};
use crate::matcher::ImageTemplateMatcher;
use crate::model::{
    AutoPageDiagnostics, AutoPageOptions, AutoPageResult, AutoPageStatus,
    AutoPageWindowDiagnostics, OcrReadDiagnostics, PageNumber, Point, RecordSnapshot, Size,
    TemplateMatch,
};
use crate::ocr::{PageReadHint, WindowsOcrClient};
use crate::profile::{AutomationProfile, WorkflowStep, load_profile};
use crate::screenshot::WindowCaptureClient;
use crate::tooltip::AutomationTooltip;
use crate::window::{self, GameWindow};

const PAGE_RECORD_MIN_WAIT: Duration = Duration::from_millis(300);
const FRESH_PAGE_STABLE_WAIT: Duration = Duration::from_millis(600);
const INCREMENTAL_DUPLICATE_RECORD_THRESHOLD: usize = 6;

pub fn run_auto_page(options: AutoPageOptions) -> AutoPageResult {
    let non_interactive = options.non_interactive;
    let mut pager = match AutoPager::new(options) {
        Ok(pager) => pager,
        Err(error) if non_interactive => {
            return AutoPageResult::failed(error.to_string(), Vec::new(), Vec::new());
        }
        Err(error) => return AutoPageResult::manual(error.to_string(), Vec::new(), Vec::new()),
    };
    match pager.run() {
        Ok(result) => result,
        Err(error) if non_interactive => AutoPageResult::failed_with_diagnostics(
            error.to_string(),
            Vec::new(),
            Vec::new(),
            pager.diagnostics,
        ),
        Err(error) => AutoPageResult::manual_with_diagnostics(
            error.to_string(),
            Vec::new(),
            Vec::new(),
            pager.diagnostics,
        ),
    }
}

struct AutoPager {
    options: AutoPageOptions,
    window: GameWindow,
    profile: AutomationProfile,
    capture: WindowCaptureClient,
    ocr: WindowsOcrClient,
    matcher: ImageTemplateMatcher,
    tooltip: AutomationTooltip,
    started_at: Instant,
    diagnostics: AutoPageDiagnostics,
}

struct PoolPageRun {
    pool: String,
    skipped: bool,
    visited_pages: u32,
    last_page: u32,
}
