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

const PAGE_RECORD_MIN_WAIT: Duration = Duration::from_millis(200);
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

impl AutoPager {
    fn new(options: AutoPageOptions) -> AutomationResult<Self> {
        window::require_windows()?;
        let profile = load_profile()?;
        let window = window::resolve_game_window(options.pid, &profile.window.class_name)?;
        let scaled_profile = profile.scaled(window.client_size())?;
        let pager = Self {
            tooltip: AutomationTooltip::new(options.tooltip),
            options,
            window: window.clone(),
            capture: WindowCaptureClient::new(window.hwnd),
            ocr: WindowsOcrClient::default(),
            matcher: ImageTemplateMatcher::new(scaled_profile.clone()),
            profile: scaled_profile,
            started_at: Instant::now(),
        };
        if let Some(reason) = pager.tooltip.unavailable_reason() {
            if reason != "disabled" {
                pager.status(
                    "tooltip unavailable",
                    "diagnostic",
                    None,
                    None,
                    None,
                    None,
                    reason,
                    false,
                );
            }
        }
        Ok(pager)
    }

    fn run(&mut self) -> AutomationResult<AutoPageResult> {
        let mut completed = Vec::new();
        let mut skipped = Vec::new();
        let mut visited_pages_by_pool = BTreeMap::new();
        let mut last_page_by_pool = BTreeMap::new();
        self.status(
            "auto page started",
            "started",
            Some("started"),
            None,
            None,
            None,
            "",
            true,
        );
        self.focus_window()?;
        for step in self.profile.workflow.clone() {
            if self.should_stop() {
                return Err(AutomationError::message("auto page stopped"));
            }
            if let Some(result) = self.run_step(&step)? {
                visited_pages_by_pool.insert(result.pool.clone(), result.visited_pages);
                last_page_by_pool.insert(result.pool.clone(), result.last_page);
                if result.skipped {
                    skipped.push(result.pool);
                } else {
                    completed.push(result.pool);
                }
            }
        }
        self.status(
            "auto page completed",
            "completed",
            Some("completed"),
            None,
            None,
            None,
            "",
            true,
        );
        Ok(AutoPageResult::completed_with_pages(
            completed,
            skipped,
            visited_pages_by_pool,
            last_page_by_pool,
        ))
    }

    fn run_step(&mut self, step: &WorkflowStep) -> AutomationResult<Option<PoolPageRun>> {
        if !step.status.is_empty() {
            self.status(
                &step.status,
                "step",
                Some(&step.status),
                None,
                None,
                None,
                "",
                true,
            );
        }
        match step.action.as_str() {
            "verifyTemplate" => {
                self.verify_template(
                    required(step.template.as_deref(), "template")?,
                    &step.status,
                )?;
                Ok(None)
            }
            "click" => {
                let point = self.point(required(step.point.as_deref(), "point")?)?;
                self.click(point, step.settle)?;
                Ok(None)
            }
            "clickUntilTemplate" => {
                self.click_until_template(step)?;
                Ok(None)
            }
            "clickTemplateUntilTemplate" => {
                self.click_template_until_template(step)?;
                Ok(None)
            }
            "pressEsc" => {
                window::foreground_escape(&self.window)?;
                thread::sleep(Duration::from_millis(100));
                Ok(None)
            }
            "page" => self.capture_pages(step).map(Some),
            other => Err(AutomationError::message(format!(
                "unsupported workflow action: {other}"
            ))),
        }
    }

    fn verify_template(&mut self, name: &str, step: &str) -> AutomationResult<()> {
        let started = Instant::now();
        let (matched, attempts) = self.wait_for_template(name)?;
        self.status(
            "template verified",
            "template",
            Some(step),
            None,
            None,
            None,
            &format!(
                "{name} edge={:.3} gray={:.3} at={},{} wait={:.2}s tries={attempts}",
                matched.edge_score,
                matched.gray_score,
                matched.point.x,
                matched.point.y,
                started.elapsed().as_secs_f64()
            ),
            true,
        );
        Ok(())
    }

    fn wait_for_template(&mut self, name: &str) -> AutomationResult<(TemplateMatch, u32)> {
        let deadline = Instant::now() + Duration::from_secs_f64(self.options.template_timeout);
        let mut attempts = 0_u32;
        let mut next_focus = Instant::now();
        let mut last_error = None;
        while Instant::now() < deadline {
            if self.should_stop() {
                return Err(AutomationError::message("auto page stopped"));
            }
            if Instant::now() >= next_focus {
                self.focus_window()?;
                next_focus = Instant::now() + Duration::from_millis(500);
            }
            attempts += 1;
            match self.try_template(name) {
                Ok(result) => return Ok((result, attempts)),
                Err(error) => {
                    last_error = Some(error);
                    self.sleep_poll();
                }
            }
        }
        let detail = last_error
            .map(|error| format!(": {error}"))
            .unwrap_or_default();
        Err(AutomationError::message(format!(
            "screen template not found after wait: {name}{detail}"
        )))
    }

    fn click_until_template(&mut self, step: &WorkflowStep) -> AutomationResult<()> {
        let template = required(step.template.as_deref(), "template")?;
        if step.point_sequence.is_empty() {
            return Err(AutomationError::message(
                "workflow step missing pointSequence",
            ));
        }
        let points = step
            .point_sequence
            .iter()
            .map(|name| self.point(name))
            .collect::<AutomationResult<Vec<_>>>()?;
        let settle = step.settle.unwrap_or(0.1);
        let deadline = Instant::now() + Duration::from_secs_f64(self.options.template_timeout);
        let started = Instant::now();
        let mut clicks = 0_u32;
        let mut last_error = None;
        while Instant::now() < deadline {
            if self.should_stop() {
                return Err(AutomationError::message("auto page stopped"));
            }
            for point in &points {
                if Instant::now() >= deadline {
                    break;
                }
                self.click(*point, Some(settle))?;
                clicks += 1;
                match self.try_template(template) {
                    Ok(matched) => {
                        self.status(
                            "template verified",
                            "template",
                            Some(&step.status),
                            None,
                            None,
                            None,
                            &format!(
                                "{template} edge={:.3} gray={:.3} at={},{} wait={:.2}s clicks={clicks}",
                                matched.edge_score,
                                matched.gray_score,
                                matched.point.x,
                                matched.point.y,
                                started.elapsed().as_secs_f64()
                            ),
                            true,
                        );
                        return Ok(());
                    }
                    Err(error) => {
                        last_error = Some(error);
                        self.sleep_poll();
                    }
                }
            }
        }
        let detail = last_error
            .map(|error| format!(": {error}"))
            .unwrap_or_default();
        Err(AutomationError::message(format!(
            "screen template not found after click sequence: {template}{detail}"
        )))
    }

    fn click_template_until_template(&mut self, step: &WorkflowStep) -> AutomationResult<()> {
        let source_template = required(step.template.as_deref(), "template")?;
        let target_template = required(step.target_template.as_deref(), "targetTemplate")?;
        let settle = step.settle.unwrap_or(0.1);
        let deadline = Instant::now() + Duration::from_secs_f64(self.options.template_timeout);
        let started = Instant::now();
        let mut clicks = 0_u32;
        let mut source: Option<TemplateMatch> = None;
        let mut next_source_click = Instant::now();
        let mut last_source_error = None;
        let mut last_target_error = None;
        while Instant::now() < deadline {
            if self.should_stop() {
                return Err(AutomationError::message("auto page stopped"));
            }
            match self.try_template(target_template) {
                Ok(target) => {
                    let source_detail = source.as_ref().map_or_else(
                        || format!("{source_template} already resolved "),
                        |source| {
                            format!(
                                "{source_template} at={},{} ",
                                source.point.x, source.point.y
                            )
                        },
                    );
                    self.status(
                        "template verified",
                        "template",
                        Some(&step.status),
                        None,
                        None,
                        None,
                        &format!(
                            "{source_detail}{target_template} edge={:.3} gray={:.3} at={},{} wait={:.2}s clicks={clicks}",
                            target.edge_score,
                            target.gray_score,
                            target.point.x,
                            target.point.y,
                            started.elapsed().as_secs_f64()
                        ),
                        true,
                    );
                    return Ok(());
                }
                Err(error) => last_target_error = Some(error),
            }
            if Instant::now() < next_source_click {
                self.sleep_poll();
                continue;
            }
            match self.find_template(source_template) {
                Ok(matched) => {
                    let click_point = if let Some(point_name) = step.point.as_deref() {
                        self.point(point_name)?
                    } else {
                        self.template_center(source_template, matched.point)?
                    };
                    self.click(click_point, Some(settle))?;
                    clicks += 1;
                    source = Some(matched);
                    next_source_click =
                        Instant::now() + Duration::from_secs_f64(self.options.click_poll_interval);
                }
                Err(error) => {
                    last_source_error = Some(error);
                    self.sleep_poll();
                }
            }
        }
        let detail = last_target_error
            .or(last_source_error)
            .map(|error| format!(": {error}"))
            .unwrap_or_default();
        Err(AutomationError::message(format!(
            "screen template not found after template click sequence: {source_template}->{target_template}{detail}"
        )))
    }

    fn capture_pages(&mut self, step: &WorkflowStep) -> AutomationResult<PoolPageRun> {
        let pool = required(step.pool.as_deref(), "pool")?.to_string();
        let page_rect = *self
            .profile
            .rects
            .get(required(step.page_rect.as_deref(), "pageRect")?)
            .ok_or_else(|| AutomationError::message("workflow pageRect missing from profile"))?;
        let next_button = self.point(required(step.next_button.as_deref(), "nextButton")?)?;
        let mut page = self.wait_for_fresh_page(page_rect, &pool)?;
        self.status(
            "page ready",
            "page",
            Some(&step.status),
            Some(&pool),
            Some(page.current),
            Some(page.total),
            "",
            true,
        );
        let mut visited_pages = 1_u32;
        if self.should_skip_pool(&pool, &step.status, &page) {
            return Ok(PoolPageRun {
                pool,
                skipped: true,
                visited_pages,
                last_page: page.current,
            });
        }
        while page.current < page.total {
            if self.should_stop() {
                return Err(AutomationError::message("auto page stopped"));
            }
            let expected = page.current + 1;
            self.status(
                "page next",
                "page",
                Some(&step.status),
                Some(&pool),
                Some(expected),
                Some(page.total),
                "",
                true,
            );
            page = self.click_page_button(page_rect, next_button, page, expected)?;
            visited_pages = page.current;
            self.wait_for_capture_lag(&pool, visited_pages, page.total)?;
            if self.should_skip_pool(&pool, &step.status, &page) {
                return Ok(PoolPageRun {
                    pool,
                    skipped: true,
                    visited_pages,
                    last_page: page.current,
                });
            }
        }
        self.status(
            "pool completed",
            "pool_completed",
            Some(&step.status),
            Some(&pool),
            Some(page.total),
            Some(page.total),
            "",
            true,
        );
        Ok(PoolPageRun {
            pool,
            skipped: false,
            visited_pages,
            last_page: page.current,
        })
    }

    fn should_skip_pool(&mut self, pool: &str, step: &str, page: &PageNumber) -> bool {
        if self.options.full_update
            || self.options.known_record_ids.is_empty()
            || self.options.record_snapshot.is_none()
        {
            return false;
        }
        let known_ids = self
            .options
            .known_record_ids
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        let deadline =
            Instant::now() + Duration::from_secs_f64(self.options.duplicate_check_timeout);
        while Instant::now() <= deadline {
            let pool_records = self.pool_records(pool);
            let duplicate_count = consecutive_known_record_count(&pool_records, &known_ids);
            if duplicate_count >= INCREMENTAL_DUPLICATE_RECORD_THRESHOLD {
                self.status(
                    "known records found; skipping pool",
                    "pool_skipped",
                    Some(step),
                    Some(pool),
                    Some(page.current),
                    Some(page.total),
                    &format!("duplicate_records={duplicate_count}"),
                    true,
                );
                return true;
            }
            if pool_records
                .last()
                .is_some_and(|record| !known_ids.contains(record.record_id.as_str()))
            {
                return false;
            }
            self.sleep_poll();
        }
        false
    }

    fn pool_records(&self, pool: &str) -> Vec<RecordSnapshot> {
        let Some(callback) = &self.options.record_snapshot else {
            return Vec::new();
        };
        callback()
            .into_iter()
            .filter(|record| record_pool(record).as_deref() == Some(pool))
            .collect()
    }

    fn click_page_button(
        &mut self,
        page_rect: crate::model::Rect,
        point: Point,
        previous: PageNumber,
        expected_page: u32,
    ) -> AutomationResult<PageNumber> {
        for attempt in 1..=2 {
            let clicked_at = Instant::now();
            window::foreground_click(&self.window, point)?;
            if let Some(page) =
                self.wait_for_page(page_rect, previous.current, expected_page, previous.total)?
            {
                self.settle_after_page_click(clicked_at);
                return Ok(page);
            }
            if attempt < 2 {
                self.status(
                    "page did not change; retrying click",
                    "retry",
                    None,
                    None,
                    Some(previous.current),
                    Some(previous.total),
                    &format!("attempt={}/2", attempt + 1),
                    true,
                );
            }
        }
        Err(AutomationError::message(format!(
            "page did not change after retry: expected {expected_page}, still {}",
            previous.current
        )))
    }

    fn settle_after_page_click(&self, clicked_at: Instant) {
        sleep_until(clicked_at + PAGE_RECORD_MIN_WAIT);
    }

    fn wait_for_capture_lag(
        &mut self,
        pool: &str,
        visited_pages: u32,
        total_pages: u32,
    ) -> AutomationResult<()> {
        let Some(decoded_page_count) = &self.options.decoded_page_count else {
            return Ok(());
        };
        let max_lag = self.options.max_capture_page_lag;
        if decoded_page_count(pool).saturating_add(max_lag) >= visited_pages as usize {
            return Ok(());
        }
        let deadline = Instant::now() + Duration::from_secs_f64(self.options.click_timeout);
        while Instant::now() < deadline {
            if self.should_stop() {
                return Err(AutomationError::message("auto page stopped"));
            }
            let decoded = decoded_page_count(pool);
            if decoded.saturating_add(max_lag) >= visited_pages as usize {
                return Ok(());
            }
            self.status(
                "capture lag waiting",
                "diagnostic",
                None,
                Some(pool),
                Some(visited_pages),
                Some(total_pages),
                &format!("decoded_pages={decoded} max_lag={max_lag}"),
                false,
            );
            thread::sleep(Duration::from_millis(100));
        }
        Ok(())
    }

    fn wait_for_page(
        &mut self,
        page_rect: crate::model::Rect,
        previous_page: u32,
        expected_page: u32,
        expected_total: u32,
    ) -> AutomationResult<Option<PageNumber>> {
        let deadline = Instant::now() + Duration::from_secs_f64(self.options.click_timeout);
        let mut last_error = None;
        let mut saw_previous = false;
        let mut unexpected_page = None::<PageNumber>;
        let mut unexpected_count = 0_u8;
        while Instant::now() < deadline {
            if self.should_stop() {
                return Err(AutomationError::message("auto page stopped"));
            }
            self.sleep_poll();
            match self.read_page_with_hint(
                page_rect,
                PageReadHint {
                    previous_current: Some(previous_page),
                    expected_current: Some(expected_page),
                    expected_total: Some(expected_total),
                },
            ) {
                Ok(page) if page.current == expected_page => return Ok(Some(page)),
                Ok(page) if page.current == previous_page => saw_previous = true,
                Ok(page) => {
                    let same_as_last = unexpected_page.as_ref().is_some_and(|last| {
                        last.current == page.current && last.total == page.total
                    });
                    unexpected_count = if same_as_last {
                        unexpected_count.saturating_add(1)
                    } else {
                        1
                    };
                    unexpected_page = Some(page.clone());
                    if unexpected_count >= 2 {
                        return Err(AutomationError::message(format!(
                            "unexpected page after click: {}/{}",
                            page.current, page.total
                        )));
                    }
                }
                Err(error) => last_error = Some(error),
            }
        }
        if let Some(error) = last_error {
            self.status(
                "OCR waiting ended",
                "diagnostic",
                None,
                None,
                None,
                None,
                &error.to_string(),
                true,
            );
            if !saw_previous {
                return Err(AutomationError::message(format!(
                    "OCR unreadable after click: {error}"
                )));
            }
        }
        if let Some(page) = unexpected_page {
            return Err(AutomationError::message(format!(
                "unexpected page after click: {}/{}",
                page.current, page.total
            )));
        }
        Ok(None)
    }

    fn wait_for_fresh_page(
        &mut self,
        page_rect: crate::model::Rect,
        pool: &str,
    ) -> AutomationResult<PageNumber> {
        let deadline = Instant::now() + Duration::from_secs_f64(self.options.template_timeout);
        let mut stable_page = None::<(PageNumber, Instant)>;
        let mut last_page = None;
        let mut last_error = None;
        while Instant::now() < deadline {
            if self.should_stop() {
                return Err(AutomationError::message("auto page stopped"));
            }
            match self.read_page_with_hint(page_rect, PageReadHint::default()) {
                Ok(page) if page.current == 1 => {
                    let now = Instant::now();
                    let stable_since = stable_page.as_ref().map_or(now, |(stable, since)| {
                        if stable.current == page.current && stable.total == page.total {
                            *since
                        } else {
                            now
                        }
                    });
                    stable_page = Some((page.clone(), stable_since));
                    last_page = Some(page.clone());
                    if now.duration_since(stable_since) >= FRESH_PAGE_STABLE_WAIT {
                        return Ok(page);
                    }
                }
                Ok(page) => {
                    stable_page = None;
                    last_page = Some(page);
                }
                Err(error) => last_error = Some(error),
            }
            self.sleep_poll();
        }
        if let Some(page) = last_page {
            return Err(AutomationError::message(format!(
                "{pool}: freshly opened record page must be 1/{}, got {}/{}",
                page.total, page.current, page.total
            )));
        }
        Err(AutomationError::message(format!(
            "{pool}: freshly opened record page unreadable: {}",
            last_error
                .map(|error| error.to_string())
                .unwrap_or_else(|| "timeout".to_string())
        )))
    }

    fn click(&mut self, point: Point, settle: Option<f64>) -> AutomationResult<()> {
        window::foreground_click(&self.window, point)?;
        thread::sleep(Duration::from_secs_f64(settle.unwrap_or(0.1)));
        Ok(())
    }

    fn try_template(&mut self, name: &str) -> AutomationResult<TemplateMatch> {
        self.focus_window()?;
        let search_rect = self.matcher.search_rect(name, self.window.client_size())?;
        let image = self.capture.capture_rect(search_rect)?;
        self.matcher.verify_in_rect(name, &image, search_rect)
    }

    fn find_template(&mut self, name: &str) -> AutomationResult<TemplateMatch> {
        self.focus_window()?;
        let search_rect = self.matcher.search_rect(name, self.window.client_size())?;
        let image = self.capture.capture_rect(search_rect)?;
        self.matcher.find_in_rect(name, &image, search_rect)
    }

    fn template_center(&self, name: &str, top_left: Point) -> AutomationResult<Point> {
        let rect = self
            .profile
            .templates
            .get(name)
            .ok_or_else(|| AutomationError::message(format!("unknown template: {name}")))?
            .rect;
        Ok(Point {
            x: top_left.x + rect.width as i32 / 2,
            y: top_left.y + rect.height as i32 / 2,
        })
    }

    fn read_page_with_hint(
        &mut self,
        page_rect: crate::model::Rect,
        hint: PageReadHint,
    ) -> AutomationResult<PageNumber> {
        self.focus_window()?;
        let image = self.capture.capture_rect(page_rect)?;
        self.ocr.read_page_number_with_hint(&image, hint)
    }

    fn point(&self, name: &str) -> AutomationResult<Point> {
        self.profile
            .points
            .get(name)
            .copied()
            .ok_or_else(|| AutomationError::message(format!("unknown point: {name}")))
    }

    fn should_stop(&self) -> bool {
        self.options.stop.load(Ordering::SeqCst) || window::escape_pressed()
    }

    fn focus_window(&mut self) -> AutomationResult<()> {
        self.window = window::refresh_window(&self.window)?;
        window::force_foreground(&self.window)
    }

    #[allow(clippy::too_many_arguments)]
    fn status(
        &self,
        message: &str,
        kind: &str,
        step: Option<&str>,
        pool: Option<&str>,
        current_page: Option<u32>,
        total_pages: Option<u32>,
        technical_detail: &str,
        replaceable: bool,
    ) {
        if let Some(callback) = &self.options.on_status {
            let status = AutoPageStatus {
                elapsed_seconds: self.started_at.elapsed().as_secs_f64(),
                message: message.to_string(),
                kind: kind.to_string(),
                step: step.map(str::to_string),
                pool: pool.map(str::to_string),
                current_page,
                total_pages,
                technical_detail: technical_detail.to_string(),
                replaceable,
            };
            self.tooltip.show(&status_text(&status));
            callback(status);
        } else {
            self.tooltip.show(&status_text(&AutoPageStatus {
                elapsed_seconds: self.started_at.elapsed().as_secs_f64(),
                message: message.to_string(),
                kind: kind.to_string(),
                step: step.map(str::to_string),
                pool: pool.map(str::to_string),
                current_page,
                total_pages,
                technical_detail: technical_detail.to_string(),
                replaceable,
            }));
        }
    }

    fn sleep_poll(&self) {
        thread::sleep(Duration::from_secs_f64(self.options.click_poll_interval));
    }
}

fn required<'a>(value: Option<&'a str>, name: &str) -> AutomationResult<&'a str> {
    value.ok_or_else(|| AutomationError::message(format!("workflow step missing {name}")))
}

fn sleep_until(deadline: Instant) {
    let now = Instant::now();
    if deadline > now {
        thread::sleep(deadline - now);
    }
}

fn consecutive_known_record_count(
    records: &[RecordSnapshot],
    known_ids: &HashSet<String>,
) -> usize {
    records
        .iter()
        .rev()
        .take_while(|record| known_ids.contains(record.record_id.as_str()))
        .count()
}

fn status_text(status: &AutoPageStatus) -> String {
    let mut text = status.message.clone();
    if let (Some(current), Some(total)) = (status.current_page, status.total_pages) {
        text.push_str(&format!(" page={current}/{total}"));
    }
    if !status.technical_detail.is_empty() {
        text.push_str(": ");
        text.push_str(&status.technical_detail);
    }
    text
}

fn record_pool(record: &RecordSnapshot) -> Option<String> {
    if record.pool_id == "CardPool_Character" {
        return Some("limited".to_string());
    }
    if record.pool_id == "CardPool_NewRole" {
        return Some("standard".to_string());
    }
    if record.record_type == "fork" || record.pool_id.starts_with("ForkLottery_") {
        return Some("fork".to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_pool_maps_capture_records_to_workflow_pools() {
        assert_eq!(
            record_pool(&RecordSnapshot {
                record_id: "a".to_string(),
                pool_id: "CardPool_Character".to_string(),
                record_type: "monopoly".to_string(),
            }),
            Some("limited".to_string())
        );
        assert_eq!(
            record_pool(&RecordSnapshot {
                record_id: "b".to_string(),
                pool_id: "CardPool_NewRole".to_string(),
                record_type: "monopoly".to_string(),
            }),
            Some("standard".to_string())
        );
        assert_eq!(
            record_pool(&RecordSnapshot {
                record_id: "c".to_string(),
                pool_id: "ForkLottery_AnHunQu".to_string(),
                record_type: "fork".to_string(),
            }),
            Some("fork".to_string())
        );
    }

    #[test]
    fn consecutive_known_record_count_only_counts_latest_run() {
        let records = vec![
            snapshot("new", "CardPool_Character", "monopoly"),
            snapshot("old-1", "CardPool_Character", "monopoly"),
            snapshot("old-2", "CardPool_Character", "monopoly"),
        ];
        let known_ids = ["old-1".to_string(), "old-2".to_string()]
            .into_iter()
            .collect::<HashSet<_>>();

        assert_eq!(consecutive_known_record_count(&records, &known_ids), 2);
    }

    #[test]
    fn consecutive_known_record_count_stops_at_latest_unknown() {
        let records = vec![
            snapshot("old-1", "CardPool_Character", "monopoly"),
            snapshot("new", "CardPool_Character", "monopoly"),
        ];
        let known_ids = ["old-1".to_string()].into_iter().collect::<HashSet<_>>();

        assert_eq!(consecutive_known_record_count(&records, &known_ids), 0);
    }

    fn snapshot(record_id: &str, pool_id: &str, record_type: &str) -> RecordSnapshot {
        RecordSnapshot {
            record_id: record_id.to_string(),
            pool_id: pool_id.to_string(),
            record_type: record_type.to_string(),
        }
    }
}
