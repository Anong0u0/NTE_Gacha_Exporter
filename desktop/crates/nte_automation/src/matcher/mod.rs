mod edge;
mod ncc;
mod scale;

use image::RgbaImage;

use crate::error::{AutomationError, AutomationResult};
use crate::model::{Point, Rect, Size, TemplateMatch};
use crate::profile::{template_bytes, AutomationProfile, TemplateSpec};

use self::edge::gradient_magnitude;
use self::ncc::{normalized_cross_correlation, Plane};
use self::scale::{resize_template, scaled_size, CANDIDATE_SCALES};

#[derive(Debug, Clone)]
pub struct ImageTemplateMatcher {
    profile: AutomationProfile,
}

impl ImageTemplateMatcher {
    pub fn new(profile: AutomationProfile) -> Self {
        Self { profile }
    }

    pub fn verify(&self, name: &str, screen_image: &RgbaImage) -> AutomationResult<TemplateMatch> {
        self.match_template(name, screen_image)
    }

    pub fn find(&self, name: &str, screen_image: &RgbaImage) -> AutomationResult<TemplateMatch> {
        self.match_template(name, screen_image)
    }

    fn match_template(
        &self,
        name: &str,
        screen_image: &RgbaImage,
    ) -> AutomationResult<TemplateMatch> {
        let spec = self
            .profile
            .templates
            .get(name)
            .ok_or_else(|| AutomationError::message(format!("unknown template: {name}")))?;
        let template = load_template_image(spec)?;
        let screen_size = Size {
            width: screen_image.width(),
            height: screen_image.height(),
        };
        let search_rect = spec.rect.expand(spec.search_padding).clamp(screen_size);
        let screen_gray = Plane::from_rgba(screen_image);
        let screen_edge = gradient_magnitude(&screen_gray);

        let mut best: Option<TemplateMatch> = None;
        for scale in CANDIDATE_SCALES {
            let candidate_size = scaled_size(spec.rect.size(), *scale);
            if candidate_size.width > search_rect.width
                || candidate_size.height > search_rect.height
            {
                continue;
            }
            let resized = resize_template(&template, candidate_size);
            let template_gray = Plane::from_rgba(&resized);
            let template_edge = gradient_magnitude(&template_gray);
            let (point, edge_score, gray_score, candidate_count) = best_position(
                &screen_edge,
                &template_edge,
                &screen_gray,
                &template_gray,
                search_rect,
            );
            let matched =
                edge_score >= spec.edge_threshold(name) && gray_score >= spec.gray_floor(name);
            let candidate = TemplateMatch {
                name: name.to_string(),
                matched,
                edge_score,
                gray_score,
                point,
                size: candidate_size,
                scale: *scale,
                searched_rect: search_rect,
                candidate_count,
            };
            if best
                .as_ref()
                .is_none_or(|current| better_match(&candidate, current, spec, name))
            {
                best = Some(candidate);
            }
        }

        let Some(match_result) = best else {
            return Err(AutomationError::message(format!(
                "template search area is smaller than template: {name}"
            )));
        };
        if match_result.matched {
            Ok(match_result)
        } else {
            Err(AutomationError::message(format!(
                "screen template not found: {name} edge={:.3} gray={:.3} at={},{} scale={:.2}",
                match_result.edge_score,
                match_result.gray_score,
                match_result.point.x,
                match_result.point.y,
                match_result.scale
            )))
        }
    }
}

fn load_template_image(spec: &TemplateSpec) -> AutomationResult<RgbaImage> {
    let bytes = template_bytes(&spec.file)
        .ok_or_else(|| AutomationError::message(format!("template not bundled: {}", spec.file)))?;
    Ok(image::load_from_memory(bytes)?.to_rgba8())
}

fn best_position(
    screen_edge: &Plane,
    template_edge: &Plane,
    screen_gray: &Plane,
    template_gray: &Plane,
    search_rect: Rect,
) -> (Point, f32, f32, u64) {
    let step = coarse_step(template_edge.width.min(template_edge.height));
    let max_x = search_rect.right() - template_edge.width as i32;
    let max_y = search_rect.bottom() - template_edge.height as i32;
    let mut best_point = Point {
        x: search_rect.x,
        y: search_rect.y,
    };
    let mut best_score = -1.0;
    let mut candidate_count = 0_u64;

    for y in positions(search_rect.y, max_y, step) {
        for x in positions(search_rect.x, max_x, step) {
            candidate_count += 1;
            let score =
                normalized_cross_correlation(screen_edge, template_edge, x as usize, y as usize);
            if score > best_score {
                best_score = score;
                best_point = Point { x, y };
            }
        }
    }

    let refine_left = (best_point.x - step as i32 - 2).max(search_rect.x);
    let refine_top = (best_point.y - step as i32 - 2).max(search_rect.y);
    let refine_right = (best_point.x + step as i32 + 2).min(max_x);
    let refine_bottom = (best_point.y + step as i32 + 2).min(max_y);
    let mut best_gray = normalized_cross_correlation(
        screen_gray,
        template_gray,
        best_point.x as usize,
        best_point.y as usize,
    );
    let mut best_quality = match_quality(best_score, best_gray);
    for y in refine_top..=refine_bottom {
        for x in refine_left..=refine_right {
            candidate_count += 1;
            let edge_score =
                normalized_cross_correlation(screen_edge, template_edge, x as usize, y as usize);
            let gray_score =
                normalized_cross_correlation(screen_gray, template_gray, x as usize, y as usize);
            let quality = match_quality(edge_score, gray_score);
            if quality > best_quality {
                best_score = edge_score;
                best_gray = gray_score;
                best_quality = quality;
                best_point = Point { x, y };
            }
        }
    }

    (best_point, best_score, best_gray, candidate_count)
}

fn better_match(
    candidate: &TemplateMatch,
    current: &TemplateMatch,
    spec: &TemplateSpec,
    name: &str,
) -> bool {
    let candidate_quality = candidate.edge_score + candidate.gray_score * 0.25;
    let current_quality = current.edge_score + current.gray_score * 0.25;
    if (candidate_quality - current_quality).abs() > 0.01 {
        return candidate_quality > current_quality;
    }
    let expected = spec.rect.center();
    distance_sq(candidate.point, expected) < distance_sq(current.point, expected)
        || (candidate.matched && !current.matched)
        || (candidate.edge_score >= spec.edge_threshold(name)
            && current.edge_score < spec.edge_threshold(name))
}

fn coarse_step(template_min: usize) -> usize {
    (template_min / 20).clamp(1, 6)
}

fn positions(start: i32, stop: i32, step: usize) -> Vec<i32> {
    if stop <= start {
        return vec![start];
    }
    let mut out = (start..=stop).step_by(step.max(1)).collect::<Vec<_>>();
    if out.last().copied() != Some(stop) {
        out.push(stop);
    }
    out
}

fn distance_sq(left: Point, right: Point) -> i64 {
    let dx = i64::from(left.x - right.x);
    let dy = i64::from(left.y - right.y);
    dx * dx + dy * dy
}

fn match_quality(edge_score: f32, gray_score: f32) -> f32 {
    edge_score + gray_score * 0.35
}

#[cfg(test)]
mod tests {
    use image::{Rgba, RgbaImage};

    use super::*;
    use crate::profile::load_profile;

    #[test]
    fn matcher_finds_offset_template() {
        let profile = load_profile().unwrap();
        let spec = &profile.templates["homeBoardFileIcon"];
        let template = load_template_image(spec).unwrap();
        let mut screen = RgbaImage::from_pixel(1920, 1080, Rgba([24, 28, 30, 255]));
        image::imageops::overlay(&mut screen, &template, 438, 982);
        let matcher = ImageTemplateMatcher::new(profile);
        let matched = matcher.find("homeBoardFileIcon", &screen).unwrap();
        assert!(matched.matched);
        assert_eq!(matched.point, Point { x: 438, y: 982 });
    }

    #[test]
    fn matcher_finds_scaled_4k_template_from_1080_asset() {
        let profile = load_profile().unwrap();
        let scaled = profile
            .scaled(Size {
                width: 3840,
                height: 2160,
            })
            .unwrap();
        let spec = &scaled.templates["recordTabSelectedCap"];
        let base_spec = &profile.templates["recordTabSelectedCap"];
        let template = resize_template(&load_template_image(base_spec).unwrap(), spec.rect.size());
        let expected_point = Point {
            x: spec.rect.x + 4,
            y: spec.rect.y - 3,
        };
        let mut screen = RgbaImage::from_pixel(3840, 2160, Rgba([8, 12, 16, 255]));
        image::imageops::overlay(
            &mut screen,
            &template,
            i64::from(expected_point.x),
            i64::from(expected_point.y),
        );
        let matcher = ImageTemplateMatcher::new(scaled);
        let matched = matcher.find("recordTabSelectedCap", &screen).unwrap();
        assert!(matched.matched);
        assert!((matched.point.x - expected_point.x).abs() <= 6);
        assert!((matched.point.y - expected_point.y).abs() <= 6);
    }
}
