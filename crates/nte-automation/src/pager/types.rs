use std::collections::BTreeMap;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use crate::error::{AutomationError, AutomationResult};
use crate::matcher::ImageTemplateMatcher;
use crate::model::{
    AUTO_PAGE_INCREMENTAL_DUPLICATE_RECORD_THRESHOLD, AutoPageControlContext,
    AutoPageControlDecision, AutoPageDiagnostics, AutoPageOptions, AutoPageResult, AutoPageStatus,
    AutoPageWindowDiagnostics, MouseClickDiagnostics, PageNumber, Point, RecordSnapshot, Size,
    TemplateMatch,
};
use crate::ocr::{PageNumberReader, PageReadHint};
use crate::profile::{AutomationProfile, WorkflowStep, load_profile};
use crate::screenshot::WindowCaptureClient;
use crate::tooltip::AutomationTooltip;
use crate::window::{self, GameWindow};

const FRESH_PAGE_STABLE_WAIT: Duration = Duration::from_millis(600);

pub fn run_auto_page(options: AutoPageOptions) -> AutoPageResult {
    let non_interactive = options.non_interactive;
    let mut pager = match AutoPager::new(options) {
        Ok(pager) => pager,
        Err(error) => {
            return failed_result(non_interactive, error, AutoPageDiagnostics::default());
        }
    };
    match pager.run() {
        Ok(result) => result,
        Err(error) => failed_result(non_interactive, error, pager.diagnostics),
    }
}

fn failed_result(
    non_interactive: bool,
    error: AutomationError,
    diagnostics: AutoPageDiagnostics,
) -> AutoPageResult {
    failure_result_with_message(non_interactive, error.to_string(), diagnostics)
}

fn failure_result_with_message(
    non_interactive: bool,
    message: String,
    diagnostics: AutoPageDiagnostics,
) -> AutoPageResult {
    if non_interactive {
        AutoPageResult::failed_with_diagnostics(message, Vec::new(), Vec::new(), diagnostics)
    } else {
        AutoPageResult::manual_with_diagnostics(message, Vec::new(), Vec::new(), diagnostics)
    }
}

struct AutoPager {
    options: AutoPageOptions,
    window: GameWindow,
    profile: AutomationProfile,
    capture: WindowCaptureClient,
    page_reader: PageNumberReader,
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

struct PageClickRequest<'a> {
    page_rect: crate::model::Rect,
    point: Point,
    pool: &'a str,
    step: &'a str,
    previous: PageNumber,
    expected_page: u32,
    visited_pages: u32,
}

enum PageClickOutcome {
    Changed(PageNumber),
    SkipPool(PageNumber),
}

enum PageWaitOutcome {
    Changed(PageNumber),
    SkipPool,
    Unchanged,
}
