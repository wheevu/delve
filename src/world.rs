use std::collections::HashSet;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

use super::Tile;

/// The visual language used by a map. It does not change tile collision or FOV.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MapTheme {
    #[default]
    Dungeon,
    Village,
    Road,
    Ruins,
    Shrine,
}

impl MapTheme {
    /// The surface to use when a map has no explicit surface layer.
    pub const fn default_surface(self) -> SurfaceKind {
        match self {
            Self::Dungeon => SurfaceKind::Floor(SurfaceMaterial::Stone),
            Self::Village => SurfaceKind::Road(SurfaceMaterial::Cobblestone),
            Self::Road => SurfaceKind::Road(SurfaceMaterial::Dirt),
            Self::Ruins => SurfaceKind::Floor(SurfaceMaterial::Moss),
            Self::Shrine => SurfaceKind::Floor(SurfaceMaterial::Timber),
        }
    }

    pub const fn is_default(&self) -> bool {
        matches!(self, Self::Dungeon)
    }
}

/// A visual material that can be used by a surface.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SurfaceMaterial {
    #[default]
    Stone,
    Cobblestone,
    Dirt,
    Grass,
    Timber,
    Moss,
}

/// The semantic surface under a tile.
///
/// Surfaces are descriptive metadata. `Tile` remains authoritative for
/// collision and line of sight in the current map implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SurfaceKind {
    Floor(SurfaceMaterial),
    Road(SurfaceMaterial),
    Ground(SurfaceMaterial),
    Mud,
    ShallowWater,
    Bridge(SurfaceMaterial),
}

impl Default for SurfaceKind {
    fn default() -> Self {
        Self::Floor(SurfaceMaterial::Stone)
    }
}

impl SurfaceKind {
    pub const fn material(self) -> Option<SurfaceMaterial> {
        match self {
            Self::Floor(material)
            | Self::Road(material)
            | Self::Ground(material)
            | Self::Bridge(material) => Some(material),
            Self::Mud | Self::ShallowWater => None,
        }
    }

    pub const fn is_road(self) -> bool {
        matches!(self, Self::Road(_))
    }

    pub const fn movement_class(self) -> MovementClass {
        match self {
            Self::Mud | Self::ShallowWater => MovementClass::Slow,
            Self::Floor(_) | Self::Road(_) | Self::Ground(_) | Self::Bridge(_) => {
                MovementClass::Normal
            }
        }
    }

    pub const fn movement_cost(self) -> u8 {
        self.movement_class().cost()
    }

    pub const fn blocks_fov(self) -> bool {
        false
    }

    pub const fn terrain_rules(self) -> TerrainRules {
        TerrainRules {
            movement: self.movement_class(),
            blocks_fov: self.blocks_fov(),
            light_radius: 0,
        }
    }
}

/// Coarse movement classification used by terrain and prop metadata.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MovementClass {
    #[default]
    Normal,
    Slow,
    Blocked,
}

impl MovementClass {
    pub const fn cost(self) -> u8 {
        match self {
            Self::Normal => 1,
            Self::Slow => 2,
            Self::Blocked => 0,
        }
    }
}

/// The small set of traversal and visibility facts needed by later systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerrainRules {
    pub movement: MovementClass,
    pub blocks_fov: bool,
    pub light_radius: u8,
}

impl TerrainRules {
    pub const fn open() -> Self {
        Self {
            movement: MovementClass::Normal,
            blocks_fov: false,
            light_radius: 0,
        }
    }

    pub const fn slow() -> Self {
        Self {
            movement: MovementClass::Slow,
            blocks_fov: false,
            light_radius: 0,
        }
    }

    pub const fn blocked(blocks_fov: bool) -> Self {
        Self {
            movement: MovementClass::Blocked,
            blocks_fov,
            light_radius: 0,
        }
    }

    pub const fn movement_cost(self) -> u8 {
        self.movement.cost()
    }
}

/// A decorative or authored environmental object at a map coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentProp {
    pub x: i32,
    pub y: i32,
    pub kind: PropKind,
}

impl EnvironmentProp {
    pub const fn new(x: i32, y: i32, kind: PropKind) -> Self {
        Self { x, y, kind }
    }

    pub const fn position(self) -> (i32, i32) {
        (self.x, self.y)
    }

    pub const fn terrain_rules(self) -> TerrainRules {
        self.kind.terrain_rules()
    }
}

/// Medieval world props that can be rendered or given a later interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PropKind {
    RoadMarker,
    Signpost,
    Brazier,
    Torch,
    Gate,
    Rubble,
    Tree,
    Cart,
    Barrel,
    Crate,
    Banner,
    Shrine,
}

impl PropKind {
    pub const fn terrain_rules(self) -> TerrainRules {
        match self {
            Self::Gate | Self::Rubble | Self::Tree => TerrainRules::blocked(true),
            Self::Brazier => TerrainRules {
                movement: MovementClass::Normal,
                blocks_fov: false,
                light_radius: 5,
            },
            Self::Torch => TerrainRules {
                movement: MovementClass::Normal,
                blocks_fov: false,
                light_radius: 3,
            },
            Self::RoadMarker
            | Self::Signpost
            | Self::Cart
            | Self::Barrel
            | Self::Crate
            | Self::Banner
            | Self::Shrine => TerrainRules::open(),
        }
    }

    #[allow(dead_code)] // Public world contract used by later transition systems.
    pub const fn movement_class(self) -> MovementClass {
        self.terrain_rules().movement
    }

    #[allow(dead_code)] // Public world contract used by later transition systems.
    pub const fn movement_cost(self) -> u8 {
        self.terrain_rules().movement_cost()
    }

    #[allow(dead_code)] // Public world contract used by later transition systems.
    pub const fn blocks_fov(self) -> bool {
        self.terrain_rules().blocks_fov
    }

    #[allow(dead_code)] // Public world contract used by later transition systems.
    pub const fn light_radius(self) -> u8 {
        self.terrain_rules().light_radius
    }
}

/// Stable identifiers for the first authored world route and dungeon floors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorldMapId {
    Village,
    OldKingsRoad,
    BellLineGate,
    Dungeon { depth: u8 },
}

/// An authored transition between two discrete world maps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldLink {
    pub from: WorldMapId,
    pub from_position: (i32, i32),
    pub to: WorldMapId,
    pub to_position: (i32, i32),
}

impl WorldLink {
    pub const fn new(
        from: WorldMapId,
        from_position: (i32, i32),
        to: WorldMapId,
        to_position: (i32, i32),
    ) -> Self {
        Self {
            from,
            from_position,
            to,
            to_position,
        }
    }

    #[allow(dead_code)] // Public world contract used by later transition systems.
    pub fn destination(
        self,
        map: WorldMapId,
        position: (i32, i32),
    ) -> Option<(WorldMapId, (i32, i32))> {
        if self.from == map && self.from_position == position {
            Some((self.to, self.to_position))
        } else {
            None
        }
    }
}

/// The kind of authored feature used as a seed for decorative placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecorationAnchorKind {
    Road,
    Landmark,
}

/// A coordinate around which deterministic decoration may be placed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecorationAnchor {
    pub x: i32,
    pub y: i32,
    pub kind: DecorationAnchorKind,
}

impl DecorationAnchor {
    pub const fn road(x: i32, y: i32) -> Self {
        Self {
            x,
            y,
            kind: DecorationAnchorKind::Road,
        }
    }

    pub const fn landmark(x: i32, y: i32) -> Self {
        Self {
            x,
            y,
            kind: DecorationAnchorKind::Landmark,
        }
    }

    pub const fn position(self) -> (i32, i32) {
        (self.x, self.y)
    }
}

/// Place stable, nearby props around authored road and landmark anchors.
///
/// The seed, map dimensions, and ordered anchors fully determine the result.
/// Anchor coordinates themselves are reserved so a caller can use the same
/// list for a road or landmark without overwriting its authored tile.
pub fn place_seeded_decorations(
    seed: u64,
    width: i32,
    height: i32,
    anchors: &[DecorationAnchor],
) -> Vec<EnvironmentProp> {
    if width <= 0 || height <= 0 || anchors.is_empty() {
        return Vec::new();
    }

    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut occupied: HashSet<(i32, i32)> =
        anchors.iter().map(|anchor| anchor.position()).collect();
    let offsets = [
        (1, 0),
        (-1, 0),
        (0, 1),
        (0, -1),
        (1, 1),
        (-1, 1),
        (1, -1),
        (-1, -1),
    ];
    let mut props = Vec::new();

    for anchor in anchors {
        let count = match anchor.kind {
            DecorationAnchorKind::Road => 1,
            DecorationAnchorKind::Landmark => 2,
        };
        let first_offset = rng.gen_range(0..offsets.len());
        let mut placed = 0;

        for attempt in 0..offsets.len() {
            if placed == count {
                break;
            }
            let (dx, dy) = offsets[(first_offset + attempt) % offsets.len()];
            let position = (anchor.x + dx, anchor.y + dy);
            if position.0 < 0
                || position.0 >= width
                || position.1 < 0
                || position.1 >= height
                || !occupied.insert(position)
            {
                continue;
            }

            let kind = match anchor.kind {
                DecorationAnchorKind::Road => match rng.gen_range(0..3) {
                    0 => PropKind::RoadMarker,
                    1 => PropKind::Signpost,
                    _ => PropKind::Torch,
                },
                DecorationAnchorKind::Landmark => match rng.gen_range(0..4) {
                    0 => PropKind::Brazier,
                    1 => PropKind::Banner,
                    2 => PropKind::Rubble,
                    _ => PropKind::Shrine,
                },
            };
            props.push(EnvironmentProp::new(position.0, position.1, kind));
            placed += 1;
        }
    }

    props.sort_by_key(|prop| (prop.y, prop.x));
    props
}

/// Classify an existing authoritative tile without changing map behavior.
pub const fn tile_terrain_rules(tile: Tile) -> TerrainRules {
    match tile {
        Tile::Wall | Tile::DoorClosed | Tile::LockedDoor | Tile::Barricade => {
            TerrainRules::blocked(true)
        }
        Tile::Urn => TerrainRules::blocked(false),
        Tile::ShallowWater | Tile::Web => TerrainRules::slow(),
        Tile::Floor
        | Tile::StairsDown
        | Tile::StairsUp
        | Tile::DoorOpen
        | Tile::TrapHidden
        | Tile::TrapRevealed
        | Tile::Inscription
        | Tile::History(_)
        | Tile::LanternLit
        | Tile::LanternDead => TerrainRules::open(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::Map;

    #[test]
    fn old_map_data_defaults_to_empty_world_metadata() {
        let json = r#"{
            "width": 2,
            "height": 2,
            "tiles": ["Floor", "Floor", "Floor", "Floor"],
            "explored": [false, false, false, false],
            "visible": [false, false, false, false],
            "rooms": []
        }"#;
        let map: Map = serde_json::from_str(json).unwrap();

        assert_eq!(map.theme, MapTheme::Dungeon);
        assert!(map.surfaces.is_none());
        assert!(map.props.is_empty());
        assert_eq!(
            map.surface_at(0, 0),
            Some(SurfaceKind::Floor(SurfaceMaterial::Stone))
        );
    }

    #[test]
    fn default_map_serialization_omits_empty_world_metadata() {
        let map = Map::new(2, 2);
        let value = serde_json::to_value(map).unwrap();

        assert!(value.get("theme").is_none());
        assert!(value.get("surfaces").is_none());
        assert!(value.get("props").is_none());
    }

    #[test]
    fn world_metadata_round_trips_through_map_serialization() {
        let mut map = Map::with_theme(3, 3, MapTheme::Road);
        assert!(map.set_surface(1, 1, SurfaceKind::Mud));
        map.props
            .push(EnvironmentProp::new(2, 1, PropKind::Brazier));

        let json = serde_json::to_string(&map).unwrap();
        let loaded: Map = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.theme, MapTheme::Road);
        assert_eq!(loaded.surface_at(1, 1), Some(SurfaceKind::Mud));
        assert_eq!(loaded.props_at(2, 1).count(), 1);
        assert_eq!(loaded.props[0].kind, PropKind::Brazier);
    }

    #[test]
    fn surface_and_prop_metadata_do_not_override_authoritative_tiles() {
        let mut map = Map::new(4, 4);
        map.tiles.fill(Tile::Floor);
        map.set_surface(1, 1, SurfaceKind::Mud);
        map.props.push(EnvironmentProp::new(2, 2, PropKind::Gate));

        assert!(map.is_walkable(1, 1));
        assert!(!map.blocks_fov(1, 1));
        assert!(map.is_walkable(2, 2));
        assert!(!map.blocks_fov(2, 2));

        let barricade = map.idx(3, 3);
        map.tiles[barricade] = Tile::Barricade;
        assert!(!map.is_walkable(3, 3));
        assert!(map.blocks_fov(3, 3));
    }

    #[test]
    fn medieval_surface_and_prop_rules_are_readable() {
        assert_eq!(
            SurfaceKind::Road(SurfaceMaterial::Cobblestone).movement_class(),
            MovementClass::Normal
        );
        assert_eq!(SurfaceKind::Mud.movement_cost(), 2);
        assert_eq!(SurfaceKind::ShallowWater.movement_cost(), 2);
        assert_eq!(
            SurfaceKind::Bridge(SurfaceMaterial::Timber).movement_cost(),
            1
        );
        assert_eq!(PropKind::Gate.movement_class(), MovementClass::Blocked);
        assert!(PropKind::Rubble.blocks_fov());
        assert_eq!(PropKind::Brazier.light_radius(), 5);
        assert_eq!(PropKind::Torch.light_radius(), 3);
        assert_eq!(
            tile_terrain_rules(Tile::ShallowWater).movement,
            MovementClass::Slow
        );
        assert!(tile_terrain_rules(Tile::DoorClosed).blocks_fov);
    }

    #[test]
    fn seeded_decorations_are_deterministic_and_stay_near_anchors() {
        let anchors = [
            DecorationAnchor::road(8, 8),
            DecorationAnchor::landmark(14, 12),
        ];
        let first = place_seeded_decorations(42, 24, 20, &anchors);
        let second = place_seeded_decorations(42, 24, 20, &anchors);

        assert_eq!(first, second);
        assert_eq!(first.len(), 3);
        assert!(first
            .iter()
            .all(|prop| { prop.x >= 0 && prop.x < 24 && prop.y >= 0 && prop.y < 20 }));
        assert!(first.iter().all(|prop| {
            anchors
                .iter()
                .any(|anchor| (prop.x - anchor.x).abs() <= 1 && (prop.y - anchor.y).abs() <= 1)
        }));
        assert!(first
            .iter()
            .enumerate()
            .all(|(index, prop)| first[index + 1..]
                .iter()
                .all(|other| prop.position() != other.position())));
    }

    #[test]
    fn authored_links_resolve_only_at_their_entry() {
        let link = WorldLink::new(
            WorldMapId::Village,
            (5, 7),
            WorldMapId::OldKingsRoad,
            (1, 3),
        );

        assert_eq!(
            link.destination(WorldMapId::Village, (5, 7)),
            Some((WorldMapId::OldKingsRoad, (1, 3)))
        );
        assert_eq!(link.destination(WorldMapId::Village, (5, 6)), None);
    }
}
