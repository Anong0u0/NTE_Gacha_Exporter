use std::collections::BTreeMap;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use crate::error::{AutomationError, AutomationResult};
use crate::matcher::ImageTemplateMatcher;
use crate::model::{
    AUTO_PAGE_INCREMENTAL_DUPLICATE_RECORD_THRESHOLD, AutoPageControlContext,
    AutoPageControlDecision, AutoPageDiagnostics, AutoPageOptions, AutoPageResult, AutoPageStatus,
    AutoPageStopReason, AutoPageWindowDiagnostics, MouseClickDiagnostics,
    PageTransitionDiagnostics, Point, RecordSnapshot, Size, TemplateMatch,
    TemplateTransitionDiagnostics,
};
use crate::ocr::{PageFingerprint, PageNumberReader, PageObservation, PageReadConstraint};
use crate::profile::{AutomationProfile, WorkflowStep, load_profile};
use crate::screenshot::WindowCaptureClient;
use crate::tooltip::AutomationTooltip;
use crate::window::{self, GameWindow};

const FRESH_PAGE_STABLE_WAIT: Duration = Duration::from_millis(600);
pub(crate) const PAGE_FINGERPRINT_STABLE_DISTANCE: f32 = 0.012650111;
pub(crate) const PAGE_CURRENT_CHANGED_DISTANCE: f32 = 0.012650111;
pub(crate) const PAGE_SUFFIX_UNCHANGED_DISTANCE: f32 = 0.05817671;
const PAGE_TRANSITION_STABLE_SAMPLES: u8 = 2;
const MAX_PAGE_CLICK_ATTEMPTS: u32 = 2;

pub fn run_auto_page(options: AutoPageOptions) -> AutoPageResult {
    let mut pager = match AutoPager::new(options) {
        Ok(pager) => pager,
        Err(error) => {
            return failed_result(error, AutoPageDiagnostics::default());
        }
    };
    match pager.run() {
        Ok(result) => result,
        Err(error) => pager.error_result(error),
    }
}

impl AutoPager {
    fn error_result(&self, error: AutomationError) -> AutoPageResult {
        let message = error.to_string();
        if self.should_stop() {
            return self.result(
                crate::model::AutoPageOutcome::Cancelled,
                "auto page cancelled".to_string(),
                Some(AutoPageStopReason {
                    code: "cancelled".to_string(),
                    message,
                }),
            );
        }
        if !self.visited_pages_by_pool.is_empty() {
            return self.result(
                crate::model::AutoPageOutcome::Partial,
                "auto page ended with partial data".to_string(),
                Some(AutoPageStopReason {
                    code: "automation_interrupted".to_string(),
                    message,
                }),
            );
        }
        self.result(
            crate::model::AutoPageOutcome::Failed,
            message.clone(),
            Some(AutoPageStopReason {
                code: "automation_failed".to_string(),
                message,
            }),
        )
    }
}

fn failed_result(
    error: AutomationError,
    diagnostics: AutoPageDiagnostics,
) -> AutoPageResult {
    AutoPageResult::failed_with_diagnostics(
        error.to_string(),
        Vec::new(),
        Vec::new(),
        diagnostics,
    )
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
    completed_pools: Vec<String>,
    skipped_pools: Vec<String>,
    visited_pages_by_pool: BTreeMap<String, u32>,
    last_page_by_pool: BTreeMap<String, u32>,
}

struct PoolPageRun {
    pool: String,
    skipped: bool,
    visited_pages: u32,
    last_page: u32,
    partial_reason: Option<AutoPageStopReason>,
}

#[derive(Debug, Clone)]
struct PageCursor {
    current: u32,
    total: Option<u32>,
    fingerprint: PageFingerprint,
}

impl PageCursor {
    fn advance(&self, current: u32, fingerprint: PageFingerprint) -> Self {
        Self {
            current,
            total: self.total,
            fingerprint,
        }
    }
}

#[derive(Default)]
struct StablePageTotal {
    candidate: Option<u32>,
    samples: u8,
}

impl StablePageTotal {
    fn reset(&mut self) {
        self.candidate = None;
        self.samples = 0;
    }

    fn observe(&mut self, value: Option<u32>, minimum: u32) {
        let Some(value) = value.filter(|value| *value >= minimum) else {
            self.reset();
            return;
        };
        if self.candidate == Some(value) {
            self.samples = self.samples.saturating_add(1);
        } else {
            self.candidate = Some(value);
            self.samples = 1;
        }
    }

    fn confirmed(&self) -> Option<u32> {
        (self.samples >= PAGE_TRANSITION_STABLE_SAMPLES)
            .then_some(self.candidate)
            .flatten()
    }
}


struct PageClickRequest<'a> {
    page_rect: crate::model::Rect,
    point: Point,
    pool: &'a str,
    previous: PageCursor,
    expected_page: u32,
}

enum PageClickOutcome {
    Changed(PageCursor),
    Partial(AutoPageStopReason),
}

enum PageWaitOutcome {
    Changed(PageFingerprint),
    Unchanged,
    Indeterminate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PageTransitionProgress {
    Waiting,
    Candidate,
    Confirmed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PageTransitionTerminal {
    Unchanged,
    Indeterminate,
}

#[derive(Default)]
struct PageTransitionTracker {
    saw_fingerprint: bool,
    all_baseline: bool,
    last_changed: Option<PageFingerprint>,
    stable_changed_samples: u8,
    current_distance: f32,
    suffix_distance: f32,
}

impl PageTransitionTracker {
    fn new() -> Self {
        Self {
            all_baseline: true,
            ..Self::default()
        }
    }

    fn observe_missing(&mut self) {
        self.all_baseline = false;
        self.reset_changed_stability();
    }

    fn observe(
        &mut self,
        baseline: &PageFingerprint,
        candidate: &PageFingerprint,
    ) -> PageTransitionProgress {
        self.saw_fingerprint = true;
        self.current_distance = baseline.current_distance(candidate);
        self.suffix_distance = baseline.suffix_distance(candidate);
        let same = self.current_distance <= PAGE_FINGERPRINT_STABLE_DISTANCE
            && self.suffix_distance <= PAGE_FINGERPRINT_STABLE_DISTANCE;
        let changed = self.current_distance >= PAGE_CURRENT_CHANGED_DISTANCE
            && self.suffix_distance <= PAGE_SUFFIX_UNCHANGED_DISTANCE;

        if same {
            self.reset_changed_stability();
            return PageTransitionProgress::Waiting;
        }

        self.all_baseline = false;
        if !changed {
            self.reset_changed_stability();
            return PageTransitionProgress::Waiting;
        }

        let stable = self.last_changed.as_ref().is_some_and(|last| {
            last.current_distance(candidate) <= PAGE_FINGERPRINT_STABLE_DISTANCE
                && last.suffix_distance(candidate) <= PAGE_FINGERPRINT_STABLE_DISTANCE
        });
        self.stable_changed_samples = if stable {
            self.stable_changed_samples.saturating_add(1)
        } else {
            1
        };
        self.last_changed = Some(candidate.clone());
        if self.stable_changed_samples >= PAGE_TRANSITION_STABLE_SAMPLES {
            PageTransitionProgress::Confirmed
        } else {
            PageTransitionProgress::Candidate
        }
    }

    fn finish(&self) -> PageTransitionTerminal {
        if self.saw_fingerprint && self.all_baseline {
            PageTransitionTerminal::Unchanged
        } else {
            PageTransitionTerminal::Indeterminate
        }
    }

    fn reset_changed_stability(&mut self) {
        self.last_changed = None;
        self.stable_changed_samples = 0;
    }
}
