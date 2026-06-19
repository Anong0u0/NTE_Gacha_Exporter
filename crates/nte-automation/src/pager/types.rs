use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use crate::error::{AutomationError, AutomationResult};
use crate::matcher::ImageTemplateMatcher;
use crate::model::{
    AutoPageOptions, AutoPageResult, AutoPageStatus, PageNumber, Point, RecordSnapshot,
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
    match AutoPager::new(options).and_then(|mut pager| pager.run()) {
        Ok(result) => result,
        Err(error) if non_interactive => {
            AutoPageResult::failed(error.to_string(), Vec::new(), Vec::new())
        }
        Err(error) => AutoPageResult::manual(error.to_string(), Vec::new(), Vec::new()),
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
}

struct PoolPageRun {
    pool: String,
    skipped: bool,
    visited_pages: u32,
    last_page: u32,
}

