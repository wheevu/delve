use crossterm::event::{KeyCode, KeyEvent};

/// Game action — what the player chose to do this turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Move(i32, i32),
    Dodge(i32, i32),
    Technique(i32, i32),
    Wait,
    Quit,
    None,
    PickUp,
    Interact,
    Inventory,
    Descend,
    Ascend,
    AutoExplore,
    SaveGame,
    LoadGame,
    TogglePickup,           // toggle auto-pickup mode
    Talk,                   // talk to adjacent NPC
    TalkAt(i32, i32),       // interact with a specific adjacent NPC
    ConversationChoice(u8), // pick a conversation option (1-3)
    QuestPanel,             // open quest/objectives panel
    ToggleDoor(i32, i32),   // open or close the facing door
    Listen(i32, i32),       // listen through the facing closed door
    Disarm(i32, i32),       // disarm a revealed trap underfoot or ahead

    // ── Instanced battle input (rules_version >= 4) ──
    /// Move the battle cursor (−1 / +1) within the current phase (command menu,
    /// target picker, or item picker).
    BattleMove(isize),
    /// Confirm the current battle selection (command, target, or item).
    BattleConfirm,
    /// Step back from target/item selection to the command menu.
    BattleCancel,
}

/// Actions available inside the inventory screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryAction {
    Select(usize), // a-z direct slot selection
    MoveUp,
    MoveDown,
    UseEquip, // Enter/Space on selected item
    Close,
    None,
}

/// Configurable key bindings.
///
/// Use `KeyConfig::default()` for the standard roguelike layout (WASD, hjkl,
/// arrows, numpad).  Override individual keys with the builder-style setters.
#[derive(Debug, Clone)]
pub struct KeyConfig {
    pub move_keys: Vec<(KeyCode, i32, i32)>,
    pub wait_keys: Vec<KeyCode>,
    pub pickup_keys: Vec<KeyCode>,
    pub toggle_pickup_keys: Vec<KeyCode>,
    pub inventory_keys: Vec<KeyCode>,
    pub descend_keys: Vec<KeyCode>,
    pub ascend_keys: Vec<KeyCode>,
    pub auto_explore_keys: Vec<KeyCode>,
    pub save_keys: Vec<KeyCode>,
    pub load_keys: Vec<KeyCode>,
    pub quit_keys: Vec<KeyCode>,
    pub talk_keys: Vec<KeyCode>,
    pub conversation_choice_keys: Vec<(KeyCode, u8)>,
    pub quest_panel_keys: Vec<KeyCode>,
}

impl Default for KeyConfig {
    fn default() -> Self {
        Self {
            move_keys: vec![
                // Cardinal
                (KeyCode::Left, -1, 0),
                (KeyCode::Char('h'), -1, 0),
                (KeyCode::Char('a'), -1, 0),
                (KeyCode::Right, 1, 0),
                (KeyCode::Char('l'), 1, 0),
                (KeyCode::Char('d'), 1, 0),
                (KeyCode::Up, 0, -1),
                (KeyCode::Char('k'), 0, -1),
                (KeyCode::Char('w'), 0, -1),
                (KeyCode::Down, 0, 1),
                (KeyCode::Char('j'), 0, 1),
                (KeyCode::Char('s'), 0, 1),
                // Diagonal
                (KeyCode::Char('y'), -1, -1),
                (KeyCode::Char('u'), 1, -1),
                (KeyCode::Char('b'), -1, 1),
                (KeyCode::Char('n'), 1, 1),
                // Numpad
                (KeyCode::Char('7'), -1, -1),
                (KeyCode::Char('9'), 1, -1),
                (KeyCode::Char('1'), -1, 1),
                (KeyCode::Char('3'), 1, 1),
            ],
            wait_keys: vec![KeyCode::Char('.'), KeyCode::Char(' ')],
            pickup_keys: vec![KeyCode::Char('g')],
            toggle_pickup_keys: vec![KeyCode::Char('e')],
            inventory_keys: vec![KeyCode::Tab, KeyCode::Char('i')],
            descend_keys: vec![KeyCode::Char('>')],
            ascend_keys: vec![KeyCode::Char('<')],
            auto_explore_keys: vec![KeyCode::Char('x')],
            save_keys: vec![KeyCode::F(5)],
            load_keys: vec![KeyCode::F(9)],
            quit_keys: vec![KeyCode::Char('q')],
            talk_keys: vec![KeyCode::Char('t')],
            conversation_choice_keys: vec![
                (KeyCode::Char('1'), 1),
                (KeyCode::Char('2'), 2),
                (KeyCode::Char('3'), 3),
            ],
            quest_panel_keys: vec![KeyCode::Char('Q'), KeyCode::Char('L')],
        }
    }
}

impl KeyConfig {
    /// Map a key event to an Action using this config.
    pub fn action_from_play_key(&self, key: &KeyEvent) -> Action {
        let code = key.code;

        for &(kc, dx, dy) in &self.move_keys {
            if kc == code {
                return Action::Move(dx, dy);
            }
        }
        if self.wait_keys.contains(&code) {
            return Action::Wait;
        }
        if code == KeyCode::Enter {
            return Action::Interact;
        }
        if self.pickup_keys.contains(&code) {
            return Action::PickUp;
        }
        if self.toggle_pickup_keys.contains(&code) {
            return Action::TogglePickup;
        }
        if self.inventory_keys.contains(&code) {
            return Action::Inventory;
        }
        if self.descend_keys.contains(&code) {
            return Action::Descend;
        }
        if self.ascend_keys.contains(&code) {
            return Action::Ascend;
        }
        if self.auto_explore_keys.contains(&code) {
            return Action::AutoExplore;
        }
        if self.save_keys.contains(&code) {
            return Action::SaveGame;
        }
        if self.load_keys.contains(&code) {
            return Action::LoadGame;
        }
        if self.quit_keys.contains(&code) {
            return Action::Quit;
        }
        if self.talk_keys.contains(&code) {
            return Action::Talk;
        }
        for &(kc, n) in &self.conversation_choice_keys {
            if kc == code {
                return Action::ConversationChoice(n);
            }
        }
        if self.quest_panel_keys.contains(&code) {
            return Action::QuestPanel;
        }
        Action::None
    }
}

/// Map a key event (playing mode) to an Action.
///
/// Compatibility aliases are preserved alongside new-user-friendly keys.
/// Delegates to the default KeyConfig.
pub fn action_from_play_key(key: &KeyEvent) -> Action {
    KeyConfig::default().action_from_play_key(key)
}

/// Map a key event (inventory mode) to an InventoryAction.
///
/// Supports both cursor-based movement (arrows/j/k) and
/// direct letter shortcut selection for power users.
pub fn inventory_action_from_key(key: &KeyEvent, inventory_len: usize) -> InventoryAction {
    match key.code {
        // Close inventory — must come before the a-z slot-select catch-all
        // so that 'i' (and any future letter close keys) aren't eaten by it.
        KeyCode::Esc | KeyCode::Tab | KeyCode::Char('i') => InventoryAction::Close,
        // Cursor movement — must be checked BEFORE the a-z slot-select
        // catch-all so j/k/w/s work for navigation when inventory is large.
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('w') => InventoryAction::MoveUp,
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('s') => InventoryAction::MoveDown,
        // Direct slot select (a-z) — only letters NOT used for navigation
        // are available for direct slot access.
        KeyCode::Char(c) if c.is_ascii_lowercase() => {
            let slot = (c as u8).wrapping_sub(b'a') as usize;
            if slot < inventory_len {
                InventoryAction::Select(slot)
            } else {
                InventoryAction::None
            }
        }
        // Use equipped item on selection
        KeyCode::Enter | KeyCode::Char(' ') => InventoryAction::UseEquip,
        _ => InventoryAction::None,
    }
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    // ── action_from_play_key ──

    mod action_from_play_key {
        use super::*;

        #[test]
        fn arrow_keys_move_cardinals() {
            assert_eq!(
                action_from_play_key(&key(KeyCode::Left)),
                Action::Move(-1, 0)
            );
            assert_eq!(
                action_from_play_key(&key(KeyCode::Right)),
                Action::Move(1, 0)
            );
            assert_eq!(action_from_play_key(&key(KeyCode::Up)), Action::Move(0, -1));
            assert_eq!(
                action_from_play_key(&key(KeyCode::Down)),
                Action::Move(0, 1)
            );
        }

        #[test]
        fn wasd_keys_move_cardinals() {
            assert_eq!(
                action_from_play_key(&key(KeyCode::Char('a'))),
                Action::Move(-1, 0)
            );
            assert_eq!(
                action_from_play_key(&key(KeyCode::Char('d'))),
                Action::Move(1, 0)
            );
            assert_eq!(
                action_from_play_key(&key(KeyCode::Char('w'))),
                Action::Move(0, -1)
            );
            assert_eq!(
                action_from_play_key(&key(KeyCode::Char('s'))),
                Action::Move(0, 1)
            );
        }

        #[test]
        fn hjkl_keys_move_cardinals() {
            assert_eq!(
                action_from_play_key(&key(KeyCode::Char('h'))),
                Action::Move(-1, 0)
            );
            assert_eq!(
                action_from_play_key(&key(KeyCode::Char('l'))),
                Action::Move(1, 0)
            );
            assert_eq!(
                action_from_play_key(&key(KeyCode::Char('k'))),
                Action::Move(0, -1)
            );
            assert_eq!(
                action_from_play_key(&key(KeyCode::Char('j'))),
                Action::Move(0, 1)
            );
        }

        #[test]
        fn diagonal_keys_return_diagonal_moves() {
            assert_eq!(
                action_from_play_key(&key(KeyCode::Char('y'))),
                Action::Move(-1, -1)
            );
            assert_eq!(
                action_from_play_key(&key(KeyCode::Char('u'))),
                Action::Move(1, -1)
            );
            assert_eq!(
                action_from_play_key(&key(KeyCode::Char('b'))),
                Action::Move(-1, 1)
            );
            assert_eq!(
                action_from_play_key(&key(KeyCode::Char('n'))),
                Action::Move(1, 1)
            );
        }

        #[test]
        fn numpad_keys_map_to_diagonals() {
            assert_eq!(
                action_from_play_key(&key(KeyCode::Char('7'))),
                Action::Move(-1, -1)
            );
            assert_eq!(
                action_from_play_key(&key(KeyCode::Char('9'))),
                Action::Move(1, -1)
            );
            assert_eq!(
                action_from_play_key(&key(KeyCode::Char('1'))),
                Action::Move(-1, 1)
            );
            assert_eq!(
                action_from_play_key(&key(KeyCode::Char('3'))),
                Action::Move(1, 1)
            );
        }

        #[test]
        fn wait_keys() {
            assert_eq!(action_from_play_key(&key(KeyCode::Char('.'))), Action::Wait);
            assert_eq!(action_from_play_key(&key(KeyCode::Char(' '))), Action::Wait);
        }

        #[test]
        fn enter_is_context_interaction() {
            assert_eq!(action_from_play_key(&key(KeyCode::Enter)), Action::Interact);
        }

        #[test]
        fn g_is_pickup() {
            assert_eq!(
                action_from_play_key(&key(KeyCode::Char('g'))),
                Action::PickUp
            );
        }

        #[test]
        fn e_is_toggle_pickup() {
            assert_eq!(
                action_from_play_key(&key(KeyCode::Char('e'))),
                Action::TogglePickup
            );
        }

        #[test]
        fn tab_and_i_are_inventory() {
            assert_eq!(action_from_play_key(&key(KeyCode::Tab)), Action::Inventory);
            assert_eq!(
                action_from_play_key(&key(KeyCode::Char('i'))),
                Action::Inventory
            );
        }

        #[test]
        fn greater_than_is_descend() {
            assert_eq!(
                action_from_play_key(&key(KeyCode::Char('>'))),
                Action::Descend
            );
        }

        #[test]
        fn less_than_is_ascend() {
            assert_eq!(
                action_from_play_key(&key(KeyCode::Char('<'))),
                Action::Ascend
            );
        }

        #[test]
        fn x_is_auto_explore() {
            assert_eq!(
                action_from_play_key(&key(KeyCode::Char('x'))),
                Action::AutoExplore
            );
        }

        #[test]
        fn f5_is_save() {
            assert_eq!(action_from_play_key(&key(KeyCode::F(5))), Action::SaveGame);
        }

        #[test]
        fn f9_is_load() {
            assert_eq!(action_from_play_key(&key(KeyCode::F(9))), Action::LoadGame);
        }

        #[test]
        fn q_is_quit() {
            assert_eq!(action_from_play_key(&key(KeyCode::Char('q'))), Action::Quit);
        }

        #[test]
        fn t_is_talk() {
            assert_eq!(action_from_play_key(&key(KeyCode::Char('t'))), Action::Talk);
        }

        #[test]
        fn unknown_key_returns_none() {
            assert_eq!(action_from_play_key(&key(KeyCode::Char('z'))), Action::None);
        }
    }

    // ── inventory_action_from_key ──

    mod inventory_action_from_key {
        use super::*;

        #[test]
        fn direct_slot_select() {
            assert_eq!(
                inventory_action_from_key(&key(KeyCode::Char('a')), 5),
                InventoryAction::Select(0)
            );
            assert_eq!(
                inventory_action_from_key(&key(KeyCode::Char('c')), 5),
                InventoryAction::Select(2)
            );
        }

        #[test]
        fn direct_slot_out_of_range_is_none() {
            assert_eq!(
                inventory_action_from_key(&key(KeyCode::Char('z')), 5),
                InventoryAction::None
            );
        }

        #[test]
        fn arrow_keys_navigate() {
            assert_eq!(
                inventory_action_from_key(&key(KeyCode::Up), 5),
                InventoryAction::MoveUp
            );
            assert_eq!(
                inventory_action_from_key(&key(KeyCode::Down), 5),
                InventoryAction::MoveDown
            );
        }

        #[test]
        fn enter_is_use() {
            assert_eq!(
                inventory_action_from_key(&key(KeyCode::Enter), 5),
                InventoryAction::UseEquip
            );
        }

        #[test]
        fn tab_closes() {
            assert_eq!(
                inventory_action_from_key(&key(KeyCode::Tab), 5),
                InventoryAction::Close
            );
        }

        #[test]
        fn escape_closes() {
            assert_eq!(
                inventory_action_from_key(&key(KeyCode::Esc), 5),
                InventoryAction::Close
            );
        }

        #[test]
        fn i_closes_even_when_empty() {
            // Regression: 'i' was eaten by the a-z slot-select arm
            // and ignored when inventory_len == 0.
            assert_eq!(
                inventory_action_from_key(&key(KeyCode::Char('i')), 0),
                InventoryAction::Close
            );
            assert_eq!(
                inventory_action_from_key(&key(KeyCode::Char('i')), 5),
                InventoryAction::Close
            );
        }

        #[test]
        fn unknown_key_in_inventory_is_none() {
            assert_eq!(
                inventory_action_from_key(&key(KeyCode::Char('z')), 26),
                InventoryAction::Select(25)
            );
        }
    }

    // ── KeyConfig ──

    mod key_config {
        use super::*;

        #[test]
        fn default_config_handles_wasd() {
            let cfg = KeyConfig::default();
            assert_eq!(
                cfg.action_from_play_key(&key(KeyCode::Char('w'))),
                Action::Move(0, -1)
            );
            assert_eq!(
                cfg.action_from_play_key(&key(KeyCode::Char('s'))),
                Action::Move(0, 1)
            );
            assert_eq!(
                cfg.action_from_play_key(&key(KeyCode::Char('a'))),
                Action::Move(-1, 0)
            );
            assert_eq!(
                cfg.action_from_play_key(&key(KeyCode::Char('d'))),
                Action::Move(1, 0)
            );
        }

        #[test]
        fn config_fields_are_public() {
            let cfg = KeyConfig::default();
            assert!(!cfg.move_keys.is_empty());
            assert!(!cfg.wait_keys.is_empty());
        }
    }
}
