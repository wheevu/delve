# delve

> A terminal roguelike about attention, consequence, and finding your way back up.

![A seeded Delve run showing the third-person dungeon, map, and help overlay](assets/delve-gameplay.gif)

Delve is a keyboard-first third-person roguelike built in Rust. Descend through
dangerous rooms, read the space around you, make a few permanent decisions, and
carry what you learned back to the surface.

There are two ways down:

- **Classic** is a replayable five-floor procedural run. Find the Crown of Depth,
  survive the return journey, and make it back to the surface.
- **Campaign — Chapter I** is a three-level authored route with named characters,
  branching conversations, persistent trust, and objective gates. Its dedicated
  completion screen records the bell choice instead of dropping you into an
  unrelated procedural floor.

## Play

You need a recent Rust toolchain and a terminal at least 80 columns wide by 24
rows tall.

```bash
cargo run --release
```

Useful starting points:

```bash
cargo run -- --seed 42       # reproducible dungeon
cargo run -- --daily         # shared seed for the current UTC day
cargo run -- --replay delve_replay_42.json
cargo run -- --help
```

Classic is the default route. From the main menu, press `m` to open Campaign —
Chapter I.

The GIF above is a real seeded session from `--seed 42`, captured directly from
the terminal UI.

## The shape of a run

| System | What it asks of you |
| --- | --- |
| Perspective dungeon | Use the half-block diorama, lantern FOV, material cues, and edge chevrons to read what is just beyond the camera. |
| Telegraph-driven combat | Spend stamina carefully, interrupt heavy windups, hold a guard for a counterstrike, and use traps, terrain, push, and stagger to change the odds. |
| Equipment with memory | Weapons wear down, cursed gear binds until broken, and three named artifacts have guaranteed places in the lower depths. |
| Places worth noticing | Camps, shrines, flooded rooms, webs, forges, ossuaries, and the cracked bell each have their own visual and mechanical signature. |
| Consequences that travel | Conversations, trust, objectives, challenges, Crown pursuit, local run history, and deterministic replays keep decisions legible after the turn that made them. |

The renderer is presentation-only around a deterministic turn simulation. Actors
ease between cells, attacks telegraph and resolve on turns, and animation never
changes collision, procedural generation, saves, or replay outcomes.

## Controls

Delve is designed to be playable without leaving the keyboard. The in-game `?`
screen is the source of truth and includes the complete control reference.

| Action | Keys |
| --- | --- |
| Move forward / back | `W` / `Up` / `K` · `S` / `Down` / `J` |
| Turn left / right | `A` / `Left` / `H` · `D` / `Right` / `L` |
| Strafe / evasive step | `Z` / `C` · `Shift+Z` / `Shift+C` |
| Technique | `Shift+T` |
| Attack, open, or talk | Bump into the target |
| Context action | `Enter` |
| Pick up / auto-pickup | `g` · `e` |
| Talk to an NPC directly | `t` |
| Inventory | `Tab` |
| Quest log | `Q` |
| Interruptible auto-explore | `x` |
| Map / help | `m` / `M` · `?` |
| Wait in defensive guard | `.` / `Space` |
| Bestiary / chronicle | `B` · `V` |
| Door / listen | `O` · `v` at a closed door |
| Descend / ascend stairs | `>` · `<` |
| Disarm a revealed trap | `r` |
| Quick-use setup / use | `Q` in the inventory, then `f` |
| Presentation settings | `F2`, `F3`, `F4`, `F6`, `F7`, `F8` |
| Save / load / quit | `F5` · `F9` · `q` |

From the main menu, `x` cycles optional challenges: no minimap, reduced sight,
double weapon wear, and relentless mobile monsters. Presentation choices persist
separately in `delve_settings.json`; completed wins and deaths are kept in a
bounded local run history.

## What is in the dungeon

- 12 monster types, including the Crown Guardian and its stronger Crown-fire phase.
- 22 item types, gear enchantments, unidentified consumables, repairable weapons,
  and three guaranteed named artifacts.
- 16 NPC kinds with authored silhouettes, role colors, branching dialogue, and
  subdued post-conversation states.
- Five landmark room types with authored silhouettes and floor signatures.
- Camps with one preparation per visit; shrines with healing, cleansing, and
  attack-and-defense blessings.
- Water, webs, traps, barricades, locked doors, projectiles, decals, lore
  fragments, and a return journey that gets more dangerous after claiming the
  Crown.

## Development

The terminal UI and simulation are tested together:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --release
```

Delve keeps its state local to the working directory. Replays are deterministic
artifacts you can inspect or verify without opening the TUI, which makes seeded
bugs and balancing changes easier to reproduce.

## License

MIT.
