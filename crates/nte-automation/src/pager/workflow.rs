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
            ocr: WindowsOcrClient::new("en-US"),
            matcher: ImageTemplateMatcher::new(scaled_profile.clone()),
            profile: scaled_profile,
            started_at: Instant::now(),
        };
        if let Some(reason) = pager.tooltip.unavailable_reason() {
            if reason != "disabled" {
                pager.status(
                    StatusEvent::new("tooltip unavailable", "diagnostic")
                        .technical_detail(reason)
                        .persistent(),
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
        self.status(StatusEvent::new("auto page started", "started").step("started"));
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
        self.status(StatusEvent::new("auto page completed", "completed").step("completed"));
        Ok(AutoPageResult::completed_with_pages(
            completed,
            skipped,
            visited_pages_by_pool,
            last_page_by_pool,
        ))
    }

    fn run_step(&mut self, step: &WorkflowStep) -> AutomationResult<Option<PoolPageRun>> {
        if !step.status.is_empty() {
            self.status(StatusEvent::new(&step.status, "step").step(&step.status));
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
            StatusEvent::new("template verified", "template")
                .step(step)
                .technical_detail(&format!(
                    "{name} edge={:.3} gray={:.3} at={},{} wait={:.2}s tries={attempts}",
                    matched.edge_score,
                    matched.gray_score,
                    matched.point.x,
                    matched.point.y,
                    started.elapsed().as_secs_f64()
                )),
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
                            StatusEvent::new("template verified", "template")
                                .step(&step.status)
                                .technical_detail(&format!(
                                "{template} edge={:.3} gray={:.3} at={},{} wait={:.2}s clicks={clicks}",
                                matched.edge_score,
                                matched.gray_score,
                                matched.point.x,
                                matched.point.y,
                                started.elapsed().as_secs_f64()
                            )),
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
                        StatusEvent::new("template verified", "template")
                            .step(&step.status)
                            .technical_detail(&format!(
                            "{source_detail}{target_template} edge={:.3} gray={:.3} at={},{} wait={:.2}s clicks={clicks}",
                            target.edge_score,
                            target.gray_score,
                            target.point.x,
                            target.point.y,
                            started.elapsed().as_secs_f64()
                        )),
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
}
