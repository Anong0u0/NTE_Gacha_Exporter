use super::ncc::Plane;

pub fn gradient_magnitude(source: &Plane) -> Plane {
    let mut data = vec![0.0; source.data.len()];
    if source.width < 3 || source.height < 3 {
        return Plane {
            width: source.width,
            height: source.height,
            data,
        };
    }
    for y in 1..source.height - 1 {
        for x in 1..source.width - 1 {
            let left = source.get(x - 1, y);
            let right = source.get(x + 1, y);
            let up = source.get(x, y - 1);
            let down = source.get(x, y + 1);
            data[y * source.width + x] = (right - left).abs() + (down - up).abs();
        }
    }
    Plane {
        width: source.width,
        height: source.height,
        data,
    }
}
