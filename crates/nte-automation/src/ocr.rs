use std::collections::{BTreeMap, HashSet};

use image::RgbaImage;

use crate::error::{AutomationError, AutomationResult};
use crate::model::{
    OcrAttemptDiagnostic, OcrReadDiagnostics, PageNumber, PageReadHintDiagnostics, Size,
};

const MIN_GLYPH_SCORE: f32 = 0.58;
const MIN_PAGE_SCORE: f32 = 0.62;
const MAX_GLYPH_CANDIDATES: usize = 3;
const THRESHOLDS: [u8; 5] = [135, 145, 155, 165, 175];

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
}

impl Default for PageNumberReader {
    fn default() -> Self {
        Self {
            templates: load_digit_templates(),
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
        let mut pages = Vec::new();

        for (candidate_index, binary) in page_number_candidates(image).into_iter().enumerate() {
            let size = Size {
                width: image.width(),
                height: image.height(),
            };
            match self.read_binary_candidate(&binary, hint) {
                Ok(candidate) => {
                    attempts.push(OcrAttemptDiagnostic {
                        candidate_index,
                        size,
                        text: Some(candidate.page.text.clone()),
                        score: Some(candidate.score),
                        glyph_count: Some(candidate.glyph_count),
                        error: None,
                    });
                    pages.push(candidate);
                }
                Err(error) => {
                    attempts.push(OcrAttemptDiagnostic {
                        candidate_index,
                        size,
                        text: None,
                        score: None,
                        glyph_count: Some(binary.glyph_count),
                        error: Some(error.to_string()),
                    });
                }
            }
        }

        pages.sort_by(|left, right| compare_page_candidates(left, right, hint));
        if let Some(best) = pages.into_iter().next() {
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

    fn read_binary_candidate(
        &self,
        binary: &BinaryPageCandidate,
        hint: PageReadHint,
    ) -> AutomationResult<PageCandidate> {
        if binary.glyphs.len() < 3 {
            return Err(AutomationError::message(format!(
                "page digit segmentation found {} glyphs",
                binary.glyphs.len()
            )));
        }

        let mut candidates = Vec::new();
        for glyph in &binary.glyphs {
            let glyph_candidates = self.classify_glyph(glyph);
            if glyph_candidates.is_empty() {
                return Err(AutomationError::message(format!(
                    "page glyph unreadable at {},{} {}x{}",
                    glyph.left, glyph.top, glyph.width, glyph.height
                )));
            }
            candidates.push(glyph_candidates);
        }

        decode_page_candidates(&candidates, hint)
            .into_iter()
            .filter(|candidate| candidate.score >= MIN_PAGE_SCORE)
            .max_by(|left, right| compare_page_candidates(left, right, hint).reverse())
            .ok_or_else(|| AutomationError::message("page digit scores below threshold"))
    }

    fn classify_glyph(&self, glyph: &GlyphMask) -> Vec<GlyphCandidate> {
        let mut best_by_char = BTreeMap::<char, f32>::new();
        for template in &self.templates {
            let score = glyph_score(glyph, template);
            best_by_char
                .entry(template.ch)
                .and_modify(|value| *value = value.max(score))
                .or_insert(score);
        }

        let mut candidates = best_by_char
            .into_iter()
            .filter(|(_, score)| *score >= MIN_GLYPH_SCORE)
            .map(|(ch, score)| GlyphCandidate { ch, score })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            right
                .score
                .total_cmp(&left.score)
                .then_with(|| left.ch.cmp(&right.ch))
        });
        candidates.truncate(MAX_GLYPH_CANDIDATES);
        candidates
    }
}

fn page_read_diagnostics(
    hint: PageReadHint,
    attempts: Vec<OcrAttemptDiagnostic>,
    error: impl Into<String>,
) -> OcrReadDiagnostics {
    OcrReadDiagnostics {
        hint: hint.into(),
        attempts,
        error: error.into(),
    }
}

fn compare_page_candidates(
    left: &PageCandidate,
    right: &PageCandidate,
    hint: PageReadHint,
) -> std::cmp::Ordering {
    page_hint_rank(&left.page, hint)
        .cmp(&page_hint_rank(&right.page, hint))
        .then_with(|| right.score.total_cmp(&left.score))
        .then_with(|| left.page.current.cmp(&right.page.current))
        .then_with(|| left.page.total.cmp(&right.page.total))
}

fn page_hint_rank(page: &PageNumber, hint: PageReadHint) -> u8 {
    let expected_current = Some(page.current) == hint.expected_current;
    let expected_total = hint.expected_total.is_none_or(|total| total == page.total);
    if expected_current && expected_total {
        return 0;
    }
    if Some(page.current) == hint.previous_current && expected_total {
        return 1;
    }
    if expected_total {
        return 2;
    }
    3
}

#[derive(Debug, Clone)]
struct PageCandidate {
    page: PageNumber,
    score: f32,
    glyph_count: usize,
}

#[derive(Debug, Clone)]
struct GlyphCandidate {
    ch: char,
    score: f32,
}

#[derive(Debug, Clone)]
struct BinaryPageCandidate {
    glyphs: Vec<GlyphMask>,
    glyph_count: usize,
}

#[derive(Debug, Clone)]
struct GlyphMask {
    left: u32,
    top: u32,
    width: u32,
    height: u32,
    data: Vec<bool>,
}

impl GlyphMask {
    fn get(&self, x: u32, y: u32) -> bool {
        self.data[(y * self.width + x) as usize]
    }
}

#[derive(Debug, Clone)]
struct GlyphTemplate {
    ch: char,
    width: u32,
    height: u32,
    data: Vec<bool>,
}

impl GlyphTemplate {
    fn get(&self, x: u32, y: u32) -> bool {
        self.data[(y * self.width + x) as usize]
    }
}

fn page_number_candidates(image: &RgbaImage) -> Vec<BinaryPageCandidate> {
    let mut candidates = Vec::new();
    let mut seen_signatures = HashSet::new();
    for threshold in THRESHOLDS {
        let mask = threshold_white_text(image, threshold);
        let glyphs = segment_glyphs(&mask, image.width(), image.height());
        let signature = glyphs
            .iter()
            .map(|glyph| (glyph.left, glyph.top, glyph.width, glyph.height))
            .collect::<Vec<_>>();
        if seen_signatures.insert(signature) {
            candidates.push(BinaryPageCandidate {
                glyph_count: glyphs.len(),
                glyphs,
            });
        }
    }
    candidates
}

fn threshold_white_text(image: &RgbaImage, threshold: u8) -> Vec<bool> {
    image
        .pixels()
        .map(|pixel| {
            let max_channel = pixel[0].max(pixel[1]).max(pixel[2]);
            let min_channel = pixel[0].min(pixel[1]).min(pixel[2]);
            let chroma = max_channel.saturating_sub(min_channel);
            let luma = (pixel[0] as f32 * 0.299 + pixel[1] as f32 * 0.587 + pixel[2] as f32 * 0.114)
                .round() as u8;
            luma >= threshold && chroma <= 58
        })
        .collect()
}

fn segment_glyphs(mask: &[bool], width: u32, height: u32) -> Vec<GlyphMask> {
    let mut visited = vec![false; mask.len()];
    let mut glyphs = Vec::new();

    for y in 2..height.saturating_sub(2) {
        for x in 2..width.saturating_sub(2) {
            let index = (y * width + x) as usize;
            if visited[index] || !mask[index] {
                continue;
            }
            let component = collect_component(mask, &mut visited, width, height, x, y);
            if let Some(glyph) = component_to_glyph(mask, width, height, &component) {
                glyphs.push(glyph);
            }
        }
    }

    glyphs.sort_by_key(|glyph| (glyph.left, glyph.top));
    glyphs
}

fn collect_component(
    mask: &[bool],
    visited: &mut [bool],
    width: u32,
    height: u32,
    start_x: u32,
    start_y: u32,
) -> Vec<(u32, u32)> {
    let mut stack = vec![(start_x, start_y)];
    let mut component = Vec::new();
    visited[(start_y * width + start_x) as usize] = true;

    while let Some((x, y)) = stack.pop() {
        component.push((x, y));
        let left = x.saturating_sub(1);
        let top = y.saturating_sub(1);
        let right = (x + 1).min(width.saturating_sub(1));
        let bottom = (y + 1).min(height.saturating_sub(1));
        for next_y in top..=bottom {
            for next_x in left..=right {
                let index = (next_y * width + next_x) as usize;
                if !visited[index] && mask[index] {
                    visited[index] = true;
                    stack.push((next_x, next_y));
                }
            }
        }
    }

    component
}

fn component_to_glyph(
    mask: &[bool],
    width: u32,
    height: u32,
    component: &[(u32, u32)],
) -> Option<GlyphMask> {
    let min_x = component.iter().map(|(x, _)| *x).min()?;
    let max_x = component.iter().map(|(x, _)| *x).max()?;
    let min_y = component.iter().map(|(_, y)| *y).min()?;
    let max_y = component.iter().map(|(_, y)| *y).max()?;
    let glyph_width = max_x - min_x + 1;
    let glyph_height = max_y - min_y + 1;
    let area = component.len() as u32;

    if area < 20 {
        return None;
    }
    if glyph_height < height / 5 || glyph_height > height * 3 / 5 {
        return None;
    }
    if glyph_width > width / 3 {
        return None;
    }

    let pad = 2_u32;
    let left = min_x.saturating_sub(pad);
    let top = min_y.saturating_sub(pad);
    let right = (max_x + pad).min(width.saturating_sub(1));
    let bottom = (max_y + pad).min(height.saturating_sub(1));
    let out_width = right - left + 1;
    let out_height = bottom - top + 1;
    let mut data = Vec::with_capacity((out_width * out_height) as usize);
    for y in top..=bottom {
        for x in left..=right {
            data.push(mask[(y * width + x) as usize]);
        }
    }

    Some(GlyphMask {
        left,
        top,
        width: out_width,
        height: out_height,
        data,
    })
}

fn decode_page_candidates(
    glyphs: &[Vec<GlyphCandidate>],
    hint: PageReadHint,
) -> Vec<PageCandidate> {
    let mut out = Vec::new();
    let mut current = String::new();
    decode_recursive(glyphs, 0, &mut current, 1.0, &mut out, hint);
    out
}

fn decode_recursive(
    glyphs: &[Vec<GlyphCandidate>],
    index: usize,
    current: &mut String,
    score_sum: f32,
    out: &mut Vec<PageCandidate>,
    hint: PageReadHint,
) {
    if index == glyphs.len() {
        if let Some(page) = parse_candidate_text(current, hint) {
            out.push(PageCandidate {
                page,
                score: score_sum / glyphs.len().max(1) as f32,
                glyph_count: glyphs.len(),
            });
        }
        return;
    }

    for candidate in &glyphs[index] {
        current.push(candidate.ch);
        decode_recursive(
            glyphs,
            index + 1,
            current,
            score_sum + candidate.score,
            out,
            hint,
        );
        current.pop();
    }
}

fn parse_candidate_text(text: &str, hint: PageReadHint) -> Option<PageNumber> {
    let (current_text, total_text) = text.split_once('/')?;
    if current_text.is_empty() || total_text.is_empty() || total_text.contains('/') {
        return None;
    }
    let current = current_text.parse::<u32>().ok()?;
    let total = total_text.parse::<u32>().ok()?;
    if current == 0 || total == 0 || current > total {
        return None;
    }
    if hint
        .expected_total
        .is_some_and(|expected| expected != total)
    {
        return None;
    }
    Some(PageNumber {
        current,
        total,
        text: text.to_string(),
    })
}

fn glyph_score(glyph: &GlyphMask, template: &GlyphTemplate) -> f32 {
    let width = glyph.width.max(template.width);
    let height = glyph.height.max(template.height);
    let mut best = 0.0_f32;

    for dx in -1..=1 {
        for dy in -1..=1 {
            let score = dice_score(glyph, template, width, height, dx, dy);
            best = best.max(score);
        }
    }

    best
}

fn dice_score(
    glyph: &GlyphMask,
    template: &GlyphTemplate,
    width: u32,
    height: u32,
    dx: i32,
    dy: i32,
) -> f32 {
    let mut intersection = 0_u32;
    let mut glyph_count = 0_u32;
    let mut template_count = 0_u32;

    for y in 0..height {
        for x in 0..width {
            let glyph_on = sample_glyph(glyph, x, y, width, height);
            let template_on =
                sample_template(template, x as i32 - dx, y as i32 - dy, width, height);
            if glyph_on {
                glyph_count += 1;
            }
            if template_on {
                template_count += 1;
            }
            if glyph_on && template_on {
                intersection += 1;
            }
        }
    }

    if glyph_count == 0 || template_count == 0 {
        return 0.0;
    }
    (2 * intersection) as f32 / (glyph_count + template_count) as f32
}

fn sample_glyph(glyph: &GlyphMask, x: u32, y: u32, width: u32, height: u32) -> bool {
    sample_mask(glyph.width, glyph.height, width, height, x, y, |sx, sy| {
        glyph.get(sx, sy)
    })
}

fn sample_template(template: &GlyphTemplate, x: i32, y: i32, width: u32, height: u32) -> bool {
    if x < 0 || y < 0 {
        return false;
    }
    sample_mask(
        template.width,
        template.height,
        width,
        height,
        x as u32,
        y as u32,
        |sx, sy| template.get(sx, sy),
    )
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
            GlyphTemplate {
                ch: *ch,
                width: image.width(),
                height: image.height(),
                data: image.pixels().map(|pixel| pixel[3] >= 128).collect(),
            }
        })
        .collect()
}

const DIGIT_TEMPLATE_BYTES: &[(char, &[u8])] = &[
    ('0', include_bytes!("../assets/page_digits/0_0.png")),
    ('0', include_bytes!("../assets/page_digits/0_1.png")),
    ('1', include_bytes!("../assets/page_digits/1_0.png")),
    ('1', include_bytes!("../assets/page_digits/1_1.png")),
    ('2', include_bytes!("../assets/page_digits/2_0.png")),
    ('2', include_bytes!("../assets/page_digits/2_1.png")),
    ('3', include_bytes!("../assets/page_digits/3_0.png")),
    ('4', include_bytes!("../assets/page_digits/4_0.png")),
    ('4', include_bytes!("../assets/page_digits/4_1.png")),
    ('5', include_bytes!("../assets/page_digits/5_0.png")),
    ('6', include_bytes!("../assets/page_digits/6_0.png")),
    ('7', include_bytes!("../assets/page_digits/7_0.png")),
    ('7', include_bytes!("../assets/page_digits/7_1.png")),
    ('8', include_bytes!("../assets/page_digits/8_0.png")),
    ('9', include_bytes!("../assets/page_digits/9_0.png")),
    ('9', include_bytes!("../assets/page_digits/9_1.png")),
    ('/', include_bytes!("../assets/page_digits/slash_0.png")),
    ('/', include_bytes!("../assets/page_digits/slash_1.png")),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_page_number_fixtures() {
        let reader = PageNumberReader::default();
        for current in 1..=20 {
            let path = format!("../tests/fixtures/page_numbers/page_{current:02}_of_49.png");
            let image = image::load_from_memory(fixture_bytes(&path))
                .unwrap_or_else(|error| panic!("{path}: {error}"))
                .to_rgba8();
            let page = reader.read_page_number(&image).unwrap_or_else(|error| {
                panic!("{path}: {error}");
            });
            assert_eq!(page.current, current, "{path}");
            assert_eq!(page.total, 49, "{path}");
        }
    }

    #[test]
    fn reads_failure_fixtures_without_windows_ocr() {
        let reader = PageNumberReader::default();
        for (path, current, total) in [
            ("../tests/fixtures/page_numbers/failure_01_of_27.png", 1, 27),
            ("../tests/fixtures/page_numbers/failure_01_of_47.png", 1, 47),
        ] {
            let image = image::load_from_memory(fixture_bytes(path))
                .unwrap_or_else(|error| panic!("{path}: {error}"))
                .to_rgba8();
            let page = reader.read_page_number(&image).unwrap_or_else(|error| {
                panic!("{path}: {error}");
            });
            assert_eq!(page.current, current, "{path}");
            assert_eq!(page.total, total, "{path}");
        }
    }

    #[test]
    fn hint_does_not_fill_missing_current_page() {
        let reader = PageNumberReader::default();
        let image = image::load_from_memory(fixture_bytes(
            "../tests/fixtures/page_numbers/failure_01_of_27.png",
        ))
        .unwrap()
        .to_rgba8();
        let cropped =
            image::imageops::crop_imm(&image, 31, 0, image.width() - 31, image.height()).to_image();
        let error = reader
            .read_page_number_with_hint(
                &cropped,
                PageReadHint {
                    previous_current: None,
                    expected_current: Some(1),
                    expected_total: Some(27),
                },
            )
            .unwrap_err();
        assert!(error.to_string().contains("cannot read page number"));
    }

    #[test]
    fn blank_page_number_fails_with_diagnostics() {
        let reader = PageNumberReader::default();
        let image = RgbaImage::new(95, 60);
        let (result, diagnostics) =
            reader.read_page_number_with_hint_diagnostics(&image, PageReadHint::default());
        assert!(result.is_err());
        assert!(!diagnostics.attempts.is_empty());
        assert!(
            diagnostics
                .attempts
                .iter()
                .all(|attempt| attempt.glyph_count == Some(0))
        );
    }

    fn fixture_bytes(path: &str) -> &'static [u8] {
        match path {
            "../tests/fixtures/page_numbers/page_01_of_49.png" => {
                include_bytes!("../tests/fixtures/page_numbers/page_01_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_02_of_49.png" => {
                include_bytes!("../tests/fixtures/page_numbers/page_02_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_03_of_49.png" => {
                include_bytes!("../tests/fixtures/page_numbers/page_03_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_04_of_49.png" => {
                include_bytes!("../tests/fixtures/page_numbers/page_04_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_05_of_49.png" => {
                include_bytes!("../tests/fixtures/page_numbers/page_05_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_06_of_49.png" => {
                include_bytes!("../tests/fixtures/page_numbers/page_06_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_07_of_49.png" => {
                include_bytes!("../tests/fixtures/page_numbers/page_07_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_08_of_49.png" => {
                include_bytes!("../tests/fixtures/page_numbers/page_08_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_09_of_49.png" => {
                include_bytes!("../tests/fixtures/page_numbers/page_09_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_10_of_49.png" => {
                include_bytes!("../tests/fixtures/page_numbers/page_10_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_11_of_49.png" => {
                include_bytes!("../tests/fixtures/page_numbers/page_11_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_12_of_49.png" => {
                include_bytes!("../tests/fixtures/page_numbers/page_12_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_13_of_49.png" => {
                include_bytes!("../tests/fixtures/page_numbers/page_13_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_14_of_49.png" => {
                include_bytes!("../tests/fixtures/page_numbers/page_14_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_15_of_49.png" => {
                include_bytes!("../tests/fixtures/page_numbers/page_15_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_16_of_49.png" => {
                include_bytes!("../tests/fixtures/page_numbers/page_16_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_17_of_49.png" => {
                include_bytes!("../tests/fixtures/page_numbers/page_17_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_18_of_49.png" => {
                include_bytes!("../tests/fixtures/page_numbers/page_18_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_19_of_49.png" => {
                include_bytes!("../tests/fixtures/page_numbers/page_19_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_20_of_49.png" => {
                include_bytes!("../tests/fixtures/page_numbers/page_20_of_49.png")
            }
            "../tests/fixtures/page_numbers/failure_01_of_27.png" => {
                include_bytes!("../tests/fixtures/page_numbers/failure_01_of_27.png")
            }
            "../tests/fixtures/page_numbers/failure_01_of_47.png" => {
                include_bytes!("../tests/fixtures/page_numbers/failure_01_of_47.png")
            }
            _ => panic!("unknown fixture: {path}"),
        }
    }
}
