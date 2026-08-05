use rand::Rng;
use serde::{Deserialize, Serialize};

#[path = "world.rs"]
pub mod world;

use self::world::tile_terrain_rules;
#[allow(unused_imports)] // Keep the world foundation available from `crate::map`.
pub use self::world::{
    place_seeded_decorations, DecorationAnchor, DecorationAnchorKind, EnvironmentProp, MapTheme,
    MovementClass, PropKind, SurfaceKind, SurfaceMaterial, TerrainRules, WorldLink, WorldMapId,
};

/// A tile on the dungeon map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tile {
    Wall,
    Floor,
    StairsDown,
    StairsUp,
    /// A closed door — blocks FOV and movement.
    DoorClosed,
    /// An open door — passable, no FOV block.
    DoorOpen,
    /// A hidden trap — revealed when stepped on.
    TrapHidden,
    /// A revealed trap — visible hazard.
    TrapRevealed,
    /// A wall inscription — decorative lore, passable floor.
    Inscription,
    /// A decorative history feature — passable floor with distinct glyph.
    History(char),
    /// Campaign: a blue lantern (lit) — walkable, objective interactable.
    LanternLit,
    /// Campaign: a dead lantern — needs to be relit.
    LanternDead,
    /// Shallow water costs the player an additional turn to cross.
    ShallowWater,
    /// Sticky webbing restrains most creatures and can be burned away.
    Web,
    /// Requires an iron key or a sufficiently heavy weapon.
    LockedDoor,
    /// Breakable clutter that may contain a small reward.
    Urn,
    /// A breakable obstruction that blocks movement and sight.
    Barricade,
}

impl Tile {
    /// Descriptive movement, visibility, and light facts for integrations.
    /// `Map::is_walkable` and `Map::blocks_fov` remain authoritative.
    pub const fn terrain_rules(self) -> TerrainRules {
        tile_terrain_rules(self)
    }

    pub const fn movement_class(self) -> MovementClass {
        self.terrain_rules().movement
    }

    pub const fn movement_cost(self) -> u8 {
        self.terrain_rules().movement_cost()
    }

    pub const fn blocks_fov(self) -> bool {
        self.terrain_rules().blocks_fov
    }
}

/// A rectangular area on the map.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn center(&self) -> (i32, i32) {
        (self.x + self.w / 2, self.y + self.h / 2)
    }

    /// A random walkable tile inside this room.
    pub fn random_tile(&self, rng: &mut impl Rng) -> (i32, i32) {
        let x = if self.w > 1 {
            rng.gen_range(self.x..self.x + self.w)
        } else {
            self.x
        };
        let y = if self.h > 1 {
            rng.gen_range(self.y..self.y + self.h)
        } else {
            self.y
        };
        (x, y)
    }

    /// Distance between centers of two rects.
    pub fn dist_to(&self, other: &Rect) -> f64 {
        let (x1, y1) = self.center();
        let (x2, y2) = other.center();
        (((x2 - x1).pow(2) + (y2 - y1).pow(2)) as f64).sqrt()
    }
}

/// The dungeon map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Map {
    pub width: i32,
    pub height: i32,
    pub tiles: Vec<Tile>,
    /// Which tiles have been explored (seen at least once).
    pub explored: Vec<bool>,
    /// Which tiles are currently visible (in FOV).
    pub visible: Vec<bool>,
    pub rooms: Vec<Rect>,
    /// The visual style for maps without an explicit per-tile surface layer.
    #[serde(default, skip_serializing_if = "MapTheme::is_default")]
    pub theme: MapTheme,
    /// Optional visual surface metadata. Tiles still own collision and FOV.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surfaces: Option<Vec<SurfaceKind>>,
    /// Authored or seeded decorative props, independent from `tiles`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub props: Vec<EnvironmentProp>,
}

impl Map {
    pub fn new(width: i32, height: i32) -> Self {
        let size = (width * height) as usize;
        Self {
            width,
            height,
            tiles: vec![Tile::Wall; size],
            explored: vec![false; size],
            visible: vec![false; size],
            rooms: Vec::new(),
            theme: MapTheme::default(),
            surfaces: None,
            props: Vec::new(),
        }
    }

    /// Construct a map with a visual theme while keeping default tile behavior.
    pub fn with_theme(width: i32, height: i32, theme: MapTheme) -> Self {
        let mut map = Self::new(width, height);
        map.theme = theme;
        map
    }

    /// Return the explicit surface at a coordinate, or the theme fallback.
    pub fn surface_at(&self, x: i32, y: i32) -> Option<SurfaceKind> {
        let index = self.checked_idx(x, y)?;
        Some(
            self.surfaces
                .as_ref()
                .and_then(|surfaces| surfaces.get(index).copied())
                .unwrap_or_else(|| self.theme.default_surface()),
        )
    }

    /// Set a visual surface without changing the authoritative tile layer.
    pub fn set_surface(&mut self, x: i32, y: i32, surface: SurfaceKind) -> bool {
        let Some(index) = self.checked_idx(x, y) else {
            return false;
        };
        let expected_size = (self.width * self.height) as usize;
        let fallback = self.theme.default_surface();
        let surfaces = self
            .surfaces
            .get_or_insert_with(|| vec![fallback; expected_size]);
        if surfaces.len() != expected_size {
            surfaces.resize(expected_size, fallback);
            surfaces.truncate(expected_size);
        }
        surfaces[index] = surface;
        true
    }

    /// Return all props authored at a coordinate.
    pub fn props_at(&self, x: i32, y: i32) -> impl Iterator<Item = &EnvironmentProp> {
        self.props
            .iter()
            .filter(move |prop| prop.x == x && prop.y == y)
    }

    /// Replace decorative props with a deterministic seeded arrangement.
    pub fn place_seeded_decorations(&mut self, seed: u64, anchors: &[DecorationAnchor]) {
        self.props = place_seeded_decorations(seed, self.width, self.height, anchors);
    }

    /// Returns the flat index for (x, y). Assumes coordinates are in bounds.
    pub fn idx(&self, x: i32, y: i32) -> usize {
        (y * self.width + x) as usize
    }

    /// Bounds-checked alternative to `idx`. Returns `None` if out of bounds.
    #[allow(dead_code)]
    pub fn checked_idx(&self, x: i32, y: i32) -> Option<usize> {
        if self.in_bounds(x, y) {
            Some(self.idx(x, y))
        } else {
            None
        }
    }

    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && x < self.width && y >= 0 && y < self.height
    }

    pub fn is_walkable(&self, x: i32, y: i32) -> bool {
        if !self.in_bounds(x, y) {
            return false;
        }
        let t = self.tiles[self.idx(x, y)];
        !matches!(
            t,
            Tile::Wall | Tile::DoorClosed | Tile::LockedDoor | Tile::Urn | Tile::Barricade
        )
    }

    /// Whether the tile blocks line of sight (for FOV and ranged attacks).
    pub fn blocks_fov(&self, x: i32, y: i32) -> bool {
        if !self.in_bounds(x, y) {
            return true;
        }
        matches!(
            self.tiles[self.idx(x, y)],
            Tile::Wall | Tile::DoorClosed | Tile::LockedDoor | Tile::Barricade
        )
    }

    /// Like `is_walkable` but treats closed doors as passable.
    /// Used for connectivity testing and map validation.
    #[cfg(test)]
    pub fn is_connectable(&self, x: i32, y: i32) -> bool {
        if !self.in_bounds(x, y) {
            return false;
        }
        !matches!(self.tiles[self.idx(x, y)], Tile::Wall)
    }

    /// Open a closed door at (x, y). Returns true if a door was opened.
    pub fn open_door(&mut self, x: i32, y: i32) -> bool {
        if !self.in_bounds(x, y) {
            return false;
        }
        let i = self.idx(x, y);
        if self.tiles[i] == Tile::DoorClosed {
            self.tiles[i] = Tile::DoorOpen;
            true
        } else {
            false
        }
    }

    /// Trigger a hidden trap at (x, y). Returns Some(trap_name) if triggered.
    pub fn trigger_trap(&mut self, x: i32, y: i32) -> Option<&'static str> {
        if !self.in_bounds(x, y) {
            return None;
        }
        let i = self.idx(x, y);
        if self.tiles[i] == Tile::TrapHidden {
            self.tiles[i] = Tile::TrapRevealed;
            Some("A trap springs!")
        } else {
            None
        }
    }

    /// Generate a dungeon using Binary Space Partitioning.
    /// If `place_down_stairs` is false, no stairs-down tile is placed
    /// (use this for the final depth so the player cannot descend further).
    pub fn generate(&mut self, rng: &mut impl Rng, place_down_stairs: bool) {
        self.tiles.fill(Tile::Wall);
        self.explored.fill(false);
        self.visible.fill(false);
        self.rooms.clear();
        self.surfaces = None;
        self.props.clear();

        let mut leaves: Vec<Rect> = Vec::new();
        let root = Rect {
            x: 1,
            y: 1,
            w: self.width - 2,
            h: self.height - 2,
        };
        bsp_split(rng, root, &mut leaves, 5, 5);

        for leaf in &leaves {
            let room = place_room(rng, *leaf, 4, 4);
            self.carve_room(&room);
            self.rooms.push(room);
        }

        // Connect rooms by spatial proximity (MST over room centers)
        connect_rooms_spatial(rng, self);

        // Pick two distant rooms for stairs (far apart from each other)
        let n = self.rooms.len();
        let (down_pos, up_pos) = if n >= 2 {
            let dn_random = rng.gen_range(0..n);
            let up_idx = (0..n)
                .filter(|&i| i != dn_random)
                .max_by(|&a, &b| {
                    let da = self.rooms[a].dist_to(&self.rooms[dn_random]);
                    let db = self.rooms[b].dist_to(&self.rooms[dn_random]);
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap_or(if dn_random == 0 { 1 } else { 0 });

            let down = self.rooms[dn_random].center();
            let up = {
                let (ux, uy) = self.rooms[up_idx].center();
                let nx = ux + 1;
                let ny = uy;
                if self.in_bounds(nx, ny) {
                    let i = self.idx(nx, ny);
                    if !matches!(self.tiles[i], Tile::StairsDown | Tile::StairsUp) {
                        (nx, ny)
                    } else {
                        (ux, uy)
                    }
                } else {
                    (ux, uy)
                }
            };
            (Some(down), Some(up))
        } else if n == 1 {
            let (cx, cy) = self.rooms[0].center();
            let up = if self.in_bounds(cx + 1, cy) {
                (cx + 1, cy)
            } else {
                (cx, cy)
            };
            (Some((cx, cy)), Some(up))
        } else {
            (None, None)
        };

        // Place stairs tiles
        if place_down_stairs {
            if let Some((dx, dy)) = down_pos {
                let i = self.idx(dx, dy);
                self.tiles[i] = Tile::StairsDown;
            }
        }
        if let Some((ux, uy)) = up_pos {
            let i = self.idx(ux, uy);
            self.tiles[i] = Tile::StairsUp;
        }

        // Place doors at room-corridor junctions
        self.place_doors(rng);
    }

    /// Place closed doors at some corridor-into-room entry points.
    /// Scans all floor tiles adjacent to walls and replaces some with doors.
    pub fn place_doors(&mut self, rng: &mut impl Rng) {
        let w = self.width;
        let h = self.height;
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let i = self.idx(x, y);
                if self.tiles[i] != Tile::Floor {
                    continue;
                }
                // A door candidate is a floor tile that has exactly 2 opposite
                // walls (N-S or E-W), forming a doorway shape.
                let n_wall = !self.is_walkable(x, y - 1);
                let s_wall = !self.is_walkable(x, y + 1);
                let e_wall = !self.is_walkable(x + 1, y);
                let w_wall = !self.is_walkable(x - 1, y);

                let is_doorway = (n_wall && s_wall && !e_wall && !w_wall)
                    || (e_wall && w_wall && !n_wall && !s_wall);

                if is_doorway && rng.gen_bool(0.25) {
                    self.tiles[i] = Tile::DoorClosed;
                }
            }
        }
    }

    fn carve_room(&mut self, room: &Rect) {
        for y in room.y..room.y + room.h {
            for x in room.x..room.x + room.w {
                if self.in_bounds(x, y) {
                    let i = self.idx(x, y);
                    self.tiles[i] = Tile::Floor;
                }
            }
        }
    }
}

// ── BSP Dungeon Generation ──

fn bsp_split(rng: &mut impl Rng, rect: Rect, leaves: &mut Vec<Rect>, min_w: i32, min_h: i32) {
    const MAX_DIM: i32 = 10;
    let can_split_h = rect.w >= 2 * min_w;
    let can_split_v = rect.h >= 2 * min_h;

    if !can_split_h && !can_split_v {
        leaves.push(rect);
        return;
    }

    // Force split if rect exceeds max
    let must_split_h = rect.w > MAX_DIM;
    let must_split_v = rect.h > MAX_DIM;

    let split_h = if must_split_h && must_split_v {
        rng.gen_bool(0.5)
    } else if must_split_h {
        true
    } else if must_split_v {
        false
    } else if can_split_h && can_split_v {
        rng.gen_bool(0.5)
    } else {
        can_split_h
    };

    if split_h {
        let split = rng.gen_range(rect.x + min_w..=rect.x + rect.w - min_w);
        bsp_split(
            rng,
            Rect {
                x: rect.x,
                y: rect.y,
                w: split - rect.x,
                h: rect.h,
            },
            leaves,
            min_w,
            min_h,
        );
        bsp_split(
            rng,
            Rect {
                x: split,
                y: rect.y,
                w: rect.x + rect.w - split,
                h: rect.h,
            },
            leaves,
            min_w,
            min_h,
        );
    } else {
        let split = rng.gen_range(rect.y + min_h..=rect.y + rect.h - min_h);
        bsp_split(
            rng,
            Rect {
                x: rect.x,
                y: rect.y,
                w: rect.w,
                h: split - rect.y,
            },
            leaves,
            min_w,
            min_h,
        );
        bsp_split(
            rng,
            Rect {
                x: rect.x,
                y: split,
                w: rect.w,
                h: rect.y + rect.h - split,
            },
            leaves,
            min_w,
            min_h,
        );
    }
}

fn place_room(rng: &mut impl Rng, leaf: Rect, min_w: i32, min_h: i32) -> Rect {
    let room_w = rng.gen_range(min_w..=leaf.w.min(10));
    let room_h = rng.gen_range(min_h..=leaf.h.min(10));
    let room_x = if leaf.w > room_w {
        rng.gen_range(leaf.x..=leaf.x + leaf.w - room_w)
    } else {
        leaf.x
    };
    let room_y = if leaf.h > room_h {
        rng.gen_range(leaf.y..=leaf.y + leaf.h - room_h)
    } else {
        leaf.y
    };
    Rect {
        x: room_x,
        y: room_y,
        w: room_w,
        h: room_h,
    }
}

/// Connect rooms using a spatial minimum spanning tree approach.
#[allow(clippy::needless_range_loop)]
fn connect_rooms_spatial(rng: &mut impl Rng, map: &mut Map) {
    if map.rooms.len() <= 1 {
        return;
    }

    // Build an MST using Prim's algorithm on room centers.
    let n = map.rooms.len();
    let mut connected = vec![false; n];
    let mut edges: Vec<(usize, usize)> = Vec::with_capacity(n - 1);
    connected[0] = true;

    for _ in 0..n - 1 {
        let mut best_dist = f64::MAX;
        let mut best_edge = (0, 0);
        for i in 0..n {
            if !connected[i] {
                continue;
            }
            for j in 0..n {
                if connected[j] {
                    continue;
                }
                let d = map.rooms[i].dist_to(&map.rooms[j]);
                if d < best_dist {
                    best_dist = d;
                    best_edge = (i, j);
                }
            }
        }
        connected[best_edge.1] = true;
        edges.push(best_edge);
    }

    // Carve corridors for each MST edge
    for (a, b) in edges {
        carve_corridor(map, map.rooms[a].center(), map.rooms[b].center(), rng);
    }

    // Add 1-2 extra corridors for looping paths (non-tree connections)
    let extra = rng.gen_range(0..=n / 2);
    for _ in 0..extra {
        let a = rng.gen_range(0..n);
        let b = rng.gen_range(0..n);
        if a != b {
            carve_corridor(map, map.rooms[a].center(), map.rooms[b].center(), rng);
        }
    }
}

pub fn carve_corridor(map: &mut Map, from: (i32, i32), to: (i32, i32), rng: &mut impl Rng) {
    let (mut x, mut y) = from;
    let (tx, ty) = to;
    let h_first = rng.gen_bool(0.5);

    let mut carve = |x: i32, y: i32| {
        if map.in_bounds(x, y) {
            let i = map.idx(x, y);
            if !matches!(
                map.tiles[i],
                Tile::StairsDown | Tile::StairsUp | Tile::DoorClosed | Tile::DoorOpen
            ) {
                map.tiles[i] = Tile::Floor;
            }
        }
    };

    if h_first {
        while x != tx {
            x += if tx > x { 1 } else { -1 };
            carve(x, y);
        }
        while y != ty {
            y += if ty > y { 1 } else { -1 };
            carve(x, y);
        }
    } else {
        while y != ty {
            y += if ty > y { 1 } else { -1 };
            carve(x, y);
        }
        while x != tx {
            x += if tx > x { 1 } else { -1 };
            carve(x, y);
        }
    }
}
