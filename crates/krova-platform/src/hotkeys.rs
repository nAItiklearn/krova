//! The global keyboard shortcuts, OS-independent: what is registered and how a
//! refusal is worded for the user.
//!
//! The map itself (actions, default chords, the user's changes) lives in
//! [`krova_core::shortcuts`]. The engine turns it into a [`HotkeySpec`]: one
//! [`HotkeyBinding`] per action that is switched on. The OS identifier of a binding
//! is its position in the spec plus one.

pub use krova_core::shortcuts::{Chord, Key, ShortcutAction};

/// One shortcut to register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HotkeyBinding {
    pub action: ShortcutAction,
    pub chord: Chord,
}

/// What the app asks the OS to register.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HotkeySpec {
    pub bindings: Vec<HotkeyBinding>,
}

impl HotkeySpec {
    /// The OS identifier of each binding, in registration order.
    pub fn ids(&self) -> impl Iterator<Item = (i32, HotkeyBinding)> + '_ {
        self.bindings
            .iter()
            .enumerate()
            .filter_map(|(i, b)| i32::try_from(i + 1).ok().map(|id| (id, *b)))
    }

    /// Which action the OS identifier `id` stands for.
    pub fn action_for_id(&self, id: i32) -> Option<ShortcutAction> {
        let index = usize::try_from(id).ok()?.checked_sub(1)?;
        self.bindings.get(index).map(|b| b.action)
    }

    pub fn wants(&self, action: ShortcutAction) -> bool {
        self.bindings.iter().any(|b| b.action == action)
    }
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
pub fn refusal_message(chord: Chord, why: Refusal) -> String {
    match why {
        Refusal::InUse => format!("{chord} is already used by another app"),
        Refusal::Other(code) => format!("{chord} could not be registered (error {code})"),
    }
}

/// A shortcut was pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HotkeyEvent {
    pub action: ShortcutAction,
}

#[cfg(test)]
mod tests {
    use super::*;
    use krova_core::shortcuts::ShortcutMap;
    use krova_core::swap::Direction;

    fn spec() -> HotkeySpec {
        HotkeySpec {
            bindings: ShortcutMap::new()
                .effective()
                .into_iter()
                .filter_map(|(action, chord)| {
                    Some(HotkeyBinding {
                        action,
                        chord: chord?,
                    })
                })
                .collect(),
        }
    }

    #[test]
    fn ids_resolve_back_to_their_action() {
        let s = spec();
        for (id, b) in s.ids() {
            assert!(id > 0);
            assert_eq!(s.action_for_id(id), Some(b.action));
        }
        assert_eq!(s.action_for_id(0), None);
        assert_eq!(s.action_for_id(-1), None);
        let past = i32::try_from(s.bindings.len() + 1).unwrap();
        assert_eq!(s.action_for_id(past), None);
        assert_eq!(
            s.action_for_id(1),
            Some(ShortcutAction::Move(Direction::Left))
        );
    }

    #[test]
    fn wants_follows_the_bindings() {
        let s = spec();
        assert!(s.wants(ShortcutAction::Switcher));
        assert!(!HotkeySpec::default().wants(ShortcutAction::Switcher));
    }

    #[test]
    fn refusals_are_worded_for_people() {
        let chord: Chord = "Win+Alt+3".parse().unwrap();
        assert_eq!(
            refusal_message(chord, Refusal::InUse),
            "Win+Alt+3 is already used by another app"
        );
        let chord: Chord = "Ctrl+Alt+Left".parse().unwrap();
        assert_eq!(
            refusal_message(chord, Refusal::Other(87)),
            "Ctrl+Alt+Left could not be registered (error 87)"
        );
    }
}
