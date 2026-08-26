impl AutoPager {
    fn click_page_button(
        &mut self,
        request: PageClickRequest<'_>,
    ) -> AutomationResult<PageClickOutcome> {
        let previous = &request.previous;
        for attempt in 1..=MAX_PAGE_CLICK_ATTEMPTS {
            let clicked_at = Instant::now();
            self.click(request.point, Duration::ZERO, Some(0.0))?;
            match self.wait_for_page_transition(clicked_at, &request, attempt)? {
                PageWaitOutcome::Changed(fingerprint) => {
                    return Ok(PageClickOutcome::Changed(
                        previous.advance(request.expected_page, fingerprint),
                    ));
                }
                PageWaitOutcome::Unchanged if attempt < MAX_PAGE_CLICK_ATTEMPTS => {
                    self.diagnostics.recovery.click_retries += 1;
                    self.status(
                        StatusEvent::new("page did not change; retrying click", "retry")
                            .page_optional(previous.current, previous.total)
                            .technical_detail(&format!(
                                "attempt={}/{}",
                                attempt + 1,
                                MAX_PAGE_CLICK_ATTEMPTS
                            )),
                    );
                }
                PageWaitOutcome::Unchanged => {
                    self.diagnostics.recovery.transition_timeouts += 1;
                    let (code, message) = if previous.total.is_some() {
                        (
                            "page_transition_not_observed",
                            format!(
                                "auto page stopped after page {}; the next page did not appear",
                                previous.current
                            ),
                        )
                    } else {
                        (
                            "end_unconfirmed",
                            format!(
                                "auto page stopped after page {}; the end of the record list could not be confirmed",
                                previous.current
                            ),
                        )
                    };
                    return Ok(PageClickOutcome::Partial(AutoPageStopReason {
                        code: code.to_string(),
                        message,
                    }));
                }
                PageWaitOutcome::Indeterminate => {
                    self.diagnostics.recovery.transition_timeouts += 1;
                    return Ok(PageClickOutcome::Partial(AutoPageStopReason {
                        code: "page_transition_indeterminate".to_string(),
                        message: format!(
                            "auto page stopped safely after page {}; page transition could not be confirmed",
                            previous.current
                        ),
                    }));
                }
            }
        }
        unreachable!("page click attempts are exhaustive")
    }

    fn settle_after_page_click(&self, clicked_at: Instant) {
        sleep_until(clicked_at + self.options.page_record_min_wait);
    }

    fn wait_for_page_transition(
        &mut self,
        clicked_at: Instant,
        request: &PageClickRequest<'_>,
        click_attempt: u32,
    ) -> AutomationResult<PageWaitOutcome> {
        let page_rect = request.page_rect;
        let pool = request.pool;
        let previous = &request.previous;
        let expected_page = request.expected_page;
        let deadline = Instant::now() + self.click_verify_wait();
        let mut transition = PageTransitionTracker::new();
        let mut last_observation = None::<PageObservation>;
        let mut candidate_pending = false;

        while Instant::now() < deadline {
            if self.should_stop() {
                return Err(AutomationError::message("auto page stopped"));
            }
            if candidate_pending {
                self.settle_after_page_click(clicked_at);
            } else {
                self.sleep_poll();
            }
            candidate_pending = false;
            let observation = self.observe_page(
                page_rect,
                PageReadConstraint {
                    known_total: previous.total,
                },
            )?;
            let Some(fingerprint) = observation.fingerprint.as_ref() else {
                transition.observe_missing();
                last_observation = Some(observation);
                continue;
            };
            match transition.observe(&previous.fingerprint, fingerprint) {
                PageTransitionProgress::Waiting => {}
                PageTransitionProgress::Candidate => candidate_pending = true,
                PageTransitionProgress::Confirmed => {
                    self.record_page_ocr_validation(&observation, expected_page, previous.total);
                    self.record_page_transition(PageTransitionDiagnostics {
                        pool: pool.to_string(),
                        from_page: previous.current,
                        expected_page,
                        click_attempt,
                        current_distance: transition.current_distance,
                        suffix_distance: transition.suffix_distance,
                        stable: true,
                        accepted: true,
                        observed_current: observation.current.confident_value(),
                        observed_total: observation.total.confident_value(),
                    });
                    return Ok(PageWaitOutcome::Changed(fingerprint.clone()));
                }
            }
            last_observation = Some(observation);
        }

        let (observed_current, observed_total) = last_observation.as_ref().map_or(
            (None, None),
            |observation| {
                (
                    observation.current.confident_value(),
                    observation.total.confident_value(),
                )
            },
        );
        self.record_page_transition(PageTransitionDiagnostics {
            pool: pool.to_string(),
            from_page: previous.current,
            expected_page,
            click_attempt,
            current_distance: transition.current_distance,
            suffix_distance: transition.suffix_distance,
            stable: transition.finish() == PageTransitionTerminal::Unchanged,
            accepted: false,
            observed_current,
            observed_total,
        });
        Ok(match transition.finish() {
            PageTransitionTerminal::Unchanged => PageWaitOutcome::Unchanged,
            PageTransitionTerminal::Indeterminate => PageWaitOutcome::Indeterminate,
        })
    }

    fn record_page_ocr_validation(
        &mut self,
        observation: &PageObservation,
        expected_page: u32,
        known_total: Option<u32>,
    ) {
        match page_ocr_validation(observation, expected_page, known_total) {
            PageOcrValidation::Matches => {}
            PageOcrValidation::Mismatch => self.diagnostics.recovery.ocr_mismatches += 1,
            PageOcrValidation::Ambiguous => self.diagnostics.recovery.ocr_ambiguous += 1,
        }
    }

    fn wait_for_fresh_page(
        &mut self,
        page_rect: crate::model::Rect,
        pool: &str,
    ) -> AutomationResult<PageCursor> {
        let deadline = Instant::now() + self.template_verify_wait();
        let mut stable_since = None::<Instant>;
        let mut previous_fingerprint = None::<PageFingerprint>;
        let mut stable_total = StablePageTotal::default();
        while Instant::now() < deadline {
            if self.should_stop() {
                return Err(AutomationError::message("auto page stopped"));
            }
            let observation = self.observe_page(page_rect, PageReadConstraint::default())?;
            let Some(fingerprint) = observation.fingerprint.as_ref() else {
                stable_since = None;
                previous_fingerprint = None;
                stable_total.reset();
                self.sleep_poll();
                continue;
            };
            let now = Instant::now();
            let stable = previous_fingerprint.as_ref().is_some_and(|previous| {
                previous.current_distance(fingerprint) <= PAGE_FINGERPRINT_STABLE_DISTANCE
                    && previous.suffix_distance(fingerprint) <= PAGE_FINGERPRINT_STABLE_DISTANCE
            });
            stable_since = Some(if stable {
                stable_since.unwrap_or(now)
            } else {
                stable_total.reset();
                now
            });
            stable_total.observe(observation.total.confident_value(), 1);
            previous_fingerprint = Some(fingerprint.clone());
            if stable_since.is_some_and(|since| now.duration_since(since) >= FRESH_PAGE_STABLE_WAIT)
            {
                let total = stable_total.confirmed();
                self.record_page_ocr_validation(&observation, 1, total);
                return Ok(PageCursor {
                    current: 1,
                    total,
                    fingerprint: fingerprint.clone(),
                });
            }
            self.sleep_poll();
        }
        self.record_page_number_failure("fresh_page_fingerprint_unavailable", page_rect);
        Err(AutomationError::message(format!(
            "{pool}: page indicator did not become stable"
        )))
    }

    fn click(
        &mut self,
        point: Point,
        foreground_settle: Duration,
        settle: Option<f64>,
    ) -> AutomationResult<()> {
        let click = window::foreground_click(&self.window, point, foreground_settle)?;
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

    fn observe_template(&mut self, name: &str) -> AutomationResult<Option<TemplateMatch>> {
        self.focus_window()?;
        let search_rect = self.matcher.search_rect(name, self.window.client_size())?;
        let image = self.capture.capture_rect(search_rect)?;
        let matched = self.matcher.observe_in_rect(name, &image, search_rect)?;
        if let Some(matched) = &matched {
            self.record_template_match(matched.clone());
        }
        Ok(matched)
    }

    fn observe_page(
        &mut self,
        page_rect: crate::model::Rect,
        constraint: PageReadConstraint,
    ) -> AutomationResult<PageObservation> {
        self.focus_window()?;
        let image = self.capture.capture_rect(page_rect)?;
        let (observation, diagnostics) = self
            .page_reader
            .observe_page_number(&image, constraint)?;
        self.diagnostics.ocr = Some(diagnostics);
        Ok(observation)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PageOcrValidation {
    Matches,
    Mismatch,
    Ambiguous,
}

fn page_ocr_validation(
    observation: &PageObservation,
    expected_page: u32,
    known_total: Option<u32>,
) -> PageOcrValidation {
    let Some(current) = observation.current.confident_value() else {
        return PageOcrValidation::Ambiguous;
    };
    if current != expected_page {
        return PageOcrValidation::Mismatch;
    }
    let Some(total) = observation.total.confident_value() else {
        return PageOcrValidation::Ambiguous;
    };
    if total < expected_page || known_total.is_some_and(|known_total| total != known_total) {
        PageOcrValidation::Mismatch
    } else {
        PageOcrValidation::Matches
    }
}
