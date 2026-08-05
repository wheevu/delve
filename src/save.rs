// ── Save data management ──
// Visual effects and decals are transient and reset on load.
// Coordinate and bounds validation ensures saved state can be safely loaded.

use crate::app::{ChallengeMode, GameMode, GameState, GameStatus, RunStats};
use crate::campaign::CampaignState;
use crate::entity::{Camp, Item, Landmark, Monster, MonsterKind, Npc, Player, Projectile};
use crate::floor::{FloorMemory, FloorState, MAX_CACHED_FLOORS};
use crate::map::Map;
use crate::ui;

/// Save format containing the medieval world layout and map contract.
pub(crate) const SAVE_VERSION: u32 = 15;
pub(crate) const SAVE_PATH: &str = "delve_save.json";
const MAX_MAP_PROPS: usize = 4_096;

pub(crate) const fn rules_version_for_save(version: u32) -> u8 {
    match version {
        0..=10 => 0,
        11 => 1,
        12..=13 => 2,
        _ => 3,
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct SaveData {
    pub version: u32,
    pub seed: u64,
    pub player: Player,
    pub map: Map,
    pub monsters: Vec<Monster>,
    pub items: Vec<Item>,
    #[serde(default)]
    pub npcs: Vec<Npc>,
    #[serde(default)]
    pub projectiles: Vec<Projectile>,
    pub messages: Vec<ui::Message>,
    pub status: GameStatus,
    pub stats: RunStats,
    #[serde(default)]
    pub auto_pickup: bool,
    #[serde(default)]
    pub game_mode: GameMode,
    #[serde(default)]
    pub challenge_mode: ChallengeMode,
    #[serde(default)]
    pub campaign: Option<CampaignState>,
    /// Landmarks on the current floor (required for BellChamber interaction).
    #[serde(default)]
    pub landmarks: Vec<(usize, Landmark)>,
    /// Landmark room indices already consumed on this floor.
    #[serde(default)]
    pub used_landmarks: Vec<usize>,
    /// Camps placed on the current floor.
    #[serde(default)]
    pub camps: Vec<Camp>,
    /// Monsters identified during this run, used by the bestiary overlay.
    #[serde(default)]
    pub seen_monsters: Vec<MonsterKind>,
    /// Monsters defeated at least once, unlocking their full behavior notes.
    #[serde(default)]
    pub defeated_monsters: Vec<MonsterKind>,
    /// Visited classic floors that should be restored on the return journey.
    #[serde(default)]
    pub floor_cache: Vec<FloorState>,
    /// Memory for the active floor.
    #[serde(default)]
    pub floor_memory: FloorMemory,
    /// Exact ChaCha stream position. Older saves fall back to their historical
    /// turn-based restoration behavior when this field is absent.
    #[serde(default)]
    pub rng_word_pos: Option<u128>,
}

impl SaveData {
    pub fn from_state(state: &GameState) -> Self {
        let mut used_landmarks: Vec<usize> = state.used_landmarks.iter().copied().collect();
        used_landmarks.sort_unstable();
        let mut seen_monsters: Vec<MonsterKind> = state.seen_monsters.iter().copied().collect();
        seen_monsters.sort_by_key(|kind| kind.name());
        let mut defeated_monsters: Vec<MonsterKind> =
            state.defeated_monsters.iter().copied().collect();
        defeated_monsters.sort_by_key(|kind| kind.name());
        Self {
            // Re-saving an old run must not silently opt it into new combat
            // rules. Starting a new run is the explicit upgrade boundary.
            version: match state.rules_version {
                0 => 10,
                1 => 11,
                2 => 13,
                _ => SAVE_VERSION,
            },
            seed: state.seed,
            player: state.player.clone(),
            map: state.map.clone(),
            monsters: state.monsters.clone(),
            items: state.items.clone(),
            npcs: state.npcs.clone(),
            projectiles: state.projectiles.clone(),
            messages: state.messages.clone_messages(),
            status: state.status,
            stats: state.stats.clone(),
            auto_pickup: state.auto_pickup,
            game_mode: state.game_mode,
            challenge_mode: state.challenge_mode,
            campaign: state.campaign.clone(),
            landmarks: state.landmarks.clone(),
            used_landmarks,
            camps: state.camps.clone(),
            seen_monsters,
            defeated_monsters,
            floor_cache: state.floor_cache.clone(),
            floor_memory: state.floor_memory.clone(),
            rng_word_pos: Some(state.rng.get_word_pos()),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version > SAVE_VERSION {
            return Err(format!(
                "Save version {} is newer than this build (max {})",
                self.version, SAVE_VERSION
            ));
        }
        if self.version < SAVE_VERSION {
            return Err(format!(
                "Save version {} is from an older world format; start a new run (current {})",
                self.version, SAVE_VERSION
            ));
        }
        let w = self.map.width;
        let h = self.map.height;
        // Cap map dimensions to prevent huge allocations.
        const MAX_MAP_DIM: i32 = 256;
        if w <= 0 || h <= 0 || w > MAX_MAP_DIM || h > MAX_MAP_DIM {
            return Err("Save corrupted: invalid map dimensions".into());
        }
        let expected_size = (w * h) as usize;
        if self.map.tiles.len() != expected_size {
            return Err("Save corrupted: map tile count mismatch".into());
        }
        if self.map.visible.len() != expected_size {
            return Err("Save corrupted: map visible length mismatch".into());
        }
        if self.map.explored.len() != expected_size {
            return Err("Save corrupted: map explored length mismatch".into());
        }
        Self::validate_map_metadata(&self.map, "active")?;

        if self.floor_cache.len() > MAX_CACHED_FLOORS {
            return Err("Save corrupted: too many cached floors".into());
        }
        let mut cached_depths = std::collections::HashSet::new();
        for floor in &self.floor_cache {
            if floor.depth > 4 || !cached_depths.insert(floor.depth) {
                return Err("Save corrupted: invalid or duplicate cached floor depth".into());
            }
            Self::validate_floor_snapshot(floor)?;
        }
        let requires_v3_memory = rules_version_for_save(self.version) >= 3;
        if self.floor_memory.attention > 100
            || self.floor_memory.room_alert.iter().any(|alert| *alert > 3)
            || (!self.floor_memory.room_alert.is_empty()
                && self.floor_memory.room_alert.len() != self.map.rooms.len())
            || (requires_v3_memory && self.floor_memory.room_alert.len() != self.map.rooms.len())
        {
            return Err("Save corrupted: invalid active floor memory".into());
        }
        if self
            .floor_memory
            .last_noise
            .is_some_and(|noise| noise.x < 0 || noise.x >= w || noise.y < 0 || noise.y >= h)
        {
            return Err("Save corrupted: active floor noise marker out of bounds".into());
        }

        // Player bounds
        let in_bounds = |x: i32, y: i32| -> bool { x >= 0 && x < w && y >= 0 && y < h };
        if !in_bounds(self.player.x, self.player.y) {
            return Err("Save corrupted: player out of bounds".into());
        }
        if self.player.stats.max_hp <= 0 {
            return Err("Save corrupted: player has invalid max HP".into());
        }
        if self.player.stats.hp < 0 || self.player.stats.hp > self.player.stats.max_hp {
            return Err("Save corrupted: player HP out of range".into());
        }
        if self.player.poison_ticks < 0 {
            return Err("Save corrupted: player poison ticks negative".into());
        }
        if self.player.lantern_turns < 0 {
            return Err("Save corrupted: player lantern turns negative".into());
        }
        if self.player.focus_turns < 0 || self.player.ward_turns < 0 {
            return Err("Save corrupted: player preparation turns negative".into());
        }
        if !(crate::entity::MAX_STAMINA..=crate::entity::MAX_STAMINA_CAP)
            .contains(&self.player.max_stamina)
            || self.player.stamina > self.player.max_stamina
        {
            return Err("Save corrupted: player stamina out of range".into());
        }
        if self.player.evasion_turns > 1 {
            return Err("Save corrupted: player evasion state out of range".into());
        }
        if self.player.counter_turns > 2 {
            return Err("Save corrupted: player counter state out of range".into());
        }
        if self
            .player
            .weapon_mastery
            .is_some_and(|mastery| !mastery.compatible_with(self.player.weapon))
        {
            return Err("Save corrupted: weapon mastery does not match equipped weapon".into());
        }

        // Monster bounds
        if self.monsters.len() > 200 {
            return Err("Save corrupted: too many monsters".into());
        }
        for (i, m) in self.monsters.iter().enumerate() {
            if !in_bounds(m.x, m.y) {
                return Err(format!("Save corrupted: monster {i} out of bounds"));
            }
            if m.stats.max_hp <= 0 {
                return Err(format!("Save corrupted: monster {i} has invalid max HP"));
            }
            if let Some(intent) = m.intent {
                if !in_bounds(intent.target_x, intent.target_y) {
                    return Err(format!(
                        "Save corrupted: monster {i} intent target out of bounds"
                    ));
                }
            }
        }

        // Item bounds
        if self.items.len() > 200 {
            return Err("Save corrupted: too many items".into());
        }
        for (i, it) in self.items.iter().enumerate() {
            if !in_bounds(it.x, it.y) {
                return Err(format!("Save corrupted: item {i} out of bounds"));
            }
        }

        // Projectile bounds
        if self.projectiles.len() > 100 {
            return Err("Save corrupted: too many projectiles".into());
        }
        for (i, p) in self.projectiles.iter().enumerate() {
            if !in_bounds(p.x, p.y) {
                return Err(format!("Save corrupted: projectile {i} out of bounds"));
            }
        }

        // NPC bounds
        if self.npcs.len() > 50 {
            return Err("Save corrupted: too many NPCs".into());
        }
        for (i, npc) in self.npcs.iter().enumerate() {
            if !in_bounds(npc.x, npc.y) {
                return Err(format!("Save corrupted: NPC {i} out of bounds"));
            }
        }

        // Room bounds
        for (i, room) in self.map.rooms.iter().enumerate() {
            if !Self::room_in_bounds(room, w, h) {
                return Err(format!("Save corrupted: room {i} out of bounds"));
            }
        }

        // Landmark room indices
        for &(ri, _) in &self.landmarks {
            if ri >= self.map.rooms.len() {
                return Err(format!(
                    "Save corrupted: landmark room index {ri} out of range ({} rooms)",
                    self.map.rooms.len()
                ));
            }
        }
        if self
            .used_landmarks
            .iter()
            .any(|room_index| *room_index >= self.map.rooms.len())
        {
            return Err("Save corrupted: used landmark room index out of range".into());
        }

        // Camp indices
        for (ci, camp) in self.camps.iter().enumerate() {
            if camp.room_idx >= self.map.rooms.len() {
                return Err(format!(
                    "Save corrupted: camp {ci} room index {} out of range",
                    camp.room_idx
                ));
            }
            for &npc_idx in &camp.npc_indices {
                if npc_idx >= self.npcs.len() {
                    return Err(format!(
                        "Save corrupted: camp {ci} npc index {npc_idx} out of range"
                    ));
                }
            }
        }

        // Campaign consistency
        if self.game_mode == GameMode::Campaign && self.campaign.is_none() {
            return Err("Save corrupted: campaign mode without campaign state".into());
        }
        if self.status == GameStatus::CampaignComplete {
            let Some(campaign) = self.campaign.as_ref() else {
                return Err("Save corrupted: completed campaign without campaign state".into());
            };
            if campaign.chapter != crate::campaign::CampaignChapter::Chapter1 || campaign.level < 3
            {
                return Err("Save corrupted: campaign ended before Chapter I was complete".into());
            }
        }

        if self.messages.len() > 500 {
            return Err("Save corrupted: too many messages".into());
        }
        Ok(())
    }

    fn validate_floor_snapshot(floor: &FloorState) -> Result<(), String> {
        let w = floor.map.width;
        let h = floor.map.height;
        const MAX_MAP_DIM: i32 = 256;
        if w <= 0 || h <= 0 || w > MAX_MAP_DIM || h > MAX_MAP_DIM {
            return Err("Save corrupted: invalid cached floor dimensions".into());
        }
        let expected_size = (w * h) as usize;
        if floor.map.tiles.len() != expected_size
            || floor.map.visible.len() != expected_size
            || floor.map.explored.len() != expected_size
        {
            return Err("Save corrupted: cached floor tile data mismatch".into());
        }
        Self::validate_map_metadata(&floor.map, "cached floor")?;
        if floor.memory.attention > 100
            || floor.memory.room_alert.len() != floor.map.rooms.len()
            || floor.memory.room_alert.iter().any(|alert| *alert > 3)
        {
            return Err("Save corrupted: invalid cached floor memory".into());
        }
        if floor
            .memory
            .last_noise
            .is_some_and(|noise| noise.x < 0 || noise.x >= w || noise.y < 0 || noise.y >= h)
        {
            return Err("Save corrupted: cached floor noise marker out of bounds".into());
        }
        let in_bounds = |x: i32, y: i32| x >= 0 && x < w && y >= 0 && y < h;
        if floor.monsters.len() > 200
            || floor.items.len() > 200
            || floor.npcs.len() > 50
            || floor.projectiles.len() > 100
        {
            return Err("Save corrupted: too many cached floor entities".into());
        }
        if floor
            .monsters
            .iter()
            .any(|monster| !in_bounds(monster.x, monster.y) || monster.stats.max_hp <= 0)
            || floor.items.iter().any(|item| !in_bounds(item.x, item.y))
            || floor.npcs.iter().any(|npc| !in_bounds(npc.x, npc.y))
            || floor
                .projectiles
                .iter()
                .any(|projectile| !in_bounds(projectile.x, projectile.y))
        {
            return Err("Save corrupted: cached floor entity out of bounds".into());
        }
        if floor.monsters.iter().any(|monster| {
            monster
                .intent
                .is_some_and(|intent| !in_bounds(intent.target_x, intent.target_y))
        }) {
            return Err("Save corrupted: cached floor intent target out of bounds".into());
        }
        for room in &floor.map.rooms {
            if !Self::room_in_bounds(room, w, h) {
                return Err("Save corrupted: cached floor room out of bounds".into());
            }
        }
        if floor
            .landmarks
            .iter()
            .any(|(room_index, _)| *room_index >= floor.map.rooms.len())
            || floor
                .used_landmarks
                .iter()
                .any(|room_index| *room_index >= floor.map.rooms.len())
        {
            return Err("Save corrupted: cached floor landmark index out of range".into());
        }
        for camp in &floor.camps {
            if camp.room_idx >= floor.map.rooms.len()
                || camp
                    .npc_indices
                    .iter()
                    .any(|npc_index| *npc_index >= floor.npcs.len())
            {
                return Err("Save corrupted: cached floor camp index out of range".into());
            }
        }
        Ok(())
    }

    fn room_in_bounds(room: &crate::map::Rect, width: i32, height: i32) -> bool {
        room.x >= 0
            && room.y >= 0
            && room.w > 0
            && room.h > 0
            && room.x <= width.saturating_sub(room.w)
            && room.y <= height.saturating_sub(room.h)
    }

    fn validate_map_metadata(map: &Map, label: &str) -> Result<(), String> {
        let expected_size = (map.width * map.height) as usize;
        if let Some(surfaces) = &map.surfaces {
            if surfaces.len() != expected_size {
                return Err(format!(
                    "Save corrupted: {label} map surface length mismatch"
                ));
            }
        }
        if map.props.len() > MAX_MAP_PROPS {
            return Err(format!(
                "Save corrupted: {label} map has too many environment props"
            ));
        }
        for (index, prop) in map.props.iter().enumerate() {
            if prop.x < 0 || prop.x >= map.width || prop.y < 0 || prop.y >= map.height {
                return Err(format!(
                    "Save corrupted: {label} environment prop {index} out of bounds"
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_runs_write_the_new_world_save_version() {
        let mut state = GameState::new(42);
        state.new_game();

        assert_eq!(SaveData::from_state(&state).version, SAVE_VERSION);
    }

    #[test]
    fn older_world_saves_are_rejected_without_migration() {
        let mut state = GameState::new(42);
        state.new_game();
        let mut save = SaveData::from_state(&state);
        save.version = SAVE_VERSION - 1;

        let error = save
            .validate()
            .expect_err("old world saves must be rejected");
        assert!(error.contains("older world format"));
        assert!(error.contains("start a new run"));
    }

    #[test]
    fn world_map_metadata_round_trips_and_validates_in_saves() {
        let mut state = GameState::new(42);
        state.new_game();
        state.map.theme = crate::map::MapTheme::Road;
        state.map.set_surface(1, 1, crate::map::SurfaceKind::Mud);
        state.map.props.push(crate::map::EnvironmentProp::new(
            2,
            2,
            crate::map::PropKind::Brazier,
        ));

        let save = SaveData::from_state(&state);
        assert!(save.validate().is_ok());
        let json = serde_json::to_string(&save).expect("save should serialize");
        let loaded: SaveData = serde_json::from_str(&json).expect("save should deserialize");

        assert_eq!(loaded.map.theme, crate::map::MapTheme::Road);
        assert_eq!(
            loaded.map.surface_at(1, 1),
            Some(crate::map::SurfaceKind::Mud)
        );
        assert_eq!(loaded.map.props_at(2, 2).count(), 1);
    }

    #[test]
    fn malformed_world_map_metadata_is_rejected() {
        let mut state = GameState::new(42);
        state.new_game();
        let mut save = SaveData::from_state(&state);
        save.map.surfaces = Some(Vec::new());

        let error = save
            .validate()
            .expect_err("invalid surface metadata must fail");
        assert!(error.contains("surface length mismatch"));

        let mut save = SaveData::from_state(&state);
        save.map.props.clear();
        save.map.props.push(crate::map::EnvironmentProp::new(
            -1,
            0,
            crate::map::PropKind::Barrel,
        ));
        let error = save
            .validate()
            .expect_err("invalid prop metadata must fail");
        assert!(error.contains("environment prop 0 out of bounds"));
    }
}
