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

}
