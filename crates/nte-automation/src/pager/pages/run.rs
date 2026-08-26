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
                .page_optional(page.current, page.total),
        );
        let mut visited_pages = 1_u32;
        self.visited_pages_by_pool.insert(pool.clone(), visited_pages);
        self.last_page_by_pool.insert(pool.clone(), page.current);
        if self.should_skip_pool(&pool, &step.status, &page)
            || self.apply_capture_control(&pool, &step.status, &page, visited_pages, false)?
        {
            return Ok(PoolPageRun {
                pool,
                skipped: true,
                visited_pages,
                last_page: page.current,
                partial_reason: None,
            });
        }
        while page.total.is_none_or(|total| page.current < total) {
            if self.should_stop() {
                return Err(AutomationError::message("auto page stopped"));
            }
            let expected = page.current + 1;
            self.status(
                StatusEvent::new("page next", "page")
                    .step(&step.status)
                    .pool(&pool)
                    .page_optional(expected, page.total),
            );
            if self.apply_capture_control(&pool, &step.status, &page, expected, false)? {
                return Ok(PoolPageRun {
                    pool,
                    skipped: true,
                    visited_pages,
                    last_page: page.current,
                    partial_reason: None,
                });
            }
            match self.click_page_button(PageClickRequest {
                page_rect,
                point: next_button,
                pool: &pool,
                previous: page.clone(),
                expected_page: expected,
            })? {
                PageClickOutcome::Changed(next_page) => {
                    page = next_page;
                    visited_pages = visited_pages.saturating_add(1);
                    self.visited_pages_by_pool
                        .insert(pool.clone(), visited_pages);
                    self.last_page_by_pool.insert(pool.clone(), page.current);
                    if self.should_skip_pool(&pool, &step.status, &page)
                        || self.apply_capture_control(
                            &pool,
                            &step.status,
                            &page,
                            visited_pages,
                            false,
                        )?
                    {
                        return Ok(PoolPageRun {
                            pool,
                            skipped: true,
                            visited_pages,
                            last_page: page.current,
                            partial_reason: None,
                        });
                    }
                }
                PageClickOutcome::Partial(reason) => {
                    return Ok(PoolPageRun {
                        pool,
                        skipped: false,
                        visited_pages,
                        last_page: page.current,
                        partial_reason: Some(reason),
                    });
                }
            }
        }
        self.complete_pool(&pool, &step.status, &page, visited_pages)?;
        Ok(PoolPageRun {
            pool,
            skipped: false,
            visited_pages,
            last_page: page.current,
            partial_reason: None,
        })
    }

    fn complete_pool(
        &mut self,
        pool: &str,
        step: &str,
        page: &PageCursor,
        visited_pages: u32,
    ) -> AutomationResult<()> {
        let _ = self.apply_capture_control(pool, step, page, visited_pages, true)?;
        self.status(
            StatusEvent::new("pool completed", "pool_completed")
                .step(step)
                .pool(pool)
                .page_optional(page.current, page.total),
        );
        Ok(())
    }

    fn should_skip_pool(&mut self, pool: &str, step: &str, page: &PageCursor) -> bool {
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
                        .page_optional(page.current, page.total)
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
        page: &PageCursor,
        visited_pages: u32,
        pool_complete: bool,
    ) -> AutomationResult<bool> {
        let mut wait_started = None::<Instant>;
        let mut last_wait_state = None::<(usize, u32)>;
        loop {
            match self.capture_control_decision(
                pool,
                step,
                page,
                visited_pages,
                pool_complete,
            ) {
                AutoPageControlDecision::Continue => return Ok(false),
                AutoPageControlDecision::SkipPool { duplicate_records } => {
                    self.status(
                        StatusEvent::new("known records found; skipping pool", "pool_skipped")
                            .step(step)
                            .pool(pool)
                            .page_optional(page.current, page.total)
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
                        return Err(AutomationError::message(capture_window_stalled_message(
                            pool,
                            visited_pages,
                            decoded_pages,
                            max_visited_pages,
                            pool_complete,
                        )));
                    }
                    self.status(
                        StatusEvent::new("capture window waiting", "diagnostic")
                            .pool(pool)
                            .page_optional(page.current, page.total)
                            .technical_detail(&capture_wait_detail(
                                decoded_pages,
                                max_visited_pages,
                                pool_complete,
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
        page: &PageCursor,
        visited_pages: u32,
        pool_complete: bool,
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
            pool_complete,
        })
    }

}

fn capture_window_stalled_message(
    pool: &str,
    visited_pages: u32,
    decoded_pages: usize,
    max_visited_pages: u32,
    pool_complete: bool,
) -> String {
    let phase = if pool_complete {
        " phase=pool_complete"
    } else {
        ""
    };
    format!(
        "capture window stalled: pool={pool}{phase} visited_pages={visited_pages} decoded_pages={decoded_pages} max_visited_pages={max_visited_pages}"
    )
}

fn capture_wait_detail(
    decoded_pages: usize,
    max_visited_pages: u32,
    pool_complete: bool,
) -> String {
    let phase = if pool_complete {
        "phase=pool_complete "
    } else {
        ""
    };
    format!(
        "{phase}decoded_pages={decoded_pages} max_visited_pages={max_visited_pages}"
    )
}
