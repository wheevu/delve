use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MsgColor {
    White,
    Red,
    Green,
    Yellow,
    Gray,
    Cyan,
}

/// A single message with color metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub text: String,
    pub color: MsgColor,
    #[serde(default = "default_repeat")]
    pub repeat: u16,
}

fn default_repeat() -> u16 {
    1
}

/// Message log with colored messages, backed by a ring buffer.
pub struct MessageLog {
    messages: VecDeque<Message>,
    max_size: usize,
}

impl MessageLog {
    pub fn new(max_size: usize) -> Self {
        Self {
            messages: VecDeque::with_capacity(max_size),
            max_size,
        }
    }

    pub fn push(&mut self, msg: impl Into<String>, color: MsgColor) {
        let text = msg.into();
        if let Some(last) = self.messages.back_mut() {
            if last.text == text && last.color == color {
                last.repeat = last.repeat.saturating_add(1);
                return;
            }
        }
        if self.messages.len() >= self.max_size {
            self.messages.pop_front();
        }
        self.messages.push_back(Message {
            text,
            color,
            repeat: 1,
        });
    }

    /// Last `n` messages, newest last.
    pub fn recent(&self, n: usize) -> Vec<&Message> {
        let start = self.messages.len().saturating_sub(n);
        self.messages.iter().skip(start).collect()
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }

    pub fn clone_messages(&self) -> Vec<Message> {
        let v: Vec<Message> = self.messages.iter().cloned().collect();
        v
    }

    pub fn set_messages(&mut self, msgs: Vec<Message>) {
        self.messages = msgs.into();
        // Trim to max_size so a loaded log never exceeds the ring buffer.
        while self.messages.len() > self.max_size {
            self.messages.pop_front();
        }
    }

    /// Total number of messages in the log.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Return a window of messages starting at `offset` (0 = newest), for `height` lines.
    /// High offsets return older messages.
    pub fn window(&self, height: usize, offset: usize) -> Vec<&Message> {
        let total = self.messages.len();
        if total == 0 {
            return Vec::new();
        }
        let end = total.saturating_sub(offset);
        let start = end.saturating_sub(height);
        self.messages.iter().skip(start).take(height).collect()
    }
}

// ── Semantic color aliases ──
// Note: KILL and SYSTEM both use White intentionally — kill messages
// are highlighted differently via their text content, not color alone.

impl MsgColor {
    pub const DAMAGE_DEALT: MsgColor = MsgColor::Yellow;
    pub const DAMAGE_TAKEN: MsgColor = MsgColor::Red;
    pub const HEALING: MsgColor = MsgColor::Green;
    pub const LOOT: MsgColor = MsgColor::Cyan;
    pub const KILL: MsgColor = MsgColor::White;
    pub const SYSTEM: MsgColor = MsgColor::White;
    pub const DIM: MsgColor = MsgColor::Gray;
}
