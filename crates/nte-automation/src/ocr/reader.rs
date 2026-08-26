#[derive(Debug, Clone, Copy)]
struct NumberPath {
    value: u32,
    digits: u8,
    score_sum: f32,
    min_score: f32,
    margin_sum: f32,
}

impl NumberPath {
    fn score(self) -> f32 {
        let digits = self.digits.max(1) as f32;
        let average = self.score_sum / digits;
        let average_margin = self.margin_sum / digits;
        let margin_score = ((average_margin + 0.20) / 0.45).clamp(0.0, 1.0);
        average * 0.76 + self.min_score * 0.18 + margin_score * 0.06
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
    pub fn observe_page_number(
        &self,
        image: &RgbaImage,
        constraint: PageReadConstraint,
    ) -> AutomationResult<(PageObservation, OcrReadDiagnostics)> {
        if image.width() == 0 || image.height() == 0 {
            return Err(AutomationError::message("page number image is empty"));
        }

        let targets = unique_targets(image);
        let slash_candidates = self.slash_candidates(&targets);
        let mut current_candidates = HashMap::<u32, FieldCandidate>::new();
        let mut total_candidates = HashMap::<u32, FieldCandidate>::new();

        if let Some(best_slash) = slash_candidates.first() {
            for slash in slash_candidates
                .iter()
                .filter(|candidate| same_slash_anchor(&targets, **candidate, *best_slash))
            {
                let target = &targets[slash.target_index];
                let (current_components, total_components) = field_components(target, slash.slot);
                merge_field_candidates(
                    &mut current_candidates,
                    self.score_field(target, &current_components),
                );
                merge_field_candidates(
                    &mut total_candidates,
                    self.score_field(target, &total_components),
                );
            }
        }

        let current = number_observation(current_candidates);
        let total = number_observation(total_candidates);
        let slash = slash_observation(&slash_candidates);
        let best_slash = slash_candidates.first().copied();
        let slash_diagnostic = best_slash.map(|candidate| {
            let target = &targets[candidate.target_index];
            (candidate.score, absolute_slot_rect(target, candidate.slot))
        });
        let segmentation = best_slash.map(|candidate| {
            segmentation_diagnostics(&targets[candidate.target_index], candidate.slot)
        });
        let fingerprint = (slash == ObservationState::Confident)
            .then_some(best_slash)
            .flatten()
            .map(|candidate| {
                page_fingerprint(image, &targets[candidate.target_index], candidate.slot)
            });
        let error = observation_error(constraint, slash, &current, &total);
        let observation = PageObservation {
            slash,
            current,
            total,
            fingerprint,
        };
        let diagnostics = page_read_diagnostics(
            constraint,
            slash_diagnostic,
            segmentation,
            &observation.current,
            &observation.total,
            error,
        );
        Ok((observation, diagnostics))
    }

    pub fn read_page_number(&self, image: &RgbaImage) -> AutomationResult<PageNumber> {
        self.read_page_number_with_constraint(image, PageReadConstraint::default())
    }

    pub fn read_page_number_with_constraint(
        &self,
        image: &RgbaImage,
        constraint: PageReadConstraint,
    ) -> AutomationResult<PageNumber> {
        self.read_page_number_with_constraint_diagnostics(image, constraint)
            .0
    }

    pub fn read_page_number_with_constraint_diagnostics(
        &self,
        image: &RgbaImage,
        constraint: PageReadConstraint,
    ) -> (AutomationResult<PageNumber>, OcrReadDiagnostics) {
        match self.observe_page_number(image, constraint) {
            Ok((observation, diagnostics)) => {
                let result = observation_to_page(&observation, constraint);
                (result, diagnostics)
            }
            Err(error) => {
                let observation = missing_page_observation();
                let diagnostics = page_read_diagnostics(
                    constraint,
                    None,
                    None,
                    &observation.current,
                    &observation.total,
                    error.to_string(),
                );
                (Err(error), diagnostics)
            }
        }
    }

    fn slash_candidates(&self, targets: &[TargetMask]) -> Vec<SlashCandidate> {
        let mut candidates = Vec::new();
        for (target_index, target) in targets.iter().enumerate() {
            let mut slots = HashSet::new();
            for component in &target.components {
                for split_count in component_split_counts(*component) {
                    slots.extend(split_component_slots(
                        *component,
                        split_count,
                        target.width,
                        target.height,
                    ));
                }
            }
            for slot in slots {
                if !has_fields_around(target, slot) {
                    continue;
                }
                let score = self.score_glyph(target, slot, '/');
                if score < MIN_GLYPH_SCORE {
                    continue;
                }
                let candidate = SlashCandidate {
                    target_index,
                    slot,
                    score,
                };
                candidates.push(candidate);
            }
        }
        candidates.sort_by(|left, right| right.score.total_cmp(&left.score));
        candidates.truncate(32);
        candidates
    }

    fn score_field(
        &self,
        target: &TargetMask,
        components: &[TextComponent],
    ) -> Vec<FieldCandidate> {
        let mut by_value = HashMap::<u32, FieldCandidate>::new();
        for slots in slot_sequences(components, target.width, target.height, MAX_FIELD_DIGITS) {
            let slot_scores = slots
                .iter()
                .map(|slot| {
                    self.classify_glyph(target, *slot)
                        .into_iter()
                        .filter(|candidate| candidate.ch.is_ascii_digit())
                        .collect::<GlyphScores>()
                })
                .collect::<Vec<_>>();
            if slot_scores.iter().any(Vec::is_empty) {
                continue;
            }
            for candidate in number_candidates(&slot_scores) {
                by_value
                    .entry(candidate.value)
                    .and_modify(|current| {
                        if candidate.score > current.score {
                            *current = candidate;
                        }
                    })
                    .or_insert(candidate);
            }
        }
        by_value.into_values().collect()
    }
}

fn same_slash_anchor(
    targets: &[TargetMask],
    candidate: SlashCandidate,
    reference: SlashCandidate,
) -> bool {
    let candidate = absolute_slot_rect(&targets[candidate.target_index], candidate.slot);
    let reference = absolute_slot_rect(&targets[reference.target_index], reference.slot);
    let candidate_center_x = candidate.x.saturating_mul(2) + candidate.width as i32;
    let reference_center_x = reference.x.saturating_mul(2) + reference.width as i32;
    let candidate_center_y = candidate.y.saturating_mul(2) + candidate.height as i32;
    let reference_center_y = reference.y.saturating_mul(2) + reference.height as i32;
    candidate_center_x.abs_diff(reference_center_x) <= 4
        && candidate_center_y.abs_diff(reference_center_y) <= 4
        && candidate.width.abs_diff(reference.width) <= 3
        && candidate.height.abs_diff(reference.height) <= 3
}

fn unique_targets(image: &RgbaImage) -> Vec<TargetMask> {
    let mut targets = Vec::new();
    let mut seen = HashSet::new();
    for threshold in THRESHOLDS {
        let mask = threshold_white_text(image, threshold);
        let Some(target) = target_from_mask(&mask, image) else {
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
        if seen.insert(signature) {
            targets.push(target);
        }
    }
    targets
}

fn has_fields_around(target: &TargetMask, slot: GlyphSlot) -> bool {
    let slot_center = slot.left.saturating_mul(2) + slot.width;
    let has_left = target
        .components
        .iter()
        .any(|component| component.left.saturating_mul(2) + component.width < slot_center);
    let has_right = target
        .components
        .iter()
        .any(|component| component.left.saturating_mul(2) + component.width > slot_center);
    has_left && has_right
}

fn field_components(
    target: &TargetMask,
    slash: GlyphSlot,
) -> (Vec<TextComponent>, Vec<TextComponent>) {
    let slash_center = slash.left.saturating_mul(2) + slash.width;
    let mut current = target
        .components
        .iter()
        .copied()
        .filter(|component| component.left.saturating_mul(2) + component.width < slash_center)
        .collect::<Vec<_>>();
    let mut total = target
        .components
        .iter()
        .copied()
        .filter(|component| component.left.saturating_mul(2) + component.width > slash_center)
        .collect::<Vec<_>>();
    current.sort_by_key(|component| (component.left, component.top));
    total.sort_by_key(|component| (component.left, component.top));
    (current, total)
}

fn slot_sequences(
    components: &[TextComponent],
    target_width: u32,
    target_height: u32,
    max_len: usize,
) -> Vec<Vec<GlyphSlot>> {
    let mut sequences = vec![Vec::<GlyphSlot>::new()];
    for component in components {
        let mut next = Vec::new();
        for split_count in component_split_counts(*component) {
            let slots = split_component_slots(
                *component,
                split_count,
                target_width,
                target_height,
            );
            for sequence in &sequences {
                let mut candidate = sequence.clone();
                candidate.extend(slots.iter().copied());
                if candidate.len() <= max_len {
                    next.push(candidate);
                }
            }
        }
        sequences = next;
    }
    sequences.sort_by_key(|sequence| usize::MAX - sequence.len());
    sequences.dedup();
    sequences.truncate(64);
    sequences
}

fn number_candidates(slot_scores: &[GlyphScores]) -> Vec<FieldCandidate> {
    if slot_scores.is_empty() || slot_scores.iter().any(Vec::is_empty) {
        return Vec::new();
    }
    let capacity = slot_scores
        .iter()
        .map(Vec::len)
        .fold(1_usize, usize::saturating_mul);
    let mut current = Vec::with_capacity(capacity);
    let mut next = Vec::with_capacity(capacity);
    current.push(NumberPath {
        value: 0,
        digits: 0,
        score_sum: 0.0,
        min_score: f32::INFINITY,
        margin_sum: 0.0,
    });
    for scores in slot_scores {
        next.clear();
        for path in &current {
            for (index, candidate) in scores.iter().enumerate() {
                let Some(digit) = candidate.ch.to_digit(10) else {
                    continue;
                };
                if path.digits == 0 && digit == 0 {
                    continue;
                }
                let Some(value) = path
                    .value
                    .checked_mul(10)
                    .and_then(|value| value.checked_add(digit))
                    .filter(|value| *value <= MAX_TOTAL_PAGE)
                else {
                    continue;
                };
                let second = scores
                    .iter()
                    .enumerate()
                    .filter(|(candidate_index, _)| *candidate_index != index)
                    .map(|(_, candidate)| candidate.score)
                    .next()
                    .unwrap_or(0.0);
                next.push(NumberPath {
                    value,
                    digits: path.digits.saturating_add(1),
                    score_sum: path.score_sum + candidate.score,
                    min_score: path.min_score.min(candidate.score),
                    margin_sum: path.margin_sum + (candidate.score - second),
                });
            }
        }
        std::mem::swap(&mut current, &mut next);
    }
    current
        .into_iter()
        .map(|path| FieldCandidate {
            value: path.value,
            score: path.score(),
        })
        .collect()
}

fn merge_field_candidates(
    destination: &mut HashMap<u32, FieldCandidate>,
    candidates: Vec<FieldCandidate>,
) {
    for candidate in candidates {
        destination
            .entry(candidate.value)
            .and_modify(|current| {
                if candidate.score > current.score {
                    *current = candidate;
                }
            })
            .or_insert(candidate);
    }
}

fn number_observation(candidates: HashMap<u32, FieldCandidate>) -> NumberObservation {
    let mut candidates = candidates
        .into_values()
        .map(|candidate| NumberCandidate {
            value: candidate.value,
            score: candidate.score,
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.value.cmp(&right.value))
    });
    candidates.truncate(MAX_RANKED_CANDIDATES);
    let state = match candidates.as_slice() {
        [] => ObservationState::Missing,
        [best, rest @ ..]
            if best.score >= MIN_DECODE_SCORE
                && (best.score >= MIN_HIGH_CONFIDENCE_SCORE
                    || rest.first().is_none_or(|second| {
                        best.score - second.score >= MIN_UNHINTED_MARGIN
                    })) =>
        {
            ObservationState::Confident
        }
        _ => ObservationState::Ambiguous,
    };
    NumberObservation { state, candidates }
}

fn slash_observation(candidates: &[SlashCandidate]) -> ObservationState {
    let Some(best) = candidates.first() else {
        return ObservationState::Missing;
    };
    if best.score >= MIN_DECODE_SCORE {
        ObservationState::Confident
    } else {
        ObservationState::Ambiguous
    }
}

fn missing_number_observation() -> NumberObservation {
    NumberObservation {
        state: ObservationState::Missing,
        candidates: Vec::new(),
    }
}

fn missing_page_observation() -> PageObservation {
    PageObservation {
        slash: ObservationState::Missing,
        current: missing_number_observation(),
        total: missing_number_observation(),
        fingerprint: None,
    }
}

fn observation_error(
    constraint: PageReadConstraint,
    slash: ObservationState,
    current: &NumberObservation,
    total: &NumberObservation,
) -> String {
    if slash != ObservationState::Confident {
        return "page slash is not confident".to_string();
    }
    let (Some(current), Some(total)) = (current.confident_value(), total.confident_value()) else {
        return "page fields are not confident".to_string();
    };
    if current > total {
        return format!("page current exceeds total: {current}/{total}");
    }
    if constraint
        .known_total
        .is_some_and(|known_total| known_total != total)
    {
        return format!(
            "page total conflicts with known total: observed={total} known={}",
            constraint.known_total.unwrap()
        );
    }
    String::new()
}

fn observation_to_page(
    observation: &PageObservation,
    constraint: PageReadConstraint,
) -> AutomationResult<PageNumber> {
    if observation.slash != ObservationState::Confident {
        return Err(AutomationError::message("cannot locate page slash"));
    }
    let current = observation
        .current
        .confident_value()
        .ok_or_else(|| AutomationError::message("current page is ambiguous"))?;
    let total = observation
        .total
        .confident_value()
        .ok_or_else(|| AutomationError::message("total page is ambiguous"))?;
    if current > total {
        return Err(AutomationError::message(format!(
            "current page exceeds total: {current}/{total}"
        )));
    }
    if constraint
        .known_total
        .is_some_and(|known_total| known_total != total)
    {
        return Err(AutomationError::message(format!(
            "observed total {total} differs from known total {}",
            constraint.known_total.unwrap()
        )));
    }
    Ok(PageNumber {
        current,
        total,
        text: format!("{current}/{total}"),
    })
}

fn absolute_slot_rect(target: &TargetMask, slot: GlyphSlot) -> Rect {
    Rect {
        x: (target.left + slot.left) as i32,
        y: (target.top + slot.top) as i32,
        width: slot.width,
        height: slot.height,
    }
}

fn segmentation_diagnostics(
    target: &TargetMask,
    slash: GlyphSlot,
) -> OcrSegmentationDiagnostics {
    let (current, total) = field_components(target, slash);
    OcrSegmentationDiagnostics {
        target_rect: Rect {
            x: target.left as i32,
            y: target.top as i32,
            width: target.width,
            height: target.height,
        },
        current_components: current
            .into_iter()
            .map(|component| absolute_component_rect(target, component))
            .collect(),
        total_components: total
            .into_iter()
            .map(|component| absolute_component_rect(target, component))
            .collect(),
    }
}

fn absolute_component_rect(target: &TargetMask, component: TextComponent) -> Rect {
    Rect {
        x: (target.left + component.left) as i32,
        y: (target.top + component.top) as i32,
        width: component.width,
        height: component.height,
    }
}

fn page_fingerprint(
    image: &RgbaImage,
    target: &TargetMask,
    slash: GlyphSlot,
) -> PageFingerprint {
    let slash_left = (target.left + slash.left) as i32;
    let field_width = (image.width() / 2).max(1);
    let current_rect = Rect {
        x: slash_left - field_width as i32,
        y: 0,
        width: field_width,
        height: image.height(),
    };
    let suffix_rect = Rect {
        x: slash_left,
        y: 0,
        width: field_width,
        height: image.height(),
    };
    PageFingerprint {
        current_mask: canonical_mask(image, current_rect),
        suffix_mask: canonical_mask(image, suffix_rect),
    }
}

fn canonical_mask(image: &RgbaImage, rect: Rect) -> Vec<u8> {
    let mut output = vec![0_u8; (FINGERPRINT_WIDTH * FINGERPRINT_HEIGHT) as usize];
    if rect.width == 0 || rect.height == 0 {
        return output;
    }
    for y in 0..FINGERPRINT_HEIGHT {
        for x in 0..FINGERPRINT_WIDTH {
            let source_x = rect.x as f32
                + (x as f32 + 0.5) * rect.width as f32 / FINGERPRINT_WIDTH as f32
                - 0.5;
            let source_y = rect.y as f32
                + (y as f32 + 0.5) * rect.height as f32 / FINGERPRINT_HEIGHT as f32
                - 0.5;
            let index = (y * FINGERPRINT_WIDTH + x) as usize;
            output[index] =
                (bilinear_white_text_weight(image, source_x, source_y) * 255.0).round() as u8;
        }
    }
    output
}
fn bilinear_white_text_weight(image: &RgbaImage, x: f32, y: f32) -> f32 {
    let left = x.floor() as i32;
    let top = y.floor() as i32;
    let x_fraction = x - left as f32;
    let y_fraction = y - top as f32;
    let top_weight = interpolated_white_text_weight(image, left, top, x_fraction);
    let bottom_weight = interpolated_white_text_weight(image, left, top + 1, x_fraction);
    top_weight + (bottom_weight - top_weight) * y_fraction
}

fn interpolated_white_text_weight(image: &RgbaImage, x: i32, y: i32, fraction: f32) -> f32 {
    let left = white_text_weight_at(image, x, y);
    let right = white_text_weight_at(image, x + 1, y);
    left + (right - left) * fraction
}

fn white_text_weight_at(image: &RgbaImage, x: i32, y: i32) -> f32 {
    if x < 0 || y < 0 || x >= image.width() as i32 || y >= image.height() as i32 {
        return 0.0;
    }
    white_text_weight(image.get_pixel(x as u32, y as u32))
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
