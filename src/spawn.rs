// Entity spawning helpers — extracted from app.rs to shrink the monolithic
// GameState surface area.  Each function is free-standing so it doesn't
// create a circular dependency with app.rs.

use rand::Rng;

use crate::entity::{
    Camp, CampKind, Item, ItemKind, Landmark, Monster, MonsterKind, Npc, NpcKind, Player,
};
use crate::map::{Map, Rect, Tile};

// ── Monster spawning ──

/// Push a random (depth‑appropriate) monster at (x, y) with scaled stats.
pub fn push_monster(monsters: &mut Vec<Monster>, rng: &mut impl Rng, x: i32, y: i32, depth: u32) {
    let kind = MonsterKind::random_for_depth(rng, depth);
    let base = kind.stats();
    let scale = 1.0 + depth as f32 * 0.15;
    let mut m = Monster::new(x, y, kind);
    let hp = (base.hp as f32 * scale).round() as i32;
    m.stats.hp = hp;
    m.stats.max_hp = hp;
    monsters.push(m);
}

/// Try 5 times to place one monster in `room`, avoiding the player tile.
pub fn spawn_one_monster(
    monsters: &mut Vec<Monster>,
    map: &Map,
    rng: &mut impl Rng,
    room: &Rect,
    depth: u32,
    player_x: i32,
    player_y: i32,
) {
    for _ in 0..5 {
        let (bx, by) = room.random_tile(rng);
        let idx = map.idx(bx, by);
        if (bx != player_x || by != player_y)
            && map.tiles[idx] == Tile::Floor
            && !monsters.iter().any(|m| m.alive && m.x == bx && m.y == by)
        {
            push_monster(monsters, rng, bx, by, depth);
            return;
        }
    }
    // Fallback — place at room origin even if it overlaps.
    push_monster(monsters, rng, room.x, room.y, depth);
}

/// Place the Crown Guardian on the deepest floor.
pub fn spawn_guardian(
    monsters: &mut Vec<Monster>,
    map: &Map,
    spawn_rooms: &[usize],
    player_room_idx: Option<usize>,
) {
    let guardian_room = spawn_rooms
        .iter()
        .rfind(|&&ri| Some(ri) != player_room_idx)
        .copied()
        .unwrap_or(spawn_rooms[0]);
    let (gx, gy) = map.rooms[guardian_room].center();
    monsters.push(Monster::new(gx, gy, MonsterKind::Guardian));
}

// ── Item spawning ──

/// Build an occupied‑cell bitmap for safe item placement.
pub fn build_occupied_set(
    map: &Map,
    player: &Player,
    monsters: &[Monster],
    items: &[Item],
) -> Vec<bool> {
    let size = (map.width * map.height) as usize;
    let mut occupied = vec![false; size];
    occupied[map.idx(player.x, player.y)] = true;
    for m in monsters {
        if m.alive && map.in_bounds(m.x, m.y) {
            occupied[map.idx(m.x, m.y)] = true;
        }
    }
    for y in 0..map.height {
        for x in 0..map.width {
            let idx = map.idx(x, y);
            let t = map.tiles[idx];
            if matches!(t, Tile::StairsDown | Tile::StairsUp) {
                occupied[idx] = true;
            }
        }
    }
    for item in items {
        occupied[map.idx(item.x, item.y)] = true;
    }
    occupied
}

/// Try to place one random item on a floor tile.
pub fn try_spawn_item(
    items: &mut Vec<Item>,
    map: &Map,
    rng: &mut impl Rng,
    depth: u32,
    occupied: &mut [bool],
) {
    let room_count = map.rooms.len();
    if room_count == 0 {
        return;
    }
    for _ in 0..30 {
        let room = &map.rooms[rng.gen_range(0..room_count)];
        let (ix, iy) = room.random_tile(rng);
        let idx = map.idx(ix, iy);
        if map.tiles[idx] == Tile::Floor && !occupied[idx] {
            occupied[idx] = true;
            let mut item = Item::new(ix, iy, ItemKind::random_for_depth(rng, depth));
            if item.kind.is_equippable() && rng.gen_bool(0.08) {
                item.cursed = true;
            }
            items.push(item);
            return;
        }
    }
}

/// Guarantee the Crown is placed on the deepest floor.
pub fn try_spawn_crown(
    items: &mut Vec<Item>,
    map: &Map,
    rng: &mut impl Rng,
    occupied: &mut [bool],
) {
    let room_count = map.rooms.len();
    if room_count == 0 {
        return;
    }
    // Try random placement first.
    for _ in 0..50 {
        let room = &map.rooms[rng.gen_range(0..room_count)];
        let (ix, iy) = room.random_tile(rng);
        let idx = map.idx(ix, iy);
        if map.tiles[idx] == Tile::Floor && !occupied[idx] {
            occupied[idx] = true;
            items.push(Item::new(ix, iy, ItemKind::CrownOfDepth));
            return;
        }
    }
    // Fallback — scan every floor tile.
    for y in 0..map.height {
        for x in 0..map.width {
            let idx = map.idx(x, y);
            if map.tiles[idx] == Tile::Floor && !occupied[idx] {
                occupied[idx] = true;
                items.push(Item::new(x, y, ItemKind::CrownOfDepth));
                return;
            }
        }
    }
}

// ── Landmark assignment ──

/// Pick 1‑2 landmark rooms (excluding the player's room).
pub fn assign_landmarks(
    landmarks: &mut Vec<(usize, Landmark)>,
    rooms: &[Rect],
    rng: &mut impl Rng,
    depth: u32,
    player_room_idx: Option<usize>,
) {
    landmarks.clear();
    let room_count = rooms.len();
    let landmark_count = if room_count >= 5 { 2 } else { 1 };
    let mut landmark_rooms: Vec<usize> = (0..room_count)
        .filter(|&i| Some(i) != player_room_idx)
        .collect();
    for i in (1..landmark_rooms.len()).rev() {
        let j = rng.gen_range(0..=i);
        landmark_rooms.swap(i, j);
    }
    for &ri in landmark_rooms.iter().take(landmark_count) {
        let lm = Landmark::random(rng, depth);
        landmarks.push((ri, lm));
    }
}

/// Mark a shrine tile as occupied so items don't spawn on it.
pub fn mark_shrine_tiles(
    landmarks: &[(usize, Landmark)],
    rooms: &[Rect],
    map: &Map,
    occupied: &mut [bool],
) {
    for &(ri, lm) in landmarks {
        if lm == Landmark::Shrine {
            let (sx, sy) = rooms[ri].center();
            for &(dx, dy) in &[(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (tx, ty) = (sx + dx, sy + dy);
                if map.is_walkable(tx, ty) && !occupied[map.idx(tx, ty)] {
                    occupied[map.idx(tx, ty)] = true;
                    break;
                }
            }
        }
    }
}

// ── NPC spawning ──

/// Place 1‑2 NPCs in rooms that aren't the player's or a landmark room.
pub fn spawn_npcs(
    npcs: &mut Vec<Npc>,
    rooms: &[Rect],
    rng: &mut impl Rng,
    depth: u32,
    player_room_idx: Option<usize>,
    landmarks: &[(usize, Landmark)],
) {
    npcs.clear();
    let room_count = rooms.len();
    let npc_count = if room_count >= 6 { 2 } else { 1 };
    let mut npc_rooms: Vec<usize> = (0..room_count)
        .filter(|&i| Some(i) != player_room_idx && !landmarks.iter().any(|(ri, _)| *ri == i))
        .collect();
    for i in (1..npc_rooms.len()).rev() {
        let j = rng.gen_range(0..=i);
        npc_rooms.swap(i, j);
    }
    for &ri in npc_rooms.iter().take(npc_count.min(npc_rooms.len())) {
        let (nx, ny) = rooms[ri].center();
        let kind = NpcKind::random_for_depth(rng, depth);
        npcs.push(Npc::new(nx, ny, kind));
    }
}

// ── Camp spawning ──

/// Place one camp in a room that isn't the player's, a landmark's, or an NPC's room.
/// Camps spawn at depth 1+ and contain multiple themed NPCs.
pub fn spawn_camp(
    npcs: &mut Vec<Npc>,
    camps: &mut Vec<Camp>,
    rooms: &[Rect],
    rng: &mut impl Rng,
    depth: u32,
    player_room_idx: Option<usize>,
    landmark_room_indices: &[usize],
) {
    let room_count = rooms.len();
    if room_count < 4 {
        return; // Not enough rooms for a camp
    }

    // Filter rooms: not the player's, not a landmark, and large enough (w >= 5, h >= 5)
    let eligible_rooms: Vec<usize> = (0..room_count)
        .filter(|&i| {
            Some(i) != player_room_idx
                && !landmark_room_indices.contains(&i)
                && rooms[i].w >= 5
                && rooms[i].h >= 5
        })
        .collect();

    if eligible_rooms.is_empty() {
        return;
    }

    let camp_room_idx = eligible_rooms[rng.gen_range(0..eligible_rooms.len())];
    let room = &rooms[camp_room_idx];
    let (cx, cy) = room.center();

    let camp_kind = CampKind::random_for_depth(rng, depth);
    let npc_kinds = camp_kind.npc_kinds();

    let mut camp_npc_indices = Vec::new();
    let base_idx = npcs.len();

    // Place camp NPCs around the room's center
    // Position offsets for placement (spread out within the room)
    let offsets: &[(i32, i32)] = match npc_kinds.len() {
        1 => &[(0, 0)],
        2 => &[(-1, 0), (1, 0)],
        3 => &[(0, -1), (-1, 1), (1, 1)],
        4 => &[(-1, -1), (1, -1), (-1, 1), (1, 1)],
        _ => &[(-2, -1), (0, -1), (2, -1), (-1, 1), (1, 1)],
    };

    for (i, &kind) in npc_kinds.iter().enumerate() {
        let offset_idx = i % offsets.len();
        let (ox, oy) = offsets[offset_idx];
        let (nx, ny) = (
            (cx + ox).clamp(room.x + 1, room.x + room.w - 2),
            (cy + oy).clamp(room.y + 1, room.y + room.h - 2),
        );
        npcs.push(Npc::new(nx, ny, kind));
        camp_npc_indices.push(base_idx + i);
    }

    camps.push(Camp {
        room_idx: camp_room_idx,
        x: cx,
        y: cy,
        kind: camp_kind,
        npc_indices: camp_npc_indices,
        rested: false,
    });
}

// ── Player‑room detection ──

/// Which room contains the player? Returns `None` if none match.
pub fn find_player_room(rooms: &[Rect], player: &Player) -> Option<usize> {
    rooms.iter().position(|r| {
        player.x >= r.x && player.x < r.x + r.w && player.y >= r.y && player.y < r.y + r.h
    })
}

/// Score a room by its summed distance from both stairs positions.
/// Used to place the player as far as possible from the nearest exit.
pub fn room_stairs_dist(
    room: &Rect,
    stairs_down: Option<(i32, i32)>,
    stairs_up: Option<(i32, i32)>,
) -> f64 {
    let (cx, cy) = room.center();
    let cx = cx as f64;
    let cy = cy as f64;
    let mut score = 0f64;
    if let Some((sx, sy)) = stairs_down {
        score += ((cx - sx as f64).powi(2) + (cy - sy as f64).powi(2)).sqrt();
    }
    if let Some((sx, sy)) = stairs_up {
        score += ((cx - sx as f64).powi(2) + (cy - sy as f64).powi(2)).sqrt();
    }
    score
}
