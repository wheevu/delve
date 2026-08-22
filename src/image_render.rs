// Terminal image rendering for Delve.
//
// Uses half-block Unicode art for consistent rendering across terminals.

use crate::visual::{self, Biome, Rgb};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

#[derive(Debug, Clone, Copy)]
pub struct SpriteFrame {
    pub rows: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpriteAnchor {
    pub x: i8,
    pub y: i8,
}

#[derive(Debug, Clone, Copy)]
pub struct AnchoredFrame {
    pub sprite: SpriteFrame,
    pub ground: SpriteAnchor,
    pub hand: SpriteAnchor,
    pub shield: SpriteAnchor,
}

impl AnchoredFrame {
    pub const fn new(
        rows: &'static [&'static str],
        ground: (i8, i8),
        hand: (i8, i8),
        shield: (i8, i8),
    ) -> Self {
        Self {
            sprite: SpriteFrame::new(rows),
            ground: SpriteAnchor {
                x: ground.0,
                y: ground.1,
            },
            hand: SpriteAnchor {
                x: hand.0,
                y: hand.1,
            },
            shield: SpriteAnchor {
                x: shield.0,
                y: shield.1,
            },
        }
    }

    pub fn anchors_in_bounds(self) -> bool {
        let width = self.sprite.width() as i8;
        let height = self.sprite.height() as i8;
        [self.ground, self.hand, self.shield]
            .into_iter()
            .all(|anchor| anchor.x >= 0 && anchor.x < width && anchor.y >= 0 && anchor.y <= height)
    }
}

impl SpriteFrame {
    pub const fn new(rows: &'static [&'static str]) -> Self {
        Self { rows }
    }

    pub fn width(self) -> usize {
        self.rows
            .iter()
            .map(|row| row.chars().count())
            .max()
            .unwrap_or(0)
    }

    pub fn height(self) -> usize {
        self.rows.len()
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// HALF-BLOCK ART RENDERER
//
// Renders "pixel art" using Unicode upper/lower half-block characters
// (▀ and ▄) with 2 vertical pixels per terminal cell.
// This effectively doubles the vertical resolution.
// ══════════════════════════════════════════════════════════════════════════════

/// A half-block canvas for drawing.
pub struct HalfBlockCanvas {
    width: usize,
    height: usize,      // in terminal cells (each cell = 2 half-block pixels)
    pixels: Vec<Color>, // top-half pixels, then bottom-half
}

impl HalfBlockCanvas {
    pub fn new(cell_width: usize, cell_height: usize) -> Self {
        let pixel_height = cell_height * 2;
        Self {
            width: cell_width,
            height: cell_height,
            pixels: vec![Color::Black; cell_width * pixel_height],
        }
    }

    /// Set a half-block pixel at (x, pixel_y) where pixel_y is 0..height*2.
    pub fn set_pixel(&mut self, x: usize, pixel_y: usize, color: Color) {
        let pixel_h = self.height * 2;
        if x >= self.width || pixel_y >= pixel_h {
            return;
        }
        self.pixels[x + pixel_y * self.width] = color;
    }

    /// Set a pixel using signed coordinates. Projected drawing should use this
    /// entry point so partially off-screen geometry clips without underflow.
    pub fn pixel(&mut self, x: i32, pixel_y: i32, color: Color) {
        if x < 0 || pixel_y < 0 {
            return;
        }
        self.set_pixel(x as usize, pixel_y as usize, color);
    }

    /// Blend one signed-coordinate pixel toward an RGB color.
    pub fn tint_pixel(&mut self, x: i32, pixel_y: i32, tint: (u8, u8, u8), amount: f32) {
        if x < 0 || pixel_y < 0 || x >= self.width as i32 || pixel_y >= self.pixel_height() as i32 {
            return;
        }
        let pixel = &mut self.pixels[x as usize + pixel_y as usize * self.width];
        let (r, g, b) = match *pixel {
            Color::Rgb(r, g, b) => (r, g, b),
            Color::Black => (0, 0, 0),
            _ => return,
        };
        let amount = amount.clamp(0.0, 1.0);
        *pixel = Color::Rgb(
            (r as f32 + (tint.0 as f32 - r as f32) * amount).round() as u8,
            (g as f32 + (tint.1 as f32 - g as f32) * amount).round() as u8,
            (b as f32 + (tint.2 as f32 - b as f32) * amount).round() as u8,
        );
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn pixel_height(&self) -> usize {
        self.height * 2
    }

    pub fn fill(&mut self, color: Color) {
        self.pixels.fill(color);
    }

    #[allow(dead_code)]
    pub fn tint(&mut self, tint: (u8, u8, u8), amount: f32) {
        self.tint_rect(
            0,
            0,
            self.width as i32,
            self.pixel_height() as i32,
            tint,
            amount,
        );
    }

    /// Darken the frame toward its edges with a smooth radial falloff.
    /// Static lens effect: keeps the eye on the center of the viewport.
    pub fn vignette(&mut self, strength: f32) {
        let strength = strength.clamp(0.0, 1.0);
        if strength <= 0.0 {
            return;
        }
        let width = self.width as f32;
        let height = self.pixel_height() as f32;
        for py in 0..self.pixel_height() {
            for px in 0..self.width {
                let nx = (px as f32 + 0.5) / width - 0.5;
                let ny = (py as f32 + 0.5) / height - 0.5;
                let d2 = (nx * nx + ny * ny) * 2.0;
                let t = ((d2 - 0.30) / 0.70).clamp(0.0, 1.0);
                let shade = t * t * (3.0 - 2.0 * t) * strength;
                if shade <= 0.0 {
                    continue;
                }
                let pixel = &mut self.pixels[px + py * self.width];
                let (r, g, b) = match *pixel {
                    Color::Rgb(r, g, b) => (r, g, b),
                    Color::Black => (0, 0, 0),
                    _ => continue,
                };
                let keep = 1.0 - shade;
                *pixel = Color::Rgb(
                    (r as f32 * keep).round() as u8,
                    (g as f32 * keep).round() as u8,
                    (b as f32 * keep).round() as u8,
                );
            }
        }
    }

    pub fn increase_contrast(&mut self, amount: f32) {
        let factor = 1.0 + amount.clamp(0.0, 1.0);
        for pixel in &mut self.pixels {
            let Color::Rgb(r, g, b) = *pixel else {
                continue;
            };
            let adjust = |channel: u8| {
                (128.0 + (channel as f32 - 128.0) * factor)
                    .round()
                    .clamp(0.0, 255.0) as u8
            };
            *pixel = Color::Rgb(adjust(r), adjust(g), adjust(b));
        }
    }

    /// Screen-blend an RGB color onto a single signed-coordinate pixel.
    ///
    /// Cheap luminous primitive: brightens a pixel toward `add` without ever
    /// clipping to flat white, because screen blending saturates gently. Used
    /// by `glow` and `bloom` to build the painterly pass.
    pub fn screen_blend_pixel(&mut self, x: i32, py: i32, add: (u8, u8, u8), amount: f32) {
        if x < 0 || py < 0 || x >= self.width as i32 || py >= self.pixel_height() as i32 {
            return;
        }
        let pixel = &mut self.pixels[x as usize + py as usize * self.width];
        let (r, g, b) = match *pixel {
            Color::Rgb(r, g, b) => (r, g, b),
            Color::Black => (0, 0, 0),
            _ => return,
        };
        let (r2, g2, b2) = visual::screen_blend((r, g, b), add, amount);
        *pixel = Color::Rgb(r2, g2, b2);
    }

    /// Additive radial glow built from `screen_blend_pixel`.
    ///
    /// One pass over the glow's bounding box; pixels outside the radius are
    /// skipped, and the core fades with a smoothstep so the halo reads as
    /// painted light rather than a hard disc. `amount` is the peak strength at
    /// the center.
    pub fn glow(&mut self, cx: i32, cy: i32, radius: f32, color: (u8, u8, u8), amount: f32) {
        if radius <= 0.0 || amount <= 0.0 {
            return;
        }
        let r = radius.ceil() as i32;
        let x0 = (cx - r).max(0);
        let x1 = (cx + r).min(self.width as i32 - 1);
        let y0 = (cy - r).max(0);
        let y1 = (cy + r).min(self.pixel_height() as i32 - 1);
        for py in y0..=y1 {
            for px in x0..=x1 {
                let dx = px - cx;
                let dy = py - cy;
                let dist = (dx * dx + dy * dy) as f32;
                let t = (1.0 - (dist.sqrt() / radius)).clamp(0.0, 1.0);
                if t <= 0.0 {
                    continue;
                }
                let falloff = t * t * (3.0 - 2.0 * t);
                self.screen_blend_pixel(px, py, color, amount * falloff);
            }
        }
    }

    /// Cheap single-pass bloom.
    ///
    /// Bright pixels (luminance above `threshold`) are lifted and bled a little
    /// into their neighbours with a screen blend, so highlights gain a soft
    /// luminous halo. Seeds are gathered first so the pass never feeds its own
    /// output back in, keeping it stable and order-independent. `strength`
    /// scales how far the halo reaches.
    pub fn bloom(&mut self, threshold: f32, strength: f32) {
        let threshold = threshold.clamp(0.0, 1.0);
        let strength = strength.clamp(0.0, 1.0);
        if strength <= 0.0 {
            return;
        }
        let width = self.width;
        let ph = self.pixel_height();
        let mut seeds: Vec<(usize, (u8, u8, u8), f32)> = Vec::new();
        for py in 0..ph {
            for px in 0..width {
                let idx = px + py * width;
                let Color::Rgb(r, g, b) = self.pixels[idx] else {
                    continue;
                };
                let lum = luminance((r, g, b)) / 255.0;
                if lum > threshold {
                    let w = (lum - threshold) / (1.0 - threshold).max(0.001);
                    seeds.push((idx, (r, g, b), w));
                }
            }
        }
        for (idx, color, w) in seeds {
            let px = (idx % width) as i32;
            let py = (idx / width) as i32;
            let core = strength * w * 0.5;
            for (ox, oy) in [
                (0, 0),
                (1, 0),
                (-1, 0),
                (0, 1),
                (0, -1),
                (2, 0),
                (-2, 0),
                (0, 2),
                (0, -2),
            ] {
                let a = if ox == 0 && oy == 0 { core } else { core * 0.5 };
                self.screen_blend_pixel(px + ox, py + oy, color, a);
            }
        }
    }

    #[allow(dead_code)]
    pub fn tint_rect(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        tint: (u8, u8, u8),
        amount: f32,
    ) {
        let amount = amount.clamp(0.0, 1.0);
        let x0 = x.max(0) as usize;
        let y0 = y.max(0) as usize;
        let x1 = (x + width).clamp(0, self.width as i32) as usize;
        let y1 = (y + height).clamp(0, self.pixel_height() as i32) as usize;
        for py in y0..y1 {
            for px in x0..x1 {
                let pixel = &mut self.pixels[px + py * self.width];
                let (r, g, b) = match *pixel {
                    Color::Rgb(r, g, b) => (r, g, b),
                    Color::Black => (0, 0, 0),
                    _ => continue,
                };
                *pixel = Color::Rgb(
                    (r as f32 + (tint.0 as f32 - r as f32) * amount).round() as u8,
                    (g as f32 + (tint.1 as f32 - g as f32) * amount).round() as u8,
                    (b as f32 + (tint.2 as f32 - b as f32) * amount).round() as u8,
                );
            }
        }
    }

    pub fn line(&mut self, mut x0: i32, mut y0: i32, x1: i32, y1: i32, color: Color) {
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            self.pixel(x0, y0, color);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = err * 2;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    pub fn rect(&mut self, x: i32, y: i32, width: i32, height: i32, color: Color) {
        if width <= 0 || height <= 0 {
            return;
        }
        for py in y..y + height {
            for px in x..x + width {
                self.pixel(px, py, color);
            }
        }
    }

    /// Draw a row-major sprite. `None` pixels are transparent; black pixels are
    /// deliberately opaque.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "binary transparency is part of the canvas contract"
        )
    )]
    pub fn masked_sprite(&mut self, x: i32, y: i32, width: usize, pixels: &[Option<Color>]) {
        if width == 0 {
            return;
        }
        for (index, pixel) in pixels.iter().enumerate() {
            if let Some(color) = pixel {
                self.pixel(
                    x + (index % width) as i32,
                    y + (index / width) as i32,
                    *color,
                );
            }
        }
    }

    /// Fill a rectangle with a color.
    #[allow(dead_code)]
    pub fn fill_rect(&mut self, x: usize, py: usize, w: usize, h: usize, color: Color) {
        let pixel_h = self.height * 2;
        for dy in 0..h {
            let py2 = py + dy;
            if py2 >= pixel_h {
                break;
            }
            for dx in 0..w {
                let px = x + dx;
                if px >= self.width {
                    break;
                }
                self.pixels[px + py2 * self.width] = color;
            }
        }
    }

    /// Draw a horizontal line.
    pub fn h_line(&mut self, x: usize, py: usize, w: usize, color: Color) {
        for dx in 0..w {
            self.set_pixel(x + dx, py, color);
        }
    }

    /// Draw a vertical line.
    #[allow(dead_code)]
    pub fn v_line(&mut self, x: usize, py: usize, h: usize, color: Color) {
        for dy in 0..h {
            self.set_pixel(x, py + dy, color);
        }
    }

    /// Render the canvas to a Ratatui buffer.
    pub fn render(&self, buf: &mut Buffer, area: Rect) {
        let _pixel_h = self.height * 2;
        for cell_y in 0..self.height.min(area.height as usize) {
            for cell_x in 0..self.width.min(area.width as usize) {
                let top_idx = cell_x + (cell_y * 2) * self.width;
                let bot_idx = cell_x + (cell_y * 2 + 1) * self.width;

                let top_color = if top_idx < self.pixels.len() {
                    self.pixels[top_idx]
                } else {
                    Color::Black
                };
                let bot_color = if bot_idx < self.pixels.len() {
                    self.pixels[bot_idx]
                } else {
                    Color::Black
                };

                let render_x = area.x + cell_x as u16;
                let render_y = area.y + cell_y as u16;
                buf[(render_x, render_y)]
                    .set_char('\u{2580}')
                    .set_fg(top_color)
                    .set_bg(bot_color);
            }
        }
    }
}

/// Perceptual luminance of an RGB triple in 0..=255.
fn luminance(c: (u8, u8, u8)) -> f32 {
    0.2126 * c.0 as f32 + 0.7152 * c.1 as f32 + 0.0722 * c.2 as f32
}

// ══════════════════════════════════════════════════════════════════════════════
// MENU VISTA — animated raycast dungeon backdrop
//
// A tiny authored corridor rendered with the same techniques as the live
// viewport: floor/ceiling casting, DDA walls, flickering torch lights, fog,
// and the Crown waiting in a lit alcove at the far end. Fully deterministic;
// `tick` only drives flame flicker, mote drift, and the camera's slow sway.
// ══════════════════════════════════════════════════════════════════════════════

const MENU_MAP: [&str; 11] = [
    "################",
    "#####......#####",
    "####........####",
    "###...T..T...###",
    "###....CC....###",
    "###....CC....###",
    "###..........###",
    "####........####",
    "####........####",
    "#####......#####",
    "################",
];

const MENU_MAP_W: i32 = 16;
const MENU_MAP_H: i32 = 11;

fn menu_tile(x: i32, y: i32) -> Option<char> {
    if x < 0 || y < 0 || x >= MENU_MAP_W || y >= MENU_MAP_H {
        return Some('#');
    }
    MENU_MAP[y as usize].chars().nth(x as usize)
}

fn menu_hash(x: i32, y: i32, px: i32, py: i32) -> u64 {
    let mut h = (x as u64)
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add(y as u64)
        .wrapping_mul(0x85EB_CA6B)
        .wrapping_add(px as u64)
        .wrapping_mul(0xC2B2_AE35)
        .wrapping_add(py as u64);
    h ^= h >> 16;
    h = h.wrapping_mul(0x27D4_EB2D);
    h ^ (h >> 15)
}

struct MenuLight {
    x: f64,
    y: f64,
    color: Rgb,
    radius: f64,
    strength: f32,
    flicker: f32,
}

fn menu_lights(
    tick: u64,
    color_filter: u8,
    low_motion: bool,
    reduced_flash: bool,
) -> Vec<MenuLight> {
    let mut lights = Vec::with_capacity(3);
    for y in 0..MENU_MAP_H {
        for x in 0..MENU_MAP_W {
            let (color, radius, strength) = match menu_tile(x, y) {
                Some('T') => ((246, 158, 66), 4.2, 0.5),
                Some('C') => ((255, 214, 90), 3.4, 0.46),
                _ => continue,
            };
            let flicker = if low_motion {
                1.0
            } else {
                let amplitude = if reduced_flash { 0.05 } else { 0.15 };
                let hash = menu_hash(x * 8, y * 8, (tick / 3) as i32, 0);
                1.0 - amplitude + amplitude * 2.0 * (hash % 100) as f32 / 100.0
            };
            lights.push(MenuLight {
                x: x as f64 + 0.5,
                y: y as f64 + 0.5,
                color: visual::grade_art(color_filter, color),
                radius,
                strength,
                flicker,
            });
        }
    }
    lights
}

fn menu_light_at(lights: &[MenuLight], wx: f64, wy: f64, base: Rgb) -> Rgb {
    let mut lit = base;
    for light in lights {
        let distance = ((wx - light.x).powi(2) + (wy - light.y).powi(2)).sqrt();
        let falloff = ((light.radius - distance) / light.radius).clamp(0.0, 1.0) as f32;
        lit = visual::lerp(lit, light.color, falloff * light.strength * light.flicker);
    }
    lit
}

/// Render the animated dungeon vista that backdrops the main menu.
pub fn draw_menu_scene(
    canvas: &mut HalfBlockCanvas,
    tick: u64,
    color_filter: u8,
    low_motion: bool,
    reduced_flash: bool,
) {
    let width = canvas.width() as i32;
    let height = canvas.pixel_height() as i32;
    if width < 4 || height < 4 {
        return;
    }
    let palette = visual::world_palette(color_filter, Biome::Depth1);
    let fog_rgb = palette.fog;
    canvas.fill(Color::Rgb(fog_rgb.0, fog_rgb.1, fog_rgb.2));

    // Camera stands at the corridor mouth, swaying gently as if held.
    let sway = if low_motion {
        0.0
    } else {
        (tick as f64 / 140.0 * std::f64::consts::TAU).sin() * 0.10
    };
    let drift = if low_motion {
        0.0
    } else {
        (tick as f64 / 240.0 * std::f64::consts::TAU).cos() * 0.15
    };
    let cam_x = 8.0 + drift;
    let cam_y = 9.4;
    let angle = -std::f64::consts::FRAC_PI_2 + sway;
    let dir_x = angle.cos();
    let dir_y = angle.sin();
    let plane_scale = (33.0_f64.to_radians()).tan();
    let plane_x = -dir_y * plane_scale;
    let plane_y = dir_x * plane_scale;
    let horizon = (height as f64 * 0.44).round() as i32;
    let max_depth = 12.0_f64;
    let lights = menu_lights(tick, color_filter, low_motion, reduced_flash);

    // ── Floor and ceiling ──
    let projection = height as f64 * 0.38;
    for screen_y in 0..height {
        let ceiling = screen_y < horizon;
        let row_depth = projection / (screen_y - horizon).abs().max(1) as f64;
        let fog_t = (row_depth / max_depth).clamp(0.0, 1.0);
        let fog = (fog_t * fog_t * (3.0 - 2.0 * fog_t)) as f32;
        for screen_x in 0..width {
            let t = screen_x as f64 / width as f64;
            let ray_x = dir_x - plane_x + 2.0 * plane_x * t;
            let ray_y = dir_y - plane_y + 2.0 * plane_y * t;
            let wx = cam_x + ray_x * row_depth;
            let wy = cam_y + ray_y * row_depth;
            let hash = menu_hash(wx.floor() as i32, wy.floor() as i32, screen_x, screen_y);
            let local_x = wx.rem_euclid(1.0);
            let local_y = wy.rem_euclid(1.0);
            let seam = local_x < 0.04 || local_y < 0.04;
            let base = if ceiling {
                visual::darken(palette.wall, 0.30)
            } else {
                palette.floor
            };
            let variation = if seam {
                0.6
            } else {
                0.86 + (hash % 8) as f32 * 0.03
            };
            let lit = menu_light_at(&lights, wx, wy, base);
            let shaded = visual::lerp(
                (
                    (lit.0 as f32 * variation) as u8,
                    (lit.1 as f32 * variation) as u8,
                    (lit.2 as f32 * variation) as u8,
                ),
                fog_rgb,
                fog * 0.85,
            );
            canvas.set_pixel(
                screen_x as usize,
                screen_y as usize,
                Color::Rgb(shaded.0, shaded.1, shaded.2),
            );
        }
    }

    // ── Walls (DDA per column) with a depth buffer for the crown ──
    let mut depth_buffer = vec![max_depth; width as usize];
    for screen_x in 0..width {
        let camera_x = 2.0 * screen_x as f64 / width as f64 - 1.0;
        let ray_x = dir_x + plane_x * camera_x;
        let ray_y = dir_y + plane_y * camera_x;
        let mut map_x = cam_x.floor() as i32;
        let mut map_y = cam_y.floor() as i32;
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
            (-1, (cam_x - map_x as f64) * delta_x)
        } else {
            (1, (map_x as f64 + 1.0 - cam_x) * delta_x)
        };
        let (step_y, mut side_y) = if ray_y < 0.0 {
            (-1, (cam_y - map_y as f64) * delta_y)
        } else {
            (1, (map_y as f64 + 1.0 - cam_y) * delta_y)
        };
        let mut hit = None;
        for _ in 0..48 {
            let side = if side_x < side_y {
                side_x += delta_x;
                map_x += step_x;
                false
            } else {
                side_y += delta_y;
                map_y += step_y;
                true
            };
            if menu_tile(map_x, map_y) == Some('#') {
                let depth = if side {
                    (map_y as f64 - cam_y + (1 - step_y) as f64 / 2.0) / ray_y
                } else {
                    (map_x as f64 - cam_x + (1 - step_x) as f64 / 2.0) / ray_x
                };
                if depth > max_depth {
                    break;
                }
                hit = Some((depth.max(0.02), side));
                break;
            }
        }
        let Some((depth, side)) = hit else {
            continue;
        };
        depth_buffer[screen_x as usize] = depth;
        let wall_height = ((height as f64 * 0.78) / depth.max(0.08)).round() as i32;
        let top = (horizon - wall_height / 2).max(0);
        let bottom = (horizon + wall_height / 2).min(height - 1);
        let wall_coord = if side {
            cam_x + depth * ray_x
        } else {
            cam_y + depth * ray_y
        };
        let texture_x = wall_coord.rem_euclid(1.0);
        let fog_t = (depth / max_depth).clamp(0.0, 1.0);
        let fog = (fog_t * fog_t * (3.0 - 2.0 * fog_t)) as f32;
        for screen_y in top..=bottom {
            let texture_y = (screen_y - top) as f64 / (bottom - top + 1).max(1) as f64;
            let mortar = (texture_x * 6.0).fract() < 0.10 || (texture_y * 7.0).fract() < 0.09;
            let course_offset = ((texture_y * 7.0).floor() as i32 & 1) as f64 * 0.5;
            let joint = (texture_x * 6.0 + course_offset).fract() < 0.08;
            let mut shade = if mortar || joint { 0.55 } else { 0.95 };
            if side {
                shade *= 0.72;
            }
            let mid = (1.0 - (2.0 * texture_y - 1.0).abs()) as f32;
            shade *= 0.86 + 0.14 * mid;
            if texture_y > 0.85 {
                shade *= 1.0 - 0.30 * ((texture_y - 0.85) / 0.15) as f32;
            }
            let lit = menu_light_at(
                &lights,
                cam_x + depth * ray_x,
                cam_y + depth * ray_y,
                (
                    (palette.wall.0 as f32 * shade) as u8,
                    (palette.wall.1 as f32 * shade) as u8,
                    (palette.wall.2 as f32 * shade) as u8,
                ),
            );
            let shaded = visual::lerp(lit, fog_rgb, fog * 0.88);
            canvas.set_pixel(
                screen_x as usize,
                screen_y as usize,
                Color::Rgb(shaded.0, shaded.1, shaded.2),
            );
        }
        for i in 1..=3i32 {
            canvas.tint_pixel(screen_x, bottom + i, (5, 6, 8), 0.30 - i as f32 * 0.08);
        }
    }

    // ── Torch flames on the side alcoves ──
    for y in 0..MENU_MAP_H {
        for x in 0..MENU_MAP_W {
            if menu_tile(x, y) != Some('T') {
                continue;
            }
            let (wx, wy) = (x as f64 + 0.5, y as f64 + 0.5);
            let (Some(cx), Some(cy)) = menu_project(
                wx, wy, cam_x, cam_y, dir_x, dir_y, plane_x, plane_y, width, horizon, height,
            ) else {
                continue;
            };
            let depth = (wx - cam_x) * dir_x + (wy - cam_y) * dir_y;
            let size = ((height as f64 * 0.30) / depth.max(0.4)).round() as i32;
            let phase = (tick / 3 + menu_hash(x, y, 0, 0)) % 4;
            let flame = visual::grade_art(color_filter, (255, 196, 84));
            let core = visual::grade_art(color_filter, (255, 240, 170));
            for dy in 0..size.max(1) {
                let half = (size - dy).max(1) / 2 + i32::from(phase == 1 && dy == 0);
                for dx in -half..=half {
                    let px = cx + dx;
                    if px >= 0
                        && (px as usize) < depth_buffer.len()
                        && depth < depth_buffer[px as usize]
                    {
                        let c = if dy > size / 2 { core } else { flame };
                        canvas.pixel(px, cy - size + dy, Color::Rgb(c.0, c.1, c.2));
                    }
                }
            }
        }
    }

    // ── The Crown, lit in its alcove ──
    let crown_pos = (8.5, 4.0);
    let crown_depth = (crown_pos.0 - cam_x) * dir_x + (crown_pos.1 - cam_y) * dir_y;
    if let (Some(cx), Some(cy)) = menu_project(
        crown_pos.0,
        crown_pos.1,
        cam_x,
        cam_y,
        dir_x,
        dir_y,
        plane_x,
        plane_y,
        width,
        horizon,
        height,
    ) {
        let size = ((height as f64 * 0.55) / crown_depth.max(0.4)).round() as i32;
        let gold = visual::grade_art(color_filter, (255, 214, 90));
        let gold_hi = visual::grade_art(color_filter, (255, 244, 180));
        let pedestal = visual::darken(palette.wall, 0.7);
        let glow_pulse = if reduced_flash || low_motion {
            0.5
        } else {
            ((tick as f64 / 50.0 * std::f64::consts::TAU).sin() * 0.5 + 0.5) * 0.8
        };
        // Halo
        let halo_r = size + (glow_pulse * 3.0) as i32;
        for dy in -halo_r..=halo_r {
            for dx in -halo_r..=halo_r {
                let d2 = dx * dx + dy * dy;
                if d2 > halo_r * halo_r || d2 < size * size / 4 {
                    continue;
                }
                let px = cx + dx;
                if px >= 0
                    && (px as usize) < depth_buffer.len()
                    && crown_depth < depth_buffer[px as usize]
                {
                    canvas.tint_pixel(
                        px,
                        cy + dy,
                        (gold.0, gold.1, gold.2),
                        (0.10 + 0.10 * glow_pulse) as f32,
                    );
                }
            }
        }
        // Pedestal block
        for dy in 0..size {
            let half = size / 2 - dy / 3;
            for dx in -half..=half {
                let px = cx + dx;
                if px >= 0
                    && (px as usize) < depth_buffer.len()
                    && crown_depth < depth_buffer[px as usize]
                {
                    canvas.pixel(px, cy + dy, Color::Rgb(pedestal.0, pedestal.1, pedestal.2));
                }
            }
        }
        // Crown silhouette: band with three points
        let band_y = cy - size / 4;
        for dy in 0..size / 4 + 1 {
            for dx in -(size / 2)..=(size / 2) {
                let px = cx + dx;
                if px >= 0
                    && (px as usize) < depth_buffer.len()
                    && crown_depth < depth_buffer[px as usize]
                {
                    canvas.pixel(px, band_y + dy, Color::Rgb(gold.0, gold.1, gold.2));
                }
            }
        }
        for (offset, point_h) in [(-(size / 3), size / 3), (0, size / 2), (size / 3, size / 3)] {
            for dy in 0..point_h.max(1) {
                let half = (point_h - dy).max(1) / 3;
                for dx in -half..=half {
                    let px = cx + offset + dx;
                    if px >= 0
                        && (px as usize) < depth_buffer.len()
                        && crown_depth < depth_buffer[px as usize]
                    {
                        let c = if dy >= point_h - 1 { gold_hi } else { gold };
                        canvas.pixel(px, band_y - dy, Color::Rgb(c.0, c.1, c.2));
                    }
                }
            }
        }
    }

    // ── Drifting motes ──
    let mote_count = if reduced_flash { 5 } else { 10 };
    for index in 0..mote_count {
        let hash = menu_hash(index * 7 + 1, 13, 37, 91);
        let x = 2 + (hash % (width - 4).max(1) as u64) as i32;
        let drift_y = if low_motion {
            0
        } else {
            ((tick / 4 + index as u64 * 7) % (height as u64 - 4).max(1)) as i32
        };
        let y = 2
            + (((hash >> 16) % (height - 4).max(1) as u64) as i32 + drift_y)
                .rem_euclid((height - 4).max(1));
        let mote = visual::darken(palette.light, 0.55);
        canvas.pixel(x, y, Color::Rgb(mote.0, mote.1, mote.2));
    }

    canvas.vignette(0.42);
}

#[allow(clippy::too_many_arguments)]
fn menu_project(
    wx: f64,
    wy: f64,
    cam_x: f64,
    cam_y: f64,
    dir_x: f64,
    dir_y: f64,
    plane_x: f64,
    plane_y: f64,
    width: i32,
    horizon: i32,
    height: i32,
) -> (Option<i32>, Option<i32>) {
    let rel_x = wx - cam_x;
    let rel_y = wy - cam_y;
    let inv_det = 1.0 / (plane_x * dir_y - dir_x * plane_y);
    let transform_x = inv_det * (dir_y * rel_x - dir_x * rel_y);
    let transform_y = inv_det * (-plane_y * rel_x + plane_x * rel_y);
    if transform_y <= 0.05 {
        return (None, None);
    }
    let screen_x = ((width as f64 / 2.0) * (1.0 + transform_x / transform_y)) as i32;
    let size = (height as f64 / transform_y).round() as i32;
    let bottom = horizon + size / 2;
    (Some(screen_x), Some(bottom))
}

// ══════════════════════════════════════════════════════════════════════════════
// PROCEDURAL TITLE ART (Crown + Dungeon scene in half-block)
// ══════════════════════════════════════════════════════════════════════════════

/// Generate a procedural title screen art.
#[allow(dead_code)]
pub fn draw_title_art(canvas: &mut HalfBlockCanvas, tick: u64) {
    let w = canvas.width;
    let h = canvas.height;
    // Use half-block pixel height for vertical calculations
    let _pixel_h = h * 2;
    let pixel_h = _pixel_h;

    // Dark stone background gradient
    for y in 0..pixel_h {
        let t = y as f32 / pixel_h as f32;
        let r = (18.0 + 6.0 * t) as u8;
        let g = (16.0 + 4.0 * t) as u8;
        let b = (24.0 + 8.0 * t) as u8;
        let bg = Color::Rgb(r, g, b);
        for x in 0..w {
            canvas.set_pixel(x, y, bg);
        }
    }

    // Draw a stylized crown in the center
    let cx = w / 2;
    let cy = pixel_h / 3;
    // Crown shape: triangular points atop a rectangular band
    let sparkle = (tick as f32 / 30.0 * std::f32::consts::TAU).sin() * 0.3 + 0.7;
    let crown_r = (45.0 * sparkle) as u8;
    let crown_g = (38.0 * sparkle) as u8;
    let crown_b = (5.0 * sparkle) as u8;
    let glow_gold = Color::Rgb(crown_r, crown_g, crown_b);

    // Band (horizontal bar)
    let band_y = cy + 8;
    let band_h: usize = 3;
    for y in band_y..band_y + band_h {
        for x in cx - 10..cx + 10 {
            if x > cx - 11 + (y - band_y) && x < cx + 11 - (y - band_y) {
                canvas.set_pixel(x, y, glow_gold);
            }
        }
    }

    // Crown points (three triangles) on the band
    for p in [-8i32, 0, 8] {
        let px = (cx as i32 + p) as usize;
        for y in cy..band_y {
            let h2 = (band_y - y) as i32;
            let half_w = h2 / 2;
            for dx in -half_w..=half_w {
                canvas.set_pixel((px as i32 + dx) as usize, y, glow_gold);
            }
        }
    }

    // Sparkle dots on crown tips
    if tick % 3 == 0 {
        for p in [-8i32, 0, 8] {
            let px = (cx as i32 + p) as usize;
            canvas.set_pixel(px, cy, Color::Rgb(255, 255, 200));
        }
    }

    // Ground line at bottom
    let ground_y = pixel_h * 3 / 4;
    canvas.h_line(0, ground_y, w, Color::Rgb(40, 38, 30));
    canvas.h_line(0, ground_y + 1, w, Color::Rgb(30, 28, 22));
}

/// Generate procedural victory art.
pub fn draw_victory_art(canvas: &mut HalfBlockCanvas, tick: u64) {
    let w = canvas.width;
    let h = canvas.height;
    let pixel_h = h * 2;

    // Warm golden background
    for y in 0..pixel_h {
        let t = y as f32 / pixel_h as f32;
        let r = (20.0 + 15.0 * t) as u8;
        let g = (16.0 + 10.0 * t) as u8;
        let b = (10.0 + 5.0 * t) as u8;
        let bg = Color::Rgb(r, g, b);
        for x in 0..w {
            canvas.set_pixel(x, y, bg);
        }
    }

    let cx = w / 2;
    let cy = pixel_h / 3;
    let pulse = (tick as f32 / 40.0 * std::f32::consts::TAU).sin() * 0.4 + 0.6;
    let gold = Color::Rgb(
        (255.0 * pulse) as u8,
        (215.0 * pulse) as u8,
        (30.0 * pulse) as u8,
    );

    // Large star shape ★
    let star_y = cy;
    for dy in -4i32..=4 {
        let w2 = 4 - dy.abs();
        for dx in -w2..=w2 {
            if dy.abs() > 2 || dx.abs() < 2 {
                canvas.set_pixel(
                    (cx as i32 + dx) as usize,
                    (star_y as i32 + dy) as usize,
                    gold,
                );
            }
        }
    }

    // Crown above star — use saturating_sub to avoid underflow on small canvases
    let crown_start = star_y.saturating_sub(10);
    let crown_end = star_y.saturating_sub(2);
    if crown_start < crown_end {
        for y in crown_start..crown_end {
            for x in cx - 6..cx + 6 {
                let dist =
                    (x as i32 - cx as i32).abs() + (y as i32 - (star_y as i32 - 6)).abs() * 2;
                if dist < 8 {
                    canvas.set_pixel(x, y, gold);
                }
            }
        }
    }

    // Rays emanating
    let rays = 8;
    for i in 0..rays {
        let angle = (i as f32 / rays as f32) * std::f32::consts::TAU;
        let dx = (angle.sin() * 12.0) as i32;
        let dy = (angle.cos() * 12.0) as i32;
        let rx = (cx as i32 + dx) as usize;
        let ry = (star_y as i32 + dy) as usize;
        canvas.set_pixel(rx, ry, Color::Rgb(255, 200, 80));
    }
}

/// Generate procedural death art.
pub fn draw_death_art(canvas: &mut HalfBlockCanvas, _tick: u64) {
    let w = canvas.width;
    let h = canvas.height;
    let pixel_h = h * 2;

    // Dark red-black gradient
    for y in 0..pixel_h {
        let t = y as f32 / pixel_h as f32;
        let r = (20.0 - 8.0 * t) as u8;
        let g = 6u8;
        let b = (8.0 + 2.0 * t) as u8;
        let bg = Color::Rgb(r, g, b);
        for x in 0..w {
            canvas.set_pixel(x, y, bg);
        }
    }

    let cx = w / 2;
    let cy = pixel_h / 3;
    let red = Color::Rgb(180, 30, 30);
    let dim_red = Color::Rgb(100, 15, 15);

    // Skull shape (ASCII-style in half-block)
    // Head circle
    for y in cy - 5..cy + 6 {
        let dy2 = (y as i32 - cy as i32).pow(2);
        for x in cx - 6..cx + 6 {
            let dx2 = (x as i32 - cx as i32).pow(2);
            let dist2 = dx2 + dy2;
            if dist2 < 36 && dist2 > 16 {
                canvas.set_pixel(x, y, red);
            } else if dist2 <= 16 {
                canvas.set_pixel(x, y, Color::Rgb(25, 5, 5));
            }
        }
    }

    // Eyes
    for y in cy - 2..cy {
        canvas.set_pixel(cx - 2, y, Color::Black);
        canvas.set_pixel(cx + 2, y, Color::Black);
    }

    // Bottom teeth line
    canvas.h_line(cx - 4, cy + 4, 8, dim_red);
    canvas.h_line(cx - 4, cy + 5, 8, dim_red);

    // Cross bones below
    let bone_y = cy + 8;
    let bone_color = Color::Rgb(140, 20, 20);
    for i in 0..6 {
        canvas.set_pixel(cx - 4 + i, bone_y - i, bone_color);
        canvas.set_pixel(cx + 4 - i, bone_y - i, bone_color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchored_frame_validates_attachment_points() {
        let valid = AnchoredFrame::new(&["###", "###", "###"], (1, 3), (2, 1), (0, 1));
        let invalid = AnchoredFrame::new(&["###", "###", "###"], (1, 3), (3, 1), (0, 1));

        assert!(valid.anchors_in_bounds());
        assert!(!invalid.anchors_in_bounds());
    }

    #[test]
    fn tint_pixel_blends_rgb_and_clips_signed_coordinates() {
        let mut canvas = HalfBlockCanvas::new(2, 1);
        canvas.pixel(0, 0, Color::Rgb(100, 100, 100));
        canvas.tint_pixel(0, 0, (0, 0, 0), 0.5);
        canvas.tint_pixel(-1, 0, (255, 255, 255), 1.0);

        assert_eq!(canvas.pixels[0], Color::Rgb(50, 50, 50));
    }

    #[test]
    fn render_packs_independent_upper_and_lower_colors() {
        let mut canvas = HalfBlockCanvas::new(1, 1);
        canvas.set_pixel(0, 0, Color::Rgb(1, 2, 3));
        canvas.set_pixel(0, 1, Color::Rgb(4, 5, 6));
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));

        canvas.render(&mut buffer, Rect::new(0, 0, 1, 1));

        let cell = &buffer[(0, 0)];
        assert_eq!(cell.symbol(), "\u{2580}");
        assert_eq!(cell.fg, Color::Rgb(1, 2, 3));
        assert_eq!(cell.bg, Color::Rgb(4, 5, 6));
    }

    #[test]
    fn black_sprite_pixel_is_opaque_and_transparent_pixel_is_not() {
        let mut canvas = HalfBlockCanvas::new(2, 1);
        canvas.fill(Color::Rgb(9, 9, 9));
        canvas.masked_sprite(0, 0, 2, &[Some(Color::Black), None]);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 2, 1));

        canvas.render(&mut buffer, Rect::new(0, 0, 2, 1));

        assert_eq!(buffer[(0, 0)].fg, Color::Black);
        assert_eq!(buffer[(1, 0)].fg, Color::Rgb(9, 9, 9));
    }

    #[test]
    fn signed_drawing_clips_tiny_canvas_operations() {
        let mut canvas = HalfBlockCanvas::new(1, 1);
        canvas.fill(Color::Rgb(1, 1, 1));
        canvas.rect(-2, -2, 3, 3, Color::Rgb(2, 2, 2));
        canvas.line(-4, 1, 4, 1, Color::Rgb(3, 3, 3));
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));

        canvas.render(&mut buffer, Rect::new(0, 0, 1, 1));

        assert_eq!(buffer[(0, 0)].fg, Color::Rgb(2, 2, 2));
        assert_eq!(buffer[(0, 0)].bg, Color::Rgb(3, 3, 3));
    }

    #[test]
    fn high_contrast_moves_dark_and_light_pixels_apart() {
        let mut canvas = HalfBlockCanvas::new(1, 1);
        canvas.set_pixel(0, 0, Color::Rgb(96, 96, 96));
        canvas.set_pixel(0, 1, Color::Rgb(160, 160, 160));
        canvas.increase_contrast(0.5);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
        canvas.render(&mut buffer, Rect::new(0, 0, 1, 1));
        assert_eq!(buffer[(0, 0)].fg, Color::Rgb(80, 80, 80));
        assert_eq!(buffer[(0, 0)].bg, Color::Rgb(176, 176, 176));
    }

    #[test]
    fn vignette_darkens_corners_without_crushing_the_center() {
        let mut canvas = HalfBlockCanvas::new(9, 5);
        canvas.fill(Color::Rgb(100, 100, 100));

        canvas.vignette(0.5);

        let corner = canvas.pixels[0];
        let center = canvas.pixels[4 + 5 * 9];
        assert!(matches!((corner, center), (
            Color::Rgb(c, _, _),
            Color::Rgb(m, _, _)
        ) if c < m && m == 100));
    }

    #[test]
    fn menu_scene_has_depth_and_clips_on_tiny_canvases() {
        let mut scene = HalfBlockCanvas::new(80, 40);
        draw_menu_scene(&mut scene, 24, 0, false, false);
        let colors = scene
            .pixels
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        assert!(
            colors.len() > 24,
            "menu vista should contain real material depth"
        );

        let mut tiny = HalfBlockCanvas::new(1, 1);
        draw_menu_scene(&mut tiny, 24, 0, false, false);
        assert_eq!(tiny.pixels.len(), 2);
    }

    #[test]
    fn screen_blend_pixel_brightens_without_clipping_flat_white() {
        let mut canvas = HalfBlockCanvas::new(3, 2);
        canvas.set_pixel(0, 0, Color::Rgb(40, 40, 40));
        // A strong add should lift the pixel but not slam it to 255.
        canvas.screen_blend_pixel(0, 0, (255, 255, 255), 0.5);
        let Color::Rgb(r, g, b) = canvas.pixels[0] else {
            panic!("expected rgb pixel");
        };
        assert!(r > 40 && g > 40 && b > 40, "should brighten");
        assert!((r, g, b) != (255, 255, 255), "screen blend must not clip");
    }

    #[test]
    fn glow_lifts_center_and_leaves_distant_corner_untouched() {
        let mut canvas = HalfBlockCanvas::new(11, 6);
        canvas.fill(Color::Rgb(10, 10, 10));
        let center = (5, 5); // py=5 is the bottom half-block pixel of cell_y=2
        canvas.glow(5, 5, 3.0, (200, 200, 200), 0.6);
        let lit = match canvas.pixels[5 + center.1 as usize * 11] {
            Color::Rgb(r, _, _) => r,
            _ => 0,
        };
        assert!(lit > 10, "center should glow");
        // Far corner is well outside the radius and must stay at the base fill.
        let corner = match canvas.pixels[0] {
            Color::Rgb(r, _, _) => r,
            _ => 255,
        };
        assert_eq!(corner, 10, "outside radius must be untouched");
    }

    #[test]
    fn bloom_lifts_bright_pixels_and_bleeds_to_neighbours() {
        let mut canvas = HalfBlockCanvas::new(5, 4);
        canvas.fill(Color::Rgb(5, 5, 5));
        // One bright seed in the middle.
        canvas.set_pixel(2, 2, Color::Rgb(250, 250, 250));
        canvas.bloom(0.6, 0.5);
        let seed = match canvas.pixels[2 + 2 * 5] {
            Color::Rgb(r, _, _) => r,
            _ => 0,
        };
        assert!(seed >= 250, "bright seed stays bright");
        // A direct orthogonal neighbour should have lifted off the 5 base.
        let neighbour = match canvas.pixels[2 + 5] {
            Color::Rgb(r, _, _) => r,
            _ => 0,
        };
        assert!(neighbour > 5, "bloom should bleed into neighbours");
    }

    #[test]
    fn luminous_primitives_noop_on_tiny_canvas() {
        let mut canvas = HalfBlockCanvas::new(1, 1);
        // Should not panic even when the glow/bloom bounds collapse.
        canvas.glow(0, 0, 0.0, (200, 200, 200), 0.5);
        canvas.glow(0, 0, 2.0, (200, 200, 200), 0.5);
        canvas.bloom(0.0, 0.0);
        canvas.screen_blend_pixel(-1, -1, (10, 10, 10), 1.0);
        assert_eq!(canvas.pixels.len(), 2);
    }
}
