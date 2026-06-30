impl AutoPager {
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
                        self.record_page_number_failure("unexpected_page_after_click", page_rect);
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
            self.record_page_number_failure("unexpected_page_after_click", page_rect);
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
            self.record_page_number_failure("fresh_page_number_not_first", page_rect);
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
}
