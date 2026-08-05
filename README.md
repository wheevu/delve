# delve

> A terminal roguelike about going down, paying attention, and finding your way back out.

![Delve gameplay: exploring a dungeon, fighting a slime, talking with an NPC, and checking the map](assets/delve-gameplay.gif)

Delve is a small third-person dungeon crawler for the terminal. You move one turn
at a time, but there is always something to notice: a lit edge of a room, a door
you have not opened, a monster winding up an attack, or a useful detail in the
space just beyond the camera. Pay attention and the dungeon starts to make sense.
Rush it and the dungeon remembers.

There are two ways down:

- **Classic** is a replayable five-floor procedural run. Find the Crown of Depth,
  survive the trip back up, and make it to the surface.
- **Campaign — Chapter I** is a three-level authored route with named characters,
  conversations, trust, and objectives that carry forward. It ends with the bell,
  and the choice you make there.

The GIF is a quick cut of real Delve captures: exploration, a slime fight, the
opening campaign levels, a conversation with Tovan Reed, the map, and inventory.
Some scenes are set up so the interesting parts arrive quickly.

## Play

You need Rust 1.86 or newer and a terminal at least 80 columns wide by 24 rows
tall.

```bash
cargo run --release
```

A few useful ways to start:

```bash
cargo run -- --seed 42       # reproducible dungeon
cargo run -- --daily         # shared seed for the current UTC day
cargo run -- --replay delve_replay_42.json
cargo run -- --help
```

Classic is the default. From the main menu, press `m` to open Campaign — Chapter I.

## What a run feels like

- You read the dungeon in pieces. The half-block view, lantern, materials, and
  small edge markers tell you what is just outside the frame.
- Fights are about timing. Enemies show their intent, stamina matters, and a guard
  or interruption at the right moment can turn a bad room around. Terrain, traps,
  pushes, and stagger are part of the fight too.
- Gear does not stay pristine. Weapons wear down, cursed items bind until they
  break, and three named artifacts have guaranteed places in the lower depths.
- Some rooms are worth remembering: camps, shrines, flooded rooms, webs, forges,
  ossuaries, and the cracked bell all have their own character.
- Choices do not disappear when the turn ends. Conversations, trust, objectives,
  challenges, the Crown, and your local run history give a run a longer memory.

Underneath, Delve is a deterministic turn simulation. The presentation eases
actors between cells and telegraphs attacks, but it does not change collision,
procedural generation, saves, or replay outcomes.

## Controls

You can play the whole game from the keyboard. Press `?` in-game for the complete
reference; these are the keys you will use most:

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
double weapon wear, and relentless mobile monsters. Presentation choices live in
`delve_settings.json`; completed wins and deaths are kept in a small local run
history.

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

Runs are deterministic, so a seed or replay file is enough to reproduce a bug,
check a balance change, or share a strange moment from the dungeon without
opening the TUI first.

Press `?` in-game for the full rules reference.
Run `delve --help` for the current command reference.

## License

MIT.
