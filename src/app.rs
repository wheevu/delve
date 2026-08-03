use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use ratatui::DefaultTerminal;
use std::time::Duration;

use crate::campaign::{self, CampaignObjective, CampaignState};
use crate::combat;
use crate::entity::{
    Camp, Discipline, Item, ItemKind, Landmark, Monster, MonsterKind, Npc, NpcKind, Player,
    Projectile, TechniqueKind,
};
use crate::fov;
use crate::input::{self, Action, InventoryAction};
use crate::inventory;
use crate::logging::GameEvent;
use crate::map::{Map, Tile};
use crate::monster_ai;
use crate::narrative;
use crate::pathfinding;
use crate::render;
use crate::settings::UiSettings;
use crate::spawn;
use crate::ui::{MessageLog, MsgColor};
use crate::visual::{
    self, PlayerActionKind, PlayerStepKind, PlayerTurnKind, VisualEffect, VisualState,
};

const MAP_WIDTH: i32 = 80;
const MAP_HEIGHT: i32 = 50;
const FOV_RADIUS: i32 = 8;
const MAX_DEPTH: u32 = 4;
pub(crate) const CURRENT_RULES_VERSION: u8 = 2;
const TICK_MS: u64 = 33;
pub(crate) const HINT_INTERACT: u8 = 1;
pub(crate) const HINT_NAVIGATION: u8 = 1 << 1;
pub(crate) const HINT_MAP: u8 = 1 << 2;
pub(crate) const HINT_INVENTORY: u8 = 1 << 3;

macro_rules! draw {
    ($self:expr) => {
        $self.terminal.draw(|f| {
            let (camera_bob, camera_sway) = if $self.head_bob && !$self.low_motion {
                $self
                    .visual
                    .player_action
                    .map(|action| action.camera_motion())
                    .unwrap_or((0, 0))
            } else {
                (0, 0)
            };
            let ctx = render::RenderCtx {
                map: &$self.state.map,
                player: &$self.state.player,
                monsters: &$self.state.monsters,
                items: &$self.state.items,
                npcs: &$self.state.npcs,
                projectiles: &$self.state.projectiles,
                landmarks: &$self.state.landmarks,
                used_landmarks: &$self.state.used_landmarks,
                seen_monsters: &$self.state.seen_monsters,
                defeated_monsters: &$self.state.defeated_monsters,
                messages: &$self.state.messages,
                status: $self.state.status,
                stats: &$self.state.stats,
                seed: $self.state.seed,
                challenge_mode: $self.state.challenge_mode,
                completed_runs: $self.run_history.len(),
                run_history: &$self.run_history,
                inventory_cursor: $self.state.inventory_cursor,
                auto_pickup: $self.state.auto_pickup,
                log_scroll: $self.state.log_scroll,
                visual: &$self.visual,
                tick: $self.visual.tick,
                dialog_npc_name: &$self.state.dialog_npc_name,
                dialog_text: &$self.state.dialog_text,
                dialog_show_choices: $self.state.dialog_show_choices,
                dialog_choices: &$self.state.dialog_choices,
                camps: &$self.state.camps,
                game_mode: $self.state.game_mode,
                campaign_objectives: $self
                    .state
                    .campaign
                    .as_ref()
                    .map(|c| c.objectives.as_slice())
                    .unwrap_or(&[]),
                campaign_flags: $self
                    .state
                    .campaign
                    .as_ref()
                    .map(|c| c.flags.as_slice())
                    .unwrap_or(&[]),
                campaign_level_name: $self
                    .state
                    .campaign
                    .as_ref()
                    .and_then(|c| crate::campaign::chapter1_level_spec(c.level).map(|s| s.name))
                    .unwrap_or(""),
                show_minimap: $self.show_minimap,
                show_help: $self.show_help,
                show_bestiary: $self.show_bestiary,
                auto_exploring: $self.auto_exploring,
                reduced_flash: $self.reduced_flash,
                high_contrast: $self.high_contrast,
                show_log_overlay: $self.show_log_overlay,
                color_filter: $self.color_filter,
                player_moving: $self.camera_bob_timer > 0,
                camera_bob,
                camera_sway,
                low_motion: $self.low_motion,
                preparation_menu: $self.state.preparation_menu,
                hints_seen: $self.hints_seen,
                camera_angle: $self.camera_angle,
            };
            render::render(f, &ctx);
        })?
    };
}

// ── Game mode ──
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GameMode {
    #[default]
    Classic,
    Campaign,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ChallengeMode {
    #[default]
    Standard,
    NoMinimap,
    DeepFog,
    FragileGear,
    Relentless,
}

impl ChallengeMode {
    pub(crate) fn next(self) -> Self {
        match self {
            Self::Standard => Self::NoMinimap,
            Self::NoMinimap => Self::DeepFog,
            Self::DeepFog => Self::FragileGear,
            Self::FragileGear => Self::Relentless,
            Self::Relentless => Self::Standard,
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Standard => "Standard",
            Self::NoMinimap => "Lost Cartographer",
            Self::DeepFog => "Dying Light",
            Self::FragileGear => "Rust Oath",
            Self::Relentless => "The Hunt",
        }
    }

    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::Standard => "the intended descent",
            Self::NoMinimap => "the minimap is unavailable",
            Self::DeepFog => "sight is reduced by two tiles",
            Self::FragileGear => "weapons wear twice as fast",
            Self::Relentless => "mobile monsters act every turn",
        }
    }

    pub(crate) fn allows_minimap(self) -> bool {
        self != Self::NoMinimap
    }
}

pub(crate) fn toggled_map_visibility(current: bool, challenge: ChallengeMode) -> Option<bool> {
    challenge.allows_minimap().then_some(!current)
}

// ── Game status ──
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GameStatus {
    Menu,
    CampaignMenu,
    Playing,
    Inventory,
    QuestPanel,
    Dead,
    Won,
    /// Chapter I reached its authored ending. This is terminal for the run,
    /// but distinct from a Classic victory so the campaign does not spill into
    /// an unrelated procedural floor.
    CampaignComplete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum PreparationMenu {
    Camp(usize),
    Shrine(usize),
}

// ── Run stats ──
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RunStats {
    pub turns: u32,
    pub kills: u32,
    pub dmg_dealt: i32,
    pub dmg_taken: i32,
    pub items_used: u32,
    pub deepest_depth: u32,
    #[serde(default)]
    pub damage_guarded: i32,
    #[serde(default)]
    pub doors_forced: u32,
    #[serde(default)]
    pub traps_disarmed: u32,
    #[serde(default)]
    pub objects_smashed: u32,
    #[serde(default)]
    pub basic_attacks: u32,
    #[serde(default)]
    pub techniques_used: u32,
    #[serde(default)]
    pub dodges: u32,
    #[serde(default)]
    pub counters: u32,
    #[serde(default)]
    pub interrupts: u32,
    #[serde(default)]
    pub guard_breaks: u32,
    #[serde(default)]
    pub stamina_spent: u32,
}

impl RunStats {
    pub fn new() -> Self {
        Self {
            turns: 0,
            kills: 0,
            dmg_dealt: 0,
            dmg_taken: 0,
            items_used: 0,
            deepest_depth: 0,
            damage_guarded: 0,
            doors_forced: 0,
            traps_disarmed: 0,
            objects_smashed: 0,
            basic_attacks: 0,
            techniques_used: 0,
            dodges: 0,
            counters: 0,
            interrupts: 0,
            guard_breaks: 0,
            stamina_spent: 0,
        }
    }
}

// ── FOV dirty reasons ──
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FovDirty {
    Clean,
    PlayerMoved,
    MapChanged,
    RadiusChanged,
    LevelChanged,
}

// ── Headless game state ──
pub struct GameState {
    pub(crate) rng: StdRng,
    pub(crate) seed: u64,
    pub(crate) map: Map,
    pub(crate) player: Player,
    pub(crate) monsters: Vec<Monster>,
    pub(crate) items: Vec<Item>,
    pub(crate) npcs: Vec<Npc>,
    pub(crate) projectiles: Vec<Projectile>,
    pub(crate) landmarks: Vec<(usize, Landmark)>,
    /// Landmark room indices already interacted with on this floor.
    pub(crate) used_landmarks: std::collections::HashSet<usize>,
    pub(crate) messages: MessageLog,
    pub(crate) status: GameStatus,
    pub(crate) stats: RunStats,
    pub(crate) astar: pathfinding::AStarWorkspace,
    pub(crate) explore_visited: Vec<bool>,
    pub(crate) occupancy: Vec<Option<usize>>,
    pub(crate) fov_dirty: FovDirty,
    pub(crate) flow_dist: Vec<i32>,
    pub(crate) inventory_cursor: usize,
    pub(crate) auto_pickup: bool,
    /// Scroll offset for the message log (0 = newest messages visible).
    pub(crate) log_scroll: usize,
    pub(crate) dialog_active: bool,
    pub(crate) dialog_text: Vec<String>,
    pub(crate) dialog_npc_name: String,
    pub(crate) dialog_npc_idx: Option<usize>,
    /// Branching conversation state: current node index in the NPC's talk tree.
    pub(crate) dialog_conv_node: usize,
    /// Whether the dialog is currently showing player choices.
    pub(crate) dialog_show_choices: bool,
    /// The choice labels currently displayed to the player.
    pub(crate) dialog_choices: Vec<String>,
    /// In campaign mode: which named NPC we're talking to (for effect resolution).
    pub(crate) dialog_campaign_npc: Option<crate::campaign::CampaignNpc>,
    /// Camps on the current floor.
    pub(crate) camps: Vec<Camp>,
    pub(crate) seen_monsters: std::collections::HashSet<crate::entity::MonsterKind>,
    pub(crate) defeated_monsters: std::collections::HashSet<crate::entity::MonsterKind>,
    pub(crate) preparation_menu: Option<PreparationMenu>,
    /// Semantic action log used to verify a completed run deterministically.
    pub(crate) replay_data: Option<crate::replay::ReplayData>,
    /// Current game mode: Classic or Campaign.
    pub(crate) game_mode: GameMode,
    pub(crate) challenge_mode: ChallengeMode,
    pub(crate) rules_version: u8,
    /// Campaign state (active only in Campaign mode).
    pub(crate) campaign: Option<CampaignState>,
}

/// Choose a fixed fire direction (one of 8) toward the target.
pub(crate) fn projectile_direction(from_x: i32, from_y: i32, to_x: i32, to_y: i32) -> (i32, i32) {
    let dx = (to_x - from_x).signum();
    let dy = (to_y - from_y).signum();
    (dx, dy)
}

/// Rotate a direction vector 45° clockwise (for a spread shot).
pub(crate) fn rotate_direction_45(dx: i32, dy: i32) -> (i32, i32) {
    match (dx, dy) {
        (1, 0) => (1, 1),
        (1, 1) => (0, 1),
        (0, 1) => (-1, 1),
        (-1, 1) => (-1, 0),
        (-1, 0) => (-1, -1),
        (-1, -1) => (0, -1),
        (0, -1) => (1, -1),
        (1, -1) => (1, 0),
        _ => (dx, dy),
    }
}

pub(crate) fn crown_pursuit_act_chance(base: f64, depth: u32) -> f64 {
    let ascent_pressure = f64::from(MAX_DEPTH.saturating_sub(depth)) * 0.05;
    (base + 0.12 + ascent_pressure).min(1.0)
}

impl GameState {
    pub fn new(seed: u64) -> Self {
        let rng = StdRng::seed_from_u64(seed);
        Self {
            rng,
            seed,
            map: Map::new(MAP_WIDTH, MAP_HEIGHT),
            player: Player::new(0, 0),
            monsters: Vec::new(),
            items: Vec::new(),
            npcs: Vec::new(),
            projectiles: Vec::new(),
            landmarks: Vec::new(),
            used_landmarks: std::collections::HashSet::new(),
            messages: MessageLog::new(200),
            status: GameStatus::Menu,
            stats: RunStats::new(),
            astar: pathfinding::AStarWorkspace::new(MAP_WIDTH, MAP_HEIGHT),
            explore_visited: vec![false; (MAP_WIDTH * MAP_HEIGHT) as usize],
            occupancy: vec![None; (MAP_WIDTH * MAP_HEIGHT) as usize],
            fov_dirty: FovDirty::Clean,
            flow_dist: vec![i32::MAX; (MAP_WIDTH * MAP_HEIGHT) as usize],
            inventory_cursor: 0,
            auto_pickup: false,
            log_scroll: 0,
            dialog_active: false,
            dialog_text: Vec::new(),
            dialog_npc_name: String::new(),
            dialog_npc_idx: None,
            dialog_conv_node: 0,
            dialog_show_choices: false,
            dialog_choices: Vec::new(),
            dialog_campaign_npc: None,
            camps: Vec::new(),
            seen_monsters: std::collections::HashSet::new(),
            defeated_monsters: std::collections::HashSet::new(),
            preparation_menu: None,
            replay_data: None,
            game_mode: GameMode::Classic,
            challenge_mode: ChallengeMode::Standard,
            rules_version: CURRENT_RULES_VERSION,
            campaign: None,
        }
    }

    // ── Game lifecycle ──
    pub fn new_game(&mut self) {
        // A manually started run always uses the current rules. Replay playback
        // restores its recorded version after generation for compatibility.
        self.rules_version = CURRENT_RULES_VERSION;
        self.rng = StdRng::seed_from_u64(self.seed);
        self.player = Player::new(0, 0);
        self.monsters.clear();
        self.items.clear();
        self.npcs.clear();
        self.projectiles.clear();
        self.landmarks.clear();
        self.used_landmarks.clear();
        self.messages.clear();
        self.stats = RunStats::new();
        self.inventory_cursor = 0;
        self.auto_pickup = false;
        self.log_scroll = 0;
        self.dialog_active = false;
        self.dialog_text.clear();
        self.dialog_npc_name.clear();
        self.dialog_npc_idx = None;
        self.dialog_conv_node = 0;
        self.dialog_show_choices = false;
        self.dialog_choices.clear();
        self.dialog_campaign_npc = None;
        self.camps.clear();
        self.seen_monsters.clear();
        self.defeated_monsters.clear();
        self.preparation_menu = None;
        // Start replay recording
        self.replay_data = Some(crate::replay::ReplayData::with_modes(
            self.seed,
            self.game_mode,
            self.challenge_mode,
            self.rules_version,
        ));
        self.status = GameStatus::Playing;

        // Generate the first level
        let depth = if self.game_mode == GameMode::Campaign {
            // Campaign mode: generate at campaign level (mapped to depth biome)
            let camp_depth = self.campaign.as_ref().map(|c| c.level).unwrap_or(0);
            self.generate_campaign_level(camp_depth);
            return;
        } else {
            self.generate_level(0);
            0
        };
        self.msg(narrative::descent_line(depth as u32), MsgColor::SYSTEM);
        crate::logging::game_event(GameEvent::NewGame { seed: self.seed });
    }

    /// Generate a campaign-mode level with authored content.
    fn generate_campaign_level(&mut self, camp_level: u32) {
        // Map campaign level to dungeon depth for biome/monster scaling
        let spec = campaign::chapter1_level_spec(camp_level);
        let depth = spec.as_ref().map(|level| level.depth).unwrap_or(camp_level);

        self.map = Map::new(MAP_WIDTH, MAP_HEIGHT);
        let place_down = depth < MAX_DEPTH;
        self.map.generate(&mut self.rng, place_down);

        let room_count = self.map.rooms.len();

        // Find stairs positions
        let mut stairs_down_pos: Option<(i32, i32)> = None;
        let mut stairs_up_pos: Option<(i32, i32)> = None;
        for y in 0..self.map.height {
            for x in 0..self.map.width {
                let t = self.map.tiles[self.map.idx(x, y)];
                if t == Tile::StairsDown {
                    stairs_down_pos = Some((x, y));
                } else if t == Tile::StairsUp {
                    stairs_up_pos = Some((x, y));
                }
            }
        }

        // Pick player spawn room
        if room_count > 0 {
            let best_room = (0..room_count)
                .max_by(|&a, &b| {
                    let score_a =
                        spawn::room_stairs_dist(&self.map.rooms[a], stairs_down_pos, stairs_up_pos);
                    let score_b =
                        spawn::room_stairs_dist(&self.map.rooms[b], stairs_down_pos, stairs_up_pos);
                    score_a
                        .partial_cmp(&score_b)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap_or(0);
            let (cx, cy) = self.map.rooms[best_room].center();
            self.player.x = cx;
            self.player.y = cy;
        } else {
            self.player.x = MAP_WIDTH / 2;
            self.player.y = MAP_HEIGHT / 2;
        }
        self.player.depth = depth;
        self.stats.deepest_depth = self.stats.deepest_depth.max(depth);

        // Spawn monsters
        self.monsters.clear();
        let base_count = spec
            .as_ref()
            .and_then(|s| s.monster_count)
            .unwrap_or(3 + depth as usize * 2);
        let monster_count = base_count.min(room_count.saturating_sub(1));

        let player_room_idx = spawn::find_player_room(&self.map.rooms, &self.player);
        let mut spawn_rooms: Vec<usize> = (0..room_count)
            .filter(|&i| Some(i) != player_room_idx)
            .collect();
        if spawn_rooms.len() < monster_count {
            spawn_rooms = (0..room_count).collect();
        }
        for i in (1..spawn_rooms.len()).rev() {
            let j = self.rng.gen_range(0..=i);
            spawn_rooms.swap(i, j);
        }

        // Use campaign enemy mix if available
        if let Some(ref spec) = spec {
            let total_weight: u32 = spec.enemy_mix.iter().map(|(_, w)| *w).sum();
            for &room_idx in spawn_rooms
                .iter()
                .take(monster_count.min(spawn_rooms.len()))
            {
                let room = self.map.rooms[room_idx];
                for _ in 0..5 {
                    let (bx, by) = room.random_tile(&mut self.rng);
                    let idx = self.map.idx(bx, by);
                    if (bx != self.player.x || by != self.player.y)
                        && self.map.tiles[idx] == Tile::Floor
                        && !self
                            .monsters
                            .iter()
                            .any(|m| m.alive && m.x == bx && m.y == by)
                    {
                        // Weighted random from campaign enemy mix
                        let roll = self.rng.gen_range(0..total_weight);
                        let mut accum = 0;
                        let mut chosen_kind = spec.enemy_mix[0].0;
                        for &(kind, weight) in spec.enemy_mix {
                            accum += weight;
                            if roll < accum {
                                chosen_kind = kind;
                                break;
                            }
                        }
                        let base = chosen_kind.stats();
                        let scale = 1.0 + depth as f32 * 0.15;
                        let hp = (base.hp as f32 * scale).round() as i32;
                        let mut m = Monster::new(bx, by, chosen_kind);
                        m.stats.hp = hp;
                        m.stats.max_hp = hp;
                        self.monsters.push(m);
                        break;
                    }
                }
            }
        } else {
            // Fallback to default monster spawning
            for &room_idx in spawn_rooms
                .iter()
                .take(monster_count.min(spawn_rooms.len()))
            {
                let room = self.map.rooms[room_idx];
                spawn::spawn_one_monster(
                    &mut self.monsters,
                    &self.map,
                    &mut self.rng,
                    &room,
                    depth,
                    self.player.x,
                    self.player.y,
                );
            }
        }

        // Spawn items
        self.items.clear();
        let item_count = 2 + depth as usize;
        let mut occupied =
            spawn::build_occupied_set(&self.map, &self.player, &self.monsters, &self.items);
        for _ in 0..item_count {
            spawn::try_spawn_item(
                &mut self.items,
                &self.map,
                &mut self.rng,
                depth,
                &mut occupied,
            );
        }

        // Assign landmarks — use campaign-specified landmarks
        self.landmarks.clear();
        self.used_landmarks.clear();
        if let Some(ref spec) = spec {
            let mut available = player_room_idx
                .map(|p| (0..room_count).filter(move |&i| i != p).collect::<Vec<_>>())
                .unwrap_or_else(|| (0..room_count).collect());
            for i in (1..available.len()).rev() {
                let j = self.rng.gen_range(0..=i);
                available.swap(i, j);
            }
            for (li, &lm) in spec.landmarks.iter().enumerate() {
                if let Some(&ri) = available.get(li) {
                    self.landmarks.push((ri, lm));
                }
            }
        } else {
            spawn::assign_landmarks(
                &mut self.landmarks,
                &self.map.rooms,
                &mut self.rng,
                depth,
                player_room_idx,
            );
        }

        // Spawn named campaign NPCs
        self.npcs.clear();
        if let Some(ref spec) = spec {
            // Place named NPCs in separate rooms
            let used_rooms: Vec<usize> = self.landmarks.iter().map(|(ri, _)| *ri).collect();
            let mut npc_rooms: Vec<usize> = (0..room_count)
                .filter(|&i| Some(i) != player_room_idx && !used_rooms.contains(&i))
                .collect();
            for i in (1..npc_rooms.len()).rev() {
                let j = self.rng.gen_range(0..=i);
                npc_rooms.swap(i, j);
            }
            for (ni, &(camp_npc, _role)) in spec.named_npcs.iter().enumerate() {
                if let Some(&ri) = npc_rooms.get(ni) {
                    let (nx, ny) = self.map.rooms[ri].center();
                    self.npcs.push(Npc::new(nx, ny, camp_npc.npc_kind()));
                }
            }
        } else {
            // Fallback to default NPC spawning
            spawn::spawn_npcs(
                &mut self.npcs,
                &self.map.rooms,
                &mut self.rng,
                depth,
                player_room_idx,
                &self.landmarks,
            );
        }

        // Spawn camp if specified
        self.camps.clear();
        if let Some(camp_kind) = spec.as_ref().and_then(|s| s.camp).filter(|_| depth >= 1) {
            let used_for_npc: Vec<usize> = self.landmarks.iter().map(|(ri, _)| *ri).collect();
            let used_for_named: Vec<usize> = self
                .npcs
                .iter()
                .filter_map(|npc| {
                    self.map.rooms.iter().position(|r| {
                        npc.x >= r.x && npc.x < r.x + r.w && npc.y >= r.y && npc.y < r.y + r.h
                    })
                })
                .collect();
            let eligible: Vec<usize> = (0..room_count)
                .filter(|&i| {
                    Some(i) != player_room_idx
                        && !used_for_npc.contains(&i)
                        && !used_for_named.contains(&i)
                })
                .collect();
            if !eligible.is_empty() {
                let camp_room_idx = eligible[self.rng.gen_range(0..eligible.len())];
                let room = &self.map.rooms[camp_room_idx];
                let (cx, cy) = room.center();
                let npc_kinds = camp_kind.npc_kinds();

                let mut camp_npc_indices = Vec::new();
                let base_idx = self.npcs.len();
                let offsets: &[(i32, i32)] = match npc_kinds.len() {
                    1 => &[(0, 0)],
                    2 => &[(-1, 0), (1, 0)],
                    3 => &[(0, -1), (-1, 1), (1, 1)],
                    4 => &[(-1, -1), (1, -1), (-1, 1), (1, 1)],
                    _ => &[(-2, -1), (0, -1), (2, -1), (-1, 1), (1, 1)],
                };
                for (i, &kind) in npc_kinds.iter().enumerate() {
                    let (ox, oy) = offsets[i % offsets.len()];
                    let (nx, ny) = (
                        (cx + ox).clamp(room.x + 1, room.x + room.w - 2),
                        (cy + oy).clamp(room.y + 1, room.y + room.h - 2),
                    );
                    self.npcs.push(Npc::new(nx, ny, kind));
                    camp_npc_indices.push(base_idx + i);
                }
                self.camps.push(Camp {
                    room_idx: camp_room_idx,
                    x: cx,
                    y: cy,
                    kind: camp_kind,
                    npc_indices: camp_npc_indices,
                    rested: false,
                });
            }
        }

        // ── Campaign-specific content placement ──
        // Set objectives BEFORE content placement; lantern code updates them.
        if let Some(ref mut campaign) = self.campaign {
            campaign.set_objectives_for_level(camp_level);
        }
        if let Some(ref cspec) = spec {
            // Set custom names on named campaign NPCs
            for (ni, &(camp_npc, _role)) in cspec.named_npcs.iter().enumerate() {
                if let Some(npc) = self.npcs.get_mut(ni) {
                    npc.custom_name = Some(camp_npc.name().to_string());
                }
            }

            // Level 1: place the Rustguard bell-token
            if camp_level == 0 {
                let has_token = self
                    .campaign
                    .as_ref()
                    .map(|c| c.has_flag(crate::campaign::CampaignFlag::RecoveredBellToken))
                    .unwrap_or(false);
                if !has_token {
                    let mut token_rooms: Vec<usize> = (0..room_count)
                        .filter(|&i| {
                            Some(i) != player_room_idx
                                && !self.landmarks.iter().any(|(ri, _)| *ri == i)
                        })
                        .collect();
                    // Shuffle to avoid always picking the same room
                    for i in (1..token_rooms.len()).rev() {
                        let j = self.rng.gen_range(0..=i);
                        token_rooms.swap(i, j);
                    }
                    let mut placed = false;
                    for &tri in &token_rooms {
                        let (tx, ty) = self.map.rooms[tri].center();
                        let ti = self.map.idx(tx, ty);
                        let token_occupied = self.items.iter().any(|it| it.x == tx && it.y == ty)
                            || self.npcs.iter().any(|n| n.x == tx && n.y == ty)
                            || self
                                .monsters
                                .iter()
                                .any(|m| m.alive && m.x == tx && m.y == ty);
                        if !token_occupied && self.map.tiles[ti] == Tile::Floor {
                            self.items.push(Item::new(tx, ty, ItemKind::BellToken));
                            placed = true;
                            break;
                        }
                    }
                    // Fallback: try any empty floor tile in the token rooms
                    if !placed {
                        for &tri in &token_rooms {
                            let room = &self.map.rooms[tri];
                            for _ in 0..20 {
                                let (tx, ty) = room.random_tile(&mut self.rng);
                                let ti = self.map.idx(tx, ty);
                                let token_occupied =
                                    self.items.iter().any(|it| it.x == tx && it.y == ty)
                                        || self.npcs.iter().any(|n| n.x == tx && n.y == ty)
                                        || self
                                            .monsters
                                            .iter()
                                            .any(|m| m.alive && m.x == tx && m.y == ty);
                                if !token_occupied && self.map.tiles[ti] == Tile::Floor {
                                    self.items.push(Item::new(tx, ty, ItemKind::BellToken));
                                    placed = true;
                                    break;
                                }
                            }
                            if placed {
                                break;
                            }
                        }
                    }
                }
            }

            // Level 2: place blue lanterns
            if camp_level == 1 {
                let lanterns_lit = self
                    .campaign
                    .as_ref()
                    .map(|c| {
                        (c.has_flag(crate::campaign::CampaignFlag::Lantern1Lit) as u32)
                            + (c.has_flag(crate::campaign::CampaignFlag::Lantern2Lit) as u32)
                            + (c.has_flag(crate::campaign::CampaignFlag::Lantern3Lit) as u32)
                    })
                    .unwrap_or(0);
                let lanterns_needed = 3usize.saturating_sub(lanterns_lit as usize);
                let mut lantern_rooms: Vec<usize> = (0..room_count)
                    .filter(|&i| {
                        Some(i) != player_room_idx
                            && !self.landmarks.iter().any(|(ri, _)| *ri == i)
                            && !self.npcs.iter().any(|npc| {
                                let r = &self.map.rooms[i];
                                npc.x >= r.x
                                    && npc.x < r.x + r.w
                                    && npc.y >= r.y
                                    && npc.y < r.y + r.h
                            })
                    })
                    .collect();
                for i in (1..lantern_rooms.len()).rev() {
                    let j = self.rng.gen_range(0..=i);
                    lantern_rooms.swap(i, j);
                }
                // Place one lit and two dead lanterns (per GAME.md spec).
                // Track progress from map tiles only — flags are historical
                // markers, not the live count.
                for (idx, &ri) in lantern_rooms.iter().enumerate().take(lanterns_needed) {
                    let (lx, ly) = self.map.rooms[ri].center();
                    let li = self.map.idx(lx, ly);
                    if self.map.tiles[li] == Tile::Floor {
                        self.map.tiles[li] = if idx < 1 {
                            Tile::LanternLit
                        } else {
                            Tile::LanternDead
                        };
                    }
                }
                // Update objective counter from map tiles only
                if let Some(ref mut campaign) = self.campaign {
                    let map_lit = self
                        .map
                        .tiles
                        .iter()
                        .filter(|t| **t == Tile::LanternLit)
                        .count() as u32;
                    campaign
                        .objectives
                        .retain(|o| !matches!(o, CampaignObjective::RelightLanterns { .. }));
                    campaign
                        .objectives
                        .push(CampaignObjective::RelightLanterns {
                            lit: map_lit,
                            needed: 3,
                        });
                }
            }

            // Level 3: ensure BellChamber landmark is placed
            if camp_level == 2 {
                let has_chamber = self
                    .landmarks
                    .iter()
                    .any(|(_, lm)| *lm == Landmark::BellChamber);
                if !has_chamber {
                    let chamber_rooms: Vec<usize> = (0..room_count)
                        .filter(|&i| {
                            Some(i) != player_room_idx
                                && !self.landmarks.iter().any(|(ri, _)| *ri == i)
                        })
                        .collect();
                    if !chamber_rooms.is_empty() {
                        let cri = chamber_rooms[self.rng.gen_range(0..chamber_rooms.len())];
                        self.landmarks.push((cri, Landmark::BellChamber));
                    }
                }
            }
        }

        // Mark NPC positions as occupied
        for npc in &self.npcs {
            let i = self.map.idx(npc.x, npc.y);
            if i < occupied.len() {
                occupied[i] = true;
            }
        }

        // Traps, inscriptions, history tiles
        let trap_count = (1 + depth as usize).min(4);
        for _ in 0..trap_count * 5 {
            let rx = self.rng.gen_range(1..self.map.width - 1);
            let ry = self.rng.gen_range(1..self.map.height - 1);
            let i = self.map.idx(rx, ry);
            if self.map.tiles[i] == Tile::Floor && !occupied[i] {
                self.map.tiles[i] = Tile::TrapHidden;
                occupied[i] = true;
                break;
            }
        }

        self.place_dungeon_objects(depth, &mut occupied);
        self.place_environmental_terrain(depth, &occupied);

        self.refresh_fov();

        // Campaign descent intro
        if self.campaign.is_some() {
            let intro = spec.map(|s| s.descent_line).unwrap_or("You descend...");
            self.msg(intro, MsgColor::SYSTEM);
        }

        crate::logging::game_event(GameEvent::LevelGen {
            depth,
            rooms: self.map.rooms.len(),
            monsters: self.monsters.len(),
            items: self.items.len(),
        });
    }

    /// Generate a level for classic (non-campaign) mode.
    pub fn generate_level(&mut self, depth: u32) {
        self.map = Map::new(MAP_WIDTH, MAP_HEIGHT);
        let place_down = depth < MAX_DEPTH;
        self.map.generate(&mut self.rng, place_down);

        // Find stairs positions
        let mut stairs_down_pos: Option<(i32, i32)> = None;
        let mut stairs_up_pos: Option<(i32, i32)> = None;
        for y in 0..self.map.height {
            for x in 0..self.map.width {
                let t = self.map.tiles[self.map.idx(x, y)];
                if t == Tile::StairsDown {
                    stairs_down_pos = Some((x, y));
                } else if t == Tile::StairsUp {
                    stairs_up_pos = Some((x, y));
                }
            }
        }

        // Pick player spawn room furthest from stairs
        let room_count = self.map.rooms.len();
        if room_count > 0 {
            let best_room = (0..room_count)
                .max_by(|&a, &b| {
                    let score_a =
                        spawn::room_stairs_dist(&self.map.rooms[a], stairs_down_pos, stairs_up_pos);
                    let score_b =
                        spawn::room_stairs_dist(&self.map.rooms[b], stairs_down_pos, stairs_up_pos);
                    score_a
                        .partial_cmp(&score_b)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap_or(0);
            let (cx, cy) = self.map.rooms[best_room].center();
            self.player.x = cx;
            self.player.y = cy;
        } else {
            self.player.x = MAP_WIDTH / 2;
            self.player.y = MAP_HEIGHT / 2;
        }
        self.player.depth = depth;
        self.stats.deepest_depth = self.stats.deepest_depth.max(depth);

        // Spawn monsters
        self.monsters.clear();
        let base_count = 3 + depth as usize * 2;
        let monster_count = if self.player.crown_held {
            (base_count + 3).min(room_count.saturating_sub(1))
        } else {
            base_count.min(room_count.saturating_sub(1))
        };

        let player_room_idx = spawn::find_player_room(&self.map.rooms, &self.player);
        let mut spawn_rooms: Vec<usize> = (0..room_count)
            .filter(|&i| Some(i) != player_room_idx)
            .collect();
        if spawn_rooms.len() < monster_count {
            spawn_rooms = (0..room_count).collect();
        }
        for i in (1..spawn_rooms.len()).rev() {
            let j = self.rng.gen_range(0..=i);
            spawn_rooms.swap(i, j);
        }
        for &room_idx in spawn_rooms
            .iter()
            .take(monster_count.min(spawn_rooms.len()))
        {
            let room = self.map.rooms[room_idx];
            spawn::spawn_one_monster(
                &mut self.monsters,
                &self.map,
                &mut self.rng,
                &room,
                depth,
                self.player.x,
                self.player.y,
            );
        }

        if self.player.crown_held {
            if let Some(hunter) = self.monsters.first_mut() {
                let (x, y) = (hunter.x, hunter.y);
                let mut pursuit = Monster::new(x, y, MonsterKind::Wraith);
                let scale = 1.0 + depth as f32 * 0.15;
                let hp = (pursuit.stats.max_hp as f32 * scale).round() as i32;
                pursuit.stats.hp = hp;
                pursuit.stats.max_hp = hp;
                pursuit.last_seen_player_x = self.player.x;
                pursuit.last_seen_player_y = self.player.y;
                pursuit.alert_turns = 8;
                *hunter = pursuit;
            }
            self.msg(
                "A cold hunter gathers in the Crown's wake.",
                MsgColor::DAMAGE_TAKEN,
            );
        }

        // Guardian boss
        if depth == MAX_DEPTH {
            spawn::spawn_guardian(&mut self.monsters, &self.map, &spawn_rooms, player_room_idx);
            self.msg(
                "An ancient presence stirs in the darkness...",
                MsgColor::DAMAGE_TAKEN,
            );
        }

        // Spawn items
        self.items.clear();
        let mut item_count = 2 + depth as usize;
        let guarantee_crown = depth == MAX_DEPTH;
        if guarantee_crown {
            item_count = item_count.saturating_add(1);
        }

        let mut occupied =
            spawn::build_occupied_set(&self.map, &self.player, &self.monsters, &self.items);
        for _ in 0..item_count {
            spawn::try_spawn_item(
                &mut self.items,
                &self.map,
                &mut self.rng,
                depth,
                &mut occupied,
            );
        }
        if guarantee_crown {
            spawn::try_spawn_crown(&mut self.items, &self.map, &mut self.rng, &mut occupied);
        }

        // Assign landmarks
        spawn::assign_landmarks(
            &mut self.landmarks,
            &self.map.rooms,
            &mut self.rng,
            depth,
            player_room_idx,
        );

        // Spawn NPCs (standalone)
        spawn::spawn_npcs(
            &mut self.npcs,
            &self.map.rooms,
            &mut self.rng,
            depth,
            player_room_idx,
            &self.landmarks,
        );

        // NPC positions are also occupied (no items/traps on them)
        for npc in &self.npcs {
            let i = self.map.idx(npc.x, npc.y);
            occupied[i] = true;
        }

        // ── Spawn camps (depth 1+) ──
        self.camps.clear();
        if depth >= 1 {
            // Compute rooms already used by landmarks
            let landmark_room_indices: Vec<usize> =
                self.landmarks.iter().map(|(ri, _)| *ri).collect();
            spawn::spawn_camp(
                &mut self.npcs,
                &mut self.camps,
                &self.map.rooms,
                &mut self.rng,
                depth,
                player_room_idx,
                &landmark_room_indices,
            );
            // Mark camp NPC positions as occupied
            for camp in &self.camps {
                for &npc_idx in &camp.npc_indices {
                    if let Some(npc) = self.npcs.get(npc_idx) {
                        let i = self.map.idx(npc.x, npc.y);
                        if i < occupied.len() {
                            occupied[i] = true;
                        }
                    }
                }
            }
        }

        // Mark shrine tiles
        spawn::mark_shrine_tiles(&self.landmarks, &self.map.rooms, &self.map, &mut occupied);

        // ── Place traps (1-4 per floor) ──
        let trap_count = (1 + depth as usize).min(4);
        for _ in 0..trap_count * 5 {
            let rx = self.rng.gen_range(1..self.map.width - 1);
            let ry = self.rng.gen_range(1..self.map.height - 1);
            let i = self.map.idx(rx, ry);
            if self.map.tiles[i] == Tile::Floor && !occupied[i] {
                self.map.tiles[i] = Tile::TrapHidden;
                occupied[i] = true;
                break;
            }
        }

        // ── Place inscription tiles (1-2 per floor) ──
        let inscription_count = self.rng.gen_range(1..=2);
        for _ in 0..inscription_count * 10 {
            let rx = self.rng.gen_range(1..self.map.width - 1);
            let ry = self.rng.gen_range(1..self.map.height - 1);
            let i = self.map.idx(rx, ry);
            if self.map.tiles[i] == Tile::Floor && !occupied[i] {
                self.map.tiles[i] = Tile::Inscription;
                occupied[i] = true;
                break;
            }
        }

        // ── Place history tiles (2-4 per floor) ──
        let history_glyphs = ['\u{03A9}', '\u{2248}', '\u{229E}', '\u{273F}', '\u{2620}'];
        let history_count = self.rng.gen_range(2..=4);
        for _ in 0..history_count * 10 {
            let rx = self.rng.gen_range(1..self.map.width - 1);
            let ry = self.rng.gen_range(1..self.map.height - 1);
            let i = self.map.idx(rx, ry);
            if self.map.tiles[i] == Tile::Floor && !occupied[i] {
                let glyph = history_glyphs[self.rng.gen_range(0..history_glyphs.len())];
                self.map.tiles[i] = Tile::History(glyph);
                occupied[i] = true;
                break;
            }
        }

        self.place_dungeon_objects(depth, &mut occupied);
        self.place_environmental_terrain(depth, &occupied);

        self.refresh_fov();
        if depth > 0 {
            self.msg(narrative::descent_line(depth), MsgColor::SYSTEM);
        }
        crate::logging::game_event(GameEvent::LevelGen {
            depth,
            rooms: self.map.rooms.len(),
            monsters: self.monsters.len(),
            items: self.items.len(),
        });
    }

    fn place_environmental_terrain(&mut self, depth: u32, occupied: &[bool]) {
        let (tile, count) = match depth {
            1 => (Tile::ShallowWater, 8),
            3 => (Tile::Web, 10),
            _ => return,
        };
        let mut placed = 0;
        for _ in 0..count * 20 {
            if placed >= count {
                break;
            }
            let x = self.rng.gen_range(1..self.map.width - 1);
            let y = self.rng.gen_range(1..self.map.height - 1);
            let index = self.map.idx(x, y);
            if self.map.tiles[index] == Tile::Floor && !occupied[index] {
                self.map.tiles[index] = tile;
                placed += 1;
            }
        }
    }

    fn place_dungeon_objects(&mut self, depth: u32, occupied: &mut [bool]) {
        if depth >= 2 {
            let doors = self
                .map
                .tiles
                .iter()
                .enumerate()
                .filter_map(|(index, tile)| (*tile == Tile::DoorClosed).then_some(index))
                .collect::<Vec<_>>();
            let player_room = self.map.rooms.iter().find(|room| {
                self.player.x >= room.x
                    && self.player.x < room.x + room.w
                    && self.player.y >= room.y
                    && self.player.y < room.y + room.h
            });
            let key_tiles = player_room
                .into_iter()
                .flat_map(|room| {
                    (room.y..room.y + room.h)
                        .flat_map(move |y| (room.x..room.x + room.w).map(move |x| (x, y)))
                })
                .filter(|(x, y)| {
                    let index = self.map.idx(*x, *y);
                    self.map.tiles[index] == Tile::Floor && !occupied[index]
                })
                .collect::<Vec<_>>();
            if !doors.is_empty() && !key_tiles.is_empty() {
                let door = doors[self.rng.gen_range(0..doors.len())];
                let (key_x, key_y) = key_tiles[self.rng.gen_range(0..key_tiles.len())];
                self.map.tiles[door] = Tile::LockedDoor;
                let key_index = self.map.idx(key_x, key_y);
                occupied[key_index] = true;
                self.items.push(Item::new(key_x, key_y, ItemKind::IronKey));
            }
        }

        let artifact = match depth {
            2 => Some(ItemKind::Emberbrand),
            3 => Some(ItemKind::TideglassMail),
            4 => Some(ItemKind::Graveward),
            _ => None,
        };
        if let Some(artifact) = artifact {
            let already_owned = self.player.inventory.contains(&artifact)
                || self.player.weapon == Some(artifact)
                || self.player.armor == Some(artifact);
            if !already_owned {
                for _ in 0..100 {
                    let x = self.rng.gen_range(1..self.map.width - 1);
                    let y = self.rng.gen_range(1..self.map.height - 1);
                    let index = self.map.idx(x, y);
                    if self.map.tiles[index] == Tile::Floor && !occupied[index] {
                        occupied[index] = true;
                        self.items.push(Item::new(x, y, artifact));
                        break;
                    }
                }
            }
        }

        let object_count = if depth == 0 { 2 } else { 4 };
        for object_number in 0..object_count {
            for _ in 0..40 {
                let x = self.rng.gen_range(1..self.map.width - 1);
                let y = self.rng.gen_range(1..self.map.height - 1);
                let index = self.map.idx(x, y);
                if self.map.tiles[index] == Tile::Floor && !occupied[index] {
                    self.map.tiles[index] = if depth > 0 && object_number == object_count - 1 {
                        Tile::Barricade
                    } else {
                        Tile::Urn
                    };
                    occupied[index] = true;
                    break;
                }
            }
        }
    }

    // (spawning helpers moved to crate::spawn)

    // ── Turn processing ──
    pub fn process_turn(&mut self, action: Action, visual: &mut VisualState) {
        if self.status != GameStatus::Playing {
            return;
        }
        if self.rules_version >= 2 && self.player.pending_level_ups > 0 {
            self.msg("Choose a discipline before acting.", MsgColor::SYSTEM);
            return;
        }

        // Record action for replay
        if let Some(ref mut replay) = self.replay_data {
            replay.record(self.stats.turns, action);
        }

        match action {
            Action::Wait => {
                self.player.guarding = true;
                if self.rules_version > 0 {
                    self.player.recover_stamina(3);
                }
                visual.start_player_action(PlayerActionKind::Guard);
                self.msg("You brace for the next blow.", MsgColor::DIM);
                self.end_turn(visual);
                return;
            }
            Action::PickUp => {
                let did_something = self.pick_up_item(visual);

                // ── Campaign lantern interaction (runs even without an item to pick up) ──
                let mut lantern_action = false;
                if self.game_mode == GameMode::Campaign {
                    let tile = self.map.tiles[self.map.idx(self.player.x, self.player.y)];
                    if tile == Tile::LanternDead {
                        lantern_action = true;
                        // Check if player has LanternOil
                        let oil_idx = self
                            .player
                            .inventory
                            .iter()
                            .position(|&k| k == ItemKind::LanternOil);
                        if let Some(oil_slot) = oil_idx {
                            self.player.inventory.remove(oil_slot);
                            let lantern_idx = self.map.idx(self.player.x, self.player.y);
                            self.map.tiles[lantern_idx] = Tile::LanternLit;
                            self.msg(
                                "You pour lantern oil on the wick. The flame burns blue.",
                                MsgColor::LOOT,
                            );
                            // Set the next unset lantern flag
                            if let Some(ref mut campaign) = self.campaign {
                                use crate::campaign::CampaignFlag;
                                if !campaign.has_flag(CampaignFlag::Lantern1Lit) {
                                    campaign.set_flag(CampaignFlag::Lantern1Lit);
                                } else if !campaign.has_flag(CampaignFlag::Lantern2Lit) {
                                    campaign.set_flag(CampaignFlag::Lantern2Lit);
                                } else if !campaign.has_flag(CampaignFlag::Lantern3Lit) {
                                    campaign.set_flag(CampaignFlag::Lantern3Lit);
                                    campaign.set_flag(CampaignFlag::AllLanternsLit);
                                }
                                // Update objective: count lit tiles directly — no
                                // double-counting of flags + tiles.
                                let map_lit = self
                                    .map
                                    .tiles
                                    .iter()
                                    .filter(|t| **t == Tile::LanternLit)
                                    .count() as u32;
                                if map_lit >= 3 {
                                    campaign.set_flag(CampaignFlag::AllLanternsLit);
                                }
                                campaign.objectives.retain(|o| {
                                    !matches!(o, CampaignObjective::RelightLanterns { .. })
                                });
                                campaign
                                    .objectives
                                    .push(CampaignObjective::RelightLanterns {
                                        lit: map_lit,
                                        needed: 3,
                                    });
                            }
                            visual.add_effect(VisualEffect::pickup_sparkle(
                                self.player.x,
                                self.player.y,
                            ));
                        } else {
                            self.msg(
                                "A dead lantern. It needs lantern oil to relight.",
                                MsgColor::DIM,
                            );
                        }
                    }
                }

                if did_something || lantern_action {
                    self.end_turn(visual);
                }
                return;
            }
            Action::Descend => {
                self.try_descend(visual);
                return;
            }
            Action::Ascend => {
                self.try_ascend(visual);
                return;
            }
            Action::AutoExplore => {
                self.auto_explore(visual);
                return;
            }
            Action::Talk => {
                self.try_talk();
                return;
            }
            Action::TalkAt(x, y) => {
                self.try_talk_at(x, y);
                return;
            }
            Action::ToggleDoor(dx, dy) => {
                self.toggle_facing_door(dx, dy, visual);
                return;
            }
            Action::Listen(dx, dy) => {
                self.listen_facing(dx, dy, visual);
                return;
            }
            Action::Disarm(dx, dy) => {
                self.disarm_trap(dx, dy, visual);
                return;
            }
            Action::Dodge(dx, dy) => {
                self.try_dodge(dx, dy, visual);
                return;
            }
            Action::Technique(dx, dy) => {
                self.try_technique(dx, dy, visual);
                return;
            }
            Action::Interact => {
                self.msg("Nothing here responds.", MsgColor::DIM);
                return;
            }
            Action::ConversationChoice(n) => {
                self.handle_conversation_choice(n);
                return;
            }
            _ => {}
        }

        let (dx, dy) = match action {
            Action::Move(dx, dy) => (dx, dy),
            _ => return,
        };
        let (nx, ny) = (self.player.x + dx, self.player.y + dy);

        if self.map.in_bounds(nx, ny) {
            match self.map.tiles[self.map.idx(nx, ny)] {
                Tile::LockedDoor => {
                    self.open_locked_door(nx, ny, visual);
                    return;
                }
                Tile::Urn => {
                    self.stats.objects_smashed += 1;
                    let index = self.map.idx(nx, ny);
                    self.map.tiles[index] = Tile::Floor;
                    let gold = self.rng.gen_range(1..=6);
                    self.player.gold += gold;
                    if self.player.inventory.len() < 26 && self.rng.gen_bool(0.2) {
                        self.player.inventory.push(ItemKind::HealthPotion);
                        crate::inventory::sort_inventory(&mut self.player);
                        self.msg(
                            format!("The urn breaks: {} gold and a potion.", gold),
                            MsgColor::LOOT,
                        );
                    } else {
                        self.msg(format!("The urn breaks: {} gold.", gold), MsgColor::LOOT);
                    }
                    visual.add_effect(VisualEffect::death_burst(nx, ny));
                    self.end_turn(visual);
                    return;
                }
                Tile::Barricade => {
                    self.stats.objects_smashed += 1;
                    let index = self.map.idx(nx, ny);
                    self.map.tiles[index] = Tile::Floor;
                    if self.player.weapon != Some(ItemKind::Warhammer) {
                        let before = self.player.stats.hp;
                        self.player.stats.hp = (self.player.stats.hp - 1).max(1);
                        self.stats.dmg_taken += before - self.player.stats.hp;
                    }
                    self.msg(
                        if self.player.weapon == Some(ItemKind::Warhammer) {
                            "The warhammer scatters the barricade."
                        } else {
                            "You wrench the barricade apart and take 1 damage."
                        },
                        MsgColor::SYSTEM,
                    );
                    visual.add_effect(VisualEffect::death_burst(nx, ny));
                    self.fov_dirty = FovDirty::MapChanged;
                    self.end_turn(visual);
                    return;
                }
                _ => {}
            }
        }

        // ── Door opening ──
        if self.map.open_door(nx, ny) {
            self.msg("You open the door.", MsgColor::SYSTEM);
            self.fov_dirty = FovDirty::MapChanged;
            self.end_turn(visual);
            return;
        }

        // ── NPC blocking ──
        if self.npcs.iter().any(|npc| npc.x == nx && npc.y == ny) {
            self.try_talk_at(nx, ny);
            return;
        }

        // Bump-to-attack
        if let Some(idx) = self
            .monsters
            .iter()
            .position(|m| m.alive && m.x == nx && m.y == ny)
        {
            let attack_cost = match self.player.weapon {
                Some(ItemKind::Warhammer) => 3,
                Some(ItemKind::Longsword | ItemKind::Emberbrand) => 2,
                _ => 1,
            };
            let steady_attack = self.rules_version == 0 || self.spend_player_stamina(attack_cost);
            let counter = self.rules_version > 0 && self.player.counter_turns > 0;
            let interrupted = self.rules_version > 0 && self.monsters[idx].windup > 0;
            if self.rules_version >= 2 {
                self.stats.basic_attacks += 1;
                self.stats.counters += u32::from(counter);
                self.stats.interrupts += u32::from(interrupted);
            }
            if counter {
                self.player.counter_turns = 0;
            }
            // Awaken stationary monsters (mimics, sentinels) on first hit.
            self.monsters[idx].awakened = true;

            let mut dmg = combat::resolve_attack(
                self.player.total_attack(),
                self.monsters[idx].stats.defense,
            );
            if self.player.weapon == Some(ItemKind::Longsword)
                && self.monsters[idx].stats.hp * 2 <= self.monsters[idx].stats.max_hp
            {
                dmg += 2;
            }
            if self.player.weapon == Some(ItemKind::Emberbrand)
                && matches!(
                    self.monsters[idx].kind,
                    MonsterKind::Spider | MonsterKind::Slime
                )
            {
                dmg += 2;
            }
            if !steady_attack {
                dmg = (dmg - 2).max(1);
            }
            if counter {
                dmg += 3 + i32::from(self.player.resolve_rank) * i32::from(self.rules_version >= 2);
            }
            if interrupted {
                dmg += 3;
            }
            let m = &mut self.monsters[idx];
            let kind = m.kind;
            let name = kind.name();
            let old_hp = m.stats.hp;
            m.stats.hp -= dmg;
            if interrupted {
                m.windup = 0;
            }
            self.stats.dmg_dealt += dmg;

            visual.start_player_action(PlayerActionKind::Attack {
                weapon: self.player.weapon,
            });
            visual.add_effect(VisualEffect::attack_flash(m.x, m.y));
            visual.add_effect(VisualEffect::damage_number(m.x, m.y, dmg));

            let msg_hit = narrative::hit_flavour(kind, dmg, false, &mut self.rng);
            let killed = m.stats.hp <= 0;
            let guardian_phase_changed = kind == MonsterKind::Guardian
                && old_hp * 2 > m.stats.max_hp
                && m.stats.hp * 2 <= m.stats.max_hp;
            if !killed && dmg >= 8 {
                m.staggered = 1;
            }
            if !killed && (counter || interrupted) {
                m.staggered = 2;
            }

            // ── Slime split (25 % chance on hit, before death resolution) ──
            if !killed && kind == MonsterKind::Slime && self.rng.gen_bool(0.25) {
                // Try to spawn a slime copy on a free adjacent tile.
                let adj = [(0, 1), (0, -1), (1, 0), (-1, 0)];
                let (mx, my) = (m.x, m.y);
                for &(sdx, sdy) in &adj {
                    let (nx, ny) = (mx + sdx, my + sdy);
                    if self.map.is_walkable(nx, ny)
                        && !(nx == self.player.x && ny == self.player.y)
                        && !self
                            .monsters
                            .iter()
                            .any(|om| om.alive && om.x == nx && om.y == ny)
                    {
                        let split = Monster::new(nx, ny, MonsterKind::Slime);
                        self.monsters.push(split);
                        self.msg("The slime splits into two!", MsgColor::DAMAGE_TAKEN);
                        break;
                    }
                }
            }

            // Emit hit message before mutable borrow on monsters ends
            self.msg(msg_hit, MsgColor::DAMAGE_DEALT);
            if !steady_attack {
                self.msg(
                    "The exhausted swing lands without its full weight.",
                    MsgColor::DIM,
                );
            }
            if counter {
                self.msg("Guard becomes counterstrike!", MsgColor::DAMAGE_DEALT);
            }
            if interrupted {
                self.msg(
                    "You break the attack before it falls!",
                    MsgColor::DAMAGE_DEALT,
                );
            }
            if guardian_phase_changed && !killed {
                self.msg(
                    "The Guardian splits its seal. Crown-fire floods the armor!",
                    MsgColor::DAMAGE_TAKEN,
                );
                visual.add_effect(VisualEffect::fire_ring(
                    self.monsters[idx].x,
                    self.monsters[idx].y,
                ));
            }

            // Log the attack
            crate::logging::game_event(GameEvent::Attack {
                attacker: "player".into(),
                defender: name.to_string(),
                damage: dmg,
                remaining_hp: self.monsters[idx].stats.hp,
            });

            if !killed && self.player.weapon == Some(ItemKind::Warhammer) {
                let (mx, my) = (self.monsters[idx].x, self.monsters[idx].y);
                let (push_x, push_y) = (mx + dx, my + dy);
                let blocked = self.monsters.iter().enumerate().any(|(other, monster)| {
                    other != idx && monster.alive && (monster.x, monster.y) == (push_x, push_y)
                });
                if self.map.is_walkable(push_x, push_y)
                    && (push_x, push_y) != (self.player.x, self.player.y)
                    && !blocked
                {
                    self.monsters[idx].x = push_x;
                    self.monsters[idx].y = push_y;
                    visual.add_effect(VisualEffect::actor_step(push_x, push_y, dx, dy));
                    self.msg("The hammer drives it backward!", MsgColor::DAMAGE_DEALT);
                }
            }

            if killed {
                self.kill_monster(idx, visual);
            }
            self.wear_weapon(1);
            self.end_turn(visual);
            return;
        }

        if self.map.is_walkable(nx, ny) {
            let from = (self.player.x, self.player.y);
            self.player.x = nx;
            self.player.y = ny;
            self.on_player_entered_tile(nx, ny, from, visual);
            if !self.player.alive {
                return;
            }
            let terrain = self.map.tiles[self.map.idx(nx, ny)];
            if terrain == Tile::Web {
                let index = self.map.idx(nx, ny);
                self.map.tiles[index] = Tile::Floor;
                self.msg("You tear free of clinging webbing.", MsgColor::DIM);
            } else if terrain == Tile::ShallowWater {
                self.msg("Cold water drags at every step.", MsgColor::DIM);
            }
            self.end_turn(visual);
            if self.player.alive
                && (terrain == Tile::Web
                    || (terrain == Tile::ShallowWater
                        && self.player.armor != Some(ItemKind::TideglassMail)))
            {
                self.end_turn(visual);
            }
        }
    }

    fn try_dodge(&mut self, dx: i32, dy: i32, visual: &mut VisualState) {
        if self.rules_version == 0 {
            self.msg(
                "Evasive steps are unavailable in legacy rules.",
                MsgColor::DIM,
            );
            return;
        }
        let (nx, ny) = (self.player.x + dx, self.player.y + dy);
        let blocked_actor = self
            .monsters
            .iter()
            .any(|monster| monster.alive && (monster.x, monster.y) == (nx, ny))
            || self.npcs.iter().any(|npc| (npc.x, npc.y) == (nx, ny));
        let hindered = self.map.in_bounds(nx, ny)
            && matches!(
                self.map.tiles[self.map.idx(nx, ny)],
                Tile::ShallowWater | Tile::Web
            );
        if !self.map.is_walkable(nx, ny) || blocked_actor || hindered {
            self.msg(
                "There is no clear footing for an evasive step.",
                MsgColor::DIM,
            );
            return;
        }
        if self.player.stamina < 3 {
            self.msg("You are too winded to evade.", MsgColor::DAMAGE_TAKEN);
            return;
        }

        self.spend_player_stamina(3);
        if self.rules_version >= 2 {
            self.stats.dodges += 1;
        }
        self.player.evasion_turns = 1;
        let from = (self.player.x, self.player.y);
        self.player.x = nx;
        self.player.y = ny;
        self.on_player_entered_tile(nx, ny, from, visual);
        if self.player.alive {
            self.end_turn(visual);
        }
    }

    pub fn pick_up_item(&mut self, visual: &mut VisualState) -> bool {
        let Some(idx) = self
            .items
            .iter()
            .position(|i| i.x == self.player.x && i.y == self.player.y)
        else {
            // ── Campaign bell chamber interaction ──
            if self.game_mode == GameMode::Campaign {
                // Check if player is in the BellChamber room
                let player_room = self.map.rooms.iter().position(|r| {
                    self.player.x >= r.x
                        && self.player.x < r.x + r.w
                        && self.player.y >= r.y
                        && self.player.y < r.y + r.h
                });
                if let Some(pri) = player_room {
                    if self
                        .landmarks
                        .iter()
                        .any(|(ri, lm)| *ri == pri && *lm == Landmark::BellChamber)
                    {
                        // Guard against repeat bell choices
                        let bell_done = self
                            .campaign
                            .as_ref()
                            .map(|c| {
                                c.has_flag(crate::campaign::CampaignFlag::BellChoiceRung)
                                    || c.has_flag(
                                        crate::campaign::CampaignFlag::BellChoiceShattered,
                                    )
                                    || c.has_flag(crate::campaign::CampaignFlag::BellChoiceMuffled)
                            })
                            .unwrap_or(false);
                        if bell_done {
                            self.msg(
                                "The bell is silent. Whatever power it held has passed.",
                                MsgColor::DIM,
                            );
                            return true;
                        }
                        let has_token = self
                            .campaign
                            .as_ref()
                            .map(|c| c.has_flag(crate::campaign::CampaignFlag::RecoveredBellToken))
                            .unwrap_or(false);
                        if has_token {
                            // Offer the bell choice dialog
                            self.msg("The cracked bell hangs before you. Its silence is wrong — waiting to be broken.", MsgColor::SYSTEM);
                            self.start_bell_choice_dialog();
                            return true;
                        } else {
                            self.msg("A massive iron bell hangs here, cracked and silent. Its surface is etched with Rustguard symbols.", MsgColor::Cyan);
                            return true;
                        }
                    }
                }
            }

            // No item — try to read a nearby inscription or history tile
            for &(dx, dy) in &[(0, 0), (0, -1), (0, 1), (-1, 0), (1, 0)] {
                let ix = self.player.x + dx;
                let iy = self.player.y + dy;
                if self.map.in_bounds(ix, iy) {
                    let i = self.map.idx(ix, iy);
                    if self.map.tiles[i] == Tile::Inscription {
                        let lore = crate::entity::lore_for_depth(self.player.depth)
                            .unwrap_or("Ancient runes, their meaning lost to time.");
                        self.msg(format!("You read: \"{}\"", lore), MsgColor::Cyan);
                        return true; // consumes a turn
                    }
                    if matches!(self.map.tiles[i], Tile::History(_)) {
                        let flavor = narrative::named_delver_epitaph(&mut self.rng);
                        self.msg(flavor, MsgColor::Cyan);
                        return true; // consumes a turn
                    }
                }
            }
            self.msg("Nothing to pick up here.", MsgColor::DIM);
            return false;
        };
        let item = self.items.remove(idx);
        let display_name = item.kind.display_name(&self.player);
        self.msg(format!("You pick up {}.", display_name), MsgColor::LOOT);
        visual.add_effect(VisualEffect::pickup_sparkle(self.player.x, self.player.y));
        crate::logging::game_event(GameEvent::PickUp {
            item: item.kind.name().to_string(),
            x: self.player.x,
            y: self.player.y,
        });

        let kind = item.kind;
        if kind == ItemKind::CrownOfDepth {
            self.player.crown_held = true;
            self.msg(
                "The Crown rests in your hands — warm, expectant, patient. Return to the surface.",
                MsgColor::LOOT,
            );
            self.player.inventory.push(kind);
            visual.add_effect(VisualEffect::crown_sparkle(self.player.x, self.player.y));
        } else if kind == ItemKind::BellToken {
            self.player.inventory.push(kind);
            self.msg(
                "Rustguard iron, cold and heavy. The bell insignia catches the torchlight.",
                MsgColor::LOOT,
            );
            // Set the recovered flag immediately
            if let Some(ref mut campaign) = self.campaign {
                campaign.set_flag(crate::campaign::CampaignFlag::RecoveredBellToken);
            }
        } else if item.cursed && kind.is_equippable() {
            let (_old, _) = inventory::equip_item(&mut self.player, kind, kind.is_weapon());
            if kind.is_weapon() {
                self.player.weapon_cursed = true;
                self.player.weapon_durability = kind.max_durability();
            } else {
                self.player.armor_cursed = true;
            }
            self.msg(
                format!("{} binds itself to you. It is cursed!", kind.name()),
                MsgColor::DAMAGE_TAKEN,
            );
        } else if kind.is_equippable()
            && ((kind.is_weapon() && self.player.weapon.is_none())
                || (kind.is_armor() && self.player.armor.is_none()))
        {
            let (_old, equip_msg) = inventory::equip_item(&mut self.player, kind, kind.is_weapon());
            self.msg(equip_msg, MsgColor::LOOT);
        } else {
            self.player.inventory.push(kind);
        }
        crate::inventory::sort_inventory(&mut self.player);
        true
    }

    pub fn use_item(&mut self, kind: ItemKind, slot: usize, visual: &mut VisualState) -> bool {
        crate::logging::game_event(GameEvent::UseItem {
            item: kind.name().to_string(),
        });
        if kind.requires_identification() {
            self.player.identified_items.insert(kind);
        }
        if kind.is_weapon() && self.player.weapon_cursed {
            self.msg(
                "Your cursed weapon refuses to be removed.",
                MsgColor::DAMAGE_TAKEN,
            );
            return false;
        }
        if kind.is_armor() && self.player.armor_cursed {
            self.msg(
                "Your cursed armor will not release you.",
                MsgColor::DAMAGE_TAKEN,
            );
            return false;
        }
        let messages = &mut self.messages;
        let mut msgs = |text: &str, color: MsgColor| messages.push(text, color);

        let result = match kind {
            ItemKind::HealthPotion | ItemKind::LargeHealthPotion => inventory::effect_potion(
                &mut self.player,
                &mut self.stats.items_used,
                slot,
                kind.heal_amount(),
                &mut msgs,
                visual,
            ),
            ItemKind::ScrollOfFire => {
                // Snapshot alive monsters before the scroll so we can detect
                // which ones died from the AoE.
                let before = self.monsters.iter().map(|m| m.alive).collect::<Vec<_>>();
                let result = inventory::effect_fire_scroll(
                    &mut self.monsters,
                    &mut self.player,
                    &mut self.stats.items_used,
                    slot,
                    &self.map,
                    &mut msgs,
                    visual,
                );
                let mut burned_webs = 0;
                for index in 0..self.map.tiles.len() {
                    if self.map.tiles[index] == Tile::Web && self.map.visible[index] {
                        self.map.tiles[index] = Tile::Floor;
                        burned_webs += 1;
                    }
                }
                if burned_webs > 0 {
                    self.messages.push(
                        format!(
                            "{} web patch{} burn away.",
                            burned_webs,
                            if burned_webs == 1 { "" } else { "es" }
                        ),
                        MsgColor::DAMAGE_DEALT,
                    );
                }
                // Award XP/gold/level-ups for monsters killed by the scroll.
                for i in (0..self.monsters.len()).rev() {
                    if before[i] && self.monsters[i].stats.hp <= 0 {
                        self.kill_monster(i, visual);
                    }
                }
                result
            }
            ItemKind::ScrollOfConfusion => inventory::effect_confusion_scroll(
                &mut self.monsters,
                &self.map,
                &mut self.rng,
                &mut self.player,
                &mut self.stats.items_used,
                slot,
                &mut msgs,
                visual,
            ),
            ItemKind::Antidote => inventory::effect_antidote(
                &mut self.player,
                &mut self.stats.items_used,
                slot,
                &mut msgs,
                visual,
            ),
            ItemKind::LanternOil => {
                let used = inventory::effect_lantern_oil(
                    &mut self.player,
                    &mut self.stats.items_used,
                    slot,
                    &mut msgs,
                    visual,
                );
                if used {
                    self.fov_dirty = FovDirty::RadiusChanged;
                }
                used
            }
            ItemKind::ScrollOfBlink => {
                let blinked = inventory::effect_blink_scroll(
                    &mut self.player,
                    &self.map,
                    &mut self.rng,
                    &mut self.stats.items_used,
                    slot,
                    &mut msgs,
                    visual,
                );
                if blinked {
                    self.fov_dirty = FovDirty::PlayerMoved;
                }
                blinked
            }
            ItemKind::SmokeBomb => inventory::effect_smoke_bomb(
                &mut self.monsters,
                &self.map,
                &mut self.rng,
                &mut self.player,
                &mut self.stats.items_used,
                slot,
                &mut msgs,
            ),
            ItemKind::EnchantmentScroll => inventory::effect_enchantment(
                &mut self.player,
                &mut self.stats.items_used,
                slot,
                &mut msgs,
            ),
            ItemKind::LoreFragment => inventory::effect_lore_fragment(
                &mut self.player,
                &mut self.stats.items_used,
                slot,
                &mut msgs,
            ),
            kind if kind.is_equippable() => {
                let (_old, equip_msg) =
                    inventory::equip_item(&mut self.player, kind, kind.is_weapon());
                self.player.inventory.remove(slot);
                self.msg(equip_msg, MsgColor::LOOT);
                self.status = GameStatus::Playing;
                true
            }
            ItemKind::CrownOfDepth => {
                self.msg(
                    "The Crown cannot be used. You must wear it to the surface.",
                    MsgColor::DIM,
                );
                self.status = GameStatus::Playing;
                false
            }
            ItemKind::BellToken => {
                self.msg(
                    "The bell-token is cold and heavy — a Rustguard key, not a tool.",
                    MsgColor::DIM,
                );
                self.status = GameStatus::Playing;
                false
            }
            _ => {
                self.msg("You can't use that item.", MsgColor::DIM);
                false
            }
        };
        if result {
            crate::inventory::sort_inventory(&mut self.player);
        }
        result
    }

    pub fn use_quick_item(&mut self, visual: &mut VisualState) {
        let Some(kind) = self.player.quick_item else {
            self.msg(
                "No quick item assigned. Press Q in the inventory.",
                MsgColor::DIM,
            );
            return;
        };
        let selected = self
            .player
            .inventory
            .iter()
            .position(|item| *item == kind)
            .map(|slot| (slot, kind))
            .or_else(|| {
                let fallback = match kind {
                    ItemKind::HealthPotion => ItemKind::LargeHealthPotion,
                    ItemKind::LargeHealthPotion => ItemKind::HealthPotion,
                    _ => return None,
                };
                self.player
                    .inventory
                    .iter()
                    .position(|item| *item == fallback)
                    .map(|slot| (slot, fallback))
            });
        let Some((slot, selected_kind)) = selected else {
            self.player.quick_item = None;
            self.msg("Your quick-item slot is empty.", MsgColor::DIM);
            return;
        };
        if let Some(replay) = &mut self.replay_data {
            replay.record_repr(self.stats.turns, crate::replay::ActionRepr::UseItem(slot));
        }
        if self.use_item(selected_kind, slot, visual) {
            if !self.player.inventory.contains(&kind) {
                self.player.quick_item = None;
            }
            self.end_turn(visual);
        }
    }

    pub(crate) fn preserve_inventory_cursor(&mut self, preferred: ItemKind, fallback_slot: usize) {
        self.inventory_cursor = self
            .player
            .inventory
            .iter()
            .position(|item| *item == preferred)
            .unwrap_or_else(|| fallback_slot.min(self.player.inventory.len().saturating_sub(1)));
    }

    /// Called when the player enters a new tile (grid-based).
    /// Handles shared side effects for all player movement.
    pub(crate) fn on_player_entered_tile(
        &mut self,
        nx: i32,
        ny: i32,
        from: (i32, i32),
        visual: &mut VisualState,
    ) {
        self.fov_dirty = FovDirty::PlayerMoved;
        self.close_dialog_if_far();
        crate::logging::game_event(GameEvent::PlayerMove {
            from,
            to: (nx, ny),
            turns: self.stats.turns,
        });

        let idx = self.map.idx(nx, ny);
        self.map.explored[idx] = true;

        // Auto-pickup.
        if self.auto_pickup {
            let has_item = self.items.iter().any(|i| i.x == nx && i.y == ny);
            if has_item {
                self.pick_up_item(visual);
            }
        }

        // Traps.
        if let Some(trap_msg) = self.map.trigger_trap(nx, ny) {
            let dmg = 3 + self.rng.gen_range(0..=4);
            self.player.stats.hp -= dmg;
            self.stats.dmg_taken += dmg;
            visual.trigger_damage_flash();
            self.msg(
                format!("{} You take {} damage!", trap_msg, dmg),
                MsgColor::DAMAGE_TAKEN,
            );
            if self.player.stats.hp <= 0 {
                self.player.alive = false;
                let epitaph = crate::narrative::death_epitaph(
                    self.player.depth,
                    self.stats.kills,
                    Some("a trap"),
                    &mut self.rng,
                );
                self.messages.push(epitaph, MsgColor::DAMAGE_TAKEN);
                self.messages
                    .push("You have died...", MsgColor::DAMAGE_TAKEN);
                self.status = GameStatus::Dead;
            }
        }
    }

    /// Close any active dialog if the player moves too far from the NPC.
    pub(crate) fn close_dialog_if_far(&mut self) {
        if !self.dialog_active {
            return;
        }
        if let Some(idx) = self.dialog_npc_idx {
            if let Some(npc) = self.npcs.get(idx) {
                let dsq = (npc.x - self.player.x).pow(2) + (npc.y - self.player.y).pow(2);
                if dsq > 8 {
                    self.close_dialog();
                }
            } else {
                // NPC no longer exists
                self.close_dialog();
            }
        }
    }

    fn close_dialog(&mut self) {
        self.dialog_active = false;
        self.dialog_text.clear();
        self.dialog_npc_idx = None;
        self.dialog_conv_node = 0;
        self.dialog_show_choices = false;
        self.dialog_choices.clear();
        self.dialog_campaign_npc = None;
        self.preparation_menu = None;
    }

    // ── NPC interaction ──

    /// Talk to the nearest adjacent NPC.
    ///
    /// First talk starts a branching conversation if the NPC has a talk tree,
    /// otherwise shows the static dialog and applies the NPC's effect.
    /// Repeat talks show the repeat dialog.
    /// Pressing 't' or Esc while a conversation is showing a response closes it.
    /// Pressing 't' while choices are shown is a no-op (use 1-3 to pick).
    pub fn try_talk(&mut self) {
        self.try_talk_target(None);
    }

    pub fn try_talk_at(&mut self, x: i32, y: i32) {
        self.try_talk_target(Some((x, y)));
    }

    fn try_talk_target(&mut self, target: Option<(i32, i32)>) {
        // If dialog is showing a response (no choices), pressing 't' closes it
        if self.dialog_active && !self.dialog_show_choices {
            self.close_dialog();
            return;
        }
        // If dialog is showing choices, 't' does nothing — player must pick a choice or move away
        if self.dialog_active && self.dialog_show_choices {
            return;
        }

        // Find the nearest adjacent NPC
        let mut talk_action: Option<(usize, NpcKind, bool)> = None;

        for (i, npc) in self.npcs.iter().enumerate() {
            let dsq = (npc.x - self.player.x).pow(2) + (npc.y - self.player.y).pow(2);
            let matches_target = target
                .map(|position| position == (npc.x, npc.y))
                .unwrap_or(dsq <= 2);
            if matches_target {
                talk_action = Some((i, npc.kind, !npc.talked));
                break;
            }
        }

        if let Some((idx, kind, first_talk)) = talk_action {
            let npc_name = self.npcs[idx].display_name();
            // Campaign mode: always try campaign-specific dialog.
            // This allows re-evaluating dialog trees based on updated flags.
            if self.game_mode == GameMode::Campaign {
                if let Some(cnpc) = self.npc_to_campaign_npc(&self.npcs[idx]) {
                    self.try_campaign_talk(idx, cnpc, kind);
                    return;
                }
            }

            if !first_talk {
                // Repeat talk — just show the repeat dialog line, no choices
                self.dialog_active = true;
                self.dialog_npc_idx = Some(idx);
                self.dialog_text = vec![kind.repeat_dialog().to_string()];
                self.dialog_npc_name = npc_name.clone();
                self.dialog_show_choices = false;
                self.dialog_choices.clear();
                self.dialog_campaign_npc = None;
                return;
            }

            // First talk — check for branching conversation
            if let Some(tree) = kind.talk_tree() {
                // Start branching conversation at node 0
                self.dialog_active = true;
                self.dialog_npc_idx = Some(idx);
                self.dialog_conv_node = 0;
                let node = &tree[0];

                self.dialog_text = vec![node.npc_line.to_string()];
                self.dialog_npc_name = npc_name.clone();

                if node.choices.is_empty() {
                    // Leaf node — just show text, no choices
                    self.dialog_show_choices = false;
                    self.dialog_choices.clear();
                    // If there's a closing line, add it
                    if let Some(closing) = node.closing {
                        self.dialog_text.push(String::new());
                        self.dialog_text.push(closing.to_string());
                    }
                } else {
                    // Show choices
                    self.dialog_show_choices = true;
                    self.dialog_choices =
                        node.choices.iter().map(|c| c.label.to_string()).collect();
                }

                // Log the NPC's opening line to the message log
                self.msg(format!("{}: {}", npc_name, node.npc_line), MsgColor::Cyan);

                // Apply NPC effect (gameplay effects for the original 4 NPCs,
                // or just mark as talked for camp/branching NPCs)
                self.apply_npc_effect(kind, idx);
                return;
            }

            // Fallback: static one-liner dialog (for flavor NPCs without talk trees)
            let lines: Vec<String> = kind.dialog().split('\n').map(|s| s.to_string()).collect();
            self.dialog_active = true;
            self.dialog_npc_idx = Some(idx);
            self.dialog_text = lines.clone();
            self.dialog_npc_name = npc_name.clone();
            self.dialog_show_choices = false;
            self.dialog_choices.clear();

            for line in &lines {
                self.msg(format!("{}: {}", npc_name, line), MsgColor::Cyan);
            }
            if !kind.effect().is_empty() {
                self.apply_npc_effect(kind, idx);
            }
            return;
        }

        self.msg("No one to talk to here.", MsgColor::DIM);
    }

    /// Handle the player picking a conversation choice (1-3).
    pub fn handle_conversation_choice(&mut self, choice_num: u8) {
        if !self.dialog_active || !self.dialog_show_choices {
            return;
        }

        // Bell chamber: special climax dialog
        if self.dialog_npc_name == "The Bell" {
            self.handle_bell_choice(choice_num);
            return;
        }

        // Campaign mode dialog
        if self.game_mode == GameMode::Campaign && self.dialog_campaign_npc.is_some() {
            self.handle_campaign_choice(choice_num);
            return;
        }

        let choice_idx = (choice_num as usize).saturating_sub(1);

        // Get the NPC's talk tree
        let npc_idx = match self.dialog_npc_idx {
            Some(i) => i,
            None => return,
        };
        let kind = match self.npcs.get(npc_idx) {
            Some(npc) => npc.kind,
            None => return,
        };
        let npc_name = self.npcs[npc_idx].display_name();
        let tree = match kind.talk_tree() {
            Some(t) => t,
            None => return,
        };

        let node = match tree.get(self.dialog_conv_node) {
            Some(n) => n,
            None => return,
        };

        let choice = match node.choices.get(choice_idx) {
            Some(c) => c,
            None => return,
        };

        // Log the player choice
        self.msg(format!("You: {}", choice.label), MsgColor::DIM);

        // Advance to the next node (or end conversation)
        if choice.next_node == usize::MAX || choice.next_node >= tree.len() {
            self.close_dialog();
            return;
        }

        let next_node = &tree[choice.next_node];
        self.dialog_conv_node = choice.next_node;

        // Show NPC's response
        self.dialog_text = vec![next_node.npc_line.to_string()];
        if let Some(closing) = next_node.closing {
            self.dialog_text.push(String::new());
            self.dialog_text.push(closing.to_string());
        }
        self.dialog_show_choices = false;
        self.dialog_choices.clear();

        // Log the NPC response
        self.msg(
            format!("{}: {}", npc_name, next_node.npc_line),
            MsgColor::Cyan,
        );
    }

    // ── Campaign dialog helpers ──

    /// Map an NpcKind to a CampaignNpc for campaign dialog routing.
    /// Only NPCs with a custom_name set are named campaign characters;
    /// generic camp NPCs with the same NpcKind are excluded.
    fn npc_to_campaign_npc(
        &self,
        npc: &crate::entity::Npc,
    ) -> Option<crate::campaign::CampaignNpc> {
        use crate::campaign::CampaignNpc;
        npc.custom_name.as_ref()?;
        match npc.kind {
            NpcKind::WatchCaptain => Some(CampaignNpc::MaraVoss),
            NpcKind::WoundedDelver => Some(CampaignNpc::TovanReed),
            NpcKind::Cartographer => Some(CampaignNpc::EddaSlate),
            NpcKind::LanternTender => Some(CampaignNpc::Jun),
            NpcKind::Merchant => Some(CampaignNpc::Vey),
            _ => None,
        }
    }

    /// Start a campaign dialog with a named NPC.
    fn try_campaign_talk(
        &mut self,
        npc_idx: usize,
        cnpc: crate::campaign::CampaignNpc,
        _kind: NpcKind,
    ) {
        let camp_level = self.campaign.as_ref().map(|c| c.level).unwrap_or(0);
        let flags = self
            .campaign
            .as_ref()
            .map(|c| c.flags.clone())
            .unwrap_or_default();
        let trust = self
            .campaign
            .as_ref()
            .map(|c| c.trust_of(cnpc))
            .unwrap_or(0);

        let tree = crate::campaign::campaign_dialog(cnpc, camp_level, &flags, trust);
        let Some(tree) = tree else { return };
        if tree.is_empty() {
            return;
        }

        self.dialog_active = true;
        self.dialog_npc_idx = Some(npc_idx);
        self.dialog_campaign_npc = Some(cnpc);
        self.dialog_conv_node = 0;
        let npc_name = self.npcs[npc_idx].display_name();
        self.dialog_npc_name = npc_name.clone();

        let node = &tree[0];
        self.dialog_text = vec![node.npc_line.to_string()];

        if node.choices.is_empty() {
            self.dialog_show_choices = false;
            self.dialog_choices.clear();
            if let Some(closing) = node.closing {
                self.dialog_text.push(String::new());
                self.dialog_text.push(closing.to_string());
            }
        } else {
            self.dialog_show_choices = true;
            self.dialog_choices = node.choices.iter().map(|c| c.label.to_string()).collect();
        }

        // Log the NPC's opening line
        self.msg(format!("{}: {}", npc_name, node.npc_line), MsgColor::Cyan);

        // Mark NPC as talked
        if let Some(npc) = self.npcs.get_mut(npc_idx) {
            npc.talked = true;
        }
    }

    /// Handle a campaign conversation choice.
    fn handle_campaign_choice(&mut self, choice_num: u8) {
        let cnpc = match self.dialog_campaign_npc {
            Some(c) => c,
            None => {
                self.close_dialog();
                return;
            }
        };

        let camp_level = self.campaign.as_ref().map(|c| c.level).unwrap_or(0);
        let flags = self
            .campaign
            .as_ref()
            .map(|c| c.flags.clone())
            .unwrap_or_default();
        let trust = self
            .campaign
            .as_ref()
            .map(|c| c.trust_of(cnpc))
            .unwrap_or(0);

        let tree = crate::campaign::campaign_dialog(cnpc, camp_level, &flags, trust);
        let Some(tree) = tree else {
            self.close_dialog();
            return;
        };

        let node = match tree.get(self.dialog_conv_node) {
            Some(n) => n,
            None => {
                self.close_dialog();
                return;
            }
        };

        let choice_idx = (choice_num as usize).saturating_sub(1);
        let choice = match node.choices.get(choice_idx) {
            Some(c) => c,
            None => return,
        };

        // Log player choice
        self.msg(format!("You: {}", choice.label), MsgColor::DIM);

        // Apply campaign effects
        for effect in choice.effect {
            self.apply_dialog_effect(effect);
        }

        // Advance dialog or close
        if choice.next_node == -2 {
            self.close_dialog();
            return;
        }
        if choice.next_node == -3 || choice.next_node < 0 {
            self.close_dialog();
            return;
        }
        if choice.next_node as usize >= tree.len() {
            self.close_dialog();
            return;
        }

        let next_node = &tree[choice.next_node as usize];
        self.dialog_conv_node = choice.next_node as usize;

        self.dialog_text = vec![next_node.npc_line.to_string()];
        if let Some(closing) = next_node.closing {
            self.dialog_text.push(String::new());
            self.dialog_text.push(closing.to_string());
        }
        self.dialog_show_choices = false;
        self.dialog_choices.clear();

        // Log NPC response
        self.msg(
            format!("{}: {}", self.dialog_npc_name, next_node.npc_line),
            MsgColor::Cyan,
        );
    }

    /// Apply a single DialogEffect.
    fn apply_dialog_effect(&mut self, effect: &crate::campaign::DialogEffect) {
        use crate::campaign::DialogEffect;

        match effect {
            DialogEffect::SetFlag(flag) => {
                if let Some(ref mut campaign) = self.campaign {
                    campaign.set_flag(*flag);
                }
            }
            DialogEffect::Trust(npc, delta) => {
                if let Some(ref mut campaign) = self.campaign {
                    campaign.modify_trust(*npc, *delta);
                }
            }
            DialogEffect::GiveItem(item) => {
                self.player.inventory.push(*item);
                crate::inventory::sort_inventory(&mut self.player);
                self.msg(format!("Received {}!", item.name()), MsgColor::LOOT);
            }
            DialogEffect::RevealMap => {
                for y in 0..self.map.height {
                    for x in 0..self.map.width {
                        let i = self.map.idx(x, y);
                        if self.map.is_walkable(x, y) {
                            self.map.explored[i] = true;
                        }
                    }
                }
                self.fov_dirty = FovDirty::MapChanged;
                self.msg("The map is revealed!", MsgColor::SYSTEM);
            }
            DialogEffect::HealAndPotion => {
                self.player.stats.hp = self.player.stats.max_hp;
                self.player.inventory.push(ItemKind::HealthPotion);
                self.msg("You feel restored and receive a potion.", MsgColor::HEALING);
            }
            DialogEffect::CompleteObjective => {
                if let Some(ref mut campaign) = self.campaign {
                    campaign.set_flag(crate::campaign::CampaignFlag::GateOpened);
                }
                self.msg("Objective complete!", MsgColor::SYSTEM);
            }
        }
    }

    /// Start the bell chamber choice dialog (campaign climax).
    fn start_bell_choice_dialog(&mut self) {
        self.dialog_active = true;
        self.dialog_npc_name = "The Bell".to_string();
        self.dialog_show_choices = true;
        self.dialog_choices = vec![
            "Ring the bell".to_string(),
            "Shatter the bell".to_string(),
            "Muffle the bell".to_string(),
        ];
        self.dialog_text = vec![
            "The cracked bell waits. Its silence presses against you like a held breath."
                .to_string(),
            "Three choices. One silence.".to_string(),
        ];
        self.dialog_campaign_npc = Some(crate::campaign::CampaignNpc::MaraVoss); // reused for routing
        self.dialog_npc_idx = None;
    }

    /// Handle the bell chamber choice (1=Ring, 2=Shatter, 3=Muffle).
    fn handle_bell_choice(&mut self, choice_num: u8) {
        use crate::campaign::CampaignFlag;

        let choice = match choice_num {
            1 => CampaignFlag::BellChoiceRung,
            2 => CampaignFlag::BellChoiceShattered,
            3 => CampaignFlag::BellChoiceMuffled,
            _ => {
                self.close_dialog();
                return;
            }
        };

        if let Some(ref mut campaign) = self.campaign {
            campaign.set_flag(choice);
        }

        self.close_dialog();

        let (result, color) = match choice {
            CampaignFlag::BellChoiceRung => (
                "The chamber answers. Somewhere above, a Rustguard bell rings in reply.\nA stairway opens beneath the broken bell.",
                MsgColor::SYSTEM,
            ),
            CampaignFlag::BellChoiceShattered => (
                "Metal screams. The bell's shards scatter in the dark.\nBeneath the ruin, stairs open downward — silent, waiting.",
                MsgColor::SYSTEM,
            ),
            CampaignFlag::BellChoiceMuffled => (
                "You press cloth against cold iron. The bell is silent — for now.\nSomething in the deep knows you refused. The stairs open anyway, but colder.",
                MsgColor::SYSTEM,
            ),
            _ => ("", MsgColor::SYSTEM),
        };

        for line in result.lines() {
            self.msg(line, color);
        }
    }

    fn apply_npc_effect(&mut self, kind: NpcKind, idx: usize) {
        match kind.effect() {
            "heal_and_potion" => {
                self.player.stats.hp = self.player.stats.max_hp;
                self.player.inventory.push(ItemKind::HealthPotion);
                self.msg(
                    "The delver presses a potion into your hands. You feel restored.",
                    MsgColor::HEALING,
                );
            }
            "reveal_map" => {
                for y in 0..self.map.height {
                    for x in 0..self.map.width {
                        let i = self.map.idx(x, y);
                        if self.map.is_walkable(x, y) {
                            self.map.explored[i] = true;
                        }
                    }
                }
                self.msg(
                    "The cartographer shares their maps. The layout becomes clear.",
                    MsgColor::LOOT,
                );
            }
            "trade" => {
                if !self.player.inventory.is_empty() {
                    let slot = self.rng.gen_range(0..self.player.inventory.len());
                    let old = self.player.inventory.remove(slot);
                    let new = match old {
                        ItemKind::RustySword => ItemKind::Longsword,
                        ItemKind::LeatherArmor => ItemKind::Chainmail,
                        _ => ItemKind::HealthPotion,
                    };
                    self.player.inventory.push(new);
                    self.msg("The merchant upgrades your wares.", MsgColor::LOOT);
                } else {
                    self.player.inventory.push(ItemKind::HealthPotion);
                    self.msg(
                        "The merchant takes pity and offers a potion.",
                        MsgColor::LOOT,
                    );
                }
            }
            "bargain" => {
                if self.rng.gen_bool(0.5) {
                    self.player.stats.max_hp = (self.player.stats.max_hp - 5).max(10);
                    self.player.stats.hp = self.player.stats.hp.min(self.player.stats.max_hp);
                    self.player.stats.attack += 3;
                    self.msg(
                        "The shrine demands blood. You feel stronger, yet lighter.",
                        MsgColor::DAMAGE_TAKEN,
                    );
                } else {
                    self.player.stats.max_hp += 5;
                    self.player.stats.hp += 5;
                    self.player.stats.attack = (self.player.stats.attack - 1).max(1);
                    self.msg("The shrine blesses you with resilience.", MsgColor::HEALING);
                }
            }
            _ => {}
        }
        crate::inventory::sort_inventory(&mut self.player);
        if let Some(npc) = self.npcs.get_mut(idx) {
            npc.talked = true;
        }
    }

    // ── FOV helpers ──

    /// The effective FOV radius taking lantern oil and Crown into account.
    pub(crate) fn current_fov_radius(&self) -> i32 {
        let base = FOV_RADIUS;
        let plus_lantern = if self.player.lantern_turns > 0 { 3 } else { 0 };
        let minus_crown = if self.player.crown_held { -2 } else { 0 };
        let challenge_penalty = if self.challenge_mode == ChallengeMode::DeepFog {
            2
        } else {
            0
        };
        (base + plus_lantern + minus_crown - challenge_penalty).max(4)
    }

    /// Recompute FOV using the current modifier-aware radius, then mark clean.
    /// All FOV recomputes must go through this helper so lantern/crown
    /// modifiers stay consistent everywhere (level gen, load, end-turn).
    fn refresh_fov(&mut self) {
        let radius = self.current_fov_radius();
        fov::compute(&mut self.map, self.player.x, self.player.y, radius);
        self.fov_dirty = FovDirty::Clean;
    }

    // ── Centralised monster death (XP, gold, level-up, drops, visuals) ──

    /// Apply a killing blow to monster `idx`.  Awards XP/gold, handles
    /// level‑up, loot drops, and death visuals.  Callers must ensure the
    /// monster is alive beforehand and should still check `hp <= 0` before
    /// calling.
    fn kill_monster(&mut self, idx: usize, visual: &mut VisualState) {
        // Collect what we need while we have a reference to the monster.
        let kind = self.monsters[idx].kind;
        let (mx, my) = (self.monsters[idx].x, self.monsters[idx].y);
        self.monsters[idx].alive = false;
        self.defeated_monsters.insert(kind);

        self.stats.kills += 1;
        let xp = kind.xp_value();
        let (g_min, g_max) = kind.gold_range();
        let gold = self.rng.gen_range(g_min..=g_max);
        self.player.gold += gold;

        let levels = self.award_xp(xp);
        let xp_msg = format!(
            "+{} XP (+{} gold){}",
            xp,
            gold,
            if levels > 0 && self.rules_version >= 2 {
                format!(
                    " - DISCIPLINE READY. You are now level {}.",
                    self.player.level
                )
            } else if levels > 0 {
                format!(" - LEVEL UP! You are now level {}.", self.player.level)
            } else {
                String::new()
            }
        );
        let kill_msg = narrative::kill_flavour(kind, &mut self.rng).to_string();

        // Death visuals
        visual.add_effect(VisualEffect::monster_death(mx, my, kind));
        visual.add_decal(visual::Decal::Blood {
            x: mx,
            y: my,
            fade: 255,
        });

        // Logging
        crate::logging::game_event(GameEvent::Kill {
            killer: "player".into(),
            victim: kind.name().to_string(),
            x: mx,
            y: my,
        });

        // Loot drop (30 %)
        if self.rng.gen_bool(0.3) {
            let dk = match self.rng.gen_range(0..3) {
                0 => ItemKind::HealthPotion,
                1 => ItemKind::RustySword,
                _ => ItemKind::LeatherArmor,
            };
            if self.map.is_walkable(mx, my)
                && !self
                    .monsters
                    .iter()
                    .any(|om| om.alive && om.x == mx && om.y == my)
                && !self.items.iter().any(|it| it.x == mx && it.y == my)
            {
                self.items.push(Item::new(mx, my, dk));
            }
        }

        // ── Skeleton reassemble (30 % chance to rise with 40 % HP) ──
        if kind == MonsterKind::Skeleton && self.rng.gen_bool(0.30) {
            // Find a free walkable tile adjacent to the death location.
            let adj = [(0, 1), (0, -1), (1, 0), (-1, 0)];
            let mut found = false;
            for &(dx, dy) in &adj {
                let (nx, ny) = (mx + dx, my + dy);
                if self.map.is_walkable(nx, ny)
                    && !(nx == self.player.x && ny == self.player.y)
                    && !self
                        .monsters
                        .iter()
                        .any(|om| om.alive && om.x == nx && om.y == ny)
                {
                    let mut sk = Monster::new(nx, ny, MonsterKind::Skeleton);
                    let hp = (sk.stats.max_hp as f64 * 0.4).round() as i32;
                    sk.stats.hp = hp;
                    sk.stats.max_hp = (sk.stats.max_hp as f64 * 0.4).round() as i32;
                    sk.awakened = true;
                    self.monsters.push(sk);
                    self.msg(
                        "The skeleton's bones rattle and reassemble!",
                        MsgColor::DAMAGE_TAKEN,
                    );
                    found = true;
                    break;
                }
            }
            let _ = found; // Nothing to do if no free tile; reassemble fails silently.
        }

        self.msg(kill_msg, MsgColor::KILL);
        self.msg(xp_msg, MsgColor::LOOT);
    }

    fn award_xp(&mut self, amount: u32) -> u8 {
        self.player.xp = self.player.xp.saturating_add(amount);
        if self.rules_version < 2 {
            return u8::from(self.player.check_level_up());
        }

        let mut levels = 0_u8;
        while self.player.xp >= self.player.xp_to_next() {
            let needed = self.player.xp_to_next();
            self.player.xp -= needed;
            self.player.level = self.player.level.saturating_add(1);
            self.player.pending_level_ups = self.player.pending_level_ups.saturating_add(1);
            levels = levels.saturating_add(1);
        }
        levels
    }

    pub(crate) fn resolve_discipline_choice(&mut self, discipline: Discipline) -> bool {
        if self.rules_version < 2 || self.player.pending_level_ups == 0 {
            return false;
        }
        self.player.pending_level_ups -= 1;
        self.player.apply_discipline(discipline);
        if let Some(replay) = &mut self.replay_data {
            replay.record_repr(
                self.stats.turns,
                crate::replay::ActionRepr::DisciplineChoice(discipline),
            );
        }
        self.msg(
            format!(
                "{} deepens: {}.",
                discipline.name(),
                discipline.description()
            ),
            MsgColor::LOOT,
        );
        true
    }

    /// Check if all campaign objectives are complete.
    fn campaign_objectives_done(&self) -> bool {
        let Some(ref campaign) = self.campaign else {
            return true; // no campaign = no objectives to block
        };
        campaign.objectives.iter().all(|o| {
            match o {
                CampaignObjective::RecoverBellToken => {
                    // Gate must be explicitly opened by Mara — token pickup alone
                    // should not satisfy the level-0 descent check.
                    campaign.has_flag(crate::campaign::CampaignFlag::GateOpened)
                }
                CampaignObjective::RelightLanterns { lit, needed } => lit >= needed,
                CampaignObjective::SilenceTheBell => {
                    campaign.has_flag(crate::campaign::CampaignFlag::BellChoiceRung)
                        || campaign.has_flag(crate::campaign::CampaignFlag::BellChoiceShattered)
                        || campaign.has_flag(crate::campaign::CampaignFlag::BellChoiceMuffled)
                }
            }
        })
    }

    fn campaign_blocker(&self) -> &'static str {
        let Some(campaign) = self.campaign.as_ref() else {
            return "The campaign state is unavailable.";
        };
        for objective in &campaign.objectives {
            match objective {
                CampaignObjective::RecoverBellToken
                    if !campaign.has_flag(crate::campaign::CampaignFlag::GateOpened) =>
                {
                    return "Return the bell-token to Mara and open the stair-gate.";
                }
                CampaignObjective::RelightLanterns { lit, needed } if lit < needed => {
                    return "Relight all three blue lanterns before descending.";
                }
                CampaignObjective::SilenceTheBell
                    if !campaign.has_flag(crate::campaign::CampaignFlag::BellChoiceRung)
                        && !campaign
                            .has_flag(crate::campaign::CampaignFlag::BellChoiceShattered)
                        && !campaign.has_flag(crate::campaign::CampaignFlag::BellChoiceMuffled) =>
                {
                    return "Return to the Bell Chamber and decide the bell's fate.";
                }
                _ => {}
            }
        }
        "Complete the current objective before descending."
    }

    pub fn try_descend(&mut self, visual: &mut VisualState) {
        if self.map.tiles[self.map.idx(self.player.x, self.player.y)] == Tile::StairsDown {
            // Campaign mode: check objective completion before descent
            if self.game_mode == GameMode::Campaign {
                if self.campaign.is_none() {
                    self.msg(self.campaign_blocker(), MsgColor::DIM);
                    return;
                }
                if !self.campaign_objectives_done() {
                    self.msg(self.campaign_blocker(), MsgColor::DIM);
                    return;
                }

                let camp_level = self.campaign.as_ref().unwrap().level;
                let next_level = camp_level + 1;
                let is_chapter_end = camp_level == 2;
                if let Some(spec) = crate::campaign::chapter1_level_spec(camp_level) {
                    for line in spec
                        .chapter_outro
                        .iter()
                        .copied()
                        .filter(|line| !line.is_empty())
                    {
                        self.msg(line, MsgColor::SYSTEM);
                    }
                }

                // Update campaign level
                if let Some(ref mut campaign) = self.campaign {
                    campaign.level = next_level;
                }

                if is_chapter_end {
                    for line in crate::campaign::CHAPTER1_ENDING
                        .iter()
                        .copied()
                        .filter(|line| !line.is_empty())
                    {
                        self.msg(line, MsgColor::SYSTEM);
                    }
                    self.player.depth = next_level;
                    self.status = GameStatus::CampaignComplete;
                    visual.add_effect(VisualEffect::stairs_transition());
                    crate::logging::game_event(GameEvent::Descend {
                        from_depth: camp_level,
                        to_depth: self.player.depth,
                    });
                    return;
                }

                self.player.depth = next_level;
                visual.add_effect(VisualEffect::stairs_transition());
                self.generate_campaign_level(next_level);
                crate::logging::game_event(GameEvent::Descend {
                    from_depth: next_level.saturating_sub(1),
                    to_depth: next_level,
                });
                return;
            }

            // Classic mode
            if self.player.depth >= MAX_DEPTH {
                self.msg("You are already at the deepest depth.", MsgColor::DIM);
            } else {
                let from = self.player.depth;
                visual.add_effect(VisualEffect::stairs_transition());
                self.generate_level(self.player.depth + 1);
                crate::logging::game_event(GameEvent::Descend {
                    from_depth: from,
                    to_depth: self.player.depth,
                });
            }
        } else {
            self.msg("There are no stairs down here.", MsgColor::DIM);
        }
    }

    pub fn try_ascend(&mut self, visual: &mut VisualState) {
        if self.map.tiles[self.map.idx(self.player.x, self.player.y)] == Tile::StairsUp {
            if self.player.depth == 0 {
                if self.player.inventory.contains(&ItemKind::CrownOfDepth) {
                    self.status = GameStatus::Won;
                    for line in narrative::win_narrative() {
                        self.msg(line, MsgColor::HEALING);
                    }
                    crate::logging::game_event(GameEvent::Win {
                        turns: self.stats.turns,
                        kills: self.stats.kills,
                        deepest: self.stats.deepest_depth,
                    });
                } else {
                    self.msg("You return to the surface empty-handed... Explore deeper to find the Crown!", MsgColor::DIM);
                }
            } else {
                // Campaign mode: don't allow backtracking
                if self.game_mode == GameMode::Campaign {
                    self.msg(
                        "You can't turn back now. The way forward is the only way.",
                        MsgColor::DIM,
                    );
                    return;
                }
                let from = self.player.depth;
                visual.add_effect(VisualEffect::stairs_transition());
                self.generate_level(self.player.depth.saturating_sub(1));
                // Crown-aware ascent message
                if self.player.crown_held {
                    self.msg(narrative::ascent_line(self.player.depth), MsgColor::SYSTEM);
                }
                crate::logging::game_event(GameEvent::Ascend {
                    from_depth: from,
                    to_depth: self.player.depth,
                });
            }
        } else {
            self.msg("There are no stairs up here.", MsgColor::DIM);
        }
    }

    // ── Occupancy grid ──
    fn rebuild_occupancy(&mut self) {
        self.occupancy.fill(None);
        for (i, m) in self.monsters.iter().enumerate() {
            if m.alive {
                let idx = self.map.idx(m.x, m.y);
                self.occupancy[idx] = Some(i);
            }
        }
    }

    pub(crate) fn cell_free(&self, x: i32, y: i32, self_idx: usize, is_wraith: bool) -> bool {
        if !self.map.in_bounds(x, y) {
            return false;
        }
        if x == self.player.x && y == self.player.y {
            return false;
        }
        // NPCs block monster movement
        if self.npcs.iter().any(|npc| npc.x == x && npc.y == y) {
            return false;
        }
        if !is_wraith && !self.map.is_walkable(x, y) {
            return false;
        }
        let i = self.map.idx(x, y);
        !matches!(self.occupancy[i], Some(idx) if idx != self_idx)
    }

    // ── End turn ──
    pub fn end_turn(&mut self, visual: &mut VisualState) {
        // Never process turn logic when the game is over.
        if self.status != GameStatus::Playing {
            return;
        }

        self.stats.turns += 1;
        self.close_dialog_if_far();

        let prev_hp: Vec<i32> = self.monsters.iter().map(|m| m.stats.hp).collect();

        // Player poison
        if self.player.poisoned {
            self.player.poison_ticks -= 1;
            self.player.stats.hp -= 1;
            self.stats.dmg_taken += 1;
            visual.trigger_damage_flash();
            self.msg("You feel the poison burning...", MsgColor::DAMAGE_TAKEN);
            visual.add_effect(VisualEffect::poison_pulse(self.player.x, self.player.y));
            visual.add_effect(VisualEffect::player_damage_number(
                self.player.x,
                self.player.y,
                1,
            ));
            if self.player.stats.hp <= 0 {
                self.player.alive = false;
                let epitaph = narrative::death_epitaph(
                    self.player.depth,
                    self.stats.kills,
                    Some("poison"),
                    &mut self.rng,
                );
                self.messages.push(epitaph, MsgColor::DAMAGE_TAKEN);
                self.msg("You succumb to poison!", MsgColor::DAMAGE_TAKEN);
                self.status = GameStatus::Dead;
                crate::logging::game_event(GameEvent::Death {
                    turns: self.stats.turns,
                    depth: self.player.depth,
                    killed_by: Some("poison".into()),
                });
                return;
            }
            if self.player.poison_ticks <= 0 {
                self.player.poisoned = false;
                self.msg("The poison wears off.", MsgColor::HEALING);
            }
        }

        // Monster poison
        for i in 0..self.monsters.len() {
            if !self.monsters[i].alive || !self.monsters[i].poisoned {
                continue;
            }
            self.monsters[i].stats.hp -= 1;
            self.monsters[i].poison_ticks -= 1;
            if self.monsters[i].stats.hp <= 0 {
                self.monsters[i].alive = false;
                continue;
            }
            if self.monsters[i].poison_ticks <= 0 {
                self.monsters[i].poisoned = false;
            }
        }

        // ── Projectile processing ──
        // Advance existing projectiles and check collisions
        let mut i = 0;
        while i < self.projectiles.len() {
            let proj = &mut self.projectiles[i];
            proj.x += proj.dx;
            proj.y += proj.dy;
            proj.lifetime -= 1;

            // Check wall collision
            let remove = if self.map.blocks_fov(proj.x, proj.y) {
                true
            // Check player collision
            } else if proj.x == self.player.x && proj.y == self.player.y {
                if self.rules_version > 0 && self.player.evasion_turns > 0 {
                    self.player.evasion_turns = 0;
                    self.msg(
                        "The dark bolt tears through your fading afterimage.",
                        MsgColor::DIM,
                    );
                    true
                } else {
                    self.player.stats.hp -= proj.damage;
                    self.stats.dmg_taken += proj.damage;
                    visual.trigger_damage_flash();
                    visual.add_effect(VisualEffect::player_damage_number(
                        self.player.x,
                        self.player.y,
                        proj.damage,
                    ));
                    visual.add_effect(VisualEffect::monster_attack_flash(
                        self.player.x,
                        self.player.y,
                    ));
                    self.msg("A dark bolt strikes you!", MsgColor::DAMAGE_TAKEN);
                    if self.player.stats.hp <= 0 {
                        self.player.alive = false;
                        let epitaph = narrative::death_epitaph(
                            self.player.depth,
                            self.stats.kills,
                            Some("a necromancer bolt"),
                            &mut self.rng,
                        );
                        self.messages.push(epitaph, MsgColor::DAMAGE_TAKEN);
                        self.messages
                            .push("You have died...", MsgColor::DAMAGE_TAKEN);
                        self.status = GameStatus::Dead;
                        self.monsters.retain(|m| m.alive);
                        crate::logging::game_event(GameEvent::Death {
                            turns: self.stats.turns,
                            depth: self.player.depth,
                            killed_by: Some("necromancer bolt".into()),
                        });
                        self.projectiles.clear();
                        return;
                    }
                    true
                }
            } else {
                proj.lifetime <= 0
            };

            if remove {
                self.projectiles.remove(i);
            } else {
                i += 1;
            }
        }

        // ── Decrement monster cooldowns ──
        for m in self.monsters.iter_mut() {
            if m.alive && m.attack_cooldown > 0 {
                m.attack_cooldown -= 1;
            }
        }

        // ── Necromancer projectile spawning ──
        for mi in 0..self.monsters.len() {
            let m = &self.monsters[mi];
            if !m.alive || m.kind != MonsterKind::Necromancer || m.attack_cooldown > 0 {
                continue;
            }
            let can_see = self.map.visible[self.map.idx(m.x, m.y)];
            if !can_see {
                continue;
            }
            let dsq = (m.x - self.player.x).pow(2) + (m.y - self.player.y).pow(2);
            let max_range = 8;
            if dsq > (max_range * max_range) || dsq <= 2 {
                continue; // too far or too close
            }

            // Determine fire direction toward player (one of 8 fixed directions)
            let (dx, dy) = projectile_direction(m.x, m.y, self.player.x, self.player.y);

            // Spawn 1 or 2 projectiles
            let bolt_count = if self.rng.gen_bool(0.35) { 2 } else { 1 };
            for b in 0..bolt_count {
                let (odx, ody) = if b == 1 {
                    // Second bolt gets a 45° clockwise offset
                    rotate_direction_45(dx, dy)
                } else {
                    (dx, dy)
                };
                self.projectiles.push(Projectile {
                    x: m.x,
                    y: m.y,
                    dx: odx,
                    dy: ody,
                    damage: 6,
                    lifetime: 8,
                });
                visual.add_effect(VisualEffect::projectile_launch(m.x, m.y, odx, ody));
            }
            self.monsters[mi].attack_cooldown = 3 + self.rng.gen_range(0..=2);
            self.msg(
                format!(
                    "The necromancer hurls {} dark bolt{}!",
                    bolt_count,
                    if bolt_count > 1 { "s" } else { "" }
                ),
                MsgColor::DAMAGE_TAKEN,
            );
        }

        self.rebuild_occupancy();

        let mut decisions: Vec<(usize, i32, i32)> = Vec::new();
        let mut chases: Vec<(usize, i32, i32, bool)> = Vec::new();
        let mut first_sight_msgs: Vec<String> = Vec::new();
        let mut windups: Vec<(usize, i32, i32, String)> = Vec::new();
        let mut clear_windups: Vec<usize> = Vec::new();
        let mut clear_staggers: Vec<usize> = Vec::new();
        let mut alert_updates: Vec<usize> = Vec::new();
        let mut decay_alerts: Vec<usize> = Vec::new();

        for (i, m) in self.monsters.iter().enumerate() {
            if !m.alive {
                continue;
            }
            if m.staggered > 0 {
                clear_staggers.push(i);
                continue;
            }

            let can_see = self.map.visible[self.map.idx(m.x, m.y)];
            if can_see && !self.seen_monsters.contains(&m.kind) {
                self.seen_monsters.insert(m.kind);
                if let Some(line) = narrative::monster_first_sight(m.kind) {
                    first_sight_msgs.push(line.to_string());
                }
            }
            if can_see {
                alert_updates.push(i);
            }

            use monster_ai::AiStrategy;
            let (strategy, _ability) = monster_ai::monster_profile(m.kind);

            // Stationary monsters must be awakened before they act at all.
            if strategy == AiStrategy::Stationary && !m.awakened {
                continue;
            }

            // Awakened monsters ignore their normal act_chance.
            let act_chance = if self.player.crown_held {
                crown_pursuit_act_chance(m.kind.act_chance(), self.player.depth)
            } else {
                m.kind.act_chance()
            };
            let act = m.awakened
                || self.challenge_mode == ChallengeMode::Relentless
                || self.rng.gen_bool(act_chance);
            if !act {
                continue;
            }

            if can_see {
                let dsq = (m.x - self.player.x).pow(2) + (m.y - self.player.y).pow(2);
                if m.windup > 0 && dsq > 2 {
                    clear_windups.push(i);
                }
                if matches!(m.kind, MonsterKind::Rat | MonsterKind::Goblin)
                    && m.stats.hp * 10 <= m.stats.max_hp * 3
                {
                    let current_distance =
                        (m.x - self.player.x).abs() + (m.y - self.player.y).abs();
                    let mut retreat = None;
                    let mut best_distance = current_distance;
                    for (dx, dy) in [(0, -1), (0, 1), (-1, 0), (1, 0)] {
                        let (nx, ny) = (m.x + dx, m.y + dy);
                        let distance = (nx - self.player.x).abs() + (ny - self.player.y).abs();
                        if distance > best_distance && self.cell_free(nx, ny, i, false) {
                            retreat = Some((i, dx, dy));
                            best_distance = distance;
                        }
                    }
                    if let Some(retreat) = retreat {
                        decisions.push(retreat);
                        continue;
                    }
                }
                if dsq <= 2 {
                    if self.rules_version > 0 && self.player.evasion_turns > 0 {
                        self.player.evasion_turns = 0;
                        visual.add_effect(VisualEffect::actor_step(
                            self.player.x,
                            self.player.y,
                            (self.player.x - m.x).signum(),
                            (self.player.y - m.y).signum(),
                        ));
                        self.messages.push(
                            format!("The {} strikes only your afterimage.", m.kind.name()),
                            MsgColor::DIM,
                        );
                        continue;
                    }
                    if telegraphs_heavy_attack(m.kind) && m.windup == 0 {
                        let warning = if self.rules_version > 0
                            && m.kind == MonsterKind::Guardian
                            && m.stats.hp * 2 <= m.stats.max_hp
                        {
                            "Crown chains uncoil. Break the reach or evade!".to_string()
                        } else {
                            format!("The {} draws back for a crushing blow!", m.kind.name())
                        };
                        windups.push((i, m.x, m.y, warning));
                        continue;
                    }
                    let heavy_followthrough = m.windup > 0;
                    if m.windup > 0 {
                        clear_windups.push(i);
                    }
                    let attack_power = monster_attack_power(m, self.rules_version);
                    let mut dmg = combat::resolve_attack(attack_power, self.player.total_defense());
                    if m.kind == MonsterKind::Wraith
                        && self.player.armor == Some(ItemKind::Graveward)
                    {
                        dmg = (dmg - 2).max(1);
                    }
                    if self.player.guarding {
                        let unguarded = combat::resolve_attack(
                            attack_power,
                            self.player.total_defense().saturating_sub(2),
                        );
                        self.stats.damage_guarded += unguarded.saturating_sub(dmg);
                        if self.rules_version > 0 {
                            let guard_cost = ((attack_power + 2) / 3).clamp(1, 4) as u8;
                            if !Self::spend_stamina_tracked(
                                &mut self.player,
                                &mut self.stats,
                                self.rules_version,
                                guard_cost,
                            ) {
                                self.player.counter_turns = 0;
                                dmg += 2;
                                if self.rules_version >= 2 {
                                    self.stats.guard_breaks += 1;
                                }
                                visual.start_player_action(PlayerActionKind::Hit);
                                self.messages.push(
                                    "Your guard breaks under the impact!",
                                    MsgColor::DAMAGE_TAKEN,
                                );
                            } else {
                                self.player.counter_turns = 2;
                            }
                        }
                    }
                    self.player.stats.hp -= dmg;
                    self.stats.dmg_taken += dmg;
                    visual.trigger_damage_flash();
                    let attack_msg = narrative::monster_hit_flavour(m.kind, dmg, &mut self.rng);

                    crate::logging::game_event(GameEvent::Attack {
                        attacker: m.kind.name().to_string(),
                        defender: "player".into(),
                        damage: dmg,
                        remaining_hp: self.player.stats.hp,
                    });
                    visual.add_effect(VisualEffect::monster_attack_flash(m.x, m.y));
                    visual.add_effect(VisualEffect::player_damage_number(
                        self.player.x,
                        self.player.y,
                        dmg,
                    ));

                    let spider_poison = m.kind == MonsterKind::Spider
                        && !self.player.poisoned
                        && self.player.ward_turns <= 0
                        && self.rng.gen_bool(0.4);
                    if spider_poison {
                        self.player.poisoned = true;
                        self.player.poison_ticks = 5;
                        visual.add_effect(VisualEffect::poison_pulse(self.player.x, self.player.y));
                    }
                    self.messages.push(attack_msg, MsgColor::DAMAGE_TAKEN);
                    if spider_poison {
                        self.messages
                            .push("You are poisoned!", MsgColor::DAMAGE_TAKEN);
                    }
                    if self.rules_version > 0
                        && heavy_followthrough
                        && m.kind == MonsterKind::Guardian
                        && m.stats.hp * 2 <= m.stats.max_hp
                    {
                        Self::spend_stamina_tracked(
                            &mut self.player,
                            &mut self.stats,
                            self.rules_version,
                            2,
                        );
                        self.messages.push(
                            "Crown chains wrench away 2 stamina!",
                            MsgColor::DAMAGE_TAKEN,
                        );
                    }

                    // ── Monster special abilities ──
                    if let Some(special_msg) =
                        monster_ai::apply_special_ability(m, &mut self.player, &mut self.rng)
                    {
                        self.messages.push(special_msg, MsgColor::DAMAGE_TAKEN);
                    }
                    if self.player.stats.hp <= 0 {
                        self.player.alive = false;
                        let killer_name = m.kind.name().to_string();
                        let epitaph = narrative::death_epitaph(
                            self.player.depth,
                            self.stats.kills,
                            Some(&killer_name),
                            &mut self.rng,
                        );
                        self.messages.push(epitaph, MsgColor::DAMAGE_TAKEN);
                        self.messages
                            .push("You have died...", MsgColor::DAMAGE_TAKEN);
                        self.status = GameStatus::Dead;
                        self.monsters.retain(|m| m.alive);
                        crate::logging::game_event(GameEvent::Death {
                            turns: self.stats.turns,
                            depth: self.player.depth,
                            killed_by: Some(killer_name),
                        });
                        return;
                    }
                } else if strategy != AiStrategy::Ranged
                    && strategy != AiStrategy::Stationary
                    && dsq <= (FOV_RADIUS * 2).pow(2)
                {
                    // Only chase/flee strategy monsters move toward the player.
                    // Ranged monsters (necromancers) use projectiles instead.
                    // Stationary monsters never chase.
                    chases.push((i, m.x, m.y, m.kind == MonsterKind::Wraith));
                }
            } else if self.player.crown_held
                && strategy == AiStrategy::Chase
                && (m.x - self.player.x).abs() + (m.y - self.player.y).abs() <= 12
            {
                chases.push((i, m.x, m.y, m.kind == MonsterKind::Wraith));
            } else if m.alert_turns > 0 {
                if m.windup > 0 {
                    clear_windups.push(i);
                }
                decay_alerts.push(i);
                let dx = (m.last_seen_player_x - m.x).signum();
                let dy = (m.last_seen_player_y - m.y).signum();
                for (step_x, step_y) in [(dx, 0), (0, dy)] {
                    if (step_x, step_y) == (0, 0) {
                        continue;
                    }
                    let (nx, ny) = (m.x + step_x, m.y + step_y);
                    if self.cell_free(nx, ny, i, m.kind == MonsterKind::Wraith) {
                        decisions.push((i, step_x, step_y));
                        break;
                    }
                }
            } else if self.rng.gen_bool(0.5) {
                let dirs = [(0, 1), (0, -1), (1, 0), (-1, 0)];
                let (dx, dy) = dirs[self.rng.gen_range(0..4)];
                let (nx, ny) = (m.x + dx, m.y + dy);
                if self.cell_free(nx, ny, i, m.kind == MonsterKind::Wraith) {
                    decisions.push((i, dx, dy));
                }
            }
        }

        for msg in first_sight_msgs {
            self.msg(msg, MsgColor::DIM);
        }
        for index in clear_windups {
            if let Some(monster) = self.monsters.get_mut(index) {
                monster.windup = 0;
            }
        }
        for index in clear_staggers {
            if let Some(monster) = self.monsters.get_mut(index) {
                monster.staggered = monster.staggered.saturating_sub(1);
            }
        }
        for index in alert_updates {
            if let Some(monster) = self.monsters.get_mut(index) {
                monster.last_seen_player_x = self.player.x;
                monster.last_seen_player_y = self.player.y;
                monster.alert_turns = if self.player.crown_held { 8 } else { 5 };
            }
        }
        for index in decay_alerts {
            if let Some(monster) = self.monsters.get_mut(index) {
                monster.alert_turns = monster.alert_turns.saturating_sub(1);
            }
        }
        for (index, x, y, message) in windups {
            if let Some(monster) = self.monsters.get_mut(index) {
                monster.windup = 1;
            }
            visual.add_effect(VisualEffect::monster_attack_flash(x, y));
            self.msg(message, MsgColor::DAMAGE_TAKEN);
        }

        crate::ai::compute_flow(
            &mut self.flow_dist,
            &self.map,
            &self.occupancy,
            self.player.x,
            self.player.y,
        );

        for (i, mx, my, is_wraith) in &chases {
            let best = crate::ai::flow_best_step(
                &self.flow_dist,
                &self.map,
                &self.occupancy,
                *i,
                *mx,
                *my,
                *is_wraith,
            );
            if let Some((nx, ny)) = best {
                decisions.push((*i, nx - mx, ny - my));
            } else if *is_wraith {
                // Wraith flow may be blocked by walls — fall back to a greedy
                // step directly toward the player that ignores walls.
                let dx = (self.player.x - mx).signum();
                let dy = (self.player.y - my).signum();
                for &(odx, ody) in &[(dx, 0), (0, dy)] {
                    let (nx, ny) = (mx + odx, my + ody);
                    if self.map.in_bounds(nx, ny)
                        && !(nx == self.player.x && ny == self.player.y)
                        && self
                            .occupancy
                            .get(self.map.idx(nx, ny))
                            .copied()
                            .flatten()
                            .is_none_or(|occ| occ == *i)
                    {
                        decisions.push((*i, odx, ody));
                        break;
                    }
                }
            }
        }

        for i in 0..self.monsters.len() {
            if !self.monsters[i].alive {
                continue;
            }
            if i < prev_hp.len() && self.monsters[i].stats.hp < prev_hp[i] {
                visual.add_effect(VisualEffect::attack_flash(
                    self.monsters[i].x,
                    self.monsters[i].y,
                ));
            }
        }

        for (i, dx, dy) in &decisions {
            let m = &self.monsters[*i];
            let old_idx = self.map.idx(m.x, m.y);
            let nx = m.x + dx;
            let ny = m.y + dy;
            let new_idx = self.map.idx(nx, ny);
            if self.cell_free(nx, ny, *i, m.kind == MonsterKind::Wraith) {
                self.occupancy[old_idx] = None;
                self.occupancy[new_idx] = Some(*i);
                let m = &mut self.monsters[*i];
                m.x += dx;
                m.y += dy;
                visual.add_effect(VisualEffect::actor_step(m.x, m.y, *dx, *dy));
            }
        }

        let web_victims = self
            .monsters
            .iter()
            .enumerate()
            .filter(|(_, monster)| {
                monster.alive
                    && !matches!(
                        monster.kind,
                        MonsterKind::Spider | MonsterKind::Bat | MonsterKind::Wraith
                    )
                    && self.map.in_bounds(monster.x, monster.y)
                    && self.map.tiles[self.map.idx(monster.x, monster.y)] == Tile::Web
            })
            .map(|(index, monster)| (index, monster.x, monster.y))
            .collect::<Vec<_>>();
        for (index, x, y) in web_victims {
            let tile_index = self.map.idx(x, y);
            self.map.tiles[tile_index] = Tile::Floor;
            self.monsters[index].staggered = 1;
            self.msg("Webbing tangles a monster's limbs.", MsgColor::DIM);
        }

        let trap_victims = self
            .monsters
            .iter()
            .enumerate()
            .filter(|(_, monster)| {
                monster.alive
                    && !matches!(monster.kind, MonsterKind::Bat | MonsterKind::Wraith)
                    && self.map.in_bounds(monster.x, monster.y)
                    && matches!(
                        self.map.tiles[self.map.idx(monster.x, monster.y)],
                        Tile::TrapHidden | Tile::TrapRevealed
                    )
            })
            .map(|(index, monster)| (index, monster.x, monster.y))
            .collect::<Vec<_>>();
        for (index, x, y) in trap_victims {
            let tile_index = self.map.idx(x, y);
            self.map.tiles[tile_index] = Tile::Floor;
            self.monsters[index].stats.hp -= 5;
            visual.add_effect(VisualEffect::attack_flash(x, y));
            self.msg(
                "A floor trap snaps shut on a monster!",
                MsgColor::DAMAGE_DEALT,
            );
            if self.monsters[index].stats.hp <= 0 {
                self.kill_monster(index, visual);
            }
        }

        self.monsters.retain(|m| m.alive);
        self.player.guarding = false;
        self.player.evasion_turns = 0;
        if self.rules_version > 0 {
            self.player.recover_stamina(1);
            self.player.counter_turns = self.player.counter_turns.saturating_sub(1);
        }

        // ── Lantern oil timer ticks once per turn, regardless of FOV dirty. ──
        if self.player.lantern_turns > 0 {
            self.player.lantern_turns -= 1;
        }
        if self.player.focus_turns > 0 {
            self.player.focus_turns -= 1;
        }
        if self.player.ward_turns > 0 {
            self.player.ward_turns -= 1;
        }

        if self.fov_dirty != FovDirty::Clean {
            self.refresh_fov();
        }

        // Ambient narrative
        if self.rng.gen_bool(0.02) {
            if let Some(line) = narrative::ambient_line(self.player.depth, &mut self.rng) {
                self.msg(line, MsgColor::DIM);
            }
        }
        // Camp ambient lines — fire when player is in or near a camp room
        for camp in &self.camps {
            if let Some(room) = self.map.rooms.get(camp.room_idx) {
                let in_room = self.player.x >= room.x
                    && self.player.x < room.x + room.w
                    && self.player.y >= room.y
                    && self.player.y < room.y + room.h;
                if in_room {
                    if let Some(line) = narrative::camp_ambient(camp.kind, &mut self.rng) {
                        self.msg(line, MsgColor::DIM);
                    }
                    break; // Only one camp ambient per turn
                }
            }
        }
        // Crown whispers only fire when the Crown is actually on the floor
        let crown_on_floor = self.player.depth == MAX_DEPTH
            && !self.player.crown_held
            && self.items.iter().any(|i| i.kind == ItemKind::CrownOfDepth);
        if crown_on_floor {
            if let Some(line) = narrative::crown_whisper(&mut self.rng) {
                self.msg(line, MsgColor::LOOT);
            }
        }
    }

    // ── Auto explore ──
    pub(crate) fn auto_explore(&mut self, visual: &mut VisualState) {
        self.explore_visited.fill(false);
        let mut queue = std::collections::VecDeque::new();
        let start = (self.player.x, self.player.y);
        self.explore_visited[self.map.idx(start.0, start.1)] = true;
        queue.push_back(start);
        let mut target = None;

        'search: while let Some((cx, cy)) = queue.pop_front() {
            let ci = self.map.idx(cx, cy);
            if !self.map.explored[ci] && self.map.is_walkable(cx, cy) && (cx, cy) != start {
                target = Some((cx, cy));
                break;
            }
            for (dx, dy) in &[(0, -1), (0, 1), (-1, 0), (1, 0)] {
                let (nx, ny) = (cx + dx, cy + dy);
                // Check bounds BEFORE calling map.idx().
                if !self.map.in_bounds(nx, ny) {
                    continue;
                }
                let ni = self.map.idx(nx, ny);
                if self.map.tiles[ni] == Tile::DoorClosed
                    || (self.map.tiles[ni] == Tile::LockedDoor
                        && (self.player.inventory.contains(&ItemKind::IronKey)
                            || self.player.weapon == Some(ItemKind::Warhammer)))
                {
                    target = Some((nx, ny));
                    break 'search;
                }
                if self.map.is_walkable(nx, ny) && !self.explore_visited[ni] {
                    self.explore_visited[ni] = true;
                    queue.push_back((nx, ny));
                }
            }
        }

        let (tx, ty) = match target {
            Some(t) => t,
            None => {
                self.msg("Nothing left to explore.", MsgColor::DIM);
                return;
            }
        };

        self.astar.reset();
        let w = self.map.width;
        let h = self.map.height;
        if let Some((nx, ny)) =
            pathfinding::a_star_step(&mut self.astar, w, h, start, (tx, ty), 500, |x, y| {
                (x, y) == (tx, ty)
                    || (self.map.is_walkable(x, y)
                        && !self.npcs.iter().any(|npc| npc.x == x && npc.y == y)
                        && !self
                            .monsters
                            .iter()
                            .any(|m| m.alive && m.x == x && m.y == y))
            })
        {
            // Route through normal movement processing so traps, auto-pickup,
            // doors, and collision side-effects all work correctly.
            let dx = nx - self.player.x;
            let dy = ny - self.player.y;
            self.process_turn(Action::Move(dx, dy), visual);
        } else {
            self.msg("Can't find a path there.", MsgColor::DIM);
        }
    }

    // ── Save / Load ──
    pub fn save_game(&self) -> std::io::Result<()> {
        let data = crate::save::SaveData::from_state(self);
        let json = serde_json::to_string_pretty(&data).map_err(std::io::Error::other)?;
        std::fs::write(crate::save::SAVE_PATH, json)
    }

    pub fn load_game(&mut self) {
        let json = match std::fs::read_to_string(crate::save::SAVE_PATH) {
            Ok(j) => j,
            Err(e) => {
                self.msg(format!("No save file found ({})", e), MsgColor::DIM);
                return;
            }
        };
        let data: crate::save::SaveData = match serde_json::from_str(&json) {
            Ok(d) => d,
            Err(e) => {
                self.msg(
                    format!("Save file corrupted: {}", e),
                    MsgColor::DAMAGE_TAKEN,
                );
                return;
            }
        };
        if let Err(e) = data.validate() {
            self.msg(format!("Cannot load save: {}", e), MsgColor::DAMAGE_TAKEN);
            return;
        }

        self.seed = data.seed;
        self.player = data.player;
        if let Some(weapon) = self.player.weapon {
            if self.player.weapon_durability == 0 && weapon.max_durability() > 0 {
                self.player.weapon_durability = weapon.max_durability();
            }
        }
        self.map = data.map;
        self.monsters = data.monsters;
        self.items = data.items;
        self.npcs = data.npcs;
        self.projectiles = data.projectiles;
        self.landmarks = data.landmarks;
        self.used_landmarks = data.used_landmarks.into_iter().collect();
        self.camps = data.camps;
        self.dialog_active = false;
        self.dialog_text.clear();
        self.dialog_npc_name.clear();
        self.dialog_npc_idx = None;
        self.dialog_conv_node = 0;
        self.dialog_show_choices = false;
        self.dialog_choices.clear();
        self.dialog_campaign_npc = None;
        self.log_scroll = 0;
        self.seen_monsters = data.seen_monsters.into_iter().collect();
        self.defeated_monsters = data.defeated_monsters.into_iter().collect();
        self.status = data.status;
        self.stats = data.stats;
        self.messages.set_messages(data.messages);
        self.auto_pickup = data.auto_pickup;
        self.game_mode = data.game_mode;
        self.challenge_mode = data.challenge_mode;
        self.rules_version = crate::save::rules_version_for_save(data.version);
        self.campaign = data.campaign;
        self.inventory_cursor = 0;
        self.preparation_menu = None;

        self.rng = StdRng::seed_from_u64(self.seed);
        for _ in 0..self.stats.turns {
            let _: u64 = self.rng.gen();
        }

        self.astar = pathfinding::AStarWorkspace::new(self.map.width, self.map.height);
        let size = (self.map.width * self.map.height) as usize;
        self.explore_visited = vec![false; size];
        self.occupancy = vec![None; size];
        self.flow_dist = vec![i32::MAX; size];

        self.fov_dirty = FovDirty::LevelChanged;
        self.refresh_fov();
        self.msg("Game loaded.", MsgColor::SYSTEM);

        if self.status == GameStatus::Menu {
            self.status = GameStatus::Playing;
        }
    }

    pub fn interact_facing(&mut self, dx: i32, dy: i32, visual: &mut VisualState) {
        let here = self.map.tiles[self.map.idx(self.player.x, self.player.y)];
        if self
            .items
            .iter()
            .any(|item| (item.x, item.y) == (self.player.x, self.player.y))
            || here == Tile::LanternDead
        {
            self.process_turn(Action::PickUp, visual);
            return;
        }
        if here == Tile::StairsDown {
            self.process_turn(Action::Descend, visual);
            return;
        }
        if here == Tile::StairsUp {
            self.process_turn(Action::Ascend, visual);
            return;
        }

        let front = (self.player.x + dx, self.player.y + dy);
        if self.npcs.iter().any(|npc| (npc.x, npc.y) == front) {
            self.process_turn(Action::TalkAt(front.0, front.1), visual);
            return;
        }
        if self.map.in_bounds(front.0, front.1) {
            let tile = self.map.tiles[self.map.idx(front.0, front.1)];
            let enemy = self
                .monsters
                .iter()
                .any(|monster| monster.alive && (monster.x, monster.y) == front);
            if matches!(
                tile,
                Tile::DoorClosed | Tile::LockedDoor | Tile::Urn | Tile::Barricade
            ) || enemy
            {
                self.process_turn(Action::Move(dx, dy), visual);
                return;
            }
        }

        let replay_turn = self.stats.turns;
        let replay_len = self
            .replay_data
            .as_ref()
            .map_or(0, |replay| replay.actions.len());
        if self.try_rest_at_camp(visual) || self.try_interact_landmark(visual) {
            if let Some(replay) = &mut self.replay_data {
                if replay.actions.len() != replay_len {
                    return;
                }
                replay.record_repr(
                    replay_turn,
                    crate::replay::ActionRepr::InteractEnvironment(dx, dy),
                );
            }
            return;
        }

        self.msg("Nothing here responds.", MsgColor::DIM);
    }

    pub fn toggle_facing_door(&mut self, dx: i32, dy: i32, visual: &mut VisualState) {
        let (x, y) = (self.player.x + dx, self.player.y + dy);
        if !self.map.in_bounds(x, y) {
            self.msg("There is no door there.", MsgColor::DIM);
            return;
        }
        let index = self.map.idx(x, y);
        match self.map.tiles[index] {
            Tile::DoorClosed => {
                self.map.tiles[index] = Tile::DoorOpen;
                self.fov_dirty = FovDirty::MapChanged;
                self.msg("You ease the door open.", MsgColor::SYSTEM);
                self.end_turn(visual);
            }
            Tile::LockedDoor => {
                self.open_locked_door(x, y, visual);
            }
            Tile::DoorOpen => {
                let occupied = self
                    .monsters
                    .iter()
                    .any(|monster| monster.alive && (monster.x, monster.y) == (x, y))
                    || self.npcs.iter().any(|npc| (npc.x, npc.y) == (x, y))
                    || self
                        .projectiles
                        .iter()
                        .any(|projectile| (projectile.x, projectile.y) == (x, y));
                if occupied {
                    self.msg("Something blocks the doorway.", MsgColor::DIM);
                    return;
                }
                self.map.tiles[index] = Tile::DoorClosed;
                self.fov_dirty = FovDirty::MapChanged;
                self.msg("You pull the door shut.", MsgColor::SYSTEM);
                self.end_turn(visual);
            }
            _ => self.msg("There is no door there.", MsgColor::DIM),
        }
    }

    fn open_locked_door(&mut self, x: i32, y: i32, visual: &mut VisualState) {
        let Some(key_slot) = self
            .player
            .inventory
            .iter()
            .position(|item| *item == ItemKind::IronKey)
        else {
            if self.player.weapon == Some(ItemKind::Warhammer) {
                let index = self.map.idx(x, y);
                self.map.tiles[index] = Tile::DoorOpen;
                self.fov_dirty = FovDirty::MapChanged;
                self.msg(
                    "The iron lock bursts beneath the hammer.",
                    MsgColor::DAMAGE_DEALT,
                );
                self.stats.doors_forced += 1;
                visual.add_effect(VisualEffect::death_burst(x, y));
                self.end_turn(visual);
            } else {
                self.msg(
                    "The door is locked. An iron key or heavy hammer might open it.",
                    MsgColor::DIM,
                );
            }
            return;
        };
        self.player.inventory.remove(key_slot);
        let index = self.map.idx(x, y);
        self.map.tiles[index] = Tile::DoorOpen;
        self.fov_dirty = FovDirty::MapChanged;
        self.msg(
            "The iron key turns once, then snaps in the lock.",
            MsgColor::SYSTEM,
        );
        self.end_turn(visual);
    }

    fn try_technique(&mut self, dx: i32, dy: i32, visual: &mut VisualState) {
        if self.rules_version < 2 {
            self.msg("Techniques are unavailable in legacy rules.", MsgColor::DIM);
            return;
        }
        if dx.abs() + dy.abs() != 1 {
            return;
        }

        let technique = TechniqueKind::from_loadout(&self.player);
        let (px, py) = (self.player.x, self.player.y);
        let right = (-dy, dx);
        let mut cells = match technique {
            TechniqueKind::Kick | TechniqueKind::ShieldBash => vec![(px + dx, py + dy)],
            TechniqueKind::Lunge => vec![(px + dx, py + dy), (px + dx * 2, py + dy * 2)],
            TechniqueKind::Sweep => vec![
                (px + dx, py + dy),
                (px + dx + right.0, py + dy + right.1),
                (px + dx - right.0, py + dy - right.1),
            ],
            TechniqueKind::Earthbreak => {
                vec![(px + dx * 2, py + dy * 2), (px + dx, py + dy)]
            }
            TechniqueKind::CinderArc => vec![
                (px + dx, py + dy),
                (px + dx * 2 - right.0, py + dy * 2 - right.1),
                (px + dx * 2, py + dy * 2),
                (px + dx * 2 + right.0, py + dy * 2 + right.1),
            ],
        };
        let front = (px + dx, py + dy);
        if self.map.blocks_fov(front.0, front.1) {
            self.msg("No clear line for that technique.", MsgColor::DIM);
            return;
        }
        cells.retain(|&(x, y)| self.map.in_bounds(x, y) && !self.map.blocks_fov(x, y));

        if technique == TechniqueKind::Lunge {
            let adjacent_has_target = self.monster_at(cells[0].0, cells[0].1).is_some();
            if adjacent_has_target {
                cells.truncate(1);
            } else {
                let middle_tile = self.map.tiles[self.map.idx(cells[0].0, cells[0].1)];
                let middle_free = self.map.is_walkable(cells[0].0, cells[0].1)
                    && !matches!(middle_tile, Tile::ShallowWater | Tile::Web)
                    && self.monster_at(cells[0].0, cells[0].1).is_none()
                    && !self.npcs.iter().any(|npc| (npc.x, npc.y) == cells[0]);
                let far_has_target = self.monster_at(cells[1].0, cells[1].1).is_some();
                if !middle_free || !far_has_target {
                    self.msg("No foe stands within lunge reach.", MsgColor::DIM);
                    return;
                }
                cells.remove(0);
            }
        }

        let target_cells = cells
            .iter()
            .copied()
            .filter(|&(x, y)| self.monster_at(x, y).is_some())
            .collect::<Vec<_>>();
        let burns_web = technique == TechniqueKind::CinderArc
            && cells.iter().any(|&(x, y)| {
                self.map.in_bounds(x, y) && self.map.tiles[self.map.idx(x, y)] == Tile::Web
            });
        if target_cells.is_empty() && !burns_web {
            self.msg(
                format!("{} finds no target.", technique.name()),
                MsgColor::DIM,
            );
            return;
        }
        if self.player.stamina < technique.cost() {
            self.msg(
                "You are too winded for that technique.",
                MsgColor::DAMAGE_TAKEN,
            );
            return;
        }

        self.spend_player_stamina(technique.cost());
        self.stats.techniques_used += 1;
        visual.start_player_action(PlayerActionKind::Technique(technique));

        if technique == TechniqueKind::Lunge && target_cells[0] == (px + dx * 2, py + dy * 2) {
            let from = (self.player.x, self.player.y);
            self.player.x += dx;
            self.player.y += dy;
            visual.add_effect(VisualEffect::actor_step(
                self.player.x,
                self.player.y,
                dx,
                dy,
            ));
            self.on_player_entered_tile(self.player.x, self.player.y, from, visual);
            if !self.player.alive {
                return;
            }
        }

        if technique == TechniqueKind::CinderArc {
            for &(x, y) in &cells {
                if !self.map.in_bounds(x, y) {
                    continue;
                }
                let tile = self.map.idx(x, y);
                if self.map.tiles[tile] == Tile::Web {
                    self.map.tiles[tile] = Tile::Floor;
                    self.fov_dirty = FovDirty::MapChanged;
                }
                visual.add_effect(VisualEffect::fire_ring(x, y));
                visual.add_decal(visual::Decal::Scorch { x, y, fade: 220 });
            }
        }

        for (x, y) in target_cells {
            let Some(index) = self.monster_at(x, y) else {
                continue;
            };
            let (bonus, stagger, fixed_damage) = match technique {
                TechniqueKind::Kick => (0, 1, Some(1)),
                TechniqueKind::ShieldBash => (0, 3, None),
                TechniqueKind::Lunge => (1, 1, None),
                TechniqueKind::Sweep => (0, 1, None),
                TechniqueKind::Earthbreak => (2, 2, None),
                TechniqueKind::CinderArc => (2, 1, None),
            };
            self.hit_with_technique(index, technique, bonus, stagger, fixed_damage, visual);
            if self.monsters[index].alive
                && matches!(technique, TechniqueKind::Kick | TechniqueKind::Earthbreak)
            {
                self.push_monster(index, dx, dy, visual);
            }
        }

        if self.player.weapon.is_some() {
            self.wear_weapon(2);
        }
        self.msg(
            format!("You commit to {}.", technique.name()),
            MsgColor::DAMAGE_DEALT,
        );
        self.end_turn(visual);
    }

    fn monster_at(&self, x: i32, y: i32) -> Option<usize> {
        self.monsters
            .iter()
            .position(|monster| monster.alive && (monster.x, monster.y) == (x, y))
    }

    fn spend_player_stamina(&mut self, amount: u8) -> bool {
        Self::spend_stamina_tracked(
            &mut self.player,
            &mut self.stats,
            self.rules_version,
            amount,
        )
    }

    fn spend_stamina_tracked(
        player: &mut Player,
        stats: &mut RunStats,
        rules_version: u8,
        amount: u8,
    ) -> bool {
        let before = player.stamina;
        let enough = player.spend_stamina(amount);
        if rules_version >= 2 {
            stats.stamina_spent = stats
                .stamina_spent
                .saturating_add(u32::from(before - player.stamina));
        }
        enough
    }

    fn hit_with_technique(
        &mut self,
        index: usize,
        technique: TechniqueKind,
        bonus: i32,
        stagger: u8,
        fixed_damage: Option<i32>,
        visual: &mut VisualState,
    ) {
        let interrupted = self.monsters[index].windup > 0;
        let mut damage = fixed_damage.unwrap_or_else(|| {
            combat::resolve_attack(
                self.player.total_attack(),
                self.monsters[index].stats.defense,
            ) + bonus
        });
        if technique == TechniqueKind::CinderArc
            && matches!(
                self.monsters[index].kind,
                MonsterKind::Spider | MonsterKind::Slime
            )
        {
            damage += 2;
        }
        let monster = &mut self.monsters[index];
        monster.awakened = true;
        monster.stats.hp -= damage;
        monster.staggered = monster.staggered.max(stagger);
        if interrupted {
            monster.windup = 0;
            self.stats.interrupts += 1;
        }
        let (x, y) = (monster.x, monster.y);
        self.stats.dmg_dealt += damage;
        visual.add_effect(VisualEffect::attack_flash(x, y));
        visual.add_effect(VisualEffect::damage_number(x, y, damage));
        if monster.stats.hp <= 0 {
            self.kill_monster(index, visual);
        }
    }

    fn push_monster(&mut self, index: usize, dx: i32, dy: i32, visual: &mut VisualState) -> bool {
        let destination = (self.monsters[index].x + dx, self.monsters[index].y + dy);
        let blocked = self.monsters.iter().enumerate().any(|(other, monster)| {
            other != index && monster.alive && (monster.x, monster.y) == destination
        }) || self.npcs.iter().any(|npc| (npc.x, npc.y) == destination)
            || (self.player.x, self.player.y) == destination;
        if blocked || !self.map.is_walkable(destination.0, destination.1) {
            return false;
        }
        self.monsters[index].x = destination.0;
        self.monsters[index].y = destination.1;
        visual.add_effect(VisualEffect::actor_step(
            destination.0,
            destination.1,
            dx,
            dy,
        ));
        true
    }

    fn wear_weapon(&mut self, base_wear: i32) {
        if self.player.weapon.is_none() || self.player.weapon_durability <= 0 {
            return;
        }
        let wear = if self.challenge_mode == ChallengeMode::FragileGear {
            base_wear.saturating_mul(2)
        } else {
            base_wear
        };
        self.player.weapon_durability = self.player.weapon_durability.saturating_sub(wear);
        if self.player.weapon_durability == 0 {
            let broken = self.player.weapon.take().expect("weapon checked above");
            self.player.weapon_cursed = false;
            self.msg(
                format!("Your {} breaks apart!", broken.name()),
                MsgColor::DAMAGE_TAKEN,
            );
        }
    }

    pub fn listen_facing(&mut self, dx: i32, dy: i32, visual: &mut VisualState) {
        let (door_x, door_y) = (self.player.x + dx, self.player.y + dy);
        if !self.map.in_bounds(door_x, door_y)
            || self.map.tiles[self.map.idx(door_x, door_y)] != Tile::DoorClosed
        {
            self.msg("Press your ear to a closed door.", MsgColor::DIM);
            return;
        }
        let heard = self
            .monsters
            .iter()
            .filter(|monster| monster.alive)
            .filter_map(|monster| {
                let rx = monster.x - door_x;
                let ry = monster.y - door_y;
                let forward = rx * dx + ry * dy;
                let lateral = (rx * dy - ry * dx).abs();
                (forward > 0 && forward <= 8 && lateral <= forward)
                    .then_some((forward + lateral, monster.kind))
            })
            .min_by_key(|(distance, _)| *distance);
        let message = match heard.map(|(_, kind)| kind) {
            Some(MonsterKind::Spider) => "Beyond the door: a dry skitter across stone.",
            Some(MonsterKind::Skeleton | MonsterKind::Wraith) => {
                "Beyond the door: a faint scrape, then stillness."
            }
            Some(MonsterKind::Necromancer) => "Beyond the door: a low, measured whisper.",
            Some(MonsterKind::Orc | MonsterKind::Guardian | MonsterKind::Sentinel) => {
                "Beyond the door: something heavy shifts its weight."
            }
            Some(_) => "Beyond the door: quiet movement in the dark.",
            None => "Only the dungeon's breathing answers.",
        };
        self.msg(message, MsgColor::Cyan);
        self.end_turn(visual);
    }

    pub fn disarm_trap(&mut self, dx: i32, dy: i32, visual: &mut VisualState) {
        let here = (self.player.x, self.player.y);
        let front = (here.0 + dx, here.1 + dy);
        let target = [here, front].into_iter().find(|(x, y)| {
            self.map.in_bounds(*x, *y) && self.map.tiles[self.map.idx(*x, *y)] == Tile::TrapRevealed
        });
        let Some((x, y)) = target else {
            self.msg("No revealed trap is within reach.", MsgColor::DIM);
            return;
        };
        let index = self.map.idx(x, y);
        self.map.tiles[index] = Tile::Floor;
        self.stats.traps_disarmed += 1;
        self.msg(
            "You wedge the mechanism and strip its trigger.",
            MsgColor::SYSTEM,
        );
        visual.add_effect(VisualEffect::pickup_sparkle(x, y));
        self.end_turn(visual);
    }

    fn try_rest_at_camp(&mut self, _visual: &mut VisualState) -> bool {
        let position = (self.player.x, self.player.y);
        let Some(camp_index) = self.camps.iter().position(|camp| {
            self.map.rooms.get(camp.room_idx).is_some_and(|room| {
                let center = room.center();
                (center.0 - position.0).abs() + (center.1 - position.1).abs() <= 1
            })
        }) else {
            return false;
        };
        if self.monsters.iter().any(|monster| {
            monster.alive
                && self.map.in_bounds(monster.x, monster.y)
                && self.map.visible[self.map.idx(monster.x, monster.y)]
        }) {
            self.msg(
                "You cannot rest while something hunts nearby.",
                MsgColor::DAMAGE_TAKEN,
            );
            return true;
        }
        if self.camps[camp_index].rested {
            self.msg("The camp has no more rest to give.", MsgColor::DIM);
            return true;
        }
        self.preparation_menu = Some(PreparationMenu::Camp(camp_index));
        true
    }

    pub fn resolve_preparation(&mut self, choice: u8, visual: &mut VisualState) {
        let Some(menu) = self.preparation_menu else {
            return;
        };
        if let Some(replay) = &mut self.replay_data {
            replay.record_repr(
                self.stats.turns,
                crate::replay::ActionRepr::PreparationChoice { menu, choice },
            );
        }
        match menu {
            PreparationMenu::Camp(camp_index) => {
                if self.camps.get(camp_index).is_none_or(|camp| camp.rested) {
                    self.preparation_menu = None;
                    return;
                }
                match choice {
                    1 => {
                        if self.player.stats.hp >= self.player.stats.max_hp {
                            self.msg("You have no wounds to rest away.", MsgColor::DIM);
                            return;
                        }
                        let before = self.player.stats.hp;
                        let restored = (self.player.stats.max_hp / 3).max(4);
                        self.player.stats.hp = (before + restored).min(self.player.stats.max_hp);
                        self.msg(
                            format!(
                                "You rest by the low fire and recover {} HP.",
                                self.player.stats.hp - before
                            ),
                            MsgColor::LOOT,
                        );
                        visual.add_effect(VisualEffect::heal_glow(self.player.x, self.player.y));
                        self.camps[camp_index].rested = true;
                        self.preparation_menu = None;
                        for _ in 0..3 {
                            if self.player.alive {
                                self.end_turn(visual);
                            }
                        }
                    }
                    2 => {
                        self.player.focus_turns = self.player.focus_turns.max(40);
                        self.camps[camp_index].rested = true;
                        self.preparation_menu = None;
                        self.msg("You hone your stance: +1 ATK for 40 turns.", MsgColor::LOOT);
                        self.end_turn(visual);
                    }
                    3 => {
                        self.player.ward_turns = self.player.ward_turns.max(40);
                        self.player.poisoned = false;
                        self.player.poison_ticks = 0;
                        self.camps[camp_index].rested = true;
                        self.preparation_menu = None;
                        self.msg(
                            "You prepare bitter warding herbs: +1 DEF and poison protection.",
                            MsgColor::LOOT,
                        );
                        self.end_turn(visual);
                    }
                    4 => {
                        let max = self
                            .player
                            .weapon
                            .map_or(0, |weapon| weapon.max_durability());
                        if max == 0 || self.player.weapon_durability >= max {
                            self.msg("Your equipped weapon needs no repair.", MsgColor::DIM);
                            return;
                        }
                        self.player.weapon_durability = max;
                        self.camps[camp_index].rested = true;
                        self.preparation_menu = None;
                        self.msg("You repair the weapon to full durability.", MsgColor::LOOT);
                        self.end_turn(visual);
                    }
                    5 => {
                        let unknown = self
                            .player
                            .inventory
                            .iter()
                            .copied()
                            .filter(|item| !self.player.knows_item(*item))
                            .collect::<Vec<_>>();
                        if unknown.is_empty() {
                            self.msg("You carry no unidentified supplies.", MsgColor::DIM);
                            return;
                        }
                        self.player.identified_items.extend(unknown);
                        self.camps[camp_index].rested = true;
                        self.preparation_menu = None;
                        self.msg(
                            "The camp's notes identify every carried supply.",
                            MsgColor::LOOT,
                        );
                        self.end_turn(visual);
                    }
                    _ => {}
                }
            }
            PreparationMenu::Shrine(room_index) => {
                let (cost, allowed) = match choice {
                    1 => (10, self.player.stats.hp < self.player.stats.max_hp),
                    2 => (8, self.player.poisoned),
                    3 => (15, true),
                    _ => return,
                };
                if !allowed {
                    self.msg("That offering would change nothing.", MsgColor::DIM);
                    return;
                }
                if self.player.gold < cost {
                    self.msg(format!("The shrine asks for {} gold.", cost), MsgColor::DIM);
                    return;
                }
                self.player.gold -= cost;
                match choice {
                    1 => self.player.stats.hp = self.player.stats.max_hp,
                    2 => {
                        self.player.poisoned = false;
                        self.player.poison_ticks = 0;
                    }
                    3 => {
                        self.player.focus_turns = self.player.focus_turns.max(60);
                        self.player.ward_turns = self.player.ward_turns.max(60);
                    }
                    _ => unreachable!(),
                }
                self.used_landmarks.insert(room_index);
                self.preparation_menu = None;
                self.msg("The offering vanishes. The shrine answers.", MsgColor::LOOT);
                visual.add_effect(VisualEffect::heal_glow(self.player.x, self.player.y));
                self.end_turn(visual);
            }
        }
    }

    fn try_interact_landmark(&mut self, visual: &mut VisualState) -> bool {
        let position = (self.player.x, self.player.y);
        let Some((room_index, landmark)) =
            self.landmarks.iter().find_map(|(room_index, landmark)| {
                self.map
                    .rooms
                    .get(*room_index)
                    .filter(|room| {
                        let center = room.center();
                        (center.0 - position.0).abs() + (center.1 - position.1).abs() <= 1
                    })
                    .map(|_| (*room_index, *landmark))
            })
        else {
            return false;
        };

        if landmark == Landmark::BellChamber {
            self.pick_up_item(visual);
            return true;
        }
        if self.used_landmarks.contains(&room_index) {
            self.msg("Nothing more remains here.", MsgColor::DIM);
            return true;
        }
        if landmark == Landmark::Shrine {
            self.preparation_menu = Some(PreparationMenu::Shrine(room_index));
            return true;
        }

        let used = match landmark {
            Landmark::SpiderNest => {
                if self.player.inventory.len() >= 26 {
                    self.msg(
                        "You find an antidote, but your pack is full.",
                        MsgColor::DIM,
                    );
                    false
                } else {
                    self.player.inventory.push(ItemKind::Antidote);
                    self.msg(
                        "Beneath the webbing, you recover an antidote.",
                        MsgColor::LOOT,
                    );
                    true
                }
            }
            Landmark::FloodedVault => {
                self.player.gold += 20;
                if self.rng.gen_bool(0.25) {
                    self.player.poisoned = true;
                    self.player.poison_ticks = self.player.poison_ticks.max(4);
                    visual.add_effect(VisualEffect::poison_pulse(self.player.x, self.player.y));
                    self.msg(
                        "You dredge 20 gold from the black water. Something toxic bites back.",
                        MsgColor::DAMAGE_TAKEN,
                    );
                } else {
                    self.msg("You dredge 20 gold from the black water.", MsgColor::LOOT);
                }
                true
            }
            Landmark::BoneChapel => {
                let levels = self.award_xp(10);
                self.msg(
                    if levels > 0 && self.rules_version >= 2 {
                        "The dead sharpen your purpose. You gain 10 XP and a discipline choice."
                    } else if levels > 0 {
                        "The names of the dead sharpen your purpose. You gain 10 XP and a level."
                    } else {
                        "The chapel's inscriptions yield 10 XP."
                    },
                    MsgColor::Cyan,
                );
                true
            }
            Landmark::CollapsedForge => {
                let old_weapon = self.player.weapon;
                let upgraded = match old_weapon {
                    Some(ItemKind::RustySword) => Some(ItemKind::Longsword),
                    Some(ItemKind::Longsword) => Some(ItemKind::Warhammer),
                    _ => None,
                };
                if let Some(weapon) = upgraded {
                    self.player.weapon = Some(weapon);
                    if let Some(slot) = self
                        .player
                        .inventory
                        .iter()
                        .position(|item| Some(*item) == old_weapon)
                    {
                        self.player.inventory[slot] = weapon;
                    }
                    self.msg(
                        "The dead forge answers once. Your weapon is reforged.",
                        MsgColor::LOOT,
                    );
                    for monster in &mut self.monsters {
                        if monster.alive
                            && self.map.in_bounds(monster.x, monster.y)
                            && self.map.visible[self.map.idx(monster.x, monster.y)]
                        {
                            monster.awakened = true;
                        }
                    }
                    self.msg("The hammer's ringing wakes the nearby dark.", MsgColor::DIM);
                    true
                } else {
                    self.msg(
                        "The forge needs a weapon it can still improve.",
                        MsgColor::DIM,
                    );
                    false
                }
            }
            Landmark::Shrine => false,
            Landmark::BellChamber => false,
        };

        if used {
            self.used_landmarks.insert(room_index);
            visual.add_effect(VisualEffect::pickup_sparkle(self.player.x, self.player.y));
            self.end_turn(visual);
        }
        true
    }

    pub fn msg(&mut self, text: impl Into<String>, color: MsgColor) {
        self.messages.push(text, color);
    }
}

// ── App ──
pub struct App {
    terminal: DefaultTerminal,
    pub state: GameState,
    pub visual: VisualState,
    /// Show minimap overlay.
    pub show_minimap: bool,
    /// Show the controls overlay.
    pub show_help: bool,
    /// Third-person player facing direction: 0 east, 1 south, 2 west, 3 north.
    pub camera_dir: i32,
    /// Presentation-only camera angle, eased toward `camera_dir`.
    pub camera_angle: f64,
    /// Transient paced auto-walk; never persisted.
    pub auto_exploring: bool,
    pub show_bestiary: bool,
    pub reduced_flash: bool,
    pub high_contrast: bool,
    pub show_log_overlay: bool,
    pub color_filter: u8,
    pub camera_easing: u8,
    pub head_bob: bool,
    pub low_motion: bool,
    camera_bob_timer: u8,
    hints_seen: u8,
    run_history: Vec<crate::run_history::RunSummary>,
    run_recorded: bool,
    should_quit: bool,
}

pub(crate) fn camera_dir_angle(camera_dir: i32) -> f64 {
    match camera_dir.rem_euclid(4) {
        0 => 0.0,
        1 => std::f64::consts::FRAC_PI_2,
        2 => std::f64::consts::PI,
        _ => -std::f64::consts::FRAC_PI_2,
    }
}

pub(crate) fn step_camera_angle(current: f64, target: f64) -> f64 {
    step_camera_angle_rate(current, target, 0.46)
}

fn step_camera_angle_rate(current: f64, target: f64, rate: f64) -> f64 {
    let mut delta = target - current;
    while delta > std::f64::consts::PI {
        delta -= std::f64::consts::TAU;
    }
    while delta < -std::f64::consts::PI {
        delta += std::f64::consts::TAU;
    }
    if delta.abs() < 0.015 {
        target
    } else {
        current + delta * rate
    }
}

pub(crate) fn camera_dir_delta(camera_dir: i32) -> (i32, i32) {
    match camera_dir.rem_euclid(4) {
        0 => (1, 0),
        1 => (0, 1),
        2 => (-1, 0),
        _ => (0, -1),
    }
}

fn quick_usable(kind: ItemKind) -> bool {
    matches!(
        kind,
        ItemKind::HealthPotion
            | ItemKind::LargeHealthPotion
            | ItemKind::Antidote
            | ItemKind::LanternOil
            | ItemKind::ScrollOfFire
            | ItemKind::ScrollOfConfusion
            | ItemKind::ScrollOfBlink
            | ItemKind::SmokeBomb
            | ItemKind::EnchantmentScroll
            | ItemKind::LoreFragment
    )
}

fn telegraphs_heavy_attack(kind: MonsterKind) -> bool {
    matches!(
        kind,
        MonsterKind::Orc | MonsterKind::Sentinel | MonsterKind::Guardian
    )
}

fn monster_attack_power(monster: &Monster, rules_version: u8) -> i32 {
    monster.stats.attack
        + i32::from(
            rules_version > 0
                && monster.kind == MonsterKind::Guardian
                && monster.stats.hp * 2 <= monster.stats.max_hp,
        ) * 3
}

fn direction_index(dx: i32, dy: i32) -> i32 {
    match (dx.signum(), dy.signum()) {
        (1, 0) => 0,
        (0, 1) => 1,
        (-1, 0) => 2,
        (0, -1) => 3,
        _ => 0,
    }
}

impl App {
    pub fn new(seed_override: Option<u64>) -> Result<Self> {
        let seed = seed_override.unwrap_or_else(|| rand::thread_rng().gen());
        let settings = UiSettings::load();
        let terminal = ratatui::try_init()?;
        Ok(Self {
            terminal,
            state: GameState::new(seed),
            visual: VisualState::new(MAP_WIDTH, MAP_HEIGHT),
            show_minimap: false,
            show_help: false,
            camera_dir: 0,
            camera_angle: 0.0,
            auto_exploring: false,
            show_bestiary: false,
            reduced_flash: settings.reduced_flash,
            high_contrast: settings.high_contrast,
            show_log_overlay: false,
            color_filter: settings.color_filter,
            camera_easing: settings.camera_easing,
            head_bob: settings.head_bob,
            low_motion: settings.low_motion,
            camera_bob_timer: 0,
            hints_seen: 0,
            run_history: crate::run_history::load(),
            run_recorded: false,
            should_quit: false,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        let tick_rate = Duration::from_millis(TICK_MS);
        loop {
            self.record_finished_run();
            self.visual.tick();
            let target_angle = camera_dir_angle(self.camera_dir);
            self.camera_angle = match self.camera_easing {
                0 => target_angle,
                2 => step_camera_angle_rate(self.camera_angle, target_angle, 0.22),
                _ => step_camera_angle(self.camera_angle, target_angle),
            };
            self.camera_bob_timer = self.camera_bob_timer.saturating_sub(1);
            if self.auto_exploring && self.visual.tick % 3 == 0 {
                self.advance_auto_explore();
            }
            draw!(self);

            if !event::poll(tick_rate)? {
                continue;
            }

            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match self.state.status {
                    GameStatus::Menu | GameStatus::CampaignMenu => self.handle_menu_key(key.code),
                    GameStatus::Playing => self.handle_playing_key(key.code),
                    GameStatus::Inventory => self.handle_inventory_key(key.code),
                    GameStatus::QuestPanel => self.handle_quest_key(key.code),
                    GameStatus::Dead | GameStatus::Won | GameStatus::CampaignComplete => {
                        self.handle_end_key(key.code)
                    }
                },
                Event::Resize(_, _) => {
                    self.state.fov_dirty = FovDirty::MapChanged;
                }
                _ => {}
            }
            if self.should_quit {
                return Ok(());
            }
        }
    }

    fn save_ui_settings(&mut self) {
        let result = UiSettings {
            reduced_flash: self.reduced_flash,
            high_contrast: self.high_contrast,
            color_filter: self.color_filter,
            camera_easing: self.camera_easing,
            head_bob: self.head_bob,
            low_motion: self.low_motion,
        }
        .save();
        if let Err(error) = result {
            eprintln!("Settings write failed: {error}");
            self.state.msg(
                format!("Settings couldn't be saved: {error}"),
                MsgColor::DAMAGE_TAKEN,
            );
        }
    }

    fn record_finished_run(&mut self) {
        if self.run_recorded
            || !matches!(
                self.state.status,
                GameStatus::Dead | GameStatus::Won | GameStatus::CampaignComplete
            )
        {
            return;
        }
        if let Err(error) = self.state.save_game() {
            eprintln!("Terminal run save failed: {error}");
            self.state.msg(
                format!("The finished run couldn't be saved: {error}"),
                MsgColor::DAMAGE_TAKEN,
            );
        }
        crate::run_history::push_bounded(
            &mut self.run_history,
            crate::run_history::RunSummary::from_state(&self.state),
        );
        if let Err(error) = crate::run_history::save(&self.run_history) {
            eprintln!("Run history write failed: {error}");
            self.state.msg(
                format!("Run history couldn't be saved: {error}"),
                MsgColor::DAMAGE_TAKEN,
            );
        }
        let outcome = crate::replay::ReplayOutcome::from_state(&self.state);
        let replay_seed = self.state.seed;
        let replay_error = if let Some(replay) = &mut self.state.replay_data {
            replay.expected = Some(outcome);
            let path = format!("delve_replay_{replay_seed}.json");
            replay.save_to_file(&path).err().map(|error| (path, error))
        } else {
            None
        };
        if let Some((path, error)) = replay_error {
            eprintln!("Replay write failed: {error}");
            self.state.msg(
                format!("Replay couldn't be written to {path}: {error}"),
                MsgColor::DAMAGE_TAKEN,
            );
        }
        self.run_recorded = true;
    }

    fn handle_menu_key(&mut self, code: KeyCode) {
        if self.show_help {
            if matches!(code, KeyCode::Char('?') | KeyCode::Esc) {
                self.show_help = false;
            }
            return;
        }
        if code == KeyCode::Char('?') {
            self.show_help = true;
            return;
        }
        match self.state.status {
            GameStatus::Menu => match code {
                KeyCode::Char('x') => {
                    self.state.challenge_mode = self.state.challenge_mode.next();
                    self.show_minimap = false;
                }
                KeyCode::Char('c') => {
                    // Classic mode
                    self.show_help = false;
                    self.state.game_mode = GameMode::Classic;
                    self.state.campaign = None;
                    self.camera_dir = 0;
                    self.visual.reset();
                    self.state.new_game();
                    self.run_recorded = false;
                    self.orient_to_open_tile();
                }
                KeyCode::Char('m') => {
                    // Campaign mode
                    self.show_help = false;
                    self.state.game_mode = GameMode::Campaign;
                    self.state.campaign = Some(CampaignState::new());
                    self.state.status = GameStatus::CampaignMenu;
                }
                KeyCode::Char('l') => {
                    self.show_help = false;
                    self.state.load_game();
                    if !self.state.challenge_mode.allows_minimap() {
                        self.show_minimap = false;
                    }
                    self.run_recorded = matches!(
                        self.state.status,
                        GameStatus::Dead | GameStatus::Won | GameStatus::CampaignComplete
                    );
                    self.visual.reset();
                    self.orient_to_open_tile();
                    crate::logging::game_event(GameEvent::Load {
                        seed: self.state.seed,
                    });
                }
                KeyCode::Char('q') => {
                    crate::logging::game_event(GameEvent::Quit { turns: 0 });
                    self.should_quit = true;
                }
                _ => {}
            },
            GameStatus::CampaignMenu => match code {
                KeyCode::Enter => {
                    // Start campaign
                    self.show_help = false;
                    self.camera_dir = 0;
                    self.visual.reset();
                    self.state.status = GameStatus::Playing;
                    self.state.new_game();
                    self.run_recorded = false;
                    self.orient_to_open_tile();
                }
                KeyCode::Char('q') | KeyCode::Esc => {
                    // Back to main menu
                    self.show_help = false;
                    self.state.status = GameStatus::Menu;
                    self.state.game_mode = GameMode::Classic;
                    self.state.campaign = None;
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn handle_playing_key(&mut self, code: KeyCode) {
        // Help is a presentation-only modal. Handle it before auto-explore so
        // opening controls never changes the run.
        if self.show_help {
            if matches!(code, KeyCode::Char('?') | KeyCode::Esc) {
                self.show_help = false;
            }
            return;
        }
        if self.state.rules_version >= 2 && self.state.player.pending_level_ups > 0 {
            let discipline = match code {
                KeyCode::Char('1') => Some(Discipline::Vigor),
                KeyCode::Char('2') => Some(Discipline::Finesse),
                KeyCode::Char('3') => Some(Discipline::Resolve),
                _ => None,
            };
            if let Some(discipline) = discipline {
                self.state.resolve_discipline_choice(discipline);
            }
            return;
        }
        if self.state.preparation_menu.is_some() {
            match code {
                KeyCode::Char('1') => self.state.resolve_preparation(1, &mut self.visual),
                KeyCode::Char('2') => self.state.resolve_preparation(2, &mut self.visual),
                KeyCode::Char('3') => self.state.resolve_preparation(3, &mut self.visual),
                KeyCode::Char('4') => self.state.resolve_preparation(4, &mut self.visual),
                KeyCode::Char('5') => self.state.resolve_preparation(5, &mut self.visual),
                KeyCode::Esc => {
                    self.state.preparation_menu = None;
                    self.state.msg("You make no offering.", MsgColor::DIM);
                }
                _ => {}
            }
            return;
        }
        if self.show_log_overlay {
            if matches!(code, KeyCode::Char('V') | KeyCode::Esc) {
                self.show_log_overlay = false;
            }
            return;
        }
        if self.show_bestiary {
            if matches!(code, KeyCode::Char('B') | KeyCode::Esc) {
                self.show_bestiary = false;
            }
            return;
        }
        // Dialogue owns all keys while open. Previously a response screen
        // could leak technique, item, or movement keys into the run.
        if self.state.dialog_active {
            if code == KeyCode::Esc {
                self.state.close_dialog();
                return;
            }
            if !self.state.dialog_show_choices
                && matches!(code, KeyCode::Char('t') | KeyCode::Char('T'))
            {
                self.state.close_dialog();
                return;
            }
            if self.state.dialog_show_choices {
                if let Some(n) = match code {
                    KeyCode::Char('1') => Some(1),
                    KeyCode::Char('2') => Some(2),
                    KeyCode::Char('3') => Some(3),
                    _ => None,
                } {
                    self.state
                        .process_turn(Action::ConversationChoice(n), &mut self.visual);
                }
            }
            return;
        }
        if code == KeyCode::Char('?') {
            self.show_help = true;
            return;
        }
        if self.auto_exploring && code != KeyCode::Char('x') {
            self.auto_exploring = false;
            self.state.msg("Auto-explore stopped.", MsgColor::DIM);
        }
        if code == KeyCode::Char('B') {
            self.show_bestiary = true;
            return;
        }
        if code == KeyCode::F(2) {
            self.reduced_flash = !self.reduced_flash;
            self.state.msg(
                if self.reduced_flash {
                    "Reduced flash on."
                } else {
                    "Reduced flash off."
                },
                MsgColor::SYSTEM,
            );
            self.save_ui_settings();
            return;
        }
        if code == KeyCode::F(3) {
            self.high_contrast = !self.high_contrast;
            self.state.msg(
                if self.high_contrast {
                    "High contrast on."
                } else {
                    "High contrast off."
                },
                MsgColor::SYSTEM,
            );
            self.save_ui_settings();
            return;
        }
        if code == KeyCode::F(4) {
            self.camera_easing = (self.camera_easing + 1) % 3;
            let label = match self.camera_easing {
                0 => "instant",
                1 => "smooth",
                _ => "cinematic",
            };
            self.state
                .msg(format!("Camera turning: {label}."), MsgColor::SYSTEM);
            self.save_ui_settings();
            return;
        }
        if code == KeyCode::F(6) {
            self.head_bob = !self.head_bob;
            self.camera_bob_timer = 0;
            self.state.msg(
                if self.head_bob {
                    "Head movement on."
                } else {
                    "Head movement off."
                },
                MsgColor::SYSTEM,
            );
            self.save_ui_settings();
            return;
        }
        if code == KeyCode::F(7) {
            self.color_filter = (self.color_filter + 1) % crate::visual::PALETTE_FAMILY_COUNT;
            let label = crate::visual::palette_name(self.color_filter);
            self.state
                .msg(format!("Viewport palette: {label}."), MsgColor::SYSTEM);
            self.save_ui_settings();
            return;
        }
        if code == KeyCode::F(8) {
            self.low_motion = !self.low_motion;
            self.camera_bob_timer = 0;
            self.state.msg(
                if self.low_motion {
                    "Low-motion presentation on."
                } else {
                    "Low-motion presentation off."
                },
                MsgColor::SYSTEM,
            );
            self.save_ui_settings();
            return;
        }
        // ── Log scrolling keys (no turn consumed) ──
        match code {
            KeyCode::Char('V') => {
                self.show_log_overlay = true;
                return;
            }
            KeyCode::PageUp => {
                let log_h = 5; // approximate log height
                let max = self.state.messages.len().saturating_sub(log_h);
                self.state.log_scroll = (self.state.log_scroll + log_h).min(max);
                return;
            }
            KeyCode::PageDown => {
                let log_h = 5;
                self.state.log_scroll = self.state.log_scroll.saturating_sub(log_h);
                return;
            }
            KeyCode::End => {
                self.state.log_scroll = 0;
                return;
            }
            _ => {}
        }

        // ── View controls (no turn consumed) ──
        match code {
            KeyCode::Char('T') => {
                let (dx, dy) = camera_dir_delta(self.camera_dir);
                self.state
                    .process_turn(Action::Technique(dx, dy), &mut self.visual);
                return;
            }
            KeyCode::Char('f') => {
                self.state.use_quick_item(&mut self.visual);
                return;
            }
            KeyCode::Char('m') | KeyCode::Char('M') => {
                let Some(next_visibility) =
                    toggled_map_visibility(self.show_minimap, self.state.challenge_mode)
                else {
                    self.show_minimap = false;
                    self.state.msg(
                        "The Lost Cartographer oath forbids the minimap.",
                        MsgColor::DIM,
                    );
                    return;
                };
                self.hints_seen |= HINT_MAP;
                self.show_minimap = next_visibility;
                self.state.msg(
                    if self.show_minimap {
                        "Map open."
                    } else {
                        "Map closed."
                    },
                    MsgColor::SYSTEM,
                );
                return;
            }
            _ => {}
        }

        if self.handle_first_person_key(code) {
            return;
        }

        let key = crossterm::event::KeyEvent::new(code, crossterm::event::KeyModifiers::NONE);
        let action = input::action_from_play_key(&key);
        match action {
            Action::Quit => {
                crate::logging::game_event(GameEvent::Quit {
                    turns: self.state.stats.turns,
                });
                if let Err(error) = self.state.save_game() {
                    eprintln!("Save failed while quitting: {error}");
                }
                self.should_quit = true;
            }
            Action::SaveGame => {
                crate::logging::game_event(GameEvent::Save);
                match self.state.save_game() {
                    Ok(()) => self.state.msg("Game saved.", MsgColor::SYSTEM),
                    Err(error) => self.state.msg(
                        format!("Couldn't save the game: {error}"),
                        MsgColor::DAMAGE_TAKEN,
                    ),
                }
            }
            Action::LoadGame => {
                self.state.load_game();
                if !self.state.challenge_mode.allows_minimap() {
                    self.show_minimap = false;
                }
                self.run_recorded = matches!(
                    self.state.status,
                    GameStatus::Dead | GameStatus::Won | GameStatus::CampaignComplete
                );
                crate::logging::game_event(GameEvent::Load {
                    seed: self.state.seed,
                });
                self.visual.reset();
                self.orient_to_open_tile();
            }
            Action::TogglePickup => {
                if let Some(replay) = &mut self.state.replay_data {
                    replay.record(self.state.stats.turns, action);
                }
                self.state.auto_pickup = !self.state.auto_pickup;
                self.state.msg(
                    if self.state.auto_pickup {
                        "Auto-pickup ON."
                    } else {
                        "Auto-pickup OFF."
                    },
                    MsgColor::SYSTEM,
                );
            }
            Action::Inventory => {
                self.hints_seen |= HINT_INVENTORY;
                self.state.inventory_cursor = 0;
                self.state.status = GameStatus::Inventory;
            }
            Action::QuestPanel => {
                self.state.status = GameStatus::QuestPanel;
            }
            Action::None => {}
            Action::AutoExplore => {
                self.auto_exploring = !self.auto_exploring;
                if self.auto_exploring {
                    self.state.msg("Auto-explore started.", MsgColor::SYSTEM);
                    self.advance_auto_explore();
                } else {
                    self.state.msg("Auto-explore stopped.", MsgColor::DIM);
                }
            }
            _ => {
                let old_depth = self.state.player.depth;
                self.state.process_turn(action, &mut self.visual);
                if self.state.player.depth != old_depth {
                    self.orient_to_open_tile();
                }
            }
        }
    }

    fn handle_first_person_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('o') => {
                let (dx, dy) = camera_dir_delta(self.camera_dir);
                self.state
                    .process_turn(Action::ToggleDoor(dx, dy), &mut self.visual);
                true
            }
            KeyCode::Char('v') => {
                let (dx, dy) = camera_dir_delta(self.camera_dir);
                self.state
                    .process_turn(Action::Listen(dx, dy), &mut self.visual);
                true
            }
            KeyCode::Char('r') => {
                let (dx, dy) = camera_dir_delta(self.camera_dir);
                self.state
                    .process_turn(Action::Disarm(dx, dy), &mut self.visual);
                true
            }
            KeyCode::Enter => {
                self.hints_seen |= HINT_INTERACT;
                let (dx, dy) = camera_dir_delta(self.camera_dir);
                let old_depth = self.state.player.depth;
                self.state.interact_facing(dx, dy, &mut self.visual);
                if self.state.player.depth != old_depth {
                    self.orient_to_open_tile();
                }
                true
            }
            KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('h') => {
                self.hints_seen |= HINT_NAVIGATION;
                self.camera_dir = (self.camera_dir - 1).rem_euclid(4);
                self.visual
                    .start_player_action(PlayerActionKind::Turn(PlayerTurnKind::Left));
                true
            }
            KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('l') => {
                self.hints_seen |= HINT_NAVIGATION;
                self.camera_dir = (self.camera_dir + 1).rem_euclid(4);
                self.visual
                    .start_player_action(PlayerActionKind::Turn(PlayerTurnKind::Right));
                true
            }
            KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('k') => {
                let (dx, dy) = camera_dir_delta(self.camera_dir);
                self.presented_move(dx, dy, PlayerStepKind::Forward);
                true
            }
            KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('j') => {
                let (dx, dy) = camera_dir_delta(self.camera_dir);
                self.presented_move(-dx, -dy, PlayerStepKind::Backward);
                true
            }
            // Strafe left
            KeyCode::Char('Z') => {
                let (dx, dy) = camera_dir_delta((self.camera_dir - 1).rem_euclid(4));
                self.presented_dodge(dx, dy, PlayerStepKind::StrafeLeft);
                true
            }
            KeyCode::Char('C') => {
                let (dx, dy) = camera_dir_delta((self.camera_dir + 1).rem_euclid(4));
                self.presented_dodge(dx, dy, PlayerStepKind::StrafeRight);
                true
            }
            KeyCode::Char('z') => {
                let (dx, dy) = camera_dir_delta((self.camera_dir - 1).rem_euclid(4));
                self.presented_move(dx, dy, PlayerStepKind::StrafeLeft);
                true
            }
            // Strafe right
            KeyCode::Char('c') => {
                let (dx, dy) = camera_dir_delta((self.camera_dir + 1).rem_euclid(4));
                self.presented_move(dx, dy, PlayerStepKind::StrafeRight);
                true
            }
            // Eat diagonal keys that would otherwise produce accidental grid moves.
            KeyCode::Char('y' | 'u' | 'b' | 'n' | '7' | '9' | '1' | '3') => true,
            _ => false,
        }
    }

    fn presented_move(&mut self, dx: i32, dy: i32, step: PlayerStepKind) {
        let before = (self.state.player.x, self.state.player.y);
        self.state
            .process_turn(Action::Move(dx, dy), &mut self.visual);
        if (self.state.player.x, self.state.player.y) != before {
            self.visual
                .start_player_action(PlayerActionKind::Step(step));
            let room = self.state.map.rooms.iter().position(|room| {
                self.state.player.x >= room.x
                    && self.state.player.x < room.x + room.w
                    && self.state.player.y >= room.y
                    && self.state.player.y < room.y + room.h
            });
            self.visual.observe_room(room);
            self.camera_bob_timer = 8;
        }
    }

    fn presented_dodge(&mut self, dx: i32, dy: i32, step: PlayerStepKind) {
        let before = (self.state.player.x, self.state.player.y);
        self.state
            .process_turn(Action::Dodge(dx, dy), &mut self.visual);
        if (self.state.player.x, self.state.player.y) != before {
            self.visual
                .start_player_action(PlayerActionKind::Dodge(step));
            let room = self.state.map.rooms.iter().position(|room| {
                self.state.player.x >= room.x
                    && self.state.player.x < room.x + room.w
                    && self.state.player.y >= room.y
                    && self.state.player.y < room.y + room.h
            });
            self.visual.observe_room(room);
            self.camera_bob_timer = 10;
        }
    }

    fn advance_auto_explore(&mut self) {
        if self.state.status != GameStatus::Playing || !self.state.player.alive {
            self.auto_exploring = false;
            return;
        }
        if self.state.monsters.iter().any(|monster| {
            monster.alive
                && self.state.map.in_bounds(monster.x, monster.y)
                && self.state.map.visible[self.state.map.idx(monster.x, monster.y)]
        }) {
            self.auto_exploring = false;
            self.state.msg(
                "Auto-explore stopped: movement in the dark.",
                MsgColor::DAMAGE_TAKEN,
            );
            return;
        }

        let before = (self.state.player.x, self.state.player.y);
        self.state.auto_explore(&mut self.visual);
        let after = (self.state.player.x, self.state.player.y);
        let delta = (after.0 - before.0, after.1 - before.1);
        if delta == (0, 0) {
            self.auto_exploring = false;
            return;
        }
        self.camera_dir = direction_index(delta.0, delta.1);
        self.visual
            .start_player_action(PlayerActionKind::Step(PlayerStepKind::Forward));
        let room = self.state.map.rooms.iter().position(|room| {
            after.0 >= room.x
                && after.0 < room.x + room.w
                && after.1 >= room.y
                && after.1 < room.y + room.h
        });
        self.visual.observe_room(room);
        self.camera_bob_timer = 8;

        let on_item = self
            .state
            .items
            .iter()
            .any(|item| (item.x, item.y) == after);
        let at_room_feature = self.state.landmarks.iter().any(|(room_index, _)| {
            self.state.map.rooms.get(*room_index).is_some_and(|room| {
                let center = room.center();
                (center.0 - after.0).abs() + (center.1 - after.1).abs() <= 1
            })
        }) || self.state.camps.iter().any(|camp| {
            self.state.map.rooms.get(camp.room_idx).is_some_and(|room| {
                let center = room.center();
                (center.0 - after.0).abs() + (center.1 - after.1).abs() <= 1
            })
        });
        let tile = self.state.map.tiles[self.state.map.idx(after.0, after.1)];
        let notable_tile = matches!(
            tile,
            Tile::StairsDown | Tile::StairsUp | Tile::TrapRevealed | Tile::LanternDead
        );
        let adjacent_npc = self
            .state
            .npcs
            .iter()
            .any(|npc| (npc.x - after.0).abs() + (npc.y - after.1).abs() <= 1);
        let adjacent_door = [(0, -1), (0, 1), (-1, 0), (1, 0)].iter().any(|(dx, dy)| {
            let (x, y) = (after.0 + dx, after.1 + dy);
            self.state.map.in_bounds(x, y)
                && matches!(
                    self.state.map.tiles[self.state.map.idx(x, y)],
                    Tile::DoorClosed | Tile::LockedDoor
                )
        });
        let threat = self.state.monsters.iter().any(|monster| {
            monster.alive
                && self.state.map.in_bounds(monster.x, monster.y)
                && self.state.map.visible[self.state.map.idx(monster.x, monster.y)]
        });
        if on_item || at_room_feature || notable_tile || adjacent_npc || adjacent_door || threat {
            self.auto_exploring = false;
            self.state.msg(
                if threat {
                    "Auto-explore stopped: threat spotted."
                } else {
                    "Auto-explore stopped: something worth attention."
                },
                MsgColor::SYSTEM,
            );
        }
    }

    fn orient_to_open_tile(&mut self) {
        let (x, y) = (self.state.player.x, self.state.player.y);
        for direction in 0..4 {
            let (dx, dy) = camera_dir_delta(direction);
            let (nx, ny) = (x + dx, y + dy);
            if self.state.map.in_bounds(nx, ny)
                && self.state.map.tiles[self.state.map.idx(nx, ny)] != Tile::Wall
            {
                self.camera_dir = direction;
                self.camera_angle = camera_dir_angle(direction);
                return;
            }
        }
    }

    fn handle_inventory_key(&mut self, code: KeyCode) {
        if code == KeyCode::Char('Q') {
            if let Some(&item) = self.state.player.inventory.get(self.state.inventory_cursor) {
                if quick_usable(item) {
                    self.state.player.quick_item = Some(item);
                    self.state
                        .msg(format!("{} assigned to F.", item.name()), MsgColor::SYSTEM);
                } else {
                    self.state
                        .msg("Only usable supplies can be assigned.", MsgColor::DIM);
                }
            }
            return;
        }
        let key = crossterm::event::KeyEvent::new(code, crossterm::event::KeyModifiers::NONE);
        let inv_action = input::inventory_action_from_key(&key, self.state.player.inventory.len());
        match inv_action {
            InventoryAction::Select(slot) => {
                if let Some(replay) = &mut self.state.replay_data {
                    replay.record_repr(
                        self.state.stats.turns,
                        crate::replay::ActionRepr::UseItem(slot),
                    );
                }
                let item = self.state.player.inventory[slot];
                if self.state.use_item(item, slot, &mut self.visual) {
                    self.state.end_turn(&mut self.visual);
                }
                self.state.preserve_inventory_cursor(item, slot);
            }
            InventoryAction::MoveUp => {
                self.state.inventory_cursor = self.state.inventory_cursor.saturating_sub(1);
            }
            InventoryAction::MoveDown => {
                let max = self.state.player.inventory.len().saturating_sub(1);
                self.state.inventory_cursor = (self.state.inventory_cursor + 1).min(max);
            }
            InventoryAction::UseEquip => {
                if !self.state.player.inventory.is_empty() {
                    if let Some(replay) = &mut self.state.replay_data {
                        replay.record_repr(
                            self.state.stats.turns,
                            crate::replay::ActionRepr::UseItem(self.state.inventory_cursor),
                        );
                    }
                    let item = self.state.player.inventory[self.state.inventory_cursor];
                    let slot = self.state.inventory_cursor;
                    if self.state.use_item(item, slot, &mut self.visual) {
                        self.state.end_turn(&mut self.visual);
                    }
                    self.state.preserve_inventory_cursor(item, slot);
                }
            }
            InventoryAction::Close => {
                self.state.status = GameStatus::Playing;
            }
            InventoryAction::None => {}
        }
    }

    fn handle_quest_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Tab | KeyCode::Esc | KeyCode::Char('Q') | KeyCode::Char('q') => {
                self.state.status = GameStatus::Playing;
            }
            _ => {}
        }
    }

    fn handle_end_key(&mut self, code: KeyCode) {
        if self.state.status == GameStatus::CampaignComplete {
            match code {
                KeyCode::Char('r') => {
                    self.state.game_mode = GameMode::Campaign;
                    self.state.campaign = Some(CampaignState::new());
                    self.camera_dir = 0;
                    self.camera_angle = 0.0;
                    self.visual.reset();
                    self.state.new_game();
                    self.run_recorded = false;
                    self.orient_to_open_tile();
                }
                KeyCode::Char('m') | KeyCode::Char('q') | KeyCode::Esc => {
                    if code == KeyCode::Char('q') {
                        crate::logging::game_event(GameEvent::Quit {
                            turns: self.state.stats.turns,
                        });
                        self.should_quit = true;
                    } else {
                        self.state.status = GameStatus::Menu;
                        self.state.game_mode = GameMode::Classic;
                        self.state.campaign = None;
                        self.visual.reset();
                    }
                }
                _ => {}
            }
            return;
        }
        match code {
            KeyCode::Char('r') => {
                self.camera_dir = 0;
                self.camera_angle = 0.0;
                self.visual.reset();
                self.state.new_game();
                self.run_recorded = false;
                self.orient_to_open_tile();
            }
            KeyCode::Char('q') => {
                crate::logging::game_event(GameEvent::Quit {
                    turns: self.state.stats.turns,
                });
                self.should_quit = true;
            }
            _ => {}
        }
    }
}
