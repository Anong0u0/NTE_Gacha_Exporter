#[derive(Debug, Clone, Copy, Default)]
pub struct PageReadHint {
    pub previous_current: Option<u32>,
    pub expected_current: Option<u32>,
    pub expected_total: Option<u32>,
}

impl From<PageReadHint> for PageReadHintDiagnostics {
    fn from(value: PageReadHint) -> Self {
        Self {
            previous_current: value.previous_current,
            expected_current: value.expected_current,
            expected_total: value.expected_total,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PageNumberReader {
    templates: Vec<GlyphTemplate>,
    template_indices_by_char: HashMap<char, Vec<usize>>,
}

impl Default for PageNumberReader {
    fn default() -> Self {
        let templates = load_digit_templates();
        Self {
            template_indices_by_char: index_templates_by_char(&templates),
            templates,
        }
    }
}

impl PageNumberReader {
    pub fn read_page_number(&self, image: &RgbaImage) -> AutomationResult<PageNumber> {
        self.read_page_number_with_hint(image, PageReadHint::default())
    }

    pub fn read_page_number_with_hint(
        &self,
        image: &RgbaImage,
        hint: PageReadHint,
    ) -> AutomationResult<PageNumber> {
        self.read_page_number_with_hint_diagnostics(image, hint).0
    }

    pub fn read_page_number_with_hint_diagnostics(
        &self,
        image: &RgbaImage,
        hint: PageReadHint,
    ) -> (AutomationResult<PageNumber>, OcrReadDiagnostics) {
        let mut attempts = Vec::new();
        let mut candidate_scores = HashMap::<String, CandidateAggregate>::new();
        let mut seen_targets = HashSet::new();
        let mut scored_pass_count = 0_usize;

        for (candidate_index, threshold) in THRESHOLDS.into_iter().enumerate() {
            let size = Size {
                width: image.width(),
                height: image.height(),
            };
            let pass = format!("threshold:{threshold}");
            let mask = threshold_white_text(image, threshold);
            let Some(target) = target_from_mask(&mask, image, threshold) else {
                attempts.push(sequence_attempt_error(
                    candidate_index,
                    size,
                    pass,
                    None,
                    "page text segmentation found 0 pixels",
                ));
                continue;
            };

            let signature = (
                target.left,
                target.top,
                target.width,
                target.height,
                target.text_height,
                target.component_count,
                target.estimated_char_count,
            );
            if !seen_targets.insert(signature) {
                continue;
            }

            match self.score_target(&target, hint) {
                Ok(scored) => {
                    scored_pass_count += 1;
                    attempts.push(sequence_attempt(
                        candidate_index,
                        size,
                        &target,
                        &scored.best,
                        scored.second.as_ref(),
                        None,
                    ));
                    let winning_text = scored.best.page.text.clone();
                    for candidate in scored.ranked {
                        let won_pass = candidate.page.text == winning_text;
                        merge_candidate_score(&mut candidate_scores, candidate, won_pass);
                    }
                }
                Err(error) => attempts.push(sequence_attempt_error(
                    candidate_index,
                    size,
                    pass,
                    Some(target.char_count_hint()),
                    error.to_string(),
                )),
            }
        }

        let mut pages = candidate_scores
            .into_values()
            .map(|candidate| candidate.into_page_candidate(scored_pass_count))
            .collect::<Vec<_>>();
        pages.sort_by(compare_page_candidates);

        if let Some(best) = pages.first().cloned() {
            let second = pages
                .iter()
                .find(|candidate| candidate.page.text != best.page.text);
            let margin = second.map_or(best.score, |candidate| best.score - candidate.score);
            if let Some(expected) = expected_candidate(&pages, hint) {
                let expected_gap = best.score - expected.score;
                if expected.score >= EXPECTED_MIN_SCORE && expected_gap <= EXPECTED_TIE_BREAK_MARGIN {
                    return (
                        Ok(expected.page.clone()),
                        page_read_diagnostics(hint, attempts, String::new()),
                    );
                }
            }
            if best.score < MIN_DECODE_SCORE {
                let error = format!(
                    "page number scores below threshold: best={} score={:.3}",
                    best.page.text, best.score
                );
                return (
                    Err(AutomationError::message(&error)),
                    page_read_diagnostics(hint, attempts, error),
                );
            }
            if hinted_expected_text(hint)
                .is_some_and(|expected_text| expected_text != best.page.text)
                && (best.score < MIN_HINTED_UNEXPECTED_SCORE
                    || margin < MIN_HINTED_UNEXPECTED_MARGIN)
            {
                let error = format!(
                    "hinted page number conflict: best={} score={:.3} expected={} margin={:.3}",
                    best.page.text,
                    best.score,
                    hinted_expected_text(hint).unwrap_or_else(|| "none".to_string()),
                    margin
                );
                return (
                    Err(AutomationError::message(&error)),
                    page_read_diagnostics(hint, attempts, error),
                );
            }
            if margin < MIN_UNHINTED_MARGIN {
                let error = format!(
                    "ambiguous page number: best={} score={:.3} second={} score={:.3}",
                    best.page.text,
                    best.score,
                    second
                        .map(|candidate| candidate.page.text.as_str())
                        .unwrap_or("none"),
                    second.map_or(0.0, |candidate| candidate.score)
                );
                return (
                    Err(AutomationError::message(&error)),
                    page_read_diagnostics(hint, attempts, error),
                );
            }
            return (
                Ok(best.page),
                page_read_diagnostics(hint, attempts, String::new()),
            );
        }

        let error = format!(
            "cannot read page number: {}",
            attempts
                .iter()
                .filter_map(|attempt| attempt.error.as_deref())
                .collect::<Vec<_>>()
                .join("; ")
        );
        (
            Err(AutomationError::message(&error)),
            page_read_diagnostics(hint, attempts, error),
        )
    }

    fn score_target(&self, target: &TargetMask, hint: PageReadHint) -> AutomationResult<PassScore> {
        let pass = format!("threshold:{}", target.threshold);
        let mut candidates_by_text = HashMap::<String, PageCandidate>::new();
        for slots in self.slot_sequences(target) {
            let slot_scores = slots
                .iter()
                .map(|slot| self.classify_glyph(target, *slot))
                .collect::<Vec<_>>();
            if slot_scores.iter().any(Vec::is_empty) {
                continue;
            }
            for page in self.page_candidates_for_len(slots.len(), hint) {
                let Some(score) = score_text_lattice(&page.text, &slot_scores) else {
                    continue;
                };
                merge_best_candidate(
                    &mut candidates_by_text,
                    PageCandidate {
                        page,
                        score,
                        pass: format!("{pass}:slots:{}", slots.len()),
                    },
                );
            }
        }

        let mut candidates = candidates_by_text.into_values().collect::<Vec<_>>();
        candidates.sort_by(compare_page_candidates);
        let Some(best) = candidates.first().cloned() else {
            return Err(AutomationError::message("no legal page candidates matched"));
        };
        let second = candidates
            .iter()
            .find(|candidate| candidate.page.text != best.page.text)
            .cloned();
        let ranked = candidates
            .iter()
            .take(AGGREGATE_TOP_CANDIDATES)
            .cloned()
            .collect();
        Ok(PassScore {
            best,
            second,
            ranked,
        })
    }

    fn page_candidates_for_len(&self, text_len: usize, hint: PageReadHint) -> Vec<PageNumber> {
        let mut pages = Vec::new();
        if let Some(total) = hint.expected_total {
            for current in 1..=total {
                self.push_len_page_candidate(&mut pages, current, total, text_len);
            }
            return pages;
        }

        for total_digits in 1..=digit_count(MAX_TOTAL_PAGE) {
            let total_start = digit_floor(total_digits);
            let total_end = digit_ceil(total_digits).min(MAX_TOTAL_PAGE);
            for current_digits in 1..=total_digits {
                if (current_digits + total_digits + 1) as usize != text_len {
                    continue;
                }

                let current_start = digit_floor(current_digits);
                let current_cap = digit_ceil(current_digits);
                for total in total_start..=total_end {
                    let current_end = current_cap.min(total);
                    if current_start > current_end {
                        continue;
                    }
                    for current in current_start..=current_end {
                        self.push_len_page_candidate(&mut pages, current, total, text_len);
                    }
                }
            }
        }
        pages
    }

    fn push_len_page_candidate(&self, pages: &mut Vec<PageNumber>, current: u32, total: u32, text_len: usize) {
        let text = format!("{current}/{total}");
        if text.len() != text_len {
            return;
        }
        pages.push(PageNumber {
            current,
            total,
            text,
        });
    }

    fn slot_sequences(&self, target: &TargetMask) -> Vec<Vec<GlyphSlot>> {
        let mut sequences = vec![Vec::<GlyphSlot>::new()];
        for component in &target.components {
            let mut next = Vec::new();
            for split_count in component_split_counts(*component) {
                let slots = split_component_slots(*component, split_count, target.width, target.height);
                for sequence in &sequences {
                    let mut candidate = sequence.clone();
                    candidate.extend(slots.iter().copied());
                    if candidate.len() <= 7 {
                        next.push(candidate);
                    }
                }
            }
            sequences = next;
        }
        sequences.sort_by_key(|sequence| {
            (
                sequence.len().abs_diff(target.estimated_char_count),
                usize::MAX - sequence.len(),
            )
        });
        sequences.truncate(64);
        sequences
    }
}

fn component_split_counts(component: TextComponent) -> Vec<usize> {
    let ratio = component.width as f32 / component.height.max(1) as f32;
    if ratio >= 2.15 {
        vec![1, 2, 3]
    } else if ratio >= 1.05 {
        vec![1, 2]
    } else {
        vec![1]
    }
}

fn split_component_slots(
    component: TextComponent,
    split_count: usize,
    target_width: u32,
    target_height: u32,
) -> Vec<GlyphSlot> {
    let split_count = split_count.max(1) as u32;
    (0..split_count)
        .filter_map(|index| {
            let left = component.left + (component.width * index) / split_count;
            let right = if index + 1 == split_count {
                component.right() + 1
            } else {
                component.left + (component.width * (index + 1)) / split_count
            };
            if right <= left {
                return None;
            }
            Some(expand_slot(
                GlyphSlot {
                    left,
                    top: component.top,
                    width: right - left,
                    height: component.height,
                },
                target_width,
                target_height,
            ))
        })
        .collect()
}

fn expand_slot(slot: GlyphSlot, target_width: u32, target_height: u32) -> GlyphSlot {
    let left = slot.left.saturating_sub(1);
    let top = slot.top.saturating_sub(1);
    let right = (slot.left + slot.width + 1).min(target_width);
    let bottom = (slot.top + slot.height + 1).min(target_height);
    GlyphSlot {
        left,
        top,
        width: right.saturating_sub(left).max(1),
        height: bottom.saturating_sub(top).max(1),
    }
}

fn score_text_lattice(text: &str, slot_scores: &[GlyphScores]) -> Option<f32> {
    if text.chars().count() != slot_scores.len() {
        return None;
    }
    let mut scores = Vec::with_capacity(slot_scores.len());
    let mut margins = Vec::with_capacity(slot_scores.len());
    for (ch, scores_for_slot) in text.chars().zip(slot_scores) {
        let index = scores_for_slot
            .iter()
            .position(|candidate| candidate.ch == ch)?;
        let score = scores_for_slot[index].score;
        let second = scores_for_slot
            .iter()
            .enumerate()
            .filter(|(candidate_index, _)| *candidate_index != index)
            .map(|(_, candidate)| candidate.score)
            .next()
            .unwrap_or(0.0);
        scores.push(score);
        margins.push(score - second);
    }
    let average = scores.iter().sum::<f32>() / scores.len().max(1) as f32;
    let min_score = scores.iter().copied().fold(f32::INFINITY, f32::min);
    let average_margin = margins.iter().sum::<f32>() / margins.len().max(1) as f32;
    let margin_score = ((average_margin + 0.20) / 0.45).clamp(0.0, 1.0);
    Some(average * 0.76 + min_score * 0.18 + margin_score * 0.06)
}

fn expected_candidate(pages: &[PageCandidate], hint: PageReadHint) -> Option<&PageCandidate> {
    let expected = hint.expected_current?;
    pages.iter().find(|candidate| {
        candidate.page.current == expected
            && hint
                .expected_total
                .is_none_or(|total| candidate.page.total == total)
    })
}

fn hinted_expected_text(hint: PageReadHint) -> Option<String> {
    Some(format!(
        "{}/{}",
        hint.expected_current?,
        hint.expected_total?
    ))
}

fn compare_page_candidates(left: &PageCandidate, right: &PageCandidate) -> std::cmp::Ordering {
    right
        .score
        .total_cmp(&left.score)
        .then_with(|| left.page.current.cmp(&right.page.current))
        .then_with(|| left.page.total.cmp(&right.page.total))
}
