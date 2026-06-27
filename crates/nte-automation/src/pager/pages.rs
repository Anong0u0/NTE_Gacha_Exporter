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
                StatusEvent::new("page next", "page")
                    .step(&step.status)
                    .pool(&pool)
                    .page(expected, page.total),
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
            if duplicate_count >= INCREMENTAL_DUPLICATE_RECORD_THRESHOLD {
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
                    StatusEvent::new("page did not change; retrying click", "retry")
                        .page(previous.current, previous.total)
                        .technical_detail(&format!("attempt={}/2", attempt + 1)),
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
                StatusEvent::new("capture lag waiting", "diagnostic")
                    .pool(pool)
                    .page(visited_pages, total_pages)
                    .technical_detail(&format!("decoded_pages={decoded} max_lag={max_lag}"))
                    .persistent(),
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
                StatusEvent::new("OCR waiting ended", "diagnostic")
                    .technical_detail(&error.to_string()),
            );
            if !saw_previous {
                self.record_page_number_failure("page_number_unreadable_after_click", page_rect);
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
        self.record_page_number_failure("fresh_page_number_unreadable", page_rect);
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
        let (result, diagnostics) = self.ocr.read_page_number_with_hint_diagnostics(&image, hint);
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

    fn record_page_number_failure(&mut self, failure_kind: &str, page_rect: crate::model::Rect) {
        self.diagnostics.failure_kind = Some(failure_kind.to_string());
        self.diagnostics.visual.page_rect = Some(page_rect);
        if self.diagnostics.page_context_png.is_some() {
            return;
        }
        match self.capture_page_context_png(page_rect) {
            Ok((context_rect, png)) => {
                self.diagnostics.visual.context_rect = Some(context_rect);
                self.diagnostics.page_context_png = Some(png);
            }
            Err(error) => {
                if let Some(ocr) = self.diagnostics.ocr.as_mut() {
                    if !ocr.error.is_empty() {
                        ocr.error.push_str("; ");
                    }
                    ocr.error
                        .push_str(&format!("page context capture failed: {error}"));
                } else {
                    self.diagnostics.ocr = Some(OcrReadDiagnostics {
                        error: format!("page context capture failed: {error}"),
                        ..OcrReadDiagnostics::default()
                    });
                }
            }
        }
    }

    fn capture_page_context_png(
        &mut self,
        page_rect: crate::model::Rect,
    ) -> AutomationResult<(crate::model::Rect, Vec<u8>)> {
        use std::io::Cursor;

        use image::{DynamicImage, ImageFormat, Rgba};

        self.focus_window()?;
        let context_rect = page_context_rect(page_rect, self.window.client_size());
        let mut image = self.capture.capture_rect(context_rect)?;
        let overlay = crate::model::Rect {
            x: page_rect.x - context_rect.x,
            y: page_rect.y - context_rect.y,
            width: page_rect.width,
            height: page_rect.height,
        };
        draw_rect_outline(&mut image, overlay, Rgba([255, 0, 0, 255]), 2);
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

fn page_context_rect(page_rect: crate::model::Rect, client_size: Size) -> crate::model::Rect {
    page_rect
        .expand(Point {
            x: page_rect.width as i32,
            y: (page_rect.height / 2) as i32,
        })
        .clamp(client_size)
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
