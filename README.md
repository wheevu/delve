# delve

> A terminal roguelike about going down, paying attention, and finding your way back out.

![Delve gameplay: walking through Warden's Village and Old King's Road, finding gear, fighting a spider with a longsword and Cinder Arc, and opening the inventory in Bell-Line Gate](assets/delve-gameplay.gif)

Delve is a small first-person dungeon crawler for the terminal. You move one turn
at a time, but there is always something to notice: a lit edge of a room, a door
you have not opened, a monster winding up an attack, or a useful detail in the
space just beyond the camera. Pay attention and the dungeon starts to make sense.
Rush it and the dungeon remembers.

That memory is literal. In a modern Classic run, floors do not quietly reset
when you climb away from them. Broken doors stay broken, loot stays taken, dead
things stay dead, and noise leaves a floor more alert. Carry the Crown back
through those rooms and the Stalker eventually follows your old route home.

There are two ways down:

- **Classic** is a replayable five-floor procedural run. Find the Crown of Depth,
  survive the trip back up, and make it to the surface.
- **Campaign — Chapter I** is a three-level authored route with named characters,
  conversations, trust, and objectives that carry forward. It ends with the bell,
  and the choice you make there determines the chapter ending.

The lead GIF is a short cut of current runtime renders: walking through generated
rooms in Warden's Village, finding gear, trading longsword blows with a spider,
firing Cinder Arc, and opening the inventory in Bell-Line Gate. It gets to
movement, gear, spellcraft, and combat quickly while keeping the actual walls,
doors, props, and route UI in frame.

## Visual direction

The current build uses a first-person half-block view with a manuscript-fantasy
medieval treatment. Generated floors carry named world locations, road and mud
surfaces, carts, signs, gates, rubble, trees, banners, braziers, torches, and
other authored props. NPCs are hand-drawn humanoid silhouettes dressed around
their roles, and conversations show the exact speaker's likeness beside the
dialogue. Doors, shallow water, webs, lanterns, and local firelight keep terrain
and light readable rather than decorative.

The world layer is a fresh-save boundary. Start a new run to see the new route
and keep older saves as archives; the loader rejects pre-v15 world saves instead
of silently remapping their floor data.

## More from the current build

The lead GIF moves quickly. These cuts linger on the details behind it. Every
reel is captured from current runtime maps: authored rooms, connected corridors,
landmarks, props, camps, and the people who inhabit them.

### Combat, spellcraft, and weapon feel

![Delve combat: a longsword sweep, Cinder Arc, Earthbreak, a necromancer bolt, and a Bell Warden collapse inside generated combat rooms](assets/delve-combat.gif)

Longsword sweep, Emberbrand's Cinder Arc, Warhammer Earthbreak, a necromancer's
bolt, and the Bell Warden's collapse.

### A route you can walk

![Delve world tour: the player walking through Warden's Village, Old King's Road, Bell-Line Gate, Frozen Depths, Spider Lairs, and the Void Chamber](assets/delve-world-tour.gif)

Each location gets a short walk rather than a hard cut: Warden's Village, Old
King's Road, Bell-Line Gate, Frozen Depths, Spider Lairs, and the Void Chamber.
Connected rooms, doors, material palettes, lanterns, webs, boss chambers, and
movement cues change with the place.

### People, gear, and choices

![Delve roles and gear: Jun, Edda Slate, and Vey speaking in the waystation, followed by a real inventory view with weapons and supplies](assets/delve-roles-and-gear.gif)

The cut follows actual campaign waystation rooms as Jun, Edda Slate, and Vey
speak in role-specific portraits, then opens the inventory over Bell-Line Gate
to show the weapons, armor, potions, lantern oil, and scrolls carried by the
player.

### Different creatures, different conversations

![Delve encounters: fighting a goblin, spider, necromancer, and Bell Warden in real generated rooms, then talking with Jun, Edda Slate, and Vey](assets/delve-mobs-and-npcs.gif)

The encounter cut moves from a goblin in Warden's Village to a spider in the
webbed Spider Lairs, a necromancer in the Toll of Rust, and a Bell Warden in the
Void Chamber, then follows the player through the waystation to speak with Jun,
Edda Slate, and Vey.

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
- Motion carries information. Directional steps leave a brief trail, evasive steps
  leave an afterimage, attacks draw weapon-specific sweeps, and techniques resolve
  through their own cast and impact shapes. Scrolls, potions, lantern oil, enemy
  spells, and NPC speech each get a distinct presentation cue.
- Gear does not stay pristine. Weapons wear down, cursed items bind until they
  break, and the forge lets you commit to one of a few named weapon masteries.
  The choice is small on purpose: Keen Edge and Riposte ask different things of
  a blade; Breaker and Quake make a hammer feel like a different tool; Emberbrand
  has its own two paths.
- Some rooms are worth remembering: camps, shrines, flooded rooms, webs, forges,
  ossuaries, and the cracked bell all have their own character. Disturbing a
  spider nest can wake the room around it; the bone chapel can briefly quiet
  your trail.
- Choices do not disappear when the turn ends. Conversations, trust, objectives,
  challenges, the Crown, and your local run history give a run a longer memory.

Underneath, Delve is a deterministic turn simulation. The presentation runs on a
small 30 FPS tick: it eases actors between cells and telegraphs attacks, but it
does not change collision, procedural generation, saves, or replay outcomes. F2
reduces flashes and F8 enables the low-motion presentation mode.

## Controls

You can play the whole game from the keyboard. Press `?` in-game for the complete
reference; these are the keys you will use most:

| Action | Keys |
| --- | --- |
| Move forward / back | `W` / `Up` / `K` · `S` / `Down` / `J` |
| Turn left / right | `A` / `Left` / `H` · `D` / `Right` / `L` |
| Strafe / evasive step | `Z` / `C` · `Shift+Z` / `Shift+C` |
| Loadout technique | `Shift+T` — Kick, Shield Bash, Lunge, Sweep, Earthbreak, or Cinder Arc |
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
| Presentation settings | `F2` flash · `F3` contrast · `F4` camera · `F6` head movement · `F7` palette · `F8` low motion |
| Save / load / quit | `F5` · `F9` · `q` |

From the main menu, `x` cycles optional challenges: no minimap, reduced sight,
double weapon wear, and relentless mobile monsters. Presentation choices live in
`delve_settings.json`; completed wins and deaths are kept in a small local run
history.

## What is in the dungeon

- 14 monster types, including the Crown Guardian, the Bell Warden, and the
  Stalker that appears on remembered Crown-return floors.
- 22 item types, gear enchantments, unidentified consumables, repairable weapons,
  and three guaranteed named artifacts.
- Six loadout techniques: Kick, Shield Bash, Lunge, Sweep, Earthbreak, and Cinder
  Arc. The forge's six named masteries change how the blade, hammer, or Emberbrand
  technique behaves.
- 16 NPC kinds with humanoid role silhouettes, clothing/held-prop details,
  branching dialogue, speaker portraits, and subdued post-conversation states.
- Six landmark room types with authored silhouettes and floor signatures.
- A medieval world layer with Village, Old King's Road, Bell-Line Gate, and
  dungeon location identities, seeded road dressing, terrain surfaces, and
  environmental props with local firelight.
- Camps with one preparation per visit; shrines with healing, cleansing, and
  attack-and-defense blessings.
- Water, webs, traps, barricades, locked doors, projectiles, decals, lore
  fragments, explicit enemy intent, versioned saves, exact RNG checkpoints, and
  a return journey that gets more dangerous after claiming the Crown.

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
opening the TUI first. Replays carry their rules version, so older recorded runs
keep their original generation and combat branches.

The current save format is version 15 and local to `delve_save.json`. It persists
the map's world theme, surfaces, and environmental props, rejects older world
formats with a fresh-run message, and rejects newer formats rather than guessing
at their layout. Replays use world format 1 and follow the same explicit boundary.

Press `?` in-game for the full rules reference.
Run `delve --help` for the current command reference.

## License

MIT.
