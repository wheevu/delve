// ══════════════════════════════════════════════════════════════════════════════
// BATTLE OVERLAY
//
// A turn-based combat scene drawn into the dungeon viewport while a battle is
// active. It keeps the normal HUD (sidebar, context strip, message log)
// untouched and only replaces the 3D viewport with large player/enemy figures,
// HP bars, a target readout, the command menu, and the latest battle message.
//
// ── REQUIRED CONTRACT ────────────────────────────────────────────────────────
// This module is compiled unconditionally; it only does anything when
// `RenderCtx` actually carries a `battle` reference at runtime. It reads the
// real `crate::battle::BattleState` (see src/battle.rs) plus the surrounding
// `RenderCtx` for the live player and monster stats:
//
//   crate::battle::BattleState {
//       pub enemy_indices:   Vec<usize>,  // indices into ctx.monsters
//       pub selected_target: usize,        // index into `enemy_indices`
//       pub command_cursor:  usize,        // selected command (0..=4)
//       pub phase:           BattlePhase,  // Input|Target|Resolve|Victory|Defeat|Fled
//       pub round:           u32,
//       pub message:         String,
//   }
//
// `RenderCtx` must additionally expose (per the integration brief):
//   pub battle: Option<&crate::battle::BattleState>
//
// Player/menace stats come from `ctx.player` and `ctx.monsters[idx]`:
//   ctx.player.stats.hp / .max_hp
//   monster.kind.name()                         (display name)
//   monster.stats.hp / .max_hp                 (HP)
//   monster.intent.map(|i| i.kind.label())     (intent, or "—")
//
// The command menu labels are mirrored locally from `BattleCommand` order
// (Fight, Technique, Item, Guard, Flee) so this renderer does not depend on
// helper methods that may or may not exist yet on `BattleCommand`.
//
// The renderer is inert (the `if let Some(battle) = ctx.battle` in render.rs
// falls through to the normal dungeon viewport) until the app agent adds the
// `battle` field to `RenderCtx`. Until then the crate will not fully compile,
// which the integration brief explicitly permits ("render compile may wait for
// integration").
// ══════════════════════════════════════════════════════════════════════════════

use crate::battle::{BattlePhase, BattleState};
use crate::entity::{ItemKind, Monster};
use crate::render::RenderCtx;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

const PLAYER_COLOR: Color = Color::Rgb(0, 255, 136);
const PLAYER_ACCENT: Color = Color::Rgb(120, 255, 200);
const ENEMY_COLOR: Color = Color::Rgb(255, 80, 80);
const ENEMY_ACCENT: Color = Color::Rgb(255, 210, 90);
const PANEL_BG: Color = Color::Rgb(14, 14, 20);
const TEXT_DIM: Color = Color::Rgb(120, 120, 130);
const TEXT_BRIGHT: Color = Color::Rgb(225, 225, 235);
const HP_FULL: Color = Color::Rgb(0, 255, 136);
const HP_MID: Color = Color::Rgb(255, 204, 0);
const HP_LOW: Color = Color::Rgb(255, 68, 68);
const CURSOR_COLOR: Color = Color::Rgb(255, 230, 120);

/// Mirrors `BattleCommand` order: Fight, Technique, Item, Guard, Flee.
const COMMAND_LABELS: &[&str] = &["Fight", "Technique", "Item", "Guard", "Flee"];

/// Hand-authored 12x12 silhouettes. `X` = main fill, `o` = accent (weapon /
/// eyes / heraldry), `.` = transparent. Drawn with half-block-safe glyphs.
const PLAYER_ART: &[&str] = &[
    "....XXXX....",
    "...XXXXXX...",
    "..XXXXXXXX..",
    "..XXoXXXX...",
    "..XXXXXXXX..",
    ".XXXXXXXXXX.",
    ".XX.XXXX.XX.",
    ".XX.XXXX.XX.",
    "...XX..XX...",
    "...XX..XX...",
    "..XXX..XXX..",
    "..XX....XX..",
];

const ENEMY_ART: &[&str] = &[
    "..XXXXXXXX..",
    ".XXXXXXXXXX.",
    "XXXXXXXXXXXX",
    "XXoXXXXXXoXX",
    "XXXXXXXXXXXX",
    "XXXXXXXXXXXX",
    "XXXXXXXXXXXX",
    ".XXXXXXXXXX.",
    "..XX.XX.XX..",
    "..XX.XX.XX..",
    ".XXX.XX.XXX.",
    ".XX......XX.",
];

/// Entry point called from `render.rs` when `ctx.battle` is `Some`.
pub fn render_battle_overlay(frame: &mut Frame, area: Rect, ctx: &RenderCtx, battle: &BattleState) {
    if area.width < 20 || area.height < 12 {
        // Too cramped for the full scene: show a compact status instead of
        // risking a clip. The 80x24 layout never triggers this.
        let compact = Paragraph::new(format!(
            "BATTLE r{}  You {}/{}  Foes {}  {}",
            battle.round,
            ctx.player.stats.hp,
            ctx.player.stats.max_hp,
            battle.enemy_indices.len(),
            battle.message
        ))
        .style(Style::default().fg(TEXT_BRIGHT).bg(PANEL_BG));
        frame.render_widget(Clear, area);
        frame.render_widget(compact, area);
        return;
    }

    // Backdrop: clear then fill so the dungeon viewport never bleeds through.
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new("").style(Style::default().bg(PANEL_BG)),
        area,
    );

    let border = Block::default()
        .borders(Borders::ALL)
        .title(format!(
            " ⚔ BATTLE · ROUND {} · {} ",
            battle.round,
            phase_label(battle.phase)
        ))
        .border_style(Style::default().fg(if ctx.high_contrast {
            Color::White
        } else {
            TEXT_DIM
        }));
    let inner = border.inner(area);
    frame.render_widget(border, area);

    let enemies = battle.enemy_indices.len();
    let roster_lines = 1 + enemies.min(2);

    // Cap every dimension to u16 to avoid overflow/underflow on tiny terminals.
    let roster_lines_u16: u16 = roster_lines.try_into().unwrap_or(u16::MAX);
    // Reserve the roster plus the player line and message line (3 extra rows).
    let reserved = roster_lines_u16.saturating_add(3).min(inner.height);
    let avail = inner.height.saturating_sub(reserved);
    let cmd_rows = COMMAND_LABELS.len().min(avail as usize).clamp(1, 6);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(roster_lines_u16),
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(cmd_rows as u16),
            Constraint::Length(1),
        ])
        .split(inner);

    render_roster(frame, chunks[0], ctx, battle);
    render_arena(frame, chunks[1], ctx, battle);
    render_player_line(frame, chunks[2], ctx, battle);
    if battle.phase == BattlePhase::Item {
        render_item_menu(frame, chunks[3], battle);
    } else {
        render_commands(frame, chunks[3], battle);
    }
    render_message(frame, chunks[4], battle);
}

fn phase_label(phase: BattlePhase) -> &'static str {
    match phase {
        BattlePhase::Input => "INPUT",
        BattlePhase::Target => "TARGET",
        BattlePhase::Item => "ITEM",
        BattlePhase::Resolve => "RESOLVE",
        BattlePhase::Victory => "VICTORY",
        BattlePhase::Defeat => "DEFEAT",
        BattlePhase::Fled => "FLED",
    }
}

/// Resolve the live monster for `enemy_indices[i]`, bounds-checked.
fn enemy_at<'a>(ctx: &'a RenderCtx, battle: &BattleState, i: usize) -> Option<&'a Monster> {
    let idx = *battle.enemy_indices.get(i)?;
    ctx.monsters.get(idx)
}

fn enemy_intent(monster: &Monster) -> String {
    monster
        .intent
        .map(|i| i.kind.label().to_string())
        .unwrap_or_else(|| "—".to_string())
}

fn hp_bar(hp: i32, max_hp: i32, width: usize) -> (String, Color) {
    let max_hp = max_hp.max(1);
    let hp = hp.clamp(0, max_hp);
    let frac = hp as f32 / max_hp as f32;
    let filled = (frac * width as f32).round() as usize;
    let color = if frac > 0.5 {
        HP_FULL
    } else if frac > 0.25 {
        HP_MID
    } else {
        HP_LOW
    };
    let mut s = String::with_capacity(width + 2);
    s.push('[');
    for i in 0..width {
        s.push(if i < filled { '█' } else { '░' });
    }
    s.push(']');
    (s, color)
}

fn render_roster(frame: &mut Frame, area: Rect, ctx: &RenderCtx, battle: &BattleState) {
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        "ENEMIES",
        Style::default().fg(TEXT_DIM),
    )));
    let target = battle
        .selected_target
        .min(battle.enemy_indices.len().saturating_sub(1));
    for i in 0..battle.enemy_indices.len().min(2) {
        let marker = if i == target { "▶ " } else { "  " };
        let Some(monster) = enemy_at(ctx, battle, i) else {
            continue;
        };
        let (bar, color) = hp_bar(monster.stats.hp, monster.stats.max_hp, 14);
        let fg = if i == target { TEXT_BRIGHT } else { TEXT_DIM };
        lines.push(Line::from(vec![
            Span::styled(marker, Style::default().fg(CURSOR_COLOR)),
            Span::styled(format!("{} ", monster.kind.name()), Style::default().fg(fg)),
            Span::styled(bar, Style::default().fg(color)),
            Span::styled(
                format!(" {}/{}", monster.stats.hp, monster.stats.max_hp),
                Style::default().fg(TEXT_DIM),
            ),
            Span::styled(
                format!("  ({})", enemy_intent(monster)),
                Style::default().fg(if ctx.reduced_flash {
                    TEXT_DIM
                } else {
                    ENEMY_ACCENT
                }),
            ),
        ]));
    }
    if battle.enemy_indices.len() > 2 {
        lines.push(Line::from(Span::styled(
            format!("… +{} more", battle.enemy_indices.len() - 2),
            Style::default().fg(TEXT_DIM),
        )));
    }
    frame.render_widget(
        Paragraph::new(lines).style(Style::default().bg(PANEL_BG)),
        area,
    );
}

fn render_arena(frame: &mut Frame, area: Rect, ctx: &RenderCtx, battle: &BattleState) {
    let halves = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(2),
            Constraint::Fill(1),
        ])
        .split(area);

    let player_intent = COMMAND_LABELS[battle.command_cursor.min(COMMAND_LABELS.len() - 1)];

    // Player on the left.
    draw_combatant(
        frame,
        halves[0],
        PLAYER_ART,
        PLAYER_COLOR,
        PLAYER_ACCENT,
        "YOU",
        player_intent,
        ctx.player.stats.hp,
        ctx.player.stats.max_hp,
    );

    // Target enemy on the right.
    if let Some(monster) = enemy_at(
        ctx,
        battle,
        battle
            .selected_target
            .min(battle.enemy_indices.len().saturating_sub(1)),
    ) {
        draw_combatant(
            frame,
            halves[2],
            ENEMY_ART,
            ENEMY_COLOR,
            ENEMY_ACCENT,
            monster.kind.name(),
            &enemy_intent(monster),
            monster.stats.hp,
            monster.stats.max_hp,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_combatant(
    frame: &mut Frame,
    area: Rect,
    art: &[&str],
    main: Color,
    accent: Color,
    name: &str,
    intent: &str,
    hp: i32,
    max_hp: i32,
) {
    let art_h = art.len() as u16;
    let art_w = art.first().map_or(0, |r| r.len() as u16);
    let y = if area.height >= art_h + 2 {
        area.y + (area.height - art_h - 2) / 2
    } else {
        area.y
    };

    // Label above the figure.
    if area.height > art_h + 1 {
        let label = format!("{}  ({})", name, intent);
        frame.render_widget(
            Paragraph::new(label).style(Style::default().fg(main)),
            Rect::new(area.x, y, area.width, 1),
        );
    }

    let x = if area.width >= art_w {
        area.x + (area.width - art_w) / 2
    } else {
        area.x
    };
    for (row, line) in art.iter().enumerate() {
        let mut spans = Vec::with_capacity(line.len());
        for ch in line.chars() {
            match ch {
                'X' => spans.push(Span::styled("█", Style::default().fg(main))),
                'o' => spans.push(Span::styled("█", Style::default().fg(accent))),
                _ => spans.push(Span::raw(" ")),
            }
        }
        frame.render_widget(
            Paragraph::new(Line::from(spans)),
            Rect::new(x, y + 1 + row as u16, art_w.max(1), 1),
        );
    }

    // HP bar directly under the figure.
    let bar_y = (y + 1 + art_h).min(area.y + area.height.saturating_sub(1));
    if bar_y < area.y + area.height {
        let (bar, color) = hp_bar(hp, max_hp, art_w.max(8) as usize);
        frame.render_widget(
            Paragraph::new(bar).style(Style::default().fg(color).bg(PANEL_BG)),
            Rect::new(x, bar_y, art_w.max(8), 1),
        );
    }
}

fn render_player_line(frame: &mut Frame, area: Rect, ctx: &RenderCtx, battle: &BattleState) {
    let (bar, color) = hp_bar(ctx.player.stats.hp, ctx.player.stats.max_hp, 16);
    let intent = COMMAND_LABELS[battle.command_cursor.min(COMMAND_LABELS.len() - 1)];
    let line = Line::from(vec![
        Span::styled(
            "YOU ",
            Style::default()
                .fg(PLAYER_COLOR)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(bar, Style::default().fg(color)),
        Span::styled(
            format!(" {}/{}", ctx.player.stats.hp, ctx.player.stats.max_hp),
            Style::default().fg(TEXT_DIM),
        ),
        Span::styled(
            format!("   intent: {}", intent),
            Style::default().fg(PLAYER_ACCENT),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(line).style(Style::default().bg(PANEL_BG)),
        area,
    );
}

fn render_commands(frame: &mut Frame, area: Rect, battle: &BattleState) {
    let cursor = battle
        .command_cursor
        .min(COMMAND_LABELS.len().saturating_sub(1));
    let mut lines: Vec<Line> = Vec::new();
    for (i, label) in COMMAND_LABELS.iter().enumerate() {
        let selected = i == cursor;
        let marker = if selected { "▶ " } else { "  " };
        lines.push(Line::from(Span::styled(
            format!("{}{}", marker, label),
            Style::default().fg(if selected { CURSOR_COLOR } else { TEXT_BRIGHT }),
        )));
    }
    frame.render_widget(
        Paragraph::new(lines).style(Style::default().bg(PANEL_BG)),
        area,
    );
}

fn render_message(frame: &mut Frame, area: Rect, battle: &BattleState) {
    frame.render_widget(
        Paragraph::new(battle.message.clone()).style(Style::default().fg(TEXT_BRIGHT).bg(PANEL_BG)),
        area,
    );
}

/// Render the consumable picker shown in the `Item` phase. Listed kinds come
/// from `battle.item_list` (the usable consumables), with the cursor marked.
fn render_item_menu(frame: &mut Frame, area: Rect, battle: &BattleState) {
    let cursor = battle
        .item_cursor
        .min(battle.item_list.len().saturating_sub(1));
    let mut lines: Vec<Line> = Vec::new();
    if battle.item_list.is_empty() {
        lines.push(Line::from(Span::styled(
            "(no usable items)",
            Style::default().fg(TEXT_DIM),
        )));
    }
    for (i, kind) in battle.item_list.iter().enumerate() {
        let selected = i == cursor;
        let marker = if selected { "▶ " } else { "  " };
        lines.push(Line::from(Span::styled(
            format!("{}{}", marker, item_label(*kind)),
            Style::default().fg(if selected { CURSOR_COLOR } else { TEXT_BRIGHT }),
        )));
    }
    frame.render_widget(
        Paragraph::new(lines).style(Style::default().bg(PANEL_BG)),
        area,
    );
}

/// Short battle-safe label for a consumable. Battle-only copies of the
/// inventory descriptions (exploration-only effects are reported as such by the
/// app layer, not faked here).
fn item_label(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::HealthPotion => "health potion",
        ItemKind::LargeHealthPotion => "large health potion",
        ItemKind::Antidote => "antidote",
        ItemKind::ScrollOfFire => "scroll of fire",
        ItemKind::ScrollOfConfusion => "scroll of confusion",
        ItemKind::ScrollOfBlink => "scroll of blink",
        ItemKind::LanternOil => "lantern oil",
        ItemKind::SmokeBomb => "smoke bomb",
        ItemKind::EnchantmentScroll => "enchantment scroll",
        ItemKind::LoreFragment => "lore fragment",
        _ => kind.name(),
    }
}
