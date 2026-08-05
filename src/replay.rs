// ── Replay system — deterministic replay via action logging ──
//
// Records every player action (Move, Wait, UseItem, etc.) alongside the
// seed and turn number.  Stored as a simple JSON array, one line per turn.
// Replay re‑seeds the RNG and replays the action list.

use crate::app::{ChallengeMode, GameMode, GameState, GameStatus, PreparationMenu, RunStats};
use crate::campaign::CampaignState;
use crate::entity::{Discipline, ItemKind, MonsterKind, NpcKind, ThreatIntent, WeaponMastery};
use crate::input::Action;
use crate::map::{EnvironmentProp, MapTheme, SurfaceKind, Tile};
use crate::visual::VisualState;

const MAX_REPLAY_ACTIONS: usize = 1_000_000;
const MAX_REPLAY_MAP_PROPS: usize = 4_096;

/// Replay schema containing the medieval world layout contract.
pub(crate) const REPLAY_FORMAT_VERSION: u8 = 1;

/// A single turn in the action log.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ActionRecord {
    pub turn: u32,
    pub action: ActionRepr,
}

/// Serializable version of `Action`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ActionRepr {
    Move(i32, i32),
    Dodge(i32, i32),
    Technique(i32, i32),
    Wait,
    PickUp,
    Descend,
    Ascend,
    AutoExplore,
    SaveGame,
    LoadGame,
    TogglePickup,
    Talk,
    TalkAt(i32, i32),
    ConversationChoice(u8),
    ToggleDoor(i32, i32),
    Listen(i32, i32),
    Disarm(i32, i32),
    UseItem(usize), // inventory slot
    PreparationChoice { menu: PreparationMenu, choice: u8 },
    DisciplineChoice(Discipline),
    InteractEnvironment(i32, i32),
    None,
}

impl From<Action> for ActionRepr {
    fn from(a: Action) -> Self {
        match a {
            Action::Move(dx, dy) => ActionRepr::Move(dx, dy),
            Action::Dodge(dx, dy) => ActionRepr::Dodge(dx, dy),
            Action::Technique(dx, dy) => ActionRepr::Technique(dx, dy),
            Action::Wait => ActionRepr::Wait,
            Action::PickUp => ActionRepr::PickUp,
            Action::Interact => ActionRepr::None, // App records the resolved gameplay action
            Action::Descend => ActionRepr::Descend,
            Action::Ascend => ActionRepr::Ascend,
            Action::AutoExplore => ActionRepr::AutoExplore,
            Action::SaveGame => ActionRepr::SaveGame,
            Action::LoadGame => ActionRepr::LoadGame,
            Action::TogglePickup => ActionRepr::TogglePickup,
            Action::Talk => ActionRepr::Talk,
            Action::TalkAt(x, y) => ActionRepr::TalkAt(x, y),
            Action::ToggleDoor(dx, dy) => ActionRepr::ToggleDoor(dx, dy),
            Action::Listen(dx, dy) => ActionRepr::Listen(dx, dy),
            Action::Disarm(dx, dy) => ActionRepr::Disarm(dx, dy),
            Action::None => ActionRepr::None,
            Action::Quit => ActionRepr::None, // quit doesn't need replay
            Action::Inventory => ActionRepr::None, // handled separately
            Action::ConversationChoice(choice) => ActionRepr::ConversationChoice(choice),
            Action::QuestPanel => ActionRepr::None, // UI toggle only
        }
    }
}

/// Full replay file — seed + action log.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReplayData {
    pub seed: u64,
    /// Distinct from `rules_version`: this rejects files whose serialized
    /// world layout predates the current map contract without changing the
    /// deterministic rules branch used for playback.
    #[serde(default)]
    pub format_version: u8,
    #[serde(default)]
    pub game_mode: GameMode,
    #[serde(default)]
    pub challenge_mode: ChallengeMode,
    #[serde(default)]
    pub rules_version: u8,
    pub actions: Vec<ActionRecord>,
    #[serde(default)]
    pub expected: Option<ReplayOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReplayOutcome {
    pub status: GameStatus,
    pub stats: RunStats,
    pub x: i32,
    pub y: i32,
    pub depth: u32,
    pub hp: i32,
    pub gold: u32,
    pub xp: u32,
    pub level: u32,
    pub inventory: Vec<ItemKind>,
    pub weapon: Option<ItemKind>,
    pub armor: Option<ItemKind>,
    #[serde(default)]
    pub weapon_mastery: Option<WeaponMastery>,
    #[serde(default)]
    pub stamina: u8,
    #[serde(default)]
    pub evasion_turns: u8,
    #[serde(default)]
    pub counter_turns: u8,
    #[serde(default = "default_replay_stamina")]
    pub max_stamina: u8,
    #[serde(default)]
    pub vigor_rank: u8,
    #[serde(default)]
    pub finesse_rank: u8,
    #[serde(default)]
    pub resolve_rank: u8,
    #[serde(default)]
    pub pending_level_ups: u8,
    #[serde(default)]
    pub rules_version: u8,
    #[serde(default)]
    pub auto_pickup: bool,
    #[serde(default)]
    pub map_tiles: Vec<Tile>,
    #[serde(default)]
    pub map_width: i32,
    #[serde(default)]
    pub map_height: i32,
    #[serde(default)]
    pub map_explored: Vec<bool>,
    #[serde(default)]
    pub map_visible: Vec<bool>,
    #[serde(default)]
    pub map_rooms: Vec<(i32, i32, i32, i32)>,
    #[serde(default)]
    pub map_theme: MapTheme,
    #[serde(default)]
    pub map_surfaces: Option<Vec<SurfaceKind>>,
    #[serde(default)]
    pub map_props: Vec<EnvironmentProp>,
    #[serde(default)]
    pub monsters: Vec<ReplayMonster>,
    #[serde(default)]
    pub items: Vec<ReplayItem>,
    #[serde(default)]
    pub npcs: Vec<ReplayNpc>,
    #[serde(default)]
    pub projectiles: Vec<(i32, i32, i32, i32, i32, i32)>,
    #[serde(default)]
    pub campaign: Option<CampaignState>,
}

fn default_replay_stamina() -> u8 {
    crate::entity::MAX_STAMINA
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReplayMonster {
    kind: MonsterKind,
    x: i32,
    y: i32,
    hp: i32,
    alive: bool,
    poisoned: bool,
    poison_ticks: i32,
    cooldown: i32,
    awakened: bool,
    windup: u8,
    staggered: u8,
    alert_turns: u8,
    #[serde(default)]
    intent: Option<ThreatIntent>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReplayItem {
    kind: ItemKind,
    x: i32,
    y: i32,
    cursed: bool,
    durability: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReplayNpc {
    kind: NpcKind,
    x: i32,
    y: i32,
    talked: bool,
    custom_name: Option<String>,
}

impl ReplayOutcome {
    pub(crate) fn validate(&self) -> Result<(), String> {
        const MAX_MAP_DIM: i32 = 256;
        if self.map_width <= 0
            || self.map_height <= 0
            || self.map_width > MAX_MAP_DIM
            || self.map_height > MAX_MAP_DIM
        {
            return Err("replay expected map has invalid dimensions".into());
        }
        let expected_size = (self.map_width * self.map_height) as usize;
        if self.map_tiles.len() != expected_size {
            return Err("replay expected map tile count mismatch".into());
        }
        if self.map_explored.len() != expected_size {
            return Err("replay expected map explored length mismatch".into());
        }
        if self.map_visible.len() != expected_size {
            return Err("replay expected map visible length mismatch".into());
        }
        if let Some(surfaces) = &self.map_surfaces {
            if surfaces.len() != expected_size {
                return Err("replay expected map surface length mismatch".into());
            }
        }
        for (index, room) in self.map_rooms.iter().enumerate() {
            if room.0 < 0
                || room.1 < 0
                || room.2 <= 0
                || room.3 <= 0
                || room.0 > self.map_width.saturating_sub(room.2)
                || room.1 > self.map_height.saturating_sub(room.3)
            {
                return Err(format!("replay expected map room {index} out of bounds"));
            }
        }
        if self.map_props.len() > MAX_REPLAY_MAP_PROPS {
            return Err("replay expected map has too many environment props".into());
        }
        for (index, prop) in self.map_props.iter().enumerate() {
            if prop.x < 0 || prop.x >= self.map_width || prop.y < 0 || prop.y >= self.map_height {
                return Err(format!(
                    "replay expected environment prop {index} out of bounds"
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn from_state(state: &GameState) -> Self {
        Self {
            status: state.status,
            stats: state.stats.clone(),
            x: state.player.x,
            y: state.player.y,
            depth: state.player.depth,
            hp: state.player.stats.hp,
            gold: state.player.gold,
            xp: state.player.xp,
            level: state.player.level,
            inventory: state.player.inventory.clone(),
            weapon: state.player.weapon,
            armor: state.player.armor,
            weapon_mastery: state.player.weapon_mastery,
            stamina: state.player.stamina,
            evasion_turns: state.player.evasion_turns,
            counter_turns: state.player.counter_turns,
            max_stamina: state.player.max_stamina,
            vigor_rank: state.player.vigor_rank,
            finesse_rank: state.player.finesse_rank,
            resolve_rank: state.player.resolve_rank,
            pending_level_ups: state.player.pending_level_ups,
            rules_version: state.rules_version,
            auto_pickup: state.auto_pickup,
            map_tiles: state.map.tiles.clone(),
            map_width: state.map.width,
            map_height: state.map.height,
            map_explored: state.map.explored.clone(),
            map_visible: state.map.visible.clone(),
            map_rooms: state
                .map
                .rooms
                .iter()
                .map(|room| (room.x, room.y, room.w, room.h))
                .collect(),
            map_theme: state.map.theme,
            map_surfaces: state.map.surfaces.clone(),
            map_props: state.map.props.clone(),
            monsters: state
                .monsters
                .iter()
                .map(|monster| ReplayMonster {
                    kind: monster.kind,
                    x: monster.x,
                    y: monster.y,
                    hp: monster.stats.hp,
                    alive: monster.alive,
                    poisoned: monster.poisoned,
                    poison_ticks: monster.poison_ticks,
                    cooldown: monster.attack_cooldown,
                    awakened: monster.awakened,
                    windup: monster.windup,
                    staggered: monster.staggered,
                    alert_turns: monster.alert_turns,
                    intent: monster.intent,
                })
                .collect(),
            items: state
                .items
                .iter()
                .map(|item| ReplayItem {
                    kind: item.kind,
                    x: item.x,
                    y: item.y,
                    cursed: item.cursed,
                    durability: item.durability,
                })
                .collect(),
            npcs: state
                .npcs
                .iter()
                .map(|npc| ReplayNpc {
                    kind: npc.kind,
                    x: npc.x,
                    y: npc.y,
                    talked: npc.talked,
                    custom_name: npc.custom_name.clone(),
                })
                .collect(),
            projectiles: state
                .projectiles
                .iter()
                .map(|projectile| {
                    (
                        projectile.x,
                        projectile.y,
                        projectile.dx,
                        projectile.dy,
                        projectile.damage,
                        projectile.lifetime,
                    )
                })
                .collect(),
            campaign: state.campaign.clone(),
        }
    }
}

impl ReplayData {
    pub(crate) fn with_modes(
        seed: u64,
        game_mode: GameMode,
        challenge_mode: ChallengeMode,
        rules_version: u8,
    ) -> Self {
        Self {
            seed,
            format_version: REPLAY_FORMAT_VERSION,
            game_mode,
            challenge_mode,
            rules_version,
            actions: Vec::new(),
            expected: None,
        }
    }

    pub fn record(&mut self, turn: u32, action: Action) {
        self.record_repr(turn, action.into());
    }

    pub(crate) fn record_repr(&mut self, turn: u32, action: ActionRepr) {
        self.actions.push(ActionRecord { turn, action });
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.format_version > REPLAY_FORMAT_VERSION {
            return Err(format!(
                "replay format version {} is newer than this build (max {})",
                self.format_version, REPLAY_FORMAT_VERSION
            ));
        }
        if self.format_version < REPLAY_FORMAT_VERSION {
            return Err(format!(
                "replay format version {} is from an older world format; start a new run (current {})",
                self.format_version, REPLAY_FORMAT_VERSION
            ));
        }
        if self.rules_version > crate::app::CURRENT_RULES_VERSION {
            return Err(format!(
                "replay rules version {} is newer than this build (max {})",
                self.rules_version,
                crate::app::CURRENT_RULES_VERSION
            ));
        }
        if self.actions.len() > MAX_REPLAY_ACTIONS {
            return Err(format!(
                "replay has {} actions; maximum is {MAX_REPLAY_ACTIONS}",
                self.actions.len()
            ));
        }
        if let Some(expected) = &self.expected {
            expected.validate()?;
        }
        Ok(())
    }

    pub fn save_to_file(&self, path: &str) -> std::io::Result<()> {
        self.validate().map_err(std::io::Error::other)?;
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)
    }
}

pub(crate) fn playback(data: &ReplayData) -> Result<GameState, String> {
    data.validate()?;
    let mut state = GameState::new(data.seed);
    state.game_mode = data.game_mode;
    state.challenge_mode = data.challenge_mode;
    state.rules_version = data.rules_version;
    if data.game_mode == GameMode::Campaign {
        state.campaign = Some(CampaignState::new());
    }
    state.new_game_with_rules(data.rules_version);
    state.replay_data = None;
    let mut visual = VisualState::new(state.map.width, state.map.height);

    for record in &data.actions {
        if record.turn != state.stats.turns {
            return Err(format!(
                "replay diverged before action: file turn {}, simulated turn {}",
                record.turn, state.stats.turns
            ));
        }
        apply_action(&mut state, record.action, &mut visual)?;
    }

    if let Some(expected) = &data.expected {
        let mut actual = ReplayOutcome::from_state(&state);
        if data.rules_version == 0 {
            // Outcomes written before tactical rules v1 did not contain these
            // fields. Keep their historical verification contract intact.
            actual.stamina = expected.stamina;
            actual.evasion_turns = expected.evasion_turns;
            actual.counter_turns = expected.counter_turns;
            actual.rules_version = expected.rules_version;
        }
        if data.rules_version < 2 {
            // Build fields were introduced with rules v2. Older expected
            // outcomes keep their pre-v2 verification shape.
            actual.max_stamina = expected.max_stamina;
            actual.vigor_rank = expected.vigor_rank;
            actual.finesse_rank = expected.finesse_rank;
            actual.resolve_rank = expected.resolve_rank;
            actual.pending_level_ups = expected.pending_level_ups;
        }
        if data.rules_version < 3 {
            // Masteries and explicit threat intents are part of v3's
            // presentation contract; legacy replays never recorded them.
            actual.weapon_mastery = expected.weapon_mastery;
            for (actual_monster, expected_monster) in
                actual.monsters.iter_mut().zip(&expected.monsters)
            {
                actual_monster.intent = expected_monster.intent;
            }
        }
        if &actual != expected {
            return Err(format!(
                "replay final state diverged: expected {:?}, got {:?}",
                expected, actual
            ));
        }
    }
    Ok(state)
}

fn apply_action(
    state: &mut GameState,
    action: ActionRepr,
    visual: &mut VisualState,
) -> Result<(), String> {
    match action {
        ActionRepr::UseItem(slot) => {
            let kind = state
                .player
                .inventory
                .get(slot)
                .copied()
                .ok_or_else(|| format!("replay item slot {slot} is out of range"))?;
            if state.use_item(kind, slot, visual) {
                state.end_turn(visual);
            }
        }
        ActionRepr::PreparationChoice { menu, choice } => {
            state.preparation_menu = Some(menu);
            state.resolve_preparation(choice, visual);
        }
        ActionRepr::DisciplineChoice(discipline) => {
            state.resolve_discipline_choice(discipline);
        }
        ActionRepr::InteractEnvironment(dx, dy) => state.interact_facing(dx, dy, visual),
        ActionRepr::TogglePickup => state.auto_pickup = !state.auto_pickup,
        ActionRepr::SaveGame
        | ActionRepr::LoadGame
        | ActionRepr::AutoExplore
        | ActionRepr::None => {}
        other => {
            let action = match other {
                ActionRepr::Move(dx, dy) => Action::Move(dx, dy),
                ActionRepr::Dodge(dx, dy) => Action::Dodge(dx, dy),
                ActionRepr::Technique(dx, dy) => Action::Technique(dx, dy),
                ActionRepr::Wait => Action::Wait,
                ActionRepr::PickUp => Action::PickUp,
                ActionRepr::Descend => Action::Descend,
                ActionRepr::Ascend => Action::Ascend,
                ActionRepr::Talk => Action::Talk,
                ActionRepr::TalkAt(x, y) => Action::TalkAt(x, y),
                ActionRepr::ConversationChoice(choice) => Action::ConversationChoice(choice),
                ActionRepr::ToggleDoor(dx, dy) => Action::ToggleDoor(dx, dy),
                ActionRepr::Listen(dx, dy) => Action::Listen(dx, dy),
                ActionRepr::Disarm(dx, dy) => Action::Disarm(dx, dy),
                _ => unreachable!("non-gameplay replay actions handled above"),
            };
            state.process_turn(action, visual);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recorded_actions_reproduce_the_expected_outcome() {
        let mut state = GameState::new(42);
        state.new_game();
        let mut visual = VisualState::new(state.map.width, state.map.height);
        state.process_turn(Action::Wait, &mut visual);
        state.process_turn(Action::Wait, &mut visual);

        let expected = ReplayOutcome::from_state(&state);
        let mut replay = state.replay_data.take().unwrap();
        replay.expected = Some(expected.clone());

        let replayed = playback(&replay).unwrap();
        assert_eq!(ReplayOutcome::from_state(&replayed), expected);
    }

    #[test]
    fn playback_rejects_a_wrong_turn_index() {
        let mut replay = ReplayData::with_modes(
            42,
            GameMode::Classic,
            ChallengeMode::Standard,
            crate::app::CURRENT_RULES_VERSION,
        );
        replay.actions.push(ActionRecord {
            turn: 3,
            action: ActionRepr::Wait,
        });

        let error = match playback(&replay) {
            Ok(_) => panic!("wrong turn index should fail playback"),
            Err(error) => error,
        };
        assert!(error.contains("file turn 3, simulated turn 0"));
    }

    #[test]
    fn playback_rejects_rules_from_a_newer_build() {
        let replay = ReplayData::with_modes(
            42,
            GameMode::Classic,
            ChallengeMode::Standard,
            crate::app::CURRENT_RULES_VERSION + 1,
        );

        let error = match playback(&replay) {
            Ok(_) => panic!("future rules should not be replayed"),
            Err(error) => error,
        };
        assert!(error.contains("newer than this build"));
    }

    #[test]
    fn playback_rejects_a_replay_from_an_older_world_format() {
        let mut replay = ReplayData::with_modes(
            42,
            GameMode::Classic,
            ChallengeMode::Standard,
            crate::app::CURRENT_RULES_VERSION,
        );
        replay.format_version = REPLAY_FORMAT_VERSION - 1;

        let error = match playback(&replay) {
            Ok(_) => panic!("old world replays should not be played"),
            Err(error) => error,
        };
        assert!(error.contains("older world format"));
        assert!(error.contains("start a new run"));
    }

    #[test]
    fn replay_format_version_round_trips_through_json() {
        let replay = ReplayData::with_modes(
            42,
            GameMode::Classic,
            ChallengeMode::Standard,
            crate::app::CURRENT_RULES_VERSION,
        );
        let json = serde_json::to_string(&replay).expect("replay should serialize");
        let decoded: ReplayData = serde_json::from_str(&json).expect("replay should deserialize");

        assert_eq!(decoded.format_version, REPLAY_FORMAT_VERSION);
    }

    #[test]
    fn replay_outcome_round_trips_world_map_metadata() {
        let mut state = GameState::new(42);
        state.new_game();
        state.map.theme = MapTheme::Road;
        state.map.set_surface(1, 1, SurfaceKind::Mud);
        state
            .map
            .props
            .push(EnvironmentProp::new(2, 2, crate::map::PropKind::Brazier));

        let outcome = ReplayOutcome::from_state(&state);
        let json = serde_json::to_string(&outcome).expect("outcome should serialize");
        let decoded: ReplayOutcome =
            serde_json::from_str(&json).expect("outcome should deserialize");

        assert_eq!(decoded.map_theme, MapTheme::Road);
        assert_eq!(decoded.map_surfaces, state.map.surfaces);
        assert_eq!(decoded.map_props, state.map.props);
        assert!(decoded.validate().is_ok());
    }

    #[test]
    fn replay_rejects_a_malformed_world_map_snapshot() {
        let mut state = GameState::new(42);
        state.new_game();
        let mut replay = ReplayData::with_modes(
            42,
            GameMode::Classic,
            ChallengeMode::Standard,
            crate::app::CURRENT_RULES_VERSION,
        );
        let mut outcome = ReplayOutcome::from_state(&state);
        outcome.map_surfaces = Some(Vec::new());
        replay.expected = Some(outcome);

        let error = match playback(&replay) {
            Ok(_) => panic!("invalid map snapshots must fail"),
            Err(error) => error,
        };
        assert!(error.contains("surface length mismatch"));
    }

    #[test]
    fn legacy_replay_defaults_to_standard_classic_mode() {
        let replay: ReplayData = serde_json::from_str(r#"{"seed":42,"actions":[]}"#).unwrap();

        assert_eq!(replay.game_mode, GameMode::Classic);
        assert_eq!(replay.challenge_mode, ChallengeMode::Standard);
        assert_eq!(replay.format_version, 0);
        assert_eq!(replay.rules_version, 0);
        assert!(replay.expected.is_none());
    }

    #[test]
    fn tactical_resources_participate_in_replay_verification() {
        let mut state = GameState::new(42);
        state.new_game();
        let mut outcome = ReplayOutcome::from_state(&state);

        outcome.stamina = outcome.stamina.saturating_sub(1);
        assert_ne!(ReplayOutcome::from_state(&state), outcome);
    }

    #[test]
    fn replay_applies_discipline_choice_without_advancing_time() {
        let mut state = GameState::new(42);
        state.new_game();
        state.replay_data = None;
        state.player.pending_level_ups = 1;
        let mut visual = VisualState::new(state.map.width, state.map.height);

        apply_action(
            &mut state,
            ActionRepr::DisciplineChoice(Discipline::Resolve),
            &mut visual,
        )
        .unwrap();

        assert_eq!(state.stats.turns, 0);
        assert_eq!(state.player.pending_level_ups, 0);
        assert_eq!(state.player.resolve_rank, 1);
    }
}
