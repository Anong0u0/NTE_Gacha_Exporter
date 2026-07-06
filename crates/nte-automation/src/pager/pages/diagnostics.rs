impl AutoPager {
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
        self.record_raw_page_failure(page_rect);
        self.record_context_failure(page_rect);
    }

    fn record_raw_page_failure(&mut self, page_rect: crate::model::Rect) {
        if self.diagnostics.raw_page_png.is_some() {
            return;
        }
        match self.capture_rect_png(page_rect) {
            Ok(png) => self.diagnostics.raw_page_png = Some(png),
            Err(error) => {
                self.diagnostics.visual.context_error =
                    Some(format!("raw page capture failed: {error}"));
            }
        }
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

    fn capture_rect_png(&mut self, rect: crate::model::Rect) -> AutomationResult<Vec<u8>> {
        use std::io::Cursor;

        use image::{DynamicImage, ImageFormat};

        self.focus_window()?;
        let image = self.capture.capture_rect(rect)?;
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image).write_to(&mut cursor, ImageFormat::Png)?;
        Ok(cursor.into_inner())
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
            let status = event.to_status(
                self.started_at.elapsed().as_secs_f64(),
                &self.options.labels,
            );
            self.tooltip.show(&status_text(&status));
            callback(status);
        } else {
            let status = event.to_status(
                self.started_at.elapsed().as_secs_f64(),
                &self.options.labels,
            );
            self.tooltip.show(&status_text(&status));
        }
    }

    fn sleep_poll(&self) {
        thread::sleep(Duration::from_secs_f64(self.options.click_poll_interval));
    }

    fn click_verify_wait(&self) -> Duration {
        Duration::from_secs_f64(self.options.click_timeout)
    }

    fn template_verify_wait(&self) -> Duration {
        Duration::from_secs_f64(self.options.template_timeout)
    }

    fn post_click_template_wait(&self) -> Duration {
        Duration::from_secs_f64(self.options.template_timeout)
    }
}
