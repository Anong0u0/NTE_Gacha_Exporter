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

#[derive(Debug, Clone)]
pub struct PreparedPlane {
    pub width: usize,
    pub height: usize,
    pub zero_mean: Vec<f32>,
    pub energy: f32,
}

impl PreparedPlane {
    pub fn new(plane: Plane) -> Self {
        let count = (plane.width * plane.height).max(1) as f32;
        let mean = plane.data.iter().copied().sum::<f32>() / count;
        let mut energy = 0.0;
        let zero_mean = plane
            .data
            .into_iter()
            .map(|value| {
                let delta = value - mean;
                energy += delta * delta;
                delta
            })
            .collect::<Vec<_>>();
        Self {
            width: plane.width,
            height: plane.height,
            zero_mean,
            energy,
        }
    }
}

#[derive(Debug, Clone)]
pub struct IntegralPlane {
    width: usize,
    sum: Vec<f32>,
    sum_sq: Vec<f32>,
}

impl IntegralPlane {
    pub fn new(plane: &Plane) -> Self {
        let width = plane.width + 1;
        let height = plane.height + 1;
        let mut sum = vec![0.0; width * height];
        let mut sum_sq = vec![0.0; width * height];
        for y in 0..plane.height {
            let mut row_sum = 0.0;
            let mut row_sum_sq = 0.0;
            for x in 0..plane.width {
                let value = plane.data[y * plane.width + x];
                row_sum += value;
                row_sum_sq += value * value;
                let index = (y + 1) * width + x + 1;
                let above = y * width + x + 1;
                sum[index] = sum[above] + row_sum;
                sum_sq[index] = sum_sq[above] + row_sum_sq;
            }
        }
        let _ = height;
        Self { width, sum, sum_sq }
    }

    fn rect_sum(values: &[f32], width: usize, x: usize, y: usize, w: usize, h: usize) -> f32 {
        let left = x;
        let top = y;
        let right = x + w;
        let bottom = y + h;
        values[bottom * width + right] + values[top * width + left]
            - values[top * width + right]
            - values[bottom * width + left]
    }

    pub fn variance_sum(&self, x: usize, y: usize, w: usize, h: usize) -> f32 {
        let count = (w * h) as f32;
        let sum = Self::rect_sum(&self.sum, self.width, x, y, w, h);
        let sum_sq = Self::rect_sum(&self.sum_sq, self.width, x, y, w, h);
        (sum_sq - (sum * sum / count)).max(0.0)
    }
}

pub fn normalized_cross_correlation_prepared(
    screen: &Plane,
    screen_integral: &IntegralPlane,
    template: &PreparedPlane,
    x: usize,
    y: usize,
) -> f32 {
    if template.width == 0
        || template.height == 0
        || x + template.width > screen.width
        || y + template.height > screen.height
    {
        return -1.0;
    }
    if template.energy <= f32::EPSILON {
        return -1.0;
    }
    let screen_energy = screen_integral.variance_sum(x, y, template.width, template.height);
    if screen_energy <= f32::EPSILON {
        return -1.0;
    }
    let mut numerator = 0.0;
    for ty in 0..template.height {
        let screen_offset = (y + ty) * screen.width + x;
        let template_offset = ty * template.width;
        for tx in 0..template.width {
            numerator += screen.data[screen_offset + tx] * template.zero_mean[template_offset + tx];
        }
    }
    let denom = (screen_energy * template.energy).sqrt();
    (numerator / denom).clamp(-1.0, 1.0)
}

fn gray(pixel: &Rgba<u8>) -> f32 {
    pixel[0] as f32 * 0.299 + pixel[1] as f32 * 0.587 + pixel[2] as f32 * 0.114
}
