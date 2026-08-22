// Visual state, effects, and palette system for Delve.
//
// DESIGN NOTE: VisualState is intentionally never serialized. Effects are
// transient presentation-only data that play out over animation ticks and
// should not persist across save/load. Gameplay state lives in GameState;
// visual candy lives here.

use std::collections::VecDeque;

use crate::entity::{ItemKind, MonsterKind, TechniqueKind};

// ── Visual tick counter ──
//
// Increments once per rendered frame. Used by the renderer to animate
// glyphs, fade effects, and drive time-based visuals. Zeroed when a new
// game starts.

pub type Tick = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerStepKind {
    Forward,
    Backward,
    StrafeLeft,
    StrafeRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerTurnKind {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerActionKind {
    Step(PlayerStepKind),
    Dodge(PlayerStepKind),
    Turn(PlayerTurnKind),
    Attack { weapon: Option<ItemKind> },
    Technique(TechniqueKind),
    UseItem(ItemKind),
    Guard,
    Hit,
    Heal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerAction {
    pub kind: PlayerActionKind,
    pub timer: u8,
}

impl PlayerAction {
    pub fn new(kind: PlayerActionKind) -> Self {
        Self {
            kind,
            timer: Self::duration(kind),
        }
    }

    pub fn duration(kind: PlayerActionKind) -> u8 {
        match kind {
            PlayerActionKind::Attack { .. } => 12,
            PlayerActionKind::Technique(_) => 14,
            PlayerActionKind::UseItem(_) => 16,
            PlayerActionKind::Hit | PlayerActionKind::Heal => 10,
            PlayerActionKind::Dodge(_) => 10,
            PlayerActionKind::Guard | PlayerActionKind::Step(_) | PlayerActionKind::Turn(_) => 8,
        }
    }

    pub fn elapsed(self) -> u8 {
        Self::duration(self.kind).saturating_sub(self.timer)
    }

    pub fn phase(self) -> u8 {
        let elapsed = self.elapsed();
        let duration = Self::duration(self.kind);
        if elapsed * 3 < duration {
            0
        } else if elapsed * 3 < duration * 2 {
            1
        } else {
            2
        }
    }

    /// Small screen-space camera displacement for this presentation frame.
    pub fn camera_motion(self) -> (i32, i32) {
        const STEP_BOB: [i32; 8] = [0, 1, 2, 1, 0, -1, 0, 0];
        let elapsed = usize::from(self.elapsed());
        match self.kind {
            PlayerActionKind::Step(PlayerStepKind::Forward) => {
                (STEP_BOB[elapsed.min(7)], if elapsed < 4 { -1 } else { 1 })
            }
            PlayerActionKind::Step(PlayerStepKind::Backward) => {
                (-STEP_BOB[elapsed.min(7)], if elapsed < 4 { 1 } else { -1 })
            }
            PlayerActionKind::Step(PlayerStepKind::StrafeLeft) => {
                (STEP_BOB[elapsed.min(7)] / 2, -2 + i32::from(elapsed >= 4))
            }
            PlayerActionKind::Step(PlayerStepKind::StrafeRight) => {
                (STEP_BOB[elapsed.min(7)] / 2, 2 - i32::from(elapsed >= 4))
            }
            PlayerActionKind::Dodge(PlayerStepKind::StrafeLeft) => {
                (1 - i32::from(elapsed > 5), -3 + i32::from(elapsed > 5))
            }
            PlayerActionKind::Dodge(PlayerStepKind::StrafeRight) => {
                (1 - i32::from(elapsed > 5), 3 - i32::from(elapsed > 5))
            }
            PlayerActionKind::Dodge(_) => (1, 0),
            PlayerActionKind::Turn(PlayerTurnKind::Left) => (0, -1),
            PlayerActionKind::Turn(PlayerTurnKind::Right) => (0, 1),
            PlayerActionKind::Attack { .. } => match self.phase() {
                0 => (-1, -1),
                1 => (1, 2),
                _ => (0, 1),
            },
            PlayerActionKind::Technique(kind) => match (kind, self.phase()) {
                (TechniqueKind::Earthbreak, 0) => (-2, 0),
                (TechniqueKind::Earthbreak, 1) => (3, 0),
                (TechniqueKind::Sweep | TechniqueKind::CinderArc, 1) => (1, 3),
                (_, 0) => (-1, -2),
                (_, 1) => (2, 1),
                _ => (0, 0),
            },
            PlayerActionKind::UseItem(_) => match self.phase() {
                0 => (0, -1),
                1 => (1, 0),
                _ => (0, 1),
            },
            PlayerActionKind::Hit => match self.phase() {
                0 => (1, -2),
                1 => (-1, 1),
                _ => (0, 0),
            },
            PlayerActionKind::Guard | PlayerActionKind::Heal => (0, 0),
        }
    }
}

// ── Visual effects ──

/// A transient visual effect that plays out over several animation ticks.
#[derive(Debug, Clone)]
pub enum VisualEffect {
    /// Brief presentation cue emitted after a monster changes map cells.
    ActorStep {
        x: i32,
        y: i32,
        dx: i32,
        dy: i32,
        timer: u8,
    },
    /// Flash the attacker's cell (x, y) — used for player and monster attacks.
    AttackFlash {
        x: i32,
        y: i32,
        timer: u8,
        flash_color: (u8, u8, u8),
    },
    /// A weapon-specific swing or impact arc anchored to the struck cell.
    WeaponSweep {
        x: i32,
        y: i32,
        dx: i32,
        dy: i32,
        weapon: Option<ItemKind>,
        counter: bool,
        timer: u8,
    },
    /// The readable cast-up beat before a technique resolves.
    TechniqueCast {
        x: i32,
        y: i32,
        dx: i32,
        dy: i32,
        kind: TechniqueKind,
        timer: u8,
    },
    /// The technique-specific shape at each affected target.
    TechniqueImpact {
        x: i32,
        y: i32,
        dx: i32,
        dy: i32,
        kind: TechniqueKind,
        timer: u8,
    },
    /// A consumable, scroll, or lore item being used in the player's hands.
    ItemCast {
        x: i32,
        y: i32,
        kind: ItemKind,
        timer: u8,
    },
    /// Dust, afterimage, or displaced cloak left by player movement.
    MovementTrail {
        x: i32,
        y: i32,
        dx: i32,
        dy: i32,
        dash: bool,
        timer: u8,
    },
    /// A monster-specific casting tell, currently used by necromancer bolts.
    MonsterCast {
        x: i32,
        y: i32,
        kind: MonsterKind,
        dx: i32,
        dy: i32,
        timer: u8,
    },
    /// A floating damage number that drifts upward.
    DamageNumber {
        x: i32,
        y: i32,
        amount: i32,
        color: (u8, u8, u8),
        timer: u8,
    },
    /// Material-specific collapse particles around a killed monster or prop.
    DeathBurst {
        x: i32,
        y: i32,
        kind: Option<MonsterKind>,
        timer: u8,
    },
    /// Directional flare emitted when a projectile is created.
    ProjectileLaunch {
        x: i32,
        y: i32,
        dx: i32,
        dy: i32,
        timer: u8,
    },
    /// Expanding ring from scroll of fire.
    FireRing {
        x: i32,
        y: i32,
        /// current radius in map cells (fractional)
        radius: f32,
        timer: u8,
    },
    /// Swirl effect on recently confused monsters.
    ConfusionSwirl {
        xs: Vec<i32>,
        ys: Vec<i32>,
        timer: u8,
    },
    /// Green/purple pulse on a poisoned entity.
    PoisonPulse { x: i32, y: i32, timer: u8 },
    /// Sparkle animation on the Crown of Depth.
    CrownSparkle { x: i32, y: i32, timer: u8 },
    /// Golden pulse when healing.
    HealGlow { x: i32, y: i32, timer: u8 },
    /// Fade transition when changing floors.
    StairsTransition { timer: u8 },
    /// Golden shimmer on pickup.
    PickupSparkle {
        #[allow(dead_code)]
        x: i32,
        #[allow(dead_code)]
        y: i32,
        timer: u8,
    },
}

impl VisualEffect {
    /// Returns `true` if the effect has expired (timer == 0).
    pub fn expired(&self) -> bool {
        match self {
            Self::ActorStep { timer, .. }
            | Self::AttackFlash { timer, .. }
            | Self::WeaponSweep { timer, .. }
            | Self::TechniqueCast { timer, .. }
            | Self::TechniqueImpact { timer, .. }
            | Self::ItemCast { timer, .. }
            | Self::MovementTrail { timer, .. }
            | Self::MonsterCast { timer, .. }
            | Self::DamageNumber { timer, .. }
            | Self::DeathBurst { timer, .. }
            | Self::ProjectileLaunch { timer, .. }
            | Self::FireRing { timer, .. }
            | Self::ConfusionSwirl { timer, .. }
            | Self::PoisonPulse { timer, .. }
            | Self::CrownSparkle { timer, .. }
            | Self::HealGlow { timer, .. }
            | Self::StairsTransition { timer, .. }
            | Self::PickupSparkle { timer, .. } => *timer == 0,
        }
    }

    /// Decrement the internal timer by one tick.
    pub fn tick(&mut self) {
        fn dec(t: &mut u8) {
            *t = t.saturating_sub(1);
        }
        match self {
            Self::ActorStep { timer, .. } | Self::AttackFlash { timer, .. } => dec(timer),
            Self::WeaponSweep { timer, .. }
            | Self::TechniqueCast { timer, .. }
            | Self::TechniqueImpact { timer, .. }
            | Self::ItemCast { timer, .. }
            | Self::MovementTrail { timer, .. }
            | Self::MonsterCast { timer, .. } => dec(timer),
            Self::DamageNumber { timer, .. } => dec(timer),
            Self::DeathBurst { timer, .. } => dec(timer),
            Self::ProjectileLaunch { timer, .. } => dec(timer),
            Self::FireRing { timer, .. } => dec(timer),
            Self::ConfusionSwirl { timer, .. } => dec(timer),
            Self::PoisonPulse { timer, .. } => dec(timer),
            Self::CrownSparkle { timer, .. } => dec(timer),
            Self::HealGlow { timer, .. } => dec(timer),
            Self::StairsTransition { timer, .. } => dec(timer),
            Self::PickupSparkle { timer, .. } => dec(timer),
        }
    }

    // ── Constructors ──

    pub fn actor_step(x: i32, y: i32, dx: i32, dy: i32) -> Self {
        Self::ActorStep {
            x,
            y,
            dx,
            dy,
            timer: 6,
        }
    }

    pub fn attack_flash(x: i32, y: i32) -> Self {
        Self::AttackFlash {
            x,
            y,
            timer: 6,
            flash_color: (255, 255, 120),
        }
    }

    pub fn monster_attack_flash(x: i32, y: i32) -> Self {
        Self::AttackFlash {
            x,
            y,
            timer: 6,
            flash_color: (255, 80, 80),
        }
    }

    pub fn weapon_sweep(
        x: i32,
        y: i32,
        dx: i32,
        dy: i32,
        weapon: Option<ItemKind>,
        counter: bool,
    ) -> Self {
        Self::WeaponSweep {
            x,
            y,
            dx,
            dy,
            weapon,
            counter,
            timer: 12,
        }
    }

    pub fn technique_cast(x: i32, y: i32, dx: i32, dy: i32, kind: TechniqueKind) -> Self {
        Self::TechniqueCast {
            x,
            y,
            dx,
            dy,
            kind,
            timer: 14,
        }
    }

    pub fn technique_impact(x: i32, y: i32, dx: i32, dy: i32, kind: TechniqueKind) -> Self {
        Self::TechniqueImpact {
            x,
            y,
            dx,
            dy,
            kind,
            timer: 10,
        }
    }

    pub fn item_cast(x: i32, y: i32, kind: ItemKind) -> Self {
        Self::ItemCast {
            x,
            y,
            kind,
            timer: 12,
        }
    }

    pub fn movement_trail(x: i32, y: i32, dx: i32, dy: i32, dash: bool) -> Self {
        Self::MovementTrail {
            x,
            y,
            dx,
            dy,
            dash,
            timer: if dash { 10 } else { 7 },
        }
    }

    pub fn monster_cast(x: i32, y: i32, kind: MonsterKind, dx: i32, dy: i32) -> Self {
        Self::MonsterCast {
            x,
            y,
            kind,
            dx,
            dy,
            timer: 10,
        }
    }

    pub fn damage_number(x: i32, y: i32, amount: i32) -> Self {
        Self::DamageNumber {
            x,
            y,
            amount,
            color: (255, 200, 60),
            timer: 12,
        }
    }

    pub fn player_damage_number(x: i32, y: i32, amount: i32) -> Self {
        Self::DamageNumber {
            x,
            y,
            amount,
            color: (255, 80, 80),
            timer: 12,
        }
    }

    pub fn death_burst(x: i32, y: i32) -> Self {
        Self::DeathBurst {
            x,
            y,
            kind: None,
            timer: 10,
        }
    }

    pub fn monster_death(x: i32, y: i32, kind: MonsterKind) -> Self {
        Self::DeathBurst {
            x,
            y,
            kind: Some(kind),
            timer: 10,
        }
    }

    pub fn projectile_launch(x: i32, y: i32, dx: i32, dy: i32) -> Self {
        Self::ProjectileLaunch {
            x,
            y,
            dx,
            dy,
            timer: 7,
        }
    }

    pub fn fire_ring(x: i32, y: i32) -> Self {
        Self::FireRing {
            x,
            y,
            radius: 0.0,
            timer: 15,
        }
    }

    pub fn confusion_swirl(xs: Vec<i32>, ys: Vec<i32>) -> Self {
        Self::ConfusionSwirl { xs, ys, timer: 14 }
    }

    pub fn poison_pulse(x: i32, y: i32) -> Self {
        Self::PoisonPulse { x, y, timer: 8 }
    }

    pub fn crown_sparkle(x: i32, y: i32) -> Self {
        Self::CrownSparkle { x, y, timer: 30 }
    }

    pub fn heal_glow(x: i32, y: i32) -> Self {
        Self::HealGlow { x, y, timer: 8 }
    }

    pub fn stairs_transition() -> Self {
        Self::StairsTransition { timer: 20 }
    }

    pub fn pickup_sparkle(x: i32, y: i32) -> Self {
        Self::PickupSparkle { x, y, timer: 6 }
    }
}

pub fn staggered_phase(tick: Tick, x: i32, y: i32, salt: u64, period: u64) -> u64 {
    if period == 0 {
        return 0;
    }
    let position =
        (x as i64 as u64).wrapping_mul(0x9E37_79B9) ^ (y as i64 as u64).rotate_left(17) ^ salt;
    tick.wrapping_add(position) % period
}

// ── Decals (permanent or long-lived) ──

#[derive(Debug, Clone, Copy)]
pub enum Decal {
    /// Blood stain from a killed monster.
    Blood { x: i32, y: i32, fade: u8 },
    /// Scorch mark from fire scroll.
    #[allow(dead_code)]
    Scorch { x: i32, y: i32, fade: u8 },
}

// ── Visual state ──

pub struct VisualState {
    pub tick: Tick,
    /// Active transient effects.
    pub effects: VecDeque<VisualEffect>,
    /// Permanent map decals (blood, scorch, etc.).
    pub decals: Vec<Decal>,
    /// Monsters that were seen on the previous tick (for flash detection).
    pub prev_monster_hp: Vec<i32>,
    /// Timer for the third-person player damage reaction.
    /// Non-zero = red flash visible; counts down each tick.
    pub damage_flash_timer: u8,
    /// Latest player presentation action. Never serialized or replayed.
    pub player_action: Option<PlayerAction>,
    /// Last authored room observed by the live camera and its entry reveal.
    pub last_room: Option<usize>,
    pub room_reveal_timer: u8,
}

impl VisualState {
    pub fn new(_map_w: i32, _map_h: i32) -> Self {
        Self {
            tick: 0,
            effects: VecDeque::new(),
            decals: Vec::new(),
            prev_monster_hp: Vec::new(),
            damage_flash_timer: 0,
            player_action: None,
            last_room: None,
            room_reveal_timer: 0,
        }
    }

    /// Trigger the damage flash overlay. Call when player takes damage.
    pub fn trigger_damage_flash(&mut self) {
        self.damage_flash_timer = 8;
        self.start_player_action(PlayerActionKind::Hit);
    }

    pub fn start_player_action(&mut self, kind: PlayerActionKind) {
        self.player_action = Some(PlayerAction::new(kind));
    }

    pub fn observe_room(&mut self, room: Option<usize>) {
        let Some(room) = room else { return };
        if self.last_room != Some(room) {
            self.last_room = Some(room);
            self.room_reveal_timer = 12;
        }
    }

    /// Call every rendered frame. Advances all timers and prunes dead effects.
    pub fn tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        if self.damage_flash_timer > 0 {
            self.damage_flash_timer -= 1;
        }
        self.room_reveal_timer = self.room_reveal_timer.saturating_sub(1);
        if let Some(action) = &mut self.player_action {
            action.timer = action.timer.saturating_sub(1);
            if action.timer == 0 {
                self.player_action = None;
            }
        }
        let mut i = 0;
        while i < self.effects.len() {
            self.effects[i].tick();
            if self.effects[i].expired() {
                self.effects.remove(i);
            } else {
                i += 1;
            }
        }
        // Also tick FireRing radius
        for eff in self.effects.iter_mut() {
            if let VisualEffect::FireRing { radius, .. } = eff {
                *radius += 0.35;
            }
        }
    }

    pub fn add_effect(&mut self, effect: VisualEffect) {
        if matches!(effect, VisualEffect::HealGlow { .. }) {
            self.start_player_action(PlayerActionKind::Heal);
        }
        self.effects.push_back(effect);
    }

    pub fn add_decal(&mut self, decal: Decal) {
        self.decals.push(decal);
    }

    pub fn reset(&mut self) {
        self.tick = 0;
        self.effects.clear();
        self.decals.clear();
        self.prev_monster_hp.clear();
        self.damage_flash_timer = 0;
        self.player_action = None;
        self.last_room = None;
        self.room_reveal_timer = 0;
    }

    /// Update previous monster HP snapshot for hit detection.
    #[allow(dead_code)]
    pub fn snapshot_monster_hp(&mut self, monsters: &[crate::entity::Monster]) {
        self.prev_monster_hp = monsters.iter().map(|m| m.stats.hp).collect();
    }
}

// ── Depth-specific palette ──

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Biome {
    Depth0, // mossy stone
    Depth1, // cold blue caverns
    Depth2, // rusty crypt
    Depth3, // toxic spider lair
    Depth4, // void/gold Crown chamber
}

impl From<u32> for Biome {
    fn from(depth: u32) -> Self {
        match depth {
            0 => Biome::Depth0,
            1 => Biome::Depth1,
            2 => Biome::Depth2,
            3 => Biome::Depth3,
            _ => Biome::Depth4,
        }
    }
}

/// RGB color convenience type.
pub type Rgb = (u8, u8, u8);

pub const PALETTE_FAMILY_COUNT: u8 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldPalette {
    pub floor: Rgb,
    pub floor_memory: Rgb,
    pub wall: Rgb,
    pub wall_memory: Rgb,
    pub fog: Rgb,
    pub light: Rgb,
}

pub fn palette_name(mode: u8) -> &'static str {
    match mode % PALETTE_FAMILY_COUNT {
        0 => "depthbound",
        1 => "torchstone",
        2 => "floodglass",
        3 => "fungal bloom",
        4 => "ossuary",
        _ => "arcade ember",
    }
}

pub fn world_palette(mode: u8, biome: Biome) -> WorldPalette {
    match mode % PALETTE_FAMILY_COUNT {
        0 => WorldPalette {
            floor: biome.floor_visible(),
            floor_memory: biome.floor_explored(),
            wall: biome.wall_visible(),
            wall_memory: biome.wall_explored(),
            fog: biome.fog_color(),
            light: biome.torch_color(),
        },
        1 => WorldPalette {
            floor: (62, 46, 34),
            floor_memory: (30, 23, 20),
            wall: (126, 88, 58),
            wall_memory: (54, 38, 30),
            fog: (22, 14, 12),
            light: (255, 184, 78),
        },
        2 => WorldPalette {
            floor: (30, 62, 72),
            floor_memory: (18, 30, 38),
            wall: (62, 112, 126),
            wall_memory: (28, 48, 58),
            fog: (10, 22, 30),
            light: (128, 232, 244),
        },
        3 => WorldPalette {
            floor: (48, 64, 38),
            floor_memory: (24, 30, 20),
            wall: (90, 108, 56),
            wall_memory: (40, 48, 28),
            fog: (16, 22, 12),
            light: (198, 238, 92),
        },
        4 => WorldPalette {
            floor: (68, 62, 54),
            floor_memory: (30, 28, 26),
            wall: (142, 130, 108),
            wall_memory: (58, 54, 48),
            fog: (20, 18, 18),
            light: (244, 218, 166),
        },
        _ => WorldPalette {
            floor: (34, 42, 68),
            floor_memory: (16, 20, 34),
            wall: (42, 142, 156),
            wall_memory: (24, 54, 66),
            fog: (8, 10, 22),
            light: (255, 92, 48),
        },
    }
}

pub fn grade_art(mode: u8, rgb: Rgb) -> Rgb {
    let luminance = rgb.0 as f32 * 0.299 + rgb.1 as f32 * 0.587 + rgb.2 as f32 * 0.114;
    let anchor = match mode % PALETTE_FAMILY_COUNT {
        0 => return rgb,
        1 => (230, 126, 54),
        2 => (72, 194, 220),
        3 => (148, 202, 78),
        4 => (206, 190, 158),
        _ => (244, 74, 54),
    };
    let amount = if luminance < 70.0 { 0.12 } else { 0.22 };
    lerp(rgb, anchor, amount)
}

/// Blend two RGB colors by factor `t` (0.0 = full a, 1.0 = full b).
pub fn blend(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    (
        (a.0 as f32 + (b.0 as f32 - a.0 as f32) * t).round() as u8,
        (a.1 as f32 + (b.1 as f32 - a.1 as f32) * t).round() as u8,
        (a.2 as f32 + (b.2 as f32 - a.2 as f32) * t).round() as u8,
    )
}

/// Darken an RGB color by multiplying each channel by `factor`.
#[allow(dead_code)]
pub fn darken(c: Rgb, factor: f32) -> Rgb {
    let f = factor.clamp(0.0, 1.0);
    (
        (c.0 as f32 * f) as u8,
        (c.1 as f32 * f) as u8,
        (c.2 as f32 * f) as u8,
    )
}

/// Lerp between two RGB colors.
#[allow(dead_code)]
pub fn lerp(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    (
        (a.0 as f32 + (b.0 as f32 - a.0 as f32) * t).round() as u8,
        (a.1 as f32 + (b.1 as f32 - a.1 as f32) * t).round() as u8,
        (a.2 as f32 + (b.2 as f32 - a.2 as f32) * t).round() as u8,
    )
}

/// Screen blend of `add` onto `base` by `amount` (0..=1).
///
/// Screen blending keeps highlights luminous: `out = 1 - (1 - base)(1 - add)`.
/// `amount` interpolates between the original `base` and the fully screened
/// result, so a small amount reads as a cheap additive glow without clipping.
pub fn screen_blend(base: Rgb, add: Rgb, amount: f32) -> Rgb {
    let amount = amount.clamp(0.0, 1.0);
    let mix = |b: u8, a: u8| -> u8 {
        let b = b as f32 / 255.0;
        let a = a as f32 / 255.0;
        let screened = 1.0 - (1.0 - b) * (1.0 - a);
        ((b + (screened - b) * amount) * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    (mix(base.0, add.0), mix(base.1, add.1), mix(base.2, add.2))
}

impl Biome {
    pub fn floor_visible(&self) -> Rgb {
        match self {
            Biome::Depth0 => (48, 52, 40), // mossy stone
            Biome::Depth1 => (38, 42, 62), // cold blue cave
            Biome::Depth2 => (52, 36, 32), // rusty crypt
            Biome::Depth3 => (44, 30, 50), // spider lair
            Biome::Depth4 => (30, 28, 42), // void/chamber
        }
    }

    pub fn wall_visible(&self) -> Rgb {
        match self {
            Biome::Depth0 => (92, 90, 70),
            Biome::Depth1 => (72, 78, 98),
            Biome::Depth2 => (90, 62, 52),
            Biome::Depth3 => (82, 62, 88),
            Biome::Depth4 => (64, 58, 78),
        }
    }

    pub fn floor_explored(&self) -> Rgb {
        match self {
            Biome::Depth0 => (28, 30, 24),
            Biome::Depth1 => (22, 28, 38),
            Biome::Depth2 => (30, 22, 20),
            Biome::Depth3 => (26, 18, 32),
            Biome::Depth4 => (18, 16, 28),
        }
    }

    pub fn wall_explored(&self) -> Rgb {
        match self {
            Biome::Depth0 => (42, 40, 34),
            Biome::Depth1 => (34, 38, 48),
            Biome::Depth2 => (44, 30, 26),
            Biome::Depth3 => (40, 30, 44),
            Biome::Depth4 => (30, 28, 38),
        }
    }

    pub fn torch_color(&self) -> Rgb {
        match self {
            Biome::Depth0 => (255, 220, 140), // warm
            Biome::Depth1 => (180, 200, 255), // cool
            Biome::Depth2 => (255, 180, 120), // orange
            Biome::Depth3 => (200, 140, 255), // purple
            Biome::Depth4 => (220, 200, 150), // golden
        }
    }

    pub fn fog_color(&self) -> Rgb {
        match self {
            Biome::Depth0 => (20, 22, 18),
            Biome::Depth1 => (16, 18, 28),
            Biome::Depth2 => (22, 16, 14),
            Biome::Depth3 => (18, 12, 24),
            Biome::Depth4 => (12, 10, 18),
        }
    }
}

// ── Contextual wall glyph selection ──
//
// Reads the 4-neighbor tiles and picks a Unicode box-drawing character.

use crate::map::{Map, Tile};

pub fn wall_glyph(map: &Map, x: i32, y: i32) -> char {
    let n = |dx: i32, dy: i32| -> bool {
        let nx = x + dx;
        let ny = y + dy;
        if !map.in_bounds(nx, ny) {
            return true; // treat out-of-bounds as wall
        }
        map.tiles[map.idx(nx, ny)] == Tile::Wall
    };

    let up = n(0, -1);
    let down = n(0, 1);
    let left = n(-1, 0);
    let right = n(1, 0);

    match (up, down, left, right) {
        // Single-axis connections
        (true, true, false, false) => '\u{2502}', // │
        (false, false, true, true) => '\u{2500}', // ─
        // Corners
        (false, true, true, false) => '\u{250C}', // ┌
        (false, true, false, true) => '\u{2510}', // ┐
        (true, false, true, false) => '\u{2514}', // └
        (true, false, false, true) => '\u{2518}', // ┘
        // T-junctions
        (false, true, true, true) => '\u{252C}', // ┬
        (true, false, true, true) => '\u{2534}', // ┴
        (true, true, false, true) => '\u{251C}', // ├
        (true, true, true, false) => '\u{2524}', // ┤
        // Cross
        (true, true, true, true) => '\u{253C}', // ┼
        // Dead ends or isolated
        (false, false, false, false) => '\u{2588}', // █ full block
        (false, false, _, _) => '\u{2588}',         // isolated horizontal
        (_, _, false, false) => '\u{2588}',         // isolated vertical
    }
}

// ── Dungeon decoration (render-only, deterministic from seed/depth/x/y) ──

/// Returns an optional decoration char for a floor tile, or `None` for plain floor.
pub fn floor_decor(seed: u64, depth: u32, x: i32, y: i32) -> Option<char> {
    // Simple hash function
    let h = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407)
        .wrapping_add(x as u64)
        .wrapping_mul(6364136223846793005)
        .wrapping_add(y as u64)
        .wrapping_mul(6364136223846793005)
        .wrapping_add(depth as u64);

    let v = (h >> 32) as u32;

    // Only decorate a small minority of floor tiles.
    if v % 100 >= 12 {
        return None;
    }

    // Select decor based on depth
    match depth {
        0 => match v % 6 {
            0 => Some('\u{00B7}'), // middle dot ·
            1 => Some('\u{2022}'), // bullet •
            2 => Some(','),
            3 => Some('`'),
            _ => None,
        },
        1 => match v % 5 {
            0 => Some('\u{00B0}'), // degree °
            1 => Some('\u{2591}'), // light shade ░
            2 => Some('\u{2219}'), // bullet operator ∙
            _ => None,
        },
        2 => match v % 5 {
            0 => Some('\u{2591}'), // light shade
            1 => Some('\u{2571}'), // light diagonal ╱
            2 => Some('\u{2572}'), // light diagonal ╲
            _ => None,
        },
        3 => match v % 6 {
            0 => Some('\u{00B7}'), // middle dot (web-like)
            1 => Some('\u{2508}'), // light vertical ┊
            2 => Some('\u{2504}'), // light horizontal ┄
            _ => None,
        },
        _ => match v % 5 {
            0 => Some('\u{2591}'), // light shade
            1 => Some('\u{25E6}'), // white bullet ◦
            2 => Some('\u{22C5}'), // dot operator ⋅
            _ => None,
        },
    }
}

/// Returns an optional decoration char for a visible wall tile.
#[allow(dead_code)]
pub fn wall_decor(seed: u64, depth: u32, x: i32, y: i32) -> Option<char> {
    let h = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(6364136223846793005)
        .wrapping_add(x as u64)
        .wrapping_mul(6364136223846793005)
        .wrapping_add(y as u64)
        .wrapping_mul(6364136223846793005)
        .wrapping_add(depth as u64);
    let v = (h >> 32) as u32;
    if v % 100 >= 3 {
        return None;
    }
    match depth {
        0 => {
            if v % 3 == 0 {
                Some('\u{2591}')
            } else {
                None
            }
        } // moss
        2 => {
            if v % 2 == 0 {
                Some('\u{2592}')
            } else {
                None
            }
        } // rust/medium shade
        3 => {
            if v % 2 == 0 {
                Some('\u{2572}')
            } else {
                None
            }
        } // ╲ web crack
        4 => {
            if v % 3 == 0 {
                Some('\u{25C6}')
            } else {
                None
            }
        } // ◆ rune
        _ => None,
    }
}

// ── Animated glyph helpers ──

/// Returns an optionally animated glyph for a monster, based on tick.
/// Returns `None` if the base symbol should be used.
pub fn monster_anim_glyph(kind: MonsterKind, tick: Tick) -> Option<char> {
    let phase = tick % 8;
    match kind {
        MonsterKind::Bat => {
            if phase < 4 {
                Some('b')
            } else {
                Some('v')
            }
        }
        MonsterKind::Wraith => {
            if phase < 4 {
                Some('W')
            } else {
                Some('w')
            }
        }
        MonsterKind::Spider => {
            if phase < 4 {
                Some('S')
            } else {
                Some('s')
            }
        }
        _ => None,
    }
}

/// Returns an optionally animated glyph for the Crown of Depth.
pub fn crown_anim_glyph(tick: Tick) -> Option<char> {
    match tick % 6 {
        0 | 1 => Some('*'),
        2 | 3 => Some('\u{2726}'), // ✦
        _ => Some('\u{25C6}'),     // ◆
    }
}

/// Returns a styled glyph for an item on the floor.
pub fn item_floor_glyph(kind: ItemKind, tick: Tick) -> Option<char> {
    match kind {
        ItemKind::CrownOfDepth => crown_anim_glyph(tick),
        ItemKind::HealthPotion
        | ItemKind::LargeHealthPotion
        | ItemKind::Antidote
        | ItemKind::LanternOil => Some('⚗'),
        ItemKind::SmokeBomb => Some('●'),
        ItemKind::ScrollOfFire | ItemKind::ScrollOfConfusion | ItemKind::ScrollOfBlink => Some('≋'),
        ItemKind::EnchantmentScroll | ItemKind::LoreFragment => Some('§'),
        ItemKind::RustySword | ItemKind::Longsword | ItemKind::Warhammer => Some('╱'),
        ItemKind::LeatherArmor | ItemKind::Chainmail | ItemKind::Shield => Some('◈'),
        ItemKind::BellToken => Some('♦'),
        ItemKind::IronKey => Some('k'),
        ItemKind::Emberbrand => Some('†'),
        ItemKind::TideglassMail => Some('◇'),
        ItemKind::Graveward => Some('⬙'),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{Map, Tile};

    #[test]
    fn staggered_phase_is_deterministic_and_position_sensitive() {
        let first = staggered_phase(12, 4, 7, 9, 48);
        assert_eq!(first, staggered_phase(12, 4, 7, 9, 48));
        assert_ne!(first, staggered_phase(12, 5, 7, 9, 48));
        assert_eq!(staggered_phase(12, 4, 7, 9, 0), 0);
    }

    #[test]
    fn actor_step_cue_expires_without_gameplay_state() {
        let mut effect = VisualEffect::actor_step(3, 4, 1, 0);
        for _ in 0..6 {
            effect.tick();
        }
        assert!(effect.expired());
    }

    #[test]
    fn player_action_exposes_three_phases_and_expires() {
        let mut visual = VisualState::new(80, 50);
        visual.start_player_action(PlayerActionKind::Attack {
            weapon: Some(ItemKind::Warhammer),
        });
        assert_eq!(visual.player_action.unwrap().phase(), 0);
        for _ in 0..4 {
            visual.tick();
        }
        assert_eq!(visual.player_action.unwrap().phase(), 1);
        for _ in 0..4 {
            visual.tick();
        }
        assert_eq!(visual.player_action.unwrap().phase(), 2);
        for _ in 0..4 {
            visual.tick();
        }
        assert!(visual.player_action.is_none());
    }

    #[test]
    fn item_use_has_a_full_cast_sequence() {
        let action = PlayerAction::new(PlayerActionKind::UseItem(ItemKind::ScrollOfFire));
        assert_eq!(PlayerAction::duration(action.kind), 16);
        assert_eq!(action.phase(), 0);
        let mut action = action;
        action.timer = 10;
        assert_eq!(action.phase(), 1);
        action.timer = 2;
        assert_eq!(action.phase(), 2);
    }

    #[test]
    fn authored_combat_effects_all_tick_and_expire() {
        let mut visual = VisualState::new(80, 50);
        visual.add_effect(VisualEffect::weapon_sweep(
            4,
            4,
            1,
            0,
            Some(ItemKind::Longsword),
            false,
        ));
        visual.add_effect(VisualEffect::technique_cast(
            4,
            4,
            1,
            0,
            TechniqueKind::Lunge,
        ));
        visual.add_effect(VisualEffect::technique_impact(
            5,
            4,
            1,
            0,
            TechniqueKind::Lunge,
        ));
        visual.add_effect(VisualEffect::item_cast(4, 4, ItemKind::ScrollOfBlink));
        visual.add_effect(VisualEffect::movement_trail(3, 4, 1, 0, true));
        visual.add_effect(VisualEffect::monster_cast(
            8,
            4,
            MonsterKind::Necromancer,
            -1,
            0,
        ));

        assert_eq!(visual.effects.len(), 6);
        for _ in 0..16 {
            visual.tick();
        }
        assert!(visual.effects.is_empty());
    }

    #[test]
    fn damage_reaction_replaces_lower_priority_motion() {
        let mut visual = VisualState::new(80, 50);
        visual.start_player_action(PlayerActionKind::Step(PlayerStepKind::Forward));
        visual.trigger_damage_flash();
        assert_eq!(
            visual.player_action.map(|action| action.kind),
            Some(PlayerActionKind::Hit)
        );
    }

    #[test]
    fn camera_motion_has_asymmetric_step_and_attack_impulse() {
        let mut step = PlayerAction::new(PlayerActionKind::Step(PlayerStepKind::Forward));
        let start = step.camera_motion();
        step.timer -= 2;
        let planted = step.camera_motion();
        assert_ne!(start, planted);
        assert!(planted.0 > start.0);

        let mut attack = PlayerAction::new(PlayerActionKind::Attack { weapon: None });
        let anticipation = attack.camera_motion();
        attack.timer -= 4;
        let impact = attack.camera_motion();
        assert!(anticipation.0 < impact.0);
        assert!(anticipation.1 < impact.1);
    }

    #[test]
    fn room_reveal_only_restarts_on_a_new_room() {
        let mut visual = VisualState::new(80, 50);
        visual.observe_room(Some(2));
        assert_eq!(visual.room_reveal_timer, 12);
        visual.tick();
        visual.observe_room(Some(2));
        assert_eq!(visual.room_reveal_timer, 11);
        visual.observe_room(None);
        assert_eq!(visual.room_reveal_timer, 11);
        visual.observe_room(Some(3));
        assert_eq!(visual.room_reveal_timer, 12);
    }

    // ── VisualEffect::expired ──

    #[test]
    fn effect_expired_when_timer_zero() {
        let flash = VisualEffect::AttackFlash {
            x: 0,
            y: 0,
            timer: 0,
            flash_color: (255, 0, 0),
        };
        assert!(flash.expired());

        let dmg = VisualEffect::DamageNumber {
            x: 0,
            y: 0,
            amount: 5,
            color: (255, 0, 0),
            timer: 0,
        };
        assert!(dmg.expired());

        let burst = VisualEffect::DeathBurst {
            x: 0,
            y: 0,
            kind: None,
            timer: 0,
        };
        assert!(burst.expired());
    }

    #[test]
    fn effect_not_expired_when_timer_positive() {
        let fire = VisualEffect::FireRing {
            x: 0,
            y: 0,
            radius: 1.0,
            timer: 5,
        };
        assert!(!fire.expired());
    }

    // ── VisualEffect::tick ──

    #[test]
    fn effect_tick_decrements_timer() {
        let mut eff = VisualEffect::attack_flash(0, 0);
        let old_timer = match &eff {
            VisualEffect::AttackFlash { timer, .. } => *timer,
            _ => 0,
        };
        eff.tick();
        let new_timer = match &eff {
            VisualEffect::AttackFlash { timer, .. } => *timer,
            _ => 0,
        };
        assert_eq!(new_timer, old_timer - 1);
    }

    #[test]
    fn effect_tick_saturates_at_zero() {
        let mut eff = VisualEffect::AttackFlash {
            x: 0,
            y: 0,
            timer: 0,
            flash_color: (255, 0, 0),
        };
        eff.tick();
        let timer = match &eff {
            VisualEffect::AttackFlash { timer, .. } => *timer,
            _ => 1,
        };
        assert_eq!(timer, 0, "timer should not go below 0");
    }

    // ── VisualState::tick ──

    #[test]
    fn visual_state_tick_prunes_expired_effects() {
        let mut vs = VisualState::new(80, 50);
        // Add an effect that will expire immediately
        let mut eff = VisualEffect::attack_flash(0, 0);
        // Manually set timer to 1 so it expires after one tick
        if let VisualEffect::AttackFlash { timer, .. } = &mut eff {
            *timer = 1;
        }
        vs.add_effect(eff);
        vs.tick(); // timer → 0, then pruned
        assert!(
            vs.effects.is_empty(),
            "effect with timer 0 should be pruned"
        );
    }

    #[test]
    fn visual_state_tick_advances_fire_ring_radius() {
        let mut vs = VisualState::new(80, 50);
        vs.add_effect(VisualEffect::fire_ring(5, 5));
        vs.tick();
        let radius = vs
            .effects
            .iter()
            .find_map(|e| {
                if let VisualEffect::FireRing { radius, .. } = e {
                    Some(*radius)
                } else {
                    None
                }
            })
            .unwrap_or(0.0);
        assert!(radius > 0.0, "fire ring radius should advance");
    }

    #[test]
    fn visual_state_reset_clears_all() {
        let mut vs = VisualState::new(80, 50);
        vs.add_effect(VisualEffect::attack_flash(0, 0));
        vs.add_decal(Decal::Blood {
            x: 0,
            y: 0,
            fade: 255,
        });
        vs.tick = 100;
        vs.reset();
        assert_eq!(vs.tick, 0);
        assert!(vs.effects.is_empty());
        assert!(vs.decals.is_empty());
    }

    // ── wall_glyph ──

    fn make_map(tiles_5x5: &[Tile; 25]) -> Map {
        let mut map = Map::new(5, 5);
        for (i, &t) in tiles_5x5.iter().enumerate() {
            map.tiles[i] = t;
        }
        map
    }

    #[test]
    fn wall_glyph_isolated_is_full_block() {
        // Single wall tile surrounded on all 4 sides by floor -> full block
        let mut tiles = [Tile::Floor; 25];
        tiles[2 * 5 + 2] = Tile::Wall; // (2,2) isolated wall
        let map = make_map(&tiles);
        assert_eq!(wall_glyph(&map, 2, 2), '\u{2588}');
    }

    #[test]
    fn wall_glyph_vertical_line() {
        let mut tiles = [Tile::Wall; 25];
        // Floors: col 0 all floor, col 2 all floor. Wall at col 1.
        for y in 0..5 {
            tiles[(y * 5) as usize] = Tile::Floor;
            tiles[(y * 5 + 2) as usize] = Tile::Floor;
        }
        let map = make_map(&tiles);
        // Wall at (1,2) has wall above (1,1) and below (1,3), floor left (0,2) and right (2,2) → │
        assert_eq!(wall_glyph(&map, 1, 2), '\u{2502}');
    }

    #[test]
    fn wall_glyph_horizontal_line() {
        let mut tiles = [Tile::Wall; 25];
        // Floors: row 0 all floor, row 2 all floor
        for x in 0..5 {
            tiles[x as usize] = Tile::Floor;
            tiles[(2 * 5 + x) as usize] = Tile::Floor;
        }
        let map = make_map(&tiles);
        // Wall at (2,1) has wall left (1,1) and right (3,1), floor above (2,0) and below (2,2) → ─
        assert_eq!(wall_glyph(&map, 2, 1), '\u{2500}');
    }

    #[test]
    fn wall_glyph_corner_bottom_right_open() {
        // ┐ : wall below and right, floor above and left
        let mut tiles = [Tile::Wall; 25];
        for x in 0..3 {
            tiles[x as usize] = Tile::Floor;
        }
        for y in 0..3 {
            tiles[(y * 5) as usize] = Tile::Floor;
        }
        let map = make_map(&tiles);
        // (1,1): up=floor, down=wall, left=floor, right=wall → ┐
        assert_eq!(wall_glyph(&map, 1, 1), '\u{2510}');
    }

    #[test]
    fn wall_glyph_out_of_bounds_treated_as_wall() {
        let mut tiles = [Tile::Wall; 25];
        tiles[6] = Tile::Floor; // (1,1) is floor
        let map = make_map(&tiles);
        // Wall at (0,0) — top and left out of bounds (→wall), down and right are walls → ┼
        assert_eq!(wall_glyph(&map, 0, 0), '\u{253C}');
    }

    // ── Biome ──

    #[test]
    fn biome_from_depth() {
        assert_eq!(Biome::from(0), Biome::Depth0);
        assert_eq!(Biome::from(1), Biome::Depth1);
        assert_eq!(Biome::from(2), Biome::Depth2);
        assert_eq!(Biome::from(3), Biome::Depth3);
        assert_eq!(Biome::from(4), Biome::Depth4);
        assert_eq!(Biome::from(99), Biome::Depth4); // beyond max → Depth4
    }

    #[test]
    fn biome_colors_are_not_black() {
        let biomes = [
            Biome::Depth0,
            Biome::Depth1,
            Biome::Depth2,
            Biome::Depth3,
            Biome::Depth4,
        ];
        for b in &biomes {
            let fg = b.floor_visible();
            assert!(
                fg.0 > 0 || fg.1 > 0 || fg.2 > 0,
                "floor_visible for {:?} should not be pure black",
                b
            );
            let wg = b.wall_visible();
            assert!(
                wg.0 > 0 || wg.1 > 0 || wg.2 > 0,
                "wall_visible for {:?} should not be pure black",
                b
            );
            let tc = b.torch_color();
            assert!(
                tc.0 > 0 || tc.1 > 0 || tc.2 > 0,
                "torch_color for {:?} should not be pure black",
                b
            );
        }
    }

    #[test]
    fn authored_palette_families_have_distinct_materials() {
        let palettes = (0..PALETTE_FAMILY_COUNT)
            .map(|mode| world_palette(mode, Biome::Depth0))
            .map(|palette| (palette.floor, palette.wall, palette.fog, palette.light))
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(palettes.len(), PALETTE_FAMILY_COUNT as usize);
    }

    #[test]
    fn every_palette_family_has_a_name() {
        let names = (0..PALETTE_FAMILY_COUNT)
            .map(palette_name)
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(names.len(), PALETTE_FAMILY_COUNT as usize);
    }

    // ── blend ──

    #[test]
    fn blend_halfway() {
        let a: Rgb = (0, 0, 0);
        let b: Rgb = (100, 100, 100);
        let result = blend(a, b, 0.5);
        assert_eq!(result, (50, 50, 50));
    }

    #[test]
    fn blend_clamps() {
        let a: Rgb = (0, 0, 0);
        let b: Rgb = (200, 200, 200);
        assert_eq!(blend(a, b, -1.0), (0, 0, 0));
        assert_eq!(blend(a, b, 2.0), (200, 200, 200));
    }

    // ── floor_decor ──

    #[test]
    fn floor_decor_returns_none_most_of_the_time() {
        // With a fixed seed, most tiles should return None
        let mut none_count = 0;
        for x in 0..20 {
            for y in 0..20 {
                if floor_decor(42, 0, x, y).is_none() {
                    none_count += 1;
                }
            }
        }
        // Should return None for most tiles (~92%)
        assert!(none_count > 300, "expected mostly None, got {none_count}");
    }

    #[test]
    fn floor_decor_same_seed_same_result() {
        let a = floor_decor(42, 3, 10, 10);
        let b = floor_decor(42, 3, 10, 10);
        assert_eq!(a, b, "same seed/depth/position should produce same decor");
    }

    // ── monster_anim_glyph ──

    #[test]
    fn bat_animates() {
        let frame0 = monster_anim_glyph(MonsterKind::Bat, 0);
        let frame4 = monster_anim_glyph(MonsterKind::Bat, 4);
        assert_ne!(frame0, frame4, "bat should animate between frames");
    }

    #[test]
    fn non_animated_monsters_return_none() {
        assert_eq!(monster_anim_glyph(MonsterKind::Rat, 0), None);
        assert_eq!(monster_anim_glyph(MonsterKind::Goblin, 0), None);
        assert_eq!(monster_anim_glyph(MonsterKind::Skeleton, 0), None);
        assert_eq!(monster_anim_glyph(MonsterKind::Orc, 0), None);
    }

    // ── crown_anim_glyph ──

    #[test]
    fn crown_anim_cycles() {
        let g0 = crown_anim_glyph(0);
        let g2 = crown_anim_glyph(2);
        let g4 = crown_anim_glyph(4);
        assert!(g0.is_some());
        assert!(g2.is_some());
        assert!(g4.is_some());
        // Different tick phases should produce different glyphs
        assert!(
            g0 != g2 || g2 != g4 || g0 != g4,
            "crown should cycle glyphs"
        );
    }
}
