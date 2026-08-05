use crate::app::{
    ChallengeMode, GameMode, GameStatus, PreparationMenu, RunStats, HINT_INTERACT, HINT_INVENTORY,
    HINT_MAP, HINT_NAVIGATION,
};
use crate::campaign::{self, CampaignFlag, CampaignObjective};
use crate::entity::{
    Camp, Item, ItemKind, Landmark, Monster, MonsterKind, Npc, NpcKind, Player, Projectile,
    TechniqueKind,
};
use crate::floor::FloorMemory;
use crate::image_render::HalfBlockCanvas;
use crate::map::{
    Map, MapTheme, PropKind, SurfaceKind, SurfaceMaterial, Tile, WorldLink, WorldMapId,
};
use crate::ui::{MessageLog, MsgColor};
use crate::visual::{
    self, Biome, PlayerActionKind, PlayerStepKind, PlayerTurnKind, VisualEffect, VisualState,
};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Gauge, Paragraph, Wrap};
use ratatui::Frame;

// ══════════════════════════════════════════════════════════════════════════════
// COLOR PALETTE
// ══════════════════════════════════════════════════════════════════════════════

const PLAYER_COLOR: Color = Color::Rgb(0, 255, 136);
const GOBLIN_COLOR: Color = Color::Rgb(255, 68, 68);
const ORC_COLOR: Color = Color::Rgb(204, 34, 34);
const SKELETON_COLOR: Color = Color::Rgb(192, 192, 192);
const RAT_COLOR: Color = Color::Rgb(160, 130, 90);
const BAT_COLOR: Color = Color::Rgb(150, 150, 200);
const SPIDER_COLOR: Color = Color::Rgb(160, 60, 160);
const WRAITH_COLOR: Color = Color::Rgb(100, 200, 255);
const SLIME_COLOR: Color = Color::Rgb(100, 220, 100);
const MIMIC_COLOR: Color = Color::Rgb(180, 140, 60);
const NECROMANCER_COLOR: Color = Color::Rgb(180, 60, 180);
const SENTINEL_COLOR: Color = Color::Rgb(200, 180, 60);
const GUARDIAN_COLOR: Color = Color::Rgb(255, 50, 50);

const NPC_COLOR: Color = Color::Rgb(100, 255, 180);
const SHRINE_COLOR: Color = Color::Rgb(255, 215, 130);

const ITEM_POTION_COLOR: Color = Color::Rgb(255, 100, 255);
const ITEM_SCROLL_COLOR: Color = Color::Rgb(255, 255, 100);
const ITEM_WEAPON_COLOR: Color = Color::Rgb(200, 200, 200);
const ITEM_ARMOR_COLOR: Color = Color::Rgb(100, 180, 255);
const ITEM_CROWN_COLOR: Color = Color::Rgb(255, 215, 0);

const PANEL_BG: Color = Color::Rgb(18, 18, 26);
const DIALOG_BG: Color = Color::Rgb(28, 22, 18);
const DIALOG_BORDER: Color = Color::Rgb(166, 132, 82);
const PORTRAIT_BG: Color = Color::Rgb(38, 28, 20);
const TEXT_DIM: Color = Color::Rgb(110, 110, 120);
const TEXT_BRIGHT: Color = Color::Rgb(220, 220, 230);
const HP_BAR_FULL: Color = Color::Rgb(0, 255, 136);
const HP_BAR_MID: Color = Color::Rgb(255, 204, 0);
const HP_BAR_LOW: Color = Color::Rgb(255, 68, 68);

const FOV_RADIUS: i32 = 8;

/// All data needed for one frame of rendering.
pub struct RenderCtx<'a> {
    pub map: &'a Map,
    pub world_map_id: WorldMapId,
    pub world_links: &'a [WorldLink],
    pub player: &'a Player,
    pub monsters: &'a [Monster],
    pub items: &'a [Item],
    pub npcs: &'a [Npc],
    pub projectiles: &'a [Projectile],
    pub landmarks: &'a [(usize, Landmark)],
    pub used_landmarks: &'a std::collections::HashSet<usize>,
    pub seen_monsters: &'a std::collections::HashSet<MonsterKind>,
    pub defeated_monsters: &'a std::collections::HashSet<MonsterKind>,
    pub messages: &'a MessageLog,
    pub status: GameStatus,
    pub stats: &'a RunStats,
    pub seed: u64,
    pub challenge_mode: ChallengeMode,
    pub floor_memory: &'a FloorMemory,
    pub cached_floor_count: usize,
    pub completed_runs: usize,
    pub run_history: &'a [crate::run_history::RunSummary],
    pub inventory_cursor: usize,
    pub auto_pickup: bool,
    pub log_scroll: usize,
    pub visual: &'a VisualState,
    /// Animation tick for stair glow and other time-based visuals.
    pub tick: u64,
    /// Active dialog: NPC name.
    pub dialog_npc_name: &'a str,
    /// Active dialog: exact NPC index, when the speaker is an NPC on this floor.
    pub dialog_npc_idx: Option<usize>,
    /// Active dialog: text lines.
    pub dialog_text: &'a [String],
    /// Whether the dialog is currently showing player choices.
    pub dialog_show_choices: bool,
    /// Choice labels to display (e.g. "Ask about the Crown").
    pub dialog_choices: &'a [String],
    /// Camps on the current floor.
    pub camps: &'a [Camp],
    /// Current game mode.
    pub game_mode: GameMode,
    /// Campaign objectives (active only in campaign mode).
    pub campaign_objectives: &'a [CampaignObjective],
    /// Campaign flags (for dialog gating).
    pub campaign_flags: &'a [crate::campaign::CampaignFlag],
    /// Campaign level name.
    pub campaign_level_name: &'a str,
    /// Whether to show the minimap overlay.
    pub show_minimap: bool,
    /// Whether the controls help overlay is open.
    pub show_help: bool,
    pub show_bestiary: bool,
    pub auto_exploring: bool,
    pub reduced_flash: bool,
    pub high_contrast: bool,
    pub show_log_overlay: bool,
    pub color_filter: u8,
    pub player_moving: bool,
    pub camera_bob: i32,
    pub camera_sway: i32,
    pub low_motion: bool,
    pub preparation_menu: Option<PreparationMenu>,
    pub hints_seen: u8,
    /// Player-facing angle in radians; 0 faces east, PI/2 faces south.
    pub camera_angle: f64,
}

// ══════════════════════════════════════════════════════════════════════════════
// MAIN RENDER DISPATCH
// ══════════════════════════════════════════════════════════════════════════════

pub fn render(frame: &mut Frame, ctx: &RenderCtx) {
    match ctx.status {
        GameStatus::Menu => {
            render_menu_v2(frame, ctx);
            if ctx.show_help {
                render_help_overlay(frame, frame.area());
            }
        }
        GameStatus::CampaignMenu => {
            render_campaign_menu(frame, ctx);
            if ctx.show_help {
                render_help_overlay(frame, frame.area());
            }
        }
        GameStatus::Playing | GameStatus::Inventory | GameStatus::QuestPanel => {
            render_game(frame, ctx)
        }
        GameStatus::Dead => render_end_screen(frame, ctx, "── YOU DIED ──", Color::Red),
        GameStatus::Won => {
            render_end_screen(frame, ctx, "★★★ YOU WON! ★★★", Color::Rgb(255, 215, 0))
        }
        GameStatus::CampaignComplete => render_campaign_complete_screen(frame, ctx),
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// GAME VIEW (playing + inventory)
// ══════════════════════════════════════════════════════════════════════════════

fn render_game(frame: &mut Frame, ctx: &RenderCtx) {
    let area = frame.area();

    // The compact game layout is intentionally designed for an 80×24
    // terminal. Below that size, a clear resize prompt is more useful than
    // silently clipping the objective, sidebar, or message log.
    if area.width < 80 || area.height < 24 {
        let msg = "Terminal too small. Resize to at least 80×24.";
        frame.render_widget(
            Paragraph::new(msg)
                .centered()
                .style(Style::default().fg(Color::Red)),
            area,
        );
        return;
    }

    // Stairs transition fade
    if let Some(eff) = ctx
        .visual
        .effects
        .iter()
        .find(|e| matches!(e, VisualEffect::StairsTransition { .. }))
    {
        let t = match eff {
            VisualEffect::StairsTransition { timer } => *timer,
            _ => 0,
        };
        if t > 0 {
            let _fade = t as f32 / 20.0;
            frame
                .buffer_mut()
                .set_style(area, Style::default().bg(Color::Black));
            let p = Paragraph::new("").style(Style::default().bg(Color::Black));
            frame.render_widget(p, area);
            frame.render_widget(
                Paragraph::new("").style(Style::default().bg(Color::Black)),
                area,
            );
            return; // Skip normal rendering during transition
        }
    }

    let log_height = if area.height < 30 { 3 } else { 5 };
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(log_height),
        ])
        .split(area);

    let sidebar_width = if area.width < 105 { 22 } else { 28 };
    let game_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Fill(1), Constraint::Length(sidebar_width)])
        .split(main_chunks[0]);

    // ── Player-centered third-person dungeon, with optional full map overlay ──
    render_dungeon_viewport(frame, game_chunks[0], ctx);
    if ctx.show_minimap {
        render_minimap(frame.buffer_mut(), game_chunks[0], ctx);
    }

    // ── Sidebar ──
    render_sidebar(frame, game_chunks[1], ctx);

    render_context_strip(frame, main_chunks[1], ctx);

    // ── Message log ──
    render_messages(frame, main_chunks[2], ctx);

    // ── Inventory overlay ──
    if ctx.status == GameStatus::Inventory {
        render_inventory_overlay(frame, area, ctx);
    }

    // ── Quest panel overlay ──
    if ctx.status == GameStatus::QuestPanel {
        render_quest_panel(frame, area, ctx);
    }

    // ── Dialog modal ──
    if !ctx.dialog_text.is_empty() {
        render_dialog_overlay(frame, area, ctx);
    }
    if ctx.preparation_menu.is_some() {
        render_preparation_overlay(frame, area, ctx);
    }

    if ctx.show_help {
        render_help_overlay(frame, area);
    }
    if ctx.show_bestiary {
        render_bestiary_overlay(frame, area, ctx);
    }
    if ctx.show_log_overlay {
        render_log_overlay(frame, area, ctx);
    }
    if ctx.player.pending_level_ups > 0 {
        render_discipline_overlay(frame, area, ctx);
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// LAYER 1: TERRAIN — tile colors with depth palette + distance lighting
// ══════════════════════════════════════════════════════════════════════════════

fn render_dungeon_viewport(frame: &mut Frame, area: Rect, ctx: &RenderCtx) {
    crate::third_person::render_viewport(frame, area, ctx);
}

// ── Tile style: color based on biome, distance lighting, and tile type ──

#[allow(clippy::too_many_arguments)]
fn tile_style(
    tile: Tile,
    visible: bool,
    biome: Biome,
    dist: f64,
    radius: i32,
    seed: u64,
    depth: u32,
    x: i32,
    y: i32,
    tick: u64,
) -> (char, Color, Color) {
    if visible {
        let base_fg = match tile {
            Tile::Wall => biome.wall_visible(),
            Tile::Floor => biome.floor_visible(),
            Tile::StairsDown => (255, 210, 20),
            Tile::StairsUp => {
                if depth >= 4 {
                    (180, 230, 255)
                } else {
                    (255, 170, 80)
                }
            }
            Tile::DoorClosed => (218, 154, 78),
            Tile::DoorOpen => (162, 128, 84),
            Tile::TrapHidden => biome.floor_visible(),
            Tile::TrapRevealed => (255, 80, 80),
            Tile::Inscription => (180, 230, 255),
            Tile::History(_) => biome.floor_visible(),
            Tile::LanternLit => (100, 200, 255),
            Tile::LanternDead => (60, 60, 80),
            Tile::ShallowWater => (74, 148, 178),
            Tile::Web => (188, 184, 196),
            Tile::LockedDoor => (194, 132, 58),
            Tile::Urn => (184, 104, 58),
            Tile::Barricade => (142, 92, 54),
        };
        let base_bg = biome.floor_visible();
        let sample = TerrainSample {
            tile,
            biome,
            seed,
            depth,
            x,
            y,
            visible: true,
        };
        let (base_fg, base_bg) = terrain_variation(sample, base_fg, base_bg);

        // Distance-based torch lighting
        let max_dist = radius as f64;
        let falloff = (dist / max_dist).clamp(0.0, 1.0);
        // Smoothstep for nicer falloff
        let t = (falloff * falloff * (3.0 - 2.0 * falloff)) as f32;
        let torch = biome.torch_color();
        let fog = biome.fog_color();
        let lit_bg = visual::blend(base_bg, fog, t);
        let lit_fg = visual::blend(base_fg, fog, t);

        // Slight torch tint at close range
        let torch_blend = ((1.0 - falloff).max(0.0).powf(2.0)) as f32;
        let mut bg = visual::blend(lit_bg, torch, torch_blend * 0.25);

        // Stair glow: subtle tick-based brightness pulse
        let stair_glow = if matches!(tile, Tile::StairsDown | Tile::StairsUp) {
            let wave = ((tick as f64 * 0.15).sin() * 0.5 + 0.5) as f32;
            0.15 + wave * 0.20 // 15-35% brightness boost
        } else {
            0.0
        };

        let fg = match tile {
            Tile::StairsDown => {
                let base = (255, 210, 20);
                let (r, g, b) = visual::blend(base, torch, torch_blend * 0.5);
                (
                    (r as f32 + (255.0 - r as f32) * stair_glow) as u8,
                    (g as f32 + (255.0 - g as f32) * stair_glow * 0.5) as u8,
                    b,
                )
            }
            Tile::StairsUp => {
                let base = if depth >= 4 {
                    (180, 230, 255)
                } else {
                    (255, 170, 80)
                };
                let (r, g, b) = visual::blend(base, torch, torch_blend * 0.5);
                (
                    (r as f32 + (255.0 - r as f32) * stair_glow) as u8,
                    (g as f32 + (255.0 - g as f32) * stair_glow) as u8,
                    b,
                )
            }
            _ => lit_fg,
        };

        if matches!(tile, Tile::StairsDown | Tile::StairsUp) {
            let stair_tint = match tile {
                Tile::StairsDown => (255, 210, 20),
                Tile::StairsUp if depth >= 4 => (180, 230, 255),
                Tile::StairsUp => (255, 170, 80),
                _ => unreachable!(),
            };
            let glow = ((1.0 - falloff) as f32 * 0.14 + stair_glow * 0.12).min(0.24);
            bg = visual::blend(bg, stair_tint, glow);
        } else if tile == Tile::DoorClosed {
            bg = visual::blend(bg, (100, 62, 28), (1.0 - falloff) as f32 * 0.12);
        }

        // Wall glyphs are computed in render_dungeon_viewport
        // where the map reference is available.
        let ch = match tile {
            Tile::Wall => '\u{2588}', // placeholder, overridden in viewport
            Tile::Floor => visual::floor_decor(seed, depth, x, y).unwrap_or(' '),
            Tile::StairsDown => '>',
            Tile::StairsUp => '<',
            Tile::DoorClosed => '+',
            Tile::DoorOpen => '╱',
            Tile::TrapHidden => ' ', // invisible until revealed
            Tile::TrapRevealed => '^',
            Tile::Inscription => '\u{00BF}', // ¿
            Tile::History(c) => c,
            Tile::LanternLit => '\u{2668}',
            Tile::LanternDead => '\u{25CB}',
            Tile::ShallowWater => '~',
            Tile::Web => ':',
            Tile::LockedDoor => '#',
            Tile::Urn => 'u',
            Tile::Barricade => 'X',
        };

        (
            ch,
            Color::Rgb(fg.0, fg.1, fg.2),
            Color::Rgb(bg.0, bg.1, bg.2),
        )
    } else {
        // Explored but not visible
        let bg = biome.floor_explored();
        let fg = if tile == Tile::Wall {
            biome.wall_explored()
        } else if tile == Tile::DoorClosed {
            (92, 62, 34)
        } else if tile == Tile::StairsDown {
            // Dim gold for explored stairs-down
            (100, 80, 10)
        } else if tile == Tile::StairsUp {
            if depth >= 4 {
                (70, 90, 100)
            } else {
                (100, 65, 30)
            }
        } else if tile == Tile::LanternLit {
            (50, 100, 140)
        } else if tile == Tile::LanternDead {
            (40, 40, 50)
        } else if tile == Tile::ShallowWater {
            (38, 72, 88)
        } else if tile == Tile::Web {
            (76, 72, 82)
        } else if tile == Tile::LockedDoor {
            (86, 56, 28)
        } else if tile == Tile::Urn {
            (78, 48, 34)
        } else if tile == Tile::Barricade {
            (66, 46, 32)
        } else {
            biome.floor_explored()
        };
        let sample = TerrainSample {
            tile,
            biome,
            seed,
            depth,
            x,
            y,
            visible: false,
        };
        let (fg, bg) = terrain_variation(sample, fg, bg);
        let ch = if tile == Tile::Wall {
            '\u{2588}'
        } else if tile == Tile::DoorClosed {
            '+'
        } else if tile == Tile::DoorOpen {
            '\u{2571}'
        } else if tile == Tile::StairsDown {
            '>'
        } else if tile == Tile::StairsUp {
            '<'
        } else if tile == Tile::LanternLit {
            '\u{2668}'
        } else if tile == Tile::LanternDead {
            '\u{25CB}'
        } else if tile == Tile::ShallowWater {
            '~'
        } else if tile == Tile::Web {
            ':'
        } else if tile == Tile::LockedDoor {
            '#'
        } else if tile == Tile::Urn {
            'u'
        } else if tile == Tile::Barricade {
            'X'
        } else {
            ' '
        };
        (
            ch,
            Color::Rgb(fg.0, fg.1, fg.2),
            Color::Rgb(bg.0, bg.1, bg.2),
        )
    }
}

fn surface_style(
    surface: Option<SurfaceKind>,
    visible: bool,
    biome: Biome,
    dist: f64,
    radius: i32,
) -> (char, Color, Color) {
    let surface = surface.unwrap_or(SurfaceKind::Floor(SurfaceMaterial::Stone));
    let (mut glyph, color) = match surface {
        SurfaceKind::Floor(material)
        | SurfaceKind::Ground(material)
        | SurfaceKind::Road(material)
        | SurfaceKind::Bridge(material) => match material {
            SurfaceMaterial::Stone => (' ', biome.floor_visible()),
            SurfaceMaterial::Cobblestone => ('·', (168, 140, 104)),
            SurfaceMaterial::Dirt => (';', (132, 92, 56)),
            SurfaceMaterial::Grass => (',', (102, 136, 76)),
            SurfaceMaterial::Timber => ('=', (148, 102, 62)),
            SurfaceMaterial::Moss => (',', (108, 136, 84)),
        },
        SurfaceKind::Mud => (';', (112, 76, 50)),
        SurfaceKind::ShallowWater => ('~', (78, 158, 184)),
    };
    if surface.is_road() && surface.material() == Some(SurfaceMaterial::Stone) {
        glyph = '·';
    }
    let color = if visible {
        depth_color(
            Color::Rgb(color.0, color.1, color.2),
            biome,
            dist,
            radius,
            0.38,
        )
    } else {
        Color::Rgb(color.0 / 2, color.1 / 2, color.2 / 2)
    };
    let background = if visible {
        let floor = biome.floor_visible();
        depth_color(
            Color::Rgb(floor.0, floor.1, floor.2),
            biome,
            dist,
            radius,
            0.22,
        )
    } else {
        Color::Rgb(10, 10, 12)
    };
    (glyph, color, background)
}

fn prop_glyph(kind: PropKind, tick: u64) -> char {
    match kind {
        PropKind::RoadMarker => '│',
        PropKind::Signpost => '⚑',
        PropKind::Brazier => {
            if tick % 8 < 4 {
                '♨'
            } else {
                '✦'
            }
        }
        PropKind::Torch => {
            if tick % 8 < 4 {
                '†'
            } else {
                '‡'
            }
        }
        PropKind::Gate => '╬',
        PropKind::Rubble => '∴',
        PropKind::Tree => '♣',
        PropKind::Cart => '▣',
        PropKind::Barrel => '◉',
        PropKind::Crate => '▤',
        PropKind::Banner => '⚑',
        PropKind::Shrine => '♰',
    }
}

fn prop_color(kind: PropKind) -> Color {
    match kind {
        PropKind::Brazier | PropKind::Torch => Color::Rgb(248, 166, 66),
        PropKind::Tree => Color::Rgb(92, 142, 76),
        PropKind::Banner | PropKind::Signpost => Color::Rgb(196, 132, 72),
        PropKind::Gate | PropKind::Rubble => Color::Rgb(164, 134, 96),
        PropKind::Cart | PropKind::Barrel | PropKind::Crate => Color::Rgb(172, 118, 66),
        PropKind::RoadMarker | PropKind::Shrine => Color::Rgb(210, 178, 110),
    }
}

#[derive(Clone, Copy)]
struct TerrainSample {
    tile: Tile,
    biome: Biome,
    seed: u64,
    depth: u32,
    x: i32,
    y: i32,
    visible: bool,
}

fn terrain_variation(
    sample: TerrainSample,
    fg: visual::Rgb,
    bg: visual::Rgb,
) -> (visual::Rgb, visual::Rgb) {
    let (accent, strength) = match sample.tile {
        Tile::Wall => (
            wall_accent(sample.biome),
            if sample.visible { 0.18 } else { 0.08 },
        ),
        Tile::Floor
        | Tile::DoorClosed
        | Tile::DoorOpen
        | Tile::TrapHidden
        | Tile::Inscription
        | Tile::History(_) => (
            floor_accent(sample.biome),
            if sample.visible { 0.14 } else { 0.06 },
        ),
        Tile::LockedDoor | Tile::Urn | Tile::Barricade => (
            floor_accent(sample.biome),
            if sample.visible { 0.16 } else { 0.07 },
        ),
        Tile::ShallowWater | Tile::Web => (
            floor_accent(sample.biome),
            if sample.visible { 0.2 } else { 0.08 },
        ),
        _ => return (fg, bg),
    };

    let bg = vary_rgb(bg, accent, sample, strength, 0x9e37_79b9);
    let fg = vary_rgb(fg, accent, sample, strength * 0.65, 0x85eb_ca6b);
    (fg, bg)
}

fn floor_accent(biome: Biome) -> visual::Rgb {
    match biome {
        Biome::Depth0 => (68, 90, 48),
        Biome::Depth1 => (48, 86, 118),
        Biome::Depth2 => (110, 66, 42),
        Biome::Depth3 => (74, 44, 92),
        Biome::Depth4 => (78, 66, 118),
    }
}

fn wall_accent(biome: Biome) -> visual::Rgb {
    match biome {
        Biome::Depth0 => (116, 112, 72),
        Biome::Depth1 => (88, 114, 150),
        Biome::Depth2 => (130, 70, 48),
        Biome::Depth3 => (116, 74, 124),
        Biome::Depth4 => (100, 88, 126),
    }
}

fn vary_rgb(
    base: visual::Rgb,
    accent: visual::Rgb,
    sample: TerrainSample,
    strength: f32,
    salt: u64,
) -> visual::Rgb {
    let h = terrain_hash(sample, salt);
    let shade = 1.0 + (((h & 0xff) as f32 / 255.0) - 0.5) * strength;
    let shaded = scale_rgb(base, shade);
    let accent_mix = (((h >> 8) & 0x0f) as f32 / 15.0) * strength;
    visual::blend(shaded, accent, accent_mix)
}

fn terrain_hash(sample: TerrainSample, salt: u64) -> u32 {
    let mut h = sample.seed ^ salt ^ ((sample.depth as u64) << 32);
    h = h.wrapping_add((sample.x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    h = h.wrapping_add((sample.y as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9));
    h ^= h >> 30;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 27;
    h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
    (h ^ (h >> 31)) as u32
}

fn scale_rgb((r, g, b): visual::Rgb, factor: f32) -> visual::Rgb {
    let scale = |v: u8| ((v as f32 * factor).round()).clamp(0.0, 255.0) as u8;
    (scale(r), scale(g), scale(b))
}

// ══════════════════════════════════════════════════════════════════════════════
// MINIMAP
// ══════════════════════════════════════════════════════════════════════════════

const MINIMAP_W: u16 = 24;
const MINIMAP_H: u16 = 12;
const MINIMAP_BG: Color = Color::Rgb(8, 8, 12);
const MINIMAP_BORDER: Color = Color::Rgb(70, 70, 86);

/// Render a compact whole-floor minimap in the top-right of the dungeon area.
pub(crate) fn render_minimap(buf: &mut Buffer, area: Rect, ctx: &RenderCtx) {
    let map = ctx.map;
    if map.width <= 0 || map.height <= 0 || area.width < 18 || area.height < 10 {
        return;
    }

    let inner_w = MINIMAP_W
        .min(area.width.saturating_sub(4).max(1))
        .min(map.width.max(1) as u16);
    let inner_h = MINIMAP_H
        .min(area.height.saturating_sub(5).max(1))
        .min(map.height.max(1) as u16);
    let total_w = inner_w + 2;
    let total_h = inner_h + 3;
    let x0 = area.x + area.width.saturating_sub(total_w + 1);
    let y0 = area.y + 1;
    let biome: Biome = ctx.player.depth.into();
    let light_radius = render_light_radius(ctx.player);
    let vpx = (ctx.player.x - inner_w as i32 / 2).clamp(0, map.width - inner_w as i32);
    let vpy = (ctx.player.y - inner_h as i32 / 2).clamp(0, map.height - inner_h as i32);
    let (facing_x, facing_y) = facing_delta(ctx.camera_angle);
    let ahead = (ctx.player.x + facing_x, ctx.player.y + facing_y);

    draw_minimap_frame(buf, x0, y0, total_w, total_h);

    for dy in 0..inner_h {
        for dx in 0..inner_w {
            let mx = vpx + dx as i32;
            let my = vpy + dy as i32;
            let render_x = x0 + 1 + dx;
            let render_y = y0 + 1 + dy;
            if !map.in_bounds(mx, my) {
                buf[(render_x, render_y)]
                    .set_char(' ')
                    .set_fg(Color::Black)
                    .set_bg(MINIMAP_BG);
                continue;
            }

            let idx = map.idx(mx, my);
            let tile = map.tiles[idx];
            let visible = map.visible[idx];
            let explored = map.explored[idx];
            if !visible && !explored {
                buf[(render_x, render_y)]
                    .set_char(' ')
                    .set_fg(Color::Black)
                    .set_bg(MINIMAP_BG);
                continue;
            }

            let dist = (((mx - ctx.player.x).pow(2) + (my - ctx.player.y).pow(2)) as f64).sqrt();
            let (mut ch, mut fg, mut bg) = tile_style(
                tile,
                visible,
                biome,
                dist,
                light_radius,
                ctx.seed,
                ctx.player.depth,
                mx,
                my,
                ctx.tick,
            );
            let (surface_ch, surface_fg, surface_bg) =
                surface_style(map.surface_at(mx, my), visible, biome, dist, light_radius);
            if matches!(
                tile,
                Tile::Floor | Tile::DoorOpen | Tile::TrapHidden | Tile::History(_)
            ) {
                ch = surface_ch;
                fg = surface_fg;
                // Keep the map's fog treatment while allowing road and ground
                // materials to read as distinct routes at a glance.
                if visible {
                    bg = surface_bg;
                }
            }
            if tile == Tile::Wall {
                ch = visual::wall_glyph(map, mx, my);
            }
            let mut entity: Option<(char, Color)> = None;
            if mx == ctx.player.x && my == ctx.player.y {
                entity = Some((minimap_player_marker(ctx.camera_angle), PLAYER_COLOR));
            } else if visible {
                if let Some(monster) = ctx
                    .monsters
                    .iter()
                    .find(|m| m.alive && m.x == mx && m.y == my)
                {
                    entity = Some((
                        monster_render_glyph(monster.kind, ctx.visual.tick),
                        depth_color(monster_color(monster.kind), biome, dist, light_radius, 0.50),
                    ));
                } else if let Some(item) = ctx.items.iter().find(|i| i.x == mx && i.y == my) {
                    let glyph = visual::item_floor_glyph(item.kind, ctx.visual.tick)
                        .unwrap_or_else(|| item.kind.symbol());
                    entity = Some((
                        glyph,
                        depth_color(item_color(item.kind), biome, dist, light_radius, 0.42),
                    ));
                } else if let Some(npc) = ctx.npcs.iter().find(|n| n.x == mx && n.y == my) {
                    entity = Some((
                        npc.kind.symbol(),
                        depth_color(NPC_COLOR, biome, dist, light_radius, 0.42),
                    ));
                } else if let Some(prop) = map.props_at(mx, my).next() {
                    entity = Some((
                        prop_glyph(prop.kind, ctx.tick),
                        depth_color(prop_color(prop.kind), biome, dist, light_radius, 0.35),
                    ));
                } else if ctx.projectiles.iter().any(|p| p.x == mx && p.y == my) {
                    entity = Some(('*', NECROMANCER_COLOR));
                }
            }

            let player_cell = mx == ctx.player.x && my == ctx.player.y;
            let (ch, fg) = entity.unwrap_or((ch, fg));
            let bg = if player_cell {
                Color::Rgb(22, 20, 16)
            } else if (mx, my) == ahead {
                Color::Rgb(58, 43, 18)
            } else if visible {
                bg
            } else {
                MINIMAP_BG
            };
            let cell = &mut buf[(render_x, render_y)];
            cell.set_char(ch).set_fg(fg).set_bg(bg);
        }
    }
    draw_minimap_legend(buf, x0 + 1, y0 + total_h - 2, inner_w);
}

fn draw_minimap_frame(buf: &mut Buffer, x0: u16, y0: u16, w: u16, h: u16) {
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            buf[(x, y)]
                .set_char(' ')
                .set_fg(MINIMAP_BORDER)
                .set_bg(MINIMAP_BG);
        }
    }
    for x in x0 + 1..x0 + w - 1 {
        buf[(x, y0)].set_char('─');
        buf[(x, y0 + h - 1)].set_char('─');
    }
    for y in y0 + 1..y0 + h - 1 {
        buf[(x0, y)].set_char('│');
        buf[(x0 + w - 1, y)].set_char('│');
    }
    buf[(x0, y0)].set_char('┌');
    buf[(x0 + w - 1, y0)].set_char('┐');
    buf[(x0, y0 + h - 1)].set_char('└');
    buf[(x0 + w - 1, y0 + h - 1)].set_char('┘');

    for (i, ch) in " map ".chars().enumerate() {
        if x0 + 2 + (i as u16) < x0 + w - 1 {
            buf[(x0 + 2 + i as u16, y0)]
                .set_char(ch)
                .set_fg(TEXT_DIM)
                .set_bg(MINIMAP_BG);
        }
    }
}

fn draw_minimap_legend(buf: &mut Buffer, x: u16, y: u16, width: u16) {
    let legend = "^you ·aim dim:memory";
    for (offset, ch) in legend.chars().take(width as usize).enumerate() {
        let fg = if offset == 0 {
            PLAYER_COLOR
        } else if (5..=8).contains(&offset) {
            Color::Rgb(238, 174, 64)
        } else if offset >= 14 {
            TEXT_DIM
        } else {
            Color::Rgb(176, 176, 188)
        };
        buf[(x + offset as u16, y)]
            .set_char(ch)
            .set_fg(fg)
            .set_bg(MINIMAP_BG);
    }
}

pub(crate) fn render_light_radius(player: &Player) -> i32 {
    let lantern_bonus = if player.lantern_turns > 0 { 3 } else { 0 };
    let crown_penalty = if player.crown_held { -2 } else { 0 };
    (FOV_RADIUS + lantern_bonus + crown_penalty).max(4)
}

fn depth_color(base: Color, biome: Biome, dist: f64, radius: i32, max_fog: f32) -> Color {
    let Some(rgb) = color_to_rgb(base) else {
        return base;
    };
    let falloff = (dist / radius as f64).clamp(0.0, 1.0) as f32;
    let near_boost = if falloff < 0.28 {
        1.22 - falloff * 0.45
    } else {
        1.0
    };
    let rgb = scale_rgb(rgb, near_boost);
    let fog_t = falloff.powf(1.25) * max_fog;
    rgb_to_color(visual::blend(rgb, biome.fog_color(), fog_t))
}

fn color_to_rgb(color: Color) -> Option<visual::Rgb> {
    match color {
        Color::Rgb(r, g, b) => Some((r, g, b)),
        _ => None,
    }
}

fn rgb_to_color((r, g, b): visual::Rgb) -> Color {
    Color::Rgb(r, g, b)
}

// ── Entity color mapping ──

fn monster_render_glyph(kind: MonsterKind, tick: visual::Tick) -> char {
    if let Some(glyph) = visual::monster_anim_glyph(kind, tick) {
        return glyph;
    }

    match kind {
        MonsterKind::Skeleton => '☠',
        MonsterKind::Slime => '●',
        MonsterKind::Mimic => '▣',
        MonsterKind::Necromancer => '♜',
        MonsterKind::Sentinel => '◆',
        MonsterKind::Guardian => 'Ω',
        MonsterKind::Stalker => '◈',
        MonsterKind::BellWarden => '⦿',
        _ => kind.symbol(),
    }
}

fn monster_color(kind: MonsterKind) -> Color {
    match kind {
        MonsterKind::Rat => RAT_COLOR,
        MonsterKind::Bat => BAT_COLOR,
        MonsterKind::Goblin => GOBLIN_COLOR,
        MonsterKind::Skeleton => SKELETON_COLOR,
        MonsterKind::Orc => ORC_COLOR,
        MonsterKind::Spider => SPIDER_COLOR,
        MonsterKind::Wraith => WRAITH_COLOR,
        MonsterKind::Slime => SLIME_COLOR,
        MonsterKind::Mimic => MIMIC_COLOR,
        MonsterKind::Necromancer => NECROMANCER_COLOR,
        MonsterKind::Sentinel => SENTINEL_COLOR,
        MonsterKind::Guardian => GUARDIAN_COLOR,
        MonsterKind::Stalker => Color::Rgb(166, 90, 202),
        MonsterKind::BellWarden => Color::Rgb(224, 164, 68),
    }
}

fn item_color(kind: ItemKind) -> Color {
    match kind {
        ItemKind::HealthPotion
        | ItemKind::LargeHealthPotion
        | ItemKind::Antidote
        | ItemKind::LanternOil
        | ItemKind::SmokeBomb => ITEM_POTION_COLOR,
        ItemKind::ScrollOfFire
        | ItemKind::ScrollOfConfusion
        | ItemKind::ScrollOfBlink
        | ItemKind::EnchantmentScroll
        | ItemKind::LoreFragment => ITEM_SCROLL_COLOR,
        ItemKind::RustySword | ItemKind::Longsword | ItemKind::Warhammer => ITEM_WEAPON_COLOR,
        ItemKind::LeatherArmor | ItemKind::Chainmail | ItemKind::Shield => ITEM_ARMOR_COLOR,
        ItemKind::CrownOfDepth => ITEM_CROWN_COLOR,
        ItemKind::BellToken => Color::Rgb(180, 160, 100),
        ItemKind::IronKey => Color::Rgb(202, 174, 104),
        ItemKind::Emberbrand => Color::Rgb(244, 104, 42),
        ItemKind::TideglassMail => Color::Rgb(74, 190, 220),
        ItemKind::Graveward => Color::Rgb(190, 184, 214),
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// SIDEBAR
// ══════════════════════════════════════════════════════════════════════════════

fn render_sidebar(frame: &mut Frame, area: Rect, ctx: &RenderCtx) {
    let player = ctx.player;
    let stats = ctx.stats;
    let seed = ctx.seed;
    let depth = player.depth;
    let threat = priority_threat(ctx);

    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(Style::default().fg(TEXT_DIM))
        .style(Style::default().bg(PANEL_BG));

    let inner = block.inner(area);
    frame.render_widget(block, area);
    let compact = inner.width < 26 || inner.height < 28;

    // HP gauge
    let hp_pct = if player.stats.max_hp > 0 {
        player.stats.hp as f32 / player.stats.max_hp as f32
    } else {
        0.0
    };
    let hp_color = if hp_pct > 0.6 {
        HP_BAR_FULL
    } else if hp_pct > 0.3 {
        HP_BAR_MID
    } else {
        HP_BAR_LOW
    };

    // Depth progress indicator: 5 dots
    let depth_dots: String = (0..5)
        .map(|d| {
            if d <= depth as usize {
                "\u{25C9}" // ◉ filled
            } else {
                "\u{25CB}" // ○ empty
            }
        })
        .collect::<Vec<_>>()
        .join("");

    let depth_name = match ctx.map.theme {
        MapTheme::Village => "Warden's Village",
        MapTheme::Road => "Old King's Road",
        MapTheme::Ruins => "Bell-Line Ruins",
        MapTheme::Shrine => "Pilgrim Shrine",
        MapTheme::Dungeon => match depth {
            0 => "Surface Cavern",
            1 => "Frozen Depths",
            2 => "Rust Crypts",
            3 => "Spider Lairs",
            4 => "Void Chamber",
            _ => "???",
        },
    };

    let mut lines = vec![
        Line::from(Span::styled(
            format!(" {} {}", depth_dots, depth_name),
            Style::default().fg(Color::Rgb(200, 180, 120)),
        )),
        Line::from(""),
        // HP bar using Gauge
    ];

    if !ctx.floor_memory.room_alert.is_empty() {
        let attention = usize::from(ctx.floor_memory.attention);
        let filled = (attention / 10).min(10);
        lines.push(Line::from(vec![
            Span::styled("Memory ", Style::default().fg(TEXT_DIM)),
            Span::styled(
                format!(
                    "{}{} {}%",
                    "▰".repeat(filled),
                    "·".repeat(10 - filled),
                    attention
                ),
                Style::default().fg(if attention >= 60 {
                    Color::Rgb(236, 104, 80)
                } else if attention >= 25 {
                    Color::Rgb(216, 174, 84)
                } else {
                    Color::Rgb(112, 182, 164)
                }),
            ),
        ]));
        if ctx.cached_floor_count > 0 {
            lines.push(Line::from(Span::styled(
                format!("Remembered floors: {}", ctx.cached_floor_count),
                Style::default().fg(TEXT_DIM),
            )));
        }
    }

    let heading = facing_name(ctx.camera_angle);
    let mut facing = vec![Span::styled("Facing ", Style::default().fg(TEXT_DIM))];
    for direction in ["N", "E", "S", "W"] {
        let active = direction == heading;
        facing.push(Span::styled(
            if active {
                format!("[{direction}] ")
            } else {
                format!(" {direction}  ")
            },
            if active {
                Style::default()
                    .fg(Color::Rgb(255, 204, 76))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Rgb(78, 78, 90))
            },
        ));
    }
    lines.push(Line::from(facing));
    let (posture, posture_color) = player_posture(ctx);
    lines.push(Line::from(vec![
        Span::styled("Posture ", Style::default().fg(TEXT_DIM)),
        Span::styled(posture, Style::default().fg(posture_color)),
    ]));

    // Render HP gauge separately as a widget
    let gauge_area = Rect {
        x: inner.x,
        y: inner.y + lines.len() as u16,
        width: inner.width.min(24),
        height: 1,
    };

    let hp_label = format!("HP {}/{}", player.stats.hp, player.stats.max_hp);
    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(hp_color).bg(Color::Rgb(30, 30, 38)))
        .percent((hp_pct * 100.0) as u16)
        .label(hp_label);
    frame.render_widget(gauge, gauge_area);

    lines.push(Line::from(""));
    lines.push(Line::from(""));

    lines.push(Line::from(vec![
        Span::raw("ATK: "),
        Span::styled(
            format!("{}", player.total_attack()),
            Style::default().fg(Color::Rgb(255, 170, 80)),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::raw("DEF: "),
        Span::styled(
            format!("{}", player.total_defense()),
            Style::default().fg(Color::Rgb(100, 160, 255)),
        ),
    ]));
    let stamina = usize::from(player.stamina.min(player.max_stamina));
    let max_stamina = usize::from(player.max_stamina);
    lines.push(Line::from(vec![
        Span::raw("STA: "),
        Span::styled(
            format!(
                "{}{}",
                "█".repeat(stamina),
                "░".repeat(max_stamina - stamina)
            ),
            Style::default().fg(if player.stamina >= 3 {
                Color::Rgb(126, 208, 136)
            } else {
                Color::Rgb(224, 126, 70)
            }),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::raw("LVL: "),
        Span::styled(
            format!("{}", player.level),
            Style::default().fg(Color::Rgb(255, 255, 100)),
        ),
        Span::raw("  XP: "),
        Span::styled(
            format!("{}/{}", player.xp, player.xp_to_next()),
            Style::default().fg(Color::Rgb(180, 255, 180)),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::raw("Gold: "),
        Span::styled(
            format!("{}", player.gold),
            Style::default().fg(Color::Rgb(255, 215, 0)),
        ),
    ]));

    if let Some(monster) = threat {
        let health = monster.stats.hp.max(0) as f32 / monster.stats.max_hp.max(1) as f32;
        let filled = (health * 10.0).ceil().clamp(0.0, 10.0) as usize;
        let bar = format!("{}{}", "█".repeat(filled), "░".repeat(10 - filled));
        let threat_color = if monster.windup > 0 {
            Color::Rgb(255, 174, 54)
        } else {
            Color::Rgb(255, 92, 72)
        };
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "══ Threat ══",
            Style::default()
                .fg(threat_color)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(vec![
            Span::styled(
                monster.kind.name().to_uppercase(),
                Style::default()
                    .fg(threat_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {}/{}", monster.stats.hp, monster.stats.max_hp),
                Style::default().fg(TEXT_BRIGHT),
            ),
        ]));
        lines.push(Line::from(Span::styled(
            bar,
            Style::default().fg(threat_color),
        )));
        lines.push(Line::from(vec![
            Span::styled("State  ", Style::default().fg(TEXT_DIM)),
            Span::styled(monster_condition(monster), Style::default().fg(TEXT_BRIGHT)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Intent ", Style::default().fg(TEXT_DIM)),
            Span::styled(
                monster_intent(monster, player),
                Style::default().fg(if monster.windup > 0 {
                    threat_color
                } else {
                    TEXT_BRIGHT
                }),
            ),
        ]));
    }
    lines.push(Line::from(""));

    if !compact {
        lines.push(Line::from(vec![
            Span::styled("Route ", Style::default().fg(TEXT_DIM)),
            Span::styled(
                format!(
                    "{} · {} links",
                    world_location_name(ctx.world_map_id),
                    ctx.world_links.len()
                ),
                Style::default().fg(Color::Rgb(178, 148, 96)),
            ),
        ]));
        lines.push(Line::from(""));
        // Gear section
        lines.push(Line::from(Span::styled(
            "══ Gear ══",
            Style::default().fg(TEXT_DIM),
        )));
        if let Some(w) = &player.weapon {
            lines.push(Line::from(vec![
                Span::raw("W "),
                Span::styled(w.name(), Style::default().fg(ITEM_WEAPON_COLOR)),
                Span::styled(
                    format!(
                        " (+{} ATK{}{})",
                        w.attack_bonus(),
                        if player.weapon_durability > 0 {
                            format!(" · {} durability", player.weapon_durability)
                        } else {
                            String::new()
                        },
                        if player.weapon_cursed {
                            " · CURSED"
                        } else {
                            ""
                        }
                    ),
                    Style::default().fg(TEXT_DIM),
                ),
            ]));
        }
        if let Some(a) = &player.armor {
            lines.push(Line::from(vec![
                Span::raw("A "),
                Span::styled(a.name(), Style::default().fg(ITEM_ARMOR_COLOR)),
                Span::styled(
                    format!(
                        " (+{} DEF{})",
                        a.defense_bonus(),
                        if player.armor_cursed {
                            " · CURSED"
                        } else {
                            ""
                        }
                    ),
                    Style::default().fg(TEXT_DIM),
                ),
            ]));
        }
        if player.weapon.is_none() && player.armor.is_none() {
            lines.push(Line::from(Span::styled(
                "(none)",
                Style::default().fg(TEXT_DIM),
            )));
        }
        let technique = TechniqueKind::from_loadout(player);
        lines.push(Line::from(vec![
            Span::styled("T ", Style::default().fg(Color::Rgb(255, 202, 92))),
            Span::styled(technique.name(), Style::default().fg(TEXT_BRIGHT)),
            Span::styled(
                format!(" ({} STA)", technique.cost()),
                Style::default().fg(TEXT_DIM),
            ),
        ]));
        if let Some(mastery) = player
            .weapon_mastery
            .filter(|mastery| mastery.compatible_with(player.weapon))
        {
            lines.push(Line::from(vec![
                Span::styled("M ", Style::default().fg(Color::Rgb(196, 134, 255))),
                Span::styled(mastery.name(), Style::default().fg(TEXT_BRIGHT)),
            ]));
        }
        lines.push(Line::from(Span::styled(
            format!(
                "V{} F{} R{}",
                player.vigor_rank, player.finesse_rank, player.resolve_rank
            ),
            Style::default().fg(TEXT_DIM),
        )));

        if threat.is_none() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "══ Run ══",
                Style::default().fg(TEXT_DIM),
            )));
            lines.push(Line::from(Span::raw(format!("Turns: {}", stats.turns))));
            lines.push(Line::from(Span::raw(format!("Kills: {}", stats.kills))));
            lines.push(Line::from(Span::raw(format!(
                "Items: {}",
                stats.items_used
            ))));
            lines.push(Line::from(Span::raw(format!(
                "Guarded: {}",
                stats.damage_guarded
            ))));
            lines.push(Line::from(Span::raw(format!(
                "Forced/Disarmed: {}/{}",
                stats.doors_forced, stats.traps_disarmed
            ))));
            lines.push(Line::from(Span::raw(format!(
                "Inv: {}/26",
                player.inventory.len()
            ))));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!("Seed {}", seed),
                Style::default().fg(TEXT_DIM),
            )));
        }
    }

    // Campaign objectives
    if ctx.game_mode == GameMode::Campaign && !ctx.campaign_objectives.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!(
                "\u{2550}\u{2550} Chapter I {}/3 \u{2550}\u{2550}",
                (ctx.player.depth + 1).min(3)
            ),
            Style::default()
                .fg(Color::Rgb(255, 215, 0))
                .add_modifier(Modifier::BOLD),
        )));
        if !ctx.campaign_level_name.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("  {}", ctx.campaign_level_name),
                Style::default().fg(Color::Rgb(200, 180, 100)),
            )));
        }
        for obj in ctx.campaign_objectives {
            lines.push(Line::from(Span::styled(
                format!("  \u{25C6} {}", obj.display()),
                Style::default().fg(Color::Rgb(180, 220, 255)),
            )));
        }
    }

    // Landmark room name
    let player_room_idx = ctx.map.rooms.iter().position(|r| {
        player.x >= r.x && player.x < r.x + r.w && player.y >= r.y && player.y < r.y + r.h
    });
    if let Some(pri) = player_room_idx {
        if let Some((_, lm)) = ctx.landmarks.iter().find(|(ri, _)| *ri == pri) {
            lines.push(Line::from(Span::styled(
                format!("Landmark: {}", lm.name()),
                Style::default().fg(SHRINE_COLOR),
            )));
            lines.push(Line::from(Span::styled(
                lm.description(),
                Style::default().fg(TEXT_DIM),
            )));
        }
        // Camp room name
        let camp_in_room = ctx.camps.iter().find(|c| c.room_idx == pri);
        if let Some(camp) = camp_in_room {
            lines.push(Line::from(Span::styled(
                format!("Camp: {}", camp.kind.name()),
                Style::default().fg(Color::Rgb(120, 200, 255)),
            )));
        }
    }

    // Lantern oil indicator
    if player.lantern_turns > 0 {
        lines.push(Line::from(Span::styled(
            format!("Lantern {}t", player.lantern_turns),
            Style::default().fg(Color::Rgb(255, 220, 100)),
        )));
    }

    // Status effects
    if player.poisoned {
        lines.push(Line::from(Span::styled(
            format!(
                "POISONED {}",
                "\u{2588}".repeat(player.poison_ticks as usize)
            ),
            Style::default().fg(SPIDER_COLOR),
        )));
    }
    if player.guarding {
        let stability = 2 + player.armor.map_or(0, |armor| armor.defense_bonus() * 2);
        lines.push(Line::from(Span::styled(
            format!("GUARDING +2 DEF · STABILITY {stability}"),
            Style::default().fg(Color::Rgb(120, 190, 255)),
        )));
    }
    if player.focus_turns > 0 {
        lines.push(Line::from(Span::styled(
            format!("FOCUS +1 ATK  {}t", player.focus_turns),
            Style::default().fg(Color::Rgb(238, 166, 72)),
        )));
    }
    if player.ward_turns > 0 {
        lines.push(Line::from(Span::styled(
            format!("WARDED +1 DEF {}t", player.ward_turns),
            Style::default().fg(Color::Rgb(112, 196, 184)),
        )));
    }
    if let Some(item) = player.quick_item {
        lines.push(Line::from(Span::styled(
            format!("F quick: {}", item.display_name(player)),
            Style::default().fg(item_color(item)),
        )));
    }
    if player.crown_held || player.inventory.contains(&ItemKind::CrownOfDepth) {
        lines.push(Line::from(Span::styled(
            "THE CROWN PULLS UPWARD",
            Style::default().fg(ITEM_CROWN_COLOR),
        )));
        lines.push(Line::from(Span::styled(
            if player.depth == 0 {
                "SURFACE EXIT ON THIS FLOOR".to_string()
            } else {
                format!("CROWN WAKE · {} ASCENTS REMAIN", player.depth)
            },
            Style::default()
                .fg(Color::Rgb(228, 112, 62))
                .add_modifier(Modifier::BOLD),
        )));
    }

    // Auto-pickup indicator
    if ctx.auto_pickup {
        lines.push(Line::from(Span::styled(
            "[auto] Pickup ON",
            Style::default().fg(Color::Rgb(100, 200, 100)),
        )));
    }
    if ctx.reduced_flash || ctx.high_contrast || ctx.low_motion {
        lines.push(Line::from(Span::styled(
            format!(
                "View:{}{}{}",
                if ctx.reduced_flash { " low-flash" } else { "" },
                if ctx.high_contrast { " contrast" } else { "" },
                if ctx.low_motion { " low-motion" } else { "" }
            ),
            Style::default().fg(TEXT_DIM),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "? controls   m map",
        Style::default().fg(TEXT_DIM),
    )));

    let p = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(p, inner);
}

fn world_location_name(id: WorldMapId) -> &'static str {
    match id {
        WorldMapId::Village => "Warden's Village",
        WorldMapId::OldKingsRoad => "Old King's Road",
        WorldMapId::BellLineGate => "Bell-Line Gate",
        WorldMapId::Dungeon { depth: 0 } => "Surface Cavern",
        WorldMapId::Dungeon { depth } => match depth {
            1 => "Frozen Depths",
            2 => "Rust Crypts",
            3 => "Spider Lairs",
            4 => "Void Chamber",
            _ => "Unknown Depth",
        },
    }
}

fn player_posture(ctx: &RenderCtx<'_>) -> (String, Color) {
    let critical = ctx.player.stats.hp * 10 <= ctx.player.stats.max_hp.max(1) * 3;
    let Some(action) = ctx.visual.player_action else {
        return if ctx.player.counter_turns > 0 {
            ("COUNTER READY".into(), Color::Rgb(255, 192, 72))
        } else if ctx.player.guarding {
            (
                format!("GUARD · {} stability", ctx.player.stamina),
                Color::Rgb(120, 190, 255),
            )
        } else if critical {
            ("WOUNDED · limping".into(), HP_BAR_LOW)
        } else if ctx.player.stamina <= 2 {
            ("WINDED · recover".into(), Color::Rgb(224, 154, 82))
        } else {
            ("ready".into(), TEXT_BRIGHT)
        };
    };
    let phase = match action.phase() {
        0 => "prepare",
        1 => "commit",
        _ => "recover",
    };
    let (name, color) = match action.kind {
        PlayerActionKind::Step(PlayerStepKind::Forward) => ("advance", TEXT_BRIGHT),
        PlayerActionKind::Step(PlayerStepKind::Backward) => ("withdraw", TEXT_BRIGHT),
        PlayerActionKind::Step(PlayerStepKind::StrafeLeft) => ("strafe left", TEXT_BRIGHT),
        PlayerActionKind::Step(PlayerStepKind::StrafeRight) => ("strafe right", TEXT_BRIGHT),
        PlayerActionKind::Dodge(PlayerStepKind::StrafeLeft) => {
            ("EVADE LEFT", Color::Rgb(126, 208, 176))
        }
        PlayerActionKind::Dodge(PlayerStepKind::StrafeRight) => {
            ("EVADE RIGHT", Color::Rgb(126, 208, 176))
        }
        PlayerActionKind::Dodge(_) => ("EVADE", Color::Rgb(126, 208, 176)),
        PlayerActionKind::Turn(PlayerTurnKind::Left) => ("turn left", TEXT_DIM),
        PlayerActionKind::Turn(PlayerTurnKind::Right) => ("turn right", TEXT_DIM),
        PlayerActionKind::Attack { .. } => ("ATTACK", Color::Rgb(255, 174, 72)),
        PlayerActionKind::Technique(kind) => (kind.name(), Color::Rgb(255, 202, 92)),
        PlayerActionKind::UseItem(kind) => (kind.name(), Color::Rgb(132, 226, 202)),
        PlayerActionKind::Guard => ("GUARD", Color::Rgb(120, 190, 255)),
        PlayerActionKind::Hit => ("HIT", HP_BAR_LOW),
        PlayerActionKind::Heal => ("mend", Color::Rgb(104, 246, 166)),
    };
    (format!("{name} · {phase}"), color)
}

fn priority_threat<'a>(ctx: &RenderCtx<'a>) -> Option<&'a Monster> {
    ctx.monsters
        .iter()
        .filter(|monster| {
            let distance = (monster.x - ctx.player.x).abs() + (monster.y - ctx.player.y).abs();
            monster.alive
                && ctx.map.in_bounds(monster.x, monster.y)
                && ctx.map.visible[ctx.map.idx(monster.x, monster.y)]
                && (distance <= 4 || monster.windup > 0)
        })
        .min_by_key(|monster| {
            let distance = (monster.x - ctx.player.x).abs() + (monster.y - ctx.player.y).abs();
            (monster.windup == 0, distance, monster.stats.hp)
        })
}

/// Build a sidebar control row: highlighted keys column + action description.
/// Keys are rendered in bold amber; action text in a brighter neutral.
fn control_row(keys: &'static str, action: &'static str) -> Line<'static> {
    let key_color = Color::Rgb(255, 200, 60); // warm amber
    let action_color = Color::Rgb(190, 190, 210);
    Line::from(vec![
        Span::styled(
            format!("{:12}", keys),
            Style::default().fg(key_color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(action, Style::default().fg(action_color)),
    ])
}

fn facing_name(angle: f64) -> &'static str {
    let quarter = (angle / std::f64::consts::FRAC_PI_2).round() as i32;
    match quarter.rem_euclid(4) {
        0 => "E",
        1 => "S",
        2 => "W",
        _ => "N",
    }
}

fn facing_delta(angle: f64) -> (i32, i32) {
    match facing_name(angle) {
        "E" => (1, 0),
        "S" => (0, 1),
        "W" => (-1, 0),
        _ => (0, -1),
    }
}

fn minimap_player_marker(angle: f64) -> char {
    match facing_name(angle) {
        "E" => '>',
        "S" => 'v',
        "W" => '<',
        _ => '^',
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// INVENTORY OVERLAY
// ══════════════════════════════════════════════════════════════════════════════

fn render_inventory_overlay(frame: &mut Frame, area: Rect, ctx: &RenderCtx) {
    let player = ctx.player;
    let cursor = ctx.inventory_cursor;
    let inv_len = player.inventory.len();

    // Calculate popup dimensions
    let popup_w = 66u16;
    let popup_h = if inv_len == 0 {
        6
    } else {
        (inv_len.min(26) as u16)
            .saturating_add(18)
            .min(area.height.saturating_sub(4))
    };

    let popup = centered_rect(area, popup_w, popup_h);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(140, 140, 160)))
        .style(Style::default().bg(PANEL_BG));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    if inv_len == 0 {
        let lines = vec![
            Line::from(Span::styled(
                "══ INVENTORY ══",
                Style::default().fg(Color::White),
            )),
            Line::from(""),
            Line::from(Span::styled("(empty)", Style::default().fg(TEXT_DIM))),
            Line::from(""),
            Line::from(Span::styled(
                "Tab / i / Esc — close",
                Style::default().fg(TEXT_DIM),
            )),
        ];
        let p = Paragraph::new(lines);
        frame.render_widget(p, inner);
        return;
    }

    // Two-column layout: left = item list, right = tooltip
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(30), Constraint::Fill(1)])
        .split(inner);

    // ── Left column: item list ──
    let cur = cursor.min(inv_len.saturating_sub(1));
    let max_slots = 26.min(inv_len);
    let mut list_lines = vec![
        Line::from(Span::styled(
            "══ INVENTORY ══",
            Style::default().fg(Color::White),
        )),
        Line::from(""),
    ];

    for i in 0..max_slots {
        let item = player.inventory[i];
        let key = (b'a' + i as u8) as char;
        let color = item_color(item);
        let is_selected = i == cur;
        let is_equipped = player.weapon == Some(item) || player.armor == Some(item);

        let is_quick = player.quick_item == Some(item);
        let category = inventory_category_badge(item);
        let prefix = if is_selected {
            "\u{25B8}"
        } else if is_equipped {
            "\u{2713}"
        } else if is_quick {
            "*"
        } else {
            " "
        };
        let sel_bg = Color::Rgb(45, 45, 62);
        let key_style = if is_selected {
            Style::default().fg(Color::White).bg(sel_bg)
        } else {
            Style::default().fg(Color::White)
        };
        let name_style = if is_selected {
            Style::default().fg(color).bg(sel_bg)
        } else if is_equipped {
            Style::default().fg(color).add_modifier(Modifier::DIM)
        } else {
            Style::default().fg(color)
        };

        // Truncate item name to fit after marker, shortcut, and category badge.
        let avail = 26usize.saturating_sub(prefix.chars().count() + category.len() + 3);
        let name = item.display_name(player);
        let display_name: String = if name.len() > avail {
            name.chars()
                .take(avail.saturating_sub(1))
                .chain(std::iter::once('\u{2026}'))
                .collect()
        } else {
            name.to_string()
        };

        list_lines.push(Line::from(vec![
            Span::styled(prefix.to_string(), name_style),
            Span::styled(format!("{} ", key), key_style),
            Span::styled(
                format!("{category} "),
                if is_selected {
                    Style::default().fg(TEXT_DIM).bg(sel_bg)
                } else {
                    Style::default().fg(TEXT_DIM)
                },
            ),
            Span::styled(display_name, name_style),
        ]));
    }

    let list_p = Paragraph::new(list_lines);
    frame.render_widget(list_p, columns[0]);

    // ── Right column: tooltip ──
    let selected = player.inventory[cur];
    let equipped = player.weapon == Some(selected) || player.armor == Some(selected);
    let cursed = (player.weapon == Some(selected) && player.weapon_cursed)
        || (player.armor == Some(selected) && player.armor_cursed);
    let mut badges = Vec::new();
    if !player.knows_item(selected) {
        badges.push("UNKNOWN".to_string());
    }
    if equipped {
        badges.push("EQUIPPED".to_string());
    }
    if cursed {
        badges.push("CURSED".to_string());
    }
    if player.weapon == Some(selected) && selected.max_durability() > 0 {
        badges.push(format!(
            "DUR {}/{}",
            player.weapon_durability,
            selected.max_durability()
        ));
    } else if selected.is_weapon() && selected.max_durability() > 0 {
        badges.push(format!("MAX DUR {}", selected.max_durability()));
    }
    let mut tip_lines = vec![
        Line::from(Span::styled(
            selected.display_name(player),
            Style::default()
                .fg(item_color(selected))
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            selected.category_label(),
            Style::default().fg(TEXT_DIM),
        )),
    ];
    if !badges.is_empty() {
        tip_lines.push(Line::from(Span::styled(
            badges.join(" · "),
            Style::default()
                .fg(if cursed {
                    Color::Rgb(236, 82, 92)
                } else {
                    Color::Rgb(232, 190, 74)
                })
                .add_modifier(Modifier::BOLD),
        )));
    }
    tip_lines.push(Line::from(""));
    tip_lines.push(Line::from(Span::styled(
        if player.knows_item(selected) {
            selected.description()
        } else {
            "Its exact effect is unknown. Use it or identify supplies at a camp."
        },
        Style::default().fg(Color::White),
    )));

    // Stat lines
    for stat in if player.knows_item(selected) {
        selected.stat_lines()
    } else {
        Vec::new()
    } {
        tip_lines.push(Line::from(Span::styled(
            format!("  \u{25B8} {}", stat),
            Style::default().fg(Color::Rgb(255, 204, 0)),
        )));
    }

    // Compare with equipped gear
    if selected.is_weapon() {
        tip_lines.push(Line::from(""));
        let current = player.weapon.map_or(0, |item| item.attack_bonus());
        let delta = selected.attack_bonus() - current;
        tip_lines.push(comparison_line("ATK", delta));
        if selected.max_durability() > 0 {
            tip_lines.push(Line::from(Span::styled(
                format!("Maximum durability: {}", selected.max_durability()),
                Style::default().fg(TEXT_DIM),
            )));
        } else {
            tip_lines.push(Line::from(Span::styled(
                "Named artifact · does not wear down",
                Style::default().fg(Color::Rgb(232, 190, 74)),
            )));
        }
        if let Some(eq) = &player.weapon {
            tip_lines.push(Line::from(vec![
                Span::styled("Equipped: ", Style::default().fg(TEXT_DIM)),
                Span::styled(
                    format!("{} (+{} ATK)", eq.name(), eq.attack_bonus()),
                    Style::default().fg(ITEM_WEAPON_COLOR),
                ),
            ]));
        } else {
            tip_lines.push(Line::from(Span::styled(
                "Equipped: (none)",
                Style::default().fg(TEXT_DIM),
            )));
        }
    }
    if selected.is_armor() {
        tip_lines.push(Line::from(""));
        let current = player.armor.map_or(0, |item| item.defense_bonus());
        tip_lines.push(comparison_line("DEF", selected.defense_bonus() - current));
        if let Some(eq) = &player.armor {
            tip_lines.push(Line::from(vec![
                Span::styled("Equipped: ", Style::default().fg(TEXT_DIM)),
                Span::styled(
                    format!("{} (+{} DEF)", eq.name(), eq.defense_bonus()),
                    Style::default().fg(ITEM_ARMOR_COLOR),
                ),
            ]));
        } else {
            tip_lines.push(Line::from(Span::styled(
                "Equipped: (none)",
                Style::default().fg(TEXT_DIM),
            )));
        }
    }
    if selected == ItemKind::CrownOfDepth {
        tip_lines.push(Line::from(""));
        tip_lines.push(Line::from(Span::styled(
            "The Crown can only be worn,",
            Style::default().fg(TEXT_DIM),
        )));
        tip_lines.push(Line::from(Span::styled(
            "not consumed or equipped.",
            Style::default().fg(TEXT_DIM),
        )));
    }

    tip_lines.push(Line::from(""));
    tip_lines.push(Line::from(Span::styled(
        "j/k — move    Enter — use/equip",
        Style::default().fg(TEXT_DIM),
    )));
    tip_lines.push(Line::from(Span::styled(
        "a-z — use    Q — assign F    Tab — close",
        Style::default().fg(TEXT_DIM),
    )));

    let tip_area = if columns[1].height >= 15 {
        let right = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(7), Constraint::Fill(1)])
            .split(columns[1]);
        crate::item_art::render_preview(
            frame,
            right[0],
            crate::item_art::appearance(selected, player.knows_item(selected)),
            ctx.tick,
            ctx.color_filter,
        );
        right[1]
    } else {
        columns[1]
    };
    let tip_p = Paragraph::new(tip_lines).wrap(Wrap { trim: false });
    frame.render_widget(tip_p, tip_area);
}

fn inventory_category_badge(item: ItemKind) -> &'static str {
    if item.is_weapon() {
        "WPN"
    } else if item.is_armor() {
        "ARM"
    } else {
        match item {
            ItemKind::CrownOfDepth | ItemKind::BellToken | ItemKind::IronKey => "KEY",
            ItemKind::LoreFragment => "LOR",
            _ => "SUP",
        }
    }
}

fn comparison_line(stat: &str, delta: i32) -> Line<'static> {
    let (label, color) = match delta.cmp(&0) {
        std::cmp::Ordering::Greater => ("upgrade", Color::Rgb(92, 214, 132)),
        std::cmp::Ordering::Less => ("tradeoff", Color::Rgb(232, 116, 82)),
        std::cmp::Ordering::Equal => ("same strength", TEXT_DIM),
    };
    Line::from(vec![
        Span::styled("Compared: ", Style::default().fg(TEXT_DIM)),
        Span::styled(
            format!("{delta:+} {stat} · {label}"),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
    ])
}

fn render_context_strip(frame: &mut Frame, area: Rect, ctx: &RenderCtx) {
    let player = ctx.player;
    let (dx, dy) = facing_delta(ctx.camera_angle);
    let front = (player.x + dx, player.y + dy);
    let current_tile = ctx.map.tiles[ctx.map.idx(player.x, player.y)];

    let (text, color) = if ctx.auto_exploring {
        (
            "Exploring · any key stops".to_string(),
            Color::Rgb(120, 210, 170),
        )
    } else if let Some(item) = ctx
        .items
        .iter()
        .find(|item| (item.x, item.y) == (player.x, player.y))
    {
        (
            format!(
                "At your feet: {}{}{}",
                item.kind.display_name(player),
                pickup_comparison(item.kind, player),
                unseen_hint(ctx, HINT_INTERACT, "  [Enter] take")
            ),
            item_color(item.kind),
        )
    } else if current_tile == Tile::StairsDown {
        (
            format!(
                "Stairs descend here{}",
                unseen_hint(ctx, HINT_INTERACT, "  [Enter] descend")
            ),
            ITEM_CROWN_COLOR,
        )
    } else if current_tile == Tile::StairsUp {
        (
            format!(
                "Stairs rise here{}",
                unseen_hint(ctx, HINT_INTERACT, "  [Enter] ascend")
            ),
            Color::Rgb(255, 170, 80),
        )
    } else if current_tile == Tile::LanternDead {
        (
            format!(
                "A dead lantern waits{}",
                unseen_hint(ctx, HINT_INTERACT, "  [Enter] relight")
            ),
            Color::Rgb(110, 150, 190),
        )
    } else if current_tile == Tile::TrapRevealed
        || (ctx.map.in_bounds(front.0, front.1)
            && ctx.map.tiles[ctx.map.idx(front.0, front.1)] == Tile::TrapRevealed)
    {
        (
            "Exposed trap within reach  [R] disarm".to_string(),
            Color::Rgb(236, 92, 64),
        )
    } else if let Some(monster) = ctx
        .monsters
        .iter()
        .find(|monster| monster.alive && (monster.x, monster.y) == front)
    {
        (
            format!(
                "{} · HP {}/{} · {} · {}{}",
                monster.kind.name(),
                monster.stats.hp,
                monster.stats.max_hp,
                monster_condition(monster),
                monster_intent(monster, player),
                unseen_hint(ctx, HINT_INTERACT, "  [Enter/W] attack")
            ),
            Color::Rgb(255, 96, 72),
        )
    } else if let Some(npc) = ctx.npcs.iter().find(|npc| (npc.x, npc.y) == front) {
        (
            format!(
                "{} stands ahead{}",
                npc.display_name(),
                unseen_hint(ctx, HINT_INTERACT, "  [Enter/T] talk")
            ),
            NPC_COLOR,
        )
    } else if ctx.map.in_bounds(front.0, front.1)
        && ctx.map.tiles[ctx.map.idx(front.0, front.1)] == Tile::LockedDoor
    {
        let solution = if player.inventory.contains(&ItemKind::IronKey) {
            "  [Enter/O] unlock with iron key"
        } else if player.weapon == Some(ItemKind::Warhammer) {
            "  [Enter/O] force with warhammer"
        } else {
            "  requires iron key or warhammer"
        };
        (
            format!("Locked iron door{solution}"),
            Color::Rgb(214, 152, 68),
        )
    } else if ctx.map.in_bounds(front.0, front.1)
        && ctx.map.tiles[ctx.map.idx(front.0, front.1)] == Tile::Urn
    {
        (
            "Sealed urn ahead  [W/Enter] smash".to_string(),
            Color::Rgb(202, 122, 72),
        )
    } else if ctx.map.in_bounds(front.0, front.1)
        && ctx.map.tiles[ctx.map.idx(front.0, front.1)] == Tile::Barricade
    {
        (
            "Barricade ahead  [W/Enter] break".to_string(),
            Color::Rgb(176, 112, 62),
        )
    } else if ctx.map.in_bounds(front.0, front.1)
        && ctx.map.tiles[ctx.map.idx(front.0, front.1)] == Tile::DoorClosed
    {
        (
            format!(
                "Closed door ahead{}",
                unseen_hint(ctx, HINT_INTERACT, "  [Enter/W] open")
            ),
            Color::Rgb(210, 146, 72),
        )
    } else if ctx.map.in_bounds(front.0, front.1)
        && ctx.map.tiles[ctx.map.idx(front.0, front.1)] == Tile::DoorOpen
    {
        (
            "Open door ahead  [O] close".to_string(),
            Color::Rgb(168, 118, 68),
        )
    } else if let Some(camp) = ctx.camps.iter().find(|camp| {
        ctx.map.rooms.get(camp.room_idx).is_some_and(|room| {
            let center = room.center();
            (center.0 - player.x).abs() + (center.1 - player.y).abs() <= 1
        })
    }) {
        if camp.rested {
            (format!("{}  already rested", camp.kind.name()), TEXT_DIM)
        } else {
            (
                format!(
                    "{}{}",
                    camp.kind.name(),
                    unseen_hint(ctx, HINT_INTERACT, "  [Enter] prepare")
                ),
                Color::Rgb(235, 158, 76),
            )
        }
    } else if let Some((room_index, landmark)) = ctx.landmarks.iter().find(|(room_index, _)| {
        ctx.map.rooms.get(*room_index).is_some_and(|room| {
            let center = room.center();
            (center.0 - player.x).abs() + (center.1 - player.y).abs() <= 1
        })
    }) {
        if ctx.used_landmarks.contains(room_index) {
            (format!("{}  exhausted", landmark.name()), TEXT_DIM)
        } else {
            (
                format!(
                    "{}{}",
                    landmark.name(),
                    unseen_hint(ctx, HINT_INTERACT, "  [Enter] inspect")
                ),
                SHRINE_COLOR,
            )
        }
    } else if let Some(direction) = adjacent_threat_direction(ctx) {
        (format!("Threat {}", direction), Color::Rgb(255, 92, 72))
    } else if ctx.floor_memory.attention >= 85 {
        (
            "The whole floor is listening · keep moving or find quiet".to_string(),
            Color::Rgb(232, 104, 80),
        )
    } else if ctx.floor_memory.attention >= 60 {
        (
            "Your trail is hot · rooms ahead are beginning to hunt".to_string(),
            Color::Rgb(224, 168, 84),
        )
    } else if let Some(sound) = distant_sound(ctx) {
        (sound, Color::Rgb(116, 178, 194))
    } else if player.crown_held {
        let route = explored_up_stair_bearing(ctx)
            .map(|bearing| format!(" · known stairs {bearing}"))
            .unwrap_or_else(|| " · find the stairs up".to_string());
        (
            format!(
                "Facing {} · Crown wake{} · {} ascent{} remain",
                facing_name(ctx.camera_angle),
                route,
                player.depth,
                if player.depth == 1 { "" } else { "s" }
            ),
            ITEM_CROWN_COLOR,
        )
    } else {
        (
            format!(
                "Facing {}{}{}{}",
                facing_name(ctx.camera_angle),
                unseen_hint(ctx, HINT_NAVIGATION, "   [A/D] turn"),
                unseen_hint(ctx, HINT_MAP, "   [m] toggle map"),
                unseen_hint(ctx, HINT_INVENTORY, "   [Tab] inventory")
            ),
            TEXT_DIM,
        )
    };

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" {}", text),
            Style::default().fg(color).bg(PANEL_BG),
        )))
        .style(Style::default().bg(PANEL_BG)),
        area,
    );
}

fn explored_up_stair_bearing(ctx: &RenderCtx<'_>) -> Option<&'static str> {
    let player = ctx.player;
    let (x, y) = (0..ctx.map.height)
        .flat_map(|y| (0..ctx.map.width).map(move |x| (x, y)))
        .find(|&(x, y)| {
            let index = ctx.map.idx(x, y);
            ctx.map.explored[index] && ctx.map.tiles[index] == Tile::StairsUp
        })?;
    let dx = x - player.x;
    let dy = y - player.y;
    if dx.abs() >= dy.abs() {
        Some(if dx >= 0 { "E" } else { "W" })
    } else {
        Some(if dy >= 0 { "S" } else { "N" })
    }
}

fn pickup_comparison(kind: ItemKind, player: &Player) -> String {
    if kind.is_weapon() {
        let current = player.weapon.map_or(0, |item| item.attack_bonus());
        let delta = kind.attack_bonus() - current;
        return format!(" ({:+} ATK)", delta);
    }
    if kind.is_armor() {
        let current = player.armor.map_or(0, |item| item.defense_bonus());
        let delta = kind.defense_bonus() - current;
        return format!(" ({:+} DEF)", delta);
    }
    match kind {
        ItemKind::HealthPotion | ItemKind::LargeHealthPotion
            if player.stats.hp >= player.stats.max_hp =>
        {
            " (no effect at full health)".to_string()
        }
        ItemKind::Antidote if !player.poisoned => " (not currently needed)".to_string(),
        _ => String::new(),
    }
}

fn distant_sound(ctx: &RenderCtx<'_>) -> Option<String> {
    if ctx.tick % 90 >= 36 {
        return None;
    }
    let (fx, fy) = facing_delta(ctx.camera_angle);
    ctx.monsters
        .iter()
        .filter(|monster| monster.alive && ctx.map.in_bounds(monster.x, monster.y))
        .filter(|monster| !ctx.map.visible[ctx.map.idx(monster.x, monster.y)])
        .filter_map(|monster| {
            let dx = monster.x - ctx.player.x;
            let dy = monster.y - ctx.player.y;
            let distance = dx.abs() + dy.abs();
            (3..=8)
                .contains(&distance)
                .then_some((distance, dx, dy, monster.kind))
        })
        .min_by_key(|(distance, _, _, _)| *distance)
        .map(|(_, dx, dy, kind)| {
            let forward = dx * fx + dy * fy;
            let side = dx * -fy + dy * fx;
            let direction = if forward.abs() >= side.abs() {
                if forward > 0 {
                    "ahead"
                } else {
                    "behind"
                }
            } else if side < 0 {
                "to the left"
            } else {
                "to the right"
            };
            let sound = match kind {
                MonsterKind::Spider => "skittering",
                MonsterKind::Skeleton => "bones scraping",
                MonsterKind::Bat => "wings",
                MonsterKind::Wraith => "a cold whisper",
                MonsterKind::Orc
                | MonsterKind::Guardian
                | MonsterKind::Sentinel
                | MonsterKind::Stalker => "heavy steps",
                MonsterKind::BellWarden => "a low bell note",
                _ => "movement",
            };
            format!("You hear {sound} {direction}")
        })
}

fn unseen_hint(ctx: &RenderCtx<'_>, bit: u8, text: &'static str) -> &'static str {
    if ctx.hints_seen & bit == 0 {
        text
    } else {
        ""
    }
}

fn monster_condition(monster: &Monster) -> &'static str {
    if monster.staggered > 0 {
        return "poise broken";
    }
    if monster.kind == MonsterKind::Guardian && monster.stats.hp * 2 <= monster.stats.max_hp {
        return "CROWN-FIRE";
    }
    let health = monster.stats.hp as f32 / monster.stats.max_hp.max(1) as f32;
    if health > 0.7 {
        "healthy"
    } else if health > 0.3 {
        "wounded"
    } else {
        "near death"
    }
}

fn monster_intent(monster: &Monster, player: &Player) -> &'static str {
    let health = monster.stats.hp as f32 / monster.stats.max_hp.max(1) as f32;
    let distance = (monster.x - player.x).abs() + (monster.y - player.y).abs();
    if let Some(intent) = monster.intent {
        match intent.kind {
            crate::entity::IntentKind::Strike if monster.windup > 0 => "HEAVY ATTACK INCOMING",
            crate::entity::IntentKind::Ring => "RESONANT BLOW INCOMING",
            crate::entity::IntentKind::Cast => "CASTING",
            crate::entity::IntentKind::Guard => "guarding",
            crate::entity::IntentKind::Hunt => "hunting",
            crate::entity::IntentKind::Retreat => "retreating",
            crate::entity::IntentKind::Strike => intent.kind.label(),
        }
    } else if monster.windup > 0 {
        "HEAVY ATTACK INCOMING"
    } else if monster.kind == MonsterKind::Guardian && monster.stats.hp * 2 <= monster.stats.max_hp
    {
        "PHASE II"
    } else if health < 0.3 && matches!(monster.kind, MonsterKind::Rat | MonsterKind::Goblin) {
        "retreating"
    } else if distance <= 1 {
        "striking"
    } else if monster.kind == MonsterKind::Necromancer {
        "casting"
    } else if monster.kind == MonsterKind::Sentinel {
        "guarding"
    } else {
        "advancing"
    }
}

fn adjacent_threat_direction(ctx: &RenderCtx) -> Option<&'static str> {
    let player = ctx.player;
    let (fx, fy) = facing_delta(ctx.camera_angle);
    let right = (-fy, fx);
    ctx.monsters
        .iter()
        .filter(|monster| {
            monster.alive
                && (monster.x - player.x).abs() + (monster.y - player.y).abs() == 1
                && ctx.map.visible[ctx.map.idx(monster.x, monster.y)]
        })
        .map(|monster| {
            let delta = (monster.x - player.x, monster.y - player.y);
            let forward = delta.0 * fx + delta.1 * fy;
            let side = delta.0 * right.0 + delta.1 * right.1;
            if forward > 0 {
                "ahead"
            } else if side < 0 {
                "to your left"
            } else if side > 0 {
                "to your right"
            } else {
                "behind you"
            }
        })
        .next()
}

fn render_help_overlay(frame: &mut Frame, area: Rect) {
    let popup = centered_rect(area, 68, 18.min(area.height.saturating_sub(2)));
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" CONTROLS · ? / Esc closes ")
        .border_style(Style::default().fg(Color::Rgb(210, 170, 90)))
        .style(Style::default().bg(PANEL_BG));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let left = vec![
        Line::from(Span::styled(
            "Movement · ?/Esc closes",
            Style::default().fg(Color::Rgb(210, 170, 90)),
        )),
        control_row("W / Up / K", "forward / bump"),
        control_row("S / Down / J", "move backward"),
        control_row("A / D", "turn left / right"),
        control_row("Z / C", "strafe left / right"),
        control_row("Shift+Z/C", "evasive step (3 STA)"),
        control_row("Shift+T", "use equipped technique"),
        control_row("Enter", "context action"),
        control_row("O / v", "door / listen"),
        control_row("R", "disarm revealed trap"),
        control_row("G", "pick up"),
        control_row("T", "talk"),
        control_row("> / <", "use stairs"),
    ];
    let right = vec![
        control_row("X", "auto-explore"),
        control_row("E", "toggle auto-pickup"),
        control_row("Tab / I", "inventory"),
        control_row("Q / L", "quest log"),
        control_row(". / Space", "wait"),
        Line::from(""),
        control_row("m / M", "toggle map overlay"),
        control_row("B", "discovered bestiary"),
        control_row("V", "expanded chronicle"),
        control_row("F2 / F3", "flash / contrast"),
        control_row("F4", "cycle turn easing"),
        control_row("F6 / F7", "head / palette"),
        control_row("F8", "low-motion mode"),
        control_row("F5 / F9", "save / load"),
        control_row("PageUp/Down", "scroll log"),
    ];
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);
    frame.render_widget(Paragraph::new(left), columns[0]);
    frame.render_widget(Paragraph::new(right), columns[1]);
}

fn render_discipline_overlay(frame: &mut Frame, area: Rect, ctx: &RenderCtx<'_>) {
    let popup = centered_rect(area, 64, 16.min(area.height.saturating_sub(2)));
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" DISCIPLINE - choose before acting ")
        .border_style(
            Style::default()
                .fg(Color::Rgb(225, 174, 82))
                .add_modifier(Modifier::BOLD),
        )
        .style(Style::default().bg(Color::Rgb(13, 12, 15)));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let p = ctx.player;
    let lines = vec![
        Line::from(Span::styled(
            format!(
                "Level {}   Choices remaining {}   V{} / F{} / R{}",
                p.level, p.pending_level_ups, p.vigor_rank, p.finesse_rank, p.resolve_rank
            ),
            Style::default().fg(TEXT_DIM),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  1  VIGOR    ",
                Style::default().fg(Color::Rgb(222, 92, 76)),
            ),
            Span::styled("+6 max HP, heal 6", Style::default().fg(TEXT_BRIGHT)),
        ]),
        Line::from(Span::styled(
            "     Survive one more committed exchange.",
            Style::default().fg(TEXT_DIM),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  2  FINESSE  ",
                Style::default().fg(Color::Rgb(100, 205, 154)),
            ),
            Span::styled(
                "+1 ATK, +1 max STA (cap 12)",
                Style::default().fg(TEXT_BRIGHT),
            ),
        ]),
        Line::from(Span::styled(
            "     More pressure, more room to evade.",
            Style::default().fg(TEXT_DIM),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  3  RESOLVE  ",
                Style::default().fg(Color::Rgb(108, 166, 230)),
            ),
            Span::styled(
                "+1 DEF, +1 counter damage",
                Style::default().fg(TEXT_BRIGHT),
            ),
        ]),
        Line::from(Span::styled(
            "     Hold the line, then answer harder.",
            Style::default().fg(TEXT_DIM),
        )),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_bestiary_overlay(frame: &mut Frame, area: Rect, ctx: &RenderCtx) {
    let popup = centered_rect(area, 62, 22.min(area.height.saturating_sub(2)));
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" BESTIARY · B / Esc closes ")
        .border_style(Style::default().fg(Color::Rgb(155, 190, 175)))
        .style(Style::default().bg(PANEL_BG));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    const KINDS: [MonsterKind; 14] = [
        MonsterKind::Rat,
        MonsterKind::Bat,
        MonsterKind::Goblin,
        MonsterKind::Skeleton,
        MonsterKind::Orc,
        MonsterKind::Spider,
        MonsterKind::Wraith,
        MonsterKind::Slime,
        MonsterKind::Mimic,
        MonsterKind::Necromancer,
        MonsterKind::Sentinel,
        MonsterKind::Guardian,
        MonsterKind::Stalker,
        MonsterKind::BellWarden,
    ];
    let mut lines = vec![
        Line::from(Span::styled(
            "Knowledge comes from surviving an encounter.",
            Style::default().fg(TEXT_DIM),
        )),
        Line::from(""),
    ];
    for kind in KINDS {
        if ctx.defeated_monsters.contains(&kind) {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {:<14}", kind.name()),
                    Style::default()
                        .fg(monster_color(kind))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    monster_behavior_hint(kind),
                    Style::default().fg(TEXT_BRIGHT),
                ),
            ]));
        } else if ctx.seen_monsters.contains(&kind) {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {:<14}", kind.name()),
                    Style::default().fg(monster_color(kind)),
                ),
                Span::styled(
                    "Observed. Defeat one to complete the entry.",
                    Style::default().fg(TEXT_DIM),
                ),
            ]));
        } else {
            lines.push(Line::from(Span::styled(
                "  ??????????????  Not yet encountered",
                Style::default().fg(TEXT_DIM),
            )));
        }
    }
    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_preparation_overlay(frame: &mut Frame, area: Rect, ctx: &RenderCtx) {
    let Some(menu) = ctx.preparation_menu else {
        return;
    };
    let popup = centered_rect(area, 58, 16.min(area.height.saturating_sub(2)));
    frame.render_widget(Clear, popup);
    let (title, intro, choices) = match menu {
        PreparationMenu::Camp(index) => {
            let name = ctx
                .camps
                .get(index)
                .map_or("Dungeon Camp", |camp| camp.kind.name());
            (
                format!(" {} · choose one preparation ", name),
                format!(
                    "HP {}/{} · preparations consume this camp",
                    ctx.player.stats.hp, ctx.player.stats.max_hp
                ),
                vec![
                    "1  Rest       recover one-third HP · 3 turns".to_string(),
                    "2  Hone       +1 ATK for 40 turns · 1 turn".to_string(),
                    "3  Ward       cleanse poison, +1 DEF/protection · 1 turn".to_string(),
                    "4  Repair     restore equipped weapon durability · 1 turn".to_string(),
                    "5  Identify   reveal all carried supplies · 1 turn".to_string(),
                ],
            )
        }
        PreparationMenu::Shrine(_) => (
            " Ancient Shrine · choose one offering ".to_string(),
            format!("You carry {} gold", ctx.player.gold),
            vec![
                "1  Restoration   full healing · 10 gold".to_string(),
                "2  Cleansing     remove poison · 8 gold".to_string(),
                "3  Blessing      +1 ATK/DEF for 60 turns · 15 gold".to_string(),
            ],
        ),
        PreparationMenu::Mastery(weapon) => {
            let options = crate::entity::WeaponMastery::options_for(Some(weapon));
            (
                format!(" {} · choose a weapon path ", weapon.name()),
                "The forge cannot teach both lessons in one run.".to_string(),
                options
                    .iter()
                    .enumerate()
                    .map(|(index, mastery)| {
                        format!(
                            "{}  {:<12} {}",
                            index + 1,
                            mastery.name(),
                            mastery.description()
                        )
                    })
                    .collect(),
            )
        }
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(Color::Rgb(220, 178, 92)))
        .style(Style::default().bg(PANEL_BG));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let mut lines = vec![
        Line::from(Span::styled(intro, Style::default().fg(TEXT_DIM))),
        Line::from(""),
    ];
    for choice in choices {
        lines.push(Line::from(Span::styled(
            choice,
            Style::default().fg(TEXT_BRIGHT),
        )));
        lines.push(Line::from(""));
    }
    lines.push(Line::from(Span::styled(
        "Esc  leave without spending a turn",
        Style::default().fg(TEXT_DIM),
    )));
    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_log_overlay(frame: &mut Frame, area: Rect, ctx: &RenderCtx) {
    let popup = centered_rect(
        area,
        area.width.saturating_sub(8).min(86),
        area.height.saturating_sub(6).min(32),
    );
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" CHRONICLE · V / Esc closes ")
        .border_style(Style::default().fg(Color::Rgb(150, 174, 184)))
        .style(Style::default().bg(PANEL_BG));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let lines = ctx
        .messages
        .recent(inner.height as usize)
        .into_iter()
        .map(|message| {
            let suffix = if message.repeat > 1 {
                format!(" x{}", message.repeat)
            } else {
                String::new()
            };
            Line::from(Span::styled(
                format!("{}{}", message.text, suffix),
                Style::default().fg(msg_color(message.color)),
            ))
        })
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn monster_behavior_hint(kind: MonsterKind) -> &'static str {
    match kind {
        MonsterKind::Rat => "Skittish; retreats when badly hurt.",
        MonsterKind::Bat => "Fast and difficult to pin down.",
        MonsterKind::Goblin => "Cowardly once bloodied.",
        MonsterKind::Skeleton => "Relentless, but unarmored.",
        MonsterKind::Orc => "Slow pressure and heavy blows.",
        MonsterKind::Spider => "Its bite can poison.",
        MonsterKind::Wraith => "Passes through solid stone.",
        MonsterKind::Slime => "Patient and hard to read.",
        MonsterKind::Mimic => "Waits where treasure should be.",
        MonsterKind::Necromancer => "Keeps distance and casts bolts.",
        MonsterKind::Sentinel => "Guards its ground.",
        MonsterKind::Guardian => "The deepest chamber's keeper.",
        MonsterKind::Stalker => "Follows disturbed floors and punishes retreat.",
        MonsterKind::BellWarden => "Telegraphs a resonant blow that must be broken.",
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// MESSAGE LOG
// ══════════════════════════════════════════════════════════════════════════════

fn render_messages(frame: &mut Frame, area: Rect, ctx: &RenderCtx) {
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(TEXT_DIM))
        .style(Style::default().bg(PANEL_BG));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let log_height = inner.height as usize;
    let offset = ctx.log_scroll;
    let recent = ctx.messages.window(log_height, offset);
    let total = ctx.messages.len();
    let lines: Vec<Line> = recent
        .iter()
        .map(|m| {
            let color = msg_color(m.color);
            let text = if m.repeat > 1 {
                format!("{} x{}", m.text, m.repeat)
            } else {
                m.text.clone()
            };
            Line::from(Span::styled(text, Style::default().fg(color)))
        })
        .collect();

    // Show scroll indicator if scrolled up
    let mut display_lines = lines;
    if offset > 0 && total > log_height {
        display_lines.push(Line::from(Span::styled(
            format!(
                "──── ↑ Log scroll {}/{} ↑ ────",
                offset,
                total.saturating_sub(1)
            ),
            Style::default().fg(TEXT_DIM),
        )));
    }

    let p = Paragraph::new(display_lines).wrap(Wrap { trim: false });
    frame.render_widget(p, inner);
}

fn msg_color(mc: MsgColor) -> Color {
    match mc {
        MsgColor::White => Color::White,
        MsgColor::Red => Color::Rgb(255, 68, 68),
        MsgColor::Green => Color::Rgb(0, 255, 136),
        MsgColor::Yellow => Color::Rgb(255, 204, 0),
        MsgColor::Gray => TEXT_DIM,
        MsgColor::Cyan => Color::Rgb(0, 200, 255),
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// MENU
// ══════════════════════════════════════════════════════════════════════════════

const MENU_TITLE: [&str; 6] = [
    "\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2597} \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2597}\u{2588}\u{2588}\u{2597}    \u{2588}\u{2588}\u{2597}   \u{2588}\u{2588}\u{2597}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2597}",
    "\u{2588}\u{2588}\u{2594}\u{2594}\u{2588}\u{2588}\u{2597}\u{2588}\u{2588}\u{2594}\u{2594}\u{2594}\u{2594}\u{2594}\u{2594}\u{2597}\u{2588}\u{2588}\u{2591}    \u{2588}\u{2588}\u{2591}   \u{2588}\u{2588}\u{2591}\u{2588}\u{2588}\u{2594}\u{2594}\u{2594}\u{2594}\u{2594}\u{2594}\u{2597}",
    "\u{2588}\u{2588}\u{2591}  \u{2588}\u{2588}\u{2591}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2597}  \u{2588}\u{2588}\u{2591}    \u{2588}\u{2588}\u{2591}   \u{2588}\u{2588}\u{2591}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2597}",
    "\u{2588}\u{2588}\u{2591}  \u{2588}\u{2588}\u{2591}\u{2588}\u{2588}\u{2594}\u{2594}\u{2594}\u{2597}  \u{2588}\u{2588}\u{2591}    \u{259a}\u{2588}\u{2588}\u{2597} \u{2588}\u{2588}\u{2594}\u{2597}\u{2588}\u{2588}\u{2594}\u{2594}\u{2594}\u{2597}",
    "\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2594}\u{2597}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2597}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2597}\u{259a}\u{2588}\u{2588}\u{2588}\u{2588}\u{2594}\u{2597} \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2597}",
    "\u{259a}\u{2594}\u{2594}\u{2594}\u{2594}\u{2594}\u{2594}\u{2597} \u{259a}\u{2594}\u{2594}\u{2594}\u{2594}\u{2594}\u{2594}\u{2594}\u{2597}\u{259a}\u{2594}\u{2594}\u{2594}\u{2594}\u{2594}\u{2594}\u{2597} \u{259a}\u{2594}\u{2594}\u{2594}\u{2594}\u{2597}  \u{259a}\u{2594}\u{2594}\u{2594}\u{2594}\u{2594}\u{2594}\u{2594}\u{2597}",
];

fn render_menu_v2(frame: &mut Frame, ctx: &RenderCtx) {
    let area = frame.area();
    let tick = ctx.visual.tick;

    if area.width >= 48 && area.height >= 20 {
        let mut scene = HalfBlockCanvas::new(area.width as usize, area.height as usize);
        crate::image_render::draw_menu_scene(
            &mut scene,
            tick,
            ctx.color_filter,
            ctx.low_motion,
            ctx.reduced_flash,
        );
        scene.render(frame.buffer_mut(), area);
    } else {
        let wave = (tick as f32 / 60.0 * std::f32::consts::TAU).sin();
        let pulse = ((wave * 0.5 + 0.5) * 6.0) as u8;
        frame.buffer_mut().set_style(
            area,
            Style::default().bg(Color::Rgb(14 + pulse, 14 + pulse, 22 + pulse)),
        );
    }

    let title_x = area.x + area.width.saturating_sub(46) / 2;
    let title_y = area.y + (area.height / 8).max(1);
    if area.width >= 46 {
        for (row, glyphs) in MENU_TITLE.iter().enumerate() {
            frame.render_widget(
                Paragraph::new(Span::styled(
                    *glyphs,
                    Style::default().fg(Color::Rgb(8, 6, 4)),
                )),
                Rect::new(title_x + 1, title_y + row as u16 + 1, 46, 1),
            );
        }
        for (row, glyphs) in MENU_TITLE.iter().enumerate() {
            let t = row as f32 / (MENU_TITLE.len() - 1) as f32;
            let gold = Color::Rgb(
                (255.0 - 60.0 * t) as u8,
                (216.0 - 90.0 * t) as u8,
                (90.0 - 60.0 * t) as u8,
            );
            frame.render_widget(
                Paragraph::new(Span::styled(*glyphs, Style::default().fg(gold))),
                Rect::new(title_x, title_y + row as u16, 46, 1),
            );
        }
    } else {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "DELVE",
                Style::default()
                    .fg(Color::Rgb(242, 190, 68))
                    .add_modifier(Modifier::BOLD),
            ))
            .centered(),
            Rect::new(area.x, title_y, area.width, 1),
        );
    }

    let key = |value: &'static str| {
        Span::styled(
            format!(" {value} "),
            Style::default()
                .fg(Color::Rgb(255, 226, 150))
                .bg(Color::Rgb(30, 24, 16))
                .add_modifier(Modifier::BOLD),
        )
    };
    let mut body = vec![
        Line::from(Span::styled(
            "A terminal roguelike",
            Style::default().fg(Color::Rgb(200, 196, 188)),
        )),
        Line::from(Span::styled(
            "Classic: five floors · Campaign: Chapter I.",
            Style::default().fg(TEXT_DIM),
        )),
        Line::from(""),
        Line::from(vec![
            key("c"),
            Span::styled("  classic mode", Style::default().fg(TEXT_BRIGHT)),
        ]),
        Line::from(vec![
            key("m"),
            Span::styled(
                "  campaign mode",
                Style::default().fg(Color::Rgb(140, 210, 255)),
            ),
        ]),
        Line::from(vec![
            key("l"),
            Span::styled("  load save", Style::default().fg(TEXT_DIM)),
        ]),
        Line::from(vec![
            key("q"),
            Span::styled("  quit", Style::default().fg(TEXT_DIM)),
        ]),
        Line::from(""),
        Line::from(vec![
            key("x"),
            Span::styled(
                format!("  Challenge: ‹ {} ›", ctx.challenge_mode.name()),
                Style::default().fg(Color::Rgb(255, 190, 110)),
            ),
        ]),
        Line::from(Span::styled(
            format!("      {}", ctx.challenge_mode.description()),
            Style::default().fg(TEXT_DIM),
        )),
    ];

    let body_height = body.len() as u16;
    let body_y = (title_y + 8).min(area.bottom().saturating_sub(body_height + 6));
    frame.render_widget(
        Paragraph::new(std::mem::take(&mut body)).centered(),
        Rect::new(area.x, body_y, area.width, body_height),
    );

    let mut footer = vec![Line::from(Span::styled(
        format!(
            "Seed {}  ·  {} runs recorded  ·  ? controls",
            ctx.seed, ctx.completed_runs
        ),
        Style::default().fg(TEXT_DIM),
    ))];
    if !ctx.run_history.is_empty() && area.height >= 38 {
        footer.push(Line::from(Span::styled(
            "Recent descents",
            Style::default().fg(Color::Rgb(190, 178, 158)),
        )));
        for run in ctx.run_history.iter().rev().take(2) {
            let outcome = if run.game_mode == GameMode::Campaign && run.won {
                "chapter complete"
            } else if run.won {
                "won"
            } else {
                "fell"
            };
            footer.push(Line::from(Span::styled(
                format!(
                    "{}  depth {}  {} turns",
                    outcome,
                    run.stats.deepest_depth + 1,
                    run.stats.turns
                ),
                Style::default().fg(TEXT_DIM),
            )));
        }
    }
    footer.push(Line::from(Span::styled(
        concat!("v", env!("CARGO_PKG_VERSION")),
        Style::default().fg(Color::Rgb(70, 68, 78)),
    )));
    let footer_height = footer.len() as u16;
    frame.render_widget(
        Paragraph::new(footer).centered(),
        Rect::new(
            area.x,
            area.bottom().saturating_sub(footer_height + 1),
            area.width,
            footer_height,
        ),
    );
}

/// Campaign intro screen — shows chapter lore before starting.
fn render_campaign_menu(frame: &mut Frame, ctx: &RenderCtx) {
    let area = frame.area();
    let tick = ctx.visual.tick;

    // Dark pulsing background
    let wave = (tick as f32 / 90.0_f32 * std::f32::consts::TAU).sin();
    let bg_pulse = ((wave * 0.5 + 0.5) * 4.0).min(4.0) as u8;
    let bg = Color::Rgb(10 + bg_pulse, 8 + bg_pulse, 18 + bg_pulse);
    frame.buffer_mut().set_style(area, Style::default().bg(bg));

    let intro_text = campaign::CHAPTER1_INTRO;

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!(
            "\u{2500}\u{2550}\u{2550} {} \u{2550}\u{2550}\u{2500}",
            campaign::CampaignChapter::Chapter1.name()
        ),
        Style::default()
            .fg(Color::Rgb(255, 215, 0))
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    for &line in intro_text {
        if line.is_empty() {
            lines.push(Line::from(""));
        } else {
            lines.push(Line::from(Span::styled(
                format!("    {}", line),
                Style::default().fg(Color::Rgb(200, 200, 220)),
            )));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "      \u{25B6} ENTER  begin the descent",
        Style::default().fg(PLAYER_COLOR),
    )));
    lines.push(Line::from(Span::styled(
        "       q/Esc  back to menu  ·  ? controls",
        Style::default().fg(TEXT_DIM),
    )));

    let p = Paragraph::new(lines).wrap(Wrap { trim: false });
    // The intro is intentionally sized to leave a little breathing room on the
    // supported 80×24 terminal instead of pinning the copy to the top edge.
    let width = 60.min(area.width.saturating_sub(4));
    let height = 22.min(area.height.saturating_sub(2));
    let centered = centered_rect(area, width, height);
    frame.render_widget(p, centered);
}

// ══════════════════════════════════════════════════════════════════════════════
// END SCREEN
// ══════════════════════════════════════════════════════════════════════════════

fn render_campaign_complete_screen(frame: &mut Frame, ctx: &RenderCtx) {
    let area = frame.area();
    let tick = ctx.visual.tick;
    let pulse = ((tick as f32 / 90.0 * std::f32::consts::TAU).sin() * 0.5 + 0.5) * 4.0;
    frame.buffer_mut().set_style(
        area,
        Style::default().bg(Color::Rgb(18, 16 + pulse as u8, 12 + pulse as u8)),
    );

    let ending = if ctx.campaign_flags.contains(&CampaignFlag::BellChoiceRung) {
        "Rung — the bell answered once, then fell quiet."
    } else if ctx
        .campaign_flags
        .contains(&CampaignFlag::BellChoiceShattered)
    {
        "Shattered — the bell has no voice left to call with."
    } else if ctx
        .campaign_flags
        .contains(&CampaignFlag::BellChoiceMuffled)
    {
        "Muffled — the bell sleeps beneath the stone."
    } else {
        "Resolved — the chamber is silent."
    };

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "── CHAPTER I COMPLETE ──",
            Style::default()
                .fg(Color::Rgb(255, 215, 0))
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            campaign::CampaignChapter::Chapter1.name(),
            Style::default().fg(Color::Rgb(220, 220, 230)),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("Bell: {ending}"),
            Style::default().fg(Color::Rgb(255, 220, 150)),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "The road below waits. It knows your name now.",
            Style::default().fg(TEXT_BRIGHT),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!(
                "Levels: 3 / 3    Turns: {}    Kills: {}",
                ctx.stats.turns, ctx.stats.kills
            ),
            Style::default().fg(TEXT_DIM),
        )),
        Line::from(Span::styled(
            format!(
                "Seed: {}    Challenge: {}",
                ctx.seed,
                ctx.challenge_mode.name()
            ),
            Style::default().fg(TEXT_DIM),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "r  restart Chapter I    m / Esc  main menu    q  quit",
            Style::default().fg(Color::Rgb(190, 190, 205)),
        )),
    ];

    let width = area.width.saturating_sub(6).min(68);
    let height = area.height.saturating_sub(2).min(lines.len() as u16 + 2);
    let popup = centered_rect(area, width, height);
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(132, 112, 62)))
        .style(Style::default().bg(Color::Rgb(18, 16, 12)));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

fn render_end_screen(frame: &mut Frame, ctx: &RenderCtx, title: &str, title_color: Color) {
    let area = frame.area();
    let tick = ctx.visual.tick;

    // Subtle background pulse — clamp to avoid u8 overflow from negative sin
    let wave = (tick as f32 / 90.0 * std::f32::consts::TAU).sin();
    let bg_pulse = ((wave * 0.5 + 0.5) * 4.0).min(4.0) as u8; // 0..4 range
    let bg = if ctx.status == GameStatus::Won {
        Color::Rgb(20, 18 + bg_pulse, 14 + bg_pulse) // warm gold
    } else {
        Color::Rgb(20 + bg_pulse, 14, 14) // dark red
    };
    frame.buffer_mut().set_style(area, Style::default().bg(bg));

    // Render half-block procedural art if space permits
    let art_height = 12u16;
    let use_half_block = area.height > 35 && area.width > 50;
    let text_y_offset: u16 = if use_half_block { art_height } else { 0 };

    if use_half_block {
        let art_area = Rect {
            x: area.x + (area.width.saturating_sub(30)) / 2,
            y: area.y + 1,
            width: 30.min(area.width),
            height: art_height.min(area.height),
        };
        let mut art = HalfBlockCanvas::new(art_area.width as usize, art_area.height as usize);
        if ctx.status == GameStatus::Won {
            crate::image_render::draw_victory_art(&mut art, tick);
        } else {
            crate::image_render::draw_death_art(&mut art, tick);
        }
        art.render(frame.buffer_mut(), art_area);
    }

    let stat_lines: Vec<(&str, i32)> = if ctx.status == GameStatus::Won {
        vec![
            ("Max depth:", ctx.stats.deepest_depth as i32 + 1),
            ("Turns:", ctx.stats.turns as i32),
            ("Kills:", ctx.stats.kills as i32),
            ("Items used:", ctx.stats.items_used as i32),
            ("Damage guarded:", ctx.stats.damage_guarded),
            ("Locks forced:", ctx.stats.doors_forced as i32),
            ("Traps disarmed:", ctx.stats.traps_disarmed as i32),
            ("Objects smashed:", ctx.stats.objects_smashed as i32),
        ]
    } else {
        vec![
            ("Depth:", ctx.stats.deepest_depth as i32 + 1),
            ("Turns:", ctx.stats.turns as i32),
            ("Kills:", ctx.stats.kills as i32),
            ("Dmg dealt:", ctx.stats.dmg_dealt),
            ("Dmg taken:", ctx.stats.dmg_taken),
            ("Items used:", ctx.stats.items_used as i32),
            ("Damage guarded:", ctx.stats.damage_guarded),
            ("Locks forced:", ctx.stats.doors_forced as i32),
            ("Traps disarmed:", ctx.stats.traps_disarmed as i32),
            ("Objects smashed:", ctx.stats.objects_smashed as i32),
        ]
    };

    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            title,
            Style::default()
                .fg(title_color)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
            Style::default().fg(TEXT_DIM),
        )),
        Line::from(""),
    ];
    for &(label, value) in &stat_lines {
        lines.push(Line::from(vec![
            Span::styled(format!("  {:<14}", label), Style::default().fg(TEXT_DIM)),
            Span::styled(format!("{}", value), Style::default().fg(TEXT_BRIGHT)),
        ]));
    }
    let show_tactical_summary = area.height >= 30;
    if show_tactical_summary {
        lines.push(Line::from(vec![
            Span::styled("  Offense:       ", Style::default().fg(TEXT_DIM)),
            Span::styled(
                format!(
                    "{} attacks / {} techniques",
                    ctx.stats.basic_attacks, ctx.stats.techniques_used
                ),
                Style::default().fg(TEXT_BRIGHT),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  Reactions:     ", Style::default().fg(TEXT_DIM)),
            Span::styled(
                format!(
                    "{} dodge / {} counter / {} interrupt",
                    ctx.stats.dodges, ctx.stats.counters, ctx.stats.interrupts
                ),
                Style::default().fg(TEXT_BRIGHT),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  Endurance:     ", Style::default().fg(TEXT_DIM)),
            Span::styled(
                format!(
                    "{} spent / {} breaks",
                    ctx.stats.stamina_spent, ctx.stats.guard_breaks
                ),
                Style::default().fg(TEXT_BRIGHT),
            ),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("  Seed:      {}", ctx.seed),
        Style::default().fg(TEXT_DIM),
    )));
    lines.push(Line::from(Span::styled(
        format!("  Challenge: {}", ctx.challenge_mode.name()),
        Style::default().fg(Color::Rgb(255, 190, 110)),
    )));
    lines.push(Line::from(Span::styled(
        format!(
            "  Build:     V{} / F{} / R{}",
            ctx.player.vigor_rank, ctx.player.finesse_rank, ctx.player.resolve_rank
        ),
        Style::default().fg(Color::Rgb(220, 184, 120)),
    )));

    // ── Show recent narrative lines (win/death prose) ──
    let recent: Vec<&str> = ctx
        .messages
        .recent(12)
        .iter()
        .rev()
        .filter(|m| !m.text.is_empty())
        .take(6)
        .map(|m| m.text.as_str())
        .collect();
    if !recent.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
            Style::default().fg(TEXT_DIM),
        )));
        for text in recent.iter() {
            // Truncate long lines to fit
            let display: String = if text.len() > 44 {
                format!("{}...", &text[..41])
            } else {
                text.to_string()
            };
            lines.push(Line::from(Span::styled(
                format!("  {}", display),
                Style::default().fg(Color::Rgb(200, 190, 160)),
            )));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
        Style::default().fg(TEXT_DIM),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  r \u{2014} restart    q \u{2014} quit",
        Style::default().fg(TEXT_DIM),
    )));

    let extra_lines = if recent.is_empty() {
        0
    } else {
        recent.len() as u16 + 2
    };
    let p = Paragraph::new(lines);
    let h = (stat_lines.len() as u16) + 14 + u16::from(show_tactical_summary) * 3 + extra_lines;
    let text_rect = Rect {
        x: area.x + (area.width.saturating_sub(46)) / 2,
        y: area.y
            + text_y_offset
            + ((area.height.saturating_sub(text_y_offset)).saturating_sub(h)) / 2,
        width: 46.min(area.width),
        height: h.min(area.height.saturating_sub(text_y_offset)),
    };
    frame.render_widget(p, text_rect);
}

// ══════════════════════════════════════════════════════════════════════════════
// DIALOG OVERLAY
// ══════════════════════════════════════════════════════════════════════════════

fn render_dialog_overlay(frame: &mut Frame, area: Rect, ctx: &RenderCtx) {
    let popup_w = area.width.saturating_sub(4).clamp(1, 76);
    let portrait_w = if popup_w >= 48 { 15 } else { 0 };
    let text_width = popup_w.saturating_sub(portrait_w + 6).max(12);
    let text_lines = ctx
        .dialog_text
        .iter()
        .map(|line| wrapped_dialog_line_count(line, text_width))
        .sum::<u16>();
    let choice_lines = if ctx.dialog_show_choices && !ctx.dialog_choices.is_empty() {
        let choices = ctx
            .dialog_choices
            .iter()
            .enumerate()
            .map(|(index, choice)| {
                wrapped_dialog_line_count(&format!("  {}. {}", index + 1, choice), text_width)
            })
            .sum::<u16>();
        choices + 4
    } else {
        2
    };
    let content_lines = text_lines + choice_lines + 2;
    let popup_h = (content_lines + 2)
        .max(13)
        .min(area.height.saturating_sub(2).max(1));

    let popup = centered_rect(area, popup_w, popup_h);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(Span::styled(
            " conversation ",
            Style::default()
                .fg(Color::Rgb(230, 192, 122))
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(DIALOG_BORDER))
        .style(Style::default().bg(DIALOG_BG));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    if portrait_w > 0 && inner.width > portrait_w + 8 {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(portrait_w), Constraint::Fill(1)])
            .split(inner);
        render_dialog_portrait(frame, columns[0], ctx);
        render_dialog_text(frame, columns[1], ctx);
    } else {
        render_dialog_text(frame, inner, ctx);
    }
}

const BELL_PORTRAIT: &[&str] = &[
    "    *    ",
    "   ***   ",
    "  #####  ",
    "  #***#  ",
    " ####### ",
    "   ###   ",
    "   # #   ",
    "  ## ##  ",
    "    .    ",
];

fn dialog_portrait_kind(npcs: &[Npc], dialog_npc_idx: Option<usize>) -> Option<NpcKind> {
    dialog_npc_idx
        .and_then(|index| npcs.get(index))
        .map(|npc| npc.kind)
}

fn render_dialog_portrait(frame: &mut Frame, area: Rect, ctx: &RenderCtx) {
    let portrait_block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(Color::Rgb(112, 88, 58)))
        .style(Style::default().bg(PORTRAIT_BG))
        .title(Span::styled(" likeness ", Style::default().fg(TEXT_DIM)));
    let inner = portrait_block.inner(area);
    frame.render_widget(portrait_block, area);

    let lines = dialog_portrait_lines(ctx);
    frame.render_widget(
        Paragraph::new(lines)
            .centered()
            .style(Style::default().bg(PORTRAIT_BG)),
        inner,
    );
}

fn dialog_portrait_lines(ctx: &RenderCtx) -> Vec<Line<'static>> {
    let (rows, primary, accent, skin) =
        if let Some(kind) = dialog_portrait_kind(ctx.npcs, ctx.dialog_npc_idx) {
            let art = crate::third_person::npc_art(kind, ctx.tick, false, true);
            (art.rows, art.primary, art.accent, art.skin)
        } else {
            (
                BELL_PORTRAIT,
                (132, 104, 64),
                (244, 210, 112),
                (244, 210, 112),
            )
        };
    let primary = visual::grade_art(ctx.color_filter, primary);
    let accent = visual::grade_art(ctx.color_filter, accent);
    let skin = visual::grade_art(ctx.color_filter, skin);

    rows.iter()
        .map(|row| {
            let spans: Vec<Span<'static>> = row
                .chars()
                .map(|symbol| {
                    let foreground = match symbol {
                        '#' => Color::Rgb(primary.0, primary.1, primary.2),
                        '*' | '+' => Color::Rgb(accent.0, accent.1, accent.2),
                        '@' => Color::Rgb(skin.0, skin.1, skin.2),
                        '.' => {
                            let dark = visual::darken(primary, 0.58);
                            Color::Rgb(dark.0, dark.1, dark.2)
                        }
                        _ => PORTRAIT_BG,
                    };
                    Span::styled(
                        symbol.to_string(),
                        Style::default().fg(foreground).bg(PORTRAIT_BG),
                    )
                })
                .collect();
            Line::from(spans)
        })
        .collect()
}

fn render_dialog_text(frame: &mut Frame, area: Rect, ctx: &RenderCtx) {
    let mut lines = vec![
        Line::from(Span::styled(
            ctx.dialog_npc_name,
            Style::default()
                .fg(Color::Rgb(230, 192, 122))
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            dialog_portrait_kind(ctx.npcs, ctx.dialog_npc_idx)
                .map(|kind| kind.name())
                .unwrap_or("a voice in the dark"),
            Style::default().fg(TEXT_DIM),
        )),
        Line::from(""),
    ];
    for line in ctx.dialog_text.iter() {
        lines.push(Line::from(Span::styled(
            line.as_str(),
            Style::default().fg(TEXT_BRIGHT),
        )));
    }

    if ctx.dialog_show_choices && !ctx.dialog_choices.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "─ Your response ─",
            Style::default().fg(Color::Rgb(164, 128, 78)),
        )));
        for (i, choice) in ctx.dialog_choices.iter().enumerate() {
            lines.push(Line::from(Span::styled(
                format!("  {}. {}", i + 1, choice),
                Style::default().fg(Color::Rgb(220, 198, 150)),
            )));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Press 1-3 to choose · Esc to close",
            Style::default().fg(TEXT_DIM),
        )));
    } else {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Press t or Esc to close",
            Style::default().fg(TEXT_DIM),
        )));
    }

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: true })
        .style(Style::default().bg(DIALOG_BG));
    frame.render_widget(p, area);
}

fn wrapped_dialog_line_count(line: &str, width: u16) -> u16 {
    let width = usize::from(width.max(1));
    let chars = line.chars().count().max(1);
    chars.div_ceil(width) as u16
}

// ══════════════════════════════════════════════════════════════════════════════
// QUEST PANEL
// ══════════════════════════════════════════════════════════════════════════════

fn render_quest_panel(frame: &mut Frame, area: Rect, ctx: &RenderCtx) {
    let popup_w = 58u16;
    // Calculate height based on content
    let objective_count = ctx.campaign_objectives.len().max(1);
    let mut content_lines: u16 = 7; // header + spacing
                                    // Objectives section
    content_lines += 1; // "── Objectives" label
    content_lines += objective_count as u16; // one per objective
    content_lines += 1; // spacing
                        // Story section
    if ctx.game_mode == GameMode::Campaign && !ctx.campaign_level_name.is_empty() {
        content_lines += 3; // level name + divider + story line
    }
    // Controls
    content_lines += 3;
    let popup_h = content_lines.min(area.height.saturating_sub(4));

    let popup = centered_rect(area, popup_w, popup_h);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(180, 160, 100)))
        .style(Style::default().bg(PANEL_BG))
        .title(Span::styled(
            " QUEST LOG ",
            Style::default().fg(Color::Rgb(255, 215, 0)),
        ));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let mut lines: Vec<Line> = Vec::new();

    if ctx.game_mode != GameMode::Campaign || ctx.campaign_objectives.is_empty() {
        // Classic mode — no active quest
        lines.push(Line::from(Span::styled(
            "No active quest.",
            Style::default().fg(TEXT_DIM),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Start a campaign from the main menu",
            Style::default().fg(TEXT_DIM),
        )));
        lines.push(Line::from(Span::styled(
            "for story-driven objectives.",
            Style::default().fg(TEXT_DIM),
        )));
    } else {
        // Campaign mode — show objectives and story
        lines.push(Line::from(Span::styled(
            "\u{2550}\u{2550} Objectives \u{2550}\u{2550}",
            Style::default()
                .fg(Color::Rgb(255, 215, 0))
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        if !ctx.campaign_level_name.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("  {} — {}", ctx.campaign_level_name, chapter_name(ctx)),
                Style::default().fg(Color::Rgb(200, 180, 100)),
            )));
            lines.push(Line::from(""));
        }

        for obj in ctx.campaign_objectives {
            let (icon, color) = if campaign_objective_done(obj, ctx) {
                ("\u{2713}", Color::Rgb(0, 255, 136))
            } else {
                ("\u{25C6}", Color::Rgb(180, 220, 255))
            };
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {}  ", icon),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    obj.display(),
                    if campaign_objective_done(obj, ctx) {
                        Style::default()
                            .fg(Color::Rgb(100, 160, 100))
                            .add_modifier(Modifier::CROSSED_OUT)
                    } else {
                        Style::default().fg(TEXT_BRIGHT)
                    },
                ),
            ]));
        }

        // Story recap based on campaign state
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "\u{2550}\u{2550} Story \u{2550}\u{2550}",
            Style::default()
                .fg(Color::Rgb(200, 160, 80))
                .add_modifier(Modifier::BOLD),
        )));

        let story_line = build_story_recap(ctx);
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("  {}", story_line),
            Style::default().fg(Color::Rgb(200, 200, 210)),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Tab / Esc — close",
        Style::default().fg(TEXT_DIM),
    )));

    let p = Paragraph::new(lines);
    frame.render_widget(p, inner);
}

/// Check if a campaign objective is complete based on flags.
fn campaign_objective_done(obj: &CampaignObjective, ctx: &RenderCtx) -> bool {
    use crate::campaign::CampaignFlag;
    let flags = ctx.campaign_flags;
    match obj {
        CampaignObjective::RecoverBellToken => flags.contains(&CampaignFlag::RecoveredBellToken),
        CampaignObjective::RelightLanterns { lit, needed } => lit >= needed,
        CampaignObjective::SilenceTheBell => {
            flags.contains(&CampaignFlag::BellChoiceRung)
                || flags.contains(&CampaignFlag::BellChoiceShattered)
                || flags.contains(&CampaignFlag::BellChoiceMuffled)
        }
    }
}

/// Build a one-line story recap based on campaign progress.
fn build_story_recap(ctx: &RenderCtx) -> &'static str {
    use crate::campaign::CampaignFlag;
    let flags = ctx.campaign_flags;

    if flags.contains(&CampaignFlag::BellChoiceRung)
        || flags.contains(&CampaignFlag::BellChoiceShattered)
        || flags.contains(&CampaignFlag::BellChoiceMuffled)
    {
        return "The bell is silenced. The Crown knows you are coming — stairs lead deeper.";
    }
    if flags.contains(&CampaignFlag::AllLanternsLit) {
        return "The three blue lanterns burn as one. Stone shifts — a path opens downward.";
    }
    if flags.contains(&CampaignFlag::AskedEddaAboutCrown) {
        return "Edda spoke of the Crown's question. The deeper you go, the more the dungeon remembers.";
    }
    if flags.contains(&CampaignFlag::SavedTovan) {
        return "Tovan owes you his life. His warning about the bell still echoes.";
    }
    if flags.contains(&CampaignFlag::RecoveredBellToken) {
        return "The Rustguard bell-token is yours. Mara Voss awaits at the gate.";
    }
    if flags.contains(&CampaignFlag::AskedJunForOil) {
        return "Jun's lantern oil burns blue. The waystation's lights must be restored.";
    }
    "The Compact sent you to recover the bell-token. Something in the deep has learned to ring back."
}

/// Get the chapter name from render context.
fn chapter_name(_ctx: &RenderCtx) -> &'static str {
    campaign::CampaignChapter::Chapter1.name()
}

// ══════════════════════════════════════════════════════════════════════════════
// HELPERS
// ══════════════════════════════════════════════════════════════════════════════

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialogue_portrait_resolves_the_exact_active_npc() {
        let npcs = vec![
            Npc::new(2, 2, NpcKind::WatchCaptain),
            Npc::new(3, 2, NpcKind::LanternTender),
        ];

        assert_eq!(
            dialog_portrait_kind(&npcs, Some(1)),
            Some(NpcKind::LanternTender)
        );
        assert_eq!(dialog_portrait_kind(&npcs, Some(9)), None);
        assert_eq!(dialog_portrait_kind(&npcs, None), None);
    }
}
