// Crash and game-event logging for Delve.
//
// Writes structured logs to delve.log in the working directory.
// On panic, dumps a backtrace and basic game-state snapshot to the log
// before the process exits.

use log::LevelFilter;
use simplelog::{ConfigBuilder, WriteLogger};
use std::fs::File;
use std::io::Write;
use std::panic;

const LOG_FILE: &str = "delve.log";

/// Initialize file logging and the global panic hook.
/// Should be called once at startup, before any game logic.
pub fn init() {
    // ── File logger ──
    let log_file = match File::create(LOG_FILE) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("delve: cannot create log file '{}': {}", LOG_FILE, e);
            return;
        }
    };

    let config = ConfigBuilder::new()
        .set_time_format_rfc3339()
        .set_target_level(LevelFilter::Off)
        .set_thread_level(LevelFilter::Off)
        .set_location_level(LevelFilter::Off)
        .build();

    if let Err(e) = WriteLogger::init(LevelFilter::Debug, config, log_file) {
        eprintln!("delve: logger init failed: {}", e);
        return;
    }

    // ── Panic hook: flush log before dying ──
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        // Flush pending log messages to disk
        log::logger().flush();

        // Dump panic info to stderr (original behaviour)
        default_hook(info);

        // Also write the panic to a separate crash file for easy discovery
        if let Ok(mut f) = File::create("delve_crash.txt") {
            let payload = info.payload();
            let msg = if let Some(s) = payload.downcast_ref::<&str>() {
                *s
            } else if let Some(s) = payload.downcast_ref::<String>() {
                s.as_str()
            } else {
                "(non-string panic payload)"
            };
            let location = info.location().map_or_else(
                || String::from("??"),
                |loc| format!("{}:{}:{}", loc.file(), loc.line(), loc.column()),
            );
            let _ = writeln!(f, "CRASH: {} at {}", msg, location);
            let _ = writeln!(f);
            let backtrace = std::backtrace::Backtrace::force_capture();
            let _ = writeln!(f, "Backtrace:\n{}", backtrace);
        }

        // Terminate immediately to avoid double-panic from the default hook
        std::process::abort();
    }));

    log::info!("── delve session start ──");
    log::info!("host: {} {}", std::env::consts::OS, std::env::consts::ARCH);
}

/// Log a game lifecycle event with structured key=value pairs.
pub fn game_event(event: GameEvent) {
    log::info!("GAME {}", event);
}

/// Semantic game events. Displayed as compact key=value lines.
#[derive(Debug, Clone)]
pub enum GameEvent {
    NewGame {
        seed: u64,
    },
    LevelGen {
        depth: u32,
        rooms: usize,
        monsters: usize,
        items: usize,
    },
    PlayerMove {
        from: (i32, i32),
        to: (i32, i32),
        turns: u32,
    },
    Attack {
        attacker: String,
        defender: String,
        damage: i32,
        remaining_hp: i32,
    },
    Kill {
        killer: String,
        victim: String,
        x: i32,
        y: i32,
    },
    PickUp {
        item: String,
        x: i32,
        y: i32,
    },
    UseItem {
        item: String,
    },
    Equip {
        item: String,
        slot: String,
    },
    Descend {
        from_depth: u32,
        to_depth: u32,
    },
    Ascend {
        from_depth: u32,
        to_depth: u32,
    },
    Win {
        turns: u32,
        kills: u32,
        deepest: u32,
    },
    Death {
        turns: u32,
        depth: u32,
        killed_by: Option<String>,
    },
    Save,
    Load {
        seed: u64,
    },
    Quit {
        turns: u32,
    },
}

impl std::fmt::Display for GameEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameEvent::NewGame { seed } => write!(f, "new_game seed={seed}"),
            GameEvent::LevelGen {
                depth,
                rooms,
                monsters,
                items,
            } => {
                write!(
                    f,
                    "level_gen depth={depth} rooms={rooms} monsters={monsters} items={items}"
                )
            }
            GameEvent::PlayerMove { from, to, turns } => {
                write!(
                    f,
                    "player_move from={},{} to={},{} turn={turns}",
                    from.0, from.1, to.0, to.1
                )
            }
            GameEvent::Attack {
                attacker,
                defender,
                damage,
                remaining_hp,
            } => {
                write!(
                    f,
                    "attack by={attacker} vs={defender} dmg={damage} hp_left={remaining_hp}"
                )
            }
            GameEvent::Kill {
                killer,
                victim,
                x,
                y,
            } => {
                write!(f, "kill killer={killer} victim={victim} at={x},{y}")
            }
            GameEvent::PickUp { item, x, y } => write!(f, "pickup item={item} at={x},{y}"),
            GameEvent::UseItem { item } => write!(f, "use_item item={item}"),
            GameEvent::Equip { item, slot } => write!(f, "equip item={item} slot={slot}"),
            GameEvent::Descend {
                from_depth,
                to_depth,
            } => {
                write!(f, "descend from={from_depth} to={to_depth}")
            }
            GameEvent::Ascend {
                from_depth,
                to_depth,
            } => {
                write!(f, "ascend from={from_depth} to={to_depth}")
            }
            GameEvent::Win {
                turns,
                kills,
                deepest,
            } => {
                write!(f, "WIN turns={turns} kills={kills} deepest={deepest}")
            }
            GameEvent::Death {
                turns,
                depth,
                killed_by,
            } => {
                write!(
                    f,
                    "DEATH turns={turns} depth={depth} by={}",
                    killed_by.as_deref().unwrap_or("unknown")
                )
            }
            GameEvent::Save => write!(f, "save"),
            GameEvent::Load { seed } => write!(f, "load seed={seed}"),
            GameEvent::Quit { turns } => write!(f, "quit turns={turns}"),
        }
    }
}
