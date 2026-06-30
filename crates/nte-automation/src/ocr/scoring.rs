fn scaled_len(value: u32, scale: f32) -> i32 {
    (value as f32 * scale).round().max(1.0) as i32
}

impl GlyphClassifier for PageNumberReader {
    fn classify_glyph(&self, target: &TargetMask, slot: GlyphSlot) -> GlyphScores {
        let mut scores = Vec::new();
        for (&ch, indices) in &self.template_indices_by_char {
            let mut best = 0.0_f32;
            for &index in indices {
                best = best.max(score_template_slot(&self.templates[index], target, slot));
            }
            if best >= MIN_GLYPH_SCORE {
                scores.push(GlyphCandidate { ch, score: best });
            }
        }
        scores.sort_by(|left, right| {
            right
                .score
                .total_cmp(&left.score)
                .then_with(|| left.ch.cmp(&right.ch))
        });
        scores.truncate(5);
        scores
    }
}

fn score_template_slot(template: &GlyphTemplate, target: &TargetMask, slot: GlyphSlot) -> f32 {
    if slot.width == 0 || slot.height == 0 {
        return 0.0;
    }
    let scale = slot.height as f32 / template.height.max(1) as f32;
    let scaled = scaled_template(template, scale);
    let base_x = slot.left as i32 + (slot.width as i32 - scaled.width as i32) / 2;
    let base_y = slot.top as i32 + (slot.height as i32 - scaled.height as i32) / 2;
    let mut best = 0.0_f32;
    for x_offset in SEQUENCE_X_OFFSETS {
        for y_offset in [-1_i32, 0, 1] {
            best = best.max(score_rendered_slot(
                target,
                slot,
                base_x + x_offset,
                base_y + y_offset,
                &scaled,
            ));
        }
    }
    best
}

fn score_rendered_slot(
    target: &TargetMask,
    slot: GlyphSlot,
    template_left: i32,
    template_top: i32,
    template: &ScaledTemplate,
) -> f32 {
    let mut rendered = vec![false; (slot.width * slot.height) as usize];
    for &(x, y) in &template.points {
        let slot_x = template_left + x as i32 - slot.left as i32;
        let slot_y = template_top + y as i32 - slot.top as i32;
        if slot_x >= 0 && slot_y >= 0 && slot_x < slot.width as i32 && slot_y < slot.height as i32 {
            rendered[(slot_y as u32 * slot.width + slot_x as u32) as usize] = true;
        }
    }

    let mut true_positive = 0.0_f32;
    let mut false_positive = 0.0_f32;
    let mut false_negative = 0.0_f32;
    let mut true_negative = 0.0_f32;
    for y in 0..slot.height {
        for x in 0..slot.width {
            let rendered = rendered[(y * slot.width + x) as usize];
            let target_weight =
                target_weight_at(target, slot.left as i32 + x as i32, slot.top as i32 + y as i32);
            match (rendered, target_weight) {
                (true, weight) => {
                    true_positive += weight;
                    false_positive += 1.0 - weight;
                }
                (false, weight) => {
                    false_negative += weight;
                    true_negative += 1.0 - weight;
                }
            }
        }
    }

    let rendered_mass = true_positive + false_positive;
    let target_mass = true_positive + false_negative;
    if rendered_mass == 0.0 || target_mass == 0.0 {
        return 0.0;
    }
    let dice = (2.0 * true_positive) / (rendered_mass + target_mass);
    let recall = true_positive / target_mass;
    let specificity_denominator = true_negative + false_positive;
    let specificity = if specificity_denominator == 0.0 {
        1.0
    } else {
        true_negative / specificity_denominator
    };
    let width_fit = 1.0
        - ((slot.width as f32 - template.width as f32).abs()
            / slot.width.max(template.width) as f32)
            .clamp(0.0, 1.0);
    dice * 0.58 + recall * 0.22 + specificity * 0.10 + width_fit * 0.10
}

fn scaled_template(template: &GlyphTemplate, scale: f32) -> ScaledTemplate {
    let width = scaled_len(template.width, scale).max(1) as u32;
    let height = scaled_len(template.height, scale).max(1) as u32;
    let mut points = Vec::new();
    for y in 0..height {
        for x in 0..width {
            if sample_mask(
                template.width,
                template.height,
                width,
                height,
                x,
                y,
                |sx, sy| template.get(sx, sy),
            ) {
                points.push((x, y));
            }
        }
    }
    ScaledTemplate {
        width,
        height,
        points,
    }
}

fn target_weight_at(target: &TargetMask, x: i32, y: i32) -> f32 {
    if x < 0 || y < 0 || x >= target.width as i32 || y >= target.height as i32 {
        return 0.0;
    }
    target.weights[(y as u32 * target.width + x as u32) as usize]
}

fn sample_mask(
    mask_width: u32,
    mask_height: u32,
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    get: impl Fn(u32, u32) -> bool,
) -> bool {
    if x >= width || y >= height {
        return false;
    }
    let sx = scaled_index(x, width, mask_width);
    let sy = scaled_index(y, height, mask_height);
    get(sx, sy)
}

fn scaled_index(value: u32, source: u32, target: u32) -> u32 {
    if source <= 1 || target <= 1 {
        return 0;
    }
    ((value as u64 * (target - 1) as u64) / (source - 1) as u64) as u32
}

fn load_digit_templates() -> Vec<GlyphTemplate> {
    DIGIT_TEMPLATE_BYTES
        .iter()
        .map(|(ch, bytes)| {
            let image = image::load_from_memory(bytes)
                .expect("bundled page digit template must decode")
                .to_rgba8();
            let (left, top, right, bottom) = template_alpha_bounds(&image)
                .expect("bundled page digit template must contain alpha mask");
            let width = right - left + 1;
            let height = bottom - top + 1;
            let mut data = Vec::with_capacity((width * height) as usize);
            for y in top..=bottom {
                for x in left..=right {
                    data.push(image.get_pixel(x, y)[3] >= 128);
                }
            }
            GlyphTemplate {
                ch: *ch,
                width,
                height,
                data,
            }
        })
        .collect()
}

fn template_alpha_bounds(image: &RgbaImage) -> Option<(u32, u32, u32, u32)> {
    let mut min_x = image.width();
    let mut min_y = image.height();
    let mut max_x = 0_u32;
    let mut max_y = 0_u32;
    let mut found = false;

    for y in 0..image.height() {
        for x in 0..image.width() {
            if image.get_pixel(x, y)[3] < 128 {
                continue;
            }
            found = true;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }

    found.then_some((min_x, min_y, max_x, max_y))
}

fn index_templates_by_char(templates: &[GlyphTemplate]) -> HashMap<char, Vec<usize>> {
    let mut indices = HashMap::<char, Vec<usize>>::new();
    for (index, template) in templates.iter().enumerate() {
        indices.entry(template.ch).or_default().push(index);
    }
    indices
}
