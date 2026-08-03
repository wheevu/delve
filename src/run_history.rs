use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::app::{ChallengeMode, GameMode, GameState, GameStatus, RunStats};

pub(crate) const RUN_HISTORY_PATH: &str = "delve_runs.json";
const MAX_RUN_HISTORY: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct RunSummary {
    pub seed: u64,
    pub won: bool,
    pub game_mode: GameMode,
    pub challenge_mode: ChallengeMode,
    pub stats: RunStats,
    pub completed_at_unix: u64,
    #[serde(default)]
    pub final_x: Option<i32>,
    #[serde(default)]
    pub final_y: Option<i32>,
    #[serde(default)]
    pub final_depth: u32,
}

impl RunSummary {
    pub(crate) fn from_state(state: &GameState) -> Self {
        Self {
            seed: state.seed,
            won: matches!(state.status, GameStatus::Won | GameStatus::CampaignComplete),
            game_mode: state.game_mode,
            challenge_mode: state.challenge_mode,
            stats: state.stats.clone(),
            completed_at_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_secs()),
            final_x: Some(state.player.x),
            final_y: Some(state.player.y),
            final_depth: state.player.depth,
        }
    }
}

pub(crate) fn load() -> Vec<RunSummary> {
    let mut history: Vec<RunSummary> = std::fs::read_to_string(RUN_HISTORY_PATH)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();
    if history.len() > MAX_RUN_HISTORY {
        history.drain(..history.len() - MAX_RUN_HISTORY);
    }
    history
}

pub(crate) fn push_bounded(history: &mut Vec<RunSummary>, summary: RunSummary) {
    history.push(summary);
    if history.len() > MAX_RUN_HISTORY {
        history.drain(..history.len() - MAX_RUN_HISTORY);
    }
}

pub(crate) fn save(history: &[RunSummary]) -> io::Result<()> {
    let json = serde_json::to_string_pretty(history).map_err(io::Error::other)?;
    let temporary_path = format!("{RUN_HISTORY_PATH}.tmp");
    std::fs::write(&temporary_path, json)?;
    match std::fs::rename(&temporary_path, RUN_HISTORY_PATH) {
        Ok(()) => Ok(()),
        Err(_) => {
            // Windows cannot atomically replace an existing destination.
            let _ = std::fs::remove_file(RUN_HISTORY_PATH);
            std::fs::rename(temporary_path, RUN_HISTORY_PATH)
        }
    }
}
