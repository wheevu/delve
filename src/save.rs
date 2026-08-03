// ── Save data management ──
// Visual effects and decals are transient and reset on load.
// Coordinate and bounds validation ensures saved state can be safely loaded.

use crate::app::{ChallengeMode, GameMode, GameState, GameStatus, RunStats};
use crate::campaign::CampaignState;
use crate::entity::{Camp, Item, Landmark, Monster, MonsterKind, Npc, Player, Projectile};
use crate::map::Map;
use crate::ui;

pub(crate) const SAVE_VERSION: u32 = 13;
pub(crate) const SAVE_PATH: &str = "delve_save.json";

pub(crate) const fn rules_version_for_save(version: u32) -> u8 {
    match version {
        0..=10 => 0,
        11 => 1,
        _ => 2,
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
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version > SAVE_VERSION {
            return Err(format!(
                "Save version {} is newer than this build (max {})",
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
            if room.x < 0
                || room.y < 0
                || room.x + room.w > w
                || room.y + room.h > h
                || room.w <= 0
                || room.h <= 0
            {
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
}
