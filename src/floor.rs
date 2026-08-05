use crate::entity::{Camp, Item, Landmark, Monster, Npc, Projectile};
use crate::map::Map;

pub(crate) const MAX_CACHED_FLOORS: usize = 5;

/// A sound or disturbance that the floor can still remember when revisited.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct NoiseEvent {
    pub x: i32,
    pub y: i32,
    pub kind: NoiseKind,
}

/// Delve's small attention vocabulary. The values describe consequences, not
/// volume simulation: only committed turns can create one of these events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum NoiseKind {
    Quiet,
    Movement,
    Impact,
    Alarm,
}

impl NoiseKind {
    pub(crate) const fn attention(self) -> u8 {
        match self {
            Self::Quiet => 0,
            Self::Movement => 1,
            Self::Impact => 3,
            Self::Alarm => 6,
        }
    }
}

/// Persistent, intentionally compact memory for one floor.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct FloorMemory {
    pub attention: u8,
    pub room_alert: Vec<u8>,
    pub last_noise: Option<NoiseEvent>,
    pub crown_pressure_applied: bool,
    pub hunter_spawned: bool,
    #[serde(default)]
    pub quiet_turns: u8,
}

impl FloorMemory {
    pub(crate) fn new(room_count: usize) -> Self {
        Self {
            attention: 0,
            room_alert: vec![0; room_count],
            last_noise: None,
            crown_pressure_applied: false,
            hunter_spawned: false,
            quiet_turns: 0,
        }
    }

    pub(crate) fn reset_for_rooms(&mut self, room_count: usize) {
        *self = Self::new(room_count);
    }
}

impl Default for FloorMemory {
    fn default() -> Self {
        Self::new(0)
    }
}

/// All mutable world state needed to restore a visited floor.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct FloorState {
    pub depth: u32,
    pub map: Map,
    pub monsters: Vec<Monster>,
    pub items: Vec<Item>,
    pub npcs: Vec<Npc>,
    pub projectiles: Vec<Projectile>,
    pub landmarks: Vec<(usize, Landmark)>,
    pub used_landmarks: Vec<usize>,
    pub camps: Vec<Camp>,
    pub memory: FloorMemory,
}
