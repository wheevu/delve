// ── Monster AI strategies and special abilities ──
//
// Replaces the flat "every monster uses flow field" approach with per‑kind
// strategies.  Each kind can have its own movement pattern, special attack,
// and behaviour hooks.

use rand::Rng;

use crate::entity::{Monster, MonsterKind, Player};
#[cfg(test)]
use crate::map::Map;

/// How a monster behaves when the player is in sight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiStrategy {
    /// Chase the player using flow field (default).
    Chase,
    /// Keep at range 3‑5 from the player, flee if too close.
    Ranged,
    /// Stationary — never moves.
    Stationary,
}

/// What kind of special attack a monster can use.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpecialAbility {
    None,
    /// Applies bleeding to the player (dot over N turns).
    Bleed {
        duration: i32,
        damage_per_turn: i32,
    },
    /// Slows the player (reduces their movement options).
    WebSlow {
        duration: i32,
    },
    /// Drains XP or max HP from the player.
    Drain {
        hp_amount: i32,
        max_hp_reduction: i32,
    },
    /// When killed, has a chance to reassemble with partial HP.
    Reassemble {
        chance: f64,
        hp_pct: f64,
    },
    /// Fires a ranged bolt (damage, max range).
    RangedBolt {
        damage: i32,
        max_range: i32,
    },
    /// Splits into two smaller copies when hit.
    Split {
        chance: f64,
    },
}

/// Map each monster kind to its AI strategy and special ability.
pub fn monster_profile(kind: MonsterKind) -> (AiStrategy, SpecialAbility) {
    match kind {
        MonsterKind::Rat => (
            AiStrategy::Chase,
            SpecialAbility::Bleed {
                duration: 3,
                damage_per_turn: 1,
            },
        ),
        MonsterKind::Bat => (AiStrategy::Chase, SpecialAbility::None),
        MonsterKind::Goblin => (AiStrategy::Chase, SpecialAbility::None),
        MonsterKind::Skeleton => (
            AiStrategy::Chase,
            SpecialAbility::Reassemble {
                chance: 0.30,
                hp_pct: 0.4,
            },
        ),
        MonsterKind::Orc => (AiStrategy::Chase, SpecialAbility::None),
        MonsterKind::Spider => (AiStrategy::Chase, SpecialAbility::WebSlow { duration: 3 }),
        MonsterKind::Wraith => (
            AiStrategy::Chase,
            SpecialAbility::Drain {
                hp_amount: 3,
                max_hp_reduction: 1,
            },
        ),
        MonsterKind::Slime => (AiStrategy::Chase, SpecialAbility::Split { chance: 0.25 }),
        MonsterKind::Mimic => (AiStrategy::Stationary, SpecialAbility::None),
        MonsterKind::Necromancer => (
            AiStrategy::Ranged,
            SpecialAbility::RangedBolt {
                damage: 6,
                max_range: 6,
            },
        ),
        MonsterKind::Sentinel => (AiStrategy::Stationary, SpecialAbility::None),
        MonsterKind::Guardian => (AiStrategy::Chase, SpecialAbility::None),
        MonsterKind::Stalker => (AiStrategy::Chase, SpecialAbility::None),
        MonsterKind::BellWarden => (AiStrategy::Chase, SpecialAbility::None),
    }
}

/// Try to apply a monster's special ability to the player.
/// Returns a message describing what happened.
pub fn apply_special_ability(
    monster: &Monster,
    player: &mut Player,
    rng: &mut impl Rng,
) -> Option<String> {
    let (_, ability) = monster_profile(monster.kind);
    match ability {
        SpecialAbility::None => None,
        SpecialAbility::Bleed {
            duration,
            damage_per_turn: _,
        } => {
            if !player.poisoned {
                // Reuse poison mechanic for bleeding
                player.poisoned = true;
                player.poison_ticks = duration;
                Some("You're bleeding!".to_string())
            } else {
                None
            }
        }
        SpecialAbility::WebSlow { duration: _ } => {
            // Slow effect — half movement range for N turns
            // For now, just notify the player
            Some("Webs slow your movements!".to_string())
        }
        SpecialAbility::Drain {
            hp_amount,
            max_hp_reduction,
        } => {
            player.stats.hp = (player.stats.hp - hp_amount).max(1);
            player.stats.max_hp = (player.stats.max_hp - max_hp_reduction).max(10);
            player.stats.hp = player.stats.hp.min(player.stats.max_hp);
            Some(format!(
                "The {} drains your life force! (-{} max HP)",
                monster.kind.name(),
                max_hp_reduction
            ))
        }
        SpecialAbility::Reassemble { chance, hp_pct: _ } => {
            if rng.gen_bool(chance) {
                Some(format!("The {} will reassemble!", monster.kind.name()))
            } else {
                None
            }
        }
        SpecialAbility::RangedBolt {
            damage: _,
            max_range: _,
        } => None,
        SpecialAbility::Split { chance: _ } => None,
    }
}

/// Process a ranged attack from a monster against the player.
/// Returns (damage, message) or None if out of range.
#[cfg(test)]
pub fn try_ranged_attack(
    monster: &Monster,
    player: &Player,
    map: &Map,
    ability: &SpecialAbility,
) -> Option<(i32, String)> {
    let (damage, max_range) = match ability {
        SpecialAbility::RangedBolt { damage, max_range } => (*damage, *max_range),
        _ => return None,
    };

    let dx = player.x - monster.x;
    let dy = player.y - monster.y;
    let dist_sq = dx * dx + dy * dy;
    if dist_sq > (max_range * max_range) {
        return None;
    }

    // Simple line‑of‑sight (Bresenham check)
    let mut _x = monster.x;
    let mut _y = monster.y;
    let steps = dx.abs().max(dy.abs());
    if steps == 0 {
        return Some((
            damage,
            format!("The {} fires a dark bolt!", monster.kind.name()),
        ));
    }
    for i in 1..steps {
        let tx = monster.x + (dx * i) / steps;
        let ty = monster.y + (dy * i) / steps;
        if map.blocks_fov(tx, ty) {
            return None; // blocked by wall/door
        }
    }

    Some((
        damage,
        format!("The {} fires a dark bolt!", monster.kind.name()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_monsters_have_profile() {
        use MonsterKind::*;
        for kind in &[
            Rat,
            Bat,
            Goblin,
            Skeleton,
            Orc,
            Spider,
            Wraith,
            Slime,
            Mimic,
            Necromancer,
            Sentinel,
            Guardian,
        ] {
            let (_, _) = monster_profile(*kind);
        }
    }

    #[test]
    fn ranged_attack_blocked_by_wall() {
        let mut map = Map::new(20, 20);
        for i in 0..400 {
            map.tiles[i] = crate::map::Tile::Floor;
        }
        let wall_idx = map.idx(5, 5);
        map.tiles[wall_idx] = crate::map::Tile::Wall;

        let monster = Monster::new(1, 5, MonsterKind::Necromancer);
        let player = Player::new(10, 5);
        let ability = SpecialAbility::RangedBolt {
            damage: 6,
            max_range: 10,
        };
        assert!(try_ranged_attack(&monster, &player, &map, &ability).is_none());
    }

    #[test]
    fn ranged_attack_hits_when_clear() {
        let mut map = Map::new(20, 20);
        for i in 0..400 {
            map.tiles[i] = crate::map::Tile::Floor;
        }
        let monster = Monster::new(1, 5, MonsterKind::Necromancer);
        let player = Player::new(5, 5);
        let ability = SpecialAbility::RangedBolt {
            damage: 6,
            max_range: 10,
        };
        assert!(try_ranged_attack(&monster, &player, &map, &ability).is_some());
    }
}
