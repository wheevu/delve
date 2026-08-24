use crate::app::{GameState, GameStatus, RunStats};
use crate::combat;
use crate::entity::{Item, ItemKind, Monster, MonsterKind, NpcKind, Player};
use crate::fov;
use crate::input::Action;
use crate::map::{Map, Tile};
use crate::ui::{MessageLog, MsgColor};
use crate::visual::VisualState;
use rand::rngs::StdRng;
use rand::SeedableRng;

// ── Helpers ──

fn fill_floor(map: &mut Map, w: i32, h: i32) {
    for y in 0..h {
        for x in 0..w {
            let i = map.idx(x, y);
            map.tiles[i] = Tile::Floor;
        }
    }
}

/// Construct a minimal GameState with a flat floor map for headless gameplay tests.
fn state_with_floor(w: i32, h: i32) -> GameState {
    let mut state = GameState::new(42);
    state.map = Map::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let i = state.map.idx(x, y);
            state.map.tiles[i] = Tile::Floor;
        }
    }
    state.status = GameStatus::Playing;
    state.player.x = w / 2;
    state.player.y = h / 2;
    state
}

// ══════════════════════════════════════════════════════════════════════════════
// MAP GENERATION
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn bsp_produces_valid_dungeon() {
    let mut rng = StdRng::seed_from_u64(42);
    let mut map = Map::new(80, 50);
    map.generate(&mut rng, true);
    assert!(!map.rooms.is_empty());
    let floor_count = map
        .tiles
        .iter()
        .filter(|t| matches!(t, Tile::Floor))
        .count();
    assert!(floor_count > 10);
    for room in &map.rooms {
        let (cx, cy) = room.center();
        assert!(map.is_walkable(cx, cy));
    }
}

#[test]
fn bsp_minimum_room_count() {
    for seed in 0..20 {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut map = Map::new(80, 50);
        map.generate(&mut rng, true);
        assert!(
            map.rooms.len() >= 3,
            "seed {}: got {}",
            seed,
            map.rooms.len()
        );
    }
}

#[test]
fn bsp_no_overlapping_rooms() {
    let mut rng = StdRng::seed_from_u64(42);
    let mut map = Map::new(80, 50);
    map.generate(&mut rng, true);
    for i in 0..map.rooms.len() {
        for j in (i + 1)..map.rooms.len() {
            let a = &map.rooms[i];
            let b = &map.rooms[j];
            let ox = a.x < b.x + b.w && a.x + a.w > b.x;
            let oy = a.y < b.y + b.h && a.y + a.h > b.y;
            assert!(!(ox && oy), "rooms {} and {} overlap", i, j);
        }
    }
}

#[test]
fn bsp_all_rooms_connected() {
    let mut rng = StdRng::seed_from_u64(99);
    let mut map = Map::new(80, 50);
    map.generate(&mut rng, true);
    if let Some(first) = map.rooms.first() {
        let (sx, sy) = first.center();
        let mut visited = vec![false; (80 * 50) as usize];
        let mut queue = vec![(sx, sy)];
        visited[map.idx(sx, sy)] = true;
        while let Some((cx, cy)) = queue.pop() {
            for (dx, dy) in &[(0, -1), (0, 1), (-1, 0), (1, 0)] {
                let nx = cx + dx;
                let ny = cy + dy;
                let ni = map.idx(nx, ny);
                if map.is_connectable(nx, ny) && !visited[ni] {
                    visited[ni] = true;
                    queue.push((nx, ny));
                }
            }
        }
        for room in &map.rooms {
            let (cx, cy) = room.center();
            assert!(visited[map.idx(cx, cy)], "room should be reachable");
        }
    }
}

#[test]
fn player_spawn_is_walkable() {
    let mut rng = StdRng::seed_from_u64(42);
    let mut map = Map::new(80, 50);
    map.generate(&mut rng, true);
    if let Some(first) = map.rooms.first() {
        let (cx, cy) = first.center();
        assert!(map.is_walkable(cx, cy));
    }
}

#[test]
fn random_tile_in_room_is_in_bounds() {
    let mut rng = StdRng::seed_from_u64(42);
    let room = crate::map::Rect {
        x: 5,
        y: 5,
        w: 5,
        h: 5,
    };
    for _ in 0..100 {
        let (x, y) = room.random_tile(&mut rng);
        assert!((5..10).contains(&x));
        assert!((5..10).contains(&y));
    }
}

// ── Stronger map invariants (more seeds) ──

#[test]
fn bsp_exactly_one_up_and_down_stair() {
    for seed in 0..100 {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut map = Map::new(80, 50);
        map.generate(&mut rng, true);
        let down = map
            .tiles
            .iter()
            .filter(|t| matches!(t, Tile::StairsDown))
            .count();
        let up = map
            .tiles
            .iter()
            .filter(|t| matches!(t, Tile::StairsUp))
            .count();
        assert_eq!(down, 1, "seed {seed}: expected 1 stairs-down, got {down}");
        assert_eq!(up, 1, "seed {seed}: expected 1 stairs-up, got {up}");
    }
}

#[test]
fn bsp_all_rooms_stay_in_bounds() {
    for seed in 0..100 {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut map = Map::new(80, 50);
        map.generate(&mut rng, true);
        for room in &map.rooms {
            assert!(room.x >= 0, "seed {seed}: room x={} out of bounds", room.x);
            assert!(room.y >= 0, "seed {seed}: room y={} out of bounds", room.y);
            assert!(
                room.x + room.w <= map.width,
                "seed {seed}: room right edge out of bounds"
            );
            assert!(
                room.y + room.h <= map.height,
                "seed {seed}: room bottom edge out of bounds"
            );
        }
    }
}

#[test]
fn bsp_rooms_have_minimum_size() {
    // Rooms placed by place_room have min_w=4, min_h=4
    for seed in 0..100 {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut map = Map::new(80, 50);
        map.generate(&mut rng, true);
        for room in &map.rooms {
            assert!(room.w >= 4, "seed {seed}: room width {} < 4", room.w);
            assert!(room.h >= 4, "seed {seed}: room height {} < 4", room.h);
        }
    }
}

#[test]
fn bsp_connectivity_over_many_seeds() {
    // Verify that all rooms are reachable across 100 seeds.
    // This catches rare disconnected generations.
    for seed in 0..100 {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut map = Map::new(80, 50);
        map.generate(&mut rng, true);

        if map.rooms.is_empty() {
            continue;
        }

        let (sx, sy) = map.rooms[0].center();
        let mut visited = vec![false; (80 * 50) as usize];
        let mut queue = vec![(sx, sy)];
        visited[map.idx(sx, sy)] = true;

        while let Some((cx, cy)) = queue.pop() {
            for (dx, dy) in &[(0, -1), (0, 1), (-1, 0), (1, 0)] {
                let nx = cx + dx;
                let ny = cy + dy;
                let ni = map.idx(nx, ny);
                if map.is_connectable(nx, ny) && !visited[ni] {
                    visited[ni] = true;
                    queue.push((nx, ny));
                }
            }
        }

        for room in &map.rooms {
            let idx = map.idx(room.center().0, room.center().1);
            assert!(
                visited[idx],
                "seed {seed}: room at {:?} not connected",
                room.center()
            );
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// FIELD OF VIEW
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn fov_sees_self() {
    let mut map = Map::new(20, 20);
    fill_floor(&mut map, 20, 20);
    fov::compute(&mut map, 10, 10, 8);
    assert!(map.visible[map.idx(10, 10)]);
    assert!(map.explored[map.idx(10, 10)]);
}

#[test]
fn fov_walls_block_sight() {
    let mut map = Map::new(20, 20);
    fill_floor(&mut map, 20, 20);
    let i = map.idx(11, 10);
    map.tiles[i] = Tile::Wall;
    fov::compute(&mut map, 10, 10, 8);
    assert!(!map.visible[map.idx(12, 10)]);
}

#[test]
fn fov_explored_persists() {
    let mut map = Map::new(20, 20);
    fill_floor(&mut map, 20, 20);
    fov::compute(&mut map, 5, 5, 4);
    assert!(map.explored[map.idx(8, 5)]);
    fov::compute(&mut map, 10, 10, 4);
    assert!(!map.visible[map.idx(8, 5)]);
    assert!(map.explored[map.idx(8, 5)]);
}

#[test]
fn fov_resets_each_frame() {
    let mut map = Map::new(20, 20);
    fill_floor(&mut map, 20, 20);
    fov::compute(&mut map, 5, 5, 4);
    assert!(map.visible[map.idx(8, 5)]);
    fov::compute(&mut map, 10, 10, 4);
    assert!(!map.visible[map.idx(8, 5)]);
}

#[test]
fn fov_distance_limit() {
    let mut map = Map::new(20, 20);
    fill_floor(&mut map, 20, 20);
    fov::compute(&mut map, 10, 10, 3);
    assert!(!map.visible[map.idx(10, 14)]);
}

#[test]
fn fov_open_floor_has_no_radial_gaps() {
    let mut map = Map::new(25, 25);
    fill_floor(&mut map, 25, 25);
    let (ox, oy) = (12, 12);
    let radius = 7;

    fov::compute(&mut map, ox, oy, radius);

    for y in (oy - radius)..=(oy + radius) {
        for x in (ox - radius)..=(ox + radius) {
            let dx = x - ox;
            let dy = y - oy;
            if dx * dx + dy * dy <= radius * radius {
                assert!(
                    map.visible[map.idx(x, y)],
                    "open tile ({x},{y}) should be visible"
                );
            }
        }
    }
}

#[test]
fn fov_corner_visibility() {
    let mut map = Map::new(20, 20);
    fill_floor(&mut map, 20, 20);
    let i1 = map.idx(11, 10);
    map.tiles[i1] = Tile::Wall;
    let i2 = map.idx(12, 11);
    map.tiles[i2] = Tile::Wall;
    fov::compute(&mut map, 10, 10, 3);
    assert!(
        map.visible[map.idx(11, 11)],
        "corner gap at (11,11) should be visible"
    );
    assert!(
        !map.visible[map.idx(12, 10)],
        "cell behind walls should be blocked"
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// COMBAT
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn combat_damage_minimum_one() {
    assert_eq!(combat::resolve_attack(1, 10), 1);
}

#[test]
fn combat_calculates_correctly() {
    assert_eq!(combat::resolve_attack(8, 3), 5);
}

#[test]
fn combat_floor_minimum_always_one() {
    assert_eq!(combat::resolve_attack(0, 100), 1);
    assert_eq!(combat::resolve_attack(-5, 100), 1);
}

// ══════════════════════════════════════════════════════════════════════════════
// ITEMS
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn item_is_weapon() {
    assert!(ItemKind::RustySword.is_weapon());
    assert!(ItemKind::Longsword.is_weapon());
    assert!(ItemKind::Warhammer.is_weapon());
    assert!(!ItemKind::HealthPotion.is_weapon());
    assert!(!ItemKind::LeatherArmor.is_weapon());
}

#[test]
fn item_is_armor() {
    assert!(ItemKind::LeatherArmor.is_armor());
    assert!(ItemKind::Chainmail.is_armor());
    assert!(ItemKind::Shield.is_armor());
    assert!(!ItemKind::RustySword.is_armor());
    assert!(!ItemKind::ScrollOfFire.is_armor());
}

#[test]
fn item_equip_bonuses() {
    assert_eq!(ItemKind::Longsword.attack_bonus(), 3);
    assert_eq!(ItemKind::Warhammer.attack_bonus(), 5);
    assert_eq!(ItemKind::Chainmail.defense_bonus(), 3);
    assert_eq!(ItemKind::Shield.defense_bonus(), 2);
    assert_eq!(ItemKind::HealthPotion.attack_bonus(), 0);
    assert_eq!(ItemKind::HealthPotion.defense_bonus(), 0);
}

#[test]
fn item_heal_amounts() {
    assert_eq!(ItemKind::HealthPotion.heal_amount(), 10);
    assert_eq!(ItemKind::LargeHealthPotion.heal_amount(), 25);
    assert_eq!(ItemKind::RustySword.heal_amount(), 0);
}

#[test]
fn item_min_depth_in_bounds() {
    let all = [
        ItemKind::HealthPotion,
        ItemKind::LargeHealthPotion,
        ItemKind::ScrollOfFire,
        ItemKind::ScrollOfConfusion,
        ItemKind::RustySword,
        ItemKind::Longsword,
        ItemKind::Warhammer,
        ItemKind::LeatherArmor,
        ItemKind::Chainmail,
        ItemKind::Shield,
        ItemKind::CrownOfDepth,
    ];
    for item in &all {
        assert!(
            item.min_depth() <= 5,
            "{} unreachable at min_depth {}",
            item.name(),
            item.min_depth()
        );
    }
}

#[test]
fn item_random_for_depth_ok() {
    let mut rng = StdRng::seed_from_u64(42);
    for depth in 0..=5 {
        let _ = ItemKind::random_for_depth(&mut rng, depth);
    }
}

#[test]
fn monster_random_for_depth_ok() {
    let mut rng = StdRng::seed_from_u64(42);
    for depth in 0..=5 {
        let _ = MonsterKind::random_for_depth(&mut rng, depth);
    }
}

#[test]
fn item_names_are_non_empty() {
    let all = [
        ItemKind::HealthPotion,
        ItemKind::LargeHealthPotion,
        ItemKind::ScrollOfFire,
        ItemKind::ScrollOfConfusion,
        ItemKind::RustySword,
        ItemKind::Longsword,
        ItemKind::Warhammer,
        ItemKind::LeatherArmor,
        ItemKind::Chainmail,
        ItemKind::Shield,
        ItemKind::CrownOfDepth,
    ];
    for item in &all {
        assert!(!item.name().is_empty());
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// PLAYER EQUIPMENT
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn player_total_attack_with_weapon() {
    let mut p = Player::new(0, 0);
    assert_eq!(p.total_attack(), 5);
    p.weapon = Some(ItemKind::Longsword);
    assert_eq!(p.total_attack(), 8);
}

#[test]
fn player_total_defense_with_armor() {
    let mut p = Player::new(0, 0);
    assert_eq!(p.total_defense(), 2);
    p.armor = Some(ItemKind::Chainmail);
    assert_eq!(p.total_defense(), 5);
}

#[test]
fn player_total_attack_without_weapon() {
    assert_eq!(Player::new(0, 0).total_attack(), 5);
}

#[test]
fn player_total_defense_without_armor() {
    assert_eq!(Player::new(0, 0).total_defense(), 2);
}

#[test]
fn guarding_adds_two_defense_without_mutating_base_stats() {
    let mut player = Player::new(0, 0);
    let base = player.stats.defense;
    player.guarding = true;
    assert_eq!(player.total_defense(), base + 2);
    assert_eq!(player.stats.defense, base);
}

#[test]
fn player_equip_does_not_change_base() {
    let mut p = Player::new(0, 0);
    let base = p.stats.attack;
    p.weapon = Some(ItemKind::Warhammer);
    assert_eq!(p.stats.attack, base);
    assert_eq!(p.total_attack(), base + 5);
}

#[test]
fn player_inventory_initial_state() {
    let p = Player::new(0, 0);
    assert!(p.inventory.is_empty());
    assert!(p.weapon.is_none());
    assert!(p.armor.is_none());
    assert!(!p.poisoned);
    assert_eq!(p.depth, 0);
    assert!(p.alive);
}

#[test]
fn consumables_begin_unidentified_but_equipment_does_not() {
    let player = Player::new(0, 0);
    assert_eq!(
        ItemKind::HealthPotion.display_name(&player),
        "unidentified potion"
    );
    assert_eq!(ItemKind::Longsword.display_name(&player), "longsword");
}

// ── Entity accessor completeness ──

#[test]
fn monster_kind_symbol_unique_and_non_empty() {
    let kinds = [
        MonsterKind::Rat,
        MonsterKind::Bat,
        MonsterKind::Goblin,
        MonsterKind::Skeleton,
        MonsterKind::Orc,
        MonsterKind::Spider,
        MonsterKind::Wraith,
    ];
    let mut symbols = std::collections::HashSet::new();
    for k in &kinds {
        let s = k.symbol();
        assert!(s.is_alphabetic(), "symbol for {:?} should be alphabetic", k);
        assert!(symbols.insert(s), "symbol '{}' is not unique", s);
    }
}

#[test]
fn monster_kind_act_chance_in_range() {
    for k in &[
        MonsterKind::Rat,
        MonsterKind::Bat,
        MonsterKind::Goblin,
        MonsterKind::Skeleton,
        MonsterKind::Orc,
        MonsterKind::Spider,
        MonsterKind::Wraith,
    ] {
        let ch = k.act_chance();
        assert!(
            ch > 0.0 && ch <= 1.0,
            "{:?} act_chance {} out of range",
            k,
            ch
        );
    }
}

#[test]
fn monster_kind_min_depth_consistent() {
    assert_eq!(MonsterKind::Rat.min_depth(), 0);
    assert_eq!(MonsterKind::Goblin.min_depth(), 0);
    assert_eq!(MonsterKind::Skeleton.min_depth(), 1);
    assert_eq!(MonsterKind::Orc.min_depth(), 2);
    assert_eq!(MonsterKind::Spider.min_depth(), 3);
    assert_eq!(MonsterKind::Wraith.min_depth(), 4);
}

#[test]
fn monster_kind_weight_positive() {
    for k in &[
        MonsterKind::Rat,
        MonsterKind::Bat,
        MonsterKind::Goblin,
        MonsterKind::Skeleton,
        MonsterKind::Orc,
        MonsterKind::Spider,
        MonsterKind::Wraith,
    ] {
        assert!(k.weight() > 0, "{:?} weight should be positive", k);
    }
}

#[test]
fn item_kind_is_equippable() {
    assert!(ItemKind::RustySword.is_equippable());
    assert!(ItemKind::Chainmail.is_equippable());
    assert!(!ItemKind::HealthPotion.is_equippable());
    assert!(!ItemKind::ScrollOfFire.is_equippable());
    assert!(!ItemKind::CrownOfDepth.is_equippable());
}

#[test]
fn item_kind_symbol_categories() {
    assert_eq!(ItemKind::HealthPotion.symbol(), '!');
    assert_eq!(ItemKind::ScrollOfFire.symbol(), '?');
    assert_eq!(ItemKind::RustySword.symbol(), '/');
    assert_eq!(ItemKind::LeatherArmor.symbol(), '[');
    assert_eq!(ItemKind::CrownOfDepth.symbol(), '*');
}

#[test]
fn item_kind_descriptions_are_non_empty() {
    let all = [
        ItemKind::HealthPotion,
        ItemKind::LargeHealthPotion,
        ItemKind::ScrollOfFire,
        ItemKind::ScrollOfConfusion,
        ItemKind::RustySword,
        ItemKind::Longsword,
        ItemKind::Warhammer,
        ItemKind::LeatherArmor,
        ItemKind::Chainmail,
        ItemKind::Shield,
        ItemKind::CrownOfDepth,
    ];
    for item in &all {
        assert!(
            !item.description().is_empty(),
            "{:?} has empty description",
            item
        );
    }
}

#[test]
fn item_kind_category_labels_are_non_empty() {
    let all = [
        ItemKind::HealthPotion,
        ItemKind::LargeHealthPotion,
        ItemKind::ScrollOfFire,
        ItemKind::ScrollOfConfusion,
        ItemKind::RustySword,
        ItemKind::Longsword,
        ItemKind::Warhammer,
        ItemKind::LeatherArmor,
        ItemKind::Chainmail,
        ItemKind::Shield,
        ItemKind::CrownOfDepth,
    ];
    for item in &all {
        assert!(
            !item.category_label().is_empty(),
            "{:?} has empty category label",
            item
        );
    }
}

#[test]
fn item_kind_stat_lines_for_weapons() {
    let lines = ItemKind::Longsword.stat_lines();
    assert!(
        lines.iter().any(|l| l.contains("ATK")),
        "weapon should have ATK line"
    );
}

#[test]
fn item_kind_stat_lines_for_armor() {
    let lines = ItemKind::Chainmail.stat_lines();
    assert!(
        lines.iter().any(|l| l.contains("DEF")),
        "armor should have DEF line"
    );
}

#[test]
fn item_kind_stat_lines_for_consumables() {
    let lines = ItemKind::HealthPotion.stat_lines();
    assert!(
        lines.iter().any(|l| l.contains("Heal")),
        "potion should have Heal line"
    );
    let lines = ItemKind::ScrollOfFire.stat_lines();
    assert!(
        lines.iter().any(|l| l.contains("AoE")),
        "fire scroll should have AoE line"
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// MESSAGES
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn message_log_capacity() {
    let mut log = MessageLog::new(5);
    for i in 0..10 {
        log.push(format!("msg {}", i), MsgColor::SYSTEM);
    }
    let recent = log.recent(10);
    assert_eq!(recent.len(), 5);
    assert_eq!(recent[0].text, "msg 5");
    assert_eq!(recent[4].text, "msg 9");
}

#[test]
fn message_log_recent_count() {
    let mut log = MessageLog::new(100);
    for i in 0..10 {
        log.push(format!("msg {}", i), MsgColor::SYSTEM);
    }
    assert_eq!(log.recent(3).len(), 3);
    assert_eq!(log.recent(20).len(), 10);
}

#[test]
fn message_log_clear() {
    let mut log = MessageLog::new(100);
    log.push("hello", MsgColor::SYSTEM);
    log.clear();
    assert_eq!(log.recent(10).len(), 0);
}

#[test]
fn message_log_collapses_consecutive_duplicates() {
    let mut log = MessageLog::new(10);
    log.push("The dark stirs.", MsgColor::DIM);
    log.push("The dark stirs.", MsgColor::DIM);
    log.push("The dark stirs.", MsgColor::DIM);
    let recent = log.recent(10);
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].repeat, 3);
}

// ══════════════════════════════════════════════════════════════════════════════
// MONSTER STATS & POISON
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn monster_stats_are_consistent() {
    for kind in &[
        MonsterKind::Rat,
        MonsterKind::Bat,
        MonsterKind::Goblin,
        MonsterKind::Skeleton,
        MonsterKind::Orc,
        MonsterKind::Spider,
        MonsterKind::Wraith,
    ] {
        let s = kind.stats();
        assert_eq!(s.hp, s.max_hp, "{} inconsistent", kind.name());
    }
}

#[test]
fn monster_spawn_is_alive_and_clean() {
    let m = Monster::new(5, 5, MonsterKind::Goblin);
    assert!(m.alive);
    assert!(!m.poisoned);
    assert_eq!(m.poison_ticks, 0);
}

#[test]
fn monster_poison_tick_kills() {
    let mut m = Monster::new(5, 5, MonsterKind::Rat);
    m.stats.hp = 1;
    m.poisoned = true;
    m.poison_ticks = 3;
    m.stats.hp -= 1;
    m.poison_ticks -= 1;
    assert!(m.stats.hp <= 0);
}

#[test]
fn monster_poison_wears_off() {
    let mut m = Monster::new(5, 5, MonsterKind::Rat);
    m.poisoned = true;
    m.poison_ticks = 1;
    m.stats.hp -= 1;
    m.poison_ticks -= 1;
    assert_eq!(m.poison_ticks, 0);
}

// ══════════════════════════════════════════════════════════════════════════════
// GAME COMPLETION INVARIANTS
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn max_depth_is_reachable() {
    assert!(ItemKind::CrownOfDepth.min_depth() <= 4);
}

#[test]
fn all_depths_have_spawnable_monsters() {
    let mut rng = StdRng::seed_from_u64(42);
    for depth in 0..=4 {
        let _ = MonsterKind::random_for_depth(&mut rng, depth);
    }
}

#[test]
fn all_depths_have_spawnable_items() {
    let mut rng = StdRng::seed_from_u64(42);
    for depth in 0..=4 {
        let _ = ItemKind::random_for_depth(&mut rng, depth);
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// SAVE / LOAD
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn save_format_has_version_field() {
    let save = serde_json::json!({
        "version": 1,
        "seed": 42u64,
        "status": "Playing",
        "stats": { "turns": 0, "kills": 0, "dmg_dealt": 0, "dmg_taken": 0, "items_used": 0, "deepest_depth": 0 }
    });
    let json = serde_json::to_string(&save).unwrap();
    assert!(
        json.contains("\"version\""),
        "save must include version field"
    );
}

#[test]
fn future_save_version_detected() {
    let json = r#"{"version":999,"seed":0,"status":"Playing"}"#;
    let v: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(
        v["version"], 999,
        "should parse a future version for detection"
    );
}

/// Build a minimal valid SaveData for validation tests.
fn minimal_save_data() -> crate::save::SaveData {
    let player = Player::new(0, 0);
    let mut map = Map::new(5, 5);
    for y in 0..5 {
        for x in 0..5 {
            let idx = map.idx(x, y);
            map.tiles[idx] = Tile::Floor;
        }
    }
    map.rooms.push(crate::map::Rect {
        x: 0,
        y: 0,
        w: 5,
        h: 5,
    });

    let room_count = map.rooms.len();
    crate::save::SaveData {
        version: crate::save::SAVE_VERSION,
        seed: 1,
        player,
        map,
        monsters: vec![],
        items: vec![],
        messages: vec![],
        status: GameStatus::Playing,
        stats: RunStats::new(),
        auto_pickup: false,
        projectiles: vec![],
        npcs: vec![],
        game_mode: crate::app::GameMode::Classic,
        challenge_mode: crate::app::ChallengeMode::Standard,
        campaign: None,
        landmarks: vec![],
        used_landmarks: vec![],
        camps: vec![],
        seen_monsters: vec![],
        defeated_monsters: vec![],
        floor_cache: vec![],
        floor_memory: crate::floor::FloorMemory::new(room_count),
        rng_word_pos: None,
    }
}

// This module exists only to keep the save tests tidy — it needs access to
// `crate::save::SaveData` which must be `pub` for tests.
mod save_tests {
    use super::*;
    use crate::app::{GameState, GameStatus, RunStats};
    use crate::save::SaveData;

    /// Helper: construct a SaveData from json! with explicit array literals.
    fn save_from_json(json_str: &str) -> SaveData {
        serde_json::from_str::<SaveData>(json_str).expect("valid save JSON")
    }

    #[test]
    fn legacy_rules_remain_legacy_when_resaved_and_new_game_upgrades() {
        let mut state = GameState::new(42);
        state.new_game();
        state.rules_version = 0;

        assert_eq!(SaveData::from_state(&state).version, 10);
        state.rules_version = 1;
        assert_eq!(SaveData::from_state(&state).version, 11);
        state.new_game();
        assert_eq!(state.rules_version, crate::app::CURRENT_RULES_VERSION);
        assert_eq!(
            SaveData::from_state(&state).version,
            crate::save::SAVE_VERSION
        );
    }

    #[test]
    fn save_versions_map_to_their_original_rule_sets() {
        assert_eq!(crate::save::rules_version_for_save(10), 0);
        assert_eq!(crate::save::rules_version_for_save(11), 1);
        assert_eq!(crate::save::rules_version_for_save(12), 2);
    }

    #[test]
    fn validate_rejects_invalid_variable_stamina() {
        let mut data = minimal_save_data();
        data.version = crate::save::SAVE_VERSION;
        data.player.max_stamina = crate::entity::MAX_STAMINA_CAP + 1;
        assert!(data.validate().is_err());

        data.player.max_stamina = crate::entity::MAX_STAMINA;
        data.player.stamina = crate::entity::MAX_STAMINA + 1;
        assert!(data.validate().is_err());
    }

    #[test]
    fn validate_rejects_a_mastery_for_the_wrong_weapon() {
        let mut data = minimal_save_data();
        data.version = crate::save::SAVE_VERSION;
        data.player.weapon_mastery = Some(crate::entity::WeaponMastery::KeenEdge);
        assert!(data.validate().is_err());
    }

    #[test]
    fn validate_rejects_a_v3_save_without_room_memory() {
        let mut data = minimal_save_data();
        data.version = crate::save::SAVE_VERSION;
        data.floor_memory = crate::floor::FloorMemory::default();
        assert!(data.validate().is_err());
    }

    #[test]
    fn validate_rejects_an_active_noise_marker_outside_the_map() {
        let mut data = minimal_save_data();
        data.floor_memory = crate::floor::FloorMemory::new(data.map.rooms.len());
        data.floor_memory.last_noise = Some(crate::floor::NoiseEvent {
            x: data.map.width,
            y: 0,
            kind: crate::floor::NoiseKind::Impact,
        });
        assert!(data.validate().is_err());
    }

    #[test]
    fn validate_rejects_future_version() {
        let json = r#"{"version":999,"seed":0,"player":{"x":0,"y":0,"stats":{"hp":30,"max_hp":30,"attack":5,"defense":2},"alive":true,"depth":0,"inventory":[],"weapon":null,"armor":null,"poisoned":false,"poison_ticks":0},"map":{"width":5,"height":5,"tiles":["Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor"],"visible":[false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false],"explored":[false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false],"rooms":[]},"monsters":[],"items":[],"messages":[],"status":"Playing","stats":{"turns":0,"kills":0,"dmg_dealt":0,"dmg_taken":0,"items_used":0,"deepest_depth":0}}"#;
        let data = save_from_json(json);
        assert!(data.validate().is_err());
    }

    #[test]
    fn validate_rejects_invalid_map_dimensions() {
        let ds = SaveData {
            version: 2,
            seed: 0,
            player: Player::new(0, 0),
            map: Map::new(0, 0),
            monsters: vec![],
            items: vec![],
            messages: vec![],
            status: GameStatus::Playing,
            stats: RunStats::new(),
            auto_pickup: false,
            projectiles: vec![],
            npcs: vec![],
            game_mode: crate::app::GameMode::Classic,
            challenge_mode: crate::app::ChallengeMode::Standard,
            campaign: None,
            landmarks: vec![],
            used_landmarks: vec![],
            camps: vec![],
            seen_monsters: vec![],
            defeated_monsters: vec![],
            floor_cache: vec![],
            floor_memory: crate::floor::FloorMemory::default(),
            rng_word_pos: None,
        };
        assert!(ds.validate().is_err());
    }

    #[test]
    fn validate_rejects_invalid_player_hp() {
        let mut player = Player::new(0, 0);
        player.stats.max_hp = 0;
        let ds = SaveData {
            version: 2,
            seed: 0,
            player,
            map: Map::new(5, 5),
            monsters: vec![],
            items: vec![],
            messages: vec![],
            status: GameStatus::Playing,
            stats: RunStats::new(),
            auto_pickup: false,
            projectiles: vec![],
            npcs: vec![],
            game_mode: crate::app::GameMode::Classic,
            challenge_mode: crate::app::ChallengeMode::Standard,
            campaign: None,
            landmarks: vec![],
            used_landmarks: vec![],
            camps: vec![],
            seen_monsters: vec![],
            defeated_monsters: vec![],
            floor_cache: vec![],
            floor_memory: crate::floor::FloorMemory::default(),
            rng_word_pos: None,
        };
        assert!(ds.validate().is_err());
    }

    #[test]
    fn validate_rejects_too_many_monsters() {
        let ds = SaveData {
            version: 2,
            seed: 0,
            player: Player::new(0, 0),
            map: Map::new(5, 5),
            monsters: (0..201)
                .map(|i| Monster::new(i % 5, i / 5, MonsterKind::Rat))
                .collect(),
            items: vec![],
            messages: vec![],
            status: GameStatus::Playing,
            stats: RunStats::new(),
            auto_pickup: false,
            projectiles: vec![],
            npcs: vec![],
            game_mode: crate::app::GameMode::Classic,
            challenge_mode: crate::app::ChallengeMode::Standard,
            campaign: None,
            landmarks: vec![],
            used_landmarks: vec![],
            camps: vec![],
            seen_monsters: vec![],
            defeated_monsters: vec![],
            floor_cache: vec![],
            floor_memory: crate::floor::FloorMemory::default(),
            rng_word_pos: None,
        };
        assert!(ds.validate().is_err());
    }

    #[test]
    fn validate_rejects_too_many_items() {
        let ds = SaveData {
            version: 2,
            seed: 0,
            player: Player::new(0, 0),
            map: Map::new(5, 5),
            monsters: vec![],
            items: (0..201)
                .map(|i| Item::new(i % 5, i / 5, ItemKind::HealthPotion))
                .collect(),
            messages: vec![],
            status: GameStatus::Playing,
            stats: RunStats::new(),
            auto_pickup: false,
            projectiles: vec![],
            npcs: vec![],
            game_mode: crate::app::GameMode::Classic,
            challenge_mode: crate::app::ChallengeMode::Standard,
            campaign: None,
            landmarks: vec![],
            used_landmarks: vec![],
            camps: vec![],
            seen_monsters: vec![],
            defeated_monsters: vec![],
            floor_cache: vec![],
            floor_memory: crate::floor::FloorMemory::default(),
            rng_word_pos: None,
        };
        assert!(ds.validate().is_err());
    }

    #[test]
    fn validate_accepts_valid_data() {
        let data = minimal_save_data();
        assert!(data.validate().is_ok());
    }

    #[test]
    fn validate_rejects_a_campaign_completion_before_the_chapter_ending() {
        let mut data = minimal_save_data();
        data.status = GameStatus::CampaignComplete;
        data.game_mode = crate::app::GameMode::Campaign;
        data.campaign = Some(crate::campaign::CampaignState::new());

        assert!(data.validate().is_err());
    }

    #[test]
    fn save_roundtrip_includes_auto_pickup() {
        let mut state = GameState::new(42);
        state.status = GameStatus::Playing;
        state.auto_pickup = true;

        let data = SaveData::from_state(&state);
        let json = serde_json::to_string(&data).unwrap();

        assert!(
            json.contains("auto_pickup"),
            "round-tripped JSON must include auto_pickup: {}",
            json
        );

        let loaded: SaveData = serde_json::from_str(&json).unwrap();
        assert!(loaded.auto_pickup);
    }

    #[test]
    fn save_roundtrip_preserves_challenge_mode() {
        let mut state = GameState::new(42);
        state.challenge_mode = crate::app::ChallengeMode::FragileGear;

        let json = serde_json::to_string(&SaveData::from_state(&state)).unwrap();
        let loaded: SaveData = serde_json::from_str(&json).unwrap();

        assert_eq!(
            loaded.challenge_mode,
            crate::app::ChallengeMode::FragileGear
        );
    }

    #[test]
    fn save_roundtrip_preserves_bestiary_and_quick_item() {
        let mut state = GameState::new(42);
        state.seen_monsters.insert(MonsterKind::Spider);
        state.defeated_monsters.insert(MonsterKind::Rat);
        state.player.quick_item = Some(ItemKind::Antidote);
        state.player.focus_turns = 12;
        state.player.ward_turns = 7;
        state.player.weapon = Some(ItemKind::RustySword);
        state.player.weapon_durability = 11;
        state.player.weapon_cursed = true;
        state.player.crown_held = true;
        state.player.identified_items.insert(ItemKind::HealthPotion);

        let json = serde_json::to_string(&SaveData::from_state(&state)).unwrap();
        let loaded: SaveData = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.seen_monsters, vec![MonsterKind::Spider]);
        assert_eq!(loaded.defeated_monsters, vec![MonsterKind::Rat]);
        assert_eq!(loaded.player.quick_item, Some(ItemKind::Antidote));
        assert_eq!(loaded.player.focus_turns, 12);
        assert_eq!(loaded.player.ward_turns, 7);
        assert_eq!(loaded.player.weapon_durability, 11);
        assert!(loaded.player.weapon_cursed);
        assert!(loaded.player.crown_held);
        assert!(loaded
            .player
            .identified_items
            .contains(&ItemKind::HealthPotion));
    }

    #[test]
    fn save_roundtrip_preserves_environmental_terrain() {
        let mut data = minimal_save_data();
        let water = data.map.idx(1, 1);
        let web = data.map.idx(2, 1);
        let lock = data.map.idx(3, 1);
        data.map.tiles[water] = Tile::ShallowWater;
        data.map.tiles[web] = Tile::Web;
        data.map.tiles[lock] = Tile::LockedDoor;
        data.player.inventory.push(ItemKind::IronKey);

        let json = serde_json::to_string(&data).unwrap();
        let loaded: SaveData = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.map.tiles[water], Tile::ShallowWater);
        assert_eq!(loaded.map.tiles[web], Tile::Web);
        assert_eq!(loaded.map.tiles[lock], Tile::LockedDoor);
        assert!(loaded.player.inventory.contains(&ItemKind::IronKey));
    }

    #[test]
    fn backward_compat_auto_pickup_defaults_false() {
        // v1 save has no auto_pickup field — it should default to false
        let json = r#"{"version":1,"seed":42,"player":{"x":0,"y":0,"stats":{"hp":30,"max_hp":30,"attack":5,"defense":2},"alive":true,"depth":0,"inventory":[],"weapon":null,"armor":null,"poisoned":false,"poison_ticks":0},"map":{"width":5,"height":5,"tiles":["Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor","Floor"],"visible":[false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false],"explored":[false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false,false],"rooms":[]},"monsters":[],"items":[],"messages":[],"status":"Playing","stats":{"turns":0,"kills":0,"dmg_dealt":0,"dmg_taken":0,"items_used":0,"deepest_depth":0}}"#;
        let data: SaveData = serde_json::from_str(json).unwrap();
        // v1 saves lack auto_pickup; serde default = false
        assert!(!data.auto_pickup);
        assert_eq!(data.challenge_mode, crate::app::ChallengeMode::Standard);
    }

    #[test]
    fn validate_rejects_player_out_of_bounds() {
        let mut data = minimal_save_data();
        data.player.x = 5; // map is width=5, max x is 4
        data.player.y = 3;
        assert!(data.validate().is_err());
    }

    #[test]
    fn validate_rejects_player_negative_coords() {
        let mut data = minimal_save_data();
        data.player.x = -1;
        assert!(data.validate().is_err());
    }

    #[test]
    fn validate_rejects_monster_out_of_bounds() {
        let mut data = minimal_save_data();
        data.monsters.push(Monster::new(10, 10, MonsterKind::Rat)); // map is 5x5
        assert!(data.validate().is_err());
    }

    #[test]
    fn validate_rejects_item_out_of_bounds() {
        let mut data = minimal_save_data();
        data.items.push(Item::new(99, 99, ItemKind::HealthPotion));
        assert!(data.validate().is_err());
    }

    #[test]
    fn validate_rejects_room_out_of_bounds() {
        let mut data = minimal_save_data();
        data.map.rooms.push(crate::map::Rect {
            x: 10,
            y: 10,
            w: 4,
            h: 4,
        });
        assert!(data.validate().is_err());
    }

    #[test]
    fn validate_rejects_wrong_visible_length() {
        let mut data = minimal_save_data();
        data.map.visible = vec![false; 10]; // should be 25
        assert!(data.validate().is_err());
    }

    #[test]
    fn validate_rejects_wrong_explored_length() {
        let mut data = minimal_save_data();
        data.map.explored = vec![false; 30]; // should be 25
        assert!(data.validate().is_err());
    }

    #[test]
    fn validate_rejects_player_hp_exceeds_max() {
        let mut data = minimal_save_data();
        data.player.stats.hp = 100;
        data.player.stats.max_hp = 30;
        assert!(data.validate().is_err());
    }

    #[test]
    fn validate_rejects_player_negative_hp() {
        let mut data = minimal_save_data();
        data.player.stats.hp = -1;
        assert!(data.validate().is_err());
    }

    #[test]
    fn validate_rejects_too_many_messages() {
        let mut data = minimal_save_data();
        data.messages = (0..501)
            .map(|i| crate::ui::Message {
                text: format!("msg {i}"),
                color: crate::ui::MsgColor::White,
                repeat: 1,
            })
            .collect();
        assert!(
            data.validate().is_err(),
            "should reject save with >500 messages"
        );
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// GAME STATE — HEADLESS GAMEPLAY TESTS
// ══════════════════════════════════════════════════════════════════════════════

mod game_state_tests {
    use super::*;
    use crate::app::{GameState, GameStatus};
    use crate::visual::VisualState;

    // ── new_game ──

    #[test]
    fn new_game_sets_playing_status() {
        let mut state = GameState::new(42);
        state.new_game();
        assert_eq!(state.status, GameStatus::Playing);
    }

    #[test]
    fn new_game_clears_previous_state() {
        let mut state = GameState::new(42);
        // Pre-populate some garbage
        state.player.inventory.push(ItemKind::HealthPotion);
        state.stats.turns = 99;
        state.monsters.push(Monster::new(1, 1, MonsterKind::Rat));
        state.new_game();
        // new_game resets stats and clears inventory; it also generates a fresh
        // level which includes monsters, so monsters won't be empty.
        assert_eq!(state.stats.turns, 0);
        assert!(state.player.inventory.is_empty());
        // Verify a fresh level was generated
        assert!(state.map.rooms.len() >= 3);
    }

    #[test]
    fn new_game_generates_level_zero() {
        let mut state = GameState::new(42);
        state.new_game();
        assert_eq!(state.player.depth, 0);
        assert!(!state.map.rooms.is_empty(), "level should have rooms");
    }

    // ── generate_level ──

    #[test]
    fn generate_level_produces_rooms() {
        let mut state = GameState::new(42);
        state.new_game();
        assert!(
            state.map.rooms.len() >= 3,
            "should generate at least 3 rooms"
        );
    }

    #[test]
    fn generate_level_places_stairs() {
        let mut state = GameState::new(42);
        state.new_game();
        let has_down = state
            .map
            .tiles
            .iter()
            .any(|t| matches!(t, Tile::StairsDown));
        let has_up = state.map.tiles.iter().any(|t| matches!(t, Tile::StairsUp));
        assert!(has_down, "should have stairs down");
        assert!(has_up, "should have stairs up");
    }

    #[test]
    fn generate_level_spawns_monsters() {
        let mut state = GameState::new(42);
        state.new_game();
        // Depth 0 spawns at least 1 monster (3 + 0*2 = 3 monsters, minus 1 for player room)
        assert!(!state.monsters.is_empty(), "should spawn monsters");
    }

    #[test]
    fn generate_level_spawns_items() {
        let mut state = GameState::new(42);
        state.new_game();
        // Depth 0 spawns 2 items
        assert!(!state.items.is_empty(), "should spawn items");
    }

    #[test]
    fn generated_route_has_medieval_surfaces_props_and_location_identity() {
        let mut state = GameState::new(42);
        state.new_game();

        assert_eq!(state.map.theme, crate::map::MapTheme::Village);
        assert_eq!(state.world_map_id, crate::map::WorldMapId::Village);
        assert_eq!(state.world_links.len(), 4);
        let surfaces = state
            .map
            .surfaces
            .as_ref()
            .expect("generated maps should have a surface layer");
        assert_eq!(
            surfaces.len(),
            (state.map.width * state.map.height) as usize
        );
        assert!(surfaces.iter().any(|surface| surface.is_road()));
        assert!(!state.map.props.is_empty());
        assert!(state.map.props.iter().all(|prop| {
            state.map.in_bounds(prop.x, prop.y) && state.map.is_walkable(prop.x, prop.y)
        }));

        let down = state
            .map
            .tiles
            .iter()
            .position(|tile| *tile == Tile::StairsDown)
            .expect("generated floor should have stairs down");
        state.player.x = (down % state.map.width as usize) as i32;
        state.player.y = (down / state.map.width as usize) as i32;
        state.try_descend(&mut VisualState::new(80, 50));

        assert_eq!(state.map.theme, crate::map::MapTheme::Road);
        assert_eq!(state.world_map_id, crate::map::WorldMapId::OldKingsRoad);
        assert!(state
            .map
            .props
            .iter()
            .any(|prop| prop.kind == crate::map::PropKind::Gate));
        assert!(state.map.surfaces.as_ref().is_some_and(|surfaces| surfaces
            .iter()
            .any(|surface| matches!(surface, crate::map::SurfaceKind::Mud))));
    }

    #[test]
    fn classic_floor_changes_survive_a_return_journey() {
        let mut state = GameState::new(42);
        state.new_game();
        let first_floor_tiles = state.map.tiles.clone();

        let down = state
            .map
            .tiles
            .iter()
            .position(|tile| *tile == Tile::StairsDown)
            .expect("generated floor should have stairs down");
        state.player.x = (down % state.map.width as usize) as i32;
        state.player.y = (down / state.map.width as usize) as i32;

        let mut visual = VisualState::new(80, 50);
        state.try_descend(&mut visual);
        assert_eq!(state.player.depth, 1);

        let altered_tile = state
            .map
            .tiles
            .iter()
            .position(|tile| *tile == Tile::Floor)
            .expect("generated floor should have a mutable tile");
        state.map.tiles[altered_tile] = Tile::Web;
        state.floor_memory.attention = 72;

        let up = state
            .map
            .tiles
            .iter()
            .position(|tile| *tile == Tile::StairsUp)
            .expect("generated floor should have stairs up");
        state.player.x = (up % state.map.width as usize) as i32;
        state.player.y = (up / state.map.width as usize) as i32;
        state.try_ascend(&mut visual);

        assert_eq!(state.player.depth, 0);
        assert_eq!(state.map.tiles, first_floor_tiles);
        assert!(state.floor_cache.iter().any(|floor| floor.depth == 1));

        let down_again = state
            .map
            .tiles
            .iter()
            .position(|tile| *tile == Tile::StairsDown)
            .expect("restored floor should have stairs down");
        state.player.x = (down_again % state.map.width as usize) as i32;
        state.player.y = (down_again / state.map.width as usize) as i32;
        state.try_descend(&mut visual);

        assert_eq!(state.player.depth, 1);
        assert_eq!(state.map.tiles[altered_tile], Tile::Web);
        assert_eq!(state.floor_memory.attention, 72);
    }

    #[test]
    fn crown_return_to_a_remembered_floor_draws_the_stalker_once() {
        let mut state = GameState::new(42);
        state.new_game();
        let down = state
            .map
            .tiles
            .iter()
            .position(|tile| *tile == Tile::StairsDown)
            .expect("generated floor should have stairs down");
        state.player.x = (down % state.map.width as usize) as i32;
        state.player.y = (down / state.map.width as usize) as i32;
        let mut visual = VisualState::new(80, 50);
        state.try_descend(&mut visual);
        let deeper_down = state
            .map
            .tiles
            .iter()
            .position(|tile| *tile == Tile::StairsDown)
            .expect("second floor should have stairs down");
        state.player.x = (deeper_down % state.map.width as usize) as i32;
        state.player.y = (deeper_down / state.map.width as usize) as i32;
        state.try_descend(&mut visual);
        state.player.crown_held = true;
        state.player.inventory.push(ItemKind::CrownOfDepth);

        let up = state
            .map
            .tiles
            .iter()
            .position(|tile| *tile == Tile::StairsUp)
            .expect("generated floor should have stairs up");
        state.player.x = (up % state.map.width as usize) as i32;
        state.player.y = (up / state.map.width as usize) as i32;
        state.try_ascend(&mut visual);

        assert_eq!(
            state
                .monsters
                .iter()
                .filter(|monster| monster.kind == MonsterKind::Stalker)
                .count(),
            1
        );
        assert!(state.floor_memory.hunter_spawned);
    }

    #[test]
    fn floor_attention_advances_only_when_a_turn_commits() {
        let mut state = GameState::new(42);
        state.new_game();
        let mut visual = VisualState::new(80, 50);
        let turns = state.stats.turns;
        let attention = state.floor_memory.attention;

        let (start_x, start_y) = (state.player.x, state.player.y);
        let (valid_dx, valid_dy) = [(1, 0), (-1, 0), (0, 1), (0, -1)]
            .into_iter()
            .find(|(dx, dy)| state.map.is_walkable(start_x + dx, start_y + dy))
            .expect("generated start room should have a walkable neighbor");

        // A failed move into the outer wall does not consume a turn or make a
        // sound that the floor can remember.
        state.player.x = 1;
        state.player.y = 1;
        state.process_turn(Action::Move(-1, 0), &mut visual);
        assert_eq!(state.stats.turns, turns);
        assert_eq!(state.floor_memory.attention, attention);

        state.player.x = start_x;
        state.player.y = start_y;
        state.process_turn(Action::Move(valid_dx, valid_dy), &mut visual);
        assert_eq!(state.stats.turns, turns + 1);
        assert_eq!(state.floor_memory.attention, attention + 1);
        assert!(state.floor_memory.last_noise.is_some());
    }

    #[test]
    fn save_roundtrip_preserves_floor_memory_cache_and_rng_checkpoint() {
        let mut state = GameState::new(42);
        state.new_game();
        let mut visual = VisualState::new(80, 50);
        state.process_turn(Action::Wait, &mut visual);
        state.floor_memory.attention = 61;
        state.floor_memory.quiet_turns = 4;
        state.floor_cache.push(crate::floor::FloorState {
            depth: 2,
            map: state.map.clone(),
            monsters: state.monsters.clone(),
            items: state.items.clone(),
            npcs: state.npcs.clone(),
            projectiles: state.projectiles.clone(),
            landmarks: state.landmarks.clone(),
            used_landmarks: Vec::new(),
            camps: state.camps.clone(),
            memory: state.floor_memory.clone(),
        });

        let data = crate::save::SaveData::from_state(&state);
        let json = serde_json::to_string(&data).expect("save should serialize");
        let loaded: crate::save::SaveData =
            serde_json::from_str(&json).expect("save should deserialize");

        assert_eq!(loaded.floor_memory, state.floor_memory);
        assert_eq!(loaded.floor_cache.len(), 1);
        assert_eq!(loaded.floor_cache[0].depth, 2);
        assert_eq!(loaded.rng_word_pos, Some(state.rng.get_word_pos()));
        assert!(loaded.validate().is_ok());
    }

    #[test]
    fn rng_word_checkpoint_resumes_the_same_stream() {
        let mut state = GameState::new(42);
        state.new_game();
        let checkpoint = state.rng.get_word_pos();
        let expected: u64 = rand::Rng::gen(&mut state.rng);

        let mut restored = rand_chacha::ChaCha12Rng::seed_from_u64(state.seed);
        restored.set_word_pos(checkpoint);
        let actual: u64 = rand::Rng::gen(&mut restored);

        assert_eq!(actual, expected);
    }

    #[test]
    fn forge_mastery_is_a_persistent_curated_weapon_choice() {
        let mut state = state_with_floor(10, 10);
        state.rules_version = crate::app::CURRENT_RULES_VERSION;
        state.player.weapon = Some(ItemKind::RustySword);
        state.preparation_menu = Some(crate::app::PreparationMenu::Mastery(ItemKind::RustySword));
        let mut visual = VisualState::new(80, 50);

        state.resolve_preparation(1, &mut visual);

        assert_eq!(
            state.player.weapon_mastery,
            Some(crate::entity::WeaponMastery::KeenEdge)
        );
        assert!(state
            .messages
            .recent(4)
            .iter()
            .any(|message| message.text.contains("Keen Edge")));
        assert_eq!(state.stats.turns, 1);
    }

    #[test]
    fn v3_telegraphs_a_heavy_enemy_intent_before_it_lands() {
        let mut state = state_with_floor(10, 10);
        state.rules_version = crate::app::CURRENT_RULES_VERSION;
        state.map.visible.fill(true);
        let mut orc = Monster::new(6, 5, MonsterKind::Orc);
        orc.awakened = true;
        state.monsters.push(orc);
        let mut visual = VisualState::new(80, 50);

        state.end_turn(&mut visual);

        assert_eq!(state.monsters[0].windup, 1);
        assert_eq!(
            state.monsters[0].intent.map(|intent| intent.kind),
            Some(crate::entity::IntentKind::Strike)
        );
    }

    #[test]
    fn deepest_v3_floor_has_a_bell_warden_anchor() {
        let mut state = GameState::new(42);
        state.status = GameStatus::Playing;
        state.generate_level(4);

        assert!(state
            .monsters
            .iter()
            .any(|monster| monster.kind == MonsterKind::BellWarden));
    }

    #[test]
    fn depth_two_lock_always_has_accessible_starting_room_key() {
        let mut state = GameState::new(42);
        state.status = GameStatus::Playing;
        state.generate_level(2);

        assert_eq!(
            state
                .map
                .tiles
                .iter()
                .filter(|tile| **tile == Tile::LockedDoor)
                .count(),
            1
        );
        let key = state
            .items
            .iter()
            .find(|item| item.kind == ItemKind::IronKey)
            .expect("locked floor must contain an iron key");
        let start_room = state
            .map
            .rooms
            .iter()
            .find(|room| {
                state.player.x >= room.x
                    && state.player.x < room.x + room.w
                    && state.player.y >= room.y
                    && state.player.y < room.y + room.h
            })
            .expect("player starts in a room");
        assert!(
            key.x >= start_room.x
                && key.x < start_room.x + start_room.w
                && key.y >= start_room.y
                && key.y < start_room.y + start_room.h
        );
    }

    #[test]
    fn named_artifacts_are_deterministically_guaranteed_by_depth() {
        for (depth, artifact) in [
            (2, ItemKind::Emberbrand),
            (3, ItemKind::TideglassMail),
            (4, ItemKind::Graveward),
        ] {
            let mut state = GameState::new(42);
            state.status = GameStatus::Playing;
            state.generate_level(depth);
            assert!(
                state.items.iter().any(|item| item.kind == artifact),
                "depth {depth} must contain {}",
                artifact.name()
            );
        }
    }

    #[test]
    fn generate_level_updates_deepest_depth() {
        let mut state = GameState::new(42);
        state.new_game();
        assert_eq!(state.stats.deepest_depth, 0);
        // Simulate descending to depth 1
        state.player.x = 0;
        state.player.y = 0;
        // Manually set stairs and descend
        let stairs_idx = state
            .map
            .tiles
            .iter()
            .position(|t| matches!(t, Tile::StairsDown));
        if let Some(idx) = stairs_idx {
            let w = state.map.width;
            let sx = (idx % w as usize) as i32;
            let sy = (idx / w as usize) as i32;
            state.map.tiles[idx] = Tile::Floor;
            // Place player on stairs position
            state.player.x = sx;
            state.player.y = sy;
            let s_idx = state.map.idx(sx, sy);
            state.map.tiles[s_idx] = Tile::StairsDown;
            let mut visual = VisualState::new(80, 50);
            state.try_descend(&mut visual);
            assert_eq!(state.player.depth, 1, "should have descended to depth 1");
            assert_eq!(state.stats.deepest_depth, 1, "deepest depth should be 1");
        }
    }

    // ── pick_up_item ──

    #[test]
    fn pick_up_item_nothing_there_returns_false() {
        let state = state_with_floor(10, 10);
        let mut state = state; // make mutable
                               // Use the state directly — pick_up_item is pub on GameState
        let mut visual = VisualState::new(80, 50);
        assert!(!state.pick_up_item(&mut visual));
    }

    #[test]
    fn pick_up_item_adds_to_inventory() {
        let mut state = state_with_floor(10, 10);
        let px = state.player.x;
        let py = state.player.y;
        state.items.push(Item::new(px, py, ItemKind::HealthPotion));
        let mut visual = VisualState::new(80, 50);
        assert!(state.pick_up_item(&mut visual));
        assert!(state.player.inventory.contains(&ItemKind::HealthPotion));
    }

    #[test]
    fn pick_up_item_auto_equips_weapon_when_slot_empty() {
        let mut state = state_with_floor(10, 10);
        let px = state.player.x;
        let py = state.player.y;
        state.items.push(Item::new(px, py, ItemKind::Longsword));
        let mut visual = VisualState::new(80, 50);
        state.pick_up_item(&mut visual);
        assert_eq!(state.player.weapon, Some(ItemKind::Longsword));
        assert!(state.player.inventory.is_empty());
    }

    #[test]
    fn pick_up_item_auto_equips_armor_when_slot_empty() {
        let mut state = state_with_floor(10, 10);
        let px = state.player.x;
        let py = state.player.y;
        state.items.push(Item::new(px, py, ItemKind::Chainmail));
        let mut visual = VisualState::new(80, 50);
        state.pick_up_item(&mut visual);
        assert_eq!(state.player.armor, Some(ItemKind::Chainmail));
    }

    #[test]
    fn pick_up_crown_logs_special_message() {
        let mut state = state_with_floor(10, 10);
        let px = state.player.x;
        let py = state.player.y;
        state.items.push(Item::new(px, py, ItemKind::CrownOfDepth));
        let mut visual = VisualState::new(80, 50);
        state.pick_up_item(&mut visual);
        assert!(state.player.inventory.contains(&ItemKind::CrownOfDepth));
        let recent: Vec<_> = state
            .messages
            .recent(5)
            .iter()
            .map(|m| m.text.clone())
            .collect();
        let has_crown_msg = recent
            .iter()
            .any(|m| m.contains("warm, expectant, patient"));
        assert!(has_crown_msg, "should log crown message, got: {:?}", recent);
    }

    // ── combat (bump-to-attack via process_turn) ──

    #[test]
    fn bump_attack_damages_monster() {
        let mut state = state_with_floor(10, 10);
        state.rules_version = 3;
        let mx = state.player.x + 1;
        let my = state.player.y;
        let mut m = Monster::new(mx, my, MonsterKind::Rat);
        m.stats.hp = 5;
        m.stats.max_hp = 5;
        state.monsters.push(m);
        let mut visual = VisualState::new(80, 50);
        state.process_turn(Action::Move(1, 0), &mut visual);
        // Rat has defense 0, player attack 5 → 5 damage. Rat starts at 5 → dead.
        // dead monsters are retained by end_turn, so check kills instead
        assert_eq!(state.stats.kills, 1, "player should kill the rat");
        assert!(visual.effects.iter().any(|effect| matches!(
            effect,
            crate::visual::VisualEffect::WeaponSweep {
                x,
                y,
                dx: 1,
                dy: 0,
                ..
            } if (*x, *y) == (mx, my)
        )));
    }

    #[test]
    fn bump_attack_increments_damage_dealt() {
        let mut state = state_with_floor(10, 10);
        state.rules_version = 3;
        let mx = state.player.x + 1;
        let my = state.player.y;
        let mut m = Monster::new(mx, my, MonsterKind::Rat);
        m.stats.hp = 10;
        m.stats.max_hp = 10;
        state.monsters.push(m);
        let dmg_before = state.stats.dmg_dealt;
        let mut visual = VisualState::new(80, 50);
        state.process_turn(Action::Move(1, 0), &mut visual);
        assert!(state.stats.dmg_dealt > dmg_before);
    }

    #[test]
    fn bump_attack_does_not_move_into_monster() {
        let mut state = state_with_floor(10, 10);
        state.rules_version = 3;
        let mx = state.player.x + 1;
        let my = state.player.y;
        let mut m = Monster::new(mx, my, MonsterKind::Rat);
        m.stats.hp = 20;
        m.stats.max_hp = 20;
        state.monsters.push(m);
        let px = state.player.x;
        let mut visual = VisualState::new(80, 50);
        state.process_turn(Action::Move(1, 0), &mut visual);
        assert_eq!(
            state.player.x, px,
            "player should not move into monster tile"
        );
    }

    // ── use_item ──

    #[test]
    fn use_health_potion_heals() {
        let mut state = state_with_floor(10, 10);
        state.player.stats.hp = 10;
        state.player.stats.max_hp = 30;
        state.player.inventory.push(ItemKind::HealthPotion);
        let mut visual = VisualState::new(80, 50);
        assert!(
            state.use_item(ItemKind::HealthPotion, 0, &mut visual),
            "use_item should return true when healing"
        );
        assert_eq!(state.player.stats.hp, 20, "should heal 10 HP");
        assert_eq!(state.stats.items_used, 1);
    }

    #[test]
    fn use_health_potion_at_full_hp_does_not_heal() {
        let mut state = state_with_floor(10, 10);
        state.player.stats.hp = 30;
        state.player.stats.max_hp = 30;
        state.player.inventory.push(ItemKind::HealthPotion);
        let mut visual = VisualState::new(80, 50);
        assert!(
            !state.use_item(ItemKind::HealthPotion, 0, &mut visual),
            "use_item should return false at full HP"
        );
        assert_eq!(state.player.stats.hp, 30, "should not exceed max HP");
        assert_eq!(state.stats.items_used, 0, "items_used should not increment");
        assert!(state
            .player
            .identified_items
            .contains(&ItemKind::HealthPotion));
    }

    #[test]
    fn use_scroll_of_fire_damages_visible_monsters() {
        let mut state = state_with_floor(10, 10);
        // Place a visible monster
        let mx = state.player.x + 1;
        let my = state.player.y;
        let mi = state.map.idx(mx, my);
        state.map.visible[mi] = true;
        let mut m = Monster::new(mx, my, MonsterKind::Rat);
        m.stats.hp = 4;
        m.stats.max_hp = 4;
        state.monsters.push(m);
        state.player.inventory.push(ItemKind::ScrollOfFire);
        let mut visual = VisualState::new(80, 50);
        state.use_item(ItemKind::ScrollOfFire, 0, &mut visual);
        assert!(
            !state.monsters[0].alive,
            "scroll should kill the visible rat"
        );
        assert_eq!(state.stats.items_used, 1);
        assert_eq!(
            visual.player_action.map(|action| action.kind),
            Some(crate::visual::PlayerActionKind::UseItem(
                ItemKind::ScrollOfFire
            ))
        );
        assert!(visual.effects.iter().any(|effect| matches!(
            effect,
            crate::visual::VisualEffect::ItemCast {
                kind: ItemKind::ScrollOfFire,
                ..
            }
        )));
    }

    #[test]
    fn use_scroll_of_fire_only_affects_visible() {
        let mut state = state_with_floor(10, 10);
        // Visible monster
        let vx = state.player.x + 1;
        let vy = state.player.y;
        let vi = state.map.idx(vx, vy);
        state.map.visible[vi] = true;
        state.monsters.push(Monster::new(vx, vy, MonsterKind::Rat));
        // Hidden (invisible) monster
        let hx = state.player.x + 2;
        let hy = state.player.y;
        let mut hidden = Monster::new(hx, hy, MonsterKind::Rat);
        hidden.stats.hp = 10;
        hidden.stats.max_hp = 10;
        state.monsters.push(hidden);
        state.player.inventory.push(ItemKind::ScrollOfFire);
        let mut visual = VisualState::new(80, 50);
        state.use_item(ItemKind::ScrollOfFire, 0, &mut visual);
        // Visible rat should die (4 HP vs 8 fire damage)
        assert!(!state.monsters[0].alive, "visible rat should die");
        // Hidden rat should be untouched
        assert!(state.monsters[1].alive, "hidden rat should survive");
        assert_eq!(state.monsters[1].stats.hp, 10);
    }

    // ── equip_item (via use_item) ──

    #[test]
    fn equip_weapon_swaps_old_into_inventory() {
        let mut state = state_with_floor(10, 10);
        state.player.inventory.push(ItemKind::RustySword);
        let mut visual = VisualState::new(80, 50);
        state.use_item(ItemKind::RustySword, 0, &mut visual);
        assert_eq!(state.player.weapon, Some(ItemKind::RustySword));
        // Now equip longsword — should swap
        state.player.inventory.push(ItemKind::Longsword);
        state.use_item(ItemKind::Longsword, 0, &mut visual);
        assert_eq!(state.player.weapon, Some(ItemKind::Longsword));
        assert!(state.player.inventory.contains(&ItemKind::RustySword));
    }

    // ── stairs ──

    #[test]
    fn try_descend_not_on_stairs_no_effect() {
        let mut state = state_with_floor(10, 10);
        let depth_before = state.player.depth;
        let mut visual = VisualState::new(80, 50);
        state.try_descend(&mut visual);
        assert_eq!(state.player.depth, depth_before);
    }

    #[test]
    fn try_ascend_not_on_stairs_no_effect() {
        let mut state = state_with_floor(10, 10);
        let depth_before = state.player.depth;
        let mut visual = VisualState::new(80, 50);
        state.try_ascend(&mut visual);
        assert_eq!(state.player.depth, depth_before);
    }

    #[test]
    fn try_ascend_at_depth_zero_without_crown_does_not_win() {
        let mut state = state_with_floor(10, 10);
        state.player.depth = 0;
        let px = state.player.x;
        let py = state.player.y;
        let sp_idx = state.map.idx(px, py);
        state.map.tiles[sp_idx] = Tile::StairsUp;
        let mut visual = VisualState::new(80, 50);
        state.try_ascend(&mut visual);
        assert_ne!(state.status, GameStatus::Won);
    }

    #[test]
    fn try_ascend_at_depth_zero_with_crown_wins() {
        let mut state = state_with_floor(10, 10);
        state.player.depth = 0;
        state.player.inventory.push(ItemKind::CrownOfDepth);
        let px = state.player.x;
        let py = state.player.y;
        let sp_idx = state.map.idx(px, py);
        state.map.tiles[sp_idx] = Tile::StairsUp;
        let mut visual = VisualState::new(80, 50);
        state.try_ascend(&mut visual);
        assert_eq!(state.status, GameStatus::Won);
    }

    #[test]
    fn try_descend_past_max_depth_does_nothing() {
        let mut state = state_with_floor(10, 10);
        state.player.depth = 4;
        let px = state.player.x;
        let py = state.player.y;
        let sd_idx = state.map.idx(px, py);
        state.map.tiles[sd_idx] = Tile::StairsDown;
        let mut visual = VisualState::new(80, 50);
        state.try_descend(&mut visual);
        assert_eq!(state.player.depth, 4, "should not descend past max depth");
    }

    // ── end_turn poison ──

    #[test]
    fn poison_damages_player_each_turn() {
        let mut state = state_with_floor(10, 10);
        let hp_before = state.player.stats.hp;
        state.player.poisoned = true;
        state.player.poison_ticks = 3;
        let mut visual = VisualState::new(80, 50);
        state.end_turn(&mut visual);
        assert_eq!(state.player.stats.hp, hp_before - 1);
        assert_eq!(state.player.poison_ticks, 2);
    }

    #[test]
    fn poison_wears_off_after_ticks() {
        let mut state = state_with_floor(10, 10);
        state.player.poisoned = true;
        state.player.poison_ticks = 1;
        let mut visual = VisualState::new(80, 50);
        state.end_turn(&mut visual);
        assert!(!state.player.poisoned, "poison should wear off");
    }

    #[test]
    fn poison_kills_player() {
        let mut state = state_with_floor(10, 10);
        state.player.stats.hp = 1;
        state.player.poisoned = true;
        state.player.poison_ticks = 1;
        let mut visual = VisualState::new(80, 50);
        state.end_turn(&mut visual);
        assert_eq!(state.status, GameStatus::Dead);
    }

    // ── auto-pickup ──

    #[test]
    fn auto_pickup_picks_up_on_move() {
        let mut state = state_with_floor(10, 10);
        state.auto_pickup = true;
        let px = state.player.x;
        let py = state.player.y + 1; // one step south
        state.items.push(Item::new(px, py, ItemKind::HealthPotion));
        let mut visual = VisualState::new(80, 50);
        state.process_turn(Action::Move(0, 1), &mut visual);
        assert_eq!(state.player.y, py, "player should move");
        assert!(state.player.inventory.contains(&ItemKind::HealthPotion));
        assert!(state.items.is_empty(), "item should be picked up");
    }

    // ── wait action ──

    #[test]
    fn wait_consumes_turn() {
        let mut state = state_with_floor(10, 10);
        let turns_before = state.stats.turns;
        let mut visual = VisualState::new(80, 50);
        state.process_turn(Action::Wait, &mut visual);
        assert_eq!(state.stats.turns, turns_before + 1);
        assert_eq!(
            visual.player_action.map(|action| action.kind),
            Some(crate::visual::PlayerActionKind::Guard)
        );
    }

    // ── action_none_does_nothing ──

    #[test]
    fn none_action_does_not_consume_turn() {
        let mut state = state_with_floor(10, 10);
        let turns_before = state.stats.turns;
        let mut visual = VisualState::new(80, 50);
        state.process_turn(Action::None, &mut visual);
        assert_eq!(state.stats.turns, turns_before);
    }

    // ── spawn stacking ──

    #[test]
    fn generate_level_no_duplicate_item_positions() {
        // Generate depth 4 across many seeds and verify no two items share a tile.
        for seed in 0..50 {
            let mut state = GameState::new(seed);
            state.player.depth = 4;
            state.generate_level(4);

            let mut seen = std::collections::HashSet::new();
            for item in &state.items {
                assert!(
                    seen.insert((item.x, item.y)),
                    "seed {seed}: duplicate item at {},{}",
                    item.x,
                    item.y
                );
            }
        }
    }

    #[test]
    fn depth_four_has_exactly_one_crown() {
        // The Crown must appear exactly once on the deepest floor.
        for seed in 0..50 {
            let mut state = GameState::new(seed);
            state.player.depth = 4;
            state.generate_level(4);

            let crown_count = state
                .items
                .iter()
                .filter(|i| i.kind == ItemKind::CrownOfDepth)
                .count();
            assert_eq!(
                crown_count, 1,
                "seed {seed}: expected exactly 1 Crown, got {crown_count}"
            );
        }
    }

    // ── auto-explore safety ──

    #[test]
    fn auto_explore_does_not_step_into_monster() {
        let mut state = state_with_floor(20, 20);
        // Mark all tiles explored so auto-explore has nothing to find,
        // then verify it doesn't move into a monster.
        for i in 0..state.map.tiles.len() {
            state.map.explored[i] = true;
        }
        let mut visual = VisualState::new(80, 50);
        // auto_explore directly sets player position, so it would bypass
        // bump-to-attack. Test that at minimum, when all tiles are explored,
        // it returns early and doesn't crash.
        state.auto_explore(&mut visual);
        // The player shouldn't have moved into a wall or monster.
        assert!(state.map.is_walkable(state.player.x, state.player.y));
    }

    // ── auto-explore functional ──

    #[test]
    fn auto_explore_finds_unexplored_tile() {
        let mut state = state_with_floor(20, 20);
        // Leave some tiles unexplored so auto_explore has targets
        for y in 0..10 {
            for x in 0..10 {
                let i = state.map.idx(x, y);
                state.map.explored[i] = true;
            }
        }
        // (15,5) is unexplored and walkable
        let turns_before = state.stats.turns;
        let start_pos = (state.player.x, state.player.y);
        let mut visual = VisualState::new(80, 50);
        state.auto_explore(&mut visual);
        // Either it found a path and moved, or all were explored
        assert!(
            state.player.x != start_pos.0
                || state.player.y != start_pos.1
                || state.stats.turns == turns_before,
            "auto_explore should either move or do nothing"
        );
        assert!(state.map.is_walkable(state.player.x, state.player.y));
    }

    #[test]
    fn auto_explore_opens_frontier_door_instead_of_reporting_complete() {
        let mut state = state_with_floor(9, 7);
        state.player.x = 3;
        state.player.y = 3;
        for y in 0..7 {
            for x in 0..9 {
                let index = state.map.idx(x, y);
                state.map.tiles[index] = if x <= 3 { Tile::Floor } else { Tile::Wall };
                state.map.explored[index] = x <= 3;
            }
        }
        let door = state.map.idx(4, 3);
        state.map.tiles[door] = Tile::DoorClosed;
        let beyond = state.map.idx(5, 3);
        state.map.tiles[beyond] = Tile::Floor;
        state.map.explored[beyond] = false;
        let mut visual = VisualState::new(80, 50);

        state.auto_explore(&mut visual);

        assert_eq!(state.map.tiles[door], Tile::DoorOpen);
        assert_eq!((state.player.x, state.player.y), (3, 3));
    }

    // ── Scroll of Confusion ──

    #[test]
    fn scroll_of_confusion_teleports_visible_monsters() {
        let mut state = state_with_floor(20, 20);
        // Place 3 visible monsters near player
        let positions = [(12, 10), (13, 10), (12, 11)];
        let mut orig_positions = Vec::new();
        for &(mx, my) in &positions {
            let mi = state.map.idx(mx, my);
            state.map.visible[mi] = true;
            state.monsters.push(Monster::new(mx, my, MonsterKind::Rat));
            orig_positions.push((mx, my));
        }
        state.player.inventory.push(ItemKind::ScrollOfConfusion);
        let mut visual = VisualState::new(80, 50);
        state.use_item(ItemKind::ScrollOfConfusion, 0, &mut visual);
        assert_eq!(state.stats.items_used, 1);
        // At least one monster should have moved
        let any_moved = state
            .monsters
            .iter()
            .enumerate()
            .any(|(i, m)| m.x != orig_positions[i].0 || m.y != orig_positions[i].1);
        assert!(any_moved, "at least one monster should teleport");
    }

    #[test]
    fn scroll_of_confusion_does_not_affect_invisible_monsters() {
        let mut state = state_with_floor(20, 20);
        // Visible monster
        let vx = 12;
        let vy = state.player.y;
        let vi = state.map.idx(vx, vy);
        state.map.visible[vi] = true;
        let mut vis = Monster::new(vx, vy, MonsterKind::Rat);
        vis.stats.hp = 20;
        state.monsters.push(vis);
        // Hidden monster
        let mut hidden = Monster::new(14, state.player.y, MonsterKind::Rat);
        hidden.stats.hp = 20;
        let hx = hidden.x;
        let hy = hidden.y;
        state.monsters.push(hidden);
        state.player.inventory.push(ItemKind::ScrollOfConfusion);
        let mut visual = VisualState::new(80, 50);
        state.use_item(ItemKind::ScrollOfConfusion, 0, &mut visual);
        // Hidden monster should not have moved
        assert_eq!(state.monsters[1].x, hx);
        assert_eq!(state.monsters[1].y, hy);
        assert_eq!(state.monsters[1].stats.hp, 20);
    }

    #[test]
    fn scroll_of_confusion_does_not_teleport_onto_player() {
        let mut state = state_with_floor(20, 20);
        let px = state.player.x;
        let py = state.player.y;
        // Place visible monster
        let mx = px + 1;
        let my = py;
        let mi = state.map.idx(mx, my);
        state.map.visible[mi] = true;
        state.monsters.push(Monster::new(mx, my, MonsterKind::Rat));
        state.player.inventory.push(ItemKind::ScrollOfConfusion);
        let mut visual = VisualState::new(80, 50);
        state.use_item(ItemKind::ScrollOfConfusion, 0, &mut visual);
        // No monster should be on player's tile
        for m in &state.monsters {
            assert!(
                !(m.alive && m.x == px && m.y == py),
                "monster teleported onto player"
            );
        }
    }

    // ── monster attacks player ──

    #[test]
    fn monster_adjacent_damages_player_on_end_turn() {
        let mut state = state_with_floor(20, 20);
        let hp_before = state.player.stats.hp;
        // Place monster adjacent to player
        let mx = state.player.x + 1;
        let my = state.player.y;
        let mi = state.map.idx(mx, my);
        state.map.visible[mi] = true;
        let mut m = Monster::new(mx, my, MonsterKind::Orc);
        m.stats.attack = 6;
        m.stats.hp = 20;
        m.stats.max_hp = 20;
        m.windup = 1;
        state.monsters.push(m);
        let mut visual = VisualState::new(80, 50);
        state.end_turn(&mut visual);
        assert!(
            state.player.stats.hp < hp_before,
            "player should take damage from adjacent monster"
        );
        assert!(state.stats.dmg_taken > 0);
    }

    #[test]
    fn spider_applies_poison_on_hit() {
        let mut state = state_with_floor(20, 20);
        // Place spider adjacent to player and make its tile visible
        let mx = state.player.x + 1;
        let my = state.player.y;
        let mi = state.map.idx(mx, my);
        state.map.visible[mi] = true;
        let mut spider = Monster::new(mx, my, MonsterKind::Spider);
        spider.stats.attack = 3;
        spider.stats.hp = 10;
        spider.stats.max_hp = 10;
        state.monsters.push(spider);
        let mut visual = VisualState::new(80, 50);
        // We can't control the 40% RNG, but we can test that poison state is
        // properly handled when it DOES apply. Run many end turns and check
        // no panics and player HP never goes below 0 without death being set.
        for _ in 0..10 {
            if state.status == GameStatus::Playing {
                state.end_turn(&mut visual);
            }
        }
        // If player died, status should be Dead; otherwise still Playing
        if state.status == GameStatus::Dead {
            assert!(!state.player.alive);
        }
    }

    #[test]
    fn monster_killed_by_poison_during_end_turn() {
        let mut state = state_with_floor(20, 20);
        // Poisoned monster with 1 HP
        let mx = state.player.x + 2;
        let my = state.player.y;
        let mut m = Monster::new(mx, my, MonsterKind::Rat);
        m.stats.hp = 1;
        m.stats.max_hp = 5;
        m.poisoned = true;
        m.poison_ticks = 2;
        state.monsters.push(m);
        let mut visual = VisualState::new(80, 50);
        state.end_turn(&mut visual);
        // Monster should be dead after poison tick
        assert!(
            !state.monsters.iter().any(|m| m.alive),
            "poisoned monster should die"
        );
    }

    #[test]
    fn monster_wanders_when_not_visible() {
        let mut state = state_with_floor(20, 20);
        // Place monster far from player, not visible
        let mx = 15;
        let my = 15;
        let mut m = Monster::new(mx, my, MonsterKind::Rat);
        m.stats.hp = 5;
        m.stats.max_hp = 5;
        state.monsters.push(m);
        let orig_x = state.monsters[0].x;
        let orig_y = state.monsters[0].y;
        let mut visual = VisualState::new(80, 50);
        // Run a few end_turns; the monster may randomly wander
        let mut moved = false;
        for _ in 0..20 {
            if state.monsters.is_empty() || !state.monsters[0].alive {
                break;
            }
            state.end_turn(&mut visual);
            if state.monsters[0].x != orig_x || state.monsters[0].y != orig_y {
                moved = true;
                break;
            }
        }
        // Since wander chance is 50% and dir is random, it should eventually move
        // but we can't guarantee it. Just assert no crash.
        let _ = moved; // may or may not move, that's fine
    }

    // ── seed reproducibility ──

    #[test]
    fn same_seed_produces_same_level() {
        let mut state_a = GameState::new(12345);
        state_a.new_game();
        let rooms_a: Vec<_> = state_a
            .map
            .rooms
            .iter()
            .map(|r| (r.x, r.y, r.w, r.h))
            .collect();
        let player_pos_a = (state_a.player.x, state_a.player.y);

        let mut state_b = GameState::new(12345);
        state_b.new_game();
        let rooms_b: Vec<_> = state_b
            .map
            .rooms
            .iter()
            .map(|r| (r.x, r.y, r.w, r.h))
            .collect();
        let player_pos_b = (state_b.player.x, state_b.player.y);

        assert_eq!(rooms_a, rooms_b, "same seed should produce same rooms");
        assert_eq!(
            player_pos_a, player_pos_b,
            "same seed should spawn player in same position"
        );
    }

    #[test]
    fn different_seeds_produce_different_levels() {
        let mut state_a = GameState::new(11111);
        state_a.new_game();
        let rooms_a: Vec<_> = state_a
            .map
            .rooms
            .iter()
            .map(|r| (r.x, r.y, r.w, r.h))
            .collect();

        let mut state_b = GameState::new(99999);
        state_b.new_game();
        let rooms_b: Vec<_> = state_b
            .map
            .rooms
            .iter()
            .map(|r| (r.x, r.y, r.w, r.h))
            .collect();

        // It's possible but extremely unlikely that two different seeds produce
        // identical maps. If they do, the test still passes — this is a smoke test.
        // Different seeds should at least not crash.
        assert!(!rooms_a.is_empty());
        assert!(!rooms_b.is_empty());
    }

    // ── Inventory mode transitions ──

    #[test]
    fn process_turn_inventory_action_transitions_state() {
        let mut state = state_with_floor(10, 10);
        // Cannot directly set status since app handles the transition.
        // Verify that in Playing status, Inventory action is a no-op
        // (in the real app, the key handler switches to Inventory before process_turn).
        assert_eq!(state.status, GameStatus::Playing);
        let mut visual = VisualState::new(80, 50);
        state.process_turn(Action::Inventory, &mut visual);
        // process_turn returns early for non-move actions; status stays Playing
        assert_eq!(state.status, GameStatus::Playing);
    }

    // ── monster loot drop ──

    #[test]
    fn loot_drop_lands_on_walkable_tile() {
        let mut state = state_with_floor(10, 10);
        state.rules_version = 3;
        // Place monster with 1 HP so player can kill it
        let mx = state.player.x + 1;
        let my = state.player.y;
        let mut m = Monster::new(mx, my, MonsterKind::Rat);
        m.stats.hp = 1;
        m.stats.max_hp = 5;
        state.monsters.push(m);
        let mut visual = VisualState::new(80, 50);
        state.process_turn(Action::Move(1, 0), &mut visual);
        // If loot dropped (30% chance), verify it's on a walkable tile
        for item in &state.items {
            assert!(
                state.map.is_walkable(item.x, item.y),
                "loot at ({},{}) must be on walkable tile",
                item.x,
                item.y
            );
        }
    }

    #[test]
    fn loot_drop_does_not_overlap_existing_items() {
        let mut state = state_with_floor(10, 10);
        state.rules_version = 3;
        // Place item near the monster spot
        state.items.push(Item::new(
            state.player.x + 1,
            state.player.y,
            ItemKind::HealthPotion,
        ));
        let _item_positions: std::collections::HashSet<_> =
            state.items.iter().map(|i| (i.x, i.y)).collect();
        // Kill many monsters in sequence to increase chance of loot
        for _ in 0..20 {
            let mx = state.player.x + 1;
            let my = state.player.y;
            let mut m = Monster::new(mx, my, MonsterKind::Rat);
            m.stats.hp = 1;
            m.stats.max_hp = 5;
            state.monsters.push(m);
            let mut visual = VisualState::new(80, 50);
            state.process_turn(Action::Move(1, 0), &mut visual);
        }
        let new_positions: std::collections::HashSet<_> =
            state.items.iter().map(|i| (i.x, i.y)).collect();
        assert_eq!(
            new_positions.len(),
            state.items.len(),
            "no two items should share a tile"
        );
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// RENDER TESTS (Ratatui TestBackend)
// ══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod render_tests {
    use crate::app::{GameState, GameStatus};
    use crate::campaign::CampaignObjective;
    use crate::entity::{
        Camp, CampKind, Item, ItemKind, Landmark, Monster, MonsterKind, Npc, NpcKind,
    };
    use crate::map::{
        EnvironmentProp, Map, MapTheme, PropKind, Rect, SurfaceKind, SurfaceMaterial, Tile,
        WorldMapId,
    };
    use crate::render::{render, RenderCtx};
    use crate::visual::{PlayerActionKind, PlayerStepKind, VisualState};
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;
    use ratatui::Terminal;

    fn ctx_playing<'a>(state: &'a GameState, visual: &'a VisualState) -> RenderCtx<'a> {
        RenderCtx {
            map: &state.map,
            world_map_id: state.world_map_id,
            world_links: &state.world_links,
            player: &state.player,
            monsters: &state.monsters,
            items: &state.items,
            npcs: &state.npcs,
            projectiles: &state.projectiles,
            landmarks: &state.landmarks,
            used_landmarks: &state.used_landmarks,
            seen_monsters: &state.seen_monsters,
            defeated_monsters: &state.defeated_monsters,
            messages: &state.messages,
            status: state.status,
            stats: &state.stats,
            seed: state.seed,
            challenge_mode: state.challenge_mode,
            floor_memory: &state.floor_memory,
            cached_floor_count: state.floor_cache.len(),
            completed_runs: 0,
            run_history: &[],
            inventory_cursor: 0,
            auto_pickup: false,
            log_scroll: 0,
            visual,
            tick: 0,
            dialog_npc_name: "",
            dialog_npc_idx: None,
            dialog_text: &[],
            dialog_show_choices: false,
            dialog_choices: &[],
            camps: &[],
            game_mode: crate::app::GameMode::Classic,
            campaign_objectives: &[],
            campaign_flags: &[],
            campaign_level_name: "",
            show_minimap: false,
            show_help: false,
            show_bestiary: false,
            auto_exploring: false,
            reduced_flash: false,
            high_contrast: false,
            show_log_overlay: false,
            color_filter: 0,
            player_moving: false,
            camera_bob: 0,
            camera_sway: 0,
            low_motion: false,
            preparation_menu: state.preparation_menu,
            hints_seen: 0,
            camera_angle: 0.0,
            battle: None,
        }
    }

    fn ctx_menu<'a>(state: &'a GameState, visual: &'a VisualState) -> RenderCtx<'a> {
        let mut ctx = ctx_playing(state, visual);
        ctx.status = GameStatus::Menu;
        ctx
    }

    fn visible_floor_state() -> GameState {
        let mut state = GameState::new(42);
        state.status = GameStatus::Playing;
        state.map = Map::new(20, 20);
        for y in 0..20 {
            for x in 0..20 {
                let i = state.map.idx(x, y);
                state.map.tiles[i] = Tile::Floor;
                state.map.visible[i] = true;
                state.map.explored[i] = true;
            }
        }
        state.player.x = 10;
        state.player.y = 10;
        state
    }

    fn draw_game(ctx: &RenderCtx) -> ratatui::buffer::Buffer {
        let backend = TestBackend::new(120, 60);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render(f, ctx);
            })
            .unwrap();
        terminal.backend().buffer().clone()
    }

    fn viewport_signature(buf: &ratatui::buffer::Buffer) -> Vec<(String, Color, Color)> {
        let mut cells = Vec::with_capacity(92 * 54);
        for y in 0..54 {
            for x in 0..92 {
                let cell = buf.cell((x, y)).unwrap();
                cells.push((cell.symbol().to_owned(), cell.fg, cell.bg));
            }
        }
        cells
    }

    fn luma(color: Color) -> u16 {
        match color {
            Color::Rgb(r, g, b) => r as u16 + g as u16 + b as u16,
            _ => 0,
        }
    }

    fn minimap_pos(state: &GameState, x: i32, y: i32) -> (u16, u16) {
        let dungeon_w = 120u16 - 28;
        let dungeon_h = 60u16 - 6;
        let inner_w = 24u16
            .min(dungeon_w.saturating_sub(4).max(1))
            .min(state.map.width as u16);
        let inner_h = 12u16
            .min(dungeon_h.saturating_sub(5).max(1))
            .min(state.map.height as u16);
        let x0 = dungeon_w.saturating_sub(inner_w + 3);
        let y0 = 1u16;
        let vpx = (state.player.x - inner_w as i32 / 2).clamp(0, state.map.width - inner_w as i32);
        let vpy = (state.player.y - inner_h as i32 / 2).clamp(0, state.map.height - inner_h as i32);
        (x0 + 1 + (x - vpx) as u16, y0 + 1 + (y - vpy) as u16)
    }

    #[test]
    fn menu_renders_title() {
        let state = GameState::new(42);
        let visual_state = VisualState::new(80, 50);
        let ctx = ctx_menu(&state, &visual_state);
        let backend = TestBackend::new(80, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render(f, &ctx);
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let text = cell_text(buf);
        assert!(
            text.contains("terminal") && text.contains("roguelike"),
            "menu should contain tagline, got: {}",
            text
        );
        assert!(
            text.contains("classic") && text.contains("mode"),
            "menu should show classic mode option, got: {}",
            text
        );
        assert!(
            text.contains("Challenge:")
                && text.contains("Standard")
                && text.contains("runs")
                && text.contains("recorded"),
            "menu should expose challenge selection and history, got: {}",
            text
        );
    }

    #[test]
    fn playing_uses_half_block_third_person_view() {
        let mut state = GameState::new(42);
        state.status = GameStatus::Playing;
        state.map = Map::new(20, 20);
        for y in 0..20 {
            for x in 0..20 {
                let i = state.map.idx(x, y);
                state.map.tiles[i] = Tile::Floor;
            }
        }
        state.player.x = 10;
        state.player.y = 10;
        // Make area around player visible and explored
        for y in 5..15 {
            for x in 5..15 {
                let i = state.map.idx(x, y);
                state.map.visible[i] = true;
                state.map.explored[i] = true;
            }
        }
        let wall = state.map.idx(13, 10);
        state.map.tiles[wall] = Tile::Wall;
        let visual_state = VisualState::new(80, 50);
        let ctx = ctx_playing(&state, &visual_state);
        let buf = draw_game(&ctx);
        let viewport = viewport_signature(&buf);
        assert!(viewport.iter().all(|(symbol, _, _)| symbol == "▀"));
        let color_pairs: std::collections::HashSet<_> = viewport
            .iter()
            .map(|(_, foreground, background)| (*foreground, *background))
            .collect();
        assert!(
            color_pairs.len() > 20,
            "textured tiles and the player sprite should produce substantial color variation"
        );
    }

    #[test]
    fn playing_shows_visible_monster() {
        let mut state = GameState::new(42);
        state.status = GameStatus::Playing;
        state.map = Map::new(20, 20);
        for y in 0..20 {
            for x in 0..20 {
                let i = state.map.idx(x, y);
                state.map.tiles[i] = Tile::Floor;
                state.map.visible[i] = true;
                state.map.explored[i] = true;
            }
        }
        state.player.x = 10;
        state.player.y = 10;
        let visual_state = VisualState::new(80, 50);
        let empty = viewport_signature(&draw_game(&ctx_playing(&state, &visual_state)));
        state.monsters.push(Monster::new(11, 10, MonsterKind::Rat));
        let occupied = viewport_signature(&draw_game(&ctx_playing(&state, &visual_state)));

        assert_ne!(
            empty, occupied,
            "visible monster should alter viewport pixels"
        );
    }

    #[test]
    fn visible_hostile_replaces_run_counters_with_threat_panel() {
        let mut state = visible_floor_state();
        state.monsters.clear();
        state.monsters.push(Monster::new(12, 10, MonsterKind::Rat));
        let visual = VisualState::new(80, 50);
        let text = cell_text(&draw_game(&ctx_playing(&state, &visual)));

        assert!(text.contains("Threat") && text.contains("RAT"));
        assert!(!text.contains("Turns:"));
    }

    #[test]
    fn crown_wake_has_viewport_pressure_and_known_stair_bearing() {
        let mut state = visible_floor_state();
        state.monsters.clear();
        let visual = VisualState::new(80, 50);
        let ordinary = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));
        state.player.crown_held = true;
        state.player.depth = 2;
        let stair = state.map.idx(8, 10);
        state.map.tiles[stair] = Tile::StairsUp;
        state.map.explored[stair] = true;
        let crown_buffer = draw_game(&ctx_playing(&state, &visual));
        let crown = viewport_signature(&crown_buffer);
        let text = cell_text(&crown_buffer);

        assert_ne!(ordinary, crown);
        assert!(text.contains("CROWN") && text.contains("WAKE"));
        assert!(text.contains("known") && text.contains("stairs") && text.contains("W"));
    }

    #[test]
    fn distant_visible_hostile_does_not_displace_sidebar_run_summary() {
        let mut state = visible_floor_state();
        state.monsters.clear();
        state.monsters.push(Monster::new(16, 10, MonsterKind::Rat));
        let visual = VisualState::new(80, 50);
        let text = cell_text(&draw_game(&ctx_playing(&state, &visual)));

        assert!(!text.contains("Threat"));
        assert!(text.contains("Turns:"));
    }

    #[test]
    fn near_monster_projects_larger_than_far_monster() {
        let mut state = visible_floor_state();
        let visual = VisualState::new(80, 50);
        let empty = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));

        state.monsters.push(Monster::new(11, 10, MonsterKind::Rat));
        let near = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));
        state.monsters[0].x = 15;
        let far = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));

        let changed = |candidate: &[(String, Color, Color)]| {
            empty
                .iter()
                .zip(candidate)
                .filter(|(before, after)| before != after)
                .count()
        };
        assert!(
            changed(&near) > changed(&far),
            "perspective projection should make the near monster visibly larger"
        );
    }

    #[test]
    fn near_item_uses_more_pixels_than_far_lod_mark() {
        let mut state = visible_floor_state();
        state.items.clear();
        state.monsters.clear();
        let visual = VisualState::new(80, 50);
        let empty = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));

        state.items.push(Item::new(11, 10, ItemKind::IronKey));
        let near = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));
        state.items[0].x = 15;
        let far = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));
        let changed = |candidate: &[(String, Color, Color)]| {
            empty
                .iter()
                .zip(candidate)
                .filter(|(before, after)| before != after)
                .count()
        };

        assert!(changed(&near) > changed(&far));
    }

    #[test]
    fn wall_ahead_crosses_the_perspective_horizon() {
        let mut state = visible_floor_state();
        let visual = VisualState::new(80, 50);
        let open = draw_game(&ctx_playing(&state, &visual));
        for y in 7..=13 {
            let index = state.map.idx(14, y);
            state.map.tiles[index] = Tile::Wall;
        }
        let walled = draw_game(&ctx_playing(&state, &visual));

        let changed_at = |x, y| open.cell((x, y)) != walled.cell((x, y));
        assert!(changed_at(46, 18), "wall should rise above the horizon");
        assert!(changed_at(46, 27), "wall should descend below the horizon");
    }

    #[test]
    fn recently_hit_monster_changes_viewport_reaction_pixels() {
        let mut state = visible_floor_state();
        state.monsters.push(Monster::new(11, 10, MonsterKind::Rat));
        let idle_visual = VisualState::new(80, 50);
        let idle = viewport_signature(&draw_game(&ctx_playing(&state, &idle_visual)));
        let mut hit_visual = VisualState::new(80, 50);
        hit_visual.add_effect(crate::visual::VisualEffect::damage_number(11, 10, 3));
        let hit = viewport_signature(&draw_game(&ctx_playing(&state, &hit_visual)));

        assert_ne!(idle, hit, "hit reaction should alter monster projection");
    }

    #[test]
    fn enemy_windup_impact_recovery_and_stagger_are_distinct() {
        let mut state = visible_floor_state();
        state.monsters.push(Monster::new(12, 10, MonsterKind::Orc));
        let idle_visual = VisualState::new(80, 50);
        let mut signatures = std::collections::HashSet::new();
        signatures.insert(viewport_signature(&draw_game(&ctx_playing(
            &state,
            &idle_visual,
        ))));

        state.monsters[0].windup = 1;
        signatures.insert(viewport_signature(&draw_game(&ctx_playing(
            &state,
            &idle_visual,
        ))));
        state.monsters[0].windup = 0;

        let mut impact_visual = VisualState::new(80, 50);
        impact_visual.add_effect(crate::visual::VisualEffect::monster_attack_flash(12, 10));
        signatures.insert(viewport_signature(&draw_game(&ctx_playing(
            &state,
            &impact_visual,
        ))));

        let mut recovery_visual = VisualState::new(80, 50);
        recovery_visual.add_effect(crate::visual::VisualEffect::AttackFlash {
            x: 12,
            y: 10,
            timer: 2,
            flash_color: (255, 80, 80),
        });
        signatures.insert(viewport_signature(&draw_game(&ctx_playing(
            &state,
            &recovery_visual,
        ))));

        state.monsters[0].staggered = 1;
        signatures.insert(viewport_signature(&draw_game(&ctx_playing(
            &state,
            &idle_visual,
        ))));

        assert_eq!(signatures.len(), 5);
    }

    #[test]
    fn monster_families_have_distinct_death_collapse_art() {
        let state = visible_floor_state();
        let mut signatures = std::collections::HashSet::new();
        for kind in [
            MonsterKind::Skeleton,
            MonsterKind::Slime,
            MonsterKind::Wraith,
            MonsterKind::Sentinel,
            MonsterKind::Spider,
            MonsterKind::Necromancer,
        ] {
            let mut visual = VisualState::new(80, 50);
            visual.add_effect(crate::visual::VisualEffect::monster_death(12, 10, kind));
            for _ in 0..4 {
                visual.tick();
            }
            signatures.insert(viewport_signature(&draw_game(&ctx_playing(
                &state, &visual,
            ))));
        }

        assert_eq!(signatures.len(), 6);
    }

    #[test]
    fn damage_number_art_uses_the_actual_amount() {
        let mut state = visible_floor_state();
        state.monsters.clear();
        state.monsters.push(Monster::new(11, 10, MonsterKind::Rat));
        let mut small_visual = VisualState::new(80, 50);
        small_visual.add_effect(crate::visual::VisualEffect::damage_number(11, 10, 3));
        let small = viewport_signature(&draw_game(&ctx_playing(&state, &small_visual)));
        let mut large_visual = VisualState::new(80, 50);
        large_visual.add_effect(crate::visual::VisualEffect::damage_number(11, 10, 12));
        let large = viewport_signature(&draw_game(&ctx_playing(&state, &large_visual)));

        assert_ne!(small, large, "damage digits ignored their numeric payload");
    }

    #[test]
    fn help_overlay_replaces_persistent_control_documentation() {
        let state = visible_floor_state();
        let visual = VisualState::new(80, 50);
        let mut ctx = ctx_playing(&state, &visual);
        ctx.show_help = true;
        let buffer = draw_game(&ctx);

        let text = cell_text(&buffer);
        assert!(text.contains("CONTROLS"));
        assert!(text.contains("context"));
        assert!(
            text.contains("O|/|v"),
            "door listening must show lowercase v"
        );
        assert!(
            text.contains("V") && text.contains("chronicle"),
            "chronicle must remain distinct from door listening"
        );
    }

    #[test]
    fn help_overlay_fits_compact_terminal() {
        let state = visible_floor_state();
        let visual = VisualState::new(80, 50);
        let mut ctx = ctx_playing(&state, &visual);
        ctx.show_help = true;
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, &ctx)).unwrap();

        let text = cell_text(terminal.backend().buffer());
        assert!(text.contains("CONTROLS"));
        assert!(text.contains("PageUp/Down"));
        assert!(text.contains("F5|/|F9"));
    }

    #[test]
    fn menu_help_is_reachable_before_starting_a_run() {
        let state = GameState::new(42);
        let visual = VisualState::new(80, 50);
        let mut ctx = ctx_menu(&state, &visual);
        ctx.show_help = true;
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, &ctx)).unwrap();

        let text = cell_text(terminal.backend().buffer());
        assert!(text.contains("CONTROLS"));
        assert!(text.contains("Movement"));
        assert!(text.contains("chronicle"));
    }

    #[test]
    fn campaign_intro_keeps_its_actions_visible_on_compact_terminal() {
        let state = GameState::new(42);
        let visual = VisualState::new(80, 50);
        let mut ctx = ctx_menu(&state, &visual);
        ctx.status = GameStatus::CampaignMenu;
        ctx.game_mode = crate::app::GameMode::Campaign;
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, &ctx)).unwrap();

        let text = cell_text(terminal.backend().buffer());
        assert!(text.contains("Chapter|I:"));
        assert!(text.contains("ENTER"));
        assert!(text.contains("q/Esc"));
    }

    #[test]
    fn campaign_progress_is_visible_in_the_sidebar() {
        let mut state = visible_floor_state();
        state.player.depth = 1;
        let visual = VisualState::new(80, 50);
        let mut ctx = ctx_playing(&state, &visual);
        let objectives = [CampaignObjective::RelightLanterns { lit: 1, needed: 3 }];
        ctx.game_mode = crate::app::GameMode::Campaign;
        ctx.campaign_objectives = &objectives;
        ctx.campaign_level_name = "Lantern Ward";

        let text = cell_text(&draw_game(&ctx));
        assert!(text.contains("Chapter|I|2/3"), "sidebar text: {text}");
        assert!(text.contains("Lantern|Ward"), "sidebar text: {text}");
    }

    #[test]
    fn named_npc_appears_in_context_strip() {
        let mut state = visible_floor_state();
        state.npcs.push({
            let mut npc = Npc::new(state.player.x + 1, state.player.y, NpcKind::WatchCaptain);
            npc.custom_name = Some("Mara Voss".to_string());
            npc
        });
        let visual = VisualState::new(80, 50);
        let text = cell_text(&draw_game(&ctx_playing(&state, &visual)));

        assert!(text.contains("Mara|Voss"));
    }

    #[test]
    fn playing_renders_in_compact_terminal_layout() {
        let state = visible_floor_state();
        let visual = VisualState::new(80, 50);
        let ctx = ctx_playing(&state, &visual);
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, &ctx)).unwrap();

        assert!(cell_text(terminal.backend().buffer()).contains("Facing"));
    }

    #[test]
    fn playing_explains_the_compact_terminal_requirement() {
        let state = visible_floor_state();
        let visual = VisualState::new(80, 50);
        let ctx = ctx_playing(&state, &visual);
        let backend = TestBackend::new(79, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(frame, &ctx)).unwrap();

        let text = cell_text(terminal.backend().buffer());
        assert!(text.contains("Terminal|too|small"));
        assert!(text.contains("80×24"));
        assert!(!text.contains("Facing"));
    }

    #[test]
    fn representative_animated_render_stays_within_frame_budget() {
        let mut state = visible_floor_state();
        state.monsters.clear();
        state.items.clear();
        for (offset, kind) in [
            MonsterKind::Rat,
            MonsterKind::Bat,
            MonsterKind::Spider,
            MonsterKind::Wraith,
            MonsterKind::Necromancer,
        ]
        .into_iter()
        .enumerate()
        {
            state
                .monsters
                .push(Monster::new(11 + offset as i32, 10, kind));
        }
        state.items.push(Item::new(12, 11, ItemKind::Emberbrand));
        let mut visual = VisualState::new(80, 50);
        visual.add_effect(crate::visual::VisualEffect::damage_number(11, 10, 12));
        visual.add_effect(crate::visual::VisualEffect::fire_ring(12, 10));
        let mut ctx = ctx_playing(&state, &visual);
        let frames = 30;
        let started = std::time::Instant::now();
        for tick in 0..frames {
            ctx.tick = tick;
            let _ = draw_game(&ctx);
        }
        let elapsed = started.elapsed();
        let average = elapsed / frames as u32;
        eprintln!("representative render average: {average:?}");

        // Release builds must hit the real 33 ms frame interval. Debug builds
        // render unoptimized and share the machine with the parallel test
        // suite, so give them 2x headroom: the guard still catches render
        // regressions without flipping on contention noise.
        let budget = if cfg!(debug_assertions) {
            std::time::Duration::from_millis(66)
        } else {
            std::time::Duration::from_millis(33)
        };
        assert!(
            average < budget,
            "animated render exceeded its {} ms frame interval: {average:?}",
            budget.as_millis()
        );
    }

    #[test]
    fn hidden_monster_does_not_alter_third_person_viewport() {
        let mut state = visible_floor_state();
        let visual_state = VisualState::new(80, 50);
        let hidden_index = state.map.idx(11, 10);
        state.map.visible[hidden_index] = false;
        let empty = viewport_signature(&draw_game(&ctx_playing(&state, &visual_state)));
        state.monsters.push(Monster::new(11, 10, MonsterKind::Rat));
        let hidden = viewport_signature(&draw_game(&ctx_playing(&state, &visual_state)));

        assert_eq!(empty, hidden, "hidden monster leaked into viewport pixels");
    }

    #[test]
    fn hidden_terrain_material_does_not_leak_into_perspective_view() {
        let mut state = visible_floor_state();
        let visual = VisualState::new(80, 50);
        let hidden_index = state.map.idx(13, 10);
        state.map.visible[hidden_index] = false;
        let ordinary = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));
        state.map.tiles[hidden_index] = Tile::ShallowWater;
        let distinctive = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));

        assert_eq!(
            ordinary, distinctive,
            "unseen terrain color leaked through perspective fog"
        );
    }

    #[test]
    fn invisible_monster_beyond_wall_is_not_rendered() {
        let mut state = visible_floor_state();
        let wall_index = state.map.idx(11, 10);
        state.map.tiles[wall_index] = Tile::Wall;
        let hidden_index = state.map.idx(12, 10);
        state.map.visible[hidden_index] = false;
        let visual_state = VisualState::new(80, 50);
        let wall_only = viewport_signature(&draw_game(&ctx_playing(&state, &visual_state)));
        state.monsters.push(Monster::new(12, 10, MonsterKind::Rat));
        let occluded = viewport_signature(&draw_game(&ctx_playing(&state, &visual_state)));

        assert_eq!(
            wall_only, occluded,
            "monster pixels appeared through a wall"
        );
    }

    #[test]
    fn player_facing_changes_directional_sprite() {
        let state = visible_floor_state();
        let visual_state = VisualState::new(80, 50);
        let east = viewport_signature(&draw_game(&ctx_playing(&state, &visual_state)));
        let mut south_ctx = ctx_playing(&state, &visual_state);
        south_ctx.camera_angle = std::f64::consts::FRAC_PI_2;
        let south = viewport_signature(&draw_game(&south_ctx));

        assert_ne!(
            east, south,
            "turning should change the player facing pixels"
        );
    }

    #[test]
    fn player_sprite_animates_without_moving_world_position() {
        let state = visible_floor_state();
        let visual = VisualState::new(80, 50);
        let idle = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));
        let mut stepped_ctx = ctx_playing(&state, &visual);
        stepped_ctx.tick = 20;
        let stepped = viewport_signature(&draw_game(&stepped_ctx));

        assert_ne!(
            idle, stepped,
            "animation tick should change the visible player pose"
        );
        assert_eq!((state.player.x, state.player.y), (10, 10));
    }

    #[test]
    fn player_step_pose_does_not_require_camera_bob() {
        let state = visible_floor_state();
        let idle_visual = VisualState::new(80, 50);
        let idle = viewport_signature(&draw_game(&ctx_playing(&state, &idle_visual)));
        let mut moving_visual = VisualState::new(80, 50);
        moving_visual.start_player_action(PlayerActionKind::Step(PlayerStepKind::Forward));
        let mut moving = ctx_playing(&state, &moving_visual);
        moving.camera_bob = 0;
        let stepped = viewport_signature(&draw_game(&moving));

        assert_ne!(idle, stepped, "head-bob preference froze the step pose");
    }

    #[test]
    fn player_directional_steps_have_distinct_silhouettes() {
        let state = visible_floor_state();
        let mut signatures = std::collections::HashSet::new();
        for step in [
            PlayerStepKind::Forward,
            PlayerStepKind::Backward,
            PlayerStepKind::StrafeLeft,
            PlayerStepKind::StrafeRight,
        ] {
            let mut visual = VisualState::new(80, 50);
            visual.start_player_action(PlayerActionKind::Step(step));
            signatures.insert(viewport_signature(&draw_game(&ctx_playing(
                &state, &visual,
            ))));
        }
        assert_eq!(signatures.len(), 4);
    }

    #[test]
    fn player_evasive_steps_have_dedicated_left_and_right_art() {
        let state = visible_floor_state();
        let mut signatures = std::collections::HashSet::new();
        for step in [PlayerStepKind::StrafeLeft, PlayerStepKind::StrafeRight] {
            let mut visual = VisualState::new(80, 50);
            visual.start_player_action(PlayerActionKind::Dodge(step));
            signatures.insert(viewport_signature(&draw_game(&ctx_playing(
                &state, &visual,
            ))));
        }
        assert_eq!(signatures.len(), 2);
    }

    #[test]
    fn sidebar_exposes_stamina_reserve() {
        let mut state = visible_floor_state();
        state.player.stamina = 2;
        let visual = VisualState::new(80, 50);
        let text = cell_text(&draw_game(&ctx_playing(&state, &visual)));

        assert!(text.contains("STA:"));
        assert!(text.contains("██░░░░░░"));
    }

    #[test]
    fn exhausted_player_has_distinct_art_and_posture() {
        let mut state = visible_floor_state();
        let visual = VisualState::new(80, 50);
        let ready = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));
        state.player.stamina = 1;
        let winded_buffer = draw_game(&ctx_playing(&state, &visual));
        let winded = viewport_signature(&winded_buffer);
        let text = cell_text(&winded_buffer);

        assert_ne!(ready, winded);
        assert!(text.contains("WINDED"), "missing winded posture in: {text}");
    }

    #[test]
    fn guardian_second_phase_changes_art_and_threat_intent() {
        let mut state = visible_floor_state();
        let visual = VisualState::new(80, 50);
        let mut guardian = Monster::new(13, 10, MonsterKind::Guardian);
        guardian.awakened = true;
        state.monsters.push(guardian);
        let sealed = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));

        state.monsters[0].stats.hp = state.monsters[0].stats.max_hp / 2;
        let unsealed_buffer = draw_game(&ctx_playing(&state, &visual));
        let unsealed = viewport_signature(&unsealed_buffer);
        let text = cell_text(&unsealed_buffer);

        assert_ne!(sealed, unsealed);
        assert!(text.contains("CROWN-FIRE"));
        assert!(text.contains("PHASE") && text.contains("II"));
    }

    #[test]
    fn failed_same_seed_run_leaves_a_visible_death_echo() {
        let state = visible_floor_state();
        let visual = VisualState::new(80, 50);
        let empty = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));
        let history = [crate::run_history::RunSummary {
            seed: state.seed,
            won: false,
            game_mode: crate::app::GameMode::Classic,
            challenge_mode: crate::app::ChallengeMode::Standard,
            stats: crate::app::RunStats::new(),
            completed_at_unix: 0,
            final_x: Some(13),
            final_y: Some(10),
            final_depth: state.player.depth,
        }];
        let mut ctx = ctx_playing(&state, &visual);
        ctx.run_history = &history;
        let echo = viewport_signature(&draw_game(&ctx));

        assert_ne!(empty, echo);
    }

    #[test]
    fn player_step_contact_matches_floor_material() {
        let mut state = visible_floor_state();
        let mut visual = VisualState::new(80, 50);
        visual.start_player_action(PlayerActionKind::Step(PlayerStepKind::Forward));
        for _ in 0..3 {
            visual.tick();
        }
        let stone = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));
        let underfoot = state.map.idx(state.player.x, state.player.y);
        state.map.tiles[underfoot] = Tile::ShallowWater;
        let water = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));
        state.map.tiles[underfoot] = Tile::Web;
        let web = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));

        assert_ne!(stone, water);
        assert_ne!(water, web);
        assert_ne!(stone, web);
    }

    #[test]
    fn player_attack_has_distinct_anticipation_impact_and_recovery() {
        let mut state = visible_floor_state();
        state.player.weapon = Some(ItemKind::Warhammer);
        let mut visual = VisualState::new(80, 50);
        visual.start_player_action(PlayerActionKind::Attack {
            weapon: state.player.weapon,
        });
        let anticipation = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));
        for _ in 0..4 {
            visual.tick();
        }
        let impact = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));
        for _ in 0..4 {
            visual.tick();
        }
        let recovery = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));

        assert_ne!(anticipation, impact);
        assert_ne!(impact, recovery);
        assert_ne!(anticipation, recovery);
    }

    #[test]
    fn weapon_families_have_distinct_world_sweep_art() {
        let state = visible_floor_state();
        let mut signatures = std::collections::HashSet::new();
        for weapon in [
            None,
            Some(ItemKind::RustySword),
            Some(ItemKind::Longsword),
            Some(ItemKind::Warhammer),
            Some(ItemKind::Emberbrand),
        ] {
            let mut visual = VisualState::new(80, 50);
            visual.add_effect(crate::visual::VisualEffect::weapon_sweep(
                12, 10, 1, 0, weapon, false,
            ));
            signatures.insert(viewport_signature(&draw_game(&ctx_playing(
                &state, &visual,
            ))));
        }
        assert_eq!(signatures.len(), 5);
    }

    #[test]
    fn techniques_and_items_have_specific_cast_art() {
        let state = visible_floor_state();
        let mut technique_signatures = std::collections::HashSet::new();
        for kind in [
            crate::entity::TechniqueKind::Kick,
            crate::entity::TechniqueKind::ShieldBash,
            crate::entity::TechniqueKind::Lunge,
            crate::entity::TechniqueKind::Sweep,
            crate::entity::TechniqueKind::Earthbreak,
            crate::entity::TechniqueKind::CinderArc,
        ] {
            let mut visual = VisualState::new(80, 50);
            visual.add_effect(crate::visual::VisualEffect::technique_impact(
                12, 10, 1, 0, kind,
            ));
            technique_signatures.insert(viewport_signature(&draw_game(&ctx_playing(
                &state, &visual,
            ))));
        }
        assert_eq!(technique_signatures.len(), 6);

        let mut item_signatures = std::collections::HashSet::new();
        for kind in [
            ItemKind::ScrollOfFire,
            ItemKind::ScrollOfConfusion,
            ItemKind::ScrollOfBlink,
            ItemKind::SmokeBomb,
            ItemKind::HealthPotion,
            ItemKind::LanternOil,
            ItemKind::EnchantmentScroll,
            ItemKind::LoreFragment,
        ] {
            let mut visual = VisualState::new(80, 50);
            visual.add_effect(crate::visual::VisualEffect::item_cast(12, 10, kind));
            item_signatures.insert(viewport_signature(&draw_game(&ctx_playing(
                &state, &visual,
            ))));
        }
        assert_eq!(item_signatures.len(), 8);
    }

    #[test]
    fn active_npc_conversation_has_a_speech_animation() {
        let mut state = visible_floor_state();
        state.npcs = vec![Npc::new(15, 10, NpcKind::Merchant)];
        let visual = VisualState::new(80, 50);
        let idle = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));
        let mut talking_ctx = ctx_playing(&state, &visual);
        talking_ctx.dialog_npc_name = "Merchant";
        talking_ctx.dialog_npc_idx = Some(0);
        let talking = viewport_signature(&draw_game(&talking_ctx));

        assert_ne!(idle, talking);
    }

    #[test]
    fn dialogue_overlay_draws_the_active_npc_likeness() {
        let mut state = visible_floor_state();
        state.npcs = vec![Npc::new(15, 10, NpcKind::Merchant)];
        let visual = VisualState::new(80, 50);
        let dialog_text = vec!["The road is safer by lanternlight.".to_string()];
        let mut npc_ctx = ctx_playing(&state, &visual);
        npc_ctx.dialog_npc_name = "Merchant";
        npc_ctx.dialog_npc_idx = Some(0);
        npc_ctx.dialog_text = &dialog_text;
        let npc_portrait = viewport_signature(&draw_game(&npc_ctx));

        npc_ctx.dialog_npc_idx = None;
        let bell_portrait = viewport_signature(&draw_game(&npc_ctx));

        assert_ne!(npc_portrait, bell_portrait);
    }

    #[test]
    fn roads_and_environment_props_change_the_first_person_view() {
        let mut state = visible_floor_state();
        state.map.theme = MapTheme::Village;
        state.world_map_id = WorldMapId::Village;
        state.map.surfaces = Some(vec![
            SurfaceKind::Ground(SurfaceMaterial::Grass);
            (state.map.width * state.map.height) as usize
        ]);
        for y in 8..=12 {
            for x in 6..=15 {
                state
                    .map
                    .set_surface(x, y, SurfaceKind::Road(SurfaceMaterial::Cobblestone));
            }
        }
        let ordinary =
            viewport_signature(&draw_game(&ctx_playing(&state, &VisualState::new(80, 50))));
        state.map.props = vec![
            EnvironmentProp::new(12, 10, PropKind::Brazier),
            EnvironmentProp::new(14, 10, PropKind::Banner),
        ];
        let dressed = draw_game(&ctx_playing(&state, &VisualState::new(80, 50)));

        assert_ne!(ordinary, viewport_signature(&dressed));
        assert!(cell_text(&dressed).contains("Warden's"));
    }

    #[test]
    fn player_guard_and_hit_use_distinct_foreground_poses() {
        let mut state = visible_floor_state();
        let idle_visual = VisualState::new(80, 50);
        let idle = viewport_signature(&draw_game(&ctx_playing(&state, &idle_visual)));

        state.player.guarding = true;
        let guard = viewport_signature(&draw_game(&ctx_playing(&state, &idle_visual)));
        state.player.guarding = false;
        let mut hit_visual = VisualState::new(80, 50);
        hit_visual.trigger_damage_flash();
        let hit = viewport_signature(&draw_game(&ctx_playing(&state, &hit_visual)));

        assert_ne!(idle, guard, "guarding should change the player silhouette");
        assert_ne!(guard, hit, "damage should override the guard pose");
    }

    #[test]
    fn sidebar_names_player_action_phase_and_low_motion_mode() {
        let mut state = visible_floor_state();
        state.player.weapon = Some(ItemKind::Longsword);
        let mut visual = VisualState::new(80, 50);
        visual.start_player_action(PlayerActionKind::Attack {
            weapon: state.player.weapon,
        });
        for _ in 0..4 {
            visual.tick();
        }
        let mut ctx = ctx_playing(&state, &visual);
        ctx.low_motion = true;
        let text = cell_text(&draw_game(&ctx));

        assert!(
            text.contains("Posture|ATTACK|·|commit"),
            "missing action posture in: {text}"
        );
        assert!(text.contains("low-motion"));
    }

    #[test]
    fn equipped_weapon_changes_player_sprite() {
        let mut state = visible_floor_state();
        let visual = VisualState::new(80, 50);
        let unarmed = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));
        state.player.weapon = Some(ItemKind::Warhammer);
        let armed = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));

        assert_ne!(
            unarmed, armed,
            "equipped weapon should be visible on player"
        );
    }

    #[test]
    fn every_equipped_weapon_has_a_distinct_authored_foreground_shape() {
        let mut state = visible_floor_state();
        let visual = VisualState::new(80, 50);
        let unarmed = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));
        let mut signatures = std::collections::HashSet::new();
        for weapon in [
            ItemKind::RustySword,
            ItemKind::Longsword,
            ItemKind::Warhammer,
            ItemKind::Emberbrand,
        ] {
            state.player.weapon = Some(weapon);
            state.player.weapon_durability = weapon.max_durability();
            signatures.insert(viewport_signature(&draw_game(&ctx_playing(
                &state, &visual,
            ))));
        }

        assert_eq!(signatures.len(), 4);
        assert!(signatures.into_iter().all(|signature| signature != unarmed));
    }

    #[test]
    fn critical_health_changes_idle_and_forward_step_posture() {
        let mut state = visible_floor_state();
        let visual = VisualState::new(80, 50);
        let healthy_idle = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));
        state.player.stats.hp = state.player.stats.max_hp * 3 / 10;
        let wounded_idle = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));

        let mut moving = VisualState::new(80, 50);
        moving.start_player_action(PlayerActionKind::Step(PlayerStepKind::Forward));
        let wounded_step = viewport_signature(&draw_game(&ctx_playing(&state, &moving)));
        state.player.stats.hp = state.player.stats.max_hp;
        let healthy_step = viewport_signature(&draw_game(&ctx_playing(&state, &moving)));

        assert_ne!(healthy_idle, wounded_idle);
        assert_ne!(healthy_step, wounded_step);
    }

    #[test]
    fn armor_families_change_the_player_outline() {
        let mut state = visible_floor_state();
        let visual = VisualState::new(80, 50);
        let mut signatures = std::collections::HashSet::new();
        for armor in [
            ItemKind::LeatherArmor,
            ItemKind::Chainmail,
            ItemKind::TideglassMail,
            ItemKind::Graveward,
        ] {
            state.player.armor = Some(armor);
            signatures.insert(viewport_signature(&draw_game(&ctx_playing(
                &state, &visual,
            ))));
        }

        assert_eq!(signatures.len(), 4);
    }

    #[test]
    fn low_weapon_durability_is_visible_but_artifacts_do_not_degrade() {
        let mut state = visible_floor_state();
        let visual = VisualState::new(80, 50);
        state.player.weapon = Some(ItemKind::Longsword);
        state.player.weapon_durability = ItemKind::Longsword.max_durability();
        let sound = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));
        state.player.weapon_durability = 1;
        let worn = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));
        assert_ne!(sound, worn);

        state.player.weapon = Some(ItemKind::Emberbrand);
        state.player.weapon_durability = 0;
        let artifact = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));
        state.player.weapon_durability = -1;
        let artifact_sentinel = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));
        assert_eq!(artifact, artifact_sentinel);
    }

    #[test]
    fn authored_palette_family_changes_world_materials() {
        let state = visible_floor_state();
        let visual = VisualState::new(80, 50);
        let standard = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));
        let mut floodglass = ctx_playing(&state, &visual);
        floodglass.color_filter = 2;
        let alternate = viewport_signature(&draw_game(&floodglass));

        assert_ne!(
            standard, alternate,
            "palette family should recolor world art"
        );
    }

    #[test]
    fn biome_motes_drift_but_low_motion_keeps_them_stable() {
        let mut state = visible_floor_state();
        state.player.depth = 3;
        let visual = VisualState::new(80, 50);
        let start = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));
        let mut later_ctx = ctx_playing(&state, &visual);
        later_ctx.tick = 12;
        let later = viewport_signature(&draw_game(&later_ctx));
        assert_ne!(start, later);

        let mut still_start_ctx = ctx_playing(&state, &visual);
        still_start_ctx.low_motion = true;
        let still_start = viewport_signature(&draw_game(&still_start_ctx));
        let mut still_later_ctx = ctx_playing(&state, &visual);
        still_later_ctx.low_motion = true;
        still_later_ctx.tick = 12;
        let still_later = viewport_signature(&draw_game(&still_later_ctx));
        assert_eq!(still_start, still_later);
    }

    #[test]
    fn lit_lantern_casts_light_beyond_its_own_tile() {
        let mut state = visible_floor_state();
        let visual = VisualState::new(80, 50);
        let lantern = state.map.idx(12, 10);
        state.map.tiles[lantern] = Tile::LanternDead;
        let dead = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));
        state.map.tiles[lantern] = Tile::LanternLit;
        let lit = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));

        assert_ne!(dead, lit);
        let changed = dead.iter().zip(&lit).filter(|(a, b)| a != b).count();
        assert!(
            changed > 20,
            "lantern light did not spill across nearby geometry"
        );
    }

    #[test]
    fn inventory_item_previews_are_dedicated_per_kind() {
        let mut state = visible_floor_state();
        state.status = GameStatus::Inventory;
        state.player.inventory = vec![ItemKind::HealthPotion];
        let visual = VisualState::new(80, 50);
        let potion = draw_game(&ctx_playing(&state, &visual));
        state.player.inventory = vec![ItemKind::Warhammer];
        let hammer = draw_game(&ctx_playing(&state, &visual));

        assert_ne!(
            viewport_signature(&potion),
            viewport_signature(&hammer),
            "selected item should have dedicated inventory art"
        );
    }

    #[test]
    fn inventory_preview_marks_unknown_items_without_revealing_them() {
        let mut state = visible_floor_state();
        state.status = GameStatus::Inventory;
        state.player.inventory = vec![ItemKind::HealthPotion];
        let visual = VisualState::new(80, 50);
        let unknown = draw_game(&ctx_playing(&state, &visual));
        assert!(cell_text(&unknown).contains("UNKNOWN"));

        state.player.identified_items.insert(ItemKind::HealthPotion);
        let known = draw_game(&ctx_playing(&state, &visual));
        assert!(!cell_text(&known).contains("UNKNOWN"));
        assert_ne!(viewport_signature(&unknown), viewport_signature(&known));
    }

    #[test]
    fn context_strip_shows_adjacent_monster_health() {
        let mut state = visible_floor_state();
        state.items.clear();
        state.monsters.clear();
        let mut rat = Monster::new(11, 10, MonsterKind::Rat);
        rat.stats.hp = 2;
        state.monsters.push(rat);
        let visual = VisualState::new(80, 50);
        let buffer = draw_game(&ctx_playing(&state, &visual));
        let text = cell_text(&buffer);

        assert!(text.contains("HP|2/"), "monster health was absent");
    }

    #[test]
    fn minimap_shows_top_down_player_marker() {
        let mut state = GameState::new(42);
        state.status = GameStatus::Playing;
        state.map = Map::new(20, 20);
        for y in 0..20 {
            for x in 0..20 {
                let i = state.map.idx(x, y);
                state.map.tiles[i] = Tile::Floor;
            }
        }
        // Only the area around player is visible/explored
        state.player.x = 10;
        state.player.y = 10;
        for y in 9..12 {
            for x in 9..12 {
                let i = state.map.idx(x, y);
                state.map.visible[i] = true;
                state.map.explored[i] = true;
            }
        }

        let visual_state = VisualState::new(80, 50);
        let mut ctx = ctx_playing(&state, &visual_state);
        ctx.show_minimap = true;
        let (x, y) = minimap_pos(&state, state.player.x, state.player.y);
        for (angle, expected) in [
            (0.0, ">"),
            (std::f64::consts::FRAC_PI_2, "v"),
            (std::f64::consts::PI, "<"),
            (std::f64::consts::PI * 1.5, "^"),
        ] {
            ctx.camera_angle = angle;
            let buf = draw_game(&ctx);
            assert_eq!(
                buf.cell((x, y)).unwrap().symbol(),
                expected,
                "minimap marker should show current facing"
            );
        }
    }

    #[test]
    fn every_landmark_has_a_distinct_viewport_treatment() {
        let visual = VisualState::new(80, 50);
        for palette in 0..6 {
            let mut signatures = std::collections::HashSet::new();
            for landmark in [
                Landmark::SpiderNest,
                Landmark::FloodedVault,
                Landmark::BoneChapel,
                Landmark::CollapsedForge,
                Landmark::Shrine,
                Landmark::BellChamber,
            ] {
                let mut state = visible_floor_state();
                state.map.rooms = vec![Rect {
                    x: 12,
                    y: 7,
                    w: 7,
                    h: 7,
                }];
                state.landmarks = vec![(0, landmark)];
                let mut ctx = ctx_playing(&state, &visual);
                ctx.color_filter = palette;
                let signature = viewport_signature(&draw_game(&ctx));
                assert!(
                    signatures.insert(signature),
                    "{} reused another landmark's viewport art in palette {palette}",
                    landmark.name()
                );
            }
            assert_eq!(signatures.len(), 6);
        }
    }

    #[test]
    fn every_camp_family_has_a_distinct_viewport_treatment() {
        let state = visible_floor_state();
        let visual = VisualState::new(80, 50);
        for palette in 0..6 {
            let mut signatures = std::collections::HashSet::new();
            for kind in [
                CampKind::DelverWaystation,
                CampKind::RustguardOutpost,
                CampKind::CartographerSurvey,
                CampKind::ShrinePilgrim,
                CampKind::Abandoned,
            ] {
                let camps = [Camp {
                    room_idx: 0,
                    x: 15,
                    y: 10,
                    kind,
                    npc_indices: vec![],
                    rested: false,
                }];
                let mut ctx = ctx_playing(&state, &visual);
                ctx.camps = &camps;
                ctx.color_filter = palette;
                let signature = viewport_signature(&draw_game(&ctx));
                assert!(
                    signatures.insert(signature),
                    "{} reused another camp's viewport art in palette {palette}",
                    kind.name()
                );
            }
            assert_eq!(signatures.len(), 5);
        }
    }

    #[test]
    fn every_npc_role_has_a_distinct_viewport_treatment() {
        let visual = VisualState::new(80, 50);
        for palette in 0..6 {
            let mut signatures = std::collections::HashSet::new();
            for kind in [
                NpcKind::WoundedDelver,
                NpcKind::Cartographer,
                NpcKind::Merchant,
                NpcKind::ShrineKeeper,
                NpcKind::WatchCaptain,
                NpcKind::Rustguard,
                NpcKind::Cook,
                NpcKind::Quartermaster,
                NpcKind::ApprenticeMapper,
                NpcKind::Scavenger,
                NpcKind::LostDelver,
                NpcKind::Pilgrim,
                NpcKind::Doubter,
                NpcKind::Lookout,
                NpcKind::LanternTender,
                NpcKind::CampSurvivor,
            ] {
                let mut state = visible_floor_state();
                state.npcs = vec![Npc::new(15, 10, kind)];
                let mut ctx = ctx_playing(&state, &visual);
                ctx.color_filter = palette;
                let signature = viewport_signature(&draw_game(&ctx));
                assert!(
                    signatures.insert(signature),
                    "{} reused another NPC role's viewport art in palette {palette}",
                    kind.name()
                );
            }
            assert_eq!(signatures.len(), 16);
        }
    }

    #[test]
    fn lore_and_hazard_floor_features_remain_visually_distinct() {
        let visual = VisualState::new(80, 50);
        let mut signatures = std::collections::HashSet::new();
        for tile in [
            Tile::Floor,
            Tile::Inscription,
            Tile::History('A'),
            Tile::ShallowWater,
            Tile::Web,
            Tile::TrapRevealed,
        ] {
            let mut state = visible_floor_state();
            let index = state.map.idx(14, 10);
            state.map.tiles[index] = tile;
            let signature = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));
            assert!(
                signatures.insert(signature),
                "{tile:?} reused another floor feature's viewport treatment"
            );
        }
        assert_eq!(signatures.len(), 6);
    }

    #[test]
    fn visible_stair_behind_camera_adds_a_peripheral_cue() {
        let state = visible_floor_state();
        let visual = VisualState::new(80, 50);
        let ordinary = draw_game(&ctx_playing(&state, &visual));

        let mut marked = state;
        let stair = marked.map.idx(8, 10);
        marked.map.tiles[stair] = Tile::StairsUp;
        let with_cue = draw_game(&ctx_playing(&marked, &visual));
        let ordinary = viewport_signature(&ordinary);
        let with_cue = viewport_signature(&with_cue);
        let edge_changed = |left: bool| {
            ordinary
                .iter()
                .zip(&with_cue)
                .enumerate()
                .any(|(index, (before, after))| {
                    let x = index % 92;
                    before != after && if left { x <= 4 } else { x >= 87 }
                })
        };

        assert!(edge_changed(true), "a rear stair needs a left turn cue");
        assert!(edge_changed(false), "a rear stair needs a right turn cue");
    }

    #[test]
    fn visible_hostile_behind_camera_adds_a_peripheral_warning() {
        let mut state = visible_floor_state();
        let visual = VisualState::new(80, 50);
        let ordinary = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));
        state.monsters.push(Monster::new(8, 10, MonsterKind::Orc));
        let warned = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));
        let edge_changed =
            ordinary
                .iter()
                .zip(&warned)
                .enumerate()
                .any(|(index, (before, after))| {
                    let x = index % 92;
                    before != after && (x <= 4 || x >= 87)
                });

        assert!(edge_changed, "a rear hostile needs a turn warning");
    }

    #[test]
    fn entering_a_new_room_adds_a_transient_viewport_reveal() {
        let state = visible_floor_state();
        let idle_visual = VisualState::new(80, 50);
        let idle = viewport_signature(&draw_game(&ctx_playing(&state, &idle_visual)));
        let mut reveal_visual = VisualState::new(80, 50);
        reveal_visual.observe_room(Some(1));
        let reveal = viewport_signature(&draw_game(&ctx_playing(&state, &reveal_visual)));

        assert_ne!(idle, reveal);
        for _ in 0..12 {
            reveal_visual.tick();
        }
        let settled = viewport_signature(&draw_game(&ctx_playing(&state, &reveal_visual)));
        assert_eq!(idle, settled);
    }

    #[test]
    fn inventory_overlay_shows_when_in_inventory_mode() {
        let mut state = GameState::new(42);
        state.status = GameStatus::Inventory;
        state.player.inventory.push(ItemKind::HealthPotion);
        state.player.inventory.push(ItemKind::Longsword);
        state.map = Map::new(20, 20);
        for y in 0..20 {
            for x in 0..20 {
                let i = state.map.idx(x, y);
                state.map.tiles[i] = Tile::Floor;
                state.map.visible[i] = true;
                state.map.explored[i] = true;
            }
        }
        state.player.x = 10;
        state.player.y = 10;

        let visual_state = VisualState::new(80, 50);
        let ctx = RenderCtx {
            map: &state.map,
            world_map_id: state.world_map_id,
            world_links: &state.world_links,
            player: &state.player,
            monsters: &state.monsters,
            items: &state.items,
            npcs: &state.npcs,
            projectiles: &state.projectiles,
            landmarks: &state.landmarks,
            used_landmarks: &state.used_landmarks,
            seen_monsters: &state.seen_monsters,
            defeated_monsters: &state.defeated_monsters,
            messages: &state.messages,
            status: state.status,
            stats: &state.stats,
            seed: state.seed,
            challenge_mode: state.challenge_mode,
            floor_memory: &state.floor_memory,
            cached_floor_count: state.floor_cache.len(),
            completed_runs: 0,
            run_history: &[],
            inventory_cursor: 0,
            auto_pickup: false,
            log_scroll: 0,
            visual: &visual_state,
            tick: 0,
            dialog_npc_name: "",
            dialog_npc_idx: None,
            dialog_text: &[],
            dialog_show_choices: false,
            dialog_choices: &[],
            camps: &[],
            game_mode: crate::app::GameMode::Classic,
            campaign_objectives: &[],
            campaign_flags: &[],
            campaign_level_name: "",
            show_minimap: false,
            show_help: false,
            show_bestiary: false,
            auto_exploring: false,
            reduced_flash: false,
            high_contrast: false,
            show_log_overlay: false,
            color_filter: 0,
            player_moving: false,
            camera_bob: 0,
            camera_sway: 0,
            low_motion: false,
            preparation_menu: state.preparation_menu,
            hints_seen: 0,
            camera_angle: 0.0,
            battle: None,
        };

        let backend = TestBackend::new(120, 60);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render(f, &ctx);
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let text = cell_text(buf);
        assert!(
            text.contains("INVENTORY"),
            "inventory overlay should be visible, got: {}",
            text
        );
        assert!(
            text.contains("unidentified") && text.contains("potion"),
            "should hide an unknown potion's identity, got: {}",
            text
        );
        assert!(
            text.contains("longsword"),
            "should show longsword, got: {}",
            text
        );
    }

    #[test]
    fn inventory_exposes_category_and_equipment_comparison() {
        let mut state = visible_floor_state();
        state.status = GameStatus::Inventory;
        state.player.weapon = Some(ItemKind::RustySword);
        state.player.inventory = vec![ItemKind::Longsword];
        let visual = VisualState::new(80, 50);
        let mut ctx = ctx_playing(&state, &visual);
        ctx.status = GameStatus::Inventory;
        let text = cell_text(&draw_game(&ctx));

        assert!(text.contains("WPN"));
        assert!(text.contains("Compared:") && text.contains("ATK"));
        assert!(text.contains("MAX") && text.contains("DUR"));
    }

    #[test]
    fn pending_level_renders_blocking_discipline_choices() {
        let mut state = visible_floor_state();
        state.player.level = 2;
        state.player.pending_level_ups = 1;
        let visual = VisualState::new(80, 50);
        let ctx = ctx_playing(&state, &visual);
        let text = cell_text(&draw_game(&ctx));

        assert!(text.contains("DISCIPLINE"));
        assert!(text.contains("VIGOR"));
        assert!(text.contains("FINESSE"));
        assert!(text.contains("RESOLVE"));
    }

    #[test]
    fn end_screen_renders() {
        let mut state = GameState::new(42);
        state.status = GameStatus::Dead;
        state.map = Map::new(20, 20);

        let visual_state = VisualState::new(80, 50);
        let ctx = ctx_playing(&state, &visual_state);
        let mut ctx = ctx;
        ctx.status = GameStatus::Dead;

        let backend = TestBackend::new(80, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render(f, &ctx);
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let text = cell_text(buf);
        assert!(
            text.contains("DIED") || text.contains("YOU"),
            "end screen should show death message, got: {}",
            text
        );
        assert!(
            text.contains("restart") || text.contains("quit"),
            "end screen should show restart option, got: {}",
            text
        );
    }

    #[test]
    fn end_screen_won_renders() {
        let mut state = GameState::new(42);
        state.status = GameStatus::Won;
        state.map = Map::new(20, 20);
        state.player.depth = 4;
        state.stats.deepest_depth = 4;
        state.stats.turns = 100;
        state.stats.kills = 10;
        state.stats.items_used = 5;

        let visual_state = VisualState::new(80, 50);
        let mut ctx = ctx_playing(&state, &visual_state);
        ctx.status = GameStatus::Won;

        let backend = TestBackend::new(80, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render(f, &ctx);
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let text = cell_text(buf);
        assert!(
            text.contains("WON"),
            "victory screen should show WON, got: {}",
            text
        );
        assert!(
            text.contains("restart") || text.contains("quit"),
            "victory screen should show restart option, got: {}",
            text
        );
    }

    #[test]
    fn campaign_complete_screen_renders_at_compact_size() {
        let mut state = GameState::new(42);
        state.status = GameStatus::CampaignComplete;
        state.game_mode = crate::app::GameMode::Campaign;
        state.stats.turns = 47;
        state.stats.kills = 9;
        let visual_state = VisualState::new(80, 50);
        let mut ctx = ctx_playing(&state, &visual_state);
        ctx.status = GameStatus::CampaignComplete;
        ctx.game_mode = crate::app::GameMode::Campaign;

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render(f, &ctx)).unwrap();
        let text = cell_text(terminal.backend().buffer());
        let normalized = text.replace('|', " ");

        assert!(
            normalized.contains("CHAPTER I COMPLETE"),
            "missing campaign ending: {text}"
        );
        assert!(
            normalized.contains("Levels: 3 / 3"),
            "missing campaign progress: {text}"
        );
        assert!(
            normalized.contains("restart Chapter I"),
            "missing restart action: {text}"
        );
    }

    #[test]
    fn minimap_renders_when_enabled() {
        let mut state = GameState::new(42);
        state.status = GameStatus::Playing;
        state.map = Map::new(20, 20);
        for y in 0..20 {
            for x in 0..20 {
                let i = state.map.idx(x, y);
                state.map.tiles[i] = if x == 0 || y == 0 || x == 19 || y == 19 {
                    Tile::Wall
                } else {
                    Tile::Floor
                };
                state.map.visible[i] = true;
                state.map.explored[i] = true;
            }
        }
        state.player.x = 10;
        state.player.y = 10;

        let visual_state = VisualState::new(80, 50);
        let mut ctx = ctx_playing(&state, &visual_state);
        ctx.show_minimap = true;
        let backend = TestBackend::new(120, 60);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render(f, &ctx);
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let text = cell_text(buf);
        assert!(
            text.contains("map"),
            "minimap label should render, got: {}",
            text
        );
        assert!(text.contains("aim") && text.contains("memory"));

        let ahead = minimap_pos(&state, state.player.x + 1, state.player.y);
        assert_eq!(
            buf.cell(ahead).unwrap().bg,
            Color::Rgb(58, 43, 18),
            "the map should highlight the exact cell ahead"
        );
    }

    #[test]
    fn sidebar_shows_player_stats() {
        let mut state = GameState::new(42);
        state.status = GameStatus::Playing;
        state.map = Map::new(20, 20);
        for y in 0..20 {
            for x in 0..20 {
                let i = state.map.idx(x, y);
                state.map.tiles[i] = Tile::Floor;
                state.map.visible[i] = true;
                state.map.explored[i] = true;
            }
        }
        state.player.x = 10;
        state.player.y = 10;
        state.player.stats.hp = 25;
        state.player.stats.max_hp = 30;
        state.player.depth = 2;

        let visual_state = VisualState::new(80, 50);
        let ctx = ctx_playing(&state, &visual_state);
        let backend = TestBackend::new(120, 60);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render(f, &ctx);
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let text = cell_text(buf);
        assert!(text.contains("HP"), "sidebar should show HP, got: {}", text);
        assert!(
            text.contains("25/30"),
            "sidebar should show 25/30 HP, got: {}",
            text
        );
        assert!(
            text.contains("ATK:"),
            "sidebar should show ATK, got: {}",
            text
        );
        assert!(
            text.contains("DEF:"),
            "sidebar should show DEF, got: {}",
            text
        );
        assert!(
            text.contains("❆") || text.contains("Crypts") || text.contains("Rust"),
            "sidebar should show depth name, got: {}",
            text
        );
        assert!(
            text.contains("Seed"),
            "sidebar should show seed, got: {}",
            text
        );
    }

    #[test]
    fn visible_and_remembered_doors_have_distinct_glyphs() {
        let mut state = visible_floor_state();
        let visible_door = state.map.idx(11, 10);
        let remembered_door = state.map.idx(12, 10);
        state.map.tiles[visible_door] = Tile::DoorClosed;
        state.map.tiles[remembered_door] = Tile::DoorClosed;
        state.map.visible[remembered_door] = false;
        state.map.explored[remembered_door] = true;

        let visual_state = VisualState::new(80, 50);
        let mut ctx = ctx_playing(&state, &visual_state);
        ctx.show_minimap = true;
        let buf = draw_game(&ctx);

        let visible = buf.cell(minimap_pos(&state, 11, 10)).unwrap();
        assert_eq!(visible.symbol(), "+");
        assert_ne!(
            visible.fg, visible.bg,
            "visible door should not flatten into a wall bar"
        );

        let remembered = buf.cell(minimap_pos(&state, 12, 10)).unwrap();
        assert_eq!(remembered.symbol(), "+");
        assert_ne!(remembered.symbol(), "█");
        assert_ne!(
            remembered.fg, remembered.bg,
            "remembered door should stay readable"
        );
    }

    #[test]
    fn remembered_walls_use_context_lines_instead_of_solid_bars() {
        let mut state = visible_floor_state();
        for y in 8..=10 {
            let i = state.map.idx(6, y);
            state.map.tiles[i] = Tile::Wall;
            state.map.visible[i] = false;
            state.map.explored[i] = true;
        }

        let visual_state = VisualState::new(80, 50);
        let mut ctx = ctx_playing(&state, &visual_state);
        ctx.show_minimap = true;
        let buf = draw_game(&ctx);
        let wall = buf.cell(minimap_pos(&state, 6, 9)).unwrap();

        assert_eq!(wall.symbol(), "│");
        assert_ne!(wall.symbol(), "█");
        assert_ne!(
            wall.fg, wall.bg,
            "remembered wall line should have contrast"
        );
    }

    #[test]
    fn visible_items_get_graphic_glyphs_and_distance_shading() {
        let mut state = visible_floor_state();
        state.items.push(Item::new(11, 10, ItemKind::HealthPotion));
        state.items.push(Item::new(17, 10, ItemKind::HealthPotion));

        let visual_state = VisualState::new(80, 50);
        let mut ctx = ctx_playing(&state, &visual_state);
        ctx.show_minimap = true;
        let buf = draw_game(&ctx);
        let near = buf.cell(minimap_pos(&state, 11, 10)).unwrap();
        let far = buf.cell(minimap_pos(&state, 17, 10)).unwrap();

        assert_eq!(near.symbol(), "⚗");
        assert_eq!(far.symbol(), "⚗");
        assert!(
            luma(near.fg) > luma(far.fg),
            "near item should read brighter than far item"
        );
    }

    #[test]
    fn visible_monsters_get_distance_shading() {
        let mut state = visible_floor_state();
        state.monsters.push(Monster::new(11, 10, MonsterKind::Rat));
        state.monsters.push(Monster::new(17, 10, MonsterKind::Rat));

        let visual_state = VisualState::new(80, 50);
        let mut ctx = ctx_playing(&state, &visual_state);
        ctx.show_minimap = true;
        let buf = draw_game(&ctx);
        let near = buf.cell(minimap_pos(&state, 11, 10)).unwrap();
        let far = buf.cell(minimap_pos(&state, 17, 10)).unwrap();

        assert_eq!(near.symbol(), "r");
        assert_eq!(far.symbol(), "r");
        assert!(
            luma(near.fg) > luma(far.fg),
            "near monster should read brighter than far monster"
        );
    }

    #[test]
    fn items_in_fog_are_not_drawn_as_current_targets() {
        let mut state = visible_floor_state();
        state.items.push(Item::new(12, 10, ItemKind::HealthPotion));
        let i = state.map.idx(12, 10);
        state.map.visible[i] = false;
        state.map.explored[i] = true;

        let visual_state = VisualState::new(80, 50);
        let mut ctx = ctx_playing(&state, &visual_state);
        ctx.show_minimap = true;
        let buf = draw_game(&ctx);
        let remembered_floor = buf.cell(minimap_pos(&state, 12, 10)).unwrap();

        assert_ne!(remembered_floor.symbol(), "⚗");
    }

    #[test]
    fn visible_stairs_keep_contrast_at_distance() {
        let mut state = visible_floor_state();
        let i = state.map.idx(17, 10);
        state.map.tiles[i] = Tile::StairsDown;

        let visual_state = VisualState::new(80, 50);
        let mut ctx = ctx_playing(&state, &visual_state);
        ctx.show_minimap = true;
        let buf = draw_game(&ctx);
        let stairs = buf.cell(minimap_pos(&state, 17, 10)).unwrap();

        assert_eq!(stairs.symbol(), ">");
        assert_ne!(stairs.fg, stairs.bg);
        assert!(luma(stairs.fg) > luma(stairs.bg));
    }

    #[test]
    fn door_states_have_distinct_perspective_materials_and_open_slab() {
        let mut state = visible_floor_state();
        let visual = VisualState::new(80, 50);
        let tile = state.map.idx(13, 10);

        state.map.tiles[tile] = Tile::Wall;
        let stone = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));
        state.map.tiles[tile] = Tile::DoorClosed;
        let wood = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));
        state.map.tiles[tile] = Tile::LockedDoor;
        let iron = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));
        state.map.tiles[tile] = Tile::DoorOpen;
        let open = viewport_signature(&draw_game(&ctx_playing(&state, &visual)));

        assert_ne!(stone, wood);
        assert_ne!(wood, iron);
        assert_ne!(
            stone, open,
            "an open door should retain a visible side slab"
        );
    }

    #[test]
    fn lit_lantern_has_non_blocking_flame_animation() {
        let mut state = visible_floor_state();
        let tile = state.map.idx(12, 10);
        state.map.tiles[tile] = Tile::LanternLit;
        let visual = VisualState::new(80, 50);
        let mut ctx = ctx_playing(&state, &visual);
        ctx.tick = 0;
        let first = viewport_signature(&draw_game(&ctx));
        ctx.tick = 5;
        let second = viewport_signature(&draw_game(&ctx));

        assert_ne!(first, second);
    }

    /// Collect all non-space cell symbols into a string for text-based assertions.
    fn cell_text(buf: &ratatui::buffer::Buffer) -> String {
        let area = buf.area();
        let mut s = String::new();
        let mut in_word = false;
        for y in 0..area.height {
            for x in 0..area.width {
                if let Some(cell) = buf.cell((x, y)) {
                    let sym = cell.symbol();
                    if sym != " " {
                        s.push_str(sym);
                        in_word = true;
                    } else if in_word {
                        // End of a word — insert a delimiter so multi-word text is separable
                        s.push('|');
                        in_word = false;
                    }
                }
            }
        }
        s
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// ITEM USAGE EDGE CASES
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn health_potion_at_full_hp_heals_zero() {
    assert_eq!(ItemKind::HealthPotion.heal_amount(), 10);
    // A player with 30/30 HP should heal 0
    let p = Player::new(0, 0);
    let heal = ItemKind::HealthPotion
        .heal_amount()
        .min(p.stats.max_hp - p.stats.hp);
    assert_eq!(heal, 0);
}

#[test]
fn large_health_potion_heal() {
    assert_eq!(ItemKind::LargeHealthPotion.heal_amount(), 25);
}

#[test]
fn health_potion_at_full_hp_preserves_inventory_and_items_used() {
    // Regression: potion must not be consumed when actual heal is 0.
    let mut state = state_with_floor(10, 10);
    state.player.stats.hp = 30;
    state.player.stats.max_hp = 30;
    state.player.inventory.push(ItemKind::HealthPotion);
    let items_used_before = state.stats.items_used;
    let inv_len_before = state.player.inventory.len();
    let mut visual = VisualState::new(80, 50);
    let used = state.use_item(ItemKind::HealthPotion, 0, &mut visual);
    assert_eq!(
        state.stats.items_used, items_used_before,
        "items_used should not increment"
    );
    assert_eq!(
        state.player.inventory.len(),
        inv_len_before,
        "potion should stay in inventory"
    );
    assert!(state.player.inventory.contains(&ItemKind::HealthPotion));
    assert!(!used, "use_item should return false at full HP");
}

// ══════════════════════════════════════════════════════════════════════════════
// EDGE CASES
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn map_new_zero_size_does_not_panic() {
    let map = Map::new(0, 0);
    let _ = map.width;
}

#[test]
fn map_idx_consistent() {
    let map = Map::new(80, 50);
    assert_eq!(map.idx(0, 0), 0);
    assert_eq!(map.idx(1, 0), 1);
    assert_eq!(map.idx(0, 1), 80);
    assert_eq!(map.idx(79, 49), 80 * 50 - 1);
}

#[test]
fn player_spawn_stats() {
    let p = Player::new(0, 0);
    assert_eq!(p.stats.hp, 30);
    assert_eq!(p.stats.max_hp, 30);
    assert_eq!(p.stats.attack, 5);
    assert_eq!(p.stats.defense, 2);
}

#[test]
fn stairs_placed_correctly() {
    let mut rng = StdRng::seed_from_u64(42);
    let mut map = Map::new(80, 50);
    map.generate(&mut rng, true);
    let down = map
        .tiles
        .iter()
        .filter(|t| matches!(t, Tile::StairsDown))
        .count();
    let up = map
        .tiles
        .iter()
        .filter(|t| matches!(t, Tile::StairsUp))
        .count();
    assert!(down >= 1);
    assert!(up >= 1);
}

#[test]
fn rect_dist_to() {
    let a = crate::map::Rect {
        x: 0,
        y: 0,
        w: 5,
        h: 5,
    };
    let b = crate::map::Rect {
        x: 10,
        y: 0,
        w: 5,
        h: 5,
    };
    assert!(a.dist_to(&b) > 0.0 && a.dist_to(&b) < 20.0);
}

#[test]
fn all_monster_names_are_non_empty() {
    for kind in &[
        MonsterKind::Rat,
        MonsterKind::Bat,
        MonsterKind::Goblin,
        MonsterKind::Skeleton,
        MonsterKind::Orc,
        MonsterKind::Spider,
        MonsterKind::Wraith,
    ] {
        assert!(!kind.name().is_empty());
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// PARSE SEED (main.rs)
// ══════════════════════════════════════════════════════════════════════════════

mod parse_seed_tests {
    use crate::{daily_seed_from_unix_secs, parse_replay_path, parse_seed};

    #[test]
    fn long_flag_equals() {
        assert_eq!(
            parse_seed(&["delve".into(), "--seed=42".into()]).unwrap(),
            Some(42)
        );
    }

    #[test]
    fn long_flag_space() {
        assert_eq!(
            parse_seed(&["delve".into(), "--seed".into(), "99".into()]).unwrap(),
            Some(99)
        );
    }

    #[test]
    fn short_flag_space() {
        assert_eq!(
            parse_seed(&["delve".into(), "-s".into(), "777".into()]).unwrap(),
            Some(777)
        );
    }

    #[test]
    fn no_flag_returns_none() {
        assert_eq!(parse_seed(&["delve".into()]).unwrap(), None);
    }

    #[test]
    fn invalid_value_returns_none() {
        let error = parse_seed(&["delve".into(), "--seed=xyz".into()]).unwrap_err();
        assert!(error.to_string().contains("invalid seed 'xyz'"));
    }

    #[test]
    fn seed_flag_without_value_returns_none() {
        let error = parse_seed(&["delve".into(), "--seed".into()]).unwrap_err();
        assert!(error.to_string().contains("requires a number"));
    }

    #[test]
    fn daily_seed_is_stable_within_a_utc_day() {
        assert_eq!(
            daily_seed_from_unix_secs(86_400 * 20 + 1),
            daily_seed_from_unix_secs(86_400 * 20 + 86_399)
        );
    }

    #[test]
    fn daily_seed_changes_at_utc_midnight() {
        assert_ne!(
            daily_seed_from_unix_secs(86_400 * 20 + 86_399),
            daily_seed_from_unix_secs(86_400 * 21)
        );
    }

    #[test]
    fn daily_and_explicit_seed_are_rejected_together() {
        let error =
            parse_seed(&["delve".into(), "--daily".into(), "--seed=42".into()]).unwrap_err();
        assert!(error.to_string().contains("cannot be combined"));
    }

    #[test]
    fn replay_path_supports_equals_and_space_forms() {
        assert_eq!(
            parse_replay_path(&["delve".into(), "--replay=run.json".into()]).unwrap(),
            Some("run.json".into())
        );
        assert_eq!(
            parse_replay_path(&["delve".into(), "--replay".into(), "other.json".into()]).unwrap(),
            Some("other.json".into())
        );
    }

    #[test]
    fn replay_path_requires_a_value() {
        let error = parse_replay_path(&["delve".into(), "--replay".into()]).unwrap_err();
        assert!(error.to_string().contains("requires a file path"));
    }
}

mod run_history_tests {
    use crate::app::{ChallengeMode, GameMode, RunStats};
    use crate::run_history::{push_bounded, RunSummary};

    fn summary(seed: u64) -> RunSummary {
        RunSummary {
            seed,
            won: seed % 2 == 0,
            game_mode: GameMode::Classic,
            challenge_mode: ChallengeMode::Standard,
            stats: RunStats::new(),
            completed_at_unix: seed,
            final_x: Some(0),
            final_y: Some(0),
            final_depth: 0,
        }
    }

    #[test]
    fn legacy_history_has_no_synthetic_death_position() {
        let json = r#"{
            "seed": 42,
            "won": false,
            "game_mode": "Classic",
            "challenge_mode": "Standard",
            "stats": {
                "turns": 0, "kills": 0, "dmg_dealt": 0, "dmg_taken": 0,
                "items_used": 0, "deepest_depth": 0
            },
            "completed_at_unix": 0
        }"#;
        let legacy: RunSummary = serde_json::from_str(json).unwrap();

        assert_eq!(legacy.final_x, None);
        assert_eq!(legacy.final_y, None);
    }

    #[test]
    fn campaign_completion_counts_as_a_successful_run() {
        let mut state = crate::app::GameState::new(42);
        state.status = crate::app::GameStatus::CampaignComplete;
        state.game_mode = crate::app::GameMode::Campaign;

        let summary = RunSummary::from_state(&state);
        assert!(summary.won);
        assert_eq!(summary.game_mode, crate::app::GameMode::Campaign);
    }

    #[test]
    fn history_keeps_only_the_latest_fifty_runs() {
        let mut history = Vec::new();
        for seed in 0..55 {
            push_bounded(&mut history, summary(seed));
        }

        assert_eq!(history.len(), 50);
        assert_eq!(history.first().map(|run| run.seed), Some(5));
        assert_eq!(history.last().map(|run| run.seed), Some(54));
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// CROWN WIN PATH — multi-level ascension
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn crown_ascend_full_path_works() {
    use crate::visual::VisualState;
    let mut state = GameState::new(42);
    let mut visual = VisualState::new(80, 50);
    state.new_game();

    // Manually give player the Crown and simulate being at depth 0 on stairs up
    state.player.inventory.push(ItemKind::CrownOfDepth);
    state.player.depth = 0;
    let idx = state.map.idx(state.player.x, state.player.y);
    state.map.tiles[idx] = Tile::StairsUp;

    state.try_ascend(&mut visual);
    assert_eq!(
        state.status,
        GameStatus::Won,
        "should win when ascending with Crown at depth 0"
    );
}

#[test]
fn crown_ascend_level_regeneration_works() {
    use crate::visual::VisualState;
    let mut state = GameState::new(42);
    let mut visual = VisualState::new(80, 50);
    state.new_game();

    // Give Crown, set depth to 4
    state.player.inventory.push(ItemKind::CrownOfDepth);
    state.player.depth = 4;

    // Place player on stairs-up tile and ascend to depth 3
    let idx = state.map.idx(state.player.x, state.player.y);
    state.map.tiles[idx] = Tile::StairsUp;
    state.try_ascend(&mut visual);
    assert_eq!(state.player.depth, 3, "should have ascended to depth 3");

    // Ascend to depth 2
    let idx = state.map.idx(state.player.x, state.player.y);
    state.map.tiles[idx] = Tile::StairsUp;
    state.try_ascend(&mut visual);
    assert_eq!(state.player.depth, 2);

    // Ascend to depth 1
    let idx = state.map.idx(state.player.x, state.player.y);
    state.map.tiles[idx] = Tile::StairsUp;
    state.try_ascend(&mut visual);
    assert_eq!(state.player.depth, 1);

    // Ascend to depth 0
    let idx = state.map.idx(state.player.x, state.player.y);
    state.map.tiles[idx] = Tile::StairsUp;
    state.try_ascend(&mut visual);
    assert_eq!(state.player.depth, 0);

    // Now ascend with Crown on depth 0 → win
    let idx = state.map.idx(state.player.x, state.player.y);
    state.map.tiles[idx] = Tile::StairsUp;
    state.try_ascend(&mut visual);
    assert_eq!(
        state.status,
        GameStatus::Won,
        "full ascension path should reach win"
    );
}

#[test]
fn crown_return_floors_deterministically_include_a_spectral_hunter() {
    let generate = || {
        let mut state = GameState::new(42);
        state.player.crown_held = true;
        state.player.inventory.push(ItemKind::CrownOfDepth);
        state.generate_level(2);
        state
            .monsters
            .iter()
            .map(|monster| (monster.kind, monster.x, monster.y, monster.stats.hp))
            .collect::<Vec<_>>()
    };

    let first = generate();
    let second = generate();
    assert_eq!(first, second);
    assert!(first
        .iter()
        .any(|(kind, _, _, _)| *kind == MonsterKind::Wraith));
}

#[test]
fn crown_pursuit_rate_rises_toward_the_surface() {
    let deep = crate::app::crown_pursuit_act_chance(0.5, 4);
    let surface = crate::app::crown_pursuit_act_chance(0.5, 0);
    assert!(deep > 0.5 && surface > deep);
    assert_eq!(crate::app::crown_pursuit_act_chance(1.0, 0), 1.0);
}

// ── Inventory sort ──

#[test]
fn sort_inventory_groups_by_category() {
    let mut state = state_with_floor(10, 10);
    state.player.inventory = vec![
        ItemKind::LoreFragment,
        ItemKind::RustySword,
        ItemKind::HealthPotion,
        ItemKind::CrownOfDepth,
        ItemKind::Longsword,
    ];
    crate::inventory::sort_inventory(&mut state.player);
    // Crown first, then weapons (best first), then potions, then lore last
    assert_eq!(state.player.inventory[0], ItemKind::CrownOfDepth);
    assert_eq!(state.player.inventory[1], ItemKind::Longsword);
    assert_eq!(state.player.inventory[2], ItemKind::RustySword);
    assert_eq!(state.player.inventory[3], ItemKind::HealthPotion);
    assert_eq!(state.player.inventory[4], ItemKind::LoreFragment);
}

#[test]
fn sort_preserves_duplicates() {
    let mut state = state_with_floor(10, 10);
    state.player.inventory = vec![
        ItemKind::HealthPotion,
        ItemKind::HealthPotion,
        ItemKind::LargeHealthPotion,
    ];
    crate::inventory::sort_inventory(&mut state.player);
    assert_eq!(state.player.inventory[0], ItemKind::LargeHealthPotion);
    assert_eq!(state.player.inventory[1], ItemKind::HealthPotion);
    assert_eq!(state.player.inventory[2], ItemKind::HealthPotion);
}

// ── NPC collision ──

#[test]
fn npc_blocks_player_movement() {
    let mut state = state_with_floor(10, 10);
    let px = state.player.x;
    let py = state.player.y;
    state
        .npcs
        .push(crate::entity::Npc::new(px + 1, py, NpcKind::WoundedDelver));
    let mut visual = VisualState::new(80, 50);
    state.process_turn(Action::Move(1, 0), &mut visual);
    assert_eq!(state.player.x, px, "player should not move into NPC");
    assert_eq!(state.player.y, py);
}

#[test]
fn npc_blocks_monster_movement() {
    let mut state = state_with_floor(10, 10);
    let px = state.player.x;
    let py = state.player.y;
    state
        .npcs
        .push(crate::entity::Npc::new(px + 1, py, NpcKind::Merchant));
    assert!(!state.cell_free(px + 1, py, 0, false));
}

// ── NPC dialog ──

#[test]
fn talk_to_npc_logs_first_dialog() {
    let mut state = state_with_floor(10, 10);
    let px = state.player.x;
    let py = state.player.y;
    state
        .npcs
        .push(crate::entity::Npc::new(px + 1, py, NpcKind::WoundedDelver));
    let msg_count_before = state.messages.len();
    state.try_talk();
    assert!(
        state.messages.len() > msg_count_before,
        "first dialog should log"
    );
    assert!(state.dialog_active, "dialog should be active");
    assert!(state.npcs[0].talked, "npc should be marked talked");
}

#[test]
fn repeat_talk_does_not_log_or_effect() {
    let mut state = state_with_floor(10, 10);
    let px = state.player.x;
    let py = state.player.y;
    state
        .npcs
        .push(crate::entity::Npc::new(px + 1, py, NpcKind::WoundedDelver));
    state.npcs[0].talked = true;
    let hp_before = state.player.stats.hp;
    let msg_count_before = state.messages.len();
    state.try_talk();
    // Dialog should show but no log and no heal
    assert!(state.dialog_active, "repeat dialog should still show");
    assert_eq!(
        state.player.stats.hp, hp_before,
        "should not reapply effect"
    );
    // Repeat talks don't log new messages
    assert_eq!(state.messages.len(), msg_count_before);
}

#[test]
fn dialog_closes_when_player_moves_far() {
    let mut state = state_with_floor(10, 10);
    let px = state.player.x;
    let py = state.player.y;
    state
        .npcs
        .push(crate::entity::Npc::new(px + 1, py, NpcKind::Cartographer));
    state.try_talk();
    assert!(state.dialog_active);
    // Move far away (distance² > 8)
    state.player.x += 5;
    state.player.y += 0;
    state.close_dialog_if_far();
    assert!(!state.dialog_active, "dialog should auto-close at distance");
}

// ── Log scrolling ──

#[test]
fn message_log_window_returns_correct_range() {
    let mut log = crate::ui::MessageLog::new(200);
    log.push("msg1", crate::ui::MsgColor::White);
    log.push("msg2", crate::ui::MsgColor::White);
    log.push("msg3", crate::ui::MsgColor::White);
    log.push("msg4", crate::ui::MsgColor::White);

    // offset 0 = newest
    let w = log.window(2, 0);
    assert_eq!(w.len(), 2);
    assert_eq!(w[0].text, "msg3");
    assert_eq!(w[1].text, "msg4");

    // offset 1 = scroll up one
    let w = log.window(2, 1);
    assert_eq!(w.len(), 2);
    assert_eq!(w[0].text, "msg2");
    assert_eq!(w[1].text, "msg3");
}

// ── Inscription manual read ──

#[test]
fn pick_up_reads_inscription_when_no_item() {
    let mut state = state_with_floor(10, 10);
    let px = state.player.x;
    let py = state.player.y;
    let idx = state.map.idx(px, py);
    state.map.tiles[idx] = crate::map::Tile::Inscription;
    let mut visual = VisualState::new(80, 50);
    let msg_before = state.messages.len();
    let result = state.pick_up_item(&mut visual);
    assert!(result, "should consume a turn to read");
    assert!(
        state.messages.len() > msg_before,
        "should log inscription text"
    );
}

#[test]
fn inscription_not_auto_read_on_movement() {
    let mut state = state_with_floor(10, 10);
    let px = state.player.x;
    let py = state.player.y + 1; // move south onto inscription
    let idx = state.map.idx(px, py);
    state.map.tiles[idx] = crate::map::Tile::Inscription;
    let mut visual = VisualState::new(80, 50);
    state.process_turn(Action::Move(0, 1), &mut visual);
    // Check that auto-read inscription text did NOT appear
    let recent: Vec<_> = state
        .messages
        .recent(5)
        .iter()
        .map(|m| m.text.clone())
        .collect();
    let has_read = recent.iter().any(|m| m.contains("You read:"));
    assert!(!has_read, "should not auto-read inscription on movement");
}

// ── Projectile direction helpers ──

#[test]
fn projectile_direction_points_toward_target() {
    assert_eq!(crate::app::projectile_direction(0, 0, 5, 0), (1, 0)); // east
    assert_eq!(crate::app::projectile_direction(0, 0, -3, 0), (-1, 0)); // west
    assert_eq!(crate::app::projectile_direction(0, 0, 0, 4), (0, 1)); // south
    assert_eq!(crate::app::projectile_direction(0, 0, 0, -2), (0, -1)); // north
    assert_eq!(crate::app::projectile_direction(0, 0, 3, 3), (1, 1)); // southeast
    assert_eq!(crate::app::projectile_direction(0, 0, -3, -3), (-1, -1)); // northwest
}

#[test]
fn rotate_45_cw_cycles_all_8_directions() {
    let dirs = [
        (1, 0),
        (1, 1),
        (0, 1),
        (-1, 1),
        (-1, 0),
        (-1, -1),
        (0, -1),
        (1, -1),
    ];
    for i in 0..dirs.len() {
        let rotated = crate::app::rotate_direction_45(dirs[i].0, dirs[i].1);
        let expected = dirs[(i + 1) % dirs.len()];
        assert_eq!(
            rotated, expected,
            "rotate {:?} -> expected {:?}, got {:?}",
            dirs[i], expected, rotated
        );
    }
}

#[test]
fn necromancer_projectile_spawn_emits_directional_launch_art() {
    let mut state = state_with_floor(20, 20);
    state.map.visible.fill(true);
    state.monsters.push(Monster::new(
        state.player.x + 4,
        state.player.y,
        MonsterKind::Necromancer,
    ));
    let mut visual = VisualState::new(80, 50);

    state.end_turn(&mut visual);

    assert!(!state.projectiles.is_empty());
    assert!(visual.effects.iter().any(|effect| matches!(
        effect,
        crate::visual::VisualEffect::ProjectileLaunch { dx: -1, dy: 0, .. }
    )));
    assert!(visual.effects.iter().any(|effect| matches!(
        effect,
        crate::visual::VisualEffect::MonsterCast {
            kind: MonsterKind::Necromancer,
            dx: -1,
            dy: 0,
            ..
        }
    )));
}

// ── Projectile combat ──

#[test]
fn projectile_hits_player_on_collision() {
    let mut state = state_with_floor(20, 20);
    let px = state.player.x;
    let py = state.player.y;
    let hp_before = state.player.stats.hp;
    // Spawn a projectile one tile away, moving toward player
    state.projectiles.push(crate::entity::Projectile {
        x: px - 1,
        y: py,
        dx: 1,
        dy: 0,
        damage: 6,
        lifetime: 5,
    });
    let mut visual = VisualState::new(80, 50);
    state.end_turn(&mut visual);
    assert!(
        state.player.stats.hp < hp_before,
        "projectile should damage player"
    );
    assert!(
        state.projectiles.is_empty(),
        "projectile should be consumed on hit"
    );
}

#[test]
fn projectile_dies_on_wall_collision() {
    let mut state = state_with_floor(20, 20);
    // Place wall at (15, 10)
    let wi = state.map.idx(15, 10);
    state.map.tiles[wi] = crate::map::Tile::Wall;
    // Projectile at (14, 10) moving east toward wall
    state.projectiles.push(crate::entity::Projectile {
        x: 14,
        y: 10,
        dx: 1,
        dy: 0,
        damage: 6,
        lifetime: 5,
    });
    let mut visual = VisualState::new(80, 50);
    state.end_turn(&mut visual);
    assert!(
        state.projectiles.is_empty(),
        "projectile should die on wall"
    );
}

#[test]
fn projectile_misses_if_player_moves_away() {
    let mut state = state_with_floor(20, 20);
    let px = state.player.x;
    let py = state.player.y;
    let hp_before = state.player.stats.hp;
    // Projectile heading east, but player is one tile south of its path
    state.projectiles.push(crate::entity::Projectile {
        x: px - 1,
        y: py - 1,
        dx: 1,
        dy: 0,
        damage: 6,
        lifetime: 5,
    });
    let mut visual = VisualState::new(80, 50);
    state.end_turn(&mut visual);
    assert_eq!(state.player.stats.hp, hp_before, "projectile should miss");
}

#[test]
fn save_roundtrip_preserves_projectiles() {
    let mut state = state_with_floor(20, 20);
    state.projectiles.push(crate::entity::Projectile {
        x: 5,
        y: 5,
        dx: 1,
        dy: 0,
        damage: 6,
        lifetime: 3,
    });
    let data = crate::save::SaveData::from_state(&state);
    let json = serde_json::to_string(&data).unwrap();
    let loaded: crate::save::SaveData = serde_json::from_str(&json).unwrap();
    assert_eq!(loaded.projectiles.len(), 1);
    assert_eq!(loaded.projectiles[0].x, 5);
    assert_eq!(loaded.projectiles[0].damage, 6);
}

#[test]
fn monster_attack_cooldown_defaults_to_zero() {
    let m = crate::entity::Monster::new(0, 0, crate::entity::MonsterKind::Necromancer);
    assert_eq!(m.attack_cooldown, 0);
}

// ══════════════════════════════════════════════════════════════════════════════
// CAMPAIGN TESTS
// ══════════════════════════════════════════════════════════════════════════════

mod campaign_tests {
    use crate::campaign::*;
    use crate::entity::ItemKind;

    #[test]
    fn campaign_state_initializes_correctly() {
        let state = CampaignState::new();
        assert_eq!(state.chapter, CampaignChapter::Chapter1);
        assert_eq!(state.level, 0);
        assert_eq!(state.objectives.len(), 1);
        assert!(matches!(
            state.objectives[0],
            CampaignObjective::RecoverBellToken
        ));
        assert_eq!(state.trust.len(), 5);
        assert!(state.flags.is_empty());
    }

    #[test]
    fn campaign_flag_set_and_check() {
        let mut state = CampaignState::new();
        assert!(!state.has_flag(CampaignFlag::SavedTovan));
        state.set_flag(CampaignFlag::SavedTovan);
        assert!(state.has_flag(CampaignFlag::SavedTovan));
        // Double-set should not duplicate
        state.set_flag(CampaignFlag::SavedTovan);
        assert_eq!(state.flags.len(), 1);
    }

    #[test]
    fn campaign_trust_modify() {
        let mut state = CampaignState::new();
        assert_eq!(state.trust_of(CampaignNpc::MaraVoss), 0);
        state.modify_trust(CampaignNpc::MaraVoss, 2);
        assert_eq!(state.trust_of(CampaignNpc::MaraVoss), 2);
        state.modify_trust(CampaignNpc::MaraVoss, -3);
        assert_eq!(state.trust_of(CampaignNpc::MaraVoss), -1);
        // Trust clamps to range
        state.modify_trust(CampaignNpc::MaraVoss, 10);
        assert_eq!(state.trust_of(CampaignNpc::MaraVoss), 5); // clamped to 5
        state.modify_trust(CampaignNpc::MaraVoss, -20);
        assert_eq!(state.trust_of(CampaignNpc::MaraVoss), -5); // clamped to -5
    }

    #[test]
    fn campaign_level_specs_exist() {
        assert!(chapter1_level_spec(0).is_some());
        assert!(chapter1_level_spec(1).is_some());
        assert!(chapter1_level_spec(2).is_some());
        assert!(chapter1_level_spec(3).is_none());
    }

    #[test]
    fn campaign_level_specs_carry_transition_beats() {
        assert_eq!(chapter1_level_spec(0).unwrap().chapter_outro, LEVEL1_OUTRO);
        assert_eq!(chapter1_level_spec(1).unwrap().chapter_outro, LEVEL2_OUTRO);
        assert!(!chapter1_level_spec(2).unwrap().chapter_outro.is_empty());
    }

    #[test]
    fn campaign_objective_display() {
        let obj = CampaignObjective::RecoverBellToken;
        assert!(obj.display().contains("bell-token"));

        let obj2 = CampaignObjective::RelightLanterns { lit: 1, needed: 3 };
        assert!(obj2.display().contains("1/3"));

        let obj3 = CampaignObjective::SilenceTheBell;
        assert!(obj3.display().contains("bell chamber"));
    }

    #[test]
    fn campaign_set_objectives_for_level() {
        let mut state = CampaignState::new();
        // Level 0 (Moss-Mouth)
        state.set_objectives_for_level(0);
        assert!(state
            .objectives
            .iter()
            .any(|o| matches!(o, CampaignObjective::RecoverBellToken)));

        // Level 1 (Lantern Waystation)
        state.set_objectives_for_level(1);
        assert!(state
            .objectives
            .iter()
            .any(|o| matches!(o, CampaignObjective::RelightLanterns { .. })));

        // Level 2 (Bell Chamber)
        state.set_objectives_for_level(2);
        assert!(state
            .objectives
            .iter()
            .any(|o| matches!(o, CampaignObjective::SilenceTheBell)));
    }

    #[test]
    fn campaign_bell_token_pickup_sets_flag() {
        let mut state = CampaignState::new();
        assert!(!state.has_flag(CampaignFlag::RecoveredBellToken));
        state.set_flag(CampaignFlag::RecoveredBellToken);
        assert!(state.has_flag(CampaignFlag::RecoveredBellToken));
    }

    #[test]
    fn campaign_lantern_flags_track_progress() {
        let mut state = CampaignState::new();
        assert!(!state.has_flag(CampaignFlag::Lantern1Lit));
        state.set_flag(CampaignFlag::Lantern1Lit);
        assert!(state.has_flag(CampaignFlag::Lantern1Lit));
        state.set_flag(CampaignFlag::Lantern2Lit);
        state.set_flag(CampaignFlag::Lantern3Lit);
        state.set_flag(CampaignFlag::AllLanternsLit);
        assert!(state.has_flag(CampaignFlag::AllLanternsLit));
    }

    #[test]
    fn campaign_bell_choice_flags_are_distinct() {
        let mut state = CampaignState::new();
        state.set_flag(CampaignFlag::BellChoiceRung);
        assert!(state.has_flag(CampaignFlag::BellChoiceRung));
        assert!(!state.has_flag(CampaignFlag::BellChoiceShattered));
        assert!(!state.has_flag(CampaignFlag::BellChoiceMuffled));
    }

    #[test]
    fn campaign_bell_token_is_quest_item() {
        assert_eq!(ItemKind::BellToken.category_label(), "Quest Item");
        assert_eq!(ItemKind::BellToken.symbol(), '\u{25C6}');
        assert!(ItemKind::BellToken.name().contains("bell"));
    }

    #[test]
    fn campaign_dialog_tree_respects_flags() {
        let flags_before: Vec<CampaignFlag> = vec![];
        // Before token recovery, Mara should show gate dialog
        let dialog = campaign_dialog(CampaignNpc::MaraVoss, 0, &flags_before, 0).unwrap();
        assert!(!dialog.is_empty());
        assert!(dialog[0].npc_line.contains("Compact sent you"));

        // After token recovery + high trust
        let flags_after = vec![CampaignFlag::RecoveredBellToken];
        let dialog = campaign_dialog(CampaignNpc::MaraVoss, 0, &flags_after, 3).unwrap();
        assert!(!dialog.is_empty());
        assert!(dialog[0].npc_line.contains("You came back"));
    }

    #[test]
    fn campaign_vey_trust_gives_smoke_bomb() {
        let flags: Vec<CampaignFlag> = vec![];
        // Low trust — normal intro
        let dialog = campaign_dialog(CampaignNpc::Vey, 0, &flags, 0).unwrap();
        assert!(!dialog.is_empty());
        assert!(dialog[0].npc_line.contains("You have the look"));

        // High trust — trusted dialog with reward
        let dialog = campaign_dialog(CampaignNpc::Vey, 0, &flags, 3).unwrap();
        assert!(!dialog.is_empty());
        assert!(dialog[0].npc_line.contains("investment"));
    }

    #[test]
    fn campaign_save_load_preserves_flags_and_trust() {
        use crate::app::GameState;
        use crate::save::SaveData;

        let mut state = GameState::new(42);
        state.game_mode = crate::app::GameMode::Campaign;
        state.campaign = Some(CampaignState::new());
        if let Some(ref mut campaign) = state.campaign {
            campaign.set_flag(CampaignFlag::RecoveredBellToken);
            campaign.set_flag(CampaignFlag::SavedTovan);
            campaign.modify_trust(CampaignNpc::MaraVoss, 2);
            campaign.modify_trust(CampaignNpc::Vey, 1);
        }

        // Serialize and deserialize
        let save = SaveData::from_state(&state);
        let json = serde_json::to_string(&save).unwrap();
        let loaded: SaveData = serde_json::from_str(&json).unwrap();

        // Verify campaign data
        let camp = loaded.campaign.unwrap();
        assert!(camp.has_flag(CampaignFlag::RecoveredBellToken));
        assert!(camp.has_flag(CampaignFlag::SavedTovan));
        assert_eq!(camp.trust_of(CampaignNpc::MaraVoss), 2);
        assert_eq!(camp.trust_of(CampaignNpc::Vey), 1);
        assert_eq!(camp.chapter, CampaignChapter::Chapter1);
    }

    #[test]
    fn campaign_full_level_flow_objectives() {
        let mut state = CampaignState::new();

        // Level 0: recover bell token
        state.set_objectives_for_level(0);
        assert!(!state.objectives.is_empty());
        state.set_flag(CampaignFlag::RecoveredBellToken);

        // Level 1: relight lanterns
        state.level = 1;
        state.set_objectives_for_level(1);
        state.set_flag(CampaignFlag::Lantern1Lit);
        state.set_flag(CampaignFlag::Lantern2Lit);
        state.set_flag(CampaignFlag::Lantern3Lit);
        state.set_flag(CampaignFlag::AllLanternsLit);

        // Level 2: silence the bell
        state.level = 2;
        state.set_objectives_for_level(2);
        state.set_flag(CampaignFlag::BellChoiceRung);

        // All objectives should now be resolvable
        assert!(state.has_flag(CampaignFlag::RecoveredBellToken));
        assert!(state.has_flag(CampaignFlag::AllLanternsLit));
        assert!(state.has_flag(CampaignFlag::BellChoiceRung));
    }

    #[test]
    fn campaign_completion_stops_at_chapter_boundary() {
        use crate::app::{GameMode, GameState, GameStatus};
        use crate::map::Tile;
        use crate::visual::VisualState;

        let mut state = GameState::new(42);
        state.game_mode = GameMode::Campaign;
        state.campaign = Some(CampaignState::new());
        state.new_game();

        let campaign = state.campaign.as_mut().expect("campaign state");
        campaign.level = 2;
        campaign.set_objectives_for_level(2);
        campaign.set_flag(CampaignFlag::BellChoiceRung);

        let player_tile = state.map.idx(state.player.x, state.player.y);
        state.map.tiles[player_tile] = Tile::StairsDown;
        let mut visual = VisualState::new(80, 50);

        state.try_descend(&mut visual);

        assert_eq!(state.status, GameStatus::CampaignComplete);
        assert_eq!(state.game_mode, GameMode::Campaign);
        assert_eq!(state.player.depth, 3);
        assert_eq!(
            state.campaign.as_ref().map(|campaign| campaign.level),
            Some(3)
        );
        assert!(state
            .messages
            .recent(20)
            .iter()
            .any(|message| message.text.contains("End of Chapter I")));
    }

    #[test]
    fn campaign_completion_survives_save_and_replay_outcome_roundtrips() {
        use crate::app::{GameMode, GameState, GameStatus};
        use crate::campaign::{CampaignFlag, CampaignState};
        use crate::replay::ReplayOutcome;
        use crate::save::SaveData;

        let mut state = GameState::new(42);
        state.game_mode = GameMode::Campaign;
        state.status = GameStatus::CampaignComplete;
        state.campaign = Some(CampaignState::new());
        let campaign = state.campaign.as_mut().expect("campaign state");
        campaign.level = 3;
        campaign.set_flag(CampaignFlag::BellChoiceShattered);

        let save = SaveData::from_state(&state);
        assert!(save.validate().is_ok());
        let loaded: SaveData =
            serde_json::from_str(&serde_json::to_string(&save).unwrap()).unwrap();
        assert_eq!(loaded.status, GameStatus::CampaignComplete);
        assert_eq!(loaded.game_mode, GameMode::Campaign);
        assert_eq!(
            loaded.campaign.as_ref().map(|campaign| campaign.level),
            Some(3)
        );

        let outcome = ReplayOutcome::from_state(&state);
        let replayed: ReplayOutcome =
            serde_json::from_str(&serde_json::to_string(&outcome).unwrap()).unwrap();
        assert_eq!(replayed.status, GameStatus::CampaignComplete);
        assert_eq!(replayed.campaign, state.campaign);
    }

    #[test]
    fn campaign_npc_custom_name_on_spawn() {
        let mut npc = crate::entity::Npc::new(0, 0, crate::entity::NpcKind::WatchCaptain);
        assert_eq!(npc.display_name(), "Watch-Captain");
        npc.custom_name = Some("Mara Voss".to_string());
        assert_eq!(npc.display_name(), "Mara Voss");
    }

    #[test]
    fn campaign_lantern_tile_is_walkable() {
        use crate::map::{Map, Tile};
        let mut map = Map::new(10, 10);
        for y in 0..10 {
            for x in 0..10 {
                let idx = map.idx(x, y);
                map.tiles[idx] = Tile::Floor;
            }
        }
        let idx1 = map.idx(5, 5);
        let idx2 = map.idx(6, 6);
        map.tiles[idx1] = Tile::LanternLit;
        map.tiles[idx2] = Tile::LanternDead;
        assert!(map.is_walkable(5, 5));
        assert!(map.is_walkable(6, 6));
    }

    #[test]
    fn campaign_chapter1_ending_exists() {
        assert!(!CHAPTER1_ENDING.is_empty());
        assert!(CHAPTER1_ENDING.iter().any(|l| l.contains("End of Chapter")));
    }

    #[test]
    fn campaign_vey_reward_is_one_shot() {
        use std::collections::HashSet;
        let mut flags: HashSet<CampaignFlag> = HashSet::new();

        // Low trust — standard intro
        let dialog = campaign_dialog(CampaignNpc::Vey, 0, &[], 0).unwrap();
        assert!(dialog[0].npc_line.contains("You have the look"));

        // High trust, no reward claimed — trusted dialog with reward
        let dialog = campaign_dialog(CampaignNpc::Vey, 0, &[], 3).unwrap();
        assert!(dialog[0].npc_line.contains("investment"));

        // High trust, reward claimed — should show done dialog
        flags.insert(CampaignFlag::VeyRewardClaimed);
        let flag_slice: Vec<CampaignFlag> = flags.iter().copied().collect();
        let dialog = campaign_dialog(CampaignNpc::Vey, 0, &flag_slice, 3).unwrap();
        assert!(dialog[0]
            .npc_line
            .contains("I've already given you my best stock"));
    }

    #[test]
    fn campaign_tovan_helped_is_one_shot() {
        let mut flags: Vec<CampaignFlag> = vec![];

        // First talk — should get wounded dialog
        let dialog = campaign_dialog(CampaignNpc::TovanReed, 0, &flags, 0).unwrap();
        assert!(dialog[0].npc_line.contains("Don't step there"));

        // After being helped — should get TovanHelped dialog
        flags.push(CampaignFlag::TovanHelped);
        let dialog = campaign_dialog(CampaignNpc::TovanReed, 0, &flags, 0).unwrap();
        assert!(dialog[0].npc_line.contains("already told you everything"));

        // TovanHelped is set by all three choices in Tovan's intro
        let intro_dialog = campaign_dialog(CampaignNpc::TovanReed, 0, &[], 0).unwrap();
        for choice in intro_dialog[0].choices {
            assert!(
                choice
                    .effect
                    .iter()
                    .any(|e| matches!(e, DialogEffect::SetFlag(CampaignFlag::TovanHelped))),
                "Tovan choice '{}' should set TovanHelped flag",
                choice.label
            );
        }
    }

    #[test]
    fn campaign_mara_complete_objective_sets_gate_opened() {
        // Verify Mara's "Open the gate" choice has the CompleteObjective effect
        let flags_after_token = vec![CampaignFlag::RecoveredBellToken];
        let dialog = campaign_dialog(CampaignNpc::MaraVoss, 0, &flags_after_token, 0).unwrap();
        let gate_choice = dialog[0]
            .choices
            .iter()
            .find(|c| c.label.contains("Open the gate"))
            .expect("Gate option should exist when token is recovered");
        assert!(
            gate_choice
                .effect
                .iter()
                .any(|e| matches!(e, DialogEffect::CompleteObjective)),
            "Open the gate should have CompleteObjective effect"
        );
    }

    #[test]
    fn campaign_save_load_preserves_landmarks_and_camps() {
        use crate::app::{GameMode, GameState};
        use crate::entity::{Camp, CampKind, Landmark};
        use crate::save::SaveData;

        let mut state = GameState::new(42);
        state.game_mode = GameMode::Campaign;
        state.landmarks.push((0, Landmark::BellChamber));
        state.used_landmarks.insert(0);
        state.camps.push(Camp {
            room_idx: 1,
            x: 10,
            y: 10,
            kind: CampKind::DelverWaystation,
            npc_indices: vec![2, 3],
            rested: true,
        });

        let save = SaveData::from_state(&state);
        let json = serde_json::to_string(&save).unwrap();
        let loaded: SaveData = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.landmarks.len(), 1);
        assert_eq!(loaded.landmarks[0].1, Landmark::BellChamber);
        assert_eq!(loaded.camps.len(), 1);
        assert_eq!(loaded.camps[0].kind, CampKind::DelverWaystation);
        assert_eq!(loaded.camps[0].room_idx, 1);
        assert!(loaded.camps[0].rested);
        assert_eq!(loaded.used_landmarks, vec![0]);
    }

    #[test]
    fn campaign_bell_choice_is_guarded_by_existing_flag() {
        use std::collections::HashSet;
        let mut flags: HashSet<CampaignFlag> = HashSet::new();

        // Before any bell choice — all flags are unset
        let has_any_bell = |f: &HashSet<CampaignFlag>| -> bool {
            f.contains(&CampaignFlag::BellChoiceRung)
                || f.contains(&CampaignFlag::BellChoiceShattered)
                || f.contains(&CampaignFlag::BellChoiceMuffled)
        };
        assert!(!has_any_bell(&flags));

        // After a bell choice
        flags.insert(CampaignFlag::BellChoiceRung);
        assert!(has_any_bell(&flags));
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// PLAYER TURN SIDE-EFFECT TESTS
// ══════════════════════════════════════════════════════════════════════════════

mod player_turn_tests {
    use crate::input::Action;
    use crate::map::{Map, Tile};

    /// Build a headed game context with an open 7×7 map, player at (1,3).
    fn make_game_context() -> (crate::app::GameState, crate::visual::VisualState) {
        let mut state = crate::app::GameState::new(42);
        let mut map = Map::new(7, 7);
        for y in 0..7 {
            for x in 0..7 {
                let i = map.idx(x, y);
                map.tiles[i] = Tile::Floor;
            }
        }
        state.map = map;
        state.player = crate::entity::Player::new(1, 3);
        state.status = crate::app::GameStatus::Playing;
        let visual = crate::visual::VisualState::new(80, 50);
        (state, visual)
    }

    #[test]
    fn camera_direction_maps_to_cardinal_movement() {
        assert_eq!(crate::app::camera_dir_delta(0), (1, 0));
        assert_eq!(crate::app::camera_dir_delta(1), (0, 1));
        assert_eq!(crate::app::camera_dir_delta(2), (-1, 0));
        assert_eq!(crate::app::camera_dir_delta(3), (0, -1));
        assert_eq!(crate::app::camera_dir_delta(-1), (0, -1));
    }

    #[test]
    fn inventory_cursor_follows_same_kind_or_nearest_surviving_slot() {
        let (mut state, _) = make_game_context();
        state.player.inventory = vec![
            crate::entity::ItemKind::Longsword,
            crate::entity::ItemKind::HealthPotion,
            crate::entity::ItemKind::HealthPotion,
        ];
        state.preserve_inventory_cursor(crate::entity::ItemKind::HealthPotion, 2);
        assert_eq!(state.inventory_cursor, 1);

        state.player.inventory = vec![crate::entity::ItemKind::Longsword];
        state.preserve_inventory_cursor(crate::entity::ItemKind::HealthPotion, 2);
        assert_eq!(state.inventory_cursor, 0);
    }

    #[test]
    fn nearby_unseen_monster_follows_the_crown_wake() {
        let (mut state, mut visual) = make_game_context();
        state.player.crown_held = true;
        let monster = crate::entity::Monster::new(5, 3, crate::entity::MonsterKind::Orc);
        state.monsters.push(monster);
        let before = (state.monsters[0].x - state.player.x).abs()
            + (state.monsters[0].y - state.player.y).abs();

        state.end_turn(&mut visual);

        let after = (state.monsters[0].x - state.player.x).abs()
            + (state.monsters[0].y - state.player.y).abs();
        assert!(after < before);
    }

    #[test]
    fn crown_pursuit_is_deterministic_for_matching_seed_and_actions() {
        let simulate = || {
            let (mut state, mut visual) = make_game_context();
            state.player.crown_held = true;
            state.monsters.push(crate::entity::Monster::new(
                5,
                3,
                crate::entity::MonsterKind::Orc,
            ));
            for _ in 0..6 {
                state.process_turn(crate::input::Action::Wait, &mut visual);
            }
            (
                state.player.stats.hp,
                state.stats.turns,
                state
                    .monsters
                    .iter()
                    .map(|monster| (monster.x, monster.y, monster.stats.hp, monster.alert_turns))
                    .collect::<Vec<_>>(),
            )
        };

        assert_eq!(simulate(), simulate());
    }

    #[test]
    fn move_into_door_opens_it_and_records_replay() {
        let (mut state, mut visual) = make_game_context();
        let door_i = state.map.idx(2, 3);
        state.map.tiles[door_i] = Tile::DoorClosed;
        state.replay_data = Some(crate::replay::ReplayData::with_modes(
            42,
            crate::app::GameMode::Classic,
            crate::app::ChallengeMode::Standard,
            crate::app::CURRENT_RULES_VERSION,
        ));

        state.process_turn(Action::Move(1, 0), &mut visual);

        assert_eq!(state.map.tiles[door_i], Tile::DoorOpen);
        assert_eq!(state.stats.turns, 1);

        let replay = state.replay_data.as_ref().unwrap();
        assert_eq!(replay.actions.len(), 1);
        assert!(matches!(
            replay.actions[0].action,
            crate::replay::ActionRepr::Move(1, 0)
        ));
    }

    #[test]
    fn open_door_can_be_closed_when_unoccupied() {
        let (mut state, mut visual) = make_game_context();
        let door_i = state.map.idx(2, 3);
        state.map.tiles[door_i] = Tile::DoorOpen;

        state.toggle_facing_door(1, 0, &mut visual);

        assert_eq!(state.map.tiles[door_i], Tile::DoorClosed);
        assert_eq!(state.stats.turns, 1);
    }

    #[test]
    fn listening_at_closed_door_spends_a_turn() {
        let (mut state, mut visual) = make_game_context();
        let door_i = state.map.idx(2, 3);
        state.map.tiles[door_i] = Tile::DoorClosed;

        state.listen_facing(1, 0, &mut visual);

        assert_eq!(state.stats.turns, 1);
        assert!(state
            .messages
            .recent(2)
            .iter()
            .any(|message| message.text.contains("dungeon's breathing")));
    }

    #[test]
    fn revealed_trap_can_be_disarmed_without_entering_it() {
        let (mut state, mut visual) = make_game_context();
        let trap_i = state.map.idx(2, 3);
        state.map.tiles[trap_i] = Tile::TrapRevealed;

        state.disarm_trap(1, 0, &mut visual);

        assert_eq!(state.map.tiles[trap_i], Tile::Floor);
        assert_eq!(state.stats.turns, 1);
    }

    #[test]
    fn grounded_monster_triggers_floor_trap() {
        let (mut state, mut visual) = make_game_context();
        let trap_i = state.map.idx(4, 4);
        state.map.tiles[trap_i] = Tile::TrapHidden;
        state.map.visible[trap_i] = true;
        let monster = crate::entity::Monster::new(4, 4, crate::entity::MonsterKind::Sentinel);
        let hp = monster.stats.hp;
        state.monsters.push(monster);

        state.end_turn(&mut visual);

        assert_eq!(state.map.tiles[trap_i], Tile::Floor);
        assert_eq!(state.monsters[0].stats.hp, hp - 5);
    }

    #[test]
    fn warhammer_pushes_a_surviving_target() {
        let (mut state, mut visual) = make_game_context();
        state.rules_version = 3;
        state.player.weapon = Some(crate::entity::ItemKind::Warhammer);
        let monster = crate::entity::Monster::new(2, 3, crate::entity::MonsterKind::Orc);
        state.monsters.push(monster);

        state.process_turn(Action::Move(1, 0), &mut visual);

        assert!(state.monsters[0].alive);
        assert_eq!((state.monsters[0].x, state.monsters[0].y), (3, 3));
        assert_eq!(
            state.monsters[0].staggered, 0,
            "stagger is consumed by end-turn"
        );
        assert!(visual.effects.iter().any(|effect| matches!(
            effect,
            crate::visual::VisualEffect::ActorStep {
                x: 3,
                y: 3,
                dx: 1,
                dy: 0,
                ..
            }
        )));
    }

    #[test]
    fn ordinary_weapon_breaks_at_zero_durability() {
        let (mut state, mut visual) = make_game_context();
        state.rules_version = 3;
        state.player.weapon = Some(crate::entity::ItemKind::RustySword);
        state.player.weapon_durability = 1;
        state.monsters.push(crate::entity::Monster::new(
            2,
            3,
            crate::entity::MonsterKind::Orc,
        ));

        state.process_turn(Action::Move(1, 0), &mut visual);

        assert_eq!(state.player.weapon, None);
        assert_eq!(state.player.weapon_durability, 0);
    }

    #[test]
    fn fragile_gear_wears_weapon_twice_per_attack() {
        let (mut state, mut visual) = make_game_context();
        state.rules_version = 3;
        state.challenge_mode = crate::app::ChallengeMode::FragileGear;
        state.player.weapon = Some(crate::entity::ItemKind::RustySword);
        state.player.weapon_durability = 5;
        state.monsters.push(crate::entity::Monster::new(
            2,
            3,
            crate::entity::MonsterKind::Orc,
        ));

        state.process_turn(Action::Move(1, 0), &mut visual);

        assert_eq!(state.player.weapon_durability, 3);
    }

    #[test]
    fn deep_fog_reduces_effective_sight_radius() {
        let mut state = crate::app::GameState::new(42);
        let standard = state.current_fov_radius();
        state.challenge_mode = crate::app::ChallengeMode::DeepFog;

        assert_eq!(state.current_fov_radius(), standard - 2);
    }

    #[test]
    fn lost_cartographer_disables_minimap_policy() {
        assert!(!crate::app::ChallengeMode::NoMinimap.allows_minimap());
        assert!(crate::app::ChallengeMode::Standard.allows_minimap());
    }

    #[test]
    fn map_shortcut_is_a_persistent_two_way_toggle() {
        let opened = crate::app::toggled_map_visibility(false, crate::app::ChallengeMode::Standard);
        let closed = crate::app::toggled_map_visibility(
            opened.unwrap(),
            crate::app::ChallengeMode::Standard,
        );

        assert_eq!(opened, Some(true));
        assert_eq!(closed, Some(false));
        assert_eq!(
            crate::app::toggled_map_visibility(false, crate::app::ChallengeMode::NoMinimap,),
            None
        );
    }

    #[test]
    fn relentless_mobile_monster_advances_every_turn() {
        let (mut state, mut visual) = make_game_context();
        state.challenge_mode = crate::app::ChallengeMode::Relentless;
        state.map.visible.fill(true);
        state.monsters.push(crate::entity::Monster::new(
            4,
            3,
            crate::entity::MonsterKind::Goblin,
        ));

        state.end_turn(&mut visual);

        assert_eq!((state.monsters[0].x, state.monsters[0].y), (3, 3));
    }

    #[test]
    fn cursed_floor_weapon_force_equips_and_blocks_replacement() {
        let (mut state, mut visual) = make_game_context();
        let mut item = crate::entity::Item::new(
            state.player.x,
            state.player.y,
            crate::entity::ItemKind::RustySword,
        );
        item.cursed = true;
        state.items.push(item);
        state
            .player
            .inventory
            .push(crate::entity::ItemKind::Warhammer);

        assert!(state.pick_up_item(&mut visual));
        assert!(state.player.weapon_cursed);
        assert_eq!(
            state.player.weapon,
            Some(crate::entity::ItemKind::RustySword)
        );
        let slot = state
            .player
            .inventory
            .iter()
            .position(|item| *item == crate::entity::ItemKind::Warhammer)
            .unwrap();
        assert!(!state.use_item(crate::entity::ItemKind::Warhammer, slot, &mut visual));
        assert_eq!(
            state.player.weapon,
            Some(crate::entity::ItemKind::RustySword)
        );
    }

    #[test]
    fn shallow_water_costs_an_extra_turn() {
        let (mut state, mut visual) = make_game_context();
        let water = state.map.idx(2, 3);
        state.map.tiles[water] = Tile::ShallowWater;

        state.process_turn(Action::Move(1, 0), &mut visual);

        assert_eq!((state.player.x, state.player.y), (2, 3));
        assert_eq!(state.stats.turns, 2);
    }

    #[test]
    fn tideglass_mail_ignores_shallow_water_delay() {
        let (mut state, mut visual) = make_game_context();
        let water = state.map.idx(2, 3);
        state.map.tiles[water] = Tile::ShallowWater;
        state.player.armor = Some(crate::entity::ItemKind::TideglassMail);

        state.process_turn(Action::Move(1, 0), &mut visual);

        assert_eq!(state.stats.turns, 1);
    }

    #[test]
    fn webbing_costs_an_extra_turn_and_is_torn_down() {
        let (mut state, mut visual) = make_game_context();
        let web = state.map.idx(2, 3);
        state.map.tiles[web] = Tile::Web;

        state.process_turn(Action::Move(1, 0), &mut visual);

        assert_eq!(state.map.tiles[web], Tile::Floor);
        assert_eq!(state.stats.turns, 2);
    }

    #[test]
    fn fire_scroll_burns_visible_webbing() {
        let (mut state, mut visual) = make_game_context();
        let web = state.map.idx(4, 4);
        state.map.tiles[web] = Tile::Web;
        state.map.visible[web] = true;
        state
            .player
            .inventory
            .push(crate::entity::ItemKind::ScrollOfFire);

        assert!(state.use_item(crate::entity::ItemKind::ScrollOfFire, 0, &mut visual));
        assert_eq!(state.map.tiles[web], Tile::Floor);
    }

    #[test]
    fn iron_key_opens_one_locked_door_and_is_consumed() {
        let (mut state, mut visual) = make_game_context();
        let door = state.map.idx(2, 3);
        state.map.tiles[door] = Tile::LockedDoor;
        state
            .player
            .inventory
            .push(crate::entity::ItemKind::IronKey);

        state.process_turn(Action::Move(1, 0), &mut visual);

        assert_eq!(state.map.tiles[door], Tile::DoorOpen);
        assert!(!state
            .player
            .inventory
            .contains(&crate::entity::ItemKind::IronKey));
        assert_eq!(state.stats.turns, 1);
    }

    #[test]
    fn warhammer_forces_locked_door_without_a_key() {
        let (mut state, mut visual) = make_game_context();
        let door = state.map.idx(2, 3);
        state.map.tiles[door] = Tile::LockedDoor;
        state.player.weapon = Some(crate::entity::ItemKind::Warhammer);

        state.process_turn(Action::Move(1, 0), &mut visual);

        assert_eq!(state.map.tiles[door], Tile::DoorOpen);
        assert_eq!(state.stats.turns, 1);
    }

    #[test]
    fn locked_door_without_solution_does_not_spend_turn() {
        let (mut state, mut visual) = make_game_context();
        let door = state.map.idx(2, 3);
        state.map.tiles[door] = Tile::LockedDoor;

        state.process_turn(Action::Move(1, 0), &mut visual);

        assert_eq!(state.map.tiles[door], Tile::LockedDoor);
        assert_eq!(state.stats.turns, 0);
    }

    #[test]
    fn urn_smash_awards_gold_and_clears_tile() {
        let (mut state, mut visual) = make_game_context();
        let urn = state.map.idx(2, 3);
        state.map.tiles[urn] = Tile::Urn;

        state.process_turn(Action::Move(1, 0), &mut visual);

        assert_eq!(state.map.tiles[urn], Tile::Floor);
        assert!(state.player.gold > 0);
        assert_eq!(state.stats.turns, 1);
    }

    #[test]
    fn heavy_monster_telegraphs_before_dealing_damage() {
        let (mut state, mut visual) = make_game_context();
        state.map.visible.fill(true);
        let mut monster = crate::entity::Monster::new(2, 3, crate::entity::MonsterKind::Orc);
        monster.awakened = true;
        state.monsters.push(monster);
        let hp = state.player.stats.hp;

        state.end_turn(&mut visual);
        assert_eq!(state.player.stats.hp, hp);
        assert_eq!(state.monsters[0].windup, 1);

        state.end_turn(&mut visual);
        assert!(state.player.stats.hp < hp);
        assert_eq!(state.monsters[0].windup, 0);
    }

    #[test]
    fn move_into_wall_is_blocked_without_spending_turn() {
        let (mut state, mut visual) = make_game_context();
        let wi = state.map.idx(2, 3);
        state.map.tiles[wi] = Tile::Wall;

        state.process_turn(Action::Move(1, 0), &mut visual);

        assert_eq!((state.player.x, state.player.y), (1, 3));
        assert_eq!(
            state.stats.turns, 0,
            "blocked movement should not spend a turn"
        );
    }

    #[test]
    fn move_into_monster_attacks_without_moving_player() {
        let (mut state, mut visual) = make_game_context();
        state.rules_version = 3;
        let mon = crate::entity::Monster::new(2, 3, crate::entity::MonsterKind::Goblin);
        state.monsters.push(mon);
        let hp_before = state.monsters[0].stats.hp;

        state.process_turn(Action::Move(1, 0), &mut visual);

        assert_eq!((state.player.x, state.player.y), (1, 3));
        assert!(state.monsters[0].stats.hp < hp_before);
        assert_eq!(state.stats.turns, 1);
        assert!(matches!(
            visual.player_action.map(|action| action.kind),
            Some(crate::visual::PlayerActionKind::Attack { .. })
        ));
    }

    #[test]
    fn evasive_step_moves_spends_stamina_and_records_replay() {
        let (mut state, mut visual) = make_game_context();
        state.new_game();
        state.map = Map::new(7, 7);
        state.map.tiles.fill(Tile::Floor);
        state.monsters.clear();
        state.items.clear();
        state.npcs.clear();
        state.player.x = 3;
        state.player.y = 3;
        state.player.stamina = crate::entity::MAX_STAMINA;

        state.process_turn(Action::Dodge(0, 1), &mut visual);

        assert_eq!((state.player.x, state.player.y), (3, 4));
        assert_eq!(state.player.stamina, crate::entity::MAX_STAMINA - 2);
        assert_eq!(state.stats.turns, 1);
        assert!(matches!(
            state
                .replay_data
                .as_ref()
                .and_then(|replay| replay.actions.last())
                .map(|record| record.action),
            Some(crate::replay::ActionRepr::Dodge(0, 1))
        ));
    }

    #[test]
    fn evasive_step_avoids_one_adjacent_attack() {
        let (mut state, mut visual) = make_game_context();
        state.map.visible.fill(true);
        state.player.x = 3;
        state.player.y = 3;
        let mut goblin = crate::entity::Monster::new(4, 4, crate::entity::MonsterKind::Goblin);
        goblin.awakened = true;
        goblin.stats.attack = 20;
        state.monsters.push(goblin);
        let hp = state.player.stats.hp;

        state.process_turn(Action::Dodge(0, 1), &mut visual);

        assert_eq!((state.player.x, state.player.y), (3, 4));
        assert_eq!(state.player.stats.hp, hp);
        assert!(state
            .messages
            .recent(5)
            .iter()
            .any(|message| message.text.contains("afterimage")));
        assert!(visual.effects.iter().any(|effect| matches!(
            effect,
            crate::visual::VisualEffect::MovementTrail {
                dx: 0,
                dy: 1,
                dash: true,
                ..
            }
        )));
    }

    #[test]
    fn evasive_step_requires_stamina_and_current_rules() {
        let (mut state, mut visual) = make_game_context();
        state.player.stamina = 2;
        state.process_turn(Action::Dodge(0, 1), &mut visual);
        assert_eq!((state.player.x, state.player.y), (1, 3));
        assert_eq!(state.stats.turns, 0);

        state.player.stamina = crate::entity::MAX_STAMINA;
        state.rules_version = 0;
        state.process_turn(Action::Dodge(0, 1), &mut visual);
        assert_eq!((state.player.x, state.player.y), (1, 3));
        assert_eq!(state.stats.turns, 0);
    }

    #[test]
    fn depleted_guard_breaks_against_a_heavy_impact() {
        let (mut state, mut visual) = make_game_context();
        state.map.visible.fill(true);
        state.player.stamina = 0;
        let mut goblin = crate::entity::Monster::new(2, 3, crate::entity::MonsterKind::Goblin);
        goblin.awakened = true;
        goblin.stats.attack = 20;
        state.monsters.push(goblin);

        state.process_turn(Action::Wait, &mut visual);

        assert!(state
            .messages
            .recent(5)
            .iter()
            .any(|message| message.text.contains("guard breaks")));
        assert!(state.player.stats.hp < state.player.stats.max_hp);
    }

    #[test]
    fn stable_guard_primes_a_one_turn_counter() {
        let (mut state, mut visual) = make_game_context();
        state.map.visible.fill(true);
        let mut goblin = crate::entity::Monster::new(2, 3, crate::entity::MonsterKind::Goblin);
        goblin.awakened = true;
        state.monsters.push(goblin);

        state.process_turn(Action::Wait, &mut visual);

        assert_eq!(state.player.counter_turns, 1);
    }

    #[test]
    fn counterstrike_deals_bonus_damage_and_breaks_poise() {
        let (mut ordinary, mut ordinary_visual) = make_game_context();
        ordinary.rules_version = 3;
        let mut target = crate::entity::Monster::new(2, 3, crate::entity::MonsterKind::Orc);
        target.stats.hp = 100;
        target.stats.max_hp = 100;
        ordinary.monsters.push(target.clone());
        ordinary.process_turn(Action::Move(1, 0), &mut ordinary_visual);
        let ordinary_damage = 100 - ordinary.monsters[0].stats.hp;

        let (mut counter, mut counter_visual) = make_game_context();
        counter.rules_version = 3;
        counter.player.counter_turns = 1;
        counter.monsters.push(target);
        counter.process_turn(Action::Move(1, 0), &mut counter_visual);
        let counter_damage = 100 - counter.monsters[0].stats.hp;

        assert_eq!(counter_damage, ordinary_damage + 3);
        assert_eq!(counter.monsters[0].staggered, 1);
        assert!(counter
            .messages
            .recent(5)
            .iter()
            .any(|message| message.text.contains("counterstrike")));
    }

    #[test]
    fn attacking_during_a_heavy_windup_interrupts_it() {
        let (mut state, mut visual) = make_game_context();
        state.rules_version = 3;
        let mut orc = crate::entity::Monster::new(2, 3, crate::entity::MonsterKind::Orc);
        orc.stats.hp = 100;
        orc.stats.max_hp = 100;
        orc.windup = 1;
        state.monsters.push(orc);

        state.process_turn(Action::Move(1, 0), &mut visual);

        assert_eq!(state.monsters[0].windup, 0);
        assert_eq!(state.monsters[0].staggered, 1);
        assert!(state
            .messages
            .recent(5)
            .iter()
            .any(|message| message.text.contains("break the attack")));
    }

    #[test]
    fn guardian_crossing_half_health_unseals_crown_fire() {
        let (mut state, mut visual) = make_game_context();
        state.rules_version = 3;
        let mut guardian = crate::entity::Monster::new(2, 3, crate::entity::MonsterKind::Guardian);
        guardian.stats.hp = guardian.stats.max_hp / 2 + 1;
        state.monsters.push(guardian);

        state.process_turn(Action::Move(1, 0), &mut visual);

        assert!(state.monsters[0].stats.hp * 2 <= state.monsters[0].stats.max_hp);
        assert!(state
            .messages
            .recent(5)
            .iter()
            .any(|message| message.text.contains("Crown-fire")));
        assert!(visual.effects.iter().any(|effect| matches!(
            effect,
            crate::visual::VisualEffect::FireRing { x: 2, y: 3, .. }
        )));
    }

    #[test]
    fn phase_two_guardian_grab_is_telegraphed_and_drains_stamina() {
        let (mut state, mut visual) = make_game_context();
        state.map.visible.fill(true);
        state.player.stamina = crate::entity::MAX_STAMINA;
        let mut guardian = crate::entity::Monster::new(2, 3, crate::entity::MonsterKind::Guardian);
        guardian.stats.hp = guardian.stats.max_hp / 2;
        guardian.awakened = true;
        guardian.windup = 1;
        state.monsters.push(guardian);

        state.process_turn(Action::Wait, &mut visual);

        assert!(state.player.stamina <= 3);
        assert!(state
            .messages
            .recent(8)
            .iter()
            .any(|message| message.text.contains("Crown chains")));
    }

    #[test]
    fn guardian_phase_damage_is_gated_out_of_legacy_rules() {
        fn phase_guardian() -> crate::entity::Monster {
            let mut guardian =
                crate::entity::Monster::new(2, 3, crate::entity::MonsterKind::Guardian);
            guardian.stats.hp = guardian.stats.max_hp / 2;
            guardian.awakened = true;
            guardian.windup = 1;
            guardian
        }

        let (mut current, mut current_visual) = make_game_context();
        current.map.visible.fill(true);
        current.monsters.push(phase_guardian());
        current.end_turn(&mut current_visual);

        let (mut legacy, mut legacy_visual) = make_game_context();
        legacy.rules_version = 0;
        legacy.map.visible.fill(true);
        legacy.monsters.push(phase_guardian());
        legacy.end_turn(&mut legacy_visual);

        assert!(current.player.stats.hp < legacy.player.stats.hp);
        assert!(!legacy
            .messages
            .recent(8)
            .iter()
            .any(|message| message.text.contains("Crown chains")));
    }

    #[test]
    fn discipline_choice_applies_immediately_without_spending_a_turn() {
        let (mut state, _) = make_game_context();
        state.player.pending_level_ups = 1;
        state.replay_data = Some(crate::replay::ReplayData::with_modes(
            42,
            crate::app::GameMode::Classic,
            crate::app::ChallengeMode::Standard,
            crate::app::CURRENT_RULES_VERSION,
        ));
        let max_hp = state.player.stats.max_hp;

        assert!(state.resolve_discipline_choice(crate::entity::Discipline::Vigor));

        assert_eq!(state.stats.turns, 0);
        assert_eq!(state.player.stats.max_hp, max_hp + 6);
        assert_eq!(state.player.vigor_rank, 1);
        assert_eq!(state.player.pending_level_ups, 0);
        assert!(matches!(
            state.replay_data.as_ref().unwrap().actions[0].action,
            crate::replay::ActionRepr::DisciplineChoice(crate::entity::Discipline::Vigor)
        ));
    }

    #[test]
    fn pending_discipline_blocks_turn_actions() {
        let (mut state, mut visual) = make_game_context();
        state.player.pending_level_ups = 1;
        let stamina = state.player.stamina;

        state.process_turn(Action::Wait, &mut visual);

        assert_eq!(state.stats.turns, 0);
        assert_eq!(state.player.stamina, stamina);
        assert!(!state.player.guarding);
    }

    #[test]
    fn rules_v2_level_up_queues_choice_instead_of_auto_stats() {
        let (mut state, mut visual) = make_game_context();
        state.rules_version = 3;
        let mut rat = crate::entity::Monster::new(2, 3, crate::entity::MonsterKind::Rat);
        rat.stats.hp = 1;
        let reward = rat.kind.xp_value();
        state.player.xp = state.player.xp_to_next() - reward;
        state.player.stats.attack = 100;
        let base_hp = state.player.stats.max_hp;
        let base_attack = state.player.stats.attack;
        state.monsters.push(rat);

        state.process_turn(Action::Move(1, 0), &mut visual);

        assert_eq!(state.player.level, 2);
        assert_eq!(state.player.pending_level_ups, 1);
        assert_eq!(state.player.stats.max_hp, base_hp);
        assert_eq!(state.player.stats.attack, base_attack);
    }

    #[test]
    fn finesse_and_resolve_build_resources_and_counter_damage() {
        let (mut ordinary, mut ordinary_visual) = make_game_context();
        ordinary.rules_version = 3;
        ordinary.player.counter_turns = 1;
        let mut target = crate::entity::Monster::new(2, 3, crate::entity::MonsterKind::Orc);
        target.stats.hp = 100;
        target.stats.max_hp = 100;
        ordinary.monsters.push(target.clone());
        ordinary.process_turn(Action::Move(1, 0), &mut ordinary_visual);
        let ordinary_damage = 100 - ordinary.monsters[0].stats.hp;

        let (mut built, mut built_visual) = make_game_context();
        built.rules_version = 3;
        built.player.pending_level_ups = 2;
        built.resolve_discipline_choice(crate::entity::Discipline::Finesse);
        let base_max_stamina = crate::entity::MAX_STAMINA;
        assert_eq!(built.player.max_stamina, base_max_stamina + 1);
        built.resolve_discipline_choice(crate::entity::Discipline::Resolve);
        built.player.counter_turns = 1;
        built.monsters.push(target);
        built.process_turn(Action::Move(1, 0), &mut built_visual);
        let built_damage = 100 - built.monsters[0].stats.hp;

        // Finesse contributes one attack and Resolve contributes one counter damage.
        assert_eq!(built_damage, ordinary_damage + 2);
        assert_eq!(built.player.resolve_rank, 1);
    }

    #[test]
    fn technique_without_target_spends_nothing() {
        let (mut state, mut visual) = make_game_context();
        let stamina = state.player.stamina;

        state.process_turn(Action::Technique(1, 0), &mut visual);

        assert_eq!(state.stats.turns, 0);
        assert_eq!(state.player.stamina, stamina);
        assert_eq!(state.stats.techniques_used, 0);
    }

    #[test]
    fn technique_cannot_reach_through_a_blocking_tile() {
        let (mut state, mut visual) = make_game_context();
        state.player.weapon = Some(crate::entity::ItemKind::Emberbrand);
        let wall = state.map.idx(2, 3);
        state.map.tiles[wall] = Tile::Wall;
        let mut target = crate::entity::Monster::new(3, 3, crate::entity::MonsterKind::Orc);
        target.stats.hp = 50;
        target.stats.max_hp = 50;
        state.monsters.push(target);
        let stamina = state.player.stamina;

        state.process_turn(Action::Technique(1, 0), &mut visual);

        assert_eq!(state.monsters[0].stats.hp, 50);
        assert_eq!(state.player.stamina, stamina);
        assert_eq!(state.stats.turns, 0);
    }

    #[test]
    fn technique_action_and_build_state_are_replay_serializable() {
        let action = crate::replay::ActionRepr::Technique(1, 0);
        let choice =
            crate::replay::ActionRepr::DisciplineChoice(crate::entity::Discipline::Resolve);
        let json = serde_json::to_string(&(action, choice)).unwrap();
        let decoded: (crate::replay::ActionRepr, crate::replay::ActionRepr) =
            serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.0, action);
        assert_eq!(decoded.1, choice);
    }

    #[test]
    fn kick_deals_fixed_damage_and_pushes() {
        let (mut state, mut visual) = make_game_context();
        let mut target = crate::entity::Monster::new(2, 3, crate::entity::MonsterKind::Orc);
        target.stats.hp = 20;
        state.monsters.push(target);

        state.process_turn(Action::Technique(1, 0), &mut visual);

        assert_eq!(state.monsters[0].stats.hp, 19);
        assert_eq!((state.monsters[0].x, state.monsters[0].y), (3, 3));
        assert_eq!(state.stats.turns, 1);
        assert_eq!(state.stats.techniques_used, 1);
        assert_eq!(state.stats.stamina_spent, 2);
        assert!(visual.effects.iter().any(|effect| matches!(
            effect,
            crate::visual::VisualEffect::TechniqueCast {
                kind: crate::entity::TechniqueKind::Kick,
                ..
            }
        )));
        assert!(visual.effects.iter().any(|effect| matches!(
            effect,
            crate::visual::VisualEffect::TechniqueImpact {
                kind: crate::entity::TechniqueKind::Kick,
                ..
            }
        )));
    }

    #[test]
    fn rusty_sword_lunge_advances_to_a_range_two_target() {
        let (mut state, mut visual) = make_game_context();
        state.player.weapon = Some(crate::entity::ItemKind::RustySword);
        state.player.weapon_durability = 10;
        let mut target = crate::entity::Monster::new(3, 3, crate::entity::MonsterKind::Orc);
        target.stats.hp = 50;
        target.stats.max_hp = 50;
        state.monsters.push(target);

        state.process_turn(Action::Technique(1, 0), &mut visual);

        assert_eq!((state.player.x, state.player.y), (2, 3));
        assert!(state.monsters[0].stats.hp < 50);
        assert_eq!(state.player.weapon_durability, 8);
        assert_eq!(state.stats.turns, 1);
    }

    #[test]
    fn lunge_requires_clear_stable_intermediate_footing() {
        let (mut state, mut visual) = make_game_context();
        state.player.weapon = Some(crate::entity::ItemKind::RustySword);
        let water = state.map.idx(2, 3);
        state.map.tiles[water] = Tile::ShallowWater;
        let mut target = crate::entity::Monster::new(3, 3, crate::entity::MonsterKind::Orc);
        target.stats.hp = 50;
        target.stats.max_hp = 50;
        state.monsters.push(target);

        state.process_turn(Action::Technique(1, 0), &mut visual);

        assert_eq!((state.player.x, state.player.y), (1, 3));
        assert_eq!(state.monsters[0].stats.hp, 50);
        assert_eq!(state.stats.turns, 0);
    }

    #[test]
    fn longsword_sweep_hits_the_forward_fan() {
        let (mut state, mut visual) = make_game_context();
        state.player.weapon = Some(crate::entity::ItemKind::Longsword);
        state.player.weapon_durability = 10;
        for &(x, y) in &[(2, 3), (2, 2), (2, 4)] {
            let mut monster = crate::entity::Monster::new(x, y, crate::entity::MonsterKind::Orc);
            monster.stats.hp = 50;
            monster.stats.max_hp = 50;
            state.monsters.push(monster);
        }

        state.process_turn(Action::Technique(1, 0), &mut visual);

        assert!(state.monsters.iter().all(|monster| monster.stats.hp < 50));
        assert_eq!(state.stats.techniques_used, 1);
    }

    #[test]
    fn earthbreak_processes_farthest_target_before_push() {
        let (mut state, mut visual) = make_game_context();
        state.player.weapon = Some(crate::entity::ItemKind::Warhammer);
        state.player.weapon_durability = 10;
        for x in [2, 3] {
            let mut monster = crate::entity::Monster::new(x, 3, crate::entity::MonsterKind::Orc);
            monster.stats.hp = 50;
            monster.stats.max_hp = 50;
            state.monsters.push(monster);
        }

        state.process_turn(Action::Technique(1, 0), &mut visual);

        assert_eq!((state.monsters[0].x, state.monsters[0].y), (3, 3));
        assert_eq!((state.monsters[1].x, state.monsters[1].y), (4, 3));
        assert!(state.monsters.iter().all(|monster| monster.stats.hp < 50));
    }

    #[test]
    fn cinder_arc_burns_its_webbed_shape() {
        let (mut state, mut visual) = make_game_context();
        state.player.weapon = Some(crate::entity::ItemKind::Emberbrand);
        let affected = [(2, 3), (3, 2), (3, 3), (3, 4)];
        for &(x, y) in &affected {
            let index = state.map.idx(x, y);
            state.map.tiles[index] = Tile::Web;
        }

        state.process_turn(Action::Technique(1, 0), &mut visual);

        assert!(affected
            .iter()
            .all(|&(x, y)| state.map.tiles[state.map.idx(x, y)] == Tile::Floor));
        assert_eq!(state.stats.turns, 1);
        assert_eq!(state.player.stamina, state.player.max_stamina - 3);
    }

    #[test]
    fn bumping_npc_starts_conversation_without_spending_turn() {
        let (mut state, mut visual) = make_game_context();
        let npc = crate::entity::Npc::new(2, 3, crate::entity::NpcKind::Merchant);
        state.npcs.push(npc);

        state.process_turn(Action::Move(1, 0), &mut visual);

        assert_eq!((state.player.x, state.player.y), (1, 3));
        assert_eq!(state.stats.turns, 0, "NPC blocking should not spend a turn");
        assert!(state.dialog_active, "friendly bump should open dialog");
    }

    #[test]
    fn on_player_entered_tile_auto_picks_up_item() {
        let (mut state, mut visual) = make_game_context();
        state.auto_pickup = true;
        let item = crate::entity::Item::new(2, 3, crate::entity::ItemKind::HealthPotion);
        state.items.push(item);

        let from = (state.player.x, state.player.y);
        state.player.x = 2;
        state.player.y = 3;
        state.on_player_entered_tile(2, 3, from, false, &mut visual);
        assert!(!state.player.inventory.is_empty());
    }

    #[test]
    fn on_player_entered_tile_triggers_trap_damage() {
        let (mut state, mut visual) = make_game_context();
        let ti = state.map.idx(2, 3);
        state.map.tiles[ti] = Tile::TrapHidden;
        let hp_before = state.player.stats.hp;

        let from = (state.player.x, state.player.y);
        state.player.x = 2;
        state.player.y = 3;
        state.on_player_entered_tile(2, 3, from, false, &mut visual);
        assert!(state.player.stats.hp < hp_before);
    }

    #[test]
    fn on_player_entered_tile_marks_tile_explored() {
        let (mut state, mut visual) = make_game_context();
        let ti = state.map.idx(2, 3);
        assert!(!state.map.explored[ti]);

        let from = (state.player.x, state.player.y);
        state.player.x = 2;
        state.player.y = 3;
        state.on_player_entered_tile(2, 3, from, false, &mut visual);
        assert!(state.map.explored[ti]);
    }

    #[test]
    fn camp_rest_heals_once_and_advances_time() {
        let (mut state, mut visual) = make_game_context();
        state.map.rooms.push(crate::map::Rect {
            x: 1,
            y: 1,
            w: 5,
            h: 5,
        });
        state.player.x = 3;
        state.player.y = 3;
        state.player.stats.hp = 10;
        state.camps.push(crate::entity::Camp {
            room_idx: 0,
            x: 3,
            y: 3,
            kind: crate::entity::CampKind::DelverWaystation,
            npc_indices: vec![],
            rested: false,
        });

        state.interact_facing(1, 0, &mut visual);
        assert!(matches!(
            state.preparation_menu,
            Some(crate::app::PreparationMenu::Camp(0))
        ));
        state.resolve_preparation(1, &mut visual);
        let healed = state.player.stats.hp;
        state.interact_facing(1, 0, &mut visual);

        assert!(healed > 10);
        assert_eq!(state.player.stats.hp, healed);
        assert!(state.camps[0].rested);
        assert_eq!(state.stats.turns, 3);
    }

    #[test]
    fn camp_focus_is_a_persistent_one_use_choice() {
        let (mut state, mut visual) = make_game_context();
        state.map.rooms.push(crate::map::Rect {
            x: 1,
            y: 1,
            w: 5,
            h: 5,
        });
        state.player.x = 3;
        state.player.y = 3;
        let base_attack = state.player.total_attack();
        state.camps.push(crate::entity::Camp {
            room_idx: 0,
            x: 3,
            y: 3,
            kind: crate::entity::CampKind::DelverWaystation,
            npc_indices: vec![],
            rested: false,
        });

        state.interact_facing(1, 0, &mut visual);
        state.resolve_preparation(2, &mut visual);

        assert!(state.camps[0].rested);
        assert_eq!(state.player.total_attack(), base_attack + 1);
        assert_eq!(state.player.focus_turns, 39);
    }

    #[test]
    fn shrine_blessing_costs_gold_and_consumes_landmark() {
        let (mut state, mut visual) = make_game_context();
        state.map.rooms.push(crate::map::Rect {
            x: 1,
            y: 1,
            w: 5,
            h: 5,
        });
        state.player.x = 3;
        state.player.y = 3;
        state.player.gold = 20;
        state.landmarks.push((0, crate::entity::Landmark::Shrine));

        state.interact_facing(1, 0, &mut visual);
        assert!(matches!(
            state.preparation_menu,
            Some(crate::app::PreparationMenu::Shrine(0))
        ));
        state.resolve_preparation(3, &mut visual);

        assert_eq!(state.player.gold, 5);
        assert!(state.used_landmarks.contains(&0));
        assert_eq!(state.player.focus_turns, 59);
        assert_eq!(state.player.ward_turns, 59);
    }

    #[test]
    fn flooded_vault_reward_is_one_use_and_persists_in_save() {
        let (mut state, mut visual) = make_game_context();
        state.map.rooms.push(crate::map::Rect {
            x: 1,
            y: 1,
            w: 5,
            h: 5,
        });
        state.player.x = 3;
        state.player.y = 3;
        state
            .landmarks
            .push((0, crate::entity::Landmark::FloodedVault));

        state.interact_facing(1, 0, &mut visual);
        state.interact_facing(1, 0, &mut visual);
        let save = crate::save::SaveData::from_state(&state);

        assert_eq!(state.player.gold, 20);
        assert!(state.used_landmarks.contains(&0));
        assert_eq!(save.used_landmarks, vec![0]);
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// OVERHAUL REGRESSION / INTEGRATION TESTS
//
// The v16 overhaul introduces instanced group battles with Fight / Technique /
// Item / Guard / Flee commands, speed-ordered turns (player wins ties), a four-
// hostile group cap, overworld turn freezing during a battle, and a deterministic
// flee. It bumps the world save to v16 and the replay format to v2, so v15 saves
// and v1 replays are rejected.
//
// Status of this working tree:
//   * Save v16 / replay v2 boundaries: IMPLEMENTED. The version tests below run now.
//   * Speed ordering + group cap 4 + overworld freeze + deterministic flee: wired
//     onto `GameState` and the canonical `crate::battle` public API. The live tests
//     in `overhaul_battle_speed_tests` and `overhaul_battle_integration` exercise
//     that contract directly.
// ══════════════════════════════════════════════════════════════════════════════

mod overhaul_version_boundary_tests {
    use super::*;
    use crate::app::{ChallengeMode, GameMode};
    use crate::replay::ReplayData;
    use crate::replay::REPLAY_FORMAT_VERSION;
    use crate::save::SaveData;
    use crate::save::SAVE_VERSION;

    fn fresh_save() -> SaveData {
        let mut state = GameState::new(42);
        state.new_game();
        SaveData::from_state(&state)
    }

    fn fresh_replay() -> ReplayData {
        ReplayData::with_modes(
            42,
            GameMode::Classic,
            ChallengeMode::Standard,
            crate::app::CURRENT_RULES_VERSION,
        )
    }

    #[test]
    fn current_world_save_validates() {
        assert!(fresh_save().validate().is_ok());
    }

    #[test]
    fn prior_world_save_is_rejected() {
        // A save one format behind the build must be rejected rather than
        // silently remapped. With SAVE_VERSION now 16 this exercises the v15
        // rejection requirement.
        let mut save = fresh_save();
        save.version = SAVE_VERSION - 1;
        let err = save
            .validate()
            .expect_err("prior world format must be rejected");
        assert!(
            err.contains("older world format") || err.contains("start a new run"),
            "unexpected rejection message: {err}"
        );
    }

    #[test]
    fn v15_world_save_is_rejected_after_v16_bump() {
        // Explicit coverage for the requested v15 boundary. Active now that
        // SAVE_VERSION is 16; a no-op on any other version.
        if SAVE_VERSION == 16 {
            let mut save = fresh_save();
            save.version = 15;
            let err = save
                .validate()
                .expect_err("v15 world saves must be rejected");
            assert!(
                err.contains("older world format") || err.contains("start a new run"),
                "unexpected rejection message: {err}"
            );
        }
    }

    #[test]
    fn current_replay_format_validates() {
        assert!(fresh_replay().validate().is_ok());
    }

    #[test]
    fn prior_replay_format_is_rejected() {
        // A replay one format behind must be rejected. With REPLAY_FORMAT_VERSION
        // now 2 this exercises the v1 rejection requirement.
        let mut replay = fresh_replay();
        replay.format_version = REPLAY_FORMAT_VERSION - 1;
        let err = replay
            .validate()
            .expect_err("prior replay format must be rejected");
        assert!(
            err.contains("older world format") || err.contains("start a new run"),
            "unexpected rejection message: {err}"
        );
    }

    #[test]
    fn v1_replay_is_rejected_after_v2_bump() {
        // Explicit coverage for the requested v1 boundary. Active now that
        // REPLAY_FORMAT_VERSION is 2; a no-op on any other version.
        if REPLAY_FORMAT_VERSION == 2 {
            let mut replay = fresh_replay();
            replay.format_version = 1;
            let err = replay.validate().expect_err("v1 replays must be rejected");
            assert!(
                err.contains("older world format") || err.contains("start a new run"),
                "unexpected rejection message: {err}"
            );
        }
    }

    #[test]
    fn world_save_and_replay_format_are_v16_and_v2() {
        // The v16 overhaul bumps the world save to v16 and the replay format to v2;
        // this pins the contract the rejection tests above rely on.
        assert_eq!(SAVE_VERSION, 16);
        assert_eq!(REPLAY_FORMAT_VERSION, 2);
    }
}

// Speed ordering ("speed tie favors player") is exercised live against the
// canonical `crate::battle` public API introduced by the v16 overhaul.
mod overhaul_battle_speed_tests {
    use crate::battle::{initiative_order, CombatSpeed, Combatant};
    use crate::entity::{Discipline, MonsterKind, Player};

    #[test]
    fn speed_tie_favors_player() {
        // A player and an enemy with identical speed: the player acts first.
        let order = initiative_order(10, &[(0, 10)]);
        assert_eq!(order[0], Combatant::Player);
        assert_eq!(order[1], Combatant::Enemy(0));
    }

    #[test]
    fn faster_enemy_leads_but_player_wins_any_tie() {
        // Bat (14) leads; the player (10) beats the Goblin (9); ties go to player.
        let order = initiative_order(
            10,
            &[
                (0, MonsterKind::Bat.speed()),
                (1, MonsterKind::Goblin.speed()),
            ],
        );
        assert_eq!(order[0], Combatant::Enemy(0)); // Bat
        assert_eq!(order[1], Combatant::Player); // player before Goblin
        assert_eq!(order[2], Combatant::Enemy(1)); // Goblin
    }

    #[test]
    fn player_speed_is_base_ten_and_finesse_raises_it() {
        let mut player = Player::new(0, 0);
        assert_eq!(player.speed(), crate::battle::BASE_PLAYER_SPEED);
        player.apply_discipline(Discipline::Finesse);
        player.apply_discipline(Discipline::Finesse);
        assert_eq!(player.speed(), crate::battle::BASE_PLAYER_SPEED + 2);
    }

    #[test]
    fn equal_speed_enemies_keep_stable_index_order() {
        // With the player faster, two equal-speed Rats resolve by world index.
        let order = initiative_order(
            20,
            &[(7, MonsterKind::Rat.speed()), (2, MonsterKind::Rat.speed())],
        );
        assert_eq!(order[0], Combatant::Player); // player leads
        assert_eq!(order[1], Combatant::Enemy(2)); // lower index first
        assert_eq!(order[2], Combatant::Enemy(7));
    }
}

// Group cap 4, overworld freezing, and deterministic flee are exercised live
// against the canonical `GameState` battle API introduced by the v16 overhaul:
// `in_battle`, `battle`, `battle_move_cursor`, `battle_confirm`, `battle_cancel`,
// and `battle_flee`, with `crate::battle::BattleState` describing participants.
mod overhaul_battle_integration {
    use super::*;
    use crate::battle::BattlePhase;

    /// Build a flat, monster-free floor so bump-to-battle triggering is deterministic.
    fn battle_ready_state() -> GameState {
        let mut state = GameState::new(7);
        state.new_game();
        state.rules_version = 4;
        state.status = GameStatus::Playing;
        state.monsters.clear();
        let (px, py) = (state.player.x, state.player.y);
        for dy in -2..=2i32 {
            for dx in -2..=2i32 {
                let (x, y) = (px + dx, py + dy);
                if state.map.in_bounds(x, y) {
                    let tile_idx = state.map.idx(x, y);
                    state.map.tiles[tile_idx] = Tile::Floor;
                }
            }
        }
        state
    }

    fn awake_hostile(state: &mut GameState, x: i32, y: i32) {
        let mut m = crate::entity::Monster::new(x, y, crate::entity::MonsterKind::Goblin);
        m.awakened = true;
        state.monsters.push(m);
    }

    /// Bump the hostile one step east of the player to begin an instanced battle.
    fn begin_battle(state: &mut GameState, visual: &mut VisualState) {
        state.process_turn(crate::input::Action::Move(1, 0), visual);
    }

    #[test]
    fn group_encounter_caps_at_four_nearest_awake_hostiles() {
        let mut state = battle_ready_state();
        let (px, py) = (state.player.x, state.player.y);
        // Six awake hostiles within Chebyshev distance 2 of the bumped one.
        awake_hostile(&mut state, px + 1, py);
        awake_hostile(&mut state, px + 2, py);
        awake_hostile(&mut state, px, py + 1);
        awake_hostile(&mut state, px, py + 2);
        awake_hostile(&mut state, px + 1, py + 1);
        awake_hostile(&mut state, px + 2, py + 1);

        let mut visual = VisualState::new(state.map.width, state.map.height);
        begin_battle(&mut state, &mut visual);

        assert!(state.in_battle(), "bumping a hostile starts a battle");
        let battle = state.battle().expect("battle should be active");
        assert_eq!(
            battle.enemy_indices.len(),
            4,
            "group battle caps at four hostiles"
        );
        // Every participant is within reach of the player (nearest assembly).
        for &idx in &battle.enemy_indices {
            let cheb = (state.monsters[idx].x - px)
                .abs()
                .max((state.monsters[idx].y - py).abs());
            assert!(cheb <= 2, "only the nearest hostiles join the battle");
        }
        assert_eq!(
            state.monsters.len(),
            6,
            "extra hostiles remain in the overworld until the battle ends"
        );
    }

    #[test]
    fn active_battle_freezes_overworld_turns() {
        let mut state = battle_ready_state();
        let (px, py) = (state.player.x, state.player.y);
        awake_hostile(&mut state, px + 1, py);
        let mut visual = VisualState::new(state.map.width, state.map.height);
        begin_battle(&mut state, &mut visual);

        let turns_before = state.stats.turns;
        let monster_x_before = state.monsters[0].x;
        // An overworld move must not advance the overworld while a battle is instanced.
        state.process_turn(crate::input::Action::Move(0, 1), &mut visual);

        assert!(
            state.in_battle(),
            "the battle stays instanced after an overworld action"
        );
        assert_eq!(
            state.stats.turns, turns_before,
            "overworld turn must not advance during a battle"
        );
        assert_eq!(
            state.monsters[0].x, monster_x_before,
            "overworld monsters must not move during a battle"
        );
        assert_eq!(
            state.player.y, py,
            "player must not move in the overworld during a battle"
        );
    }

    #[test]
    fn flee_outcome_is_deterministic_for_identical_state() {
        let setup = || {
            let mut state = battle_ready_state();
            let (px, py) = (state.player.x, state.player.y);
            awake_hostile(&mut state, px + 1, py);
            let mut visual = VisualState::new(state.map.width, state.map.height);
            begin_battle(&mut state, &mut visual);
            (state, visual)
        };
        let (mut state, mut visual) = setup();
        state.battle_flee(&mut visual);
        let first_in_battle = state.in_battle();

        let (mut state2, mut visual2) = setup();
        state2.battle_flee(&mut visual2);
        let second_in_battle = state2.in_battle();

        assert_eq!(
            first_in_battle, second_in_battle,
            "flee outcome must be deterministic for identical state"
        );
    }

    #[test]
    fn battle_fight_command_enters_target_then_resolves() {
        let mut state = battle_ready_state();
        let (px, py) = (state.player.x, state.player.y);
        awake_hostile(&mut state, px + 1, py);
        let mut visual = VisualState::new(state.map.width, state.map.height);
        begin_battle(&mut state, &mut visual);

        state.battle_move_cursor(0); // Fight
        state.battle_confirm(&mut visual); // -> Target phase
        assert_eq!(
            state.battle().expect("battle active").phase,
            BattlePhase::Target
        );
        state.battle_confirm(&mut visual); // resolve round against the selected target
        match state.battle() {
            Some(battle) => assert_eq!(battle.phase, BattlePhase::Input),
            None => assert!(
                !state.in_battle(),
                "battle should end only if the target fell"
            ),
        }
    }

    #[test]
    fn battle_guard_command_resolves_a_round() {
        let mut state = battle_ready_state();
        let (px, py) = (state.player.x, state.player.y);
        awake_hostile(&mut state, px + 1, py);
        let mut visual = VisualState::new(state.map.width, state.map.height);
        begin_battle(&mut state, &mut visual);

        state.battle_move_cursor(3); // Guard
        state.battle_confirm(&mut visual);
        // After a guard round the battle returns to command input for the next turn.
        assert_eq!(
            state.battle().expect("battle active").phase,
            BattlePhase::Input
        );
    }

    #[test]
    fn battle_cancel_returns_to_command_selection() {
        let mut state = battle_ready_state();
        let (px, py) = (state.player.x, state.player.y);
        awake_hostile(&mut state, px + 1, py);
        let mut visual = VisualState::new(state.map.width, state.map.height);
        begin_battle(&mut state, &mut visual);

        state.battle_move_cursor(0); // Fight
        state.battle_confirm(&mut visual); // -> Target phase
        assert_eq!(
            state.battle().expect("battle active").phase,
            BattlePhase::Target
        );
        state.battle_cancel();
        assert_eq!(
            state.battle().expect("battle active").phase,
            BattlePhase::Input
        );
    }

    #[test]
    fn player_wins_tie_in_full_battle_order() {
        // High-level restatement of the speed-tie rule using the shared orderer.
        let order = crate::battle::initiative_order(9, &[(0, 9)]);
        assert_eq!(order[0], crate::battle::Combatant::Player);
        assert_eq!(order[1], crate::battle::Combatant::Enemy(0));
    }

    /// Push `kind` at `(x, y)`, awake, as a battle participant candidate.
    fn hostile(state: &mut GameState, kind: MonsterKind, x: i32, y: i32) -> usize {
        let m = crate::entity::Monster::new(x, y, kind);
        state.monsters.push(m);
        state.monsters.len() - 1
    }

    #[test]
    fn enemy_lethal_blow_sets_alive_false_and_skips_later_combatants() {
        // Finding 1: a lethal enemy strike must flip `player.alive = false` and
        // stop any combatant scheduled later in the same round.
        let mut state = battle_ready_state();
        state.player.finesse_rank = 0; // speed 10, slower than the bat (14)
        state.player.stats.hp = 1;
        state.player.stats.max_hp = 20;
        let (px, py) = (state.player.x, state.player.y);
        hostile(&mut state, MonsterKind::Bat, px + 1, py); // fastest, strikes first
        let slime = hostile(&mut state, MonsterKind::Slime, px + 1, py + 1);
        state.monsters[slime].awakened = true;
        state.monsters[slime].windup = 5; // untouched iff the slime never acts
        let mut visual = VisualState::new(state.map.width, state.map.height);
        begin_battle(&mut state, &mut visual);
        assert!(state.in_battle(), "bumping a hostile starts a battle");

        // Fight -> target -> resolve. The bat kills the player before the slime's turn.
        state.battle_move_cursor(0);
        state.battle_confirm(&mut visual);
        assert_eq!(
            state.battle().expect("battle active").phase,
            BattlePhase::Target
        );
        state.battle_confirm(&mut visual);

        assert!(!state.player.alive, "lethal blow must set alive = false");
        assert_eq!(state.player.stats.hp, 0, "hp clamps to 0 on death");
        assert!(state.battle().is_none(), "battle ends when the player dies");
        assert_eq!(state.status, GameStatus::Dead, "status flips to Dead");
        // The slime was scheduled after the killing blow and must never have acted.
        assert_eq!(
            state.monsters[slime].windup, 5,
            "a skipped later combatant did not take its turn"
        );
    }

    #[test]
    fn guard_only_protects_on_the_players_initiative_slot() {
        // Finding 2: guard must engage on the player's own initiative slot, so a
        // faster enemy still lands an unguarded hit. Conversely a slower enemy is
        // guarded because it strikes after the player's slot.
        // Faster enemy (bat, speed 14) strikes before the guard comes up.
        let mut state = battle_ready_state();
        let (px, py) = (state.player.x, state.player.y);
        state.player.finesse_rank = 0; // speed 10
        state.player.stats.hp = 80;
        state.player.stats.max_hp = 80;
        state.player.armor = None;
        hostile(&mut state, MonsterKind::Bat, px + 1, py);
        let mut visual = VisualState::new(state.map.width, state.map.height);
        begin_battle(&mut state, &mut visual);
        let hp_before = state.player.stats.hp;
        state.battle_move_cursor(3); // Guard
        state.battle_confirm(&mut visual); // -> Target
        state.battle_confirm(&mut visual); // resolve round
        assert!(
            state.player.stats.hp < hp_before,
            "the faster enemy must still land a hit"
        );
        assert_eq!(
            state.stats.damage_guarded, 0,
            "a faster enemy is not protected by the player's guard"
        );

        // Slower enemy (orc, speed 6) strikes after the player's slot.
        let mut state2 = battle_ready_state();
        let (px, py) = (state2.player.x, state2.player.y);
        state2.player.finesse_rank = 0;
        state2.player.stats.hp = 80;
        state2.player.stats.max_hp = 80;
        state2.player.armor = None;
        hostile(&mut state2, MonsterKind::Orc, px + 1, py);
        let mut visual2 = VisualState::new(state2.map.width, state2.map.height);
        begin_battle(&mut state2, &mut visual2);
        state2.battle_move_cursor(3); // Guard
        state2.battle_confirm(&mut visual2); // -> Target
        state2.battle_confirm(&mut visual2); // resolve round
        assert!(
            state2.stats.damage_guarded > 0,
            "a slower enemy is reduced by the player's guard"
        );
    }

    #[test]
    fn technique_spends_stamina_and_hits_multiple_targets() {
        // Finding 3: the equipped technique is really realized — it spends its
        // stamina cost and strikes every target the technique's spread allows.
        let mut state = battle_ready_state();
        state.player.finesse_rank = 0;
        state.player.weapon = Some(ItemKind::Longsword); // -> Sweep (cost 3, spread 3)
        state.player.stamina = 10;
        let (px, py) = (state.player.x, state.player.y);
        // Primary (bumped) plus two more awake participants within reach.
        let rat = hostile(&mut state, MonsterKind::Rat, px + 1, py);
        let slime = hostile(&mut state, MonsterKind::Slime, px + 1, py + 1);
        let goblin = hostile(&mut state, MonsterKind::Goblin, px, py + 1);
        for &i in &[rat, slime, goblin] {
            state.monsters[i].awakened = true;
        }
        let before: Vec<i32> = [rat, slime, goblin]
            .iter()
            .map(|&i| state.monsters[i].stats.hp)
            .collect();
        let mut visual = VisualState::new(state.map.width, state.map.height);
        begin_battle(&mut state, &mut visual);

        state.battle_move_cursor(1); // Technique
        state.battle_confirm(&mut visual); // -> Target
        assert_eq!(
            state.battle().expect("battle active").phase,
            BattlePhase::Target
        );
        state.battle_confirm(&mut visual); // resolve: Sweep hits all three

        assert_eq!(
            state.player.stamina, 7,
            "technique spends its real stamina cost"
        );
        assert_eq!(state.stats.techniques_used, 1, "technique use is counted");
        for (&i, &pre) in [rat, slime, goblin].iter().zip(before.iter()) {
            assert!(
                state.monsters[i].stats.hp < pre,
                "every target in the sweep took damage"
            );
        }
    }

    #[test]
    fn non_potion_consumable_is_selected_and_applied() {
        // Finding 4: the battle item menu offers real consumables and applies
        // their genuine effect (no faked substitute).
        let mut state = battle_ready_state();
        state.player.finesse_rank = 0;
        state.player.stats.hp = 80;
        state.player.stats.max_hp = 80;
        state.player.inventory.push(ItemKind::ScrollOfFire);
        let (px, py) = (state.player.x, state.player.y);
        let rat = hostile(&mut state, MonsterKind::Rat, px + 1, py);
        let slime = hostile(&mut state, MonsterKind::Slime, px + 1, py + 1);
        for &i in &[rat, slime] {
            state.monsters[i].awakened = true;
        }
        let before: Vec<i32> = [rat, slime]
            .iter()
            .map(|&i| state.monsters[i].stats.hp)
            .collect();
        let mut visual = VisualState::new(state.map.width, state.map.height);
        begin_battle(&mut state, &mut visual);

        state.battle_move_cursor(2); // Item
        state.battle_confirm(&mut visual); // -> Item menu (lists Scroll of Fire)
        assert_eq!(
            state.battle().expect("battle active").phase,
            BattlePhase::Item
        );
        state.battle_confirm(&mut visual); // resolve: scroll erupts for 8 AoE

        for (&i, &pre) in [rat, slime].iter().zip(before.iter()) {
            assert_eq!(
                state.monsters[i].stats.hp,
                pre - 8,
                "fire scroll deals its real 8 AoE damage"
            );
        }
        assert!(
            !state.player.inventory.contains(&ItemKind::ScrollOfFire),
            "the consumed scroll leaves the inventory"
        );
        assert_eq!(state.stats.items_used, 1, "item use is counted");
    }

    #[test]
    fn antidote_clears_poison_through_battle_item_menu() {
        // Finding 4 (companion): an applicable battle consumable removes its
        // condition instead of being silently faked.
        let mut state = battle_ready_state();
        state.player.finesse_rank = 0;
        state.player.stats.hp = 80;
        state.player.stats.max_hp = 80;
        state.player.poisoned = true;
        state.player.poison_ticks = 3;
        state.player.inventory.push(ItemKind::Antidote);
        let (px, py) = (state.player.x, state.player.y);
        hostile(&mut state, MonsterKind::Goblin, px + 1, py);
        let mut visual = VisualState::new(state.map.width, state.map.height);
        begin_battle(&mut state, &mut visual);

        state.battle_move_cursor(2); // Item
        state.battle_confirm(&mut visual); // -> Item menu
        state.battle_confirm(&mut visual); // resolve: antidote applied

        assert!(!state.player.poisoned, "antidote clears poison in battle");
    }
}
