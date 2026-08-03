// Flow field AI for monster pathfinding.
// Generates a distance map from the player outward,
// then each monster picks the adjacent cell with the lowest distance.

use crate::map::Map;

/// BFS flow field from (px, py) outward, updating `dist`.
pub fn compute_flow(dist: &mut [i32], map: &Map, occupancy: &[Option<usize>], px: i32, py: i32) {
    dist.fill(i32::MAX);

    if !map.in_bounds(px, py) {
        return;
    }

    let pi = map.idx(px, py);
    dist[pi] = 0;

    let mut queue = std::collections::VecDeque::new();
    queue.push_back((px, py));

    while let Some((cx, cy)) = queue.pop_front() {
        let ci = map.idx(cx, cy);
        let nd = dist[ci].saturating_add(1);

        for (dx, dy) in &[(0, -1), (0, 1), (-1, 0), (1, 0)] {
            let nx = cx + dx;
            let ny = cy + dy;
            if !map.in_bounds(nx, ny) {
                continue;
            }
            let ni = map.idx(nx, ny);
            if dist[ni] != i32::MAX {
                continue;
            }
            if !map.is_walkable(nx, ny) {
                continue;
            }
            if occupancy[ni].is_some() {
                continue;
            }
            dist[ni] = nd;
            queue.push_back((nx, ny));
        }
    }
}

/// Choose the best adjacent step toward the player on the flow field.
/// Wraith monsters ignore wall walkability but still respect occupancy.
pub fn flow_best_step(
    dist: &[i32],
    map: &Map,
    occupancy: &[Option<usize>],
    mi: usize,
    mx: i32,
    my: i32,
    is_wraith: bool,
) -> Option<(i32, i32)> {
    let me_idx = map.idx(mx, my);
    let mut best: Option<(i32, i32, i32)> = None;

    for (dx, dy) in &[(0, -1), (0, 1), (-1, 0), (1, 0)] {
        let nx = mx + dx;
        let ny = my + dy;
        if !map.in_bounds(nx, ny) {
            continue;
        }
        let ni = map.idx(nx, ny);

        if !is_wraith {
            if !map.is_walkable(nx, ny) {
                continue;
            }
            if let Some(occ) = occupancy[ni] {
                if occ != mi {
                    continue;
                }
            }
        } else {
            let occ = occupancy[ni];
            if let Some(occ_idx) = occ {
                if occ_idx != mi {
                    continue;
                }
            }
        }

        let d = dist[ni];
        if d == i32::MAX {
            continue;
        }

        match best {
            None => best = Some((d, nx, ny)),
            Some((bd, _, _)) if d < bd => best = Some((d, nx, ny)),
            _ => {}
        }
    }

    if let Some((d, nx, ny)) = best {
        if d < dist[me_idx] {
            return Some((nx, ny));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{Map, Tile};

    fn fill_floor(map: &mut Map, w: i32, h: i32) {
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                map.tiles[i] = Tile::Floor;
            }
        }
    }

    fn set_wall(map: &mut Map, w: i32, x: i32, y: i32) {
        let i = (y * w + x) as usize;
        map.tiles[i] = Tile::Wall;
    }

    // ── compute_flow ──

    #[test]
    fn flow_player_at_origin_dist_zero() {
        let mut map = Map::new(10, 10);
        fill_floor(&mut map, 10, 10);
        let occupancy = vec![None; 100];
        let mut dist = vec![i32::MAX; 100];
        compute_flow(&mut dist, &map, &occupancy, 5, 5);
        assert_eq!(dist[map.idx(5, 5)], 0);
    }

    #[test]
    fn flow_adjacent_cells_have_distance_one() {
        let mut map = Map::new(10, 10);
        fill_floor(&mut map, 10, 10);
        let occupancy = vec![None; 100];
        let mut dist = vec![i32::MAX; 100];
        compute_flow(&mut dist, &map, &occupancy, 5, 5);
        assert_eq!(dist[map.idx(4, 5)], 1);
        assert_eq!(dist[map.idx(6, 5)], 1);
        assert_eq!(dist[map.idx(5, 4)], 1);
        assert_eq!(dist[map.idx(5, 6)], 1);
    }

    #[test]
    fn flow_walls_block_propagation() {
        let mut map = Map::new(10, 10);
        fill_floor(&mut map, 10, 10);
        // Wall at (6,5) — player at (5,5), right blocked
        set_wall(&mut map, 10, 6, 5);
        let occupancy = vec![None; 100];
        let mut dist = vec![i32::MAX; 100];
        compute_flow(&mut dist, &map, &occupancy, 5, 5);
        assert_eq!(dist[map.idx(6, 5)], i32::MAX, "wall should be unreachable");
        assert_ne!(dist[map.idx(5, 6)], i32::MAX, "down should be reachable");
    }

    #[test]
    fn flow_occupancy_blocks_propagation() {
        let mut map = Map::new(10, 10);
        fill_floor(&mut map, 10, 10);
        let mut occupancy = vec![None; 100];
        // Occupy (6,5) — a monster is there
        occupancy[map.idx(6, 5)] = Some(0);
        let mut dist = vec![i32::MAX; 100];
        compute_flow(&mut dist, &map, &occupancy, 5, 5);
        assert_eq!(
            dist[map.idx(6, 5)],
            i32::MAX,
            "occupied cell should be unreachable"
        );
    }

    #[test]
    fn flow_player_out_of_bounds_sets_all_unreachable() {
        let map = Map::new(10, 10);
        let occupancy = vec![None; 100];
        let mut dist = vec![i32::MAX; 100];
        compute_flow(&mut dist, &map, &occupancy, -1, -1);
        for d in &dist {
            assert_eq!(*d, i32::MAX);
        }
    }

    // ── flow_best_step ──

    #[test]
    fn best_step_moves_toward_player() {
        let mut map = Map::new(10, 10);
        fill_floor(&mut map, 10, 10);
        let occupancy = vec![None; 100];
        let mut dist = vec![i32::MAX; 100];
        // Player at (7,5), monster at (5,5) → best step should be (6,5)
        compute_flow(&mut dist, &map, &occupancy, 7, 5);
        let step = flow_best_step(&dist, &map, &occupancy, 0, 5, 5, false);
        assert_eq!(step, Some((6, 5)));

        // Player at (5,7), monster at (5,5) → best step should be (5,6)
        compute_flow(&mut dist, &map, &occupancy, 5, 7);
        let step = flow_best_step(&dist, &map, &occupancy, 0, 5, 5, false);
        assert_eq!(step, Some((5, 6)));
    }

    #[test]
    fn best_step_returns_none_when_surrounded_by_walls() {
        let mut map = Map::new(10, 10);
        fill_floor(&mut map, 10, 10);
        // Wall off monster at (5,5)
        set_wall(&mut map, 10, 4, 5);
        set_wall(&mut map, 10, 6, 5);
        set_wall(&mut map, 10, 5, 4);
        set_wall(&mut map, 10, 5, 6);
        let occupancy = vec![None; 100];
        let mut dist = vec![i32::MAX; 100];
        compute_flow(&mut dist, &map, &occupancy, 7, 5);
        let step = flow_best_step(&dist, &map, &occupancy, 0, 5, 5, false);
        assert_eq!(step, None, "surrounded monster should have no valid step");
    }

    #[test]
    fn best_step_wraith_ignores_walls() {
        let mut map = Map::new(10, 10);
        fill_floor(&mut map, 10, 10);
        // Wall at (6,5) — flow field won't reach it, but wraith can still
        // find a path around the wall, e.g. via (5,6)
        set_wall(&mut map, 10, 6, 5);
        let occupancy = vec![None; 100];
        let mut dist = vec![i32::MAX; 100];
        compute_flow(&mut dist, &map, &occupancy, 7, 5);
        let step = flow_best_step(&dist, &map, &occupancy, 0, 5, 5, true);
        // Wraith can't step into the wall (flow field doesn't reach it),
        // but should still pick a valid step around it
        assert!(step.is_some(), "wraith should still be able to move");
        let (nx, ny) = step.unwrap();
        assert!(
            nx == 5 && ny == 6 || nx == 5 && ny == 4,
            "wraith should step around the wall ({nx},{ny})"
        );
    }

    #[test]
    fn best_step_wraith_respects_occupancy() {
        let mut map = Map::new(10, 10);
        fill_floor(&mut map, 10, 10);
        // Another monster at (6,5) — wraith can't move there
        let mut occupancy = vec![None; 100];
        occupancy[map.idx(6, 5)] = Some(1); // different monster
        let mut dist = vec![i32::MAX; 100];
        compute_flow(&mut dist, &map, &occupancy, 7, 5);
        let step = flow_best_step(&dist, &map, &occupancy, 0, 5, 5, true);
        assert_ne!(step, Some((6, 5)), "wraith should respect occupancy");
    }

    #[test]
    fn best_step_does_not_move_away_from_player() {
        let mut map = Map::new(10, 10);
        fill_floor(&mut map, 10, 10);
        let occupancy = vec![None; 100];
        let mut dist = vec![i32::MAX; 100];
        // Player at (6,5), monster at (5,5) → step toward player
        compute_flow(&mut dist, &map, &occupancy, 6, 5);
        let step = flow_best_step(&dist, &map, &occupancy, 0, 5, 5, false);
        assert_eq!(step, Some((6, 5)));
    }

    #[test]
    fn flow_with_self_occupancy_is_passable() {
        // The monster itself occupies its own tile but should still be
        // able to read its own position in the occupancy grid.
        let mut map = Map::new(10, 10);
        fill_floor(&mut map, 10, 10);
        let mut occupancy = vec![None; 100];
        // Monster 0 at (5,5)
        occupancy[map.idx(5, 5)] = Some(0);
        let mut dist = vec![i32::MAX; 100];
        compute_flow(&mut dist, &map, &occupancy, 7, 5);
        let step = flow_best_step(&dist, &map, &occupancy, 0, 5, 5, false);
        assert!(
            step.is_some(),
            "monster should be able to move from its own tile"
        );
    }
}
