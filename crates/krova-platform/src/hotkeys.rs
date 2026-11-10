//! The global keyboard shortcuts, OS-independent: which keys there are, what each
//! one asks for, and how a refusal is worded for the user.
//!
//! The chord is `<modifiers>+<key>` where the modifiers come from
//! [`krova_core::ShortcutModifier`]. Thirteen shortcuts in two groups: the twelve
//! that move the focused window (Left, Right, Up and 1 to 9) and `Space`, which
//! opens the layout switcher. The two groups are switched on and off separately
//! ([`HotkeySpec`]).

use krova_core::registry::KeyAction;
use krova_core::ShortcutModifier;

/// The non-modifier key of a shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Left,
    Right,
    Up,
    /// `1` to `9` on the main keyboard row.
    Digit(u8),
    Space,
}

impl Key {
    /// Name as shown to the user (`Left`, `3`, `Space`).
    pub fn label(self) -> String {
        match self {
            Key::Left => "Left".to_owned(),
            Key::Right => "Right".to_owned(),
            Key::Up => "Up".to_owned(),
            Key::Digit(n) => n.to_string(),
            Key::Space => "Space".to_owned(),
        }
    }
}

/// One registered shortcut. `id` is the identifier handed to the OS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HotkeyDef {
    pub id: i32,
    pub key: Key,
    pub action: KeyAction,
}

const fn def(id: i32, key: Key, action: KeyAction) -> HotkeyDef {
    HotkeyDef { id, key, action }
}

/// Every shortcut Krova can register, in the order it registers them.
pub const HOTKEYS: [HotkeyDef; 13] = [
    def(1, Key::Left, KeyAction::PrevZone),
    def(2, Key::Right, KeyAction::NextZone),
    def(3, Key::Up, KeyAction::NextMonitor),
    def(4, Key::Digit(1), KeyAction::Zone(1)),
    def(5, Key::Digit(2), KeyAction::Zone(2)),
    def(6, Key::Digit(3), KeyAction::Zone(3)),
    def(7, Key::Digit(4), KeyAction::Zone(4)),
    def(8, Key::Digit(5), KeyAction::Zone(5)),
    def(9, Key::Digit(6), KeyAction::Zone(6)),
    def(10, Key::Digit(7), KeyAction::Zone(7)),
    def(11, Key::Digit(8), KeyAction::Zone(8)),
    def(12, Key::Digit(9), KeyAction::Zone(9)),
    def(13, Key::Space, KeyAction::OpenSwitcher),
];

/// Which action the OS identifier `id` stands for.
pub fn action_for_id(id: i32) -> Option<KeyAction> {
    HOTKEYS.iter().find(|h| h.id == id).map(|h| h.action)
}

/// `Win+Alt+3`, `Ctrl+Alt+Left`, ...
pub fn chord_label(modifier: ShortcutModifier, key: Key) -> String {
    format!("{}+{}", modifier.label(), key.label())
}

/// Why the OS refused to register a shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Another program (or Windows itself) already owns the chord.
    InUse,
    /// Any other failure, with the OS error code.
    Other(u32),
}

/// The sentence shown in the settings for a shortcut that could not be registered.
pub fn refusal_message(modifier: ShortcutModifier, key: Key, why: Refusal) -> String {
    let chord = chord_label(modifier, key);
    match why {
        Refusal::InUse => format!("{chord} is already used by another app"),
        Refusal::Other(code) => format!("{chord} could not be registered (error {code})"),
    }
}

/// What the app asked for when it started the shortcuts: the chord's modifiers and
/// which of the two groups to register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HotkeySpec {
    pub modifier: ShortcutModifier,
    /// The twelve chords that move the focused window.
    pub window_moves: bool,
    /// `<mods>+Space`: open the layout switcher.
    pub switcher: bool,
}

impl HotkeySpec {
    /// Whether the shortcut that asks for `action` belongs to a requested group.
    pub fn wants(self, action: KeyAction) -> bool {
        if action == KeyAction::OpenSwitcher {
            self.switcher
        } else {
            self.window_moves
        }
    }

    /// The shortcuts to register for this request, in registration order.
    pub fn defs(self) -> Vec<HotkeyDef> {
        HOTKEYS
            .iter()
            .copied()
            .filter(|h| self.wants(h.action))
            .collect()
    }
}

/// A shortcut was pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HotkeyEvent {
    pub action: KeyAction,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn there_are_thirteen_unique_shortcuts() {
        assert_eq!(HOTKEYS.len(), 13);
        let ids: HashSet<i32> = HOTKEYS.iter().map(|h| h.id).collect();
        assert_eq!(ids.len(), 13, "ids are unique");
        assert!(ids.iter().all(|&i| i > 0), "ids are positive");
        let labels: HashSet<String> = HOTKEYS.iter().map(|h| h.key.label()).collect();
        assert_eq!(labels.len(), 13, "keys are unique");
    }

    fn spec(window_moves: bool, switcher: bool) -> HotkeySpec {
        HotkeySpec {
            modifier: ShortcutModifier::CtrlAltShift,
            window_moves,
            switcher,
        }
    }

    #[test]
    fn the_two_groups_are_requested_separately() {
        let both = spec(true, true).defs();
        assert_eq!(both.len(), 13);
        assert_eq!(both, HOTKEYS.to_vec(), "registration order is unchanged");

        let moves = spec(true, false).defs();
        assert_eq!(moves.len(), 12);
        assert!(moves.iter().all(|h| h.action != KeyAction::OpenSwitcher));
        assert!(moves.iter().all(|h| h.key != Key::Space));

        let switcher = spec(false, true).defs();
        assert_eq!(switcher.len(), 1);
        assert_eq!(switcher[0].key, Key::Space);
        assert_eq!(switcher[0].action, KeyAction::OpenSwitcher);

        assert!(spec(false, false).defs().is_empty());
    }

    #[test]
    fn wants_follows_the_group_of_the_action() {
        assert!(spec(true, false).wants(KeyAction::NextZone));
        assert!(spec(true, false).wants(KeyAction::Zone(3)));
        assert!(!spec(true, false).wants(KeyAction::OpenSwitcher));
        assert!(spec(false, true).wants(KeyAction::OpenSwitcher));
        assert!(!spec(false, true).wants(KeyAction::PrevZone));
        assert!(!spec(false, true).wants(KeyAction::NextMonitor));
    }

    #[test]
    fn the_mapping_matches_the_documented_keyboard_map() {
        let find = |key| HOTKEYS.iter().find(|h| h.key == key).unwrap().action;
        assert_eq!(find(Key::Left), KeyAction::PrevZone);
        assert_eq!(find(Key::Right), KeyAction::NextZone);
        assert_eq!(find(Key::Up), KeyAction::NextMonitor);
        assert_eq!(find(Key::Space), KeyAction::OpenSwitcher);
        for n in 1..=9u8 {
            assert_eq!(find(Key::Digit(n)), KeyAction::Zone(n));
        }
        assert!(!HOTKEYS.iter().any(|h| h.key == Key::Digit(0)));
        assert!(!HOTKEYS.iter().any(|h| h.key == Key::Digit(10)));
    }

    #[test]
    fn ids_resolve_back_to_their_action() {
        for h in HOTKEYS {
            assert_eq!(action_for_id(h.id), Some(h.action));
        }
        assert_eq!(action_for_id(0), None);
        assert_eq!(action_for_id(13), Some(KeyAction::OpenSwitcher));
        assert_eq!(action_for_id(14), None);
        assert_eq!(action_for_id(-1), None);
    }

    #[test]
    fn chords_read_like_the_ui_text() {
        assert_eq!(
            chord_label(ShortcutModifier::WinAlt, Key::Digit(3)),
            "Win+Alt+3"
        );
        assert_eq!(
            chord_label(ShortcutModifier::CtrlAlt, Key::Left),
            "Ctrl+Alt+Left"
        );
        assert_eq!(
            chord_label(ShortcutModifier::CtrlAltShift, Key::Up),
            "Ctrl+Alt+Shift+Up"
        );
        assert_eq!(
            chord_label(ShortcutModifier::WinCtrl, Key::Right),
            "Win+Ctrl+Right"
        );
        assert_eq!(
            chord_label(ShortcutModifier::CtrlAltShift, Key::Space),
            "Ctrl+Alt+Shift+Space"
        );
    }

    #[test]
    fn refusals_are_worded_for_people() {
        assert_eq!(
            refusal_message(ShortcutModifier::WinAlt, Key::Digit(3), Refusal::InUse),
            "Win+Alt+3 is already used by another app"
        );
        assert_eq!(
            refusal_message(ShortcutModifier::WinCtrl, Key::Left, Refusal::Other(87)),
            "Win+Ctrl+Left could not be registered (error 87)"
        );
        assert_eq!(
            refusal_message(ShortcutModifier::CtrlAlt, Key::Space, Refusal::InUse),
            "Ctrl+Alt+Space is already used by another app"
        );
    }
}
