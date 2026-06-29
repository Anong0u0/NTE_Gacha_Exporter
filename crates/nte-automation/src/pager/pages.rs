impl AutoPager {
    fn capture_pages(&mut self, step: &WorkflowStep) -> AutomationResult<PoolPageRun> {
        let pool = required(step.pool.as_deref(), "pool")?.to_string();
        let page_rect = *self
            .profile
            .rects
            .get(required(step.page_rect.as_deref(), "pageRect")?)
            .ok_or_else(|| AutomationError::message("workflow pageRect missing from profile"))?;
        let next_button = self.point(required(step.next_button.as_deref(), "nextButton")?)?;
        self.diagnostics.visual.pool = Some(pool.clone());
        self.diagnostics.visual.page_rect = Some(page_rect);
        self.diagnostics.visual.next_button = Some(next_button);
        let mut page = self.wait_for_fresh_page(page_rect, &pool)?;
        self.status(
            StatusEvent::new("page ready", "page")
                .step(&step.status)
                .pool(&pool)
                .page(page.current, page.total),
        );
        let mut visited_pages = 1_u32;
        if self.should_skip_pool(&pool, &step.status, &page)
            || self.apply_capture_control(&pool, &step.status, &page, visited_pages)?
        {
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
                StatusEvent::new("page next", "page")
                    .step(&step.status)
                    .pool(&pool)
                    .page(expected, page.total),
            );
            if self.apply_capture_control(&pool, &step.status, &page, expected)? {
                return Ok(PoolPageRun {
                    pool,
                    skipped: true,
                    visited_pages,
                    last_page: page.current,
                });
            }
            match self.click_page_button(PageClickRequest {
                page_rect,
                point: next_button,
                pool: &pool,
                step: &step.status,
                previous: page,
                expected_page: expected,
                visited_pages,
            })? {
                PageClickOutcome::Changed(next_page) => {
                    page = next_page;
                    visited_pages = page.current;
                    if self.should_skip_pool(&pool, &step.status, &page)
                        || self.apply_capture_control(&pool, &step.status, &page, visited_pages)?
                    {
                        return Ok(PoolPageRun {
                            pool,
                            skipped: true,
                            visited_pages,
                            last_page: page.current,
                        });
                    }
                }
                PageClickOutcome::SkipPool(last_page) => {
                    return Ok(PoolPageRun {
                        pool,
                        skipped: true,
                        visited_pages,
                        last_page: last_page.current,
                    });
                }
            }
        }
        self.status(
            StatusEvent::new("pool completed", "pool_completed")
                .step(&step.status)
                .pool(&pool)
                .page(page.total, page.total),
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
            || self.options.known_record_keys.is_empty()
            || self.options.record_snapshot.is_none()
        {
            return false;
        }
        let known_counts = record_key_counts(&self.options.known_record_keys);
        let deadline =
            Instant::now() + Duration::from_secs_f64(self.options.duplicate_check_timeout);
        while Instant::now() <= deadline {
            let pool_records = self.pool_records(pool);
            let duplicate_count = consecutive_known_record_count(&pool_records, &known_counts);
            if duplicate_count >= AUTO_PAGE_INCREMENTAL_DUPLICATE_RECORD_THRESHOLD {
                self.status(
                    StatusEvent::new("known records found; skipping pool", "pool_skipped")
                        .step(step)
                        .pool(pool)
                        .page(page.current, page.total)
                        .technical_detail(&format!("duplicate_records={duplicate_count}")),
                );
                return true;
            }
            if pool_records
                .last()
                .is_some_and(|record| !known_counts.contains_key(record.record_key.as_str()))
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

    fn apply_capture_control(
        &mut self,
        pool: &str,
        step: &str,
        page: &PageNumber,
        visited_pages: u32,
    ) -> AutomationResult<bool> {
        let mut wait_started = None::<Instant>;
        let mut last_wait_state = None::<(usize, u32)>;
        loop {
            match self.capture_control_decision(pool, step, page, visited_pages) {
                AutoPageControlDecision::Continue => return Ok(false),
                AutoPageControlDecision::SkipPool { duplicate_records } => {
                    self.status(
                        StatusEvent::new("known records found; skipping pool", "pool_skipped")
                            .step(step)
                            .pool(pool)
                            .page(page.current, page.total)
                            .technical_detail(&format!("duplicate_records={duplicate_records}")),
                    );
                    return Ok(true);
                }
                AutoPageControlDecision::WaitCapture {
                    decoded_pages,
                    max_visited_pages,
                } => {
                    if self.should_stop() {
                        return Err(AutomationError::message("auto page stopped"));
                    }
                    let wait_state = (decoded_pages, max_visited_pages);
                    if last_wait_state != Some(wait_state) {
                        last_wait_state = Some(wait_state);
                        wait_started = Some(Instant::now());
                    }
                    if wait_started
                        .is_some_and(|started| started.elapsed().as_secs_f64() >= self.options.click_timeout)
                    {
                        return Err(AutomationError::message(format!(
                            "capture window stalled: pool={pool} visited_pages={visited_pages} decoded_pages={decoded_pages} max_visited_pages={max_visited_pages}"
                        )));
                    }
                    self.status(
                        StatusEvent::new("capture window waiting", "diagnostic")
                            .pool(pool)
                            .page(page.current, page.total)
                            .technical_detail(&format!(
                                "decoded_pages={decoded_pages} max_visited_pages={max_visited_pages}"
                            ))
                            .persistent(),
                    );
                    self.sleep_poll();
                }
            }
        }
    }

    fn capture_control_decision(
        &self,
        pool: &str,
        step: &str,
        page: &PageNumber,
        visited_pages: u32,
    ) -> AutoPageControlDecision {
        let Some(callback) = &self.options.control else {
            return AutoPageControlDecision::Continue;
        };
        callback(AutoPageControlContext {
            pool: pool.to_string(),
            step: step.to_string(),
            current_page: page.current,
            total_pages: page.total,
            visited_pages,
        })
    }

    fn click_page_button(
        &mut self,
        request: PageClickRequest<'_>,
    ) -> AutomationResult<PageClickOutcome> {
        let previous = request.previous;
        for attempt in 1..=2 {
            let clicked_at = Instant::now();
            let click = window::foreground_click(&self.window, request.point)?;
            self.record_click(click);
            match self.wait_for_page(
                request.page_rect,
                request.pool,
                request.step,
                &previous,
                request.expected_page,
                request.visited_pages,
            )? {
                PageWaitOutcome::Changed(page) => {
                    self.settle_after_page_click(clicked_at);
                    return Ok(PageClickOutcome::Changed(page));
                }
                PageWaitOutcome::SkipPool => return Ok(PageClickOutcome::SkipPool(previous)),
                PageWaitOutcome::Unchanged => {}
            }
            if attempt < 2 {
                self.status(
                    StatusEvent::new("page did not change; retrying click", "retry")
                        .page(previous.current, previous.total)
                        .technical_detail(&format!("attempt={}/2", attempt + 1)),
                );
            }
        }
        Err(AutomationError::message(format!(
            "page did not change after retry: expected {}, still {}",
            request.expected_page,
            previous.current
        )))
    }

    fn settle_after_page_click(&self, clicked_at: Instant) {
        sleep_until(clicked_at + self.options.page_record_min_wait);
    }

    fn wait_for_page(
        &mut self,
        page_rect: crate::model::Rect,
        pool: &str,
        step: &str,
        previous: &PageNumber,
        expected_page: u32,
        visited_pages: u32,
    ) -> AutomationResult<PageWaitOutcome> {
        let deadline = Instant::now() + Duration::from_secs_f64(self.options.click_timeout);
        let mut last_error = None;
        let mut saw_previous = false;
        let mut unexpected_page = None::<PageNumber>;
        let mut unexpected_count = 0_u8;
        while Instant::now() < deadline {
            if self.should_stop() {
                return Err(AutomationError::message("auto page stopped"));
            }
            match self.capture_control_decision(pool, step, previous, visited_pages) {
                AutoPageControlDecision::Continue => {}
                AutoPageControlDecision::SkipPool { duplicate_records } => {
                    self.status(
                        StatusEvent::new("known records found; skipping pool", "pool_skipped")
                            .step(step)
                            .pool(pool)
                            .page(previous.current, previous.total)
                            .technical_detail(&format!("duplicate_records={duplicate_records}")),
                    );
                    return Ok(PageWaitOutcome::SkipPool);
                }
                AutoPageControlDecision::WaitCapture {
                    decoded_pages,
                    max_visited_pages,
                } => {
                    if Instant::now() >= deadline {
                        return Err(AutomationError::message(format!(
                            "capture window stalled: pool={pool} visited_pages={visited_pages} decoded_pages={decoded_pages} max_visited_pages={max_visited_pages}"
                        )));
                    }
                    self.status(
                        StatusEvent::new("capture window waiting", "diagnostic")
                            .pool(pool)
                            .page(previous.current, previous.total)
                            .technical_detail(&format!(
                                "decoded_pages={decoded_pages} max_visited_pages={max_visited_pages}"
                            ))
                            .persistent(),
                    );
                    self.sleep_poll();
                    continue;
                }
            }
            self.sleep_poll();
            match self.read_page_with_hint(
                page_rect,
                PageReadHint {
                    previous_current: Some(previous.current),
                    expected_current: Some(expected_page),
                    expected_total: Some(previous.total),
                },
            ) {
                Ok(page) if page.current == expected_page => {
                    return Ok(PageWaitOutcome::Changed(page));
                }
                Ok(page) if page.current == previous.current => saw_previous = true,
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
                StatusEvent::new("page number waiting ended", "diagnostic")
                    .technical_detail(&error.to_string()),
            );
            if !saw_previous {
                self.record_page_number_failure("page_number_unreadable_after_click", page_rect);
                return Err(AutomationError::message(format!(
                    "page number unreadable after click: {error}"
                )));
            }
        }
        if let Some(page) = unexpected_page {
            return Err(AutomationError::message(format!(
                "unexpected page after click: {}/{}",
                page.current, page.total
            )));
        }
        Ok(PageWaitOutcome::Unchanged)
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
        self.record_page_number_failure("fresh_page_number_unreadable", page_rect);
        Err(AutomationError::message(format!(
            "{pool}: freshly opened record page unreadable: {}",
            last_error
                .map(|error| error.to_string())
                .unwrap_or_else(|| "timeout".to_string())
        )))
    }

    fn click(&mut self, point: Point, settle: Option<f64>) -> AutomationResult<()> {
        let click = window::foreground_click(&self.window, point)?;
        self.record_click(click);
        thread::sleep(Duration::from_secs_f64(settle.unwrap_or(0.1)));
        Ok(())
    }

    fn try_template(&mut self, name: &str) -> AutomationResult<TemplateMatch> {
        self.focus_window()?;
        let search_rect = self.matcher.search_rect(name, self.window.client_size())?;
        let image = self.capture.capture_rect(search_rect)?;
        let matched = self.matcher.verify_in_rect(name, &image, search_rect)?;
        self.record_template_match(matched.clone());
        Ok(matched)
    }

    fn find_template(&mut self, name: &str) -> AutomationResult<TemplateMatch> {
        self.focus_window()?;
        let search_rect = self.matcher.search_rect(name, self.window.client_size())?;
        let image = self.capture.capture_rect(search_rect)?;
        let matched = self.matcher.find_in_rect(name, &image, search_rect)?;
        self.record_template_match(matched.clone());
        Ok(matched)
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
        let (result, diagnostics) = self
            .page_reader
            .read_page_number_with_hint_diagnostics(&image, hint);
        self.diagnostics.ocr = Some(diagnostics);
        result
    }

    fn record_template_match(&mut self, matched: TemplateMatch) {
        const MAX_TEMPLATE_MATCHES: usize = 8;
        self.diagnostics.visual.last_template_matches.push(matched);
        let extra = self
            .diagnostics
            .visual
            .last_template_matches
            .len()
            .saturating_sub(MAX_TEMPLATE_MATCHES);
        if extra > 0 {
            self.diagnostics
                .visual
                .last_template_matches
                .drain(0..extra);
        }
    }

    fn record_click(&mut self, click: MouseClickDiagnostics) {
        self.diagnostics.input.mouse_buttons_swapped = Some(click.mouse_buttons_swapped);
        self.diagnostics.input.last_click = Some(click);
    }

    fn record_template_failure(&mut self, name: &str) {
        self.diagnostics.failure_kind = Some("template_not_found".to_string());
        if self.diagnostics.context_png.is_some() {
            return;
        }
        match self.matcher.search_rect(name, self.window.client_size()) {
            Ok(search_rect) => {
                self.diagnostics.visual.template_search_rect = Some(search_rect);
                self.record_context_failure(search_rect);
            }
            Err(error) => {
                self.diagnostics.visual.context_error =
                    Some(format!("template search rect unavailable: {error}"));
            }
        }
    }

    fn record_page_number_failure(&mut self, failure_kind: &str, page_rect: crate::model::Rect) {
        self.diagnostics.failure_kind = Some(failure_kind.to_string());
        self.diagnostics.visual.page_rect = Some(page_rect);
        self.record_context_failure(page_rect);
    }

    fn record_context_failure(&mut self, highlight_rect: crate::model::Rect) {
        if self.diagnostics.context_png.is_some() {
            return;
        }
        match self.capture_context_png(highlight_rect) {
            Ok((context_rect, png)) => {
                self.diagnostics.visual.context_rect = Some(context_rect);
                self.diagnostics.context_png = Some(png);
            }
            Err(error) => {
                self.diagnostics.visual.context_error =
                    Some(format!("context capture failed: {error}"));
            }
        }
    }

    fn capture_context_png(
        &mut self,
        highlight_rect: crate::model::Rect,
    ) -> AutomationResult<(crate::model::Rect, Vec<u8>)> {
        use std::io::Cursor;

        use image::{DynamicImage, ImageFormat, Rgba};

        self.focus_window()?;
        let client_size = self.window.client_size();
        let cursor_position = window::current_cursor_client_position(&self.window).ok();
        self.diagnostics.visual.cursor_client_position = cursor_position;
        let cursor_for_context = cursor_position.filter(|point| point_in_size(*point, client_size));
        let context_rect = page_context_rect(highlight_rect, client_size, cursor_for_context);
        let mut image = self.capture.capture_rect(context_rect)?;
        self.diagnostics.visual.cursor_in_context =
            cursor_position.map(|point| rect_contains_point(context_rect, point));
        let overlay = crate::model::Rect {
            x: highlight_rect.x - context_rect.x,
            y: highlight_rect.y - context_rect.y,
            width: highlight_rect.width,
            height: highlight_rect.height,
        };
        draw_rect_outline(&mut image, overlay, Rgba([255, 0, 0, 255]), 2);
        if let Some(cursor) = cursor_position.filter(|point| rect_contains_point(context_rect, *point)) {
            draw_cursor_marker(
                &mut image,
                Point {
                    x: cursor.x - context_rect.x,
                    y: cursor.y - context_rect.y,
                },
                Rgba([0, 220, 255, 255]),
            );
        }
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image).write_to(&mut cursor, ImageFormat::Png)?;
        Ok((context_rect, cursor.into_inner()))
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

    fn status(&self, event: StatusEvent<'_>) {
        if let Some(callback) = &self.options.on_status {
            let status = event.to_status(self.started_at.elapsed().as_secs_f64());
            self.tooltip.show(&status_text(&status));
            callback(status);
        } else {
            let status = event.to_status(self.started_at.elapsed().as_secs_f64());
            self.tooltip.show(&status_text(&status));
        }
    }

    fn sleep_poll(&self) {
        thread::sleep(Duration::from_secs_f64(self.options.click_poll_interval));
    }
}

const CURSOR_CONTEXT_PADDING: i32 = 48;
const CURSOR_MARKER_RADIUS: i32 = 4;
const CURSOR_MARKER_ARM: i32 = 12;

fn page_context_rect(
    page_rect: crate::model::Rect,
    client_size: Size,
    cursor: Option<Point>,
) -> crate::model::Rect {
    let base = page_rect
        .expand(Point {
            x: page_rect.width as i32,
            y: (page_rect.height / 2) as i32,
        });
    cursor
        .map(|point| union_rect(base, point_context_rect(point, CURSOR_CONTEXT_PADDING)))
        .unwrap_or(base)
        .clamp(client_size)
}

fn point_context_rect(point: Point, padding: i32) -> crate::model::Rect {
    let padding = padding.max(0);
    let size = (padding as u32).saturating_mul(2).saturating_add(1);
    crate::model::Rect {
        x: point.x - padding,
        y: point.y - padding,
        width: size,
        height: size,
    }
}

fn union_rect(left: crate::model::Rect, right: crate::model::Rect) -> crate::model::Rect {
    let x = left.x.min(right.x);
    let y = left.y.min(right.y);
    let right_edge = left.right().max(right.right());
    let bottom_edge = left.bottom().max(right.bottom());
    crate::model::Rect {
        x,
        y,
        width: (right_edge - x).max(1) as u32,
        height: (bottom_edge - y).max(1) as u32,
    }
}

fn point_in_size(point: Point, size: Size) -> bool {
    point.x >= 0 && point.y >= 0 && point.x < size.width as i32 && point.y < size.height as i32
}

fn rect_contains_point(rect: crate::model::Rect, point: Point) -> bool {
    point.x >= rect.x && point.y >= rect.y && point.x < rect.right() && point.y < rect.bottom()
}

fn draw_rect_outline(
    image: &mut image::RgbaImage,
    rect: crate::model::Rect,
    color: image::Rgba<u8>,
    thickness: u32,
) {
    let width = image.width() as i32;
    let height = image.height() as i32;
    let left = rect.x.clamp(0, width.saturating_sub(1));
    let top = rect.y.clamp(0, height.saturating_sub(1));
    let right = rect.right().saturating_sub(1).clamp(0, width.saturating_sub(1));
    let bottom = rect
        .bottom()
        .saturating_sub(1)
        .clamp(0, height.saturating_sub(1));
    let thickness = thickness.max(1) as i32;
    for offset in 0..thickness {
        let l = (left - offset).clamp(0, width.saturating_sub(1));
        let t = (top - offset).clamp(0, height.saturating_sub(1));
        let r = (right + offset).clamp(0, width.saturating_sub(1));
        let b = (bottom + offset).clamp(0, height.saturating_sub(1));
        for x in l..=r {
            image.put_pixel(x as u32, t as u32, color);
            image.put_pixel(x as u32, b as u32, color);
        }
        for y in t..=b {
            image.put_pixel(l as u32, y as u32, color);
            image.put_pixel(r as u32, y as u32, color);
        }
    }
}

fn draw_cursor_marker(image: &mut image::RgbaImage, point: Point, color: image::Rgba<u8>) {
    for y in -CURSOR_MARKER_RADIUS..=CURSOR_MARKER_RADIUS {
        for x in -CURSOR_MARKER_RADIUS..=CURSOR_MARKER_RADIUS {
            if x * x + y * y <= CURSOR_MARKER_RADIUS * CURSOR_MARKER_RADIUS {
                put_pixel_if_in_bounds(image, point.x + x, point.y + y, color);
            }
        }
    }
    for offset in -CURSOR_MARKER_ARM..=CURSOR_MARKER_ARM {
        put_pixel_if_in_bounds(image, point.x + offset, point.y, color);
        put_pixel_if_in_bounds(image, point.x, point.y + offset, color);
    }
}

fn put_pixel_if_in_bounds(
    image: &mut image::RgbaImage,
    x: i32,
    y: i32,
    color: image::Rgba<u8>,
) {
    if x >= 0 && y >= 0 && x < image.width() as i32 && y < image.height() as i32 {
        image.put_pixel(x as u32, y as u32, color);
    }
}
