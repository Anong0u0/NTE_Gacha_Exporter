const CURSOR_CONTEXT_PADDING: i32 = 48;
const CURSOR_MARKER_RADIUS: i32 = 4;
const CURSOR_MARKER_ARM: i32 = 12;

fn page_context_rect(
    page_rect: crate::model::Rect,
    client_size: Size,
    cursor: Option<Point>,
) -> crate::model::Rect {
    let base = page_rect
        .expand(Point {
            x: page_rect.width as i32,
            y: (page_rect.height / 2) as i32,
        });
    cursor
        .map(|point| union_rect(base, point_context_rect(point, CURSOR_CONTEXT_PADDING)))
        .unwrap_or(base)
        .clamp(client_size)
}

fn point_context_rect(point: Point, padding: i32) -> crate::model::Rect {
    let padding = padding.max(0);
    let size = (padding as u32).saturating_mul(2).saturating_add(1);
    crate::model::Rect {
        x: point.x - padding,
        y: point.y - padding,
        width: size,
        height: size,
    }
}

fn union_rect(left: crate::model::Rect, right: crate::model::Rect) -> crate::model::Rect {
    let x = left.x.min(right.x);
    let y = left.y.min(right.y);
    let right_edge = left.right().max(right.right());
    let bottom_edge = left.bottom().max(right.bottom());
    crate::model::Rect {
        x,
        y,
        width: (right_edge - x).max(1) as u32,
        height: (bottom_edge - y).max(1) as u32,
    }
}

fn point_in_size(point: Point, size: Size) -> bool {
    point.x >= 0 && point.y >= 0 && point.x < size.width as i32 && point.y < size.height as i32
}

fn rect_contains_point(rect: crate::model::Rect, point: Point) -> bool {
    point.x >= rect.x && point.y >= rect.y && point.x < rect.right() && point.y < rect.bottom()
}

fn draw_rect_outline(
    image: &mut image::RgbaImage,
    rect: crate::model::Rect,
    color: image::Rgba<u8>,
    thickness: u32,
) {
    let width = image.width() as i32;
    let height = image.height() as i32;
    let left = rect.x.clamp(0, width.saturating_sub(1));
    let top = rect.y.clamp(0, height.saturating_sub(1));
    let right = rect.right().saturating_sub(1).clamp(0, width.saturating_sub(1));
    let bottom = rect
        .bottom()
        .saturating_sub(1)
        .clamp(0, height.saturating_sub(1));
    let thickness = thickness.max(1) as i32;
    for offset in 0..thickness {
        let l = (left - offset).clamp(0, width.saturating_sub(1));
        let t = (top - offset).clamp(0, height.saturating_sub(1));
        let r = (right + offset).clamp(0, width.saturating_sub(1));
        let b = (bottom + offset).clamp(0, height.saturating_sub(1));
        for x in l..=r {
            image.put_pixel(x as u32, t as u32, color);
            image.put_pixel(x as u32, b as u32, color);
        }
        for y in t..=b {
            image.put_pixel(l as u32, y as u32, color);
            image.put_pixel(r as u32, y as u32, color);
        }
    }
}

fn draw_cursor_marker(image: &mut image::RgbaImage, point: Point, color: image::Rgba<u8>) {
    for y in -CURSOR_MARKER_RADIUS..=CURSOR_MARKER_RADIUS {
        for x in -CURSOR_MARKER_RADIUS..=CURSOR_MARKER_RADIUS {
            if x * x + y * y <= CURSOR_MARKER_RADIUS * CURSOR_MARKER_RADIUS {
                put_pixel_if_in_bounds(image, point.x + x, point.y + y, color);
            }
        }
    }
    for offset in -CURSOR_MARKER_ARM..=CURSOR_MARKER_ARM {
        put_pixel_if_in_bounds(image, point.x + offset, point.y, color);
        put_pixel_if_in_bounds(image, point.x, point.y + offset, color);
    }
}

fn put_pixel_if_in_bounds(
    image: &mut image::RgbaImage,
    x: i32,
    y: i32,
    color: image::Rgba<u8>,
) {
    if x >= 0 && y >= 0 && x < image.width() as i32 && y < image.height() as i32 {
        image.put_pixel(x as u32, y as u32, color);
    }
}
