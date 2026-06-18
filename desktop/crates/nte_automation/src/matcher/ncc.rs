use image::{Rgba, RgbaImage};

#[derive(Debug, Clone)]
pub struct Plane {
    pub width: usize,
    pub height: usize,
    pub data: Vec<f32>,
}

impl Plane {
    pub fn from_rgba(image: &RgbaImage) -> Self {
        let width = image.width() as usize;
        let height = image.height() as usize;
        let data = image
            .pixels()
            .map(|pixel| gray(pixel) / 255.0)
            .collect::<Vec<_>>();
        Self {
            width,
            height,
            data,
        }
    }

    pub fn get(&self, x: usize, y: usize) -> f32 {
        self.data[y * self.width + x]
    }
}

pub fn normalized_cross_correlation(screen: &Plane, template: &Plane, x: usize, y: usize) -> f32 {
    if template.width == 0
        || template.height == 0
        || x + template.width > screen.width
        || y + template.height > screen.height
    {
        return -1.0;
    }
    let count = (template.width * template.height) as f32;
    let mut sum_screen = 0.0;
    let mut sum_template = 0.0;
    for ty in 0..template.height {
        let screen_offset = (y + ty) * screen.width + x;
        let template_offset = ty * template.width;
        for tx in 0..template.width {
            sum_screen += screen.data[screen_offset + tx];
            sum_template += template.data[template_offset + tx];
        }
    }
    let mean_screen = sum_screen / count;
    let mean_template = sum_template / count;
    let mut numerator = 0.0;
    let mut screen_energy = 0.0;
    let mut template_energy = 0.0;
    for ty in 0..template.height {
        let screen_offset = (y + ty) * screen.width + x;
        let template_offset = ty * template.width;
        for tx in 0..template.width {
            let screen_delta = screen.data[screen_offset + tx] - mean_screen;
            let template_delta = template.data[template_offset + tx] - mean_template;
            numerator += screen_delta * template_delta;
            screen_energy += screen_delta * screen_delta;
            template_energy += template_delta * template_delta;
        }
    }
    let denom = (screen_energy * template_energy).sqrt();
    if denom <= f32::EPSILON {
        -1.0
    } else {
        (numerator / denom).clamp(-1.0, 1.0)
    }
}

fn gray(pixel: &Rgba<u8>) -> f32 {
    pixel[0] as f32 * 0.299 + pixel[1] as f32 * 0.587 + pixel[2] as f32 * 0.114
}
