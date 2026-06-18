mod edge;
mod ncc;
mod scale;

use std::collections::BTreeMap;

use image::RgbaImage;

use crate::error::{AutomationError, AutomationResult};
use crate::model::{Point, Rect, Size, TemplateMatch};
use crate::profile::{template_bytes, AutomationProfile, TemplateSpec};

use self::edge::gradient_magnitude;
use self::ncc::{normalized_cross_correlation_prepared, IntegralPlane, Plane, PreparedPlane};
use self::scale::{resize_template, scaled_size, CANDIDATE_SCALES};

#[derive(Debug, Clone)]
pub struct ImageTemplateMatcher {
    profile: AutomationProfile,
    templates: BTreeMap<String, PreparedTemplate>,
}

#[derive(Debug, Clone)]
struct PreparedTemplate {
    scales: Vec<PreparedTemplateScale>,
}

#[derive(Debug, Clone)]
struct PreparedTemplateScale {
    size: Size,
    scale: f32,
    gray: PreparedPlane,
    edge: PreparedPlane,
}

impl ImageTemplateMatcher {
    pub fn new(profile: AutomationProfile) -> Self {
        let templates = profile
            .templates
            .iter()
            .filter_map(|(name, spec)| {
                prepare_template(spec)
                    .ok()
                    .map(|template| (name.clone(), template))
            })
            .collect::<BTreeMap<_, _>>();
        Self { profile, templates }
    }

    pub fn verify(&self, name: &str, screen_image: &RgbaImage) -> AutomationResult<TemplateMatch> {
        self.match_template(name, screen_image)
    }

    pub fn find(&self, name: &str, screen_image: &RgbaImage) -> AutomationResult<TemplateMatch> {
        self.match_template(name, screen_image)
    }

    pub fn search_rect(&self, name: &str, screen_size: Size) -> AutomationResult<Rect> {
        let spec = self
            .profile
            .templates
            .get(name)
            .ok_or_else(|| AutomationError::message(format!("unknown template: {name}")))?;
        Ok(spec.rect.expand(spec.search_padding).clamp(screen_size))
    }

    pub fn verify_in_rect(
        &self,
        name: &str,
        screen_image: &RgbaImage,
        screen_rect: Rect,
    ) -> AutomationResult<TemplateMatch> {
        self.match_template_in_rect(name, screen_image, screen_rect)
    }

    pub fn find_in_rect(
        &self,
        name: &str,
        screen_image: &RgbaImage,
        screen_rect: Rect,
    ) -> AutomationResult<TemplateMatch> {
        self.match_template_in_rect(name, screen_image, screen_rect)
    }

    fn match_template(
        &self,
        name: &str,
        screen_image: &RgbaImage,
    ) -> AutomationResult<TemplateMatch> {
        let screen_size = Size {
            width: screen_image.width(),
            height: screen_image.height(),
        };
        let search_rect = self.search_rect(name, screen_size)?;
        self.match_template_with_search(
            name,
            screen_image,
            Point { x: 0, y: 0 },
            search_rect,
            search_rect,
        )
    }

    fn match_template_in_rect(
        &self,
        name: &str,
        screen_image: &RgbaImage,
        screen_rect: Rect,
    ) -> AutomationResult<TemplateMatch> {
        let local_search_rect = Rect {
            x: 0,
            y: 0,
            width: screen_image.width(),
            height: screen_image.height(),
        };
        self.match_template_with_search(
            name,
            screen_image,
            Point {
                x: screen_rect.x,
                y: screen_rect.y,
            },
            local_search_rect,
            screen_rect,
        )
    }

    fn match_template_with_search(
        &self,
        name: &str,
        screen_image: &RgbaImage,
        screen_origin: Point,
        search_rect: Rect,
        searched_rect: Rect,
    ) -> AutomationResult<TemplateMatch> {
        let spec = self
            .profile
            .templates
            .get(name)
            .ok_or_else(|| AutomationError::message(format!("unknown template: {name}")))?;
        let prepared = self
            .templates
            .get(name)
            .ok_or_else(|| AutomationError::message(format!("template not prepared: {name}")))?;
        let screen_gray = Plane::from_rgba(screen_image);
        let screen_edge = gradient_magnitude(&screen_gray);
        let screen_gray_integral = IntegralPlane::new(&screen_gray);
        let screen_edge_integral = IntegralPlane::new(&screen_edge);

        let mut best: Option<TemplateMatch> = None;
        for scale in &prepared.scales {
            if scale.size.width > search_rect.width || scale.size.height > search_rect.height {
                continue;
            }
            let (local_point, edge_score, gray_score, candidate_count) = best_position(
                &screen_edge,
                &screen_edge_integral,
                &scale.edge,
                &screen_gray,
                &screen_gray_integral,
                &scale.gray,
                search_rect,
            );
            let point = Point {
                x: local_point.x + screen_origin.x,
                y: local_point.y + screen_origin.y,
            };
            let matched =
                edge_score >= spec.edge_threshold(name) && gray_score >= spec.gray_floor(name);
            let candidate = TemplateMatch {
                name: name.to_string(),
                matched,
                edge_score,
                gray_score,
                point,
                size: scale.size,
                scale: scale.scale,
                searched_rect,
                candidate_count,
            };
            if best
                .as_ref()
                .is_none_or(|current| better_match(&candidate, current, spec, name))
            {
                best = Some(candidate);
            }
            if best
                .as_ref()
                .is_some_and(|current| high_confidence_match(current, spec, name))
            {
                break;
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

fn prepare_template(spec: &TemplateSpec) -> AutomationResult<PreparedTemplate> {
    let template = load_template_image(spec)?;
    let scales = CANDIDATE_SCALES
        .iter()
        .map(|scale| {
            let size = scaled_size(spec.rect.size(), *scale);
            let resized = resize_template(&template, size);
            let gray_plane = Plane::from_rgba(&resized);
            let edge_plane = gradient_magnitude(&gray_plane);
            PreparedTemplateScale {
                size,
                scale: *scale,
                gray: PreparedPlane::new(gray_plane),
                edge: PreparedPlane::new(edge_plane),
            }
        })
        .collect();
    Ok(PreparedTemplate { scales })
}

fn load_template_image(spec: &TemplateSpec) -> AutomationResult<RgbaImage> {
    let bytes = template_bytes(&spec.file)
        .ok_or_else(|| AutomationError::message(format!("template not bundled: {}", spec.file)))?;
    Ok(image::load_from_memory(bytes)?.to_rgba8())
}

fn best_position(
    screen_edge: &Plane,
    screen_edge_integral: &IntegralPlane,
    template_edge: &PreparedPlane,
    screen_gray: &Plane,
    screen_gray_integral: &IntegralPlane,
    template_gray: &PreparedPlane,
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
            let score = normalized_cross_correlation_prepared(
                screen_edge,
                screen_edge_integral,
                template_edge,
                x as usize,
                y as usize,
            );
            if score > best_score {
                best_score = score;
                best_point = Point { x, y };
                if best_score >= 0.80 {
                    break;
                }
            }
        }
        if best_score >= 0.80 {
            break;
        }
    }

    let refine_left = (best_point.x - step as i32 - 2).max(search_rect.x);
    let refine_top = (best_point.y - step as i32 - 2).max(search_rect.y);
    let refine_right = (best_point.x + step as i32 + 2).min(max_x);
    let refine_bottom = (best_point.y + step as i32 + 2).min(max_y);
    let mut best_gray = normalized_cross_correlation_prepared(
        screen_gray,
        screen_gray_integral,
        template_gray,
        best_point.x as usize,
        best_point.y as usize,
    );
    let mut best_quality = match_quality(best_score, best_gray);
    for y in refine_top..=refine_bottom {
        for x in refine_left..=refine_right {
            candidate_count += 1;
            let edge_score = normalized_cross_correlation_prepared(
                screen_edge,
                screen_edge_integral,
                template_edge,
                x as usize,
                y as usize,
            );
            let gray_score = normalized_cross_correlation_prepared(
                screen_gray,
                screen_gray_integral,
                template_gray,
                x as usize,
                y as usize,
            );
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

fn high_confidence_match(candidate: &TemplateMatch, spec: &TemplateSpec, name: &str) -> bool {
    candidate.matched
        && candidate.edge_score >= spec.edge_threshold(name) + 0.20
        && candidate.gray_score >= 0.90
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
    fn matcher_finds_template_in_cropped_search_rect_with_absolute_point() {
        let profile = load_profile().unwrap();
        let spec = &profile.templates["homeBoardFileIcon"];
        let template = load_template_image(spec).unwrap();
        let search_rect = spec.rect.expand(spec.search_padding).clamp(Size {
            width: 1920,
            height: 1080,
        });
        let expected = Point { x: 438, y: 982 };
        let mut crop = RgbaImage::from_pixel(
            search_rect.width,
            search_rect.height,
            Rgba([24, 28, 30, 255]),
        );
        image::imageops::overlay(
            &mut crop,
            &template,
            i64::from(expected.x - search_rect.x),
            i64::from(expected.y - search_rect.y),
        );
        let matcher = ImageTemplateMatcher::new(profile);

        let matched = matcher
            .find_in_rect("homeBoardFileIcon", &crop, search_rect)
            .unwrap();

        assert!(matched.matched);
        assert_eq!(matched.point, expected);
        assert_eq!(matched.searched_rect, search_rect);
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
