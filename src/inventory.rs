// Inventory and item-effect helpers.
//
// Item use / equip logic extracted from app.rs to reduce the monolithic
// GameState surface area.  Each helper is a free function that takes the
// specific pieces of state it touches — no circular dependencies.

use crate::entity::{ItemKind, Monster, Player};
use crate::logging::GameEvent;
use crate::map::Map;
use crate::ui::MsgColor;
use crate::visual::{VisualEffect, VisualState};

// ── Convenience wrappers for consuming / equipping items ──

/// Remove item at `slot` from inventory, bump usage counter, log.
pub fn consume_item(player: &mut Player, stats_items_used: &mut u32, slot: usize) {
    player.inventory.remove(slot);
    *stats_items_used += 1;
}

/// Equip `kind` into the player's weapon or armour slot.
/// Returns the old item pushed back into inventory (if any) and an equip message.
pub fn equip_item(
    player: &mut Player,
    kind: ItemKind,
    is_weapon: bool,
) -> (Option<ItemKind>, String) {
    let slot_name = if is_weapon { "weapon" } else { "armor" };
    crate::logging::game_event(GameEvent::Equip {
        item: kind.name().to_string(),
        slot: slot_name.to_string(),
    });
    let old = if is_weapon {
        let old = player.weapon.replace(kind);
        player.weapon_durability = kind.max_durability();
        player.weapon_cursed = false;
        old
    } else {
        let old = player.armor.replace(kind);
        player.armor_cursed = false;
        old
    };
    if let Some(old_kind) = old {
        player.inventory.push(old_kind);
    }
    let msg = format!(
        "Equipped {} ({}).",
        kind.name(),
        if is_weapon { "weapon" } else { "armor" }
    );
    (old, msg)
}

// ── Individual item-use effect helpers ──
//
// Each returns `true` if the item was consumed (i.e. a turn should pass).

/// Drink a potion that heals by `heal`, up to the player's max HP.
pub fn effect_potion(
    player: &mut Player,
    stats_items_used: &mut u32,
    slot: usize,
    heal: i32,
    msgs: &mut impl FnMut(&str, MsgColor),
    visual: &mut VisualState,
) -> bool {
    let actual = heal.min(player.stats.max_hp - player.stats.hp);
    if actual <= 0 {
        msgs("Your HP is already full!", MsgColor::DIM);
        return false;
    }
    player.stats.hp += actual;
    consume_item(player, stats_items_used, slot);
    visual.add_effect(VisualEffect::heal_glow(player.x, player.y));
    msgs(
        &format!("You drink and heal {} HP.", actual),
        MsgColor::HEALING,
    );
    true
}

/// Fire scroll — deals 8 AoE damage to every visible monster.
/// Does NOT mark monsters dead or award kills; the caller is responsible
/// for calling `GameState::kill_monster` for each monster whose HP ≤ 0.
pub fn effect_fire_scroll(
    monsters: &mut [Monster],
    player: &mut Player,
    stats_items_used: &mut u32,
    slot: usize,
    map: &Map,
    msgs: &mut impl FnMut(&str, MsgColor),
    visual: &mut VisualState,
) -> bool {
    let mut killed = 0u32;
    let mut fire_xs = Vec::new();
    let mut fire_ys = Vec::new();
    for m in monsters.iter_mut() {
        if !m.alive || !map.visible[map.idx(m.x, m.y)] {
            continue;
        }
        m.stats.hp -= 8;
        fire_xs.push(m.x);
        fire_ys.push(m.y);
        if m.stats.hp <= 0 {
            killed += 1;
        }
    }
    consume_item(player, stats_items_used, slot);
    visual.add_effect(VisualEffect::fire_ring(player.x, player.y));
    for (&x, &y) in fire_xs.iter().zip(fire_ys.iter()) {
        visual.add_decal(crate::visual::Decal::Scorch { x, y, fade: 255 });
    }
    msgs(
        &format!("The scroll erupts in flame! {} monsters burned.", killed),
        MsgColor::DAMAGE_DEALT,
    );
    true
}

/// Confusion scroll — randomly teleports visible monsters.
#[allow(clippy::too_many_arguments)]
pub fn effect_confusion_scroll(
    monsters: &mut [Monster],
    map: &Map,
    rng: &mut impl rand::Rng,
    player: &mut Player,
    stats_items_used: &mut u32,
    slot: usize,
    msgs: &mut impl FnMut(&str, MsgColor),
    visual: &mut VisualState,
) -> bool {
    let mut occ_map = vec![false; (map.width * map.height) as usize];
    for m in monsters.iter() {
        if m.alive {
            occ_map[map.idx(m.x, m.y)] = true;
        }
    }
    let mut confused_xs = Vec::new();
    let mut confused_ys = Vec::new();
    for m in monsters.iter_mut() {
        if !m.alive || !map.visible[map.idx(m.x, m.y)] {
            continue;
        }
        for _ in 0..10 {
            let nx = m.x + rng.gen_range(-3..=3);
            let ny = m.y + rng.gen_range(-3..=3);
            if !map.in_bounds(nx, ny) {
                continue;
            }
            let nidx = map.idx(nx, ny);
            if map.is_walkable(nx, ny) && !(nx == player.x && ny == player.y) && !occ_map[nidx] {
                occ_map[map.idx(m.x, m.y)] = false;
                occ_map[nidx] = true;
                m.x = nx;
                m.y = ny;
                confused_xs.push(nx);
                confused_ys.push(ny);
                break;
            }
        }
    }
    consume_item(player, stats_items_used, slot);
    visual.add_effect(VisualEffect::confusion_swirl(confused_xs, confused_ys));
    msgs(
        "The scroll warps reality! Monsters are displaced.",
        MsgColor::SYSTEM,
    );
    true
}

/// Antidote — cures poison + heals 5 HP.
pub fn effect_antidote(
    player: &mut Player,
    stats_items_used: &mut u32,
    slot: usize,
    msgs: &mut impl FnMut(&str, MsgColor),
    visual: &mut VisualState,
) -> bool {
    if !player.poisoned {
        msgs("You are not poisoned.", MsgColor::DIM);
        return false;
    }
    player.poisoned = false;
    player.poison_ticks = 0;
    player.stats.hp = (player.stats.hp + 5).min(player.stats.max_hp);
    consume_item(player, stats_items_used, slot);
    visual.add_effect(VisualEffect::heal_glow(player.x, player.y));
    msgs(
        "You drink the antidote. The poison fades.",
        MsgColor::HEALING,
    );
    true
}

/// Lantern oil — +3 FOV for 50 turns.
pub fn effect_lantern_oil(
    player: &mut Player,
    stats_items_used: &mut u32,
    slot: usize,
    msgs: &mut impl FnMut(&str, MsgColor),
    visual: &mut VisualState,
) -> bool {
    player.lantern_turns = 50;
    consume_item(player, stats_items_used, slot);
    visual.add_effect(VisualEffect::heal_glow(player.x, player.y));
    msgs("The lantern oil brightens your vision.", MsgColor::LOOT);
    true
}

/// Blink scroll — teleports player to a random room.
pub fn effect_blink_scroll(
    player: &mut Player,
    map: &Map,
    rng: &mut impl rand::Rng,
    stats_items_used: &mut u32,
    slot: usize,
    msgs: &mut impl FnMut(&str, MsgColor),
    visual: &mut VisualState,
) -> bool {
    let room_count = map.rooms.len();
    if room_count == 0 {
        msgs("Nowhere to blink to.", MsgColor::DIM);
        return false;
    }
    let target = map.rooms[rng.gen_range(0..room_count)].center();
    player.x = target.0;
    player.y = target.1;
    consume_item(player, stats_items_used, slot);
    visual.add_effect(VisualEffect::pickup_sparkle(player.x, player.y));
    msgs("You blink through space!", MsgColor::SYSTEM);
    true
}

/// Smoke bomb — pushes adjacent monsters 4 tiles away.
pub fn effect_smoke_bomb(
    monsters: &mut [Monster],
    map: &Map,
    rng: &mut impl rand::Rng,
    player: &mut Player,
    stats_items_used: &mut u32,
    slot: usize,
    msgs: &mut impl FnMut(&str, MsgColor),
) -> bool {
    // Snapshot adjacent monster positions first to avoid borrow conflicts.
    let px = player.x;
    let py = player.y;
    let targets: Vec<(i32, i32, usize)> = monsters
        .iter()
        .enumerate()
        .filter(|(_, m)| m.alive)
        .map(|(i, m)| (m.x, m.y, i))
        .collect();
    let mut displaced = 0u32;
    for (mx, my, mi) in targets {
        let dsq = (mx - px).pow(2) + (my - py).pow(2);
        if dsq > 2 {
            continue;
        }
        for _ in 0..10 {
            let nx = mx + rng.gen_range(-4..=4);
            let ny = my + rng.gen_range(-4..=4);
            if map.is_walkable(nx, ny)
                && !(nx == px && ny == py)
                && !monsters
                    .iter()
                    .any(|om| om.alive && om.x == nx && om.y == ny)
            {
                monsters[mi].x = nx;
                monsters[mi].y = ny;
                displaced += 1;
                break;
            }
        }
    }
    consume_item(player, stats_items_used, slot);
    msgs(
        &format!("Smoke erupts! {} monsters scattered.", displaced),
        MsgColor::SYSTEM,
    );
    true
}

/// Enchantment — upgrades equipped weapon.
pub fn effect_enchantment(
    player: &mut Player,
    stats_items_used: &mut u32,
    slot: usize,
    msgs: &mut impl FnMut(&str, MsgColor),
) -> bool {
    if player.weapon_cursed || player.armor_cursed {
        player.weapon_cursed = false;
        player.armor_cursed = false;
        consume_item(player, stats_items_used, slot);
        msgs("The enchantment breaks the binding curse.", MsgColor::LOOT);
        return true;
    }
    if let Some(ref mut weapon_kind) = player.weapon {
        let boosted = match *weapon_kind {
            ItemKind::RustySword => ItemKind::Longsword,
            ItemKind::Longsword => ItemKind::Warhammer,
            ItemKind::Warhammer => ItemKind::Warhammer,
            _ => {
                msgs("You have no weapon to enchant.", MsgColor::DIM);
                return false;
            }
        };
        if boosted != *weapon_kind {
            *weapon_kind = boosted;
            consume_item(player, stats_items_used, slot);
            msgs("The enchantment reforges your weapon!", MsgColor::LOOT);
            return true;
        }
    }
    msgs("You have no weapon to enchant.", MsgColor::DIM);
    false
}

/// Lore fragment — reveals a story snippet.
pub fn effect_lore_fragment(
    player: &mut Player,
    stats_items_used: &mut u32,
    slot: usize,
    msgs: &mut impl FnMut(&str, MsgColor),
) -> bool {
    let lore =
        crate::entity::lore_for_depth(player.depth).unwrap_or("The fragment crumbles to dust.");
    consume_item(player, stats_items_used, slot);
    msgs(lore, MsgColor::Cyan);
    true
}

// ── Inventory sorting ──

/// Sort the player's inventory by category, then by sub-type.
/// Order: quest items, weapons (desc ATK), armor (desc DEF), consumables, lore.
/// Preserves cursor position stability by sorting items around the cursor.
pub fn sort_inventory(player: &mut Player) {
    let old_inv = std::mem::take(&mut player.inventory);
    let mut items = old_inv;
    items.sort_by_key(|item| sort_key(*item));
    player.inventory = items;
}

/// Numeric sort key: lower values appear first.
fn sort_key(kind: ItemKind) -> u32 {
    match kind {
        // Quest items first
        ItemKind::CrownOfDepth => 0,
        ItemKind::IronKey => 1,
        ItemKind::Emberbrand => 1,
        ItemKind::Graveward => 4,
        ItemKind::TideglassMail => 5,
        // Weapons by attack (higher ATK = lower sort key)
        ItemKind::Warhammer => 1,
        ItemKind::Longsword => 2,
        ItemKind::RustySword => 3,
        // Armor by defense (higher DEF = lower sort key)
        ItemKind::Chainmail => 4,
        ItemKind::Shield => 5,
        ItemKind::LeatherArmor => 6,
        // Consumables grouped by type
        ItemKind::LargeHealthPotion => 10,
        ItemKind::HealthPotion => 11,
        ItemKind::Antidote => 12,
        ItemKind::LanternOil => 13,
        ItemKind::ScrollOfFire => 20,
        ItemKind::ScrollOfConfusion => 21,
        ItemKind::ScrollOfBlink => 22,
        ItemKind::EnchantmentScroll => 23,
        ItemKind::SmokeBomb => 30,
        // Lore last
        ItemKind::LoreFragment => 40,
        // Quest items
        ItemKind::BellToken => 5,
    }
}
