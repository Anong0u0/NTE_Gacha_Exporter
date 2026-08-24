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
            page_reader: PageNumberReader::default(),
            matcher: ImageTemplateMatcher::new(scaled_profile.clone()),
            profile: scaled_profile,
            started_at: Instant::now(),
            diagnostics: AutoPageDiagnostics {
                window: Some(AutoPageWindowDiagnostics {
                    hwnd: window.hwnd,
                    pid: window.pid,
                    class_name: window.class_name.clone(),
                    title: window.title.clone(),
                    client_size: window.client_size(),
                    profile_base_size: profile.base_client_size,
                }),
                ..AutoPageDiagnostics::default()
            },
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
                if let Some(settle) = step.settle {
                    thread::sleep(Duration::from_secs_f64(settle));
                }
                Ok(None)
            }
            "click" => {
                let point = self.point(required(step.point.as_deref(), "point")?)?;
                self.click(point, Duration::ZERO, step.settle)?;
                Ok(None)
            }
            "clickTemplate" => {
                self.click_template(step)?;
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
        let deadline = Instant::now() + self.template_verify_wait();
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
        self.record_template_failure(name);
        Err(AutomationError::message(format!(
            "screen template not found after wait: {name}{detail}"
        )))
    }

    fn click_template(&mut self, step: &WorkflowStep) -> AutomationResult<()> {
        self.click_template_transition(step, step.target_template.as_deref())
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
        let deadline = Instant::now() + self.post_click_template_wait();
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
                self.click(*point, Duration::ZERO, Some(settle))?;
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
        self.record_template_failure(template);
        let error = AutomationError::message(format!(
            "screen template not found after click sequence: {template}{detail}"
        ));
        Err(error)
    }

    fn click_template_until_template(&mut self, step: &WorkflowStep) -> AutomationResult<()> {
        let target_template = required(step.target_template.as_deref(), "targetTemplate")?;
        self.click_template_transition(step, Some(target_template))
    }

    fn click_template_transition(
        &mut self,
        step: &WorkflowStep,
        target_template: Option<&str>,
    ) -> AutomationResult<()> {
        let source_template = required(step.template.as_deref(), "template")?;
        let first_click_settle = step.first_click_settle.unwrap_or(0.0);
        let post_click_settle = step.settle.unwrap_or(0.1);
        let mut deadline = Instant::now() + self.template_verify_wait();
        let started = Instant::now();
        let mut state = TemplateTransitionState::default();
        let mut checks = 0_u32;
        let mut last_target_match = None;
        let mut last_click_point = None;
        let mut last_source_error = None;
        let mut last_target_error = None;

        while Instant::now() < deadline {
            if self.should_stop() {
                return Err(AutomationError::message("auto page stopped"));
            }
            checks += 1;

            let target_match = target_template.and_then(|target| {
                match self.observe_template(target) {
                    Ok(Some(matched)) => {
                        last_target_error = None;
                        last_target_match = Some(matched.clone());
                        Some(matched)
                    }
                    Ok(None) => {
                        last_target_error = None;
                        None
                    }
                    Err(error) => {
                        last_target_error = Some(error);
                        None
                    }
                }
            });

            let (source_match, source_presence) = if target_match.is_some() {
                (None, TemplatePresence::Unavailable)
            } else {
                match self.observe_template(source_template) {
                    Ok(Some(matched)) => {
                        last_source_error = None;
                        (Some(matched), TemplatePresence::Present)
                    }
                    Ok(None) => {
                        last_source_error = None;
                        (None, TemplatePresence::Absent)
                    }
                    Err(error) => {
                        last_source_error = Some(error);
                        (None, TemplatePresence::Unavailable)
                    }
                }
            };

            match state.observe(
                target_template.is_some(),
                target_match.is_some(),
                source_presence,
            ) {
                TemplateTransitionDecision::Complete(verification) => {
                    let verification_name = verification.name(target_template);
                    let click_text = last_click_point
                        .map(|point: Point| format!("{},{}", point.x, point.y))
                        .unwrap_or_else(|| "none".to_string());
                    let target_text = last_target_match.as_ref().map_or_else(
                        || "none".to_string(),
                        |matched| format!("{},{}", matched.point.x, matched.point.y),
                    );
                    let detail = format!(
                        "source={source_template} target={} target_at={target_text} click={click_text} first={first_click_settle:.2}s post={post_click_settle:.2}s clicks={} checks={checks} verify={verification_name} wait={:.2}s",
                        target_template.unwrap_or("none"),
                        state.clicks,
                        started.elapsed().as_secs_f64(),
                    );
                    self.diagnostics.visual.template_transition =
                        Some(TemplateTransitionDiagnostics {
                            source_template: source_template.to_string(),
                            target_template: target_template.map(str::to_string),
                            verification: verification_name,
                            click_point: last_click_point,
                            first_click_settle,
                            post_click_settle,
                            click_attempts: state.clicks,
                            checks,
                            verified: true,
                        });
                    self.status(
                        StatusEvent::new("template transition verified", "template")
                            .step(&step.status)
                            .technical_detail(&detail),
                    );
                    return Ok(());
                }
                TemplateTransitionDecision::ClickSource { first_attempt } => {
                    if !first_attempt && Instant::now() >= deadline {
                        break;
                    }
                    let matched = source_match.expect("source match required for click decision");
                    let foreground_settle = if first_attempt {
                        Duration::from_secs_f64(first_click_settle)
                    } else {
                        Duration::ZERO
                    };
                    let click_point = template_match_center(&matched);
                    self.click(click_point, foreground_settle, Some(post_click_settle))?;
                    state.clicked();
                    last_click_point = Some(click_point);
                    if first_attempt {
                        deadline = Instant::now() + self.post_click_template_wait();
                    }
                }
                TemplateTransitionDecision::Wait => self.sleep_poll(),
            }
        }

        if state.clicks == 0 {
            let detail = last_source_error
                .or(last_target_error)
                .map(|error| format!(": {error}"))
                .unwrap_or_default();
            self.record_template_failure(source_template);
            return Err(AutomationError::message(format!(
                "screen template not found after wait: {source_template}{detail}"
            )));
        }

        let verification = target_template
            .map(|target| format!("target:{target}"))
            .unwrap_or_else(|| "source_gone".to_string());
        let failure_template = target_template.unwrap_or(source_template);
        let failure_error = if target_template.is_some() {
            last_target_error.or(last_source_error)
        } else {
            last_source_error.or(last_target_error)
        };
        let detail = failure_error
            .map(|error| format!(": {error}"))
            .unwrap_or_default();
        self.diagnostics.visual.template_transition = Some(TemplateTransitionDiagnostics {
            source_template: source_template.to_string(),
            target_template: target_template.map(str::to_string),
            verification: verification.clone(),
            click_point: last_click_point,
            first_click_settle,
            post_click_settle,
            click_attempts: state.clicks,
            checks,
            verified: false,
        });
        self.record_template_transition_failure(failure_template);
        Err(AutomationError::message(format!(
            "screen did not transition after template click: {source_template}; verify={verification} clicks={} first={first_click_settle:.2}s post={post_click_settle:.2}s{detail}",
            state.clicks,
        )))
    }
}

const TEMPLATE_ABSENCE_STABLE_SAMPLES: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TemplateTransitionVerification {
    Target,
    SourceGone,
}

impl TemplateTransitionVerification {
    fn name(self, target_template: Option<&str>) -> String {
        match self {
            Self::Target => format!("target:{}", target_template.unwrap_or("unknown")),
            Self::SourceGone => "source_gone".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TemplatePresence {
    Present,
    Absent,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TemplateTransitionDecision {
    Complete(TemplateTransitionVerification),
    ClickSource { first_attempt: bool },
    Wait,
}

#[derive(Debug, Default)]
struct TemplateTransitionState {
    clicks: u32,
    source_absence_samples: u8,
}

impl TemplateTransitionState {
    fn observe(
        &mut self,
        target_required: bool,
        target_found: bool,
        source: TemplatePresence,
    ) -> TemplateTransitionDecision {
        if target_required && target_found {
            return TemplateTransitionDecision::Complete(TemplateTransitionVerification::Target);
        }
        match source {
            TemplatePresence::Present => {
                self.source_absence_samples = 0;
                TemplateTransitionDecision::ClickSource {
                    first_attempt: self.clicks == 0,
                }
            }
            TemplatePresence::Absent if !target_required && self.clicks > 0 => {
                self.source_absence_samples = self.source_absence_samples.saturating_add(1);
                if self.source_absence_samples >= TEMPLATE_ABSENCE_STABLE_SAMPLES {
                    TemplateTransitionDecision::Complete(
                        TemplateTransitionVerification::SourceGone,
                    )
                } else {
                    TemplateTransitionDecision::Wait
                }
            }
            TemplatePresence::Absent | TemplatePresence::Unavailable => {
                self.source_absence_samples = 0;
                TemplateTransitionDecision::Wait
            }
        }
    }

    fn clicked(&mut self) {
        self.clicks += 1;
        self.source_absence_samples = 0;
    }
}

fn template_match_center(matched: &TemplateMatch) -> Point {
    Point {
        x: matched.point.x + matched.size.width as i32 / 2,
        y: matched.point.y + matched.size.height as i32 / 2,
    }
}
