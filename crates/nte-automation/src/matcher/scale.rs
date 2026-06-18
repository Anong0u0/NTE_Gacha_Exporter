use image::{RgbaImage, imageops};

use crate::model::Size;

pub const CANDIDATE_SCALES: &[f32] = &[1.0, 0.97, 1.03, 0.94, 1.06];

pub fn resize_template(template: &RgbaImage, size: Size) -> RgbaImage {
    if template.width() == size.width && template.height() == size.height {
        return template.clone();
    }
    imageops::resize(
        template,
        size.width.max(1),
        size.height.max(1),
        imageops::FilterType::Triangle,
    )
}

pub fn scaled_size(base: Size, multiplier: f32) -> Size {
    Size {
        width: ((base.width as f32 * multiplier).round() as u32).max(1),
        height: ((base.height as f32 * multiplier).round() as u32).max(1),
    }
}
