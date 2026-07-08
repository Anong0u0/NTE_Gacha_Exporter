fn target_from_mask(mask: &[bool], image: &RgbaImage, threshold: u8) -> Option<TargetMask> {
    let width = image.width();
    let height = image.height();
    let components = connected_components(mask, width, height)
        .into_iter()
        .filter(|component| is_text_component(component, width, height))
        .collect::<Vec<_>>();
    if components.is_empty() {
        return None;
    }

    let min_x = components
        .iter()
        .map(|component| component.left)
        .min()
        .unwrap();
    let min_y = components
        .iter()
        .map(|component| component.top)
        .min()
        .unwrap();
    let max_x = components
        .iter()
        .map(|component| component.right())
        .max()
        .unwrap();
    let max_y = components
        .iter()
        .map(|component| component.bottom())
        .max()
        .unwrap();
    let text_height = max_y - min_y + 1;
    let component_count = components.len();
    let min_char_count = component_count.min(MAX_PAGE_TEXT_LEN);
    let estimated_char_count = components
        .iter()
        .map(estimate_component_char_count)
        .sum::<usize>()
        .clamp(min_char_count, MAX_PAGE_TEXT_LEN);

    let pad = 1_u32;
    let left = min_x.saturating_sub(pad);
    let top = min_y.saturating_sub(pad);
    let right = (max_x + pad).min(width.saturating_sub(1));
    let bottom = (max_y + pad).min(height.saturating_sub(1));
    let out_width = right - left + 1;
    let out_height = bottom - top + 1;
    if out_width < 8 || out_height < 8 {
        return None;
    }

    let mut text_components = components
        .iter()
        .map(|component| TextComponent {
            left: component.left - left,
            top: component.top - top,
            width: component.width,
            height: component.height,
        })
        .collect::<Vec<_>>();
    text_components.sort_by_key(|component| (component.left, component.top));

    let mut weights = Vec::with_capacity((out_width * out_height) as usize);
    for y in top..=bottom {
        for x in left..=right {
            let weight = white_text_weight(image.get_pixel(x, y));
            weights.push(weight);
        }
    }

    Some(TargetMask {
        threshold,
        left,
        top,
        width: out_width,
        height: out_height,
        text_height,
        component_count,
        estimated_char_count,
        components: text_components,
        weights,
    })
}

#[derive(Debug, Clone, Copy)]
struct MaskComponent {
    area: u32,
    left: u32,
    top: u32,
    width: u32,
    height: u32,
}

impl MaskComponent {
    fn right(self) -> u32 {
        self.left + self.width - 1
    }

    fn bottom(self) -> u32 {
        self.top + self.height - 1
    }
}

fn connected_components(mask: &[bool], width: u32, height: u32) -> Vec<MaskComponent> {
    let mut seen = vec![false; mask.len()];
    let mut components = Vec::new();

    for y in 0..height {
        for x in 0..width {
            let index = (y * width + x) as usize;
            if !mask[index] || seen[index] {
                continue;
            }

            let mut stack = vec![(x, y)];
            seen[index] = true;
            let mut left = x;
            let mut right = x;
            let mut top = y;
            let mut bottom = y;
            let mut area = 0_u32;

            while let Some((cx, cy)) = stack.pop() {
                area += 1;
                left = left.min(cx);
                right = right.max(cx);
                top = top.min(cy);
                bottom = bottom.max(cy);

                for (dx, dy) in [(1_i32, 0_i32), (-1, 0), (0, 1), (0, -1)] {
                    let nx = cx as i32 + dx;
                    let ny = cy as i32 + dy;
                    if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 {
                        continue;
                    }
                    let next_index = (ny as u32 * width + nx as u32) as usize;
                    if mask[next_index] && !seen[next_index] {
                        seen[next_index] = true;
                        stack.push((nx as u32, ny as u32));
                    }
                }
            }

            components.push(MaskComponent {
                area,
                left,
                top,
                width: right - left + 1,
                height: bottom - top + 1,
            });
        }
    }

    components
}

fn is_text_component(component: &MaskComponent, image_width: u32, image_height: u32) -> bool {
    if component.area < 15 || component.width < 3 || component.height < 6 {
        return false;
    }

    let wide_component = component.width * 4 >= image_width * 3;
    let short_component = component.height * 4 <= image_height;
    if wide_component && short_component {
        return false;
    }

    component.width <= component.height * 8
}

fn estimate_component_char_count(component: &MaskComponent) -> usize {
    ((component.width as f32 / component.height.max(1) as f32) * 1.15)
        .round()
        .clamp(1.0, 3.0) as usize
}

fn threshold_white_text(image: &RgbaImage, threshold: u8) -> Vec<bool> {
    image
        .pixels()
        .map(|pixel| {
            let (luma, chroma) = luma_chroma(pixel);
            luma >= threshold && chroma <= 58
        })
        .collect()
}

fn white_text_weight(pixel: &image::Rgba<u8>) -> f32 {
    let (luma, chroma) = luma_chroma(pixel);
    if chroma > 58 || luma < 64 {
        return 0.0;
    }
    ((luma as f32 - 64.0) / 191.0).clamp(0.0, 1.0)
}

fn luma_chroma(pixel: &image::Rgba<u8>) -> (u8, u8) {
    let max_channel = pixel[0].max(pixel[1]).max(pixel[2]);
    let min_channel = pixel[0].min(pixel[1]).min(pixel[2]);
    let chroma = max_channel.saturating_sub(min_channel);
    let luma =
        (pixel[0] as f32 * 0.299 + pixel[1] as f32 * 0.587 + pixel[2] as f32 * 0.114).round() as u8;
    (luma, chroma)
}
