use crate::entity::{CampKind, ItemKind, Landmark, MonsterKind, NpcKind, TechniqueKind};
use crate::image_render::{AnchoredFrame, HalfBlockCanvas, SpriteFrame};
use crate::map::{PropKind, SurfaceKind, SurfaceMaterial, Tile};
use crate::render::RenderCtx;
use crate::visual::{
    self, Biome, PlayerActionKind, PlayerStepKind, PlayerTurnKind, Rgb, VisualEffect,
};
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::Frame;

const FOV: f64 = 66.0_f64.to_radians();
const MAX_VIEW_DEPTH: f64 = 18.0;

#[derive(Clone, Copy)]
struct Camera {
    x: f64,
    y: f64,
    dir_x: f64,
    dir_y: f64,
    plane_x: f64,
    plane_y: f64,
}

#[derive(Clone, Copy)]
struct RayHit {
    depth: f64,
    texture_x: f64,
    side: bool,
    tile: Tile,
    tile_x: i32,
    tile_y: i32,
    /// Exact world-space point where the ray met the wall face.
    hit_x: f64,
    hit_y: f64,
}

#[derive(Clone, Copy)]
struct LocalLight {
    x: f64,
    y: f64,
    color: Rgb,
    radius: f64,
    strength: f32,
}

#[derive(Clone, Copy)]
enum BillboardKind {
    Item(ItemKind, u64),
    Monster(MonsterVisual),
    Npc(NpcKind, u64, bool, bool),
    Prop(PropKind, u64),
    Projectile(i32, i32, u64),
    Landmark(Landmark, bool),
    Camp(CampKind, bool),
    Feature(Tile),
    Stain(Rgb),
    DeathEcho(u64),
    Effect(EffectArt),
}

#[derive(Clone, Copy)]
enum EffectArt {
    Attack {
        color: Rgb,
        timer: u8,
        material: ImpactMaterial,
    },
    WeaponSweep {
        weapon: Option<ItemKind>,
        dx: i32,
        dy: i32,
        counter: bool,
        timer: u8,
    },
    Technique {
        kind: TechniqueKind,
        dx: i32,
        dy: i32,
        impact: bool,
        timer: u8,
    },
    ItemCast {
        kind: ItemKind,
        timer: u8,
    },
    Movement {
        dx: i32,
        dy: i32,
        dash: bool,
        timer: u8,
    },
    MonsterCast {
        kind: MonsterKind,
        dx: i32,
        dy: i32,
        timer: u8,
    },
    Damage {
        amount: i32,
        color: Rgb,
        timer: u8,
    },
    Death {
        timer: u8,
        kind: Option<MonsterKind>,
    },
    Launch {
        dx: i32,
        dy: i32,
        timer: u8,
    },
    Poison {
        timer: u8,
    },
    Heal {
        timer: u8,
    },
    Sparkle {
        timer: u8,
    },
    Fire {
        timer: u8,
    },
    Confusion {
        timer: u8,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImpactMaterial {
    Flesh,
    Bone,
    Metal,
    Slime,
    Spectral,
    Water,
    Web,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MonsterPose {
    Idle,
    Step,
    Windup,
    Attack,
    Recover,
    Hit,
    Stagger,
}

#[derive(Clone, Copy)]
struct MonsterVisual {
    kind: MonsterKind,
    pose: MonsterPose,
    poisoned: bool,
    enraged: bool,
    phase: u64,
    step: (i32, i32),
}

#[derive(Clone, Copy)]
struct Billboard {
    world_x: f64,
    world_y: f64,
    kind: BillboardKind,
    scale: f64,
}

#[derive(Clone, Copy)]
struct Projection {
    center_x: i32,
    bottom_y: i32,
    pixel_size: i32,
    depth: f64,
}

#[derive(Clone, Copy)]
enum FloorSignature {
    Landmark(Landmark, i32, i32),
    Camp(CampKind, bool, i32, i32),
}

#[derive(Clone, Copy)]
struct RenderQuality {
    /// Number of short extrusion layers used for projected volume art.
    volume_layers: i32,
    /// Whether small bevels and secondary material marks are affordable.
    fine_detail: bool,
    /// Maximum number of background motes for this viewport.
    ambient_motes: usize,
    /// Maximum number of near-camera motes for this viewport.
    foreground_motes: usize,
    /// Whether light shafts should be drawn.
    light_shafts: bool,
}

#[derive(Clone, Copy)]
struct VolumeProfile {
    extrusion: i32,
    lift: i32,
    shadow: f32,
}

/// Hand-authored humanoid art shared by the viewport and dialogue portraits.
/// `#` is the role's clothing, `*` is its prop or heraldry, and `@` is skin.
#[derive(Clone, Copy)]
pub(crate) struct NpcArt {
    pub(crate) rows: &'static [&'static str],
    pub(crate) primary: Rgb,
    pub(crate) accent: Rgb,
    pub(crate) skin: Rgb,
}

fn render_quality(canvas: &HalfBlockCanvas, ctx: &RenderCtx<'_>) -> RenderQuality {
    let pixels = canvas.width().saturating_mul(canvas.pixel_height());
    let compact = pixels < 4_000;
    let full = pixels >= 8_000;
    RenderQuality {
        volume_layers: if compact { 1 } else { 2 },
        fine_detail: !compact,
        ambient_motes: if compact {
            6
        } else if full {
            14
        } else {
            10
        },
        foreground_motes: if compact {
            2
        } else if full {
            6
        } else {
            4
        },
        light_shafts: full && !ctx.reduced_flash,
    }
}

/// Pull the depth palettes toward stone, timber, iron, and torchlight while
/// leaving the selected color filter and camera projection untouched.
fn medieval_palette(ctx: &RenderCtx<'_>, base: visual::WorldPalette) -> visual::WorldPalette {
    let (floor, wall, fog, light) = match Biome::from(ctx.player.depth) {
        Biome::Depth0 => ((78, 68, 50), (132, 112, 76), (28, 24, 20), (246, 176, 86)),
        Biome::Depth1 => ((58, 70, 78), (108, 124, 130), (24, 28, 32), (184, 208, 220)),
        Biome::Depth2 => ((72, 50, 40), (126, 82, 58), (28, 20, 18), (244, 144, 64)),
        Biome::Depth3 => ((62, 48, 62), (106, 84, 92), (26, 20, 26), (220, 164, 110)),
        Biome::Depth4 => ((56, 48, 48), (112, 94, 72), (22, 18, 18), (236, 188, 100)),
    };
    visual::WorldPalette {
        floor: visual::lerp(base.floor, floor, 0.24),
        floor_memory: visual::lerp(base.floor_memory, visual::darken(floor, 0.56), 0.18),
        wall: visual::lerp(base.wall, wall, 0.28),
        wall_memory: visual::lerp(base.wall_memory, visual::darken(wall, 0.54), 0.20),
        fog: visual::lerp(base.fog, fog, 0.16),
        light: visual::lerp(base.light, light, 0.18),
    }
}

pub(crate) fn render_viewport(frame: &mut Frame<'_>, area: Rect, ctx: &RenderCtx<'_>) {
    let mut canvas = HalfBlockCanvas::new(area.width as usize, area.height as usize);
    let width = canvas.width() as i32;
    let height = canvas.pixel_height() as i32;
    if width < 4 || height < 4 {
        canvas.render(frame.buffer_mut(), area);
        return;
    }

    let palette = medieval_palette(
        ctx,
        visual::world_palette(ctx.color_filter, Biome::from(ctx.player.depth)),
    );
    let quality = render_quality(&canvas, ctx);
    let camera = camera(ctx);
    let lights = collect_local_lights(ctx, palette);
    let horizon = (height as f64 * 0.43).round() as i32 + ctx.camera_bob;
    draw_floor_and_ceiling(&mut canvas, ctx, camera, palette, horizon, &lights);
    draw_ambient_backdrop(&mut canvas, palette, camera, horizon, &lights, quality);
    let wall_depth = draw_walls(&mut canvas, ctx, camera, palette, horizon, &lights);
    draw_billboards(&mut canvas, ctx, camera, horizon, &wall_depth, quality);
    draw_ambient_motes(&mut canvas, ctx, horizon, quality, false);
    draw_peripheral_navigation(&mut canvas, ctx, camera, horizon);
    draw_forward_focus(&mut canvas, ctx, horizon);
    draw_ambient_motes(&mut canvas, ctx, horizon, quality, true);
    draw_player_foreground(&mut canvas, ctx);
    draw_screen_effects(&mut canvas, ctx);
    apply_luminous_pass(&mut canvas, ctx, &wall_depth, quality, horizon);

    canvas.vignette(0.32);
    if ctx.high_contrast {
        canvas.increase_contrast(0.18);
    }
    canvas.render(frame.buffer_mut(), area);
}

fn draw_ambient_backdrop(
    canvas: &mut HalfBlockCanvas,
    palette: visual::WorldPalette,
    camera: Camera,
    horizon: i32,
    lights: &[LocalLight],
    quality: RenderQuality,
) {
    let width = canvas.width() as i32;
    let height = canvas.pixel_height() as i32;
    let haze = visual::lerp(palette.fog, palette.light, 0.18);
    let band_top = (horizon - 4).clamp(0, height - 1);
    let band_height = (7.min(height - band_top)).max(0);
    if band_height > 0 {
        canvas.tint_rect(0, band_top, width, band_height, haze, 0.10);
    }

    if !quality.light_shafts {
        return;
    }
    for (index, light) in lights.iter().take(4).enumerate() {
        let probe = Billboard::at_world(light.x, light.y, BillboardKind::Stain(light.color), 0.1);
        let Some(projection) = project(canvas, camera, horizon, probe) else {
            continue;
        };
        let source_y = (projection.bottom_y - projection.pixel_size / 2).clamp(1, height - 2);
        let center = projection.center_x;
        let span = 4 + (index as i32 % 3) * 2;
        for step in 0..=6 {
            let t = step as f32 / 6.0;
            let y = source_y - ((source_y - horizon) as f32 * t) as i32;
            let half_width = (span as f32 * (0.22 + 0.78 * t)) as i32;
            for x in (center - half_width)..=(center + half_width) {
                canvas.tint_pixel(x, y, light.color, light.strength * 0.035);
            }
        }
    }
}

fn draw_ambient_motes(
    canvas: &mut HalfBlockCanvas,
    ctx: &RenderCtx<'_>,
    horizon: i32,
    quality: RenderQuality,
    foreground: bool,
) {
    let width = canvas.width() as i32;
    let height = canvas.pixel_height() as i32;
    if width < 8 || height < 8 {
        return;
    }
    let biome = Biome::from(ctx.player.depth);
    let (rgb, count, drift) = match biome {
        Biome::Depth0 => ((188, 146, 86), 8, 1),
        Biome::Depth1 => ((138, 204, 222), 7, -1),
        Biome::Depth2 => ((178, 106, 62), 9, -1),
        Biome::Depth3 => ((154, 202, 126), 10, 1),
        Biome::Depth4 => ((174, 166, 206), 11, -1),
    };
    let count = if foreground {
        quality.foreground_motes.min(count / 2 + 1)
    } else {
        quality.ambient_motes.min(count)
    };
    let count = if ctx.reduced_flash { count / 2 } else { count };
    let rgb = visual::grade_art(ctx.color_filter, rgb);
    let foreground_top = (horizon + 1).clamp(0, height - 1);
    let foreground_bottom = (height - 2).max(foreground_top);
    for index in 0..count {
        let hash = texture_hash(
            ctx.seed ^ if foreground { 0xF04E_6EED } else { 0xA77E_5EED },
            index as i32,
            ctx.player.depth as i32,
            width,
            height,
        );
        let x = 3 + (hash % (width - 6) as u64) as i32;
        let base_y = if foreground {
            let foreground_height = (height - horizon - 4).max(1) as u64;
            foreground_top + 1 + ((hash >> 16) % foreground_height) as i32
        } else {
            3 + ((hash >> 16) % (height - 6) as u64) as i32
        };
        let motion = if ctx.low_motion {
            0
        } else {
            ((ctx.tick / if foreground { 2 } else { 3 }
                + index as u64 * if foreground { 7 } else { 5 })
                % if foreground { 13 } else { 9 }) as i32
                * drift
        };
        let y = if foreground {
            (base_y + motion).clamp(foreground_top, foreground_bottom)
        } else {
            (base_y + motion).rem_euclid(height - 6) + 3
        };
        let mote = if y < horizon {
            visual::darken(rgb, 0.64)
        } else {
            rgb
        };
        let pulse = if ctx.low_motion {
            1.0
        } else {
            0.86 + ((ctx.tick / 2 + index as u64 * 3) % 7) as f32 * 0.025
        };
        let mote = scale(mote, pulse);
        let shadow = visual::darken(mote, 0.68);
        canvas.pixel(x, y, color(mote));
        match biome {
            Biome::Depth0 if index % 3 == 0 => {
                canvas.pixel(x - 1, y - 1, color(shadow));
            }
            Biome::Depth1 if index % 3 == 0 => {
                canvas.line(x - 1, y, x + 1, y, color(shadow));
            }
            Biome::Depth2 if index % 2 == 0 => {
                canvas.pixel(x, y + 1, color(shadow));
                if !foreground && index % 4 == 0 {
                    canvas.pixel(x + 1, y - 1, color(mote));
                }
            }
            Biome::Depth3 if index % 3 == 0 => {
                canvas.pixel(x + 1, y, color(shadow));
                if !foreground {
                    canvas.pixel(x, y + 1, color(shadow));
                }
            }
            Biome::Depth4 if index % 3 == 0 => {
                canvas.pixel(x - 1, y, color(shadow));
                canvas.pixel(x, y - 1, color(shadow));
            }
            _ => {}
        }
        if foreground && index % 2 == 0 {
            canvas.pixel(x, y + 1, color(shadow));
        }
    }
}

fn camera(ctx: &RenderCtx<'_>) -> Camera {
    let dir_x = ctx.camera_angle.cos();
    let dir_y = ctx.camera_angle.sin();
    let plane_scale = (FOV / 2.0).tan();
    // The eye sits just behind the player but remains inside the player's tile.
    let lateral = ctx.camera_sway as f64 * 0.008;
    let x = ctx.player.x as f64 + 0.5 - dir_x * 0.28 - dir_y * lateral;
    let y = ctx.player.y as f64 + 0.5 - dir_y * 0.28 + dir_x * lateral;
    Camera {
        x,
        y,
        dir_x,
        dir_y,
        plane_x: -dir_y * plane_scale,
        plane_y: dir_x * plane_scale,
    }
}

fn collect_local_lights(ctx: &RenderCtx<'_>, palette: visual::WorldPalette) -> Vec<LocalLight> {
    let mut lights = Vec::with_capacity(ctx.camps.len() + ctx.landmarks.len() + 8);
    for y in 0..ctx.map.height {
        for x in 0..ctx.map.width {
            let index = ctx.map.idx(x, y);
            if ctx.map.visible[index] && ctx.map.tiles[index] == Tile::LanternLit {
                lights.push(LocalLight {
                    x: x as f64 + 0.5,
                    y: y as f64 + 0.5,
                    color: palette.light,
                    radius: 3.6,
                    strength: 0.42,
                });
            }
        }
    }
    // A handful of authored flames is enough to sell the scene. Keeping the
    // light count bounded matters because every light is sampled per floor
    // pixel in the raycaster.
    for prop in ctx
        .map
        .props
        .iter()
        .filter(|prop| visible(ctx, prop.x, prop.y))
        .filter(|prop| prop.terrain_rules().light_radius > 0)
        .take(3)
    {
        let radius = prop.terrain_rules().light_radius;
        let kind = prop.kind;
        lights.push(LocalLight {
            x: prop.x as f64 + 0.5,
            y: prop.y as f64 + 0.5,
            color: visual::grade_art(ctx.color_filter, (226, 150, 62)),
            radius: f64::from(radius),
            strength: if kind == PropKind::Torch { 0.28 } else { 0.42 },
        });
    }
    for camp in ctx.camps {
        lights.push(LocalLight {
            x: camp.x as f64 + 0.5,
            y: camp.y as f64 + 0.5,
            color: visual::grade_art(ctx.color_filter, (246, 138, 48)),
            radius: 4.4,
            strength: if camp.rested { 0.28 } else { 0.52 },
        });
    }
    for &(room_index, landmark) in ctx.landmarks {
        if !matches!(landmark, Landmark::Shrine | Landmark::CollapsedForge) {
            continue;
        }
        let Some(room) = ctx.map.rooms.get(room_index) else {
            continue;
        };
        let (x, y) = room.center();
        lights.push(LocalLight {
            x: x as f64 + 0.5,
            y: y as f64 + 0.5,
            color: landmark_color(landmark),
            radius: 4.8,
            strength: 0.38,
        });
    }
    if ctx.player.lantern_turns > 0 {
        lights.push(LocalLight {
            x: ctx.player.x as f64 + 0.5,
            y: ctx.player.y as f64 + 0.5,
            color: palette.light,
            radius: 3.2,
            strength: 0.34,
        });
    }
    if ctx.player.weapon == Some(ItemKind::Emberbrand) {
        lights.push(LocalLight {
            x: ctx.player.x as f64 + 0.8,
            y: ctx.player.y as f64 + 0.5,
            color: visual::grade_art(ctx.color_filter, (244, 92, 34)),
            radius: 2.8,
            strength: if ctx.reduced_flash { 0.12 } else { 0.22 },
        });
    }
    for effect in &ctx.visual.effects {
        let (x, y, color, radius, strength) = match effect {
            VisualEffect::AttackFlash {
                x,
                y,
                flash_color,
                timer,
            } => (
                *x,
                *y,
                *flash_color,
                2.4,
                f32::from(*timer) / 6.0 * if ctx.reduced_flash { 0.08 } else { 0.18 },
            ),
            VisualEffect::FireRing {
                x,
                y,
                radius,
                timer,
            } => (
                *x,
                *y,
                (248, 102, 28),
                (2.0 + *radius as f64 * 0.35).min(5.0),
                f32::from(*timer) / 15.0 * if ctx.reduced_flash { 0.12 } else { 0.24 },
            ),
            VisualEffect::PoisonPulse { x, y, timer } => {
                (*x, *y, (92, 224, 82), 2.0, f32::from(*timer) / 8.0 * 0.12)
            }
            VisualEffect::HealGlow { x, y, timer } => (
                *x,
                *y,
                (104, 246, 166),
                2.6,
                f32::from(*timer) / 8.0 * if ctx.reduced_flash { 0.10 } else { 0.18 },
            ),
            VisualEffect::CrownSparkle { x, y, timer } => {
                (*x, *y, (246, 202, 54), 2.2, f32::from(*timer) / 30.0 * 0.14)
            }
            _ => continue,
        };
        if strength > 0.0 && visible(ctx, x, y) {
            lights.push(LocalLight {
                x: x as f64 + 0.5,
                y: y as f64 + 0.5,
                color: visual::grade_art(ctx.color_filter, color),
                radius,
                strength,
            });
        }
    }
    // Flame sources breathe: deterministic per-position flicker so every
    // torch burns on its own phase without any stored state.
    for light in &mut lights {
        light.strength *= flame_flicker(ctx, light.x, light.y);
    }
    lights
}

fn flame_flicker(ctx: &RenderCtx<'_>, x: f64, y: f64) -> f32 {
    if ctx.low_motion {
        return 1.0;
    }
    let amplitude = if ctx.reduced_flash { 0.05 } else { 0.14 };
    let hash = texture_hash(
        ctx.seed ^ 0xF1A4_E5E5,
        (x * 8.0) as i32,
        (y * 8.0) as i32,
        (ctx.tick / 3) as i32,
        0,
    );
    1.0 - amplitude + amplitude * 2.0 * (hash % 100) as f32 / 100.0
}

fn apply_local_lights(
    mut base: Rgb,
    world_x: f64,
    world_y: f64,
    ceiling: bool,
    lights: &[LocalLight],
) -> Rgb {
    for light in lights {
        let distance = ((world_x - light.x).powi(2) + (world_y - light.y).powi(2)).sqrt();
        let falloff = ((light.radius - distance) / light.radius).clamp(0.0, 1.0) as f32;
        let ceiling_factor = if ceiling { 0.46 } else { 1.0 };
        base = visual::lerp(base, light.color, falloff * light.strength * ceiling_factor);
    }
    base
}

fn draw_floor_and_ceiling(
    canvas: &mut HalfBlockCanvas,
    ctx: &RenderCtx<'_>,
    camera: Camera,
    palette: visual::WorldPalette,
    horizon: i32,
    lights: &[LocalLight],
) {
    let width = canvas.width() as i32;
    let height = canvas.pixel_height() as i32;
    canvas.fill(color(palette.fog));
    let ray_left = (camera.dir_x - camera.plane_x, camera.dir_y - camera.plane_y);
    let ray_right = (camera.dir_x + camera.plane_x, camera.dir_y + camera.plane_y);
    let projection = height as f64 * 0.38;

    for screen_y in 0..height {
        let ceiling = screen_y < horizon;
        let distance_from_horizon = (screen_y - horizon).abs().max(1) as f64;
        let row_depth = projection / distance_from_horizon;
        for screen_x in 0..width {
            let t = screen_x as f64 / width.max(1) as f64;
            let ray_x = ray_left.0 + (ray_right.0 - ray_left.0) * t;
            let ray_y = ray_left.1 + (ray_right.1 - ray_left.1) * t;
            let world_x = camera.x + ray_x * row_depth;
            let world_y = camera.y + ray_y * row_depth;
            let tile_x = world_x.floor() as i32;
            let tile_y = world_y.floor() as i32;
            let signature = (!ceiling)
                .then(|| floor_signature(ctx, tile_x, tile_y))
                .flatten();
            let base = if ceiling {
                // Vaulted dark: the ceiling falls off toward the top of the
                // frame and the side walls, suggesting an arched cavern roof
                // rather than a flat slab overhead.
                let vault_t = screen_y as f32 / horizon.max(1) as f32;
                let side_t = (screen_x - width / 2).abs() as f32 / (width / 2).max(1) as f32;
                let vault = 0.60 + 0.40 * vault_t * (1.0 - 0.30 * side_t);
                scale(visual::darken(palette.wall, 0.34), vault)
            } else {
                floor_material(ctx, palette, tile_x, tile_y, signature)
            };
            let local_x = world_x.rem_euclid(1.0);
            let local_y = world_y.rem_euclid(1.0);
            let seam = local_x < 0.035 || local_y < 0.035;
            let texture = texture_hash(ctx.seed, tile_x, tile_y, screen_x, screen_y);
            let variation = if seam {
                0.62
            } else {
                0.88 + (texture % 8) as f32 * 0.025
            };
            let fog_t = (row_depth / MAX_VIEW_DEPTH).clamp(0.0, 1.0);
            let fog = (fog_t * fog_t * (3.0 - 2.0 * fog_t)) as f32;
            let dither =
                ((texture % 9) as f32 * 0.004 - 0.016) * (fog * (1.0 - fog) * 4.0).clamp(0.0, 1.0);
            let detailed = floor_signature_detail(
                ctx, signature, base, world_x, world_y, local_x, local_y, texture,
            );
            let detailed = floor_tile_detail(
                ctx, tile_x, tile_y, detailed, world_x, world_y, local_x, local_y, texture,
            );
            let detailed = if ceiling {
                ceiling_detail(detailed, local_x, local_y, texture)
            } else {
                detailed
            };
            let detailed = apply_local_lights(detailed, world_x, world_y, ceiling, lights);
            let shaded = visual::lerp(
                scale(detailed, variation),
                palette.fog,
                (fog * 0.82 + dither).clamp(0.0, 1.0),
            );
            canvas.pixel(screen_x, screen_y, color(shaded));
        }
    }

    let haze = visual::lerp(palette.fog, palette.light, 0.12);
    for offset in -1..=1 {
        canvas.line(
            0,
            horizon + offset,
            width - 1,
            horizon + offset,
            color(haze),
        );
    }
}

fn ceiling_detail(base: Rgb, local_x: f64, local_y: f64, texture: u64) -> Rgb {
    let beam = local_x < 0.055 || local_y < 0.055;
    let fracture = texture % 97 == 0 || (texture % 131 == 0 && local_x > 0.45);
    if beam {
        visual::darken(base, 0.58)
    } else if fracture {
        visual::darken(base, 0.72)
    } else {
        base
    }
}

fn floor_material(
    ctx: &RenderCtx<'_>,
    palette: visual::WorldPalette,
    x: i32,
    y: i32,
    signature: Option<FloorSignature>,
) -> Rgb {
    if !ctx.map.in_bounds(x, y) {
        return palette.fog;
    }
    let index = ctx.map.idx(x, y);
    if !ctx.map.visible[index] {
        return palette.fog;
    }
    let material = match ctx.map.tiles[index] {
        Tile::ShallowWater => visual::grade_art(ctx.color_filter, (38, 102, 136)),
        Tile::Web => visual::grade_art(ctx.color_filter, (104, 98, 116)),
        Tile::DoorOpen => visual::grade_art(ctx.color_filter, (82, 54, 34)),
        Tile::TrapRevealed => visual::grade_art(ctx.color_filter, (124, 48, 40)),
        Tile::LanternLit => visual::lerp(palette.floor, palette.light, 0.42),
        Tile::LanternDead => visual::darken(palette.floor, 0.62),
        Tile::StairsDown => visual::grade_art(ctx.color_filter, (68, 48, 96)),
        Tile::StairsUp => visual::grade_art(ctx.color_filter, (126, 94, 58)),
        _ => surface_material(ctx, palette, x, y),
    };
    match signature {
        Some(FloorSignature::Landmark(kind, _, _)) => {
            visual::lerp(material, landmark_color(kind), 0.18)
        }
        Some(FloorSignature::Camp(kind, rested, _, _)) => {
            let warmth = if rested { 0.1 } else { 0.2 };
            visual::lerp(material, camp_color(kind), warmth)
        }
        None => material,
    }
}

fn surface_material(ctx: &RenderCtx<'_>, palette: visual::WorldPalette, x: i32, y: i32) -> Rgb {
    let Some(surface) = ctx.map.surface_at(x, y) else {
        return palette.floor;
    };
    let rgb = match surface {
        SurfaceKind::Floor(material)
        | SurfaceKind::Ground(material)
        | SurfaceKind::Road(material)
        | SurfaceKind::Bridge(material) => match material {
            SurfaceMaterial::Stone => palette.floor,
            SurfaceMaterial::Cobblestone => (112, 94, 72),
            SurfaceMaterial::Dirt => (98, 70, 46),
            SurfaceMaterial::Grass => (56, 78, 48),
            SurfaceMaterial::Timber => (100, 68, 44),
            SurfaceMaterial::Moss => (68, 88, 56),
        },
        SurfaceKind::Mud => (76, 54, 40),
        SurfaceKind::ShallowWater => (38, 102, 136),
    };
    visual::grade_art(ctx.color_filter, rgb)
}

fn floor_signature(ctx: &RenderCtx<'_>, x: i32, y: i32) -> Option<FloorSignature> {
    for &(room_index, landmark) in ctx.landmarks {
        let Some(room) = ctx.map.rooms.get(room_index) else {
            continue;
        };
        if x >= room.x && x < room.x + room.w && y >= room.y && y < room.y + room.h {
            let (cx, cy) = room.center();
            return Some(FloorSignature::Landmark(landmark, cx, cy));
        }
    }
    ctx.camps.iter().find_map(|camp| {
        let distance = (x - camp.x).pow(2) + (y - camp.y).pow(2);
        (distance <= 12).then_some(FloorSignature::Camp(camp.kind, camp.rested, camp.x, camp.y))
    })
}

#[allow(clippy::too_many_arguments)]
fn floor_signature_detail(
    ctx: &RenderCtx<'_>,
    signature: Option<FloorSignature>,
    base: Rgb,
    world_x: f64,
    world_y: f64,
    local_x: f64,
    local_y: f64,
    texture: u64,
) -> Rgb {
    let Some(signature) = signature else {
        return base;
    };
    match signature {
        FloorSignature::Landmark(Landmark::SpiderNest, _, _) => {
            let diagonal =
                (local_x - local_y).abs() < 0.035 || (local_x + local_y - 1.0).abs() < 0.035;
            if diagonal || texture % 29 == 0 {
                visual::lerp(
                    base,
                    visual::grade_art(ctx.color_filter, (220, 214, 226)),
                    0.62,
                )
            } else {
                base
            }
        }
        FloorSignature::Landmark(Landmark::FloodedVault, _, _) => {
            let ripple = ((world_x + world_y * 0.7) * 5.0).sin().abs() < 0.12;
            if ripple {
                visual::lerp(
                    base,
                    visual::grade_art(ctx.color_filter, (102, 202, 224)),
                    0.46,
                )
            } else {
                visual::lerp(
                    base,
                    visual::grade_art(ctx.color_filter, (32, 92, 132)),
                    0.2,
                )
            }
        }
        FloorSignature::Landmark(Landmark::BoneChapel, _, _) => {
            if texture % 23 < 2 && (0.2..0.8).contains(&local_x) {
                visual::lerp(
                    base,
                    visual::grade_art(ctx.color_filter, (226, 216, 174)),
                    0.56,
                )
            } else {
                base
            }
        }
        FloorSignature::Landmark(Landmark::CollapsedForge, _, _) => {
            if texture % 31 == 0 {
                visual::lerp(
                    base,
                    visual::grade_art(ctx.color_filter, (242, 88, 28)),
                    0.7,
                )
            } else {
                visual::darken(base, 0.9)
            }
        }
        FloorSignature::Landmark(Landmark::Shrine, cx, cy) => {
            let radius = ((world_x - (cx as f64 + 0.5)).powi(2)
                + (world_y - (cy as f64 + 0.5)).powi(2))
            .sqrt();
            if (radius * 2.2).fract() < 0.13 {
                visual::lerp(
                    base,
                    visual::grade_art(ctx.color_filter, (255, 230, 142)),
                    0.58,
                )
            } else {
                base
            }
        }
        FloorSignature::Landmark(Landmark::BellChamber, cx, cy) => {
            let crack = ((world_x - cx as f64) - (world_y - cy as f64) * 0.45).abs() < 0.055;
            if crack {
                visual::lerp(
                    base,
                    visual::grade_art(ctx.color_filter, (218, 146, 54)),
                    0.52,
                )
            } else {
                visual::darken(base, 0.88)
            }
        }
        FloorSignature::Camp(_, rested, cx, cy) => {
            let distance = ((world_x - (cx as f64 + 0.5)).powi(2)
                + (world_y - (cy as f64 + 0.5)).powi(2))
            .sqrt();
            let strength = ((3.4 - distance) / 3.4).clamp(0.0, 1.0) as f32;
            let light = if rested {
                (142, 92, 52)
            } else {
                (248, 146, 54)
            };
            visual::lerp(
                base,
                visual::grade_art(ctx.color_filter, light),
                strength * 0.28,
            )
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn floor_tile_detail(
    ctx: &RenderCtx<'_>,
    x: i32,
    y: i32,
    base: Rgb,
    world_x: f64,
    world_y: f64,
    local_x: f64,
    local_y: f64,
    texture: u64,
) -> Rgb {
    if !visible(ctx, x, y) {
        return base;
    }
    let tile_detail = match ctx.map.tiles[ctx.map.idx(x, y)] {
        Tile::ShallowWater => {
            let ripple = ((world_x * 0.8 + world_y) * 7.0).sin().abs() < 0.14;
            if ripple {
                visual::lerp(
                    base,
                    visual::grade_art(ctx.color_filter, (128, 218, 238)),
                    0.5,
                )
            } else {
                base
            }
        }
        Tile::Web => {
            let strand = (local_x - local_y).abs() < 0.04
                || (local_x + local_y - 1.0).abs() < 0.04
                || (local_x - 0.5).abs() < 0.025;
            if strand {
                visual::lerp(
                    base,
                    visual::grade_art(ctx.color_filter, (226, 220, 232)),
                    0.64,
                )
            } else {
                base
            }
        }
        Tile::TrapRevealed => {
            let blade =
                (local_x - local_y).abs() < 0.055 || (local_x + local_y - 1.0).abs() < 0.055;
            if blade {
                visual::lerp(
                    base,
                    visual::grade_art(ctx.color_filter, (242, 116, 54)),
                    0.58,
                )
            } else {
                base
            }
        }
        Tile::DoorOpen => {
            if local_x < 0.08 || (local_y * 4.0).fract() < 0.08 {
                visual::darken(base, 0.58)
            } else {
                base
            }
        }
        Tile::StairsDown | Tile::StairsUp => {
            if (local_y * 5.0).fract() < 0.16 {
                visual::lerp(
                    base,
                    visual::grade_art(ctx.color_filter, (226, 202, 164)),
                    0.34,
                )
            } else {
                base
            }
        }
        Tile::Inscription | Tile::History(_) => {
            if texture % 13 == 0 {
                visual::lerp(
                    base,
                    visual::grade_art(ctx.color_filter, (122, 204, 226)),
                    0.5,
                )
            } else {
                base
            }
        }
        _ => base,
    };
    biome_floor_detail(
        ctx,
        tile_detail,
        world_x,
        world_y,
        local_x,
        local_y,
        texture,
    )
}

fn biome_floor_detail(
    ctx: &RenderCtx<'_>,
    base: Rgb,
    world_x: f64,
    world_y: f64,
    local_x: f64,
    local_y: f64,
    texture: u64,
) -> Rgb {
    let (mark, amount, present) = match Biome::from(ctx.player.depth) {
        Biome::Depth0 => (
            visual::grade_art(ctx.color_filter, (104, 136, 72)),
            0.20,
            (local_y > 0.78 || local_x < 0.08) && texture % 23 == 0,
        ),
        Biome::Depth1 => (
            visual::grade_art(ctx.color_filter, (84, 164, 202)),
            0.18,
            ((world_x * 0.7 + world_y * 1.1) * 4.0).sin().abs() < 0.08 && texture % 3 == 0,
        ),
        Biome::Depth2 => (
            visual::grade_art(ctx.color_filter, (148, 66, 38)),
            0.22,
            local_x > 0.62 && texture % 29 == 0,
        ),
        Biome::Depth3 => (
            visual::grade_art(ctx.color_filter, (126, 74, 156)),
            0.20,
            ((local_x - local_y).abs() < 0.035 || (local_x + local_y - 1.0).abs() < 0.035)
                && texture % 4 == 0,
        ),
        Biome::Depth4 => (
            visual::grade_art(ctx.color_filter, (174, 132, 70)),
            0.18,
            (local_x - 0.5).abs() < 0.035 && texture % 11 == 0,
        ),
    };
    if present {
        visual::lerp(base, mark, amount)
    } else {
        base
    }
}

fn draw_walls(
    canvas: &mut HalfBlockCanvas,
    ctx: &RenderCtx<'_>,
    camera: Camera,
    palette: visual::WorldPalette,
    horizon: i32,
    lights: &[LocalLight],
) -> Vec<f64> {
    let width = canvas.width() as i32;
    let height = canvas.pixel_height() as i32;
    let mut depth_buffer = vec![MAX_VIEW_DEPTH; width as usize];

    for screen_x in 0..width {
        let camera_x = 2.0 * screen_x as f64 / width as f64 - 1.0;
        let ray_x = camera.dir_x + camera.plane_x * camera_x;
        let ray_y = camera.dir_y + camera.plane_y * camera_x;
        let Some(hit) = cast_ray(ctx, camera.x, camera.y, ray_x, ray_y) else {
            continue;
        };
        depth_buffer[screen_x as usize] = hit.depth;
        let wall_height = ((height as f64 * 0.78) / hit.depth.max(0.08)).round() as i32;
        let top = (horizon - wall_height / 2).max(0);
        let bottom = (horizon + wall_height / 2).min(height - 1);
        for screen_y in top..=bottom {
            let texture_y = (screen_y - top) as f64 / (bottom - top + 1).max(1) as f64;
            let base = wall_material(ctx, palette, hit.tile);
            // Catch-light along the top lip; the bottom lip falls into the
            // floor-contact occlusion below instead of glowing.
            let edge = texture_y <= 0.025;
            let (surface, mut shade) = match hit.tile {
                Tile::DoorClosed => {
                    let plank = (hit.texture_x * 5.0).fract() < 0.10;
                    let jamb = !(0.09..=0.91).contains(&hit.texture_x) || texture_y < 0.08;
                    let bevel = !(0.14..=0.86).contains(&hit.texture_x)
                        || !(0.14..=0.88).contains(&texture_y);
                    let handle =
                        (hit.texture_x - 0.72).abs() < 0.025 && (texture_y - 0.56).abs() < 0.035;
                    if handle {
                        (visual::grade_art(ctx.color_filter, (196, 154, 72)), 1.08)
                    } else if jamb {
                        (visual::darken(base, 0.48), 0.72)
                    } else {
                        (base, if plank || bevel { 0.54 } else { 0.96 })
                    }
                }
                Tile::LockedDoor => {
                    let jamb = !(0.08..=0.92).contains(&hit.texture_x) || texture_y < 0.07;
                    let iron_band = (texture_y - 0.24).abs() < 0.055
                        || (texture_y - 0.76).abs() < 0.055
                        || (hit.texture_x - 0.5).abs() < 0.045;
                    let lock =
                        (hit.texture_x - 0.68).abs() < 0.045 && (texture_y - 0.53).abs() < 0.055;
                    if jamb {
                        (visual::darken(base, 0.42), 0.68)
                    } else if lock {
                        (visual::grade_art(ctx.color_filter, (202, 166, 82)), 1.0)
                    } else if iron_band {
                        (visual::grade_art(ctx.color_filter, (128, 132, 126)), 0.82)
                    } else {
                        (base, 0.92)
                    }
                }
                Tile::Barricade => {
                    let crossing = (hit.texture_x * 2.0 + texture_y * 3.0).fract() < 0.20
                        || (hit.texture_x * 2.0 - texture_y * 3.0).fract().abs() < 0.20;
                    (base, if crossing { 1.04 } else { 0.48 })
                }
                _ => {
                    let mortar =
                        (hit.texture_x * 6.0).fract() < 0.10 || (texture_y * 7.0).fract() < 0.09;
                    let course_offset = ((texture_y * 7.0).floor() as i32 & 1) as f64 * 0.5;
                    let vertical_joint = ((hit.texture_x * 6.0 + course_offset).fract()) < 0.08;
                    (base, if mortar || vertical_joint { 0.54 } else { 0.94 })
                }
            };
            let texture = texture_hash(ctx.seed, screen_x, screen_y, hit.tile_x, hit.tile_y);
            let surface =
                wall_surface_detail(ctx, hit.tile, surface, hit.texture_x, texture_y, texture);
            if hit.side {
                shade *= 0.72;
            }
            if hit.texture_x < 0.018 || hit.texture_x > 0.982 {
                shade *= 0.58;
            }
            if edge {
                shade *= 1.18;
            }
            // Cylindrical volume: brightest near the eye line, falling off
            // toward both joints so flat columns read as rounded mass.
            let mid = (1.0 - (2.0 * texture_y - 1.0).abs()) as f32;
            shade *= 0.86 + 0.14 * mid;
            // Ambient occlusion where the wall meets floor and ceiling.
            if texture_y > 0.85 {
                let t = ((texture_y - 0.85) / 0.15) as f32;
                shade *= 1.0 - 0.30 * t;
            }
            if texture_y < 0.10 {
                let t = (1.0 - texture_y / 0.10) as f32;
                shade *= 1.0 - 0.20 * t;
            }
            let fog_t = (hit.depth / MAX_VIEW_DEPTH).clamp(0.0, 1.0);
            let fog = (fog_t * fog_t * (3.0 - 2.0 * fog_t)) as f32;
            // Per-pixel dither melts the banding at the fog boundary.
            let dither =
                ((texture % 9) as f32 * 0.004 - 0.016) * (fog * (1.0 - fog) * 4.0).clamp(0.0, 1.0);
            let lit =
                apply_local_lights(scale(surface, shade), hit.hit_x, hit.hit_y, false, lights);
            let shaded = visual::lerp(lit, palette.fog, (fog * 0.88 + dither).clamp(0.0, 1.0));
            canvas.pixel(screen_x, screen_y, color(shaded));
        }
        // Grounded contact shadow: the floor darkens just past the wall base
        // and the ceiling just past the wall top, seating the wall in space.
        for i in 1..=3i32 {
            let falloff = 0.30 - i as f32 * 0.08;
            canvas.tint_pixel(screen_x, bottom + i, (5, 6, 8), falloff);
            canvas.tint_pixel(screen_x, top - i, (5, 6, 8), falloff * 0.6);
        }
    }
    depth_buffer
}

fn cast_ray(
    ctx: &RenderCtx<'_>,
    origin_x: f64,
    origin_y: f64,
    ray_x: f64,
    ray_y: f64,
) -> Option<RayHit> {
    let mut map_x = origin_x.floor() as i32;
    let mut map_y = origin_y.floor() as i32;
    let delta_x = if ray_x.abs() < 1e-8 {
        1e30
    } else {
        (1.0 / ray_x).abs()
    };
    let delta_y = if ray_y.abs() < 1e-8 {
        1e30
    } else {
        (1.0 / ray_y).abs()
    };
    let (step_x, mut side_x) = if ray_x < 0.0 {
        (-1, (origin_x - map_x as f64) * delta_x)
    } else {
        (1, (map_x as f64 + 1.0 - origin_x) * delta_x)
    };
    let (step_y, mut side_y) = if ray_y < 0.0 {
        (-1, (origin_y - map_y as f64) * delta_y)
    } else {
        (1, (map_y as f64 + 1.0 - origin_y) * delta_y)
    };
    for _ in 0..96 {
        let side = if side_x < side_y {
            side_x += delta_x;
            map_x += step_x;
            false
        } else {
            side_y += delta_y;
            map_y += step_y;
            true
        };
        if !ctx.map.in_bounds(map_x, map_y) {
            return None;
        }
        if !ctx.map.visible[ctx.map.idx(map_x, map_y)] {
            return None;
        }
        if ctx.map.blocks_fov(map_x, map_y) {
            let depth = if side {
                (map_y as f64 - origin_y + (1 - step_y) as f64 / 2.0) / ray_y
            } else {
                (map_x as f64 - origin_x + (1 - step_x) as f64 / 2.0) / ray_x
            };
            if depth > MAX_VIEW_DEPTH {
                return None;
            }
            let wall_coordinate = if side {
                origin_x + depth * ray_x
            } else {
                origin_y + depth * ray_y
            };
            return Some(RayHit {
                depth: depth.max(0.02),
                texture_x: wall_coordinate.rem_euclid(1.0),
                side,
                tile: ctx.map.tiles[ctx.map.idx(map_x, map_y)],
                tile_x: map_x,
                tile_y: map_y,
                hit_x: origin_x + depth * ray_x,
                hit_y: origin_y + depth * ray_y,
            });
        }
    }
    None
}

fn wall_material(ctx: &RenderCtx<'_>, palette: visual::WorldPalette, tile: Tile) -> Rgb {
    match tile {
        Tile::DoorClosed => visual::grade_art(ctx.color_filter, (116, 72, 38)),
        Tile::LockedDoor => visual::grade_art(ctx.color_filter, (104, 78, 52)),
        Tile::Barricade => visual::grade_art(ctx.color_filter, (92, 58, 36)),
        _ => palette.wall,
    }
}

fn wall_surface_detail(
    ctx: &RenderCtx<'_>,
    tile: Tile,
    base: Rgb,
    texture_x: f64,
    texture_y: f64,
    texture: u64,
) -> Rgb {
    if matches!(tile, Tile::DoorClosed | Tile::LockedDoor | Tile::Barricade) {
        return base;
    }
    let biome = Biome::from(ctx.player.depth);
    let (mark, amount, present) = match biome {
        Biome::Depth0 => (
            visual::grade_art(ctx.color_filter, (106, 142, 76)),
            0.24,
            texture_y > 0.58 && texture % 17 < 2,
        ),
        Biome::Depth1 => (
            visual::grade_art(ctx.color_filter, (94, 178, 210)),
            0.22,
            (texture_x * 5.0 + texture_y * 2.0).fract() < 0.045 && texture % 3 == 0,
        ),
        Biome::Depth2 => (
            visual::grade_art(ctx.color_filter, (166, 74, 42)),
            0.28,
            texture_x > 0.62 && texture_y > 0.18 && texture_y < 0.88 && texture % 19 == 0,
        ),
        Biome::Depth3 => (
            visual::grade_art(ctx.color_filter, (148, 86, 176)),
            0.26,
            ((texture_x - texture_y).abs() < 0.026 || (texture_x + texture_y - 1.0).abs() < 0.026)
                && texture % 5 == 0,
        ),
        Biome::Depth4 => (
            visual::grade_art(ctx.color_filter, (194, 154, 82)),
            0.24,
            (texture_x - 0.5).abs() < 0.026 && texture_y > 0.16 && texture % 7 == 0,
        ),
    };
    if present {
        visual::lerp(base, mark, amount)
    } else {
        base
    }
}

fn draw_billboards(
    canvas: &mut HalfBlockCanvas,
    ctx: &RenderCtx<'_>,
    camera: Camera,
    horizon: i32,
    wall_depth: &[f64],
    quality: RenderQuality,
) {
    let mut billboards = collect_billboards(ctx);
    billboards.sort_by(|a, b| {
        distance_sq(camera, b.world_x, b.world_y)
            .total_cmp(&distance_sq(camera, a.world_x, a.world_y))
    });
    for billboard in billboards {
        let Some(projection) = project(canvas, camera, horizon, billboard) else {
            continue;
        };
        draw_billboard(canvas, ctx, billboard.kind, projection, wall_depth, quality);
    }
}

fn collect_billboards(ctx: &RenderCtx<'_>) -> Vec<Billboard> {
    let mut sprites = Vec::new();
    for y in 0..ctx.map.height {
        for x in 0..ctx.map.width {
            if !visible(ctx, x, y) {
                continue;
            }
            let tile = ctx.map.tiles[ctx.map.idx(x, y)];
            if matches!(
                tile,
                Tile::StairsDown
                    | Tile::StairsUp
                    | Tile::LanternLit
                    | Tile::LanternDead
                    | Tile::TrapRevealed
                    | Tile::Urn
                    | Tile::DoorOpen
                    | Tile::Inscription
                    | Tile::History(_)
            ) {
                sprites.push(Billboard::at(x, y, BillboardKind::Feature(tile), 0.58));
            }
        }
    }
    for prop in &ctx.map.props {
        if visible(ctx, prop.x, prop.y) {
            let phase = visual::staggered_phase(ctx.tick, prop.x, prop.y, 0x5052_4F50, 32);
            sprites.push(Billboard::at(
                prop.x,
                prop.y,
                BillboardKind::Prop(prop.kind, phase),
                prop_scale(prop.kind),
            ));
        }
    }
    if let Some((x, y)) = ctx.run_history.iter().rev().find_map(|run| {
        let (Some(x), Some(y)) = (run.final_x, run.final_y) else {
            return None;
        };
        (!run.won
            && run.seed == ctx.seed
            && run.final_depth == ctx.player.depth
            && ctx.map.in_bounds(x, y)
            && (x, y) != (ctx.player.x, ctx.player.y)
            && visible(ctx, x, y))
        .then_some((x, y))
    }) {
        let phase = visual::staggered_phase(ctx.tick, x, y, 0xec40, 48);
        sprites.push(Billboard::at(x, y, BillboardKind::DeathEcho(phase), 0.7));
    }
    for item in ctx.items {
        if visible(ctx, item.x, item.y) {
            let phase = visual::staggered_phase(ctx.tick, item.x, item.y, item.kind as u64, 48);
            sprites.push(Billboard::at(
                item.x,
                item.y,
                BillboardKind::Item(item.kind, phase),
                0.46,
            ));
        }
    }
    for (npc_index, npc) in ctx.npcs.iter().enumerate() {
        if visible(ctx, npc.x, npc.y) {
            let phase = visual::staggered_phase(ctx.tick, npc.x, npc.y, 0x004e_5043, 64);
            let talking = ctx.dialog_npc_idx == Some(npc_index);
            sprites.push(Billboard::at(
                npc.x,
                npc.y,
                BillboardKind::Npc(npc.kind, phase, npc.talked, talking),
                0.82,
            ));
        }
    }
    for monster in ctx.monsters.iter().filter(|monster| monster.alive) {
        if visible(ctx, monster.x, monster.y) {
            let hit = ctx.visual.effects.iter().any(|effect| {
                matches!(effect, VisualEffect::DamageNumber { x, y, .. } if (*x, *y) == (monster.x, monster.y))
            });
            let attacking = ctx.visual.effects.iter().find_map(|effect| match effect {
                VisualEffect::AttackFlash {
                    x,
                    y,
                    flash_color,
                    timer,
                } if (*x, *y) == (monster.x, monster.y) && flash_color.0 > flash_color.1 => {
                    Some(*timer)
                }
                _ => None,
            });
            let step = ctx.visual.effects.iter().find_map(|effect| match effect {
                VisualEffect::ActorStep {
                    x,
                    y,
                    dx,
                    dy,
                    timer,
                } if (*x, *y) == (monster.x, monster.y) => Some((*dx, *dy, *timer)),
                _ => None,
            });
            let pose = if hit {
                MonsterPose::Hit
            } else if monster.windup > 0 {
                MonsterPose::Windup
            } else if attacking.is_some_and(|timer| timer > 2) {
                MonsterPose::Attack
            } else if attacking.is_some() {
                MonsterPose::Recover
            } else if monster.staggered > 0 {
                MonsterPose::Stagger
            } else if step.is_some() {
                MonsterPose::Step
            } else {
                MonsterPose::Idle
            };
            let phase =
                visual::staggered_phase(ctx.tick, monster.x, monster.y, monster.kind as u64, 48);
            let (world_x, world_y) = step.map_or(
                (monster.x as f64 + 0.5, monster.y as f64 + 0.5),
                |(dx, dy, timer)| {
                    let progress = actor_step_progress(timer);
                    (
                        monster.x as f64 + 0.5 - dx as f64 * (1.0 - progress),
                        monster.y as f64 + 0.5 - dy as f64 * (1.0 - progress),
                    )
                },
            );
            sprites.push(Billboard::at_world(
                world_x,
                world_y,
                BillboardKind::Monster(MonsterVisual {
                    kind: monster.kind,
                    pose,
                    poisoned: monster.poisoned,
                    enraged: monster.kind == MonsterKind::Guardian
                        && monster.stats.hp * 2 <= monster.stats.max_hp,
                    phase,
                    step: step.map_or((0, 0), |(dx, dy, _)| (dx, dy)),
                }),
                0.9,
            ));
        }
    }
    for projectile in ctx.projectiles {
        if visible(ctx, projectile.x, projectile.y) {
            let phase = visual::staggered_phase(ctx.tick, projectile.x, projectile.y, 0xb017, 8);
            sprites.push(Billboard::at(
                projectile.x,
                projectile.y,
                BillboardKind::Projectile(projectile.dx, projectile.dy, phase),
                0.28,
            ));
        }
    }
    for &(room_index, landmark) in ctx.landmarks {
        let Some(room) = ctx.map.rooms.get(room_index) else {
            continue;
        };
        let (x, y) = room.center();
        if visible(ctx, x, y) {
            sprites.push(Billboard::at(
                x,
                y,
                BillboardKind::Landmark(landmark, ctx.used_landmarks.contains(&room_index)),
                1.1,
            ));
        }
    }
    for camp in ctx.camps {
        if visible(ctx, camp.x, camp.y) {
            sprites.push(Billboard::at(
                camp.x,
                camp.y,
                BillboardKind::Camp(camp.kind, camp.rested),
                0.82,
            ));
        }
    }
    for decal in &ctx.visual.decals {
        let (x, y, stain) = match *decal {
            crate::visual::Decal::Blood { x, y, fade } => {
                (x, y, scale((126, 24, 26), fade as f32 / 255.0))
            }
            crate::visual::Decal::Scorch { x, y, fade } => {
                (x, y, scale((70, 46, 28), fade as f32 / 255.0))
            }
        };
        if visible(ctx, x, y) {
            sprites.push(Billboard::at(x, y, BillboardKind::Stain(stain), 0.32));
        }
    }
    collect_effect_billboards(ctx, &mut sprites);
    sprites
}

fn collect_effect_billboards(ctx: &RenderCtx<'_>, sprites: &mut Vec<Billboard>) {
    for effect in &ctx.visual.effects {
        let anchored = match effect {
            VisualEffect::AttackFlash {
                x,
                y,
                timer,
                flash_color,
            } => Some((
                *x,
                *y,
                EffectArt::Attack {
                    color: *flash_color,
                    timer: *timer,
                    material: impact_material(ctx, *x, *y),
                },
                0.62,
            )),
            VisualEffect::WeaponSweep {
                x,
                y,
                dx,
                dy,
                weapon,
                counter,
                timer,
            } => Some((
                *x,
                *y,
                EffectArt::WeaponSweep {
                    weapon: *weapon,
                    dx: *dx,
                    dy: *dy,
                    counter: *counter,
                    timer: *timer,
                },
                0.72,
            )),
            VisualEffect::TechniqueCast {
                x,
                y,
                dx,
                dy,
                kind,
                timer,
            } => Some((
                *x,
                *y,
                EffectArt::Technique {
                    kind: *kind,
                    dx: *dx,
                    dy: *dy,
                    impact: false,
                    timer: *timer,
                },
                0.62,
            )),
            VisualEffect::TechniqueImpact {
                x,
                y,
                dx,
                dy,
                kind,
                timer,
            } => Some((
                *x,
                *y,
                EffectArt::Technique {
                    kind: *kind,
                    dx: *dx,
                    dy: *dy,
                    impact: true,
                    timer: *timer,
                },
                0.66,
            )),
            VisualEffect::ItemCast { x, y, kind, timer } => Some((
                *x,
                *y,
                EffectArt::ItemCast {
                    kind: *kind,
                    timer: *timer,
                },
                0.62,
            )),
            VisualEffect::MovementTrail {
                x,
                y,
                dx,
                dy,
                dash,
                timer,
            } => Some((
                *x,
                *y,
                EffectArt::Movement {
                    dx: *dx,
                    dy: *dy,
                    dash: *dash,
                    timer: *timer,
                },
                if *dash { 0.66 } else { 0.48 },
            )),
            VisualEffect::MonsterCast {
                x,
                y,
                kind,
                dx,
                dy,
                timer,
            } => Some((
                *x,
                *y,
                EffectArt::MonsterCast {
                    kind: *kind,
                    dx: *dx,
                    dy: *dy,
                    timer: *timer,
                },
                0.64,
            )),
            VisualEffect::DamageNumber {
                x,
                y,
                amount,
                color,
                timer,
            } => Some((
                *x,
                *y,
                EffectArt::Damage {
                    amount: *amount,
                    color: *color,
                    timer: *timer,
                },
                if *amount >= 10 { 0.72 } else { 0.56 },
            )),
            VisualEffect::DeathBurst { x, y, kind, timer } => Some((
                *x,
                *y,
                EffectArt::Death {
                    timer: *timer,
                    kind: *kind,
                },
                0.72,
            )),
            VisualEffect::ProjectileLaunch {
                x,
                y,
                dx,
                dy,
                timer,
            } => Some((
                *x,
                *y,
                EffectArt::Launch {
                    dx: *dx,
                    dy: *dy,
                    timer: *timer,
                },
                0.54,
            )),
            VisualEffect::PoisonPulse { x, y, timer } => {
                Some((*x, *y, EffectArt::Poison { timer: *timer }, 0.62))
            }
            VisualEffect::HealGlow { x, y, timer } => {
                Some((*x, *y, EffectArt::Heal { timer: *timer }, 0.7))
            }
            VisualEffect::CrownSparkle { x, y, timer }
            | VisualEffect::PickupSparkle { x, y, timer } => {
                Some((*x, *y, EffectArt::Sparkle { timer: *timer }, 0.56))
            }
            _ => None,
        };
        if let Some((x, y, effect_art, scale)) = anchored {
            if visible(ctx, x, y) {
                sprites.push(Billboard::at(
                    x,
                    y,
                    BillboardKind::Effect(effect_art),
                    scale,
                ));
            }
        }
        if let VisualEffect::FireRing {
            x,
            y,
            radius,
            timer,
        } = effect
        {
            let radius = radius.round() as i32;
            let diagonal = (radius as f32 * 0.7).round() as i32;
            for (dx, dy) in [
                (-radius, 0),
                (radius, 0),
                (0, -radius),
                (0, radius),
                (-diagonal, -diagonal),
                (diagonal, -diagonal),
                (-diagonal, diagonal),
                (diagonal, diagonal),
            ] {
                if visible(ctx, x + dx, y + dy) {
                    sprites.push(Billboard::at(
                        x + dx,
                        y + dy,
                        BillboardKind::Effect(EffectArt::Fire { timer: *timer }),
                        0.5,
                    ));
                }
            }
        }
        if let VisualEffect::ConfusionSwirl { xs, ys, timer } = effect {
            for (&x, &y) in xs.iter().zip(ys) {
                if visible(ctx, x, y) {
                    sprites.push(Billboard::at(
                        x,
                        y,
                        BillboardKind::Effect(EffectArt::Confusion { timer: *timer }),
                        0.5,
                    ));
                }
            }
        }
    }
}

fn impact_material(ctx: &RenderCtx<'_>, x: i32, y: i32) -> ImpactMaterial {
    if let Some(monster) = ctx
        .monsters
        .iter()
        .find(|monster| monster.alive && (monster.x, monster.y) == (x, y))
    {
        return monster_impact_material(monster.kind);
    }
    if ctx.map.in_bounds(x, y) {
        match ctx.map.tiles[ctx.map.idx(x, y)] {
            Tile::ShallowWater => return ImpactMaterial::Water,
            Tile::Web => return ImpactMaterial::Web,
            _ => {}
        }
    }
    ImpactMaterial::Flesh
}

fn monster_impact_material(kind: MonsterKind) -> ImpactMaterial {
    match kind {
        MonsterKind::Skeleton => ImpactMaterial::Bone,
        MonsterKind::Sentinel | MonsterKind::Guardian => ImpactMaterial::Metal,
        MonsterKind::Slime => ImpactMaterial::Slime,
        MonsterKind::Wraith => ImpactMaterial::Spectral,
        _ => ImpactMaterial::Flesh,
    }
}

impl Billboard {
    fn at(x: i32, y: i32, kind: BillboardKind, scale: f64) -> Self {
        Self::at_world(x as f64 + 0.5, y as f64 + 0.5, kind, scale)
    }

    fn at_world(world_x: f64, world_y: f64, kind: BillboardKind, scale: f64) -> Self {
        Self {
            world_x,
            world_y,
            kind,
            scale,
        }
    }
}

fn actor_step_progress(timer: u8) -> f64 {
    let linear = f64::from(6_u8.saturating_sub(timer)) / 5.0;
    let clamped = linear.clamp(0.0, 1.0);
    clamped * clamped * (3.0 - 2.0 * clamped)
}

fn project(
    canvas: &HalfBlockCanvas,
    camera: Camera,
    horizon: i32,
    billboard: Billboard,
) -> Option<Projection> {
    let relative_x = billboard.world_x - camera.x;
    let relative_y = billboard.world_y - camera.y;
    let determinant = camera.plane_x * camera.dir_y - camera.dir_x * camera.plane_y;
    let inverse = 1.0 / determinant;
    let lateral = inverse * (camera.dir_y * relative_x - camera.dir_x * relative_y);
    let depth = inverse * (-camera.plane_y * relative_x + camera.plane_x * relative_y);
    if !(0.12..MAX_VIEW_DEPTH).contains(&depth) {
        return None;
    }
    let width = canvas.width() as f64;
    let height = canvas.pixel_height() as f64;
    let center_x = (width * 0.5 * (1.0 + lateral / depth)).round() as i32;
    let base_size = height * 0.72 / depth;
    let pixel_size = (base_size * billboard.scale)
        .round()
        .clamp(2.0, height * 0.9) as i32;
    let bottom_y = horizon + (base_size * 0.5).round() as i32;
    if center_x + pixel_size < 0 || center_x - pixel_size >= width as i32 {
        return None;
    }
    Some(Projection {
        center_x,
        bottom_y,
        pixel_size,
        depth,
    })
}

fn draw_billboard(
    canvas: &mut HalfBlockCanvas,
    ctx: &RenderCtx<'_>,
    kind: BillboardKind,
    projection: Projection,
    wall_depth: &[f64],
    quality: RenderQuality,
) {
    if let BillboardKind::Item(item, phase) = kind {
        let appearance = crate::item_art::appearance(item, ctx.player.knows_item(item));
        crate::item_art::draw_billboard_item(
            canvas,
            projection.center_x,
            projection.bottom_y,
            projection.pixel_size,
            appearance,
            phase,
            ctx.color_filter,
            projection.depth,
            wall_depth,
        );
        return;
    }
    if let BillboardKind::Effect(effect) = kind {
        draw_effect_art(canvas, ctx, effect, projection, wall_depth);
        return;
    }
    if matches!(
        kind,
        BillboardKind::Feature(Tile::StairsDown | Tile::StairsUp)
    ) {
        draw_ground_shadow(canvas, projection, wall_depth, 0.88);
        draw_stair_planes(canvas, ctx, kind, projection, wall_depth);
        return;
    }
    if matches!(
        kind,
        BillboardKind::Monster(_)
            | BillboardKind::Npc(_, _, _, _)
            | BillboardKind::Prop(_, _)
            | BillboardKind::Landmark(_, _)
            | BillboardKind::Camp(_, _)
            | BillboardKind::DeathEcho(_)
            | BillboardKind::Feature(_)
    ) {
        draw_ground_shadow(canvas, projection, wall_depth, volume_profile(kind).shadow);
    }
    let monster_pose = match kind {
        BillboardKind::Monster(monster) => Some(monster.pose),
        _ => None,
    };
    let npc_talking = match kind {
        BillboardKind::Npc(_, _, _, talking) => talking,
        _ => false,
    };
    let mut projection = projection;
    let (pattern, primary, secondary): (&[&str], Rgb, Rgb) = match kind {
        BillboardKind::Monster(monster) => {
            let frame = monster_frame(monster.kind, monster.pose, monster.phase);
            let base = monster_color(monster.kind);
            let body = if monster.pose == MonsterPose::Hit {
                visual::lerp(base, (255, 232, 180), 0.66)
            } else if monster.enraged {
                visual::lerp(base, (255, 154, 42), 0.48)
            } else if monster.poisoned {
                visual::lerp(base, (84, 224, 92), 0.38)
            } else {
                base
            };
            match monster.pose {
                MonsterPose::Step => {
                    projection.bottom_y -= (monster.phase % 2) as i32;
                    projection.center_x += (monster.step.0 - monster.step.1).signum();
                }
                MonsterPose::Attack => projection.pixel_size = projection.pixel_size * 11 / 10,
                MonsterPose::Recover => {
                    projection.pixel_size = projection.pixel_size * 9 / 10;
                    projection.center_x -= 1;
                }
                MonsterPose::Hit => projection.center_x += 1,
                MonsterPose::Stagger => projection.center_x -= 1,
                _ => {}
            }
            if monster.enraged {
                projection.pixel_size = projection.pixel_size * 11 / 10;
                projection.bottom_y -= i32::from(monster.phase % 8 < 4);
            }
            let accent = if monster.enraged {
                (255, 202, 74)
            } else if monster.pose == MonsterPose::Windup {
                (255, 176, 54)
            } else {
                (238, 66, 46)
            };
            (frame.rows, body, accent)
        }
        BillboardKind::Npc(kind, phase, talked, talking) => {
            projection.bottom_y -= i32::from(phase >= 60);
            let art = npc_art(kind, phase, talked, talking);
            (art.rows, art.primary, art.accent)
        }
        BillboardKind::Prop(kind, phase) => {
            let (pattern, primary, accent) = prop_art(kind, phase);
            (pattern, primary, accent)
        }
        BillboardKind::Projectile(dx, dy, phase) => {
            let pattern: &[&str] = if dx == 0 {
                &["  *  ", "  #  ", "  #  ", "  *  "]
            } else if dy == 0 {
                &["*##*", " ## "]
            } else if dx.signum() == dy.signum() {
                &["   *", "  # ", " #  ", "*   "]
            } else {
                &["*   ", " #  ", "  # ", "   *"]
            };
            projection.center_x += (phase % 2) as i32;
            (pattern, (172, 76, 224), (248, 226, 255))
        }
        BillboardKind::Landmark(landmark, used) => {
            let (pattern, active_base, active_accent) = landmark_art(landmark, ctx.tick);
            let base = if used { (74, 70, 68) } else { active_base };
            let accent = if used { (112, 106, 96) } else { active_accent };
            (pattern, base, accent)
        }
        BillboardKind::Camp(kind, rested) => {
            let (pattern, active_base, active_accent) = camp_art(kind, ctx.tick);
            let base = if rested { (70, 68, 64) } else { active_base };
            let accent = if rested { (108, 94, 80) } else { active_accent };
            (pattern, base, accent)
        }
        BillboardKind::DeathEcho(phase) => {
            projection.bottom_y -= i32::from(phase >= 24);
            (
                &[
                    "  ...  ", " .***. ", " .*#*. ", "..###..", " .###. ", " .#.#. ", ".     .",
                ],
                (104, 154, 184),
                (196, 226, 232),
            )
        }
        BillboardKind::Feature(tile) => match tile {
            Tile::StairsDown => (&["#####", " .#. ", "  #  "], (72, 54, 104), (184, 156, 224)),
            Tile::StairsUp => (&["  #  ", " .#. ", "#####"], (134, 98, 56), (242, 208, 142)),
            Tile::LanternLit => (
                if ctx.tick % 8 < 4 {
                    &["  *  ", " .#. ", "  #  "]
                } else {
                    &["  .  ", "  *  ", " .#. ", "  #  "]
                },
                (188, 116, 38),
                (255, 224, 112),
            ),
            Tile::LanternDead => (&["  .  ", " .#. ", "  #  "], (66, 74, 84), (126, 138, 148)),
            Tile::TrapRevealed => (&["#   #", " # # ", "  *  "], (132, 48, 40), (232, 152, 72)),
            Tile::Urn => (&[" .#. ", " ### ", " .#. "], (112, 72, 44), (210, 154, 92)),
            Tile::DoorOpen => (
                &["#    ", "#... ", "#... ", "#    "],
                (104, 64, 34),
                (188, 126, 62),
            ),
            Tile::Inscription => (
                &[" .#. ", "#####", "#*.*#", "#.*.#", "#####"],
                (82, 122, 138),
                (174, 232, 244),
            ),
            Tile::History(mark) => (
                if mark as u32 % 2 == 0 {
                    &["  *  ", " .#. ", ".###.", "#.*.#", "#####"]
                } else {
                    &[" * * ", " .#. ", ".###.", "#*..#", "#####"]
                },
                (102, 92, 76),
                (198, 188, 146),
            ),
            _ => unreachable!(),
        },
        BillboardKind::Stain(stain) => (
            &[" ### ", "#####", " ### "],
            stain,
            visual::darken(stain, 0.5),
        ),
        BillboardKind::Effect(_) => unreachable!(),
        BillboardKind::Item(_, _) => unreachable!(),
    };
    let tertiary = match kind {
        BillboardKind::Npc(kind, phase, talked, talking) => {
            Some(npc_art(kind, phase, talked, talking).skin)
        }
        _ => None,
    };
    draw_volume_pattern(
        canvas,
        pattern,
        projection,
        visual::grade_art(ctx.color_filter, primary),
        visual::grade_art(ctx.color_filter, secondary),
        wall_depth,
        volume_profile(kind),
        quality,
        tertiary.map(|rgb| visual::grade_art(ctx.color_filter, rgb)),
    );
    if let Some(pose) = monster_pose {
        draw_monster_pose_cue(canvas, ctx, pose, projection, wall_depth);
    }
    if npc_talking {
        draw_npc_talk_cue(canvas, ctx, projection, wall_depth);
    }
}

fn draw_npc_talk_cue(
    canvas: &mut HalfBlockCanvas,
    ctx: &RenderCtx<'_>,
    projection: Projection,
    wall_depth: &[f64],
) {
    let unit = (projection.pixel_size / 10).max(1);
    let cx = projection.center_x;
    let top = projection.bottom_y - projection.pixel_size;
    let rgb = visual::grade_art(ctx.color_filter, (244, 224, 156));
    let phase = if ctx.low_motion {
        0
    } else {
        (ctx.tick / 3) as i32 % 3
    };
    for (index, (offset_x, offset_y)) in [(-2, -1), (0, -3), (2, -1)].into_iter().enumerate() {
        if index as i32 <= phase {
            effect_pixel(
                canvas,
                cx + offset_x * unit,
                top + offset_y * unit,
                rgb,
                projection,
                wall_depth,
            );
        }
    }
}

fn draw_monster_pose_cue(
    canvas: &mut HalfBlockCanvas,
    ctx: &RenderCtx<'_>,
    pose: MonsterPose,
    projection: Projection,
    wall_depth: &[f64],
) {
    let unit = (projection.pixel_size / 9).max(1);
    let cx = projection.center_x;
    let top = projection.bottom_y - projection.pixel_size;
    match pose {
        MonsterPose::Windup => {
            let tell = visual::grade_art(ctx.color_filter, (255, 180, 54));
            for (dx, dy) in [(-4, 2), (-3, 0), (-1, -1), (1, -1), (3, 0), (4, 2)] {
                effect_pixel(
                    canvas,
                    cx + dx * unit,
                    top + dy * unit,
                    tell,
                    projection,
                    wall_depth,
                );
            }
        }
        MonsterPose::Stagger => {
            let broken = visual::grade_art(ctx.color_filter, (236, 226, 174));
            for side in [-1, 1] {
                effect_pixel(
                    canvas,
                    cx + side * 4 * unit,
                    top + 2 * unit,
                    broken,
                    projection,
                    wall_depth,
                );
                effect_pixel(
                    canvas,
                    cx + side * 3 * unit,
                    top + 3 * unit,
                    broken,
                    projection,
                    wall_depth,
                );
            }
        }
        MonsterPose::Recover => {
            let recovery = visual::grade_art(ctx.color_filter, (142, 76, 64));
            for step in 0..4 {
                effect_pixel(
                    canvas,
                    cx + (2 + step) * unit,
                    top + (4 + step) * unit,
                    recovery,
                    projection,
                    wall_depth,
                );
            }
        }
        _ => {}
    }
}

fn draw_stair_planes(
    canvas: &mut HalfBlockCanvas,
    ctx: &RenderCtx<'_>,
    kind: BillboardKind,
    projection: Projection,
    wall_depth: &[f64],
) {
    let BillboardKind::Feature(tile) = kind else {
        return;
    };
    let unit = (projection.pixel_size / 11).max(1);
    let base = match tile {
        Tile::StairsUp => visual::grade_art(ctx.color_filter, (166, 130, 78)),
        Tile::StairsDown => visual::grade_art(ctx.color_filter, (72, 58, 94)),
        _ => return,
    };
    let edge = visual::lerp(base, (244, 220, 164), 0.48);
    let cx = projection.center_x;
    let floor_y = projection.bottom_y - unit;
    for step in 0..5 {
        let (half_width, y) = if tile == Tile::StairsUp {
            (5 - step, floor_y - step * 2 * unit)
        } else {
            (5 - step / 2, floor_y - step * unit)
        };
        let step_color = if tile == Tile::StairsDown && step >= 3 {
            visual::darken(base, 0.44)
        } else {
            visual::darken(base, 1.0 - step as f32 * 0.08)
        };
        for x in (cx - half_width * unit)..=(cx + half_width * unit) {
            effect_pixel(canvas, x, y, step_color, projection, wall_depth);
        }
        for x in (cx - half_width * unit)..=(cx + half_width * unit) {
            effect_pixel(canvas, x, y - 1, edge, projection, wall_depth);
        }
    }
    if tile == Tile::StairsDown {
        for y in (floor_y - 6 * unit)..=(floor_y - 4 * unit) {
            for x in (cx - 3 * unit)..=(cx + 3 * unit) {
                effect_pixel(canvas, x, y, (10, 10, 18), projection, wall_depth);
            }
        }
    }
}

fn draw_ground_shadow(
    canvas: &mut HalfBlockCanvas,
    projection: Projection,
    wall_depth: &[f64],
    strength: f32,
) {
    let half_width = (projection.pixel_size / 3).max(1);
    let shadow_height = (projection.pixel_size / 12).clamp(1, 3);
    let center = projection.center_x;
    for y in 0..shadow_height {
        let inset = y;
        for x in (center - half_width + inset)..=(center + half_width - inset) {
            if x >= 0
                && wall_depth
                    .get(x as usize)
                    .is_some_and(|wall| projection.depth < *wall)
            {
                canvas.tint_pixel(
                    x,
                    projection.bottom_y - shadow_height + y + 1,
                    (4, 5, 7),
                    0.52 * strength.clamp(0.0, 1.4),
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_volume_pattern(
    canvas: &mut HalfBlockCanvas,
    pattern: &[&str],
    projection: Projection,
    primary: Rgb,
    secondary: Rgb,
    wall_depth: &[f64],
    profile: VolumeProfile,
    quality: RenderQuality,
    tertiary: Option<Rgb>,
) {
    let columns = pattern.first().map_or(1, |row| row.chars().count()) as i32;
    let rows = pattern.len() as i32;
    let scale = (projection.pixel_size / rows.max(columns)).max(1);
    let width = columns * scale;
    let height = rows * scale;
    let left = projection.center_x - width / 2;
    let top = projection.bottom_y - height - profile.lift;
    let thickness = profile.extrusion * (scale / 2).max(1);
    for (row, pixels) in pattern.iter().enumerate() {
        for (column, pixel) in pixels.chars().enumerate() {
            let rgb = match pixel {
                '#' => Some(primary),
                '*' | '+' => Some(secondary),
                '@' => Some(tertiary.unwrap_or(secondary)),
                '.' => Some(visual::darken(primary, 0.58)),
                _ => None,
            };
            let Some(rgb) = rgb else { continue };
            for layer in (1..=quality.volume_layers).rev() {
                let offset = thickness * layer / quality.volume_layers.max(1);
                let x = left + column as i32 * scale + offset;
                let y = top + row as i32 * scale + offset / 2;
                let side = visual::darken(rgb, 0.42 + 0.10 * layer as f32);
                for py in 0..scale {
                    for px in 0..scale {
                        let projected_x = x + px;
                        if projected_x >= 0
                            && wall_depth
                                .get(projected_x as usize)
                                .is_some_and(|wall| projection.depth < *wall)
                        {
                            canvas.pixel(projected_x, y + py, color(side));
                        }
                    }
                }
            }
            for py in 0..scale {
                for px in 0..scale {
                    let x = left + column as i32 * scale + px;
                    if x >= 0
                        && wall_depth
                            .get(x as usize)
                            .is_some_and(|wall| projection.depth < *wall)
                    {
                        // Sculpt each source pixel as a tiny lit plane. At
                        // close range this gives every monster and NPC a
                        // consistent upper-left key light and lower-right
                        // falloff without changing its authored silhouette.
                        let sculpted = if scale <= 1 || pixel == '.' || !quality.fine_detail {
                            rgb
                        } else {
                            let denominator = (2 * (scale - 1)).max(1) as f32;
                            let plane = (px + py) as f32 / denominator;
                            let factor = 1.10 - 0.22 * plane;
                            (
                                (rgb.0 as f32 * factor).round().clamp(0.0, 255.0) as u8,
                                (rgb.1 as f32 * factor).round().clamp(0.0, 255.0) as u8,
                                (rgb.2 as f32 * factor).round().clamp(0.0, 255.0) as u8,
                            )
                        };
                        canvas.pixel(x, top + row as i32 * scale + py, color(sculpted));
                    }
                }
            }
        }
    }
}

fn volume_profile(kind: BillboardKind) -> VolumeProfile {
    match kind {
        BillboardKind::Monster(monster) => match monster.kind {
            MonsterKind::Bat => VolumeProfile {
                extrusion: 1,
                lift: 2,
                shadow: 0.62,
            },
            MonsterKind::Wraith => VolumeProfile {
                extrusion: 1,
                lift: 1,
                shadow: 0.52,
            },
            MonsterKind::Guardian => VolumeProfile {
                extrusion: 2,
                lift: 0,
                shadow: 1.0,
            },
            MonsterKind::Slime => VolumeProfile {
                extrusion: 2,
                lift: 0,
                shadow: 0.88,
            },
            _ => VolumeProfile {
                extrusion: 1,
                lift: 0,
                shadow: 0.82,
            },
        },
        BillboardKind::Npc(kind, _, _, _) => match kind {
            NpcKind::WoundedDelver | NpcKind::LostDelver => VolumeProfile {
                extrusion: 1,
                lift: 0,
                shadow: 0.72,
            },
            NpcKind::LanternTender | NpcKind::ShrineKeeper => VolumeProfile {
                extrusion: 1,
                lift: 1,
                shadow: 0.76,
            },
            _ => VolumeProfile {
                extrusion: 1,
                lift: 0,
                shadow: 0.8,
            },
        },
        BillboardKind::Landmark(_, _) => VolumeProfile {
            extrusion: 2,
            lift: 0,
            shadow: 0.9,
        },
        BillboardKind::Camp(_, _) => VolumeProfile {
            extrusion: 1,
            lift: 0,
            shadow: 0.86,
        },
        BillboardKind::Prop(kind, _) => match kind {
            PropKind::Tree | PropKind::Gate | PropKind::Cart => VolumeProfile {
                extrusion: 1,
                lift: 1,
                shadow: 0.84,
            },
            PropKind::Torch | PropKind::Brazier => VolumeProfile {
                extrusion: 1,
                lift: 1,
                shadow: 0.54,
            },
            _ => VolumeProfile {
                extrusion: 1,
                lift: 0,
                shadow: 0.72,
            },
        },
        BillboardKind::DeathEcho(_) => VolumeProfile {
            extrusion: 1,
            lift: 1,
            shadow: 0.45,
        },
        BillboardKind::Feature(Tile::LanternLit) => VolumeProfile {
            extrusion: 1,
            lift: 1,
            shadow: 0.5,
        },
        BillboardKind::Feature(_) => VolumeProfile {
            extrusion: 1,
            lift: 0,
            shadow: 0.72,
        },
        BillboardKind::Projectile(_, _, _) | BillboardKind::Stain(_) => VolumeProfile {
            extrusion: 0,
            lift: 0,
            shadow: 0.0,
        },
        BillboardKind::Effect(_) | BillboardKind::Item(_, _) => VolumeProfile {
            extrusion: 0,
            lift: 0,
            shadow: 0.0,
        },
    }
}

fn prop_scale(kind: PropKind) -> f64 {
    match kind {
        PropKind::Tree | PropKind::Gate => 0.92,
        PropKind::Cart => 0.76,
        PropKind::Brazier | PropKind::Torch | PropKind::Banner => 0.62,
        PropKind::Shrine => 0.68,
        _ => 0.52,
    }
}

fn prop_art(kind: PropKind, phase: u64) -> (&'static [&'static str], Rgb, Rgb) {
    let flame = phase % 8 < 4;
    match kind {
        PropKind::RoadMarker => (
            &["  *  ", "  #  ", "  #  ", " ### "],
            (126, 92, 56),
            (222, 184, 104),
        ),
        PropKind::Signpost => (
            &["  +  ", "#####", "  #  ", "  #  "],
            (116, 78, 44),
            (218, 178, 96),
        ),
        PropKind::Brazier => (
            if flame {
                &["  *  ", " *** ", " ### ", "#####"]
            } else {
                &["  .  ", " *** ", " ### ", "#####"]
            },
            (104, 70, 46),
            (246, 166, 58),
        ),
        PropKind::Torch => (
            if flame {
                &[" * ", " # ", " # ", "###"]
            } else {
                &[" . ", " # ", " # ", "###"]
            },
            (108, 70, 42),
            (250, 184, 74),
        ),
        PropKind::Gate => (
            &["## ##", "## ##", "#####", "## ##"],
            (94, 76, 58),
            (196, 154, 84),
        ),
        PropKind::Rubble => (&[" . . ", ".###.", "#####"], (96, 88, 76), (164, 142, 108)),
        PropKind::Tree => (
            &["  *  ", " *** ", "*****", "  #  ", " ### "],
            (46, 92, 58),
            (144, 122, 72),
        ),
        PropKind::Cart => (
            &[" ###  ", "##### ", "#  ## ", "  o o "],
            (114, 76, 44),
            (210, 160, 86),
        ),
        PropKind::Barrel => (
            &[" .#. ", "#####", "#####", " .#. "],
            (112, 76, 46),
            (198, 142, 76),
        ),
        PropKind::Crate => (&["#####", "#/#/#", "#####"], (118, 82, 48), (196, 150, 82)),
        PropKind::Banner => (
            &["  #  ", " *** ", " ### ", "  #  "],
            (104, 54, 46),
            (216, 166, 88),
        ),
        PropKind::Shrine => (
            &["  +  ", " +#+ ", "#####", "  #  "],
            (102, 78, 54),
            (222, 190, 112),
        ),
    }
}

fn draw_effect_art(
    canvas: &mut HalfBlockCanvas,
    ctx: &RenderCtx<'_>,
    effect: EffectArt,
    projection: Projection,
    wall_depth: &[f64],
) {
    let unit = (projection.pixel_size / 7).max(1);
    let cx = projection.center_x;
    let cy = projection.bottom_y - projection.pixel_size / 2;
    match effect {
        EffectArt::Attack {
            color: rgb,
            timer,
            material,
        } => {
            let rgb = visual::grade_art(
                ctx.color_filter,
                match material {
                    ImpactMaterial::Flesh => rgb,
                    ImpactMaterial::Bone => (232, 222, 184),
                    ImpactMaterial::Metal => (255, 198, 72),
                    ImpactMaterial::Slime => (108, 232, 92),
                    ImpactMaterial::Spectral => (126, 220, 255),
                    ImpactMaterial::Water => (112, 202, 232),
                    ImpactMaterial::Web => (216, 222, 210),
                },
            );
            let reach = 3 + i32::from(timer <= 3);
            for step in -reach..=reach {
                effect_pixel(
                    canvas,
                    cx + step * unit,
                    cy - step * unit + i32::from(timer >= 5),
                    rgb,
                    projection,
                    wall_depth,
                );
            }
            let fragments: &[(i32, i32)] = match material {
                ImpactMaterial::Flesh => &[(-3, 2), (2, 3), (4, 1)],
                ImpactMaterial::Bone => &[(-4, -2), (-2, 3), (3, -3), (4, 2)],
                ImpactMaterial::Metal => &[(-4, 0), (4, 0), (0, -4), (0, 4)],
                ImpactMaterial::Slime => &[(-3, 3), (0, 4), (3, 3)],
                ImpactMaterial::Spectral => &[(-3, -3), (0, -5), (3, -3)],
                ImpactMaterial::Water => &[(-4, 2), (-2, 1), (2, 1), (4, 2)],
                ImpactMaterial::Web => &[(-4, -3), (-4, 3), (4, -3), (4, 3)],
            };
            for &(dx, dy) in fragments {
                effect_pixel(
                    canvas,
                    cx + dx * unit,
                    cy + dy * unit,
                    visual::darken(rgb, 0.78),
                    projection,
                    wall_depth,
                );
            }
        }
        EffectArt::WeaponSweep {
            weapon,
            dx,
            dy,
            counter,
            timer,
        } => draw_weapon_sweep_art(
            canvas, ctx, projection, wall_depth, weapon, dx, dy, counter, timer,
        ),
        EffectArt::Technique {
            kind,
            dx,
            dy,
            impact,
            timer,
        } => draw_technique_art(
            canvas, ctx, projection, wall_depth, kind, dx, dy, impact, timer,
        ),
        EffectArt::ItemCast { kind, timer } => {
            draw_item_cast_art(canvas, ctx, projection, wall_depth, kind, timer)
        }
        EffectArt::Movement {
            dx,
            dy,
            dash,
            timer,
        } => draw_movement_art(canvas, ctx, projection, wall_depth, dx, dy, dash, timer),
        EffectArt::MonsterCast {
            kind,
            dx,
            dy,
            timer,
        } => draw_monster_cast_art(canvas, ctx, projection, wall_depth, kind, dx, dy, timer),
        EffectArt::Damage {
            amount,
            color: rgb,
            timer,
        } => {
            let rise = i32::from(12u8.saturating_sub(timer)) / 3;
            draw_damage_digits(
                canvas,
                amount,
                cx,
                cy - projection.pixel_size / 2 - rise,
                visual::grade_art(ctx.color_filter, rgb),
                projection,
                wall_depth,
            );
        }
        EffectArt::Death { timer, kind } => {
            let progress = i32::from(10u8.saturating_sub(timer)).min(5);
            let (rgb, fragments): (Rgb, &[(i32, i32)]) = match kind {
                Some(MonsterKind::Skeleton) => (
                    (224, 216, 180),
                    &[(-3, -1), (3, -1), (-2, -4), (2, -3), (-1, 2), (1, 3)],
                ),
                Some(MonsterKind::Slime) => {
                    ((82, 202, 92), &[(-4, 2), (-2, 3), (0, 4), (2, 3), (4, 2)])
                }
                Some(MonsterKind::Wraith) => (
                    (92, 200, 238),
                    &[(-2, -5), (0, -7), (2, -5), (-3, -3), (3, -3)],
                ),
                Some(MonsterKind::Sentinel | MonsterKind::Guardian) => (
                    (224, 174, 62),
                    &[(-4, -2), (4, -2), (-3, 3), (3, 3), (0, -5)],
                ),
                Some(MonsterKind::Spider) => (
                    (174, 62, 158),
                    &[(-5, -1), (5, -1), (-4, 2), (4, 2), (-2, 4), (2, 4)],
                ),
                Some(MonsterKind::Necromancer) => (
                    (184, 78, 204),
                    &[(-3, -4), (0, -6), (3, -4), (-2, 2), (2, 2)],
                ),
                None => ((154, 104, 58), &[(-3, -2), (3, -2), (-2, 2), (2, 3)]),
                _ => (
                    (198, 62, 44),
                    &[(-2, -1), (2, -1), (-1, -3), (1, -4), (-3, -2), (3, -2)],
                ),
            };
            let rgb = visual::grade_art(ctx.color_filter, rgb);
            for &(dx, dy) in fragments {
                effect_pixel(
                    canvas,
                    cx + dx * progress.max(1),
                    cy + dy * progress.max(1) / 2,
                    rgb,
                    projection,
                    wall_depth,
                );
            }
        }
        EffectArt::Launch { dx, dy, timer } => {
            let outward = i32::from(7u8.saturating_sub(timer)).min(4);
            let core = visual::grade_art(ctx.color_filter, (212, 94, 238));
            let rim = visual::grade_art(ctx.color_filter, (112, 54, 184));
            for step in 0..=3 {
                effect_pixel(
                    canvas,
                    cx + (dx - dy) * (outward + step) * unit,
                    cy + (dx + dy) * (outward + step) * unit / 2,
                    if step == 0 { core } else { rim },
                    projection,
                    wall_depth,
                );
            }
            for (ox, oy) in [(-2, 0), (2, 0), (0, -2), (0, 2)] {
                effect_pixel(
                    canvas,
                    cx + ox * unit,
                    cy + oy * unit,
                    rim,
                    projection,
                    wall_depth,
                );
            }
        }
        EffectArt::Poison { timer } => {
            let rgb = visual::grade_art(ctx.color_filter, (92, 224, 82));
            let phase = i32::from(timer % 4);
            for (dx, dy) in [(-2, 0), (-1, -2), (1, -2), (2, 0), (1, 2), (-1, 2)] {
                effect_pixel(
                    canvas,
                    cx + (dx + phase - 2) * unit,
                    cy + dy * unit,
                    rgb,
                    projection,
                    wall_depth,
                );
            }
        }
        EffectArt::Heal { timer } => {
            let rgb = visual::grade_art(ctx.color_filter, (104, 246, 166));
            let lift = i32::from(8u8.saturating_sub(timer));
            for dx in [-2, 0, 2] {
                for dy in 0..4 {
                    effect_pixel(
                        canvas,
                        cx + dx * unit,
                        cy - (dy * 2 + lift) * unit,
                        rgb,
                        projection,
                        wall_depth,
                    );
                }
            }
        }
        EffectArt::Sparkle { timer } => {
            let rgb = visual::grade_art(ctx.color_filter, (255, 218, 66));
            let radius = 1 + i32::from(timer % 3);
            effect_pixel(canvas, cx, cy, (255, 250, 204), projection, wall_depth);
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                effect_pixel(
                    canvas,
                    cx + dx * radius * unit,
                    cy + dy * radius * unit,
                    rgb,
                    projection,
                    wall_depth,
                );
            }
        }
        EffectArt::Fire { timer } => {
            let outer = visual::grade_art(ctx.color_filter, (242, 72, 24));
            let inner = visual::grade_art(ctx.color_filter, (255, 210, 54));
            let sway = i32::from(timer % 3) - 1;
            for (dx, dy, rgb) in [
                (-1, 0, outer),
                (0, 0, inner),
                (1, 0, outer),
                (0, -1, inner),
                (sway, -2, outer),
            ] {
                effect_pixel(
                    canvas,
                    cx + dx * unit,
                    cy + dy * unit,
                    rgb,
                    projection,
                    wall_depth,
                );
            }
        }
        EffectArt::Confusion { timer } => {
            let colors = [
                visual::grade_art(ctx.color_filter, (174, 92, 226)),
                visual::grade_art(ctx.color_filter, (76, 190, 232)),
            ];
            let rotate = i32::from(timer % 4);
            for (index, (dx, dy)) in [(-2, 0), (0, -2), (2, 0), (0, 2)].into_iter().enumerate() {
                let (dx, dy) = match rotate {
                    1 => (-dy, dx),
                    2 => (-dx, -dy),
                    3 => (dy, -dx),
                    _ => (dx, dy),
                };
                effect_pixel(
                    canvas,
                    cx + dx * unit,
                    cy + dy * unit,
                    colors[index % 2],
                    projection,
                    wall_depth,
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_weapon_sweep_art(
    canvas: &mut HalfBlockCanvas,
    ctx: &RenderCtx<'_>,
    projection: Projection,
    wall_depth: &[f64],
    weapon: Option<ItemKind>,
    dx: i32,
    dy: i32,
    counter: bool,
    timer: u8,
) {
    let unit = (projection.pixel_size / 8).max(1);
    let cx = projection.center_x;
    let cy = projection.bottom_y - projection.pixel_size / 2;
    let progress = if ctx.low_motion {
        8
    } else {
        12_u8.saturating_sub(timer).max(1)
    };
    let (base, points): (Rgb, &[(i32, i32)]) = match weapon {
        Some(ItemKind::Warhammer) => (
            (226, 178, 74),
            &[(-3, -2), (-2, 0), (0, 2), (2, 0), (3, -2)],
        ),
        Some(ItemKind::Emberbrand) => (
            (248, 82, 30),
            &[(-4, 2), (-3, 0), (-1, -2), (1, -3), (3, -1), (4, 1)],
        ),
        Some(ItemKind::Longsword) => (
            (220, 236, 230),
            &[(-4, 3), (-3, 1), (-2, -1), (0, -3), (2, -4), (4, -2)],
        ),
        Some(ItemKind::RustySword) => (
            (194, 150, 92),
            &[(-3, 3), (-3, 1), (-2, -1), (0, -2), (2, -1), (3, 1)],
        ),
        _ => (
            (218, 190, 126),
            &[(-2, 2), (-1, 0), (0, -2), (1, 0), (2, 2)],
        ),
    };
    let base = visual::grade_art(ctx.color_filter, base);
    let edge = visual::grade_art(
        ctx.color_filter,
        if counter {
            (255, 246, 160)
        } else {
            visual::lerp(base, (255, 246, 184), 0.46)
        },
    );
    let side = if dy != 0 {
        dy.signum()
    } else if dx != 0 {
        dx.signum()
    } else {
        1
    };
    let count = ((usize::from(progress) + 2) / 2).min(points.len());
    for (index, &(offset_x, offset_y)) in points.iter().take(count).enumerate() {
        let rgb = if index + 1 == count { edge } else { base };
        effect_pixel(
            canvas,
            cx + offset_x * side * unit,
            cy + offset_y * unit,
            rgb,
            projection,
            wall_depth,
        );
    }
    if timer <= 3 || ctx.low_motion {
        for (offset_x, offset_y) in [(-2, 0), (0, -1), (2, 0), (0, 1)] {
            effect_pixel(
                canvas,
                cx + offset_x * unit,
                cy + offset_y * unit,
                edge,
                projection,
                wall_depth,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_technique_art(
    canvas: &mut HalfBlockCanvas,
    ctx: &RenderCtx<'_>,
    projection: Projection,
    wall_depth: &[f64],
    kind: TechniqueKind,
    dx: i32,
    dy: i32,
    impact: bool,
    timer: u8,
) {
    let unit = (projection.pixel_size / 8).max(1);
    let cx = projection.center_x;
    let cy = projection.bottom_y - projection.pixel_size / 2;
    let (primary, accent) = match kind {
        TechniqueKind::Kick => ((218, 178, 112), (244, 230, 174)),
        TechniqueKind::ShieldBash => ((76, 166, 226), (180, 232, 255)),
        TechniqueKind::Lunge => ((226, 232, 214), (255, 250, 210)),
        TechniqueKind::Sweep => ((206, 226, 190), (244, 250, 206)),
        TechniqueKind::Earthbreak => ((190, 122, 58), (248, 188, 74)),
        TechniqueKind::CinderArc => ((238, 62, 24), (255, 218, 74)),
    };
    let primary = visual::grade_art(ctx.color_filter, primary);
    let accent = visual::grade_art(ctx.color_filter, accent);
    let progress = if impact {
        10_u8.saturating_sub(timer).max(1)
    } else {
        14_u8.saturating_sub(timer).max(1)
    };
    let radius = if ctx.low_motion {
        2
    } else {
        1 + i32::from(progress / 3)
    };
    if !impact {
        effect_pixel(canvas, cx, cy, accent, projection, wall_depth);
        for (index, (offset_x, offset_y)) in [(-2, 0), (-1, -2), (1, -2), (2, 0), (1, 2), (-1, 2)]
            .into_iter()
            .enumerate()
        {
            if index as u8 <= progress.saturating_add(1) {
                effect_pixel(
                    canvas,
                    cx + offset_x * radius * unit,
                    cy + offset_y * radius * unit,
                    if index % 2 == 0 { accent } else { primary },
                    projection,
                    wall_depth,
                );
            }
        }
        let forward = if dx != 0 { dx } else { -dy };
        for step in 1..=i32::from(progress.min(4)) {
            effect_pixel(
                canvas,
                cx + forward * step * unit,
                cy - step * unit / 2,
                primary,
                projection,
                wall_depth,
            );
        }
        return;
    }

    match kind {
        TechniqueKind::Kick => {
            for (offset_x, offset_y) in [(-3, 2), (-2, 1), (0, 0), (2, -1), (3, -2)] {
                effect_pixel(
                    canvas,
                    cx + offset_x * unit,
                    cy + offset_y * unit,
                    primary,
                    projection,
                    wall_depth,
                );
            }
        }
        TechniqueKind::ShieldBash => {
            for (offset_x, offset_y) in [(-2, -2), (0, -3), (2, -2), (-3, 0), (3, 0), (0, 2)] {
                effect_pixel(
                    canvas,
                    cx + offset_x * unit,
                    cy + offset_y * unit,
                    accent,
                    projection,
                    wall_depth,
                );
            }
        }
        TechniqueKind::Lunge => {
            let forward = if dx != 0 { dx } else { -dy };
            for step in 1..=4 {
                effect_pixel(
                    canvas,
                    cx + forward * step * unit,
                    cy - step * unit / 3,
                    if step == 4 { accent } else { primary },
                    projection,
                    wall_depth,
                );
            }
        }
        TechniqueKind::Sweep => {
            for (offset_x, offset_y) in [
                (-4, 1),
                (-3, -1),
                (-2, -2),
                (0, -3),
                (2, -2),
                (3, -1),
                (4, 1),
            ] {
                effect_pixel(
                    canvas,
                    cx + offset_x * unit,
                    cy + offset_y * unit,
                    if offset_x.abs() % 2 == 0 {
                        accent
                    } else {
                        primary
                    },
                    projection,
                    wall_depth,
                );
            }
        }
        TechniqueKind::Earthbreak => {
            for offset_x in [-3, -1, 1, 3] {
                for step in 0..=2 {
                    effect_pixel(
                        canvas,
                        cx + offset_x * unit + step * offset_x.signum() * unit / 2,
                        cy + (step + 1) * unit,
                        if step == 0 { accent } else { primary },
                        projection,
                        wall_depth,
                    );
                }
            }
        }
        TechniqueKind::CinderArc => {
            for (offset_x, offset_y) in [(-3, 2), (-2, 0), (-1, -2), (1, -2), (2, 0), (3, 2)] {
                effect_pixel(
                    canvas,
                    cx + offset_x * unit,
                    cy + offset_y * unit,
                    if offset_y < 0 { accent } else { primary },
                    projection,
                    wall_depth,
                );
            }
        }
    }
}

fn draw_item_cast_art(
    canvas: &mut HalfBlockCanvas,
    ctx: &RenderCtx<'_>,
    projection: Projection,
    wall_depth: &[f64],
    kind: ItemKind,
    timer: u8,
) {
    let unit = (projection.pixel_size / 8).max(1);
    let cx = projection.center_x;
    let cy = projection.bottom_y - projection.pixel_size / 2;
    let pulse = if ctx.low_motion {
        2
    } else {
        1 + i32::from(12_u8.saturating_sub(timer) / 3)
    };
    let (primary, accent) = match kind {
        ItemKind::ScrollOfFire => ((238, 64, 24), (255, 222, 76)),
        ItemKind::ScrollOfConfusion => ((170, 82, 226), (92, 202, 238)),
        ItemKind::ScrollOfBlink => ((76, 204, 232), (198, 252, 255)),
        ItemKind::SmokeBomb => ((126, 132, 148), (218, 224, 232)),
        ItemKind::HealthPotion | ItemKind::LargeHealthPotion | ItemKind::Antidote => {
            ((74, 214, 142), (198, 255, 190))
        }
        ItemKind::LanternOil => ((232, 158, 42), (255, 236, 128)),
        ItemKind::EnchantmentScroll => ((220, 194, 110), (248, 244, 190)),
        ItemKind::LoreFragment => ((82, 168, 218), (190, 238, 255)),
        _ => ((198, 176, 104), (255, 236, 156)),
    };
    let primary = visual::grade_art(ctx.color_filter, primary);
    let accent = visual::grade_art(ctx.color_filter, accent);
    match kind {
        ItemKind::ScrollOfFire | ItemKind::LanternOil => {
            for (offset_x, offset_y, rgb) in [
                (-1, 1, primary),
                (0, -1, accent),
                (1, 1, primary),
                (0, 2, primary),
            ] {
                effect_pixel(
                    canvas,
                    cx + offset_x * pulse * unit,
                    cy + offset_y * unit,
                    rgb,
                    projection,
                    wall_depth,
                );
            }
        }
        ItemKind::ScrollOfConfusion => {
            for (index, (offset_x, offset_y)) in
                [(-2, 0), (0, -2), (2, 0), (0, 2)].into_iter().enumerate()
            {
                effect_pixel(
                    canvas,
                    cx + offset_x * pulse * unit / 2,
                    cy + offset_y * pulse * unit / 2,
                    if index % 2 == 0 { primary } else { accent },
                    projection,
                    wall_depth,
                );
            }
        }
        ItemKind::ScrollOfBlink => {
            for (offset_x, offset_y) in [(0, -2), (2, 0), (0, 2), (-2, 0), (0, 0)] {
                effect_pixel(
                    canvas,
                    cx + offset_x * pulse * unit / 2,
                    cy + offset_y * pulse * unit / 2,
                    accent,
                    projection,
                    wall_depth,
                );
            }
        }
        ItemKind::SmokeBomb => {
            for (offset_x, offset_y) in [(-2, 0), (-1, -1), (0, -2), (1, -1), (2, 0), (0, 1)] {
                effect_pixel(
                    canvas,
                    cx + offset_x * pulse * unit / 2,
                    cy + offset_y * pulse * unit / 2,
                    primary,
                    projection,
                    wall_depth,
                );
            }
        }
        ItemKind::HealthPotion | ItemKind::LargeHealthPotion | ItemKind::Antidote => {
            for (offset_x, offset_y) in [
                (-2, 0),
                (-1, 0),
                (0, 0),
                (1, 0),
                (2, 0),
                (0, -2),
                (0, -1),
                (0, 1),
                (0, 2),
            ] {
                effect_pixel(
                    canvas,
                    cx + offset_x * unit,
                    cy + offset_y * unit,
                    accent,
                    projection,
                    wall_depth,
                );
            }
        }
        ItemKind::EnchantmentScroll | ItemKind::LoreFragment => {
            for (offset_x, offset_y) in [
                (-2, -1),
                (-1, -2),
                (1, -2),
                (2, -1),
                (2, 1),
                (1, 2),
                (-1, 2),
                (-2, 1),
            ] {
                effect_pixel(
                    canvas,
                    cx + offset_x * unit,
                    cy + offset_y * unit,
                    if offset_x.abs() == 2 { primary } else { accent },
                    projection,
                    wall_depth,
                );
            }
        }
        _ => effect_pixel(canvas, cx, cy, accent, projection, wall_depth),
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_movement_art(
    canvas: &mut HalfBlockCanvas,
    ctx: &RenderCtx<'_>,
    projection: Projection,
    wall_depth: &[f64],
    dx: i32,
    dy: i32,
    dash: bool,
    timer: u8,
) {
    let unit = (projection.pixel_size / 9).max(1);
    let cx = projection.center_x;
    let cy = projection.bottom_y - projection.pixel_size / 2;
    let primary = visual::grade_art(
        ctx.color_filter,
        if dash {
            (110, 226, 202)
        } else {
            (186, 164, 112)
        },
    );
    let phase = if ctx.low_motion {
        2
    } else {
        i32::from(timer % 4)
    };
    for index in 0..=if dash { 3 } else { 2 } {
        let distance = index + 1 + phase / 2;
        let offset_x = (dx - dy) * distance;
        let offset_y = (dx + dy) * distance / 2;
        effect_pixel(
            canvas,
            cx - offset_x * unit,
            cy - offset_y * unit,
            if index == 0 {
                primary
            } else {
                visual::darken(primary, 0.62)
            },
            projection,
            wall_depth,
        );
    }
    for (offset_x, offset_y) in [(-2, 1), (0, 2), (2, 1)] {
        effect_pixel(
            canvas,
            cx + offset_x * unit,
            cy + offset_y * unit,
            visual::darken(primary, 0.72),
            projection,
            wall_depth,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_monster_cast_art(
    canvas: &mut HalfBlockCanvas,
    ctx: &RenderCtx<'_>,
    projection: Projection,
    wall_depth: &[f64],
    kind: MonsterKind,
    dx: i32,
    dy: i32,
    timer: u8,
) {
    let unit = (projection.pixel_size / 8).max(1);
    let cx = projection.center_x;
    let cy = projection.bottom_y - projection.pixel_size / 2;
    let (core, rim) = match kind {
        MonsterKind::BellWarden => ((248, 196, 62), (224, 108, 42)),
        MonsterKind::Necromancer => ((220, 102, 246), (106, 54, 188)),
        _ => ((226, 122, 78), (148, 62, 54)),
    };
    let core = visual::grade_art(ctx.color_filter, core);
    let rim = visual::grade_art(ctx.color_filter, rim);
    effect_pixel(canvas, cx, cy, core, projection, wall_depth);
    let outward = if ctx.low_motion {
        2
    } else {
        1 + i32::from(10_u8.saturating_sub(timer) / 3)
    };
    for step in 1..=outward.min(4) {
        effect_pixel(
            canvas,
            cx + (dx - dy) * step * unit,
            cy + (dx + dy) * step * unit / 2,
            rim,
            projection,
            wall_depth,
        );
    }
    for (offset_x, offset_y) in [(-2, 0), (2, 0), (0, -2), (0, 2)] {
        effect_pixel(
            canvas,
            cx + offset_x * unit,
            cy + offset_y * unit,
            rim,
            projection,
            wall_depth,
        );
    }
}

fn draw_damage_digits(
    canvas: &mut HalfBlockCanvas,
    amount: i32,
    center_x: i32,
    top: i32,
    rgb: Rgb,
    projection: Projection,
    wall_depth: &[f64],
) {
    let text = amount.unsigned_abs().min(99).to_string();
    let width = text.len() as i32 * 4 - 1;
    let left = center_x - width / 2;
    for (digit_index, digit) in text.chars().enumerate() {
        let rows = digit_rows(digit);
        for (row, mask) in rows.into_iter().enumerate() {
            for column in 0..3 {
                if mask & (1 << (2 - column)) != 0 {
                    effect_pixel(
                        canvas,
                        left + digit_index as i32 * 4 + column,
                        top + row as i32,
                        rgb,
                        projection,
                        wall_depth,
                    );
                }
            }
        }
    }
}

fn digit_rows(digit: char) -> [u8; 5] {
    match digit {
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b010, 0b010, 0b010],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        _ => [0; 5],
    }
}

fn effect_pixel(
    canvas: &mut HalfBlockCanvas,
    x: i32,
    y: i32,
    rgb: Rgb,
    projection: Projection,
    wall_depth: &[f64],
) {
    if x >= 0
        && wall_depth
            .get(x as usize)
            .is_some_and(|wall| projection.depth < *wall)
    {
        canvas.pixel(x, y, color(rgb));
    }
}

fn draw_player_foreground(canvas: &mut HalfBlockCanvas, ctx: &RenderCtx<'_>) {
    let width = canvas.width() as i32;
    let height = canvas.pixel_height() as i32;
    let frame = player_frame(ctx);
    debug_assert!(frame.anchors_in_bounds());
    let scale = ((height / 3) / frame.sprite.height().max(1) as i32).clamp(1, 2);
    let frame_width = frame.sprite.width() as i32 * scale;
    let bob = ctx.camera_bob;
    let bottom = height + bob;
    let mut left = width / 2 - frame_width / 2 - width / 11 - ctx.camera_sway;
    if ctx.player.poisoned && !ctx.reduced_flash && !ctx.low_motion {
        left += if (ctx.tick / 2) & 1 == 0 { -1 } else { 1 };
    }
    if let Some(action) = ctx.visual.player_action {
        if action.kind == PlayerActionKind::Hit {
            left += match action.phase() {
                0 => -2 * scale,
                1 => scale,
                _ => 0,
            };
        }
    }
    let damaged = ctx.visual.damage_flash_timer > 0;
    let healing = ctx.visual.effects.iter().any(|effect| {
        matches!(effect, VisualEffect::HealGlow { x, y, .. } if (*x, *y) == (ctx.player.x, ctx.player.y))
    });
    let mut cloak = player_cloak(ctx);
    if damaged {
        cloak = visual::lerp(cloak, (246, 54, 42), 0.58);
    } else if healing {
        cloak = visual::lerp(cloak, (96, 242, 154), 0.5);
    }
    cloak = visual::grade_art(ctx.color_filter, cloak);
    let shoulder = visual::lerp(cloak, (226, 214, 176), 0.35);
    let shadow = visual::darken(cloak, 0.42);
    let top = bottom - i32::from(frame.ground.y) * scale;
    let skin = visual::grade_art(ctx.color_filter, (190, 148, 106));
    let hair = visual::grade_art(ctx.color_filter, (54, 36, 28));
    draw_player_contact_shadow(canvas, ctx, left, bottom, scale);
    draw_player_cloak_tail(canvas, ctx, left, top, scale, shadow);
    for (row, pixels) in frame.sprite.rows.iter().enumerate() {
        for (column, pixel) in pixels.chars().enumerate() {
            let rgb = match pixel {
                '#' => Some(cloak),
                '+' => Some(shoulder),
                '.' => Some(shadow),
                's' => Some(skin),
                'h' => Some(hair),
                'b' => Some((30, 34, 38)),
                _ => None,
            };
            let Some(rgb) = rgb else { continue };
            scaled_rect(
                canvas,
                left + column as i32 * scale,
                top + row as i32 * scale,
                scale,
                scale,
                rgb,
            );
        }
    }
    let dodging = ctx
        .visual
        .player_action
        .is_some_and(|action| matches!(action.kind, PlayerActionKind::Dodge(_)));
    if !dodging {
        let (head_sum, head_count) = frame
            .sprite
            .rows
            .iter()
            .take(4)
            .flat_map(|row| row.chars().enumerate())
            .filter(|(_, pixel)| *pixel == 'h')
            .fold((0i32, 0i32), |(sum, count), (column, _)| {
                (sum + column as i32, count + 1)
            });
        let pose_shift = if head_count == 0 {
            0
        } else {
            head_sum / head_count - 6
        };
        draw_player_volume_details(
            canvas, ctx, left, top, scale, pose_shift, cloak, shoulder, shadow,
        );
    }
    draw_armor_details(canvas, ctx, left, top, scale, cloak);
    if ctx.player.crown_held {
        let crown = visual::grade_art(ctx.color_filter, (246, 202, 54));
        for (column, spike_height) in [(4, 2), (6, 3), (9, 2)] {
            scaled_rect(
                canvas,
                left + column * scale,
                top - (spike_height - 1) * scale,
                scale,
                spike_height * scale,
                crown,
            );
        }
        if !ctx.reduced_flash {
            let (column, height) = [(4, 2), (6, 3), (9, 2)][(ctx.tick as usize / 3) % 3];
            canvas.pixel(
                left + column * scale,
                top - (height - 1) * scale,
                color((255, 244, 156)),
            );
        }
    }
    draw_foreground_equipment(canvas, ctx, frame, left, top, scale);
}

#[allow(clippy::too_many_arguments)]
fn draw_player_volume_details(
    canvas: &mut HalfBlockCanvas,
    ctx: &RenderCtx<'_>,
    left: i32,
    top: i32,
    scale: i32,
    pose_shift: i32,
    cloak: Rgb,
    shoulder: Rgb,
    shadow: Rgb,
) {
    let rim = visual::grade_art(
        ctx.color_filter,
        visual::lerp(shoulder, (255, 220, 146), 0.28),
    );
    let deep = visual::darken(shadow, 0.58);
    let trim = visual::grade_art(ctx.color_filter, (190, 134, 58));

    // Stable upper-left key light makes every pose read as the same solid
    // figure; folds taper toward the feet instead of filling one flat mass.
    let shifted_left = left + pose_shift * scale;
    scaled_rect(canvas, shifted_left + 5 * scale, top, 2 * scale, scale, rim);
    scaled_rect(
        canvas,
        shifted_left + 4 * scale,
        top + 2 * scale,
        scale,
        2 * scale,
        deep,
    );
    scaled_rect(
        canvas,
        shifted_left + 2 * scale,
        top + 4 * scale,
        scale,
        3 * scale,
        rim,
    );
    scaled_rect(
        canvas,
        shifted_left + 10 * scale,
        top + 5 * scale,
        scale,
        3 * scale,
        deep,
    );
    for (column, start, length, shade) in [
        (4, 6, 4, visual::darken(cloak, 0.70)),
        (6, 6, 5, visual::darken(cloak, 0.56)),
        (8, 7, 3, visual::darken(cloak, 0.74)),
    ] {
        scaled_rect(
            canvas,
            shifted_left + column * scale,
            top + start * scale,
            scale,
            length * scale,
            shade,
        );
    }
    scaled_rect(
        canvas,
        shifted_left + 4 * scale,
        top + 9 * scale,
        5 * scale,
        scale,
        deep,
    );
    scaled_rect(
        canvas,
        shifted_left + 6 * scale,
        top + 9 * scale,
        scale,
        scale,
        trim,
    );

    let boot_rim = visual::grade_art(ctx.color_filter, (82, 74, 66));
    scaled_rect(
        canvas,
        shifted_left + 3 * scale,
        top + 12 * scale,
        2 * scale,
        scale,
        boot_rim,
    );
    scaled_rect(
        canvas,
        shifted_left + 8 * scale,
        top + 12 * scale,
        2 * scale,
        scale,
        boot_rim,
    );
}

fn draw_player_contact_shadow(
    canvas: &mut HalfBlockCanvas,
    ctx: &RenderCtx<'_>,
    left: i32,
    bottom: i32,
    scale: i32,
) {
    let center = left + 6 * scale;
    for row in 0..(2 * scale).max(1) {
        let inset = row / 2;
        canvas.line(
            center - 5 * scale + inset,
            bottom - row - 1,
            center + 5 * scale - inset,
            bottom - row - 1,
            color((5, 6, 8)),
        );
    }
    let stepping = ctx.visual.player_action.is_some_and(|action| {
        matches!(action.kind, PlayerActionKind::Step(_)) && action.phase() == 1
    });
    if !stepping || !ctx.map.in_bounds(ctx.player.x, ctx.player.y) {
        return;
    }
    let tile = ctx.map.tiles[ctx.map.idx(ctx.player.x, ctx.player.y)];
    match tile {
        Tile::ShallowWater => {
            let water = visual::grade_art(ctx.color_filter, (104, 196, 224));
            canvas.line(
                center - 7 * scale,
                bottom - 1,
                center - 3 * scale,
                bottom - 1,
                color(water),
            );
            canvas.line(
                center + 3 * scale,
                bottom - 1,
                center + 7 * scale,
                bottom - 1,
                color(visual::darken(water, 0.7)),
            );
        }
        Tile::Web => {
            let fiber = visual::grade_art(ctx.color_filter, (192, 204, 194));
            for dx in [-6, -3, 3, 6] {
                canvas.line(
                    center,
                    bottom - 1,
                    center + dx * scale,
                    bottom - 2 * scale,
                    color(fiber),
                );
            }
        }
        _ => {
            let palette = visual::world_palette(ctx.color_filter, Biome::from(ctx.player.depth));
            let dust = visual::lerp(palette.floor, palette.light, 0.28);
            for (dx, dy) in [(-6, -1), (-4, -2), (4, -2), (7, -1)] {
                canvas.pixel(center + dx * scale, bottom + dy * scale, color(dust));
            }
        }
    }
}

fn draw_player_cloak_tail(
    canvas: &mut HalfBlockCanvas,
    ctx: &RenderCtx<'_>,
    left: i32,
    top: i32,
    scale: i32,
    shadow: Rgb,
) {
    let action = ctx.visual.player_action;
    let (trail_x, trail_y) = match action.map(|action| action.kind) {
        Some(PlayerActionKind::Step(PlayerStepKind::StrafeLeft))
        | Some(PlayerActionKind::Turn(PlayerTurnKind::Left)) => (3, 0),
        Some(PlayerActionKind::Step(PlayerStepKind::StrafeRight))
        | Some(PlayerActionKind::Turn(PlayerTurnKind::Right)) => (-3, 0),
        Some(PlayerActionKind::Dodge(PlayerStepKind::StrafeLeft)) => (5, 1),
        Some(PlayerActionKind::Dodge(PlayerStepKind::StrafeRight)) => (-5, 1),
        Some(PlayerActionKind::Dodge(_)) => (2, 1),
        Some(PlayerActionKind::Step(PlayerStepKind::Forward)) => (-1, 1),
        Some(PlayerActionKind::Step(PlayerStepKind::Backward)) => (1, -1),
        Some(PlayerActionKind::Attack { .. } | PlayerActionKind::Technique(_)) => (-2, 0),
        _ => (0, 0),
    };
    if (trail_x, trail_y) == (0, 0) {
        return;
    }
    let x = left + (6 + trail_x) * scale;
    let y = top + (9 + trail_y) * scale;
    scaled_rect(canvas, x, y, 3 * scale, 4 * scale, shadow);
    scaled_rect(
        canvas,
        x + trail_x.signum() * scale,
        y + 3 * scale,
        2 * scale,
        2 * scale,
        visual::darken(shadow, 0.74),
    );
}

const PLAYER_IDLE_A: SpriteFrame = SpriteFrame::new(&[
    "     hhh     ",
    "    hsssh    ",
    "    hsssh    ",
    "   ++###++   ",
    "  ++#####++  ",
    "  +#######++ ",
    "  .#######.  ",
    "  .#######.  ",
    "   .#####.   ",
    "   .#####.   ",
    "   .##.##.   ",
    "   .##.##.   ",
    "   bb   bb   ",
    "  bbb   bbb  ",
    " ....   .... ",
]);

const PLAYER_IDLE_B: SpriteFrame = SpriteFrame::new(&[
    "     hhh     ",
    "    hsssh    ",
    "    hsssh    ",
    "   ++###++   ",
    "  ++#####++  ",
    "  +#######++ ",
    "  .#######.  ",
    "   #######   ",
    "   .#####.   ",
    "   .#####.   ",
    "   .##.##.   ",
    "   .##.##.   ",
    "  bb     bb  ",
    " bbb     bbb ",
    "....     ....",
]);

const PLAYER_WOUNDED: SpriteFrame = SpriteFrame::new(&[
    "    hhh      ",
    "   hsssh     ",
    "   hsssh     ",
    "  ++###++    ",
    " ++#####++   ",
    ".+#######++  ",
    ".#######.    ",
    " .#######.   ",
    "  .#####.    ",
    "   .#####.   ",
    "   .##.##.   ",
    "  .##  .##   ",
    "  bb    bb   ",
    " bbb     bbb ",
    "....      ...",
]);

const PLAYER_LIMP_STEP: SpriteFrame = SpriteFrame::new(&[
    "     hhh     ",
    "    hsssh    ",
    "    hsssh    ",
    "   ++###++   ",
    "  ++#####++  ",
    " .+#######++ ",
    " .#######.   ",
    "  .#######.  ",
    "   .#####.   ",
    "   .#####.   ",
    "   .###.##   ",
    "  .##   .##  ",
    " bb      bb  ",
    "bbb       bbb",
    "...       ...",
]);

const PLAYER_STEP: SpriteFrame = SpriteFrame::new(&[
    "      hhh    ",
    "     hsssh   ",
    "     hsssh   ",
    "   +++###++  ",
    "  ++######++ ",
    "  +#######++.",
    "  .#######.  ",
    "   #######.  ",
    "   .#####.   ",
    "   .#####.   ",
    "   .###.##   ",
    "  .##  .##   ",
    " bb      bb  ",
    "bbb      bbb ",
    "...      ....",
]);

const PLAYER_BACKSTEP: SpriteFrame = SpriteFrame::new(&[
    "    hhh      ",
    "   hsssh     ",
    "   hsssh     ",
    "  ++###+++   ",
    " ++######++  ",
    ".+#######++  ",
    "  .#######.  ",
    "  .#######   ",
    "   .#####.   ",
    "   .#####.   ",
    "   ##.###.   ",
    "   ##.  ##.  ",
    "   bb     bb ",
    "  bbb     bbb",
    "....      ...",
]);

const PLAYER_STRAFE_LEFT: SpriteFrame = SpriteFrame::new(&[
    "    hhh      ",
    "   hsssh     ",
    "   hsssh     ",
    "  ++###++    ",
    " ++#####++   ",
    "++#######++  ",
    ".#######.    ",
    ".#######.    ",
    " .#####.     ",
    " .#####.     ",
    " .##.##.     ",
    ".##  .##     ",
    "bb     bb    ",
    "bbb     bbb  ",
    "....     ....",
]);

const PLAYER_STRAFE_RIGHT: SpriteFrame = SpriteFrame::new(&[
    "      hhh    ",
    "     hsssh   ",
    "     hsssh   ",
    "    ++###++  ",
    "   ++#####++ ",
    "  ++#######++",
    "    .#######.",
    "    .#######.",
    "     .#####. ",
    "     .#####. ",
    "     .##.##. ",
    "     ##.  ##.",
    "    bb     bb",
    "  bbb     bbb",
    "....     ....",
]);

const PLAYER_DODGE_LEFT: SpriteFrame = SpriteFrame::new(&[
    "             ",
    "             ",
    "  hhh        ",
    " hsssh       ",
    "++###++      ",
    ".#####++     ",
    ".#######++   ",
    " .#######.   ",
    "  .#####.    ",
    " .#####.     ",
    ".##.##       ",
    "##   .##     ",
    "b      bb    ",
    "bbb      bbb ",
    "....      ...",
]);

const PLAYER_DODGE_RIGHT: SpriteFrame = SpriteFrame::new(&[
    "             ",
    "             ",
    "        hhh  ",
    "       hsssh ",
    "      ++###++",
    "     ++#####.",
    "   ++#######.",
    "   .#######. ",
    "    .#####.  ",
    "     .#####. ",
    "       ##.##.",
    "     ##.   ##",
    "    bb      b",
    " bbb      bbb",
    "...      ....",
]);

const PLAYER_TURN_LEFT: SpriteFrame = SpriteFrame::new(&[
    "    hhh      ",
    "   hsssh     ",
    "   hsssh     ",
    "  +++###++   ",
    " ++######++  ",
    ".+#######++  ",
    ".########.   ",
    " .#######.   ",
    "  .#####.    ",
    "   .#####.   ",
    "   .##.##.   ",
    "   .##.##.   ",
    "   bb   bb   ",
    "  bbb   bbb  ",
    " ....   .... ",
]);

const PLAYER_TURN_RIGHT: SpriteFrame = SpriteFrame::new(&[
    "      hhh    ",
    "     hsssh   ",
    "     hsssh   ",
    "   ++###+++  ",
    "  ++######++ ",
    "  ++#######+.",
    "   .########.",
    "   .#######. ",
    "    .#####.  ",
    "   .#####.   ",
    "   .##.##.   ",
    "   .##.##.   ",
    "   bb   bb   ",
    "  bbb   bbb  ",
    " ....   .... ",
]);

const PLAYER_ATTACK_READY: SpriteFrame = SpriteFrame::new(&[
    "    hhh      ",
    "   hsssh     ",
    "   hsssh     ",
    "  +++###++   ",
    " ++######++  ",
    ".+#######++  ",
    ".########.   ",
    " .#######.   ",
    "  .######.   ",
    "   .#####.   ",
    "   .##.##.   ",
    "  .##  .##   ",
    "  bb    bb   ",
    " bbb    bbb  ",
    "....    .... ",
]);

const PLAYER_ATTACK: SpriteFrame = SpriteFrame::new(&[
    "       hhh   ",
    "      hsssh  ",
    "      hsssh  ",
    "   +++####++ ",
    "  ++#######++",
    "  +#########.",
    "  .########. ",
    "   .#######  ",
    "   .######.  ",
    "   .#####.   ",
    "   .##.##.   ",
    "  .##  .##   ",
    "  bb    bb   ",
    " bbb    bbb  ",
    "....    .... ",
]);

const PLAYER_ATTACK_RECOVER: SpriteFrame = SpriteFrame::new(&[
    "      hhh    ",
    "     hsssh   ",
    "     hsssh   ",
    "   +++###++  ",
    "  ++######++ ",
    "  +#######++.",
    "  .########. ",
    "   .#######  ",
    "   .######.  ",
    "   .#####.   ",
    "   .##.##.   ",
    "   ##.  ##.  ",
    "   bb    bb  ",
    "  bbb    bbb ",
    " ....    ....",
]);

const PLAYER_CAST_READY: SpriteFrame = SpriteFrame::new(&[
    "     hhh     ",
    "    hsssh    ",
    "    hsssh    ",
    "   ++###++   ",
    "  ++#####++  ",
    " +#########+ ",
    ".+####.####+.",
    " .#######.   ",
    "  .#####.    ",
    "   .###.     ",
    "  .##.##.    ",
    "  .##.##.    ",
    "  bb   bb    ",
    " bbb   bbb   ",
    "....   ....  ",
]);

const PLAYER_CAST: SpriteFrame = SpriteFrame::new(&[
    "     hhh     ",
    "    hsssh    ",
    "    hsssh    ",
    "  +++###+++  ",
    " +#########+ ",
    ".###########.",
    ".###.###.###.",
    " .#########. ",
    "  .#######.  ",
    "   .#####.   ",
    "   .##.##.   ",
    "  .##  .##   ",
    "  bb    bb   ",
    " bbb    bbb  ",
    "....    .... ",
]);

const PLAYER_CAST_RECOVER: SpriteFrame = SpriteFrame::new(&[
    "     hhh     ",
    "    hsssh    ",
    "    hsssh    ",
    "   ++###++   ",
    "  ++#####++  ",
    "  +#######++ ",
    "  .#######.  ",
    "   .#####.   ",
    "   .#####.   ",
    "   .##.##.   ",
    "  .##  .##   ",
    "  .##  .##   ",
    "  bb    bb   ",
    " bbb    bbb  ",
    "....    .... ",
]);

const PLAYER_GUARD: SpriteFrame = SpriteFrame::new(&[
    "     hhh     ",
    "    hsssh    ",
    "    hsssh    ",
    " ++++###++++ ",
    "+###########+",
    ".###########.",
    ".###########.",
    " .#########. ",
    "  .#######.  ",
    "   .#####.   ",
    "   .##.##.   ",
    "   .##.##.   ",
    "   bb   bb   ",
    "  bbb   bbb  ",
    " ....   .... ",
]);

const PLAYER_HIT: SpriteFrame = SpriteFrame::new(&[
    "   hhh       ",
    "  hsssh      ",
    "  hsssh      ",
    " ++###++     ",
    "++#####++    ",
    ".#######.    ",
    ".#######.    ",
    " .#######.   ",
    "  .#####.    ",
    "  .#####.    ",
    "  .##.##.    ",
    "  .##.##.    ",
    "  bb   bb    ",
    " bbb   bbb   ",
    "....   ....  ",
]);

const PLAYER_EXHAUSTED: SpriteFrame = SpriteFrame::new(&[
    "             ",
    "   hhh       ",
    "  hsssh      ",
    "  hsssh      ",
    " ++###+++    ",
    "+########+.  ",
    ".#########.  ",
    " .########.  ",
    "  .######.   ",
    "   .#####.   ",
    "   .##.##.   ",
    "   .##  ##.  ",
    "   bb    bb  ",
    "  bbb    bbb ",
    " ....    ....",
]);

fn player_frame(ctx: &RenderCtx<'_>) -> AnchoredFrame {
    let action = ctx.visual.player_action;
    let critical = ctx.player.stats.hp * 10 <= ctx.player.stats.max_hp.max(1) * 3;
    let (sprite, hand, shield) = match action.map(|action| (action.kind, action.phase())) {
        Some((PlayerActionKind::Hit, _)) => (PLAYER_HIT, (10, 10), (1, 9)),
        Some((PlayerActionKind::Heal, 0)) => (PLAYER_GUARD, (8, 10), (2, 8)),
        Some((PlayerActionKind::Heal, _)) => (PLAYER_IDLE_A, (10, 10), (1, 9)),
        Some((PlayerActionKind::Guard, _)) => (PLAYER_GUARD, (10, 9), (1, 7)),
        Some((PlayerActionKind::Attack { .. }, 0)) => (PLAYER_ATTACK_READY, (3, 7), (1, 8)),
        Some((PlayerActionKind::Attack { .. }, 1)) => (PLAYER_ATTACK, (11, 6), (1, 9)),
        Some((PlayerActionKind::Attack { .. }, _)) => (PLAYER_ATTACK_RECOVER, (10, 9), (1, 9)),
        Some((PlayerActionKind::UseItem(_), 0)) => (PLAYER_CAST_READY, (8, 7), (2, 8)),
        Some((PlayerActionKind::UseItem(_), 1)) => (PLAYER_CAST, (9, 6), (2, 8)),
        Some((PlayerActionKind::UseItem(_), _)) => (PLAYER_CAST_RECOVER, (10, 9), (1, 9)),
        Some((PlayerActionKind::Technique(TechniqueKind::Kick), 0)) => {
            (PLAYER_BACKSTEP, (8, 9), (1, 8))
        }
        Some((PlayerActionKind::Technique(TechniqueKind::Kick), 1)) => {
            (PLAYER_DODGE_RIGHT, (9, 8), (1, 9))
        }
        Some((PlayerActionKind::Technique(TechniqueKind::ShieldBash), 0)) => {
            (PLAYER_GUARD, (8, 9), (3, 7))
        }
        Some((PlayerActionKind::Technique(TechniqueKind::ShieldBash), 1)) => {
            (PLAYER_STRAFE_RIGHT, (8, 8), (4, 7))
        }
        Some((PlayerActionKind::Technique(TechniqueKind::Lunge), 0)) => {
            (PLAYER_ATTACK_READY, (3, 7), (1, 8))
        }
        Some((PlayerActionKind::Technique(TechniqueKind::Lunge), 1)) => {
            (PLAYER_STEP, (11, 7), (1, 9))
        }
        Some((PlayerActionKind::Technique(TechniqueKind::Earthbreak), 0)) => {
            (PLAYER_ATTACK_READY, (4, 6), (1, 8))
        }
        Some((PlayerActionKind::Technique(TechniqueKind::Earthbreak), 1)) => {
            (PLAYER_ATTACK_RECOVER, (10, 8), (1, 9))
        }
        Some((PlayerActionKind::Technique(_), 0)) => (PLAYER_ATTACK_READY, (3, 7), (1, 8)),
        Some((PlayerActionKind::Technique(_), 1)) => (PLAYER_ATTACK, (11, 6), (1, 9)),
        Some((PlayerActionKind::Technique(_), _)) => (PLAYER_ATTACK_RECOVER, (10, 9), (1, 9)),
        Some((PlayerActionKind::Step(PlayerStepKind::Forward), 2)) => {
            (PLAYER_IDLE_B, (10, 10), (1, 9))
        }
        Some((PlayerActionKind::Step(PlayerStepKind::Forward), _)) if critical => {
            (PLAYER_LIMP_STEP, (9, 10), (1, 9))
        }
        Some((PlayerActionKind::Step(PlayerStepKind::Forward), _)) => {
            (PLAYER_STEP, (10, 10), (1, 9))
        }
        Some((PlayerActionKind::Step(PlayerStepKind::Backward), _)) => {
            (PLAYER_BACKSTEP, (9, 10), (1, 9))
        }
        Some((PlayerActionKind::Step(PlayerStepKind::StrafeLeft), _)) => {
            (PLAYER_STRAFE_LEFT, (8, 10), (1, 9))
        }
        Some((PlayerActionKind::Step(PlayerStepKind::StrafeRight), _)) => {
            (PLAYER_STRAFE_RIGHT, (11, 10), (2, 9))
        }
        Some((PlayerActionKind::Dodge(PlayerStepKind::StrafeLeft), _)) => {
            (PLAYER_DODGE_LEFT, (7, 10), (1, 9))
        }
        Some((PlayerActionKind::Dodge(PlayerStepKind::StrafeRight), _)) => {
            (PLAYER_DODGE_RIGHT, (11, 10), (2, 9))
        }
        Some((PlayerActionKind::Dodge(_), _)) => (PLAYER_BACKSTEP, (9, 10), (1, 9)),
        Some((PlayerActionKind::Turn(PlayerTurnKind::Left), _)) => {
            (PLAYER_TURN_LEFT, (8, 10), (1, 9))
        }
        Some((PlayerActionKind::Turn(PlayerTurnKind::Right), _)) => {
            (PLAYER_TURN_RIGHT, (11, 10), (2, 9))
        }
        None if ctx.player.guarding => (PLAYER_GUARD, (10, 9), (1, 7)),
        None if ctx.player_moving => (PLAYER_STEP, (10, 10), (1, 9)),
        None if critical => (PLAYER_WOUNDED, (9, 10), (1, 9)),
        None if ctx.player.stamina <= 2 => (PLAYER_EXHAUSTED, (9, 10), (1, 9)),
        None if ctx.tick % 32 < 16 => (PLAYER_IDLE_A, (10, 10), (1, 9)),
        None => (PLAYER_IDLE_B, (10, 10), (1, 9)),
    };
    AnchoredFrame::new(sprite.rows, (6, 15), hand, shield)
}

fn draw_armor_details(
    canvas: &mut HalfBlockCanvas,
    ctx: &RenderCtx<'_>,
    left: i32,
    top: i32,
    scale: i32,
    cloak: Rgb,
) {
    let armor = ctx.player.armor;
    let accent = match armor {
        Some(ItemKind::Chainmail) => visual::lerp(cloak, (238, 244, 242), 0.58),
        Some(ItemKind::TideglassMail) => (164, 242, 255),
        Some(ItemKind::Graveward) => (226, 220, 246),
        Some(ItemKind::LeatherArmor) => (210, 142, 78),
        _ => return,
    };
    let accent = visual::grade_art(ctx.color_filter, accent);
    let points: &[(i32, i32)] = match armor {
        Some(ItemKind::LeatherArmor) => &[
            (3, 4),
            (4, 4),
            (8, 4),
            (9, 4),
            (4, 6),
            (8, 6),
            (5, 8),
            (7, 8),
        ],
        Some(ItemKind::Chainmail) => &[
            (3, 4),
            (4, 4),
            (5, 5),
            (7, 5),
            (8, 4),
            (9, 4),
            (4, 7),
            (6, 7),
            (8, 7),
            (5, 9),
            (7, 9),
        ],
        Some(ItemKind::TideglassMail) => &[
            (4, 4),
            (8, 4),
            (5, 5),
            (7, 5),
            (4, 7),
            (8, 7),
            (5, 9),
            (6, 9),
            (7, 9),
        ],
        Some(ItemKind::Graveward) => &[
            (2, 4),
            (3, 4),
            (4, 4),
            (8, 4),
            (9, 4),
            (10, 4),
            (3, 5),
            (9, 5),
            (4, 7),
            (6, 7),
            (8, 7),
            (5, 9),
            (7, 9),
        ],
        _ => &[],
    };
    for &(column, row) in points {
        scaled_rect(
            canvas,
            left + column * scale,
            top + row * scale,
            scale,
            scale,
            accent,
        );
    }
    if armor == Some(ItemKind::TideglassMail) {
        let shimmer_column = 4 + ((ctx.tick / 3) % 5) as i32;
        scaled_rect(
            canvas,
            left + shimmer_column * scale,
            top + 6 * scale,
            scale,
            3 * scale,
            visual::grade_art(ctx.color_filter, (210, 252, 255)),
        );
    }
}

fn draw_foreground_equipment(
    canvas: &mut HalfBlockCanvas,
    ctx: &RenderCtx<'_>,
    frame: AnchoredFrame,
    left: i32,
    top: i32,
    scale: i32,
) {
    let hand_x = left + i32::from(frame.hand.x) * scale;
    let hand_y = top + i32::from(frame.hand.y) * scale;
    let action = ctx
        .visual
        .player_action
        .and_then(|action| match action.kind {
            PlayerActionKind::Attack { weapon } => Some((weapon, action.phase())),
            PlayerActionKind::Technique(_) => Some((ctx.player.weapon, action.phase())),
            _ => None,
        });
    let item_action = ctx
        .visual
        .player_action
        .and_then(|action| match action.kind {
            PlayerActionKind::UseItem(kind) => Some((kind, action.phase())),
            _ => None,
        });
    let phase = action.map_or(3, |(_, phase)| phase);
    if ctx.player.guarding || ctx.player.armor == Some(ItemKind::Shield) {
        let shield_x = left + i32::from(frame.shield.x) * scale;
        let shield_y = top + i32::from(frame.shield.y) * scale;
        draw_held_shield(canvas, ctx, shield_x, shield_y, scale, phase);
    }
    let Some(weapon) = action.and_then(|(weapon, _)| weapon).or(ctx.player.weapon) else {
        if let Some((kind, phase)) = item_action {
            draw_held_item(canvas, ctx, hand_x, hand_y, scale, kind, phase);
        }
        return;
    };
    if !weapon.is_weapon() {
        if let Some((kind, phase)) = item_action {
            draw_held_item(canvas, ctx, hand_x, hand_y, scale, kind, phase);
        }
        return;
    }
    draw_held_weapon(canvas, ctx, weapon, hand_x, hand_y, scale, phase);
    if let Some((kind, phase)) = item_action {
        draw_held_item(canvas, ctx, hand_x, hand_y, scale, kind, phase);
    }
}

fn draw_held_item(
    canvas: &mut HalfBlockCanvas,
    ctx: &RenderCtx<'_>,
    hand_x: i32,
    hand_y: i32,
    scale: i32,
    kind: ItemKind,
    phase: u8,
) {
    let (body, accent) = match kind {
        ItemKind::ScrollOfFire => ((188, 70, 34), (255, 218, 72)),
        ItemKind::ScrollOfConfusion => ((134, 74, 194), (226, 142, 250)),
        ItemKind::ScrollOfBlink => ((54, 154, 194), (184, 246, 255)),
        ItemKind::EnchantmentScroll => ((180, 148, 74), (255, 238, 164)),
        ItemKind::LoreFragment => ((54, 116, 168), (174, 228, 250)),
        ItemKind::HealthPotion | ItemKind::LargeHealthPotion | ItemKind::Antidote => {
            ((74, 168, 102), (198, 255, 184))
        }
        ItemKind::LanternOil => ((190, 126, 36), (255, 226, 110)),
        ItemKind::SmokeBomb => ((112, 120, 136), (224, 230, 238)),
        _ => ((160, 126, 66), (246, 224, 142)),
    };
    let body = visual::grade_art(ctx.color_filter, body);
    let accent = visual::grade_art(ctx.color_filter, accent);
    let lift = if phase == 0 {
        2
    } else if phase == 1 {
        -scale
    } else {
        0
    };
    let item_x = hand_x + scale + lift;
    let item_y = hand_y - 4 * scale;
    if matches!(
        kind,
        ItemKind::ScrollOfFire
            | ItemKind::ScrollOfConfusion
            | ItemKind::ScrollOfBlink
            | ItemKind::EnchantmentScroll
            | ItemKind::LoreFragment
    ) {
        scaled_rect(canvas, item_x, item_y, 4 * scale, 5 * scale, body);
        canvas.line(
            item_x + scale,
            item_y + scale,
            item_x + 3 * scale,
            item_y + scale,
            color(accent),
        );
        canvas.line(
            item_x + scale,
            item_y + 3 * scale,
            item_x + 2 * scale,
            item_y + 2 * scale,
            color(accent),
        );
    } else {
        scaled_rect(canvas, item_x + scale, item_y, 2 * scale, 4 * scale, body);
        scaled_rect(
            canvas,
            item_x + 2 * scale,
            item_y - scale,
            scale,
            scale,
            accent,
        );
        canvas.pixel(item_x + 2 * scale, item_y + 2 * scale, color(accent));
    }
}

fn draw_held_shield(
    canvas: &mut HalfBlockCanvas,
    ctx: &RenderCtx<'_>,
    center_x: i32,
    center_y: i32,
    scale: i32,
    phase: u8,
) {
    let rim = visual::grade_art(ctx.color_filter, (94, 158, 204));
    let face = visual::grade_art(ctx.color_filter, (42, 82, 122));
    let leather = visual::grade_art(ctx.color_filter, (142, 88, 44));
    let shadow = visual::darken(face, 0.48);
    let sway = if phase == 1 { scale } else { 0 };
    for (row, width) in [(0, 2), (1, 4), (2, 5), (3, 5), (4, 4), (5, 2)] {
        let y = center_y - 2 * scale + row * scale;
        let x = center_x - width * scale / 2 + scale;
        scaled_rect(canvas, x + scale, y + scale, width * scale, scale, shadow);
        scaled_rect(canvas, x + sway, y, width * scale, scale, face);
        if row <= 2 {
            scaled_rect(canvas, x + sway, y, scale, scale, rim);
        }
    }
    scaled_rect(
        canvas,
        center_x - scale + sway,
        center_y - scale,
        scale,
        3 * scale,
        leather,
    );
    scaled_rect(
        canvas,
        center_x - scale + sway,
        center_y - scale,
        scale,
        scale,
        visual::grade_art(ctx.color_filter, (224, 198, 96)),
    );
}

fn draw_held_weapon(
    canvas: &mut HalfBlockCanvas,
    ctx: &RenderCtx<'_>,
    weapon: ItemKind,
    hand_x: i32,
    hand_y: i32,
    scale: i32,
    phase: u8,
) {
    let worn =
        weapon.max_durability() > 0 && ctx.player.weapon_durability * 3 <= weapon.max_durability();
    let rust = visual::grade_art(ctx.color_filter, (150, 92, 52));
    let leather = visual::grade_art(ctx.color_filter, (112, 64, 34));
    let dark_metal = visual::grade_art(ctx.color_filter, (56, 62, 68));
    let notch = visual::grade_art(ctx.color_filter, (44, 38, 36));

    match weapon {
        ItemKind::RustySword => {
            let (tip_x, tip_y) = match phase {
                0 => (hand_x - 4 * scale, hand_y - 9 * scale),
                1 => (hand_x + 11 * scale, hand_y - 6 * scale),
                2 => (hand_x + 9 * scale, hand_y + 2 * scale),
                _ => (hand_x + 8 * scale, hand_y - 7 * scale),
            };
            let blade = visual::grade_art(
                ctx.color_filter,
                if worn {
                    visual::darken((162, 118, 76), 0.62)
                } else {
                    (162, 118, 76)
                },
            );
            draw_weapon_stroke(
                canvas, hand_x, hand_y, tip_x, tip_y, scale, dark_metal, blade,
            );
            scaled_rect(
                canvas,
                hand_x - scale,
                hand_y,
                2 * scale,
                2 * scale,
                leather,
            );
            canvas.line(
                hand_x - 2 * scale,
                hand_y - scale,
                hand_x + 2 * scale,
                hand_y + scale,
                color(rust),
            );
            canvas.pixel(
                tip_x,
                tip_y,
                color(visual::grade_art(ctx.color_filter, (218, 182, 120))),
            );
            if worn {
                let notch_x = (hand_x + tip_x) / 2;
                let notch_y = (hand_y + tip_y) / 2;
                scaled_rect(canvas, notch_x, notch_y, scale, scale, notch);
            }
        }
        ItemKind::Longsword => {
            let (tip_x, tip_y) = match phase {
                0 => (hand_x - 4 * scale, hand_y - 12 * scale),
                1 => (hand_x + 14 * scale, hand_y - 8 * scale),
                2 => (hand_x + 11 * scale, hand_y + 2 * scale),
                _ => (hand_x + 11 * scale, hand_y - 10 * scale),
            };
            let blade = visual::grade_art(
                ctx.color_filter,
                if worn {
                    visual::darken((204, 216, 214), 0.66)
                } else {
                    (204, 216, 214)
                },
            );
            let edge = visual::grade_art(ctx.color_filter, (250, 250, 232));
            draw_weapon_stroke(
                canvas, hand_x, hand_y, tip_x, tip_y, scale, dark_metal, blade,
            );
            let guard_y = hand_y - scale;
            canvas.line(
                hand_x - 3 * scale,
                guard_y,
                hand_x + 3 * scale,
                guard_y,
                color(edge),
            );
            scaled_rect(
                canvas,
                hand_x - scale,
                hand_y,
                2 * scale,
                3 * scale,
                leather,
            );
            canvas.pixel(tip_x, tip_y, color(edge));
            if phase == 1 && !ctx.reduced_flash {
                canvas.pixel(tip_x - scale, tip_y + scale, color(edge));
            }
            if worn {
                let notch_x = (hand_x + tip_x) / 2;
                let notch_y = (hand_y + tip_y) / 2;
                scaled_rect(canvas, notch_x, notch_y, scale, scale, notch);
            }
        }
        ItemKind::Warhammer => {
            let (head_x, head_y) = match phase {
                0 => (hand_x + 2 * scale, hand_y - 11 * scale),
                1 => (hand_x + 10 * scale, hand_y + 2 * scale),
                2 => (hand_x + 7 * scale, hand_y + 3 * scale),
                _ => (hand_x + 7 * scale, hand_y - 8 * scale),
            };
            let shaft = visual::grade_art(ctx.color_filter, (142, 88, 46));
            let head = visual::grade_art(
                ctx.color_filter,
                if worn {
                    visual::darken((166, 174, 170), 0.64)
                } else {
                    (166, 174, 170)
                },
            );
            draw_weapon_stroke(
                canvas,
                hand_x,
                hand_y + scale,
                head_x,
                head_y,
                scale,
                visual::darken(shaft, 0.54),
                shaft,
            );
            scaled_rect(
                canvas,
                hand_x - scale,
                hand_y,
                2 * scale,
                3 * scale,
                leather,
            );
            scaled_rect(
                canvas,
                head_x - 3 * scale,
                head_y - scale,
                6 * scale,
                3 * scale,
                visual::darken(head, 0.46),
            );
            scaled_rect(
                canvas,
                head_x - 2 * scale,
                head_y - 2 * scale,
                4 * scale,
                2 * scale,
                head,
            );
            scaled_rect(
                canvas,
                head_x - 3 * scale,
                head_y - scale,
                scale,
                3 * scale,
                visual::grade_art(ctx.color_filter, (224, 228, 212)),
            );
            canvas.pixel(head_x + 3 * scale, head_y, color(dark_metal));
        }
        ItemKind::Emberbrand => {
            let (tip_x, tip_y) = match phase {
                0 => (hand_x - 3 * scale, hand_y - 11 * scale),
                1 => (hand_x + 13 * scale, hand_y - 7 * scale),
                2 => (hand_x + 10 * scale, hand_y + scale),
                _ => (hand_x + 10 * scale, hand_y - 9 * scale),
            };
            let core = visual::grade_art(ctx.color_filter, (255, 190, 56));
            let ember = visual::grade_art(ctx.color_filter, (232, 62, 22));
            let edge = visual::grade_art(ctx.color_filter, (255, 232, 112));
            draw_weapon_stroke(canvas, hand_x, hand_y, tip_x, tip_y, scale, ember, core);
            let mid_x = (hand_x + tip_x) / 2;
            let mid_y = (hand_y + tip_y) / 2;
            canvas.pixel(mid_x + scale, mid_y - scale, color(edge));
            canvas.pixel(tip_x, tip_y, color(edge));
            scaled_rect(
                canvas,
                hand_x - scale,
                hand_y,
                2 * scale,
                2 * scale,
                leather,
            );
            canvas.pixel(hand_x + scale, hand_y - scale, color(ember));
            if !ctx.reduced_flash {
                let drift = if ctx.low_motion {
                    0
                } else {
                    (ctx.tick % 3) as i32 - 1
                };
                canvas.pixel(
                    mid_x + drift,
                    mid_y + scale,
                    color(visual::grade_art(ctx.color_filter, (246, 92, 28))),
                );
                if phase == 1 {
                    canvas.line(
                        hand_x,
                        hand_y + scale,
                        tip_x - 2 * scale,
                        tip_y + 2 * scale,
                        color(visual::darken(ember, 0.74)),
                    );
                }
            }
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_weapon_stroke(
    canvas: &mut HalfBlockCanvas,
    start_x: i32,
    start_y: i32,
    end_x: i32,
    end_y: i32,
    scale: i32,
    shadow: Rgb,
    face: Rgb,
) {
    canvas.line(
        start_x + scale,
        start_y + scale,
        end_x + scale,
        end_y + scale,
        color(shadow),
    );
    canvas.line(start_x, start_y, end_x, end_y, color(face));
}

/// Painterly/luminous post pass over the half-block viewport.
///
/// Three cheap layers, all gated by the accessibility flags:
///   1. Depth-haze — a luminous fog tint keyed off the wall depth buffer
///      (walls use the exact per-column distance; floor/ceiling reuse the same
///      projection the raycaster used). Skipped under `high_contrast`.
///   2. Glow — a soft radial halo behind the player foreground, the visible
///      face of the `glow` primitive. Reduced under `reduced_flash`.
///   3. Bloom — a single-pass `bloom` over bright pixels; weakened under
///      `high_contrast` and `reduced_flash`.
///
/// The pass is inherently static, so `low_motion` needs no special handling.
fn apply_luminous_pass(
    canvas: &mut HalfBlockCanvas,
    ctx: &RenderCtx<'_>,
    wall_depth: &[f64],
    quality: RenderQuality,
    horizon: i32,
) {
    let width = canvas.width() as i32;
    let height = canvas.pixel_height() as i32;
    if width < 8 || height < 8 {
        return;
    }
    let palette = medieval_palette(
        ctx,
        visual::world_palette(ctx.color_filter, Biome::from(ctx.player.depth)),
    );
    let haze = visual::lerp(palette.fog, palette.light, 0.22);

    // ── 1. Depth-haze layer ──
    if !ctx.high_contrast {
        let projection = height as f64 * 0.38;
        for px in 0..width {
            let wall_d = wall_depth
                .get(px as usize)
                .copied()
                .unwrap_or(MAX_VIEW_DEPTH);
            for py in 0..height {
                let row_depth = if (py - horizon).abs() < 1 {
                    wall_d
                } else {
                    projection / (py - horizon).abs() as f64
                };
                let t = (row_depth / MAX_VIEW_DEPTH).clamp(0.0, 1.0);
                let fog_t = (t * t * (3.0 - 2.0 * t)) as f32 * 0.16;
                if fog_t <= 0.0 {
                    continue;
                }
                canvas.screen_blend_pixel(px, py, haze, fog_t);
            }
        }
    }

    // ── 2. Soft glow behind the player foreground ──
    let base_glow = if ctx.reduced_flash { 0.05 } else { 0.10 };
    let glow_amt = if quality.fine_detail {
        base_glow
    } else {
        base_glow * 0.6
    };
    let cx = width / 2;
    let cy = (height - 4).clamp(2, height - 2);
    canvas.glow(
        cx,
        cy,
        (width as f32 * 0.22).max(6.0),
        palette.light,
        glow_amt,
    );

    // ── 3. Bloom ──
    let (threshold, strength) = if ctx.high_contrast {
        (0.72, 0.06)
    } else if ctx.reduced_flash {
        (0.62, 0.10)
    } else {
        (0.58, 0.16)
    };
    canvas.bloom(threshold, strength);
}

fn draw_screen_effects(canvas: &mut HalfBlockCanvas, ctx: &RenderCtx<'_>) {
    let width = canvas.width() as i32;
    let height = canvas.pixel_height() as i32;
    if ctx.visual.room_reveal_timer > 0 {
        let thickness = if ctx.low_motion {
            1
        } else {
            1 + i32::from(ctx.visual.room_reveal_timer > 8)
        };
        let strength = f32::from(ctx.visual.room_reveal_timer) / 12.0 * 0.24;
        let reveal = visual::world_palette(ctx.color_filter, Biome::from(ctx.player.depth)).light;
        canvas.tint_rect(0, 0, width, thickness, reveal, strength);
        canvas.tint_rect(
            0,
            height - thickness,
            width,
            thickness,
            reveal,
            strength * 0.72,
        );
        canvas.tint_rect(0, 0, thickness, height, reveal, strength * 0.62);
        canvas.tint_rect(
            width - thickness,
            0,
            thickness,
            height,
            reveal,
            strength * 0.62,
        );
    }
    if ctx.player.crown_held {
        let pulse = if ctx.reduced_flash || ctx.tick % 24 < 12 {
            0.08
        } else {
            0.13
        };
        canvas.tint_rect(0, 0, 2, height, (154, 48, 28), pulse);
        canvas.tint_rect(width - 2, 0, 2, height, (224, 156, 42), pulse);
        canvas.tint_rect(0, height - 2, width, 2, (188, 112, 34), pulse * 0.7);
        let phase = if ctx.low_motion {
            0
        } else {
            (ctx.tick / 3) as i32
        };
        for band in 0..4 {
            let y = (height / 5 * (band + 1) + (phase + band * 3).rem_euclid(5) - 2)
                .clamp(1, height - 2);
            let side = if band & 1 == 0 { 1 } else { width - 4 };
            canvas.line(
                side,
                y,
                side + if band & 1 == 0 { 3 } else { -3 },
                y,
                color(visual::grade_art(ctx.color_filter, (214, 128, 42))),
            );
        }
        if !ctx.reduced_flash {
            let tear_y = (height / 3 + phase.rem_euclid(7) - 3).clamp(2, height - 3);
            for x in (width / 3)..(width * 2 / 3) {
                if (x + phase) % 5 == 0 {
                    canvas.pixel(x, tear_y, color((118, 36, 28)));
                }
            }
        }
    }
    if ctx.visual.damage_flash_timer > 0 && !ctx.reduced_flash {
        let side = ((ctx.tick / 2) & 1) as i32;
        let x = if side == 0 { 0 } else { width - 3 };
        canvas.tint_rect(x, 0, 3, height, (220, 32, 28), 0.42);
    }
}

fn draw_peripheral_navigation(
    canvas: &mut HalfBlockCanvas,
    ctx: &RenderCtx<'_>,
    camera: Camera,
    horizon: i32,
) {
    let mut left = None;
    let mut right = None;
    for y in 0..ctx.map.height {
        for x in 0..ctx.map.width {
            if !visible(ctx, x, y) {
                continue;
            }
            let tile = ctx.map.tiles[ctx.map.idx(x, y)];
            if matches!(tile, Tile::StairsDown | Tile::StairsUp) {
                let rgb = if tile == Tile::StairsUp {
                    (242, 208, 142)
                } else {
                    (184, 156, 224)
                };
                consider_peripheral_target(camera, x, y, rgb, &mut left, &mut right);
            }
        }
    }
    for &(room_index, landmark) in ctx.landmarks {
        let Some(room) = ctx.map.rooms.get(room_index) else {
            continue;
        };
        let (x, y) = room.center();
        if visible(ctx, x, y) {
            consider_peripheral_target(
                camera,
                x,
                y,
                landmark_color(landmark),
                &mut left,
                &mut right,
            );
        }
    }
    for camp in ctx.camps {
        if visible(ctx, camp.x, camp.y) {
            consider_peripheral_target(
                camera,
                camp.x,
                camp.y,
                camp_color(camp.kind),
                &mut left,
                &mut right,
            );
        }
    }
    for monster in ctx.monsters {
        if monster.alive && visible(ctx, monster.x, monster.y) {
            consider_peripheral_target(
                camera,
                monster.x,
                monster.y,
                visual::lerp(monster_color(monster.kind), (255, 54, 42), 0.46),
                &mut left,
                &mut right,
            );
        }
    }

    let y = horizon.clamp(4, canvas.pixel_height() as i32 - 5);
    if let Some((_, rgb)) = left {
        let rgb = visual::grade_art(ctx.color_filter, rgb);
        canvas.line(3, y - 2, 1, y, color(rgb));
        canvas.line(1, y, 3, y + 2, color(rgb));
        canvas.pixel(3, y, color(visual::darken(rgb, 0.64)));
    }
    if let Some((_, rgb)) = right {
        let rgb = visual::grade_art(ctx.color_filter, rgb);
        let edge = canvas.width() as i32 - 2;
        canvas.line(edge - 2, y - 2, edge, y, color(rgb));
        canvas.line(edge, y, edge - 2, y + 2, color(rgb));
        canvas.pixel(edge - 2, y, color(visual::darken(rgb, 0.64)));
    }
}

fn consider_peripheral_target(
    camera: Camera,
    x: i32,
    y: i32,
    rgb: Rgb,
    left: &mut Option<(f64, Rgb)>,
    right: &mut Option<(f64, Rgb)>,
) {
    let relative_x = x as f64 + 0.5 - camera.x;
    let relative_y = y as f64 + 0.5 - camera.y;
    let forward = relative_x * camera.dir_x + relative_y * camera.dir_y;
    let lateral = relative_x * -camera.dir_y + relative_y * camera.dir_x;
    if forward > 0.12 && lateral.abs() <= forward * (FOV / 2.0).tan() * 0.92 {
        return;
    }
    let distance = relative_x.powi(2) + relative_y.powi(2);
    if forward <= 0.12 && lateral.abs() < 0.15 {
        if left.is_none_or(|(best, _)| distance < best) {
            *left = Some((distance, rgb));
        }
        if right.is_none_or(|(best, _)| distance < best) {
            *right = Some((distance, rgb));
        }
        return;
    }
    let slot = if lateral < 0.0 { left } else { right };
    if slot.is_none_or(|(best, _)| distance < best) {
        *slot = Some((distance, rgb));
    }
}

fn draw_forward_focus(canvas: &mut HalfBlockCanvas, ctx: &RenderCtx<'_>, horizon: i32) {
    let quarter = (ctx.camera_angle / std::f64::consts::FRAC_PI_2).round() as i32;
    let (dx, dy) = match quarter.rem_euclid(4) {
        0 => (1, 0),
        1 => (0, 1),
        2 => (-1, 0),
        _ => (0, -1),
    };
    let front = (ctx.player.x + dx, ctx.player.y + dy);
    if !ctx.map.in_bounds(front.0, front.1) {
        return;
    }
    if !visible(ctx, front.0, front.1) {
        return;
    }
    let hostile = ctx
        .monsters
        .iter()
        .any(|monster| monster.alive && (monster.x, monster.y) == front);
    let actionable = hostile
        || ctx.npcs.iter().any(|npc| (npc.x, npc.y) == front)
        || ctx.items.iter().any(|item| (item.x, item.y) == front)
        || matches!(
            ctx.map.tiles[ctx.map.idx(front.0, front.1)],
            Tile::DoorClosed
                | Tile::DoorOpen
                | Tile::LockedDoor
                | Tile::TrapRevealed
                | Tile::Urn
                | Tile::Barricade
                | Tile::LanternDead
                | Tile::StairsDown
                | Tile::StairsUp
                | Tile::Inscription
                | Tile::History(_)
        );
    if !actionable {
        return;
    }
    let focus = if hostile {
        visual::grade_art(ctx.color_filter, (255, 82, 58))
    } else {
        visual::grade_art(ctx.color_filter, (246, 190, 72))
    };
    let center_x = canvas.width() as i32 / 2;
    let half_w = (canvas.width() as i32 / 12).clamp(5, 9);
    let half_h = 4;
    for (sx, sy) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
        let x = center_x + sx * half_w;
        let y = horizon + sy * half_h;
        canvas.line(x, y, x - sx * 2, y, color(focus));
        canvas.line(x, y, x, y - sy * 2, color(focus));
    }
}

fn player_cloak(ctx: &RenderCtx<'_>) -> Rgb {
    if ctx.player.poisoned {
        return (72, 168, 78);
    }
    if ctx.player.crown_held {
        return (172, 112, 38);
    }
    match ctx.player.armor {
        Some(ItemKind::LeatherArmor) => (138, 82, 48),
        Some(ItemKind::Chainmail) => (118, 140, 150),
        Some(ItemKind::Shield) => (62, 122, 170),
        Some(ItemKind::TideglassMail) => (40, 160, 192),
        Some(ItemKind::Graveward) => (146, 140, 178),
        _ => (40, 144, 112),
    }
}

fn scaled_rect(canvas: &mut HalfBlockCanvas, x: i32, y: i32, w: i32, h: i32, rgb: Rgb) {
    canvas.rect(x, y, w, h, color(rgb));
}

fn visible(ctx: &RenderCtx<'_>, x: i32, y: i32) -> bool {
    ctx.map.in_bounds(x, y) && ctx.map.visible[ctx.map.idx(x, y)]
}

fn distance_sq(camera: Camera, x: f64, y: f64) -> f64 {
    (x - camera.x).powi(2) + (y - camera.y).powi(2)
}

fn texture_hash(seed: u64, x: i32, y: i32, px: i32, py: i32) -> u64 {
    seed ^ (x as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (y as i64 as u64).rotate_left(27)
        ^ (px as i64 as u64).rotate_left(11)
        ^ py as i64 as u64
}

fn landmark_color(kind: Landmark) -> Rgb {
    match kind {
        Landmark::SpiderNest => (176, 168, 190),
        Landmark::FloodedVault => (54, 154, 198),
        Landmark::BoneChapel => (206, 194, 154),
        Landmark::CollapsedForge => (218, 78, 30),
        Landmark::Shrine => (242, 208, 118),
        Landmark::BellChamber => (190, 124, 40),
    }
}

fn landmark_art(kind: Landmark, tick: u64) -> (&'static [&'static str], Rgb, Rgb) {
    match kind {
        Landmark::SpiderNest => (
            if tick % 12 < 6 {
                &[
                    "*..#..*", ".*###*.", "..###..", "#######", ".#.#.#.", "#.. ..#",
                ]
            } else {
                &[
                    "...#...", "**###**", "..###..", "#######", ".#.#.#.", "#.. ..#",
                ]
            },
            (176, 168, 190),
            (232, 226, 238),
        ),
        Landmark::FloodedVault => (
            &[
                ".#####.", "#.....#", "#.###.#", "#.#.#.#", "*******", ".*.*.*.",
            ],
            (54, 154, 198),
            (138, 226, 240),
        ),
        Landmark::BoneChapel => (
            &[
                "*..*..*", ".#.#.#.", "#######", "#.#.#.#", ".#...#.", "*.....*",
            ],
            (206, 194, 154),
            (244, 232, 194),
        ),
        Landmark::CollapsedForge => (
            if tick % 10 < 5 {
                &[
                    "..***..", "..###..", "..###..", "#######", ".#####.", "##...##",
                ]
            } else {
                &[
                    "...*...", "..*#*..", "..###..", "#######", ".#####.", "##...##",
                ]
            },
            (218, 78, 30),
            (255, 190, 58),
        ),
        Landmark::Shrine => (
            if tick % 16 < 8 {
                &[
                    "...*...", "..*#*..", "..###..", ".#####.", "#######", "..###..",
                ]
            } else {
                &[
                    "..*.*..", "...#...", ".*###*.", ".#####.", "#######", "..###..",
                ]
            },
            (242, 208, 118),
            (255, 244, 188),
        ),
        Landmark::BellChamber => (
            if tick % 14 < 7 {
                &[
                    "..###..", ".#...#.", ".#...#.", ".#####.", "..###..", "...*...", "...#...",
                ]
            } else {
                &[
                    "..###..", ".#...#.", ".#...#.", ".#####.", "..###..", "..*....", "..#....",
                ]
            },
            (190, 124, 40),
            (244, 194, 88),
        ),
    }
}

fn camp_color(kind: CampKind) -> Rgb {
    match kind {
        CampKind::DelverWaystation => (84, 118, 136),
        CampKind::RustguardOutpost => (120, 66, 50),
        CampKind::CartographerSurvey => (66, 122, 140),
        CampKind::ShrinePilgrim => (146, 120, 76),
        CampKind::Abandoned => (72, 68, 66),
    }
}

fn camp_art(kind: CampKind, tick: u64) -> (&'static [&'static str], Rgb, Rgb) {
    let flame_centered = tick % 8 < 4;
    match kind {
        CampKind::DelverWaystation => (
            if flame_centered {
                &["...*...", "..###..", ".#####.", "#######", "##...##"]
            } else {
                &["..*....", "..###..", ".#####.", "#######", "##...##"]
            },
            (84, 118, 136),
            (248, 148, 56),
        ),
        CampKind::RustguardOutpost => (
            &["*..#...", "#######", ".#####.", "###.###", "#.#.#.#"],
            (120, 66, 50),
            (226, 168, 74),
        ),
        CampKind::CartographerSurvey => (
            &["...*...", "...#...", ".#####.", "#.#.#.#", "#.....#"],
            (66, 122, 140),
            (218, 210, 142),
        ),
        CampKind::ShrinePilgrim => (
            &["*..*..*", "..###..", ".#####.", "#######", ".#...#."],
            (146, 120, 76),
            (252, 210, 112),
        ),
        CampKind::Abandoned => (
            &[".......", "...#...", "..###..", ".##.##.", "#.....#"],
            (72, 68, 66),
            (122, 88, 66),
        ),
    }
}

pub(crate) fn npc_art(kind: NpcKind, phase: u64, talked: bool, talking: bool) -> NpcArt {
    let beat = phase / if talking { 2 } else { 4 };
    let active = beat % 2 == 0;
    let (rows, primary, accent, skin): (&[&str], Rgb, Rgb, Rgb) = match kind {
        NpcKind::WoundedDelver => (
            if active {
                &[
                    "    @    ",
                    "   @@@   ",
                    "  .@@@.  ",
                    "  ####   ",
                    " *#####* ",
                    "  ###### ",
                    "   ###   ",
                    "   # #   ",
                    "  #   #  ",
                ]
            } else {
                &[
                    "    @    ",
                    "   @@@   ",
                    "  .@@@.  ",
                    "  ####   ",
                    "  #####* ",
                    " *#####  ",
                    "   ###   ",
                    "   # #   ",
                    "  #   #  ",
                ]
            },
            (126, 118, 108),
            (190, 66, 54),
            (216, 164, 128),
        ),
        NpcKind::Cartographer => (
            if active {
                &[
                    "    @    ",
                    "   @@@   ",
                    "  .@@@.  ",
                    "  #####  ",
                    " *#####* ",
                    "  ##*##  ",
                    "  ###### ",
                    "   # #   ",
                    "  #   #  ",
                ]
            } else {
                &[
                    "    @    ",
                    "   @@@   ",
                    "  .@@@.  ",
                    "  #####  ",
                    "  #####* ",
                    " *##*##  ",
                    "  ###### ",
                    "   # #   ",
                    "  #   #  ",
                ]
            },
            (72, 154, 166),
            (232, 214, 150),
            (226, 184, 142),
        ),
        NpcKind::Merchant => (
            if active {
                &[
                    "   ***   ",
                    "   @@@   ",
                    "  .@@@.  ",
                    "  #####  ",
                    " *#####* ",
                    "*#######*",
                    "  ##*##  ",
                    "   # #   ",
                    "  #   #  ",
                ]
            } else {
                &[
                    "   ***   ",
                    "   @@@   ",
                    "  .@@@.  ",
                    "  #####  ",
                    " *#####* ",
                    "*###*###*",
                    "  #####  ",
                    "   # #   ",
                    "  #   #  ",
                ]
            },
            (174, 112, 48),
            (244, 208, 90),
            (198, 144, 98),
        ),
        NpcKind::ShrineKeeper => (
            if active {
                &[
                    "   +++   ",
                    "    @    ",
                    "   @@@   ",
                    "  *@@@*  ",
                    "  #####  ",
                    " *#####* ",
                    "  ##*##  ",
                    "   # #   ",
                    "  #   #  ",
                ]
            } else {
                &[
                    "  +++++  ",
                    "    @    ",
                    "   @@@   ",
                    "  *@@@*  ",
                    "  #####  ",
                    " *#####* ",
                    "  ##*##  ",
                    "   # #   ",
                    "  #   #  ",
                ]
            },
            (154, 128, 78),
            (246, 224, 138),
            (222, 178, 132),
        ),
        NpcKind::WatchCaptain => (
            if active {
                &[
                    "   +++   ",
                    "   @@@   ",
                    "  #####  ",
                    " *#####* ",
                    "  ##*##  ",
                    "  ###### ",
                    "   ### * ",
                    "   # # * ",
                    "  #   #  ",
                ]
            } else {
                &[
                    "  +++++  ",
                    "   @@@   ",
                    "  #####  ",
                    " *#####* ",
                    "  ##*##  ",
                    "  #####  ",
                    "   ###*  ",
                    "   # # * ",
                    "  #   #  ",
                ]
            },
            (142, 66, 52),
            (234, 174, 76),
            (206, 150, 116),
        ),
        NpcKind::Rustguard => (
            if active {
                &[
                    "   ###   ",
                    "  @@@@   ",
                    "  .@@@.  ",
                    " *#####  ",
                    " *###### ",
                    "  #####  ",
                    "   ###   ",
                    "  #  #   ",
                    " *#  # * ",
                ]
            } else {
                &[
                    "   ###   ",
                    "  @@@@   ",
                    "  .@@@.  ",
                    " *#####* ",
                    " *###### ",
                    "  #####  ",
                    "   ###   ",
                    "  #  #   ",
                    " *#  # * ",
                ]
            },
            (116, 84, 66),
            (188, 196, 194),
            (188, 146, 112),
        ),
        NpcKind::Cook => (
            if active {
                &[
                    "  +++    ",
                    "   @@@   ",
                    "  .@@@.  ",
                    "  #####  ",
                    " *#####* ",
                    "  ##*##  ",
                    "   ### * ",
                    "   # # * ",
                    "  #   #  ",
                ]
            } else {
                &[
                    "  +++++  ",
                    "   @@@   ",
                    "  .@@@.  ",
                    "  #####  ",
                    " *#####* ",
                    " *##*##* ",
                    "   ### * ",
                    "   # # * ",
                    "  #   #  ",
                ]
            },
            (166, 112, 70),
            (238, 226, 188),
            (224, 178, 136),
        ),
        NpcKind::Quartermaster => (
            if active {
                &[
                    "   @@@   ",
                    "  .@@@.  ",
                    "  #####  ",
                    " *#####* ",
                    "  ##*##  ",
                    "  ###### ",
                    "   ###   ",
                    "   # #   ",
                    "  #   #  ",
                ]
            } else {
                &[
                    "   @@@   ",
                    "  .@@@.  ",
                    "  #####  ",
                    " *#####* ",
                    "  #*###  ",
                    "  ###### ",
                    "   ###   ",
                    "   # #   ",
                    "  #   #  ",
                ]
            },
            (112, 104, 82),
            (222, 194, 112),
            (198, 154, 112),
        ),
        NpcKind::ApprenticeMapper => (
            if active {
                &[
                    "    @    ",
                    "   @@@   ",
                    "  *@@@.  ",
                    "  #####  ",
                    " *###*#* ",
                    "  ##*##  ",
                    "   ###   ",
                    "   # #   ",
                    "  #   #  ",
                ]
            } else {
                &[
                    "    @    ",
                    "   @@@   ",
                    "  .@@@*  ",
                    " *#####  ",
                    " *#*###* ",
                    "  ##*##  ",
                    "   ###   ",
                    "   # #   ",
                    "  #   #  ",
                ]
            },
            (76, 142, 156),
            (214, 204, 146),
            (224, 182, 142),
        ),
        NpcKind::Scavenger => (
            if active {
                &[
                    "   @@@   ",
                    "  .@@@.  ",
                    " *#####  ",
                    "  ###### ",
                    " *#####* ",
                    "  ##*##  ",
                    "   ###   ",
                    " * # # * ",
                    "  #   #  ",
                ]
            } else {
                &[
                    "   @@@   ",
                    "  .@@@.  ",
                    "  #####* ",
                    " *###### ",
                    " *#####* ",
                    "  ##*##  ",
                    "   ###   ",
                    "  #  # * ",
                    "  #   #  ",
                ]
            },
            (118, 104, 74),
            (198, 150, 72),
            (186, 132, 92),
        ),
        NpcKind::LostDelver => (
            if active {
                &[
                    "   ***   ",
                    "  @@@@   ",
                    "  .@@@.  ",
                    "  #####  ",
                    " *#####* ",
                    "  ##*##  ",
                    "   ###   ",
                    "   # # * ",
                    "  #   #  ",
                ]
            } else {
                &[
                    "  *** *  ",
                    "  @@@@   ",
                    "  .@@@.  ",
                    " *#####  ",
                    " *#####* ",
                    "  ##*##  ",
                    "   ###   ",
                    "   # #*  ",
                    "  #   #  ",
                ]
            },
            (82, 124, 128),
            (220, 188, 118),
            (202, 156, 116),
        ),
        NpcKind::Pilgrim => (
            if active {
                &[
                    "   +++   ",
                    "   @@@   ",
                    "  .@@@.  ",
                    "  #####* ",
                    " *#####* ",
                    "  ##*##* ",
                    "   ###*  ",
                    "   # #*  ",
                    "  #   #  ",
                ]
            } else {
                &[
                    "  +++++  ",
                    "   @@@   ",
                    "  .@@@.  ",
                    " *#####* ",
                    "  #####* ",
                    "  ##*##* ",
                    "   ###*  ",
                    "   # #*  ",
                    "  #   #  ",
                ]
            },
            (142, 118, 82),
            (238, 216, 144),
            (224, 184, 142),
        ),
        NpcKind::Doubter => (
            if active {
                &[
                    "   ###   ",
                    "  #@@@#  ",
                    "  .@@@.  ",
                    "  #####  ",
                    " *##*##* ",
                    "  ###### ",
                    "   ###   ",
                    "   # #   ",
                    "  #   #  ",
                ]
            } else {
                &[
                    "  #####  ",
                    "  #@@@#  ",
                    "  .@@@.  ",
                    "  #####  ",
                    " *#####* ",
                    "  ##*##  ",
                    "   ###   ",
                    "   # #   ",
                    "  #   #  ",
                ]
            },
            (108, 104, 112),
            (190, 158, 116),
            (194, 156, 126),
        ),
        NpcKind::Lookout => (
            if beat % 3 != 1 {
                &[
                    "    @  * ",
                    "   @@@  * ",
                    "  .@@@.  ",
                    " *#####* ",
                    "  ###### ",
                    " *##*##  ",
                    "   ### * ",
                    "   # # * ",
                    "  #   #  ",
                ]
            } else {
                &[
                    "   @   * ",
                    "   @@@  * ",
                    "  .@@@.  ",
                    " *#####* ",
                    "  ###### ",
                    "  ##*##  ",
                    "   ### * ",
                    "   # # * ",
                    "  #   #  ",
                ]
            },
            (106, 94, 76),
            (226, 188, 82),
            (208, 162, 118),
        ),
        NpcKind::LanternTender => (
            if beat % 3 != 1 {
                &[
                    "   +++   ",
                    "   @@@   ",
                    "  .@@@.  ",
                    "  #####  ",
                    " *#####* ",
                    "  ####*  ",
                    "   ###*  ",
                    "   # #*  ",
                    "  #   #  ",
                ]
            } else {
                &[
                    "  +++++  ",
                    "   @@@   ",
                    "  .@@@.  ",
                    " *#####  ",
                    " *#####* ",
                    "  ####*  ",
                    "   ###*  ",
                    "   # #*  ",
                    "  #   #  ",
                ]
            },
            (70, 136, 150),
            (116, 222, 246),
            (224, 182, 138),
        ),
        NpcKind::CampSurvivor => (
            if active {
                &[
                    "   @@@   ",
                    "  .@@@.  ",
                    "  ####   ",
                    " *#####* ",
                    "  ###*#  ",
                    "  ###### ",
                    "   ###   ",
                    " * # #   ",
                    "  #   #  ",
                ]
            } else {
                &[
                    "   @@@   ",
                    "  .@@@.  ",
                    "  ####   ",
                    " *#####* ",
                    "  #*###  ",
                    " *###### ",
                    "   ###   ",
                    "   # # * ",
                    "  #   #  ",
                ]
            },
            (92, 82, 76),
            (174, 116, 74),
            (194, 148, 110),
        ),
    };
    let (primary, accent, skin) = if talked {
        (
            visual::darken(primary, 0.74),
            visual::darken(accent, 0.72),
            visual::darken(skin, 0.78),
        )
    } else {
        (primary, accent, skin)
    };
    NpcArt {
        rows,
        primary,
        accent,
        skin,
    }
}

fn monster_color(kind: MonsterKind) -> Rgb {
    match kind {
        MonsterKind::Rat => (146, 108, 72),
        MonsterKind::Bat => (130, 130, 176),
        MonsterKind::Goblin => (184, 54, 44),
        MonsterKind::Skeleton => (190, 186, 164),
        MonsterKind::Orc => (154, 44, 34),
        MonsterKind::Spider => (146, 52, 142),
        MonsterKind::Wraith => (72, 174, 220),
        MonsterKind::Slime => (66, 186, 84),
        MonsterKind::Mimic => (166, 122, 48),
        MonsterKind::Necromancer => (166, 50, 168),
        MonsterKind::Sentinel => (180, 154, 44),
        MonsterKind::Guardian => (224, 40, 34),
        MonsterKind::Stalker => (166, 90, 202),
        MonsterKind::BellWarden => (224, 164, 68),
    }
}

fn monster_frame(kind: MonsterKind, pose: MonsterPose, phase: u64) -> SpriteFrame {
    if pose == MonsterPose::Attack {
        return monster_attack_frame(kind);
    }
    let alternate = phase >= 24;
    let rows: &'static [&'static str] = match kind {
        MonsterKind::Rat => {
            if alternate {
                &[
                    "       ", "  .*.  ", " .###. ", "##.#.##", " .###. ", "  # #. ", " .     ",
                ]
            } else {
                &[
                    "       ", "  .*.  ", " .###. ", "##.#.##", " .###. ", " .# #  ", "     . ",
                ]
            }
        }
        MonsterKind::Bat => {
            if alternate {
                &[
                    "*     *", "##. .##", ".#####.", "  #*#  ", "  .#.  ", "   #   ", "       ",
                ]
            } else {
                &[
                    "   *   ", "* .#. *", "#######", ".##*##.", "  .#.  ", "   #   ", "       ",
                ]
            }
        }
        MonsterKind::Goblin => &[
            " .   . ", " .***. ", " .###. ", "##.#.##", " .###. ", " .# #. ", " ## ## ",
        ],
        MonsterKind::Skeleton => &[
            "  ...  ", " .***. ", " .#.#. ", "  ###  ", "##.#.##", "  # #  ", " .# #. ",
        ],
        MonsterKind::Orc => &[
            ".*   *.", ".#####.", "##*#*##", "#######", ".#####.", " ## ## ", "##   ##",
        ],
        MonsterKind::Spider => {
            if alternate {
                &[
                    "       ", "*.   .*", " #***# ", "###*###", " #***# ", ".#   #.", "#     #",
                ]
            } else {
                &[
                    "       ", ".*   *.", "# #*# #", "###*###", "# #*# #", "#     #", ".     .",
                ]
            }
        }
        MonsterKind::Wraith => {
            if alternate {
                &[
                    "  .*.  ", " .###. ", " .#*#. ", ".#####.", " .###. ", "  # #  ", " .   . ",
                ]
            } else {
                &[
                    "   *   ", "  ###  ", " .#*#. ", " ######", ".####. ", " .# #. ", "  . .  ",
                ]
            }
        }
        MonsterKind::Slime => {
            if alternate {
                &[
                    "       ", "       ", "  .*.  ", " .###. ", ".#*#*#.", "#######", ".#####.",
                ]
            } else {
                &[
                    "       ", "   .   ", "  ###  ", " .#*#. ", ".#####.", "#######", ".#####.",
                ]
            }
        }
        MonsterKind::Mimic => &[
            " .+++. ", ".#####.", "##***##", "#######", "#*#.#*#", ".#####.", "##   ##",
        ],
        MonsterKind::Necromancer => &[
            "  .*.  ", " .###. ", " .#*#. ", "##.#.##", " .###. ", "  # #  ", " .* *. ",
        ],
        MonsterKind::Sentinel => &[
            " .***. ", ".#####.", "##.#.##", "#######", ".#####.", " .# #. ", "### ###",
        ],
        MonsterKind::Guardian => &[
            "*.*.*.*", ".#####.", "##***##", "#######", "##.#.##", "### ###", "##   ##",
        ],
        MonsterKind::Stalker => &[
            "  .*.  ", " .###. ", ".#*#*#.", "##.#.##", " .###. ", "  # #  ", " .   . ",
        ],
        MonsterKind::BellWarden => &[
            "  ***  ", " .###. ", ".#####.", "##*#*##", "#######", " .# #. ", " .###. ",
        ],
    };
    SpriteFrame::new(rows)
}

fn monster_attack_frame(kind: MonsterKind) -> SpriteFrame {
    let rows: &'static [&'static str] = match kind {
        MonsterKind::Rat => &[
            "       ", "       ", "   .*. ", " .####.", "###.#.#", "  .##. ", " .#  # ",
        ],
        MonsterKind::Bat => &[
            "*     *", ".#. .#.", " ######", "  #*#  ", " .###. ", "  # #  ", "       ",
        ],
        MonsterKind::Goblin => &[
            " .   * ", " .***# ", " .#### ", "##.#.##", "  ###. ", " .# ## ", "##   # ",
        ],
        MonsterKind::Skeleton => &[
            "  ... *", " .***.#", " .#.#. ", "  #### ", "##.#   ", "  # #  ", " .#  # ",
        ],
        MonsterKind::Orc => &[
            ".*   **", ".#####*", "##*#*##", "#######", " .#####", " ##  ##", "##    #",
        ],
        MonsterKind::Spider => &[
            "*     *", " # * # ", "###*###", " #***# ", "# #*# #", ".#   #.", "#     #",
        ],
        MonsterKind::Wraith => &[
            "    *  ", "  .###*", " .#*## ", ".##### ", " .###  ", " #  #  ", ".    . ",
        ],
        MonsterKind::Slime => &[
            "       ", "    .* ", "  .####", " .#*#*#", ".######", "#######", ".#####.",
        ],
        MonsterKind::Mimic => &[
            "  ***  ", " .###. ", "##***##", "#*#.#*#", "#######", ".#####.", "##   ##",
        ],
        MonsterKind::Necromancer => &[
            "*  .*  ", "# .### ", "# .#*#.", "###.#.#", "  .### ", "  # #  ", " .* *. ",
        ],
        MonsterKind::Sentinel => &[
            "***.   ", "#####. ", "##.#.##", "#######", " .#####", " .#  # ", "###  ##",
        ],
        MonsterKind::Guardian => &[
            "*.*.*.*", "#######", "##***##", "#######", " .#####", "### ###", "##    #",
        ],
        MonsterKind::Stalker => &[
            " * .** ", " .###* ", ".#*#*#.", "##.# ##", " .###. ", "  # #  ", ".    . ",
        ],
        MonsterKind::BellWarden => &[
            "*** ***", ".#####.", "##***##", "#######", " .#####", " .# #. ", " .###. ",
        ],
    };
    SpriteFrame::new(rows)
}

fn scale((red, green, blue): Rgb, factor: f32) -> Rgb {
    (
        (red as f32 * factor).round().clamp(0.0, 255.0) as u8,
        (green as f32 * factor).round().clamp(0.0, 255.0) as u8,
        (blue as f32 * factor).round().clamp(0.0, 255.0) as u8,
    )
}

fn color((red, green, blue): Rgb) -> Color {
    Color::Rgb(red, green, blue)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_monster_has_a_unique_precise_silhouette() {
        let kinds = [
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
        let signatures = kinds
            .into_iter()
            .map(|kind| monster_frame(kind, MonsterPose::Idle, 0).rows.join("\n"))
            .collect::<HashSet<_>>();

        assert_eq!(signatures.len(), kinds.len());
    }

    #[test]
    fn animated_monsters_have_stable_frame_bounds() {
        for kind in [
            MonsterKind::Rat,
            MonsterKind::Bat,
            MonsterKind::Spider,
            MonsterKind::Wraith,
            MonsterKind::Slime,
        ] {
            let first = monster_frame(kind, MonsterPose::Idle, 0);
            let second = monster_frame(kind, MonsterPose::Idle, 30);
            assert_eq!(
                (first.width(), first.height()),
                (second.width(), second.height())
            );
            assert_ne!(first.rows, second.rows);
        }
    }

    #[test]
    fn every_monster_attack_changes_its_silhouette_without_changing_bounds() {
        for kind in [
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
        ] {
            let idle = monster_frame(kind, MonsterPose::Idle, 0);
            let attack = monster_frame(kind, MonsterPose::Attack, 0);
            assert_eq!(
                (idle.width(), idle.height()),
                (attack.width(), attack.height()),
                "{} attack bounds",
                kind.name()
            );
            assert_ne!(idle.rows, attack.rows, "{} attack frame", kind.name());
        }
    }

    #[test]
    fn actor_step_interpolation_has_eased_exact_endpoints() {
        assert_eq!(actor_step_progress(6), 0.0);
        assert_eq!(actor_step_progress(1), 1.0);
        assert_eq!(actor_step_progress(0), 1.0);
        assert!(actor_step_progress(4) < actor_step_progress(3));
    }

    #[test]
    fn damage_digit_atlas_distinguishes_all_values() {
        let digits = ('0'..='9').map(digit_rows).collect::<HashSet<_>>();
        assert_eq!(digits.len(), 10);
    }

    #[test]
    fn player_pose_frames_keep_fixed_bounds() {
        for frame in [
            PLAYER_IDLE_A,
            PLAYER_IDLE_B,
            PLAYER_WOUNDED,
            PLAYER_LIMP_STEP,
            PLAYER_STEP,
            PLAYER_BACKSTEP,
            PLAYER_STRAFE_LEFT,
            PLAYER_STRAFE_RIGHT,
            PLAYER_DODGE_LEFT,
            PLAYER_DODGE_RIGHT,
            PLAYER_TURN_LEFT,
            PLAYER_TURN_RIGHT,
            PLAYER_ATTACK_READY,
            PLAYER_ATTACK,
            PLAYER_ATTACK_RECOVER,
            PLAYER_CAST_READY,
            PLAYER_CAST,
            PLAYER_CAST_RECOVER,
            PLAYER_GUARD,
            PLAYER_HIT,
            PLAYER_EXHAUSTED,
        ] {
            assert_eq!((frame.width(), frame.height()), (13, 15));
        }
    }

    #[test]
    fn monster_impacts_use_material_specific_art() {
        assert_eq!(
            monster_impact_material(MonsterKind::Rat),
            ImpactMaterial::Flesh
        );
        assert_eq!(
            monster_impact_material(MonsterKind::Skeleton),
            ImpactMaterial::Bone
        );
        assert_eq!(
            monster_impact_material(MonsterKind::Sentinel),
            ImpactMaterial::Metal
        );
        assert_eq!(
            monster_impact_material(MonsterKind::Slime),
            ImpactMaterial::Slime
        );
        assert_eq!(
            monster_impact_material(MonsterKind::Wraith),
            ImpactMaterial::Spectral
        );
    }
}
