use image::{Rgba, RgbaImage, imageops};

use crate::error::{AutomationError, AutomationResult};
use crate::model::PageNumber;

#[derive(Debug, Clone, Copy, Default)]
pub struct PageReadHint {
    pub previous_current: Option<u32>,
    pub expected_current: Option<u32>,
    pub expected_total: Option<u32>,
}

impl PageReadHint {
    fn is_empty(self) -> bool {
        self.previous_current.is_none()
            && self.expected_current.is_none()
            && self.expected_total.is_none()
    }
}

#[derive(Debug, Clone)]
pub struct WindowsOcrClient {
    #[cfg_attr(not(windows), allow(dead_code))]
    language: String,
}

impl Default for WindowsOcrClient {
    fn default() -> Self {
        Self::new("en-US")
    }
}

impl WindowsOcrClient {
    pub fn new(language: impl Into<String>) -> Self {
        Self {
            language: language.into(),
        }
    }

    pub fn read_page_number(&self, image: &RgbaImage) -> AutomationResult<PageNumber> {
        self.read_page_number_with_hint(image, PageReadHint::default())
    }

    pub fn read_page_number_with_hint(
        &self,
        image: &RgbaImage,
        hint: PageReadHint,
    ) -> AutomationResult<PageNumber> {
        let mut errors = Vec::new();
        let mut best_page = None::<PageNumber>;
        for candidate in page_number_candidates(image) {
            match self.read_text(&candidate).and_then(|text| {
                if hint.is_empty() {
                    parse_page_text(&text)
                } else {
                    parse_page_text_with_hint(&text, hint)
                }
            }) {
                Ok(page) if hint.is_empty() => return Ok(page),
                Ok(page) if page_hint_rank(&page, hint) == 0 => return Ok(page),
                Ok(page) => {
                    let replace = best_page.as_ref().is_none_or(|best| {
                        page_hint_rank(&page, hint) < page_hint_rank(best, hint)
                    });
                    if replace {
                        best_page = Some(page);
                    }
                }
                Err(error) => errors.push(error.to_string()),
            }
        }
        if let Some(page) = best_page {
            return Ok(page);
        }
        Err(AutomationError::message(format!(
            "cannot read page number: {}",
            errors.join("; ")
        )))
    }

    #[cfg(not(windows))]
    pub fn read_text(&self, _image: &RgbaImage) -> AutomationResult<String> {
        let _ = self;
        Err(AutomationError::message("Windows OCR requires Windows"))
    }

    #[cfg(windows)]
    pub fn read_text(&self, image: &RgbaImage) -> AutomationResult<String> {
        let path = write_temp_png(image)?;
        let result = read_text_from_png(&path, &self.language);
        let _ = std::fs::remove_file(&path);
        result
    }
}

pub fn parse_page_text(text: &str) -> AutomationResult<PageNumber> {
    let normalized = normalize_ocr_text(text);
    let Some((current_text, total_text)) = first_digit_pair(&normalized) else {
        return Err(AutomationError::message(format!(
            "cannot parse page text: {text:?}"
        )));
    };
    page_from_digit_pair(text, &current_text, &total_text)
}

pub fn parse_page_text_with_hint(text: &str, hint: PageReadHint) -> AutomationResult<PageNumber> {
    let normalized = normalize_ocr_text(text);
    let pairs = digit_pairs(&normalized);
    if pairs.is_empty() {
        return Err(AutomationError::message(format!(
            "cannot parse page text: {text:?}"
        )));
    }
    let mut candidates = Vec::new();
    for (current_text, total_text) in pairs {
        if let Ok(page) = page_from_digit_pair(text, &current_text, &total_text) {
            candidates.push(page);
        }
        for hinted_current in [hint.expected_current, hint.previous_current]
            .into_iter()
            .flatten()
        {
            if !current_text_matches_hint(&current_text, hinted_current) {
                continue;
            }
            let Ok(total) = parse_positive_u32(&total_text, "total") else {
                continue;
            };
            if hinted_current <= total
                && hint
                    .expected_total
                    .is_none_or(|expected_total| expected_total == total)
            {
                candidates.push(PageNumber {
                    current: hinted_current,
                    total,
                    text: text.to_string(),
                });
            }
        }
    }
    candidates
        .into_iter()
        .filter(|page| hint.expected_total.is_none_or(|total| page.total == total))
        .min_by_key(|page| page_hint_rank(page, hint))
        .ok_or_else(|| AutomationError::message(format!("cannot parse hinted page text: {text:?}")))
}

fn page_hint_rank(page: &PageNumber, hint: PageReadHint) -> u8 {
    if Some(page.current) == hint.expected_current {
        return 0;
    }
    if Some(page.current) == hint.previous_current {
        return 1;
    }
    2
}

fn current_text_matches_hint(current_text: &str, current: u32) -> bool {
    let hinted = current.to_string();
    if current_text == hinted {
        return true;
    }
    if current_text.len() <= hinted.len() {
        return false;
    }
    let repeated = hinted.repeat(current_text.len().div_ceil(hinted.len()));
    if repeated.starts_with(current_text) {
        return true;
    }
    current_text.starts_with(&hinted) || current_text.ends_with(&hinted)
}

fn first_digit_pair(text: &str) -> Option<(String, String)> {
    digit_pairs(text).into_iter().next()
}

fn digit_pairs(text: &str) -> Vec<(String, String)> {
    let groups = digit_groups(text);
    groups
        .windows(2)
        .map(|window| (window[0].clone(), window[1].clone()))
        .collect()
}

fn page_from_digit_pair(
    original_text: &str,
    current_text: &str,
    total_text: &str,
) -> AutomationResult<PageNumber> {
    let current = parse_positive_u32(current_text, "current")?;
    let total = parse_positive_u32(total_text, "total")?;
    if current > total {
        return Err(AutomationError::message(format!(
            "invalid page number: {current}/{total}"
        )));
    }
    Ok(PageNumber {
        current,
        total,
        text: original_text.to_string(),
    })
}

fn parse_positive_u32(text: &str, field: &str) -> AutomationResult<u32> {
    let value = text
        .parse::<u32>()
        .map_err(|error| AutomationError::message(format!("cannot parse page {field}: {error}")))?;
    if value == 0 {
        return Err(AutomationError::message(format!(
            "invalid page {field}: {value}"
        )));
    }
    Ok(value)
}

fn normalize_ocr_text(text: &str) -> String {
    text.chars()
        .map(|ch| match ch {
            'O' | 'o' => '0',
            'I' | 'l' | '|' => '1',
            '／' | '：' => '/',
            '０' => '0',
            '１' => '1',
            '２' => '2',
            '３' => '3',
            '４' => '4',
            '５' => '5',
            '６' => '6',
            '７' => '7',
            '８' => '8',
            '９' => '9',
            other => other,
        })
        .collect()
}

fn digit_groups(text: &str) -> Vec<String> {
    let mut groups = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_ascii_digit() {
            current.push(ch);
        } else if !current.is_empty() {
            groups.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        groups.push(current);
    }
    groups
}

fn page_number_candidates(image: &RgbaImage) -> Vec<RgbaImage> {
    let mut candidates = vec![image.clone()];
    let scaled_width = image.width().saturating_mul(4).max(1);
    let scaled_height = image.height().saturating_mul(4).max(1);
    candidates.push(imageops::resize(
        image,
        scaled_width,
        scaled_height,
        imageops::FilterType::Lanczos3,
    ));
    let contrast = high_contrast_grayscale(image);
    candidates.push(imageops::resize(
        &contrast,
        scaled_width,
        scaled_height,
        imageops::FilterType::Lanczos3,
    ));
    let inverted = invert(&contrast);
    candidates.push(imageops::resize(
        &inverted,
        scaled_width,
        scaled_height,
        imageops::FilterType::Lanczos3,
    ));
    candidates
}

fn high_contrast_grayscale(image: &RgbaImage) -> RgbaImage {
    let mut values = Vec::with_capacity(image.width() as usize * image.height() as usize);
    let mut min = u8::MAX;
    let mut max = u8::MIN;
    for pixel in image.pixels() {
        let value =
            (pixel[0] as f32 * 0.299 + pixel[1] as f32 * 0.587 + pixel[2] as f32 * 0.114) as u8;
        min = min.min(value);
        max = max.max(value);
        values.push(value);
    }
    let range = (max.saturating_sub(min)).max(1) as f32;
    let mut out = RgbaImage::new(image.width(), image.height());
    for (pixel, value) in out.pixels_mut().zip(values) {
        let stretched = (((value.saturating_sub(min)) as f32 / range) * 255.0).round() as u8;
        *pixel = Rgba([stretched, stretched, stretched, 255]);
    }
    out
}

fn invert(image: &RgbaImage) -> RgbaImage {
    let mut out = image.clone();
    for pixel in out.pixels_mut() {
        pixel[0] = 255 - pixel[0];
        pixel[1] = 255 - pixel[1];
        pixel[2] = 255 - pixel[2];
        pixel[3] = 255;
    }
    out
}

#[cfg(windows)]
fn write_temp_png(image: &RgbaImage) -> AutomationResult<std::path::PathBuf> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| AutomationError::message(error.to_string()))?
        .as_nanos();
    let path = std::env::temp_dir().join(format!("nte_ocr_{}_{}.png", std::process::id(), stamp));
    image::DynamicImage::ImageRgba8(image.clone())
        .to_rgb8()
        .save(&path)?;
    Ok(path)
}

#[cfg(windows)]
fn read_text_from_png(path: &std::path::Path, language: &str) -> AutomationResult<String> {
    use windows::Globalization::Language;
    use windows::Graphics::Imaging::BitmapDecoder;
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::{FileAccessMode, StorageFile};
    use windows::core::HSTRING;

    let path_text = HSTRING::from(path.to_string_lossy().as_ref());
    let language_text = HSTRING::from(language);
    let file = StorageFile::GetFileFromPathAsync(&path_text)?.get()?;
    let stream = file.OpenAsync(FileAccessMode::Read)?.get()?;
    let decoder = BitmapDecoder::CreateAsync(&stream)?.get()?;
    let bitmap = decoder.GetSoftwareBitmapAsync()?.get()?;
    let engine = Language::CreateLanguage(&language_text)
        .ok()
        .and_then(|language| OcrEngine::TryCreateFromLanguage(&language).ok())
        .or_else(|| OcrEngine::TryCreateFromUserProfileLanguages().ok())
        .ok_or_else(|| AutomationError::message("Windows OCR engine is unavailable"))?;
    let result = engine.RecognizeAsync(&bitmap)?.get()?;
    Ok(result.Text()?.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_page_text_accepts_common_ocr_noise() {
        let page = parse_page_text("O1／O8").unwrap();
        assert_eq!(page.current, 1);
        assert_eq!(page.total, 8);

        let page = parse_page_text("l2 ： 2O").unwrap();
        assert_eq!(page.current, 12);
        assert_eq!(page.total, 20);
    }

    #[test]
    fn parse_page_text_rejects_invalid_order() {
        let error = parse_page_text("9/3").unwrap_err();
        assert!(error.to_string().contains("invalid page number"));
    }

    #[test]
    fn parse_page_text_with_hint_prefers_expected_transition() {
        let page = parse_page_text_with_hint(
            "22/49",
            PageReadHint {
                previous_current: Some(1),
                expected_current: Some(2),
                expected_total: Some(49),
            },
        )
        .unwrap();
        assert_eq!(page.current, 2);
        assert_eq!(page.total, 49);
    }

    #[test]
    fn parse_page_text_with_hint_keeps_real_unexpected_page_without_context_match() {
        let page = parse_page_text_with_hint(
            "22/49",
            PageReadHint {
                previous_current: Some(20),
                expected_current: Some(21),
                expected_total: Some(49),
            },
        )
        .unwrap();
        assert_eq!(page.current, 22);
        assert_eq!(page.total, 49);
    }
}
