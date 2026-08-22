mod ai;
mod app;
mod battle;
mod campaign;
mod combat;
mod entity;
mod floor;
mod fov;
mod image_render;
mod input;
mod inventory;
mod item_art;
mod logging;
mod map;
mod monster_ai;
mod narrative;
mod pathfinding;
mod render;
mod replay;
mod run_history;
mod save;
mod settings;
mod spawn;
mod third_person;
mod ui;
mod visual;

#[cfg(test)]
mod tests;

use color_eyre::Result;

fn main() -> Result<()> {
    color_eyre::install()?;
    logging::init();

    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return Ok(());
    }

    if let Some(path) = parse_replay_path(&args)? {
        return run_replay(&path);
    }

    let seed = parse_seed(&args)?;

    let mut app = app::App::new(seed)?;
    let result = app.run();
    ratatui::try_restore()?;
    result
}

fn parse_replay_path(args: &[String]) -> Result<Option<String>> {
    for (index, argument) in args.iter().enumerate().skip(1) {
        if let Some(path) = argument.strip_prefix("--replay=") {
            if path.is_empty() {
                return Err(color_eyre::eyre::eyre!("--replay requires a file path"));
            }
            return Ok(Some(path.to_string()));
        }
        if argument == "--replay" {
            return args
                .get(index + 1)
                .cloned()
                .map(Some)
                .ok_or_else(|| color_eyre::eyre::eyre!("--replay requires a file path"));
        }
    }
    Ok(None)
}

fn run_replay(path: &str) -> Result<()> {
    let json = std::fs::read_to_string(path)?;
    let data: replay::ReplayData = serde_json::from_str(&json)?;
    let verified = data.expected.is_some();
    let state = replay::playback(&data).map_err(|error| color_eyre::eyre::eyre!(error))?;
    println!(
        "Replay {}: {:?}, depth {}, {} turns, {} kills, seed {}",
        if verified { "verified" } else { "completed" },
        state.status,
        state.player.depth + 1,
        state.stats.turns,
        state.stats.kills,
        state.seed
    );
    Ok(())
}

pub(crate) fn parse_seed(args: &[String]) -> Result<Option<u64>> {
    let daily = args.iter().any(|arg| arg == "--daily");
    if daily
        && args
            .iter()
            .any(|arg| arg == "--seed" || arg == "-s" || arg.starts_with("--seed="))
    {
        return Err(color_eyre::eyre::eyre!(
            "--daily cannot be combined with --seed"
        ));
    }
    if daily {
        let seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| color_eyre::eyre::eyre!("system clock is before the Unix epoch"))?
            .as_secs();
        return Ok(Some(daily_seed_from_unix_secs(seconds)));
    }
    for i in 1..args.len() {
        if let Some(val) = args[i].strip_prefix("--seed=") {
            return Ok(Some(val.parse::<u64>().map_err(|_| {
                color_eyre::eyre::eyre!("invalid seed '{val}': expected a positive whole number")
            })?));
        }
        if args[i] == "--seed" || args[i] == "-s" {
            let val = args
                .get(i + 1)
                .ok_or_else(|| color_eyre::eyre::eyre!("{} requires a number", args[i]))?;
            return Ok(Some(val.parse::<u64>().map_err(|_| {
                color_eyre::eyre::eyre!("invalid seed '{val}': expected a positive whole number")
            })?));
        }
    }
    Ok(None)
}

pub(crate) fn daily_seed_from_unix_secs(seconds: u64) -> u64 {
    let utc_day = seconds / 86_400;
    utc_day.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD311_E5EE_DA11_0001
}

fn print_help() {
    println!(
        r#"delve — a terminal roguelike

USAGE:
    delve [OPTIONS]

OPTIONS:
    --replay=<FILE>, --replay <FILE>
        Verify and summarize a recorded run without opening the terminal UI.

    --daily
        Play the shared UTC daily dungeon seed.

    --seed=<N>, -s <N>, --seed <N>
        Set the dungeon seed for reproducible runs.
        Example:  delve --seed=42
                  delve -s 12345

    --help, -h
        Print this help message.

GAMEPLAY:
    Descend through 5 procedurally generated dungeon floors,
    find the Crown of Depth, and return to the surface to win.
    The deepest floor holds a Guardian — and no way further down.
    The main menu also opens Chapter I of the authored campaign.
    Maps use a manuscript-fantasy palette with roads, dressed NPCs,
    torchlight, and environmental landmarks to make each route readable.
    Modern Classic runs remember visited floors: noise raises attention,
    quiet turns cool it, and the Crown can wake a Stalker on the return route.
    World-format changes start a fresh run; older saves are kept on disk but
    will not be loaded into a newer world.
    The current loadout technique is one of Kick, Shield Bash, Lunge, Sweep,
    Earthbreak, or Cinder Arc; the forge can add a named mastery.

CONTROLS:
    W / Up / k            move forward
    S / Down / j          move backward
    A / Left / h          turn left
    D / Right / l         turn right
    Z                     strafe left
    C                     strafe right
    Shift+Z / Shift+C     evasive step
    Shift+T               use loadout technique
    g                     pick up item
    Enter                 context action (pickup, talk, stairs)
    e                     toggle auto-pickup
    t                     talk to NPC
    Tab / i               open inventory
    Q                     open quest log
    j/k / arrows          navigate inventory
    Enter / Space         use/equip selected inventory item
    >                     descend stairs
    <                     ascend stairs
    x                     auto-explore
    m                     show/hide minimap
    O                     open/close a facing door
    v                     listen through a closed door
    r                     disarm a revealed trap
    B                     discovered bestiary
    V                     expanded chronicle
    f                     use the assigned quick item
    . / Space             wait one turn
    F2                    reduced flash
    F3                    high contrast
    F4                    camera easing
    F6                    head movement
    F7                    viewport palette
    F8                    low-motion presentation
    F5                    save game
    F9                    load game
    ?                     controls (also available from the main menu)
    Esc                   close the active panel or conversation
    q                     quit (auto-saves)
"#
    );
}
