use crate::map::Map;

/// Compute visible tiles from origin.
pub fn compute(map: &mut Map, origin_x: i32, origin_y: i32, radius: i32) {
    map.visible.fill(false);

    let origin_idx = map.idx(origin_x, origin_y);
    map.visible[origin_idx] = true;
    map.explored[origin_idx] = true;

    let r2 = radius * radius;
    for y in (origin_y - radius)..=(origin_y + radius) {
        for x in (origin_x - radius)..=(origin_x + radius) {
            if !map.in_bounds(x, y) {
                continue;
            }

            let dx = x - origin_x;
            let dy = y - origin_y;
            if dx * dx + dy * dy > r2 {
                continue;
            }

            if has_line_of_sight(map, origin_x, origin_y, x, y) {
                let idx = map.idx(x, y);
                map.visible[idx] = true;
                map.explored[idx] = true;
            }
        }
    }
}

fn has_line_of_sight(
    map: &Map,
    origin_x: i32,
    origin_y: i32,
    target_x: i32,
    target_y: i32,
) -> bool {
    if (origin_x, origin_y) == (target_x, target_y) {
        return true;
    }

    let mut x = origin_x;
    let mut y = origin_y;
    let dx = (target_x - origin_x).abs();
    let dy = -(target_y - origin_y).abs();
    let sx = if origin_x < target_x { 1 } else { -1 };
    let sy = if origin_y < target_y { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        let e2 = err * 2;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }

        if !map.in_bounds(x, y) {
            return false;
        }

        if (x, y) == (target_x, target_y) {
            return true;
        }

        if map.blocks_fov(x, y) {
            return false;
        }
    }
}
