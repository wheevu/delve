use crate::entity::ItemKind;
use crate::image_render::{HalfBlockCanvas, SpriteFrame};
use crate::visual::{self, Rgb};
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::Frame;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ItemAppearance {
    Known(ItemKind),
    UnknownPotion,
    UnknownScroll,
    UnknownBomb,
}

#[derive(Clone, Copy)]
struct ItemArt {
    frame: SpriteFrame,
    primary: Rgb,
    secondary: Rgb,
    highlight: Rgb,
}

pub(crate) fn appearance(kind: ItemKind, known: bool) -> ItemAppearance {
    if known || !kind.requires_identification() {
        ItemAppearance::Known(kind)
    } else if matches!(
        kind,
        ItemKind::HealthPotion
            | ItemKind::LargeHealthPotion
            | ItemKind::Antidote
            | ItemKind::LanternOil
    ) {
        ItemAppearance::UnknownPotion
    } else if kind == ItemKind::SmokeBomb {
        ItemAppearance::UnknownBomb
    } else {
        ItemAppearance::UnknownScroll
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_billboard_item(
    canvas: &mut HalfBlockCanvas,
    center_x: i32,
    bottom_y: i32,
    pixel_size: i32,
    appearance: ItemAppearance,
    tick: u64,
    palette_mode: u8,
    depth: f64,
    wall_depth: &[f64],
) {
    let art = art(appearance);
    if pixel_size < 6 {
        draw_far_mark(
            canvas,
            center_x,
            bottom_y,
            art,
            tick,
            palette_mode,
            depth,
            wall_depth,
        );
        return;
    }
    let scale = (pixel_size / art.frame.height().max(1) as i32).max(1);
    let width = art.frame.width() as i32 * scale;
    let height = art.frame.height() as i32 * scale;
    let x = center_x - width / 2;
    let y = bottom_y - height;
    draw(
        canvas,
        x,
        y,
        scale,
        art,
        tick,
        palette_mode,
        Some((depth, wall_depth)),
    );
}

pub(crate) fn render_preview(
    frame: &mut Frame<'_>,
    area: Rect,
    appearance: ItemAppearance,
    tick: u64,
    palette_mode: u8,
) {
    if area.width < 12 || area.height < 5 {
        return;
    }
    let mut canvas = HalfBlockCanvas::new(area.width as usize, area.height as usize);
    let art = art(appearance);
    let background = visual::grade_art(palette_mode, (15, 17, 24));
    canvas.fill(color(background));
    let glow = visual::darken(visual::grade_art(palette_mode, art.primary), 0.22);
    let cx = canvas.width() as i32 / 2;
    let cy = canvas.pixel_height() as i32 / 2;
    for radius in (2..=8).rev() {
        let amount = 0.12 + (8 - radius) as f32 * 0.025;
        let halo = visual::lerp(background, glow, amount);
        canvas.line(cx - radius, cy, cx + radius, cy, color(halo));
    }
    let scale = if canvas.pixel_height() >= 14 { 2 } else { 1 };
    let width = art.frame.width() as i32 * scale;
    let height = art.frame.height() as i32 * scale;
    let x = cx - width / 2;
    let y = cy - height / 2;
    canvas.rect(
        cx - width / 3,
        (y + height - scale).min(canvas.pixel_height() as i32 - 1),
        width * 2 / 3,
        scale,
        color((8, 9, 12)),
    );
    draw(&mut canvas, x, y, scale, art, tick, palette_mode, None);
    canvas.render(frame.buffer_mut(), area);
}

#[allow(clippy::too_many_arguments)]
fn draw(
    canvas: &mut HalfBlockCanvas,
    x: i32,
    y: i32,
    scale: i32,
    art: ItemArt,
    tick: u64,
    palette_mode: u8,
    depth_clip: Option<(f64, &[f64])>,
) {
    let pulse = item_pulse(art, tick);
    let primary = visual::grade_art(palette_mode, art.primary);
    let secondary = visual::grade_art(palette_mode, art.secondary);
    let highlight = visual::grade_art(
        palette_mode,
        visual::lerp(art.highlight, (255, 246, 218), pulse),
    );
    let extrusion = (scale / 2).clamp(1, 2);
    for (row, pixels) in art.frame.rows.iter().enumerate() {
        for (column, pixel) in pixels.chars().enumerate() {
            let rgb = match pixel {
                '#' => Some(primary),
                '+' | '*' => Some(secondary),
                '.' => Some(visual::darken(primary, 0.34)),
                _ => None,
            };
            let Some(rgb) = rgb else { continue };
            for layer in (1..=extrusion).rev() {
                let offset_x = layer;
                let offset_y = (layer / 2).max(1);
                let x = x + column as i32 * scale + offset_x;
                let y = y + row as i32 * scale + offset_y;
                for py in 0..scale {
                    for px in 0..scale {
                        let screen_x = x + px;
                        if depth_visible(screen_x, depth_clip) {
                            canvas.pixel(
                                screen_x,
                                y + py,
                                color(visual::darken(rgb, 0.42 + layer as f32 * 0.08)),
                            );
                        }
                    }
                }
            }
        }
    }
    for (row, pixels) in art.frame.rows.iter().enumerate() {
        for (column, pixel) in pixels.chars().enumerate() {
            let rgb = match pixel {
                '#' => Some(primary),
                '+' => Some(secondary),
                '*' => Some(highlight),
                '.' => Some(visual::darken(primary, 0.34)),
                _ => None,
            };
            let Some(rgb) = rgb else { continue };
            for py in 0..scale {
                for px in 0..scale {
                    let screen_x = x + column as i32 * scale + px;
                    if depth_visible(screen_x, depth_clip) {
                        canvas.pixel(screen_x, y + row as i32 * scale + py, color(rgb));
                    }
                }
            }
        }
    }
}

fn depth_visible(screen_x: i32, depth_clip: Option<(f64, &[f64])>) -> bool {
    depth_clip.is_none_or(|(depth, wall_depth)| {
        screen_x >= 0
            && wall_depth
                .get(screen_x as usize)
                .is_some_and(|wall| depth < *wall)
    })
}

#[allow(clippy::too_many_arguments)]
fn draw_far_mark(
    canvas: &mut HalfBlockCanvas,
    center_x: i32,
    bottom_y: i32,
    art: ItemArt,
    phase: u64,
    palette_mode: u8,
    depth: f64,
    wall_depth: &[f64],
) {
    let primary = visual::grade_art(palette_mode, art.primary);
    let highlight = visual::grade_art(palette_mode, art.highlight);
    for (dx, dy, rgb) in [
        (0, -2, highlight),
        (-1, -1, primary),
        (0, -1, primary),
        (1, -1, primary),
        (0, 0, visual::darken(primary, 0.45)),
    ] {
        let x = center_x + dx;
        if x >= 0 && wall_depth.get(x as usize).is_some_and(|wall| depth < *wall) {
            canvas.pixel(x, bottom_y + dy - i32::from(phase > 44), color(rgb));
        }
    }
}

fn item_pulse(art: ItemArt, phase: u64) -> f32 {
    let material_bias = if art.highlight.0 > art.highlight.1.saturating_add(40) {
        0.18
    } else if art.primary.2 > art.primary.0 {
        0.12
    } else {
        0.07
    };
    let wave = match phase % 48 {
        0..=7 => 0.0,
        8..=15 => 0.35,
        16..=23 => 0.7,
        24..=31 => 1.0,
        32..=39 => 0.55,
        _ => 0.2,
    };
    0.03 + material_bias * wave
}

fn art(appearance: ItemAppearance) -> ItemArt {
    match appearance {
        ItemAppearance::UnknownPotion => sprite(
            &[
                "  +++  ", "  ...  ", " .***. ", ".####*.", ".####*.", " .###. ", "  ...  ",
            ],
            (76, 82, 94),
            (126, 94, 58),
            (194, 202, 212),
        ),
        ItemAppearance::UnknownScroll => sprite(
            &[
                ".+++++.", ".#####.", ".#.*.#.", ".#...#.", ".#.*.#.", ".#####.", ".+++++.",
            ],
            (136, 126, 102),
            (92, 78, 56),
            (208, 198, 166),
        ),
        ItemAppearance::UnknownBomb => sprite(
            &[
                "   +*  ", "  .+.  ", " .###. ", ".#####.", ".##*##.", " .###. ", "  ...  ",
            ],
            (72, 76, 84),
            (126, 84, 46),
            (196, 204, 212),
        ),
        ItemAppearance::Known(kind) => known_art(kind),
    }
}

fn known_art(kind: ItemKind) -> ItemArt {
    match kind {
        ItemKind::HealthPotion => sprite(
            &[
                "  +++  ", "  ...  ", " .***. ", ".##*##.", ".#####.", " .###. ", "  ...  ",
            ],
            (202, 42, 78),
            (108, 52, 40),
            (255, 164, 184),
        ),
        ItemKind::LargeHealthPotion => sprite(
            &[
                " +++++ ", " ..... ", ".*****.", ".##*##.", ".#####.", ".#####.", " ..... ",
            ],
            (218, 48, 84),
            (120, 54, 42),
            (255, 184, 198),
        ),
        ItemKind::Antidote => sprite(
            &[
                "   ++  ", "  .*.  ", " .##*. ", ".##*##.", ".#####.", " .###. ", "  ...  ",
            ],
            (46, 178, 88),
            (72, 72, 38),
            (174, 255, 190),
        ),
        ItemKind::LanternOil => sprite(
            &[
                "  ++++ ", "  .... ", " .***. ", ".####. ", ".####. ", " .##.  ", "  ..   ",
            ],
            (208, 126, 32),
            (92, 62, 34),
            (255, 222, 116),
        ),
        ItemKind::ScrollOfFire => sprite(
            &[
                ".+++++.", ".#####.", ".#.*.#.", ".#***#.", ".##*##.", ".#####.", ".+++++.",
            ],
            (188, 64, 30),
            (218, 184, 116),
            (255, 214, 48),
        ),
        ItemKind::ScrollOfConfusion => sprite(
            &[
                ".+++++.", ".#####.", ".#*#*#.", ".##*##.", ".#*#*#.", ".#####.", ".+++++.",
            ],
            (126, 62, 184),
            (210, 180, 116),
            (234, 154, 255),
        ),
        ItemKind::ScrollOfBlink => sprite(
            &[
                ".+++++.", ".##*##.", ".#*..#.", ".#.*.#.", ".#..*#.", ".##*##.", ".+++++.",
            ],
            (58, 112, 210),
            (210, 182, 120),
            (152, 224, 255),
        ),
        ItemKind::SmokeBomb => sprite(
            &[
                "   +*  ", "  .+.  ", " .###. ", ".#####.", ".##*##.", " .###. ", "  ...  ",
            ],
            (78, 84, 96),
            (146, 92, 44),
            (218, 224, 230),
        ),
        ItemKind::RustySword => sprite(
            &[
                "     * ", "    #. ", "   #.  ", "  #.   ", "+#+    ", " .     ", ".      ",
            ],
            (150, 108, 72),
            (92, 56, 32),
            (214, 186, 138),
        ),
        ItemKind::Longsword => sprite(
            &[
                "     * ", "    ## ", "   ##  ", "  ##   ", "+##+   ", " .     ", ".      ",
            ],
            (196, 204, 202),
            (106, 68, 38),
            (255, 255, 244),
        ),
        ItemKind::Warhammer => sprite(
            &[
                " +++++ ", ".#####.", " +###. ", "   #.  ", "   #.  ", "   #.  ", "   .   ",
            ],
            (160, 168, 166),
            (92, 62, 36),
            (234, 228, 208),
        ),
        ItemKind::EnchantmentScroll => sprite(
            &[
                ".*****.", ".##+##.", ".#+*+#.", ".+***+.", ".#+*+#.", ".##+##.", ".*****.",
            ],
            (184, 132, 40),
            (216, 188, 124),
            (255, 244, 144),
        ),
        ItemKind::LeatherArmor => sprite(
            &[
                " +   + ", ".#+ +#.", ".#####.", ".##*##.", " .###. ", " .#.#. ", " .. .. ",
            ],
            (136, 78, 42),
            (82, 48, 28),
            (218, 154, 88),
        ),
        ItemKind::Chainmail => sprite(
            &[
                " +   + ", ".#* *#.", ".*#*#*.", ".#*#*#.", " .*#*. ", " .#.#. ", " .. .. ",
            ],
            (132, 150, 160),
            (68, 82, 94),
            (228, 240, 242),
        ),
        ItemKind::Shield => sprite(
            &[
                ".+++++.", ".##*##.", ".#####.", ".##+##.", " .###. ", "  .#.  ", "   .   ",
            ],
            (70, 120, 174),
            (118, 72, 38),
            (188, 230, 255),
        ),
        ItemKind::CrownOfDepth => sprite(
            &[
                "*  *  *", "#*.#.*#", ".#####.", " .###. ", " +++++ ", " .***. ", "  ...  ",
            ],
            (234, 178, 34),
            (142, 52, 36),
            (255, 248, 150),
        ),
        ItemKind::LoreFragment => sprite(
            &[
                ".++++  ", ".####+ ", ".#.*#+ ", ".#..#+ ", ".#.*#+ ", ".####+ ", ".....  ",
            ],
            (178, 156, 98),
            (104, 70, 40),
            (246, 228, 166),
        ),
        ItemKind::BellToken => sprite(
            &[
                "   *   ", "  +#+  ", " .###. ", ".#####.", " .###. ", "  +#+  ", "   .   ",
            ],
            (166, 104, 34),
            (86, 52, 30),
            (248, 204, 100),
        ),
        ItemKind::IronKey => sprite(
            &[
                " ***   ", "*...*  ", "*.*.*  ", " ***#  ", "    #+ ", "    ##+", "     + ",
            ],
            (172, 152, 98),
            (88, 72, 42),
            (244, 228, 166),
        ),
        ItemKind::Emberbrand => sprite(
            &[
                "     * ", "    #* ", "   #*  ", "  #*   ", "+##+   ", " .     ", ".      ",
            ],
            (230, 66, 22),
            (102, 54, 28),
            (255, 218, 66),
        ),
        ItemKind::TideglassMail => sprite(
            &[
                " *   * ", ".#+ +#.", ".#*#*#.", ".*#+#*.", " .#*#. ", " .#.#. ", " .. .. ",
            ],
            (42, 162, 198),
            (42, 82, 118),
            (176, 248, 255),
        ),
        ItemKind::Graveward => sprite(
            &[
                ".*+ +*.", ".#####.", ".#*#*#.", ".##+##.", " .###. ", " .#.#. ", " .. .. ",
            ],
            (160, 154, 188),
            (66, 60, 92),
            (240, 234, 255),
        ),
    }
}

fn sprite(rows: &'static [&'static str], primary: Rgb, secondary: Rgb, highlight: Rgb) -> ItemArt {
    ItemArt {
        frame: SpriteFrame::new(rows),
        primary,
        secondary,
        highlight,
    }
}

fn color((red, green, blue): Rgb) -> Color {
    Color::Rgb(red, green, blue)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    const KINDS: [ItemKind; 22] = [
        ItemKind::HealthPotion,
        ItemKind::LargeHealthPotion,
        ItemKind::Antidote,
        ItemKind::LanternOil,
        ItemKind::ScrollOfFire,
        ItemKind::ScrollOfConfusion,
        ItemKind::ScrollOfBlink,
        ItemKind::SmokeBomb,
        ItemKind::RustySword,
        ItemKind::Longsword,
        ItemKind::Warhammer,
        ItemKind::EnchantmentScroll,
        ItemKind::LeatherArmor,
        ItemKind::Chainmail,
        ItemKind::Shield,
        ItemKind::CrownOfDepth,
        ItemKind::LoreFragment,
        ItemKind::BellToken,
        ItemKind::IronKey,
        ItemKind::Emberbrand,
        ItemKind::TideglassMail,
        ItemKind::Graveward,
    ];

    #[test]
    fn every_known_item_has_a_distinct_seven_pixel_art_signature() {
        let signatures = KINDS
            .into_iter()
            .map(|kind| {
                let art = art(ItemAppearance::Known(kind));
                assert_eq!(art.frame.width(), 7);
                assert_eq!(art.frame.height(), 7);
                format!("{:?}{:?}{:?}", art.frame.rows, art.primary, art.secondary)
            })
            .collect::<HashSet<_>>();
        assert_eq!(signatures.len(), KINDS.len());
    }

    #[test]
    fn unidentified_items_share_only_their_category_appearance() {
        assert_eq!(
            appearance(ItemKind::HealthPotion, false),
            appearance(ItemKind::Antidote, false)
        );
        assert_eq!(
            appearance(ItemKind::ScrollOfFire, false),
            appearance(ItemKind::ScrollOfBlink, false)
        );
        assert_ne!(
            appearance(ItemKind::HealthPotion, false),
            appearance(ItemKind::ScrollOfFire, false)
        );
        assert_eq!(
            appearance(ItemKind::HealthPotion, true),
            ItemAppearance::Known(ItemKind::HealthPotion)
        );
    }
}
