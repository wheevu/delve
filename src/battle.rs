// ══════════════════════════════════════════════════════════════════════════════
// BATTLE
//
// Canonical, self-contained battle API for the instanced "Pokémon-style"
// group battle used by `app.rs` and drawn by `battle_render.rs`.
//
// The types here are the exact public surface other modules depend on:
//
//   * `BattleState`  — the live battle snapshot. `app.rs` builds it with a
//                       struct literal, so its six fields are part of the
//                       contract and must stay public with these names.
//   * `BattlePhase`  — the per-round phase machine (Input / Target / Resolve /
//                       Victory / Defeat / Fled). `battle_render.rs` matches
//                       all six in `phase_label`.
//   * `BattleCommand`— the command menu (Fight / Technique / Item / Guard /
//                       Flee), treated as unit variants.
//   * `Combatant`    — an initiative participant (Player or Enemy(index)).
//   * `initiative_order` — builds the speed-ordered turn list.
//
// Speed helpers (`MonsterKind::speed` / `Player::speed`) are exposed through
// the local `CombatSpeed` trait. This keeps the change inside `battle.rs`:
// `MonsterKind` and `Player` live in `entity.rs`, which is out of scope for
// this edit, and the computation needs no new save-schema fields (the player's
// Finesse bonus is read from the existing `finesse_rank`).
// ══════════════════════════════════════════════════════════════════════════════

use crate::entity::{ItemKind, MonsterKind, Player};

/// Base battle speed of the player before any Finesse bonus.
pub const BASE_PLAYER_SPEED: u32 = 10;

/// A participant in a battle's initiative order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combatant {
    Player,
    Enemy(usize),
}

/// The phase of a single battle round.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattlePhase {
    /// Waiting for the player to pick a command.
    Input,
    /// Picking which enemy to act on.
    Target,
    /// Picking which consumable to use. Item selection does not need an enemy
    /// target; it is a distinct phase entered from the command menu.
    Item,
    /// Resolving the chosen action and enemy replies.
    #[expect(
        dead_code,
        reason = "Matched exhaustively by battle_render::phase_label; live gameplay currently only drives Input/Target/Item."
    )]
    Resolve,
    /// All enemies defeated.
    #[expect(
        dead_code,
        reason = "Matched exhaustively by battle_render::phase_label; live gameplay currently only drives Input/Target/Item."
    )]
    Victory,
    /// The player has fallen.
    #[expect(
        dead_code,
        reason = "Matched exhaustively by battle_render::phase_label; live gameplay currently only drives Input/Target/Item."
    )]
    Defeat,
    /// The player broke away from the fight.
    #[expect(
        dead_code,
        reason = "Matched exhaustively by battle_render::phase_label; live gameplay currently only drives Input/Target/Item."
    )]
    Fled,
}

/// A command the player can issue during `BattlePhase::Input`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleCommand {
    Fight,
    Technique,
    Item,
    Guard,
    Flee,
}

impl BattleCommand {
    /// The canonical command menu, in display and cursor order.
    pub fn all() -> &'static [BattleCommand] {
        &[
            BattleCommand::Fight,
            BattleCommand::Technique,
            BattleCommand::Item,
            BattleCommand::Guard,
            BattleCommand::Flee,
        ]
    }
}

/// Live state of an active instanced battle.
///
/// `app.rs` constructs this with a struct literal, so every field is public
/// and must keep its name and type. `battle_render.rs` reads the same fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattleState {
    /// Combatant indices (into `ctx.monsters`) gathered into this battle.
    pub enemy_indices: Vec<usize>,
    /// Cursor into `enemy_indices` for the currently targeted enemy.
    pub selected_target: usize,
    /// Cursor into `BattleCommand::all()` for the selected command (0..=4).
    pub command_cursor: usize,
    /// Cursor into the usable-item list while in the `Item` phase (0-based).
    pub item_cursor: usize,
    /// The consumable kinds offered in the `Item` phase, filled when the phase
    /// is entered. Indices line up with `item_cursor`.
    pub item_list: Vec<ItemKind>,
    /// The consumable chosen in the `Item` phase, pending the round resolve.
    pub selected_item: Option<ItemKind>,
    /// Current phase of the battle loop.
    pub phase: BattlePhase,
    /// 1-based round counter.
    pub round: u32,
    /// Latest human-readable battle message for the HUD.
    pub message: String,
}

impl BattleState {
    /// Build a fresh battle around the given enemy indices, starting in the
    /// `Input` phase on round 1 with an empty message.
    pub fn new(enemy_indices: Vec<usize>) -> Self {
        Self {
            enemy_indices,
            selected_target: 0,
            command_cursor: 0,
            item_cursor: 0,
            item_list: Vec::new(),
            selected_item: None,
            phase: BattlePhase::Input,
            round: 1,
            message: String::new(),
        }
    }

    /// The command under the current cursor.
    pub fn command(&self) -> BattleCommand {
        let cmds = BattleCommand::all();
        cmds[self.command_cursor.min(cmds.len() - 1)]
    }
}

/// Build the initiative order for a round.
///
/// Sorts every participant by speed, **descending** (fastest acts first). On a
/// speed tie the player always acts before any enemy, and equal-speed enemies
/// keep their ascending world-index order (a stable tie break).
pub fn initiative_order(player_speed: u32, enemies: &[(usize, u32)]) -> Vec<Combatant> {
    // (speed, is_player, world_index)
    let mut entries: Vec<(u32, bool, usize)> = Vec::with_capacity(enemies.len() + 1);
    entries.push((player_speed, true, usize::MAX));
    for &(index, speed) in enemies {
        entries.push((speed, false, index));
    }

    entries.sort_by(|a, b| {
        // Primary key: speed, highest first.
        match b.0.cmp(&a.0) {
            std::cmp::Ordering::Equal => {
                // Same speed: player before enemies, then lower world index first.
                match (a.1, b.1) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.2.cmp(&b.2),
                }
            }
            other => other,
        }
    });

    entries
        .into_iter()
        .map(|(.., is_player, index)| {
            if is_player {
                Combatant::Player
            } else {
                Combatant::Enemy(index)
            }
        })
        .collect()
}

/// Speed of a battle participant.
///
/// Implemented as a trait on `MonsterKind` and `Player` so the logic lives in
/// `battle.rs` without editing `entity.rs` (and without adding any save-schema
/// field). Bring the trait into scope to call `.speed()`:
///
/// ```ignore
/// use crate::battle::CombatSpeed;
/// let s = MonsterKind::Bat.speed();
/// ```
pub trait CombatSpeed {
    fn speed(&self) -> u32;
}

impl CombatSpeed for MonsterKind {
    fn speed(&self) -> u32 {
        match self {
            MonsterKind::Rat => 8,
            MonsterKind::Bat => 14,
            MonsterKind::Goblin => 9,
            MonsterKind::Skeleton => 7,
            MonsterKind::Orc => 6,
            MonsterKind::Spider => 11,
            MonsterKind::Wraith => 10,
            MonsterKind::Slime => 5,
            MonsterKind::Mimic => 7,
            MonsterKind::Necromancer => 9,
            MonsterKind::Sentinel => 8,
            MonsterKind::Guardian => 7,
            MonsterKind::Stalker => 12,
            MonsterKind::BellWarden => 9,
        }
    }
}

impl CombatSpeed for Player {
    fn speed(&self) -> u32 {
        // Base speed plus one point per Finesse rank (read from the existing
        // save-schema field, no new persistence needed).
        BASE_PLAYER_SPEED + u32::from(self.finesse_rank)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{Discipline, MonsterKind, Player};

    #[test]
    fn command_menu_order_is_stable() {
        let cmds = BattleCommand::all();
        assert_eq!(cmds.len(), 5);
        assert_eq!(cmds[0], BattleCommand::Fight);
        assert_eq!(cmds[1], BattleCommand::Technique);
        assert_eq!(cmds[2], BattleCommand::Item);
        assert_eq!(cmds[3], BattleCommand::Guard);
        assert_eq!(cmds[4], BattleCommand::Flee);
    }

    #[test]
    fn battle_state_constructor_and_command() {
        let mut bs = BattleState::new(vec![3, 7, 1]);
        assert_eq!(bs.enemy_indices, vec![3, 7, 1]);
        assert_eq!(bs.selected_target, 0);
        assert_eq!(bs.command_cursor, 0);
        assert_eq!(bs.item_cursor, 0);
        assert_eq!(bs.item_list, Vec::new());
        assert_eq!(bs.selected_item, None);
        assert_eq!(bs.phase, BattlePhase::Input);
        assert_eq!(bs.round, 1);
        assert!(bs.message.is_empty());

        // command() reads the cursor through the canonical order, clamped to
        // the last command when the cursor is set past it.
        assert_eq!(bs.command(), BattleCommand::Fight);
        bs.command_cursor = 1;
        assert_eq!(bs.command(), BattleCommand::Technique);
        bs.command_cursor = 3;
        assert_eq!(bs.command(), BattleCommand::Guard);
        bs.command_cursor = 4;
        assert_eq!(bs.command(), BattleCommand::Flee);
        bs.command_cursor = 99;
        assert_eq!(bs.command(), BattleCommand::Flee);
    }

    #[test]
    fn initiative_player_wins_a_speed_tie() {
        let order = initiative_order(10, &[(0, 10)]);
        assert_eq!(order, vec![Combatant::Player, Combatant::Enemy(0)]);
    }

    #[test]
    fn initiative_faster_enemy_leads_but_player_beats_lower_tie() {
        let order = initiative_order(
            10,
            &[
                (0, MonsterKind::Bat.speed()),
                (1, MonsterKind::Goblin.speed()),
            ],
        );
        assert_eq!(order[0], Combatant::Enemy(0)); // Bat (14)
        assert_eq!(order[1], Combatant::Player); // player (10) beats Goblin (9)
        assert_eq!(order[2], Combatant::Enemy(1)); // Goblin
    }

    #[test]
    fn initiative_equal_speed_enemies_keep_stable_index_order() {
        let order = initiative_order(
            20,
            &[(7, MonsterKind::Rat.speed()), (2, MonsterKind::Rat.speed())],
        );
        assert_eq!(
            order,
            vec![Combatant::Player, Combatant::Enemy(2), Combatant::Enemy(7)]
        );
    }

    #[test]
    fn monster_kind_speed_contract() {
        assert_eq!(MonsterKind::Bat.speed(), 14);
        assert_eq!(MonsterKind::Goblin.speed(), 9);
        assert_eq!(MonsterKind::Rat.speed(), 8);
    }

    #[test]
    fn player_speed_is_base_ten_and_finesse_raises_it() {
        let mut player = Player::new(0, 0);
        assert_eq!(player.speed(), BASE_PLAYER_SPEED);
        player.apply_discipline(Discipline::Finesse);
        player.apply_discipline(Discipline::Finesse);
        assert_eq!(player.speed(), BASE_PLAYER_SPEED + 2);
    }
}
