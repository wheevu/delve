use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Reusable workspace for A* pathfinding. Size the workspace to `width * height`.
pub struct AStarWorkspace {
    pub came_from: Vec<(i32, i32)>,
    pub g_score: Vec<i32>,
    pub f_score: Vec<i32>,
}

impl AStarWorkspace {
    pub fn new(width: i32, height: i32) -> Self {
        let size = (width * height) as usize;
        Self {
            came_from: vec![(0, 0); size],
            g_score: vec![i32::MAX; size],
            f_score: vec![i32::MAX; size],
        }
    }

    pub fn reset(&mut self) {
        self.g_score.fill(i32::MAX);
        self.f_score.fill(i32::MAX);
    }
}

/// Run A* from `start` to `goal`, returning the **first step** from `start`
/// toward `goal`. Returns `None` if no path is found within `max_steps`.
///
/// `can_walk` returns `true` when a cell is passable (the caller may allow the
/// goal even when it is not normally walkable).
///
/// # Panics
///
/// Panics if coordinates are out of bounds relative to the workspace size.
/// The caller must ensure `width * height` matches the workspace dimensions.
pub fn a_star_step<F>(
    ws: &mut AStarWorkspace,
    width: i32,
    height: i32,
    start: (i32, i32),
    goal: (i32, i32),
    max_steps: usize,
    can_walk: F,
) -> Option<(i32, i32)>
where
    F: Fn(i32, i32) -> bool,
{
    if start == goal {
        return Some(start);
    }

    ws.reset();
    let idx = |x: i32, y: i32| -> usize { (y * width + x) as usize };
    let h = |x: i32, y: i32| -> i32 { (goal.0 - x).abs() + (goal.1 - y).abs() };

    let si = idx(start.0, start.1);
    ws.g_score[si] = 0;
    ws.f_score[si] = h(start.0, start.1);

    let mut open = BinaryHeap::new();
    open.push(Reverse((ws.f_score[si], start.0, start.1)));
    let mut steps = 0;

    while let Some(Reverse((_, cx, cy))) = open.pop() {
        steps += 1;
        if steps > max_steps {
            break;
        }

        if (cx, cy) == goal {
            // Walk back from goal to the step immediately after start.
            let mut cur = goal;
            while ws.came_from[idx(cur.0, cur.1)] != start {
                cur = ws.came_from[idx(cur.0, cur.1)];
            }
            // `cur` is now the first step from `start` toward `goal`.
            return Some(cur);
        }

        let ci = idx(cx, cy);
        for (dx, dy) in &[(0, -1), (0, 1), (-1, 0), (1, 0)] {
            let (nx, ny) = (cx + dx, cy + dy);
            if nx < 0 || nx >= width || ny < 0 || ny >= height {
                continue;
            }
            if !can_walk(nx, ny) && (nx, ny) != goal {
                continue;
            }
            let ni = idx(nx, ny);
            let t = ws.g_score[ci].saturating_add(1);
            if t < ws.g_score[ni] {
                ws.came_from[ni] = (cx, cy);
                ws.g_score[ni] = t;
                ws.f_score[ni] = t + h(nx, ny);
                open.push(Reverse((ws.f_score[ni], nx, ny)));
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fill_floor(width: i32, height: i32) -> Vec<bool> {
        vec![true; (width * height) as usize]
    }

    #[test]
    fn adjacent_step() {
        let mut ws = AStarWorkspace::new(10, 10);
        let passable = fill_floor(10, 10);
        let result = a_star_step(&mut ws, 10, 10, (0, 0), (1, 0), 100, |x, y| {
            passable[(y * 10 + x) as usize]
        });
        assert_eq!(result, Some((1, 0)));
    }

    #[test]
    fn two_step_path() {
        let mut ws = AStarWorkspace::new(10, 10);
        let passable = fill_floor(10, 10);
        // start → (1,0) → (2,0)
        let result = a_star_step(&mut ws, 10, 10, (0, 0), (2, 0), 100, |x, y| {
            passable[(y * 10 + x) as usize]
        });
        assert_eq!(result, Some((1, 0)), "should return first step, not goal");
    }

    #[test]
    fn start_equals_goal() {
        let mut ws = AStarWorkspace::new(10, 10);
        let passable = fill_floor(10, 10);
        let result = a_star_step(&mut ws, 10, 10, (3, 3), (3, 3), 100, |x, y| {
            passable[(y * 10 + x) as usize]
        });
        assert_eq!(result, Some((3, 3)));
    }

    #[test]
    fn wall_blocked() {
        let mut ws = AStarWorkspace::new(10, 10);
        let mut passable = fill_floor(10, 10);
        for y in 0..10 {
            passable[(y * 10 + 5) as usize] = false;
        }
        let result = a_star_step(&mut ws, 10, 10, (0, 0), (9, 0), 100, |x, y| {
            passable[(y * 10 + x) as usize]
        });
        assert_eq!(result, None);
    }

    #[test]
    fn l_corridor_first_step() {
        // 10x10 all walls except L-shaped corridor from (1,1) to (8,8)
        let mut ws = AStarWorkspace::new(10, 10);
        let mut passable = [false; 100];
        for y in 1..9 {
            passable[(y * 10 + 1) as usize] = true;
        }
        for x in 1..9 {
            passable[(8 * 10 + x) as usize] = true;
        }
        let result = a_star_step(&mut ws, 10, 10, (1, 1), (8, 8), 500, |x, y| {
            passable[(y * 10 + x) as usize]
        });
        assert!(result.is_some());
        let (nx, ny) = result.unwrap();
        assert!(
            (nx, ny) == (1, 2) || (nx, ny) == (2, 1),
            "first step should be adjacent to start, got {:?}",
            (nx, ny)
        );
    }
}
