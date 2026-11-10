//! The global shortcut map: every action Krova binds, its default chord, and the
//! user's changes.
//!
//! Base chord `Ctrl+Alt`. Every action can be rebound or switched off one by one;
//! [`ShortcutMap`] stores only what the user changed (`action id -> chord | null`),
//! so new actions in later versions get their default without a migration.
//!
//! Chords travel as text: `Ctrl+Alt+Left`, `Ctrl+Alt+Shift+3`, `Win+Alt+F1`. The
//! modifiers are written in the fixed order Win, Ctrl, Alt, Shift and parsed in any
//! order, case-insensitively. A chord needs Ctrl, Alt or Win: Shift alone (or no
//! modifier) would steal ordinary typing.
//!
//! Known collisions of the defaults: `Ctrl+Alt+Left` / `Right` are "navigate back /
//! forward" in several IDEs (Visual Studio, JetBrains) and `Ctrl+Alt+Arrow` rotates
//! the screen on PCs with older Intel graphics drivers (when their hot keys are on).
//! Windows then refuses the chord and the UI shows it as in use, with suggestions.

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::config::ShortcutModifier;
use crate::swap::Direction;

/// The non-modifier key of a chord.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Key {
    Left,
    Right,
    Up,
    Down,
    /// `0` to `9` on the main row.
    Digit(u8),
    /// `A` to `Z` (stored upper case).
    Letter(char),
    /// `F1` to `F12`.
    F(u8),
    Space,
    Enter,
    Slash,
    BracketLeft,
    BracketRight,
    Comma,
    Period,
    Minus,
    Equals,
}

impl Key {
    pub fn label(self) -> String {
        match self {
            Key::Left => "Left".into(),
            Key::Right => "Right".into(),
            Key::Up => "Up".into(),
            Key::Down => "Down".into(),
            Key::Digit(n) => n.to_string(),
            Key::Letter(c) => c.to_string(),
            Key::F(n) => format!("F{n}"),
            Key::Space => "Space".into(),
            Key::Enter => "Enter".into(),
            Key::Slash => "/".into(),
            Key::BracketLeft => "[".into(),
            Key::BracketRight => "]".into(),
            Key::Comma => ",".into(),
            Key::Period => ".".into(),
            Key::Minus => "-".into(),
            Key::Equals => "=".into(),
        }
    }

    fn parse(s: &str) -> Option<Key> {
        let lower = s.to_ascii_lowercase();
        Some(match lower.as_str() {
            "left" => Key::Left,
            "right" => Key::Right,
            "up" => Key::Up,
            "down" => Key::Down,
            "space" => Key::Space,
            "enter" | "return" => Key::Enter,
            "/" | "slash" => Key::Slash,
            "[" => Key::BracketLeft,
            "]" => Key::BracketRight,
            "," => Key::Comma,
            "." => Key::Period,
            "-" => Key::Minus,
            "=" => Key::Equals,
            _ => {
                let mut chars = s.chars();
                let first = chars.next()?;
                let rest: String = chars.collect();
                if rest.is_empty() && first.is_ascii_digit() {
                    Key::Digit(first as u8 - b'0')
                } else if rest.is_empty() && first.is_ascii_alphabetic() {
                    Key::Letter(first.to_ascii_uppercase())
                } else if (first == 'f' || first == 'F') && !rest.is_empty() {
                    let n: u8 = rest.parse().ok()?;
                    if (1..=12).contains(&n) {
                        Key::F(n)
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            }
        })
    }
}

/// A key with its modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Chord {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
    pub key: Key,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChordError {
    #[error("\"{0}\" is not a shortcut (write it like Ctrl+Alt+Left)")]
    Malformed(String),
    #[error("a shortcut needs Ctrl, Alt or Win")]
    NoModifier,
}

impl Chord {
    pub const fn new(ctrl: bool, alt: bool, shift: bool, win: bool, key: Key) -> Self {
        Self {
            ctrl,
            alt,
            shift,
            win,
            key,
        }
    }

    /// `Ctrl+Alt+<key>`.
    pub const fn ctrl_alt(key: Key) -> Self {
        Self::new(true, true, false, false, key)
    }

    /// `Ctrl+Alt+Shift+<key>`.
    pub const fn ctrl_alt_shift(key: Key) -> Self {
        Self::new(true, true, true, false, key)
    }

    /// The same key with other modifiers.
    pub fn with_mods(self, ctrl: bool, alt: bool, shift: bool, win: bool) -> Self {
        Self::new(ctrl, alt, shift, win, self.key)
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts: Vec<String> = Vec::with_capacity(5);
        for (on, name) in [
            (self.win, "Win"),
            (self.ctrl, "Ctrl"),
            (self.alt, "Alt"),
            (self.shift, "Shift"),
        ] {
            if on {
                parts.push(name.to_owned());
            }
        }
        parts.push(self.key.label());
        f.write_str(&parts.join("+"))
    }
}

impl FromStr for Chord {
    type Err = ChordError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bad = || ChordError::Malformed(s.chars().take(40).collect());
        let text = s.trim();
        if text.is_empty() || text.len() > 40 {
            return Err(bad());
        }
        // "+" itself is not a key Krova binds, so a plain split is enough; the last
        // part is the key.
        let parts: Vec<&str> = text.split('+').map(str::trim).collect();
        let (key, mods) = parts.split_last().ok_or_else(bad)?;
        let key = Key::parse(key).ok_or_else(bad)?;
        let mut chord = Chord::new(false, false, false, false, key);
        for m in mods {
            let flag = match m.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => &mut chord.ctrl,
                "alt" | "option" => &mut chord.alt,
                "shift" => &mut chord.shift,
                "win" | "super" | "meta" | "cmd" => &mut chord.win,
                _ => return Err(bad()),
            };
            if *flag {
                return Err(bad());
            }
            *flag = true;
        }
        if !(chord.ctrl || chord.alt || chord.win) {
            return Err(ChordError::NoModifier);
        }
        Ok(chord)
    }
}

impl Serialize for Chord {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Chord {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

/// Where an action is listed in the shortcut sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ShortcutGroup {
    Windows,
    Layouts,
    Krova,
}

/// Everything a global shortcut can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShortcutAction {
    /// Move the focused window to the neighbouring zone (past the edge: the next
    /// monitor in that direction).
    Move(Direction),
    /// Swap the focused window with the one in the neighbouring zone.
    Swap(Direction),
    /// Move the focused window to zone N (1..=9, reading order).
    Zone(u8),
    /// Apply layout N (1..=9, the monitor's list order) on the monitor under the cursor.
    ApplyLayout(u8),
    /// Pause or resume auto-arrange (windows stay where they are).
    Pause,
    /// Open the layout switcher.
    Switcher,
    /// Show every shortcut.
    CheatSheet,
    /// Previous / next window of the focused window's stack.
    StackPrev,
    StackNext,
    /// Focus mode (implemented by the focus feature; the engine only reports it).
    Focus,
    /// Let the focused window fill its monitor, and back into its zone.
    MaximizeInZone,
}

fn dir_id(d: Direction) -> &'static str {
    match d {
        Direction::Left => "left",
        Direction::Right => "right",
        Direction::Up => "up",
        Direction::Down => "down",
    }
}

fn dir_key(d: Direction) -> Key {
    match d {
        Direction::Left => Key::Left,
        Direction::Right => Key::Right,
        Direction::Up => Key::Up,
        Direction::Down => Key::Down,
    }
}

impl ShortcutAction {
    /// Every action, in the order of the shortcut sheet.
    pub fn all() -> Vec<ShortcutAction> {
        let mut v = Vec::with_capacity(32);
        v.extend(Direction::ALL.map(ShortcutAction::Move));
        v.extend(Direction::ALL.map(ShortcutAction::Swap));
        v.extend((1..=9).map(ShortcutAction::Zone));
        v.extend([
            ShortcutAction::MaximizeInZone,
            ShortcutAction::StackPrev,
            ShortcutAction::StackNext,
        ]);
        v.extend((1..=9).map(ShortcutAction::ApplyLayout));
        v.push(ShortcutAction::Switcher);
        v.extend([
            ShortcutAction::Pause,
            ShortcutAction::CheatSheet,
            ShortcutAction::Focus,
        ]);
        v
    }

    /// Stable id, the key of [`ShortcutMap`]: `move_left`, `zone_3`, `pause`, ...
    pub fn id(self) -> String {
        match self {
            ShortcutAction::Move(d) => format!("move_{}", dir_id(d)),
            ShortcutAction::Swap(d) => format!("swap_{}", dir_id(d)),
            ShortcutAction::Zone(n) => format!("zone_{n}"),
            ShortcutAction::ApplyLayout(n) => format!("layout_{n}"),
            ShortcutAction::Pause => "pause".into(),
            ShortcutAction::Switcher => "switcher".into(),
            ShortcutAction::CheatSheet => "cheat_sheet".into(),
            ShortcutAction::StackPrev => "stack_prev".into(),
            ShortcutAction::StackNext => "stack_next".into(),
            ShortcutAction::Focus => "focus".into(),
            ShortcutAction::MaximizeInZone => "maximize_in_zone".into(),
        }
    }

    pub fn from_id(id: &str) -> Option<ShortcutAction> {
        Self::all().into_iter().find(|a| a.id() == id)
    }

    /// One to four words for the shortcut sheet.
    pub fn label(self) -> String {
        let dir = |d: Direction| match d {
            Direction::Left => "left",
            Direction::Right => "right",
            Direction::Up => "up",
            Direction::Down => "down",
        };
        match self {
            ShortcutAction::Move(d) => format!("Move {}", dir(d)),
            ShortcutAction::Swap(d) => format!("Swap {}", dir(d)),
            ShortcutAction::Zone(n) => format!("Zone {n}"),
            ShortcutAction::ApplyLayout(n) => format!("Layout {n}"),
            ShortcutAction::Pause => "Pause arranging".into(),
            ShortcutAction::Switcher => "Layout switcher".into(),
            ShortcutAction::CheatSheet => "All shortcuts".into(),
            ShortcutAction::StackPrev => "Previous tab".into(),
            ShortcutAction::StackNext => "Next tab".into(),
            ShortcutAction::Focus => "Focus".into(),
            ShortcutAction::MaximizeInZone => "Fill screen".into(),
        }
    }

    pub fn group(self) -> ShortcutGroup {
        match self {
            ShortcutAction::Move(_)
            | ShortcutAction::Swap(_)
            | ShortcutAction::Zone(_)
            | ShortcutAction::StackPrev
            | ShortcutAction::StackNext
            | ShortcutAction::MaximizeInZone => ShortcutGroup::Windows,
            ShortcutAction::ApplyLayout(_) | ShortcutAction::Switcher => ShortcutGroup::Layouts,
            ShortcutAction::Pause | ShortcutAction::CheatSheet | ShortcutAction::Focus => {
                ShortcutGroup::Krova
            }
        }
    }

    /// The chord this action has unless the user changed it.
    pub fn default_chord(self) -> Option<Chord> {
        Some(match self {
            ShortcutAction::Move(d) => Chord::ctrl_alt(dir_key(d)),
            ShortcutAction::Swap(d) => Chord::ctrl_alt_shift(dir_key(d)),
            ShortcutAction::Zone(n) => Chord::ctrl_alt(Key::Digit(n)),
            ShortcutAction::ApplyLayout(n) => Chord::ctrl_alt_shift(Key::Digit(n)),
            ShortcutAction::Pause => Chord::ctrl_alt(Key::Letter('T')),
            ShortcutAction::Switcher => Chord::ctrl_alt(Key::Space),
            ShortcutAction::CheatSheet => Chord::ctrl_alt(Key::Slash),
            ShortcutAction::StackPrev => Chord::ctrl_alt(Key::BracketLeft),
            ShortcutAction::StackNext => Chord::ctrl_alt(Key::BracketRight),
            ShortcutAction::Focus => Chord::ctrl_alt(Key::Letter('F')),
            ShortcutAction::MaximizeInZone => Chord::ctrl_alt(Key::Enter),
        })
    }
}

/// The user's changes to the default map: `action id -> chord`, or `null` for an
/// action switched off. Unknown ids and unreadable chords are dropped on load.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
#[serde(transparent)]
pub struct ShortcutMap {
    overrides: BTreeMap<String, Option<Chord>>,
}

impl<'de> Deserialize<'de> for ShortcutMap {
    /// Lenient on purpose: a bad entry is dropped, never the whole config.
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(d)?;
        Ok(Self::from_value(&value))
    }
}

impl ShortcutMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Lenient parse of a stored map: bad entries are skipped.
    pub fn from_value(value: &serde_json::Value) -> Self {
        let mut map = Self::default();
        let Some(object) = value.as_object() else {
            return map;
        };
        for (id, chord) in object.iter().take(256) {
            let Some(action) = ShortcutAction::from_id(id) else {
                continue;
            };
            match chord {
                serde_json::Value::Null => map.set(action, None),
                serde_json::Value::String(text) => {
                    if let Ok(c) = text.parse() {
                        map.set(action, Some(c));
                    }
                }
                _ => {}
            }
        }
        map
    }

    /// The chord of `action` now: the user's choice, else the default.
    pub fn chord(&self, action: ShortcutAction) -> Option<Chord> {
        match self.overrides.get(&action.id()) {
            Some(choice) => *choice,
            None => action.default_chord(),
        }
    }

    /// Give `action` a chord (`None` switches it off). Choosing the default removes
    /// the override.
    pub fn set(&mut self, action: ShortcutAction, chord: Option<Chord>) {
        if chord == action.default_chord() {
            self.overrides.remove(&action.id());
        } else {
            self.overrides.insert(action.id(), chord);
        }
    }

    /// Back to the defaults.
    pub fn reset(&mut self) {
        self.overrides.clear();
    }

    pub fn is_default(&self) -> bool {
        self.overrides.is_empty()
    }

    /// Every action with its chord now.
    pub fn effective(&self) -> Vec<(ShortcutAction, Option<Chord>)> {
        ShortcutAction::all()
            .into_iter()
            .map(|a| (a, self.chord(a)))
            .collect()
    }

    /// The other action that already uses `chord`, if any.
    pub fn owner_of(&self, chord: Chord, except: ShortcutAction) -> Option<ShortcutAction> {
        self.effective()
            .into_iter()
            .find(|&(a, c)| a != except && c == Some(chord))
            .map(|(a, _)| a)
    }

    /// Repair: unknown ids dropped; when two actions share a chord, the one later in
    /// the sheet is switched off.
    pub fn sanitized(mut self) -> Self {
        self.overrides
            .retain(|id, _| ShortcutAction::from_id(id).is_some());
        let mut seen: Vec<Chord> = Vec::new();
        for (action, chord) in self.effective() {
            if let Some(c) = chord {
                if seen.contains(&c) {
                    self.overrides.insert(action.id(), None);
                } else {
                    seen.push(c);
                }
            }
        }
        self
    }

    /// Carry over the old `shortcut_modifier` setting: a user who had picked
    /// `Win+Alt` or `Win+Ctrl` keeps it for the shortcuts that existed then (move
    /// left / right / up, zones 1-9, the switcher). Ctrl+Alt is the new base, and
    /// Ctrl+Alt+Shift (the old default) now belongs to swapping, so both start from
    /// the new defaults.
    pub fn migrated_from(modifier: ShortcutModifier) -> Self {
        let (ctrl, alt, shift, win) = match modifier {
            ShortcutModifier::WinAlt => (false, true, false, true),
            ShortcutModifier::WinCtrl => (true, false, false, true),
            ShortcutModifier::CtrlAlt | ShortcutModifier::CtrlAltShift => return Self::default(),
        };
        let mut map = Self::default();
        let mut old: Vec<ShortcutAction> = vec![
            ShortcutAction::Move(Direction::Left),
            ShortcutAction::Move(Direction::Right),
            ShortcutAction::Move(Direction::Up),
            ShortcutAction::Switcher,
        ];
        old.extend((1..=9).map(ShortcutAction::Zone));
        for action in old {
            if let Some(c) = action.default_chord() {
                map.set(action, Some(c.with_mods(ctrl, alt, shift, win)));
            }
        }
        map
    }
}

/// Other chords for the same key, in the order worth offering, for a chord that is
/// taken. The caller filters out the ones in use.
pub fn alternatives(chord: Chord) -> Vec<Chord> {
    let options = [
        (true, true, false, false),
        (true, true, true, false),
        (true, false, true, false),
        (false, true, true, false),
        (true, false, false, true),
        (false, true, false, true),
        (true, true, false, true),
        (false, false, true, true),
    ];
    options
        .into_iter()
        .map(|(c, a, s, w)| chord.with_mods(c, a, s, w))
        .filter(|c| *c != chord)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chords_round_trip_through_text() {
        for text in [
            "Ctrl+Alt+Left",
            "Ctrl+Alt+Shift+3",
            "Win+Alt+F1",
            "Ctrl+Alt+/",
            "Ctrl+Alt+[",
            "Ctrl+Alt+Enter",
            "Alt+Shift+T",
        ] {
            let c: Chord = text.parse().unwrap();
            assert_eq!(c.to_string(), text);
        }
        let c: Chord = " shift+ALT+ctrl+t ".parse().unwrap();
        assert_eq!(c.to_string(), "Ctrl+Alt+Shift+T");
        let c: Chord = "Cmd+Option+Space".parse().unwrap();
        assert_eq!(c.to_string(), "Win+Alt+Space");
    }

    #[test]
    fn bad_chords_are_refused() {
        for text in [
            "",
            "Left",
            "Shift+Left",
            "Ctrl+Ctrl+A",
            "Ctrl+Hyper+A",
            "Ctrl+F13",
            "Ctrl+AB",
        ] {
            assert!(text.parse::<Chord>().is_err(), "{text}");
        }
        assert_eq!("Shift+A".parse::<Chord>(), Err(ChordError::NoModifier));
        let long = "Ctrl+".repeat(50);
        assert!(matches!(long.parse::<Chord>(), Err(ChordError::Malformed(m)) if m.len() <= 40));
    }

    #[test]
    fn every_action_has_a_unique_id_and_default_chord() {
        let all = ShortcutAction::all();
        assert_eq!(all.len(), 33);
        let mut ids: Vec<String> = all.iter().map(|a| a.id()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), all.len());
        let mut chords: Vec<Chord> = all.iter().filter_map(|a| a.default_chord()).collect();
        assert_eq!(chords.len(), all.len());
        chords.sort();
        chords.dedup();
        assert_eq!(chords.len(), all.len(), "no two defaults collide");
        for a in all {
            assert_eq!(ShortcutAction::from_id(&a.id()), Some(a));
            let words = a.label().split_whitespace().count();
            assert!((1..=4).contains(&words), "{}", a.label());
        }
    }

    #[test]
    fn the_defaults_are_the_ctrl_alt_scheme() {
        let m = ShortcutMap::new();
        let text = |a| m.chord(a).map(|c| c.to_string());
        assert_eq!(
            text(ShortcutAction::Move(Direction::Left)).as_deref(),
            Some("Ctrl+Alt+Left")
        );
        assert_eq!(
            text(ShortcutAction::Swap(Direction::Down)).as_deref(),
            Some("Ctrl+Alt+Shift+Down")
        );
        assert_eq!(text(ShortcutAction::Zone(4)).as_deref(), Some("Ctrl+Alt+4"));
        assert_eq!(
            text(ShortcutAction::ApplyLayout(2)).as_deref(),
            Some("Ctrl+Alt+Shift+2")
        );
        assert_eq!(text(ShortcutAction::Pause).as_deref(), Some("Ctrl+Alt+T"));
        assert_eq!(
            text(ShortcutAction::Switcher).as_deref(),
            Some("Ctrl+Alt+Space")
        );
        assert_eq!(
            text(ShortcutAction::CheatSheet).as_deref(),
            Some("Ctrl+Alt+/")
        );
        assert_eq!(
            text(ShortcutAction::StackNext).as_deref(),
            Some("Ctrl+Alt+]")
        );
        assert_eq!(text(ShortcutAction::Focus).as_deref(), Some("Ctrl+Alt+F"));
        assert_eq!(
            text(ShortcutAction::MaximizeInZone).as_deref(),
            Some("Ctrl+Alt+Enter")
        );
    }

    #[test]
    fn only_changes_are_stored_and_off_is_null() {
        let mut m = ShortcutMap::new();
        m.set(ShortcutAction::Pause, None);
        m.set(ShortcutAction::Focus, Some("Ctrl+Shift+F".parse().unwrap()));
        m.set(
            ShortcutAction::Zone(1),
            ShortcutAction::Zone(1).default_chord(),
        );
        let json = serde_json::to_value(&m).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"focus": "Ctrl+Shift+F", "pause": null})
        );
        let back = ShortcutMap::from_value(&json);
        assert_eq!(back, m);
        assert_eq!(back.chord(ShortcutAction::Pause), None);
        m.reset();
        assert!(m.is_default());
    }

    #[test]
    fn a_stored_map_with_bad_entries_keeps_the_good_ones() {
        let json = serde_json::json!({
            "pause": "Ctrl+Alt+P",
            "teleport": "Ctrl+Alt+X",
            "focus": "Shift+F",
            "zone_2": 7,
        });
        let m = ShortcutMap::from_value(&json);
        assert_eq!(
            m.chord(ShortcutAction::Pause).unwrap().to_string(),
            "Ctrl+Alt+P"
        );
        assert_eq!(
            m.chord(ShortcutAction::Focus),
            ShortcutAction::Focus.default_chord()
        );
        // A non-object or a wrongly typed map loads as the defaults.
        assert!(ShortcutMap::from_value(&serde_json::json!([1, 2])).is_default());
    }

    #[test]
    fn sanitising_switches_off_a_later_duplicate() {
        let mut m = ShortcutMap::new();
        // Give "pause" the chord of "move left" (earlier in the sheet).
        m.overrides
            .insert("pause".into(), Some("Ctrl+Alt+Left".parse().unwrap()));
        let m = m.sanitized();
        assert_eq!(m.chord(ShortcutAction::Pause), None);
        assert!(m.chord(ShortcutAction::Move(Direction::Left)).is_some());
        assert_eq!(
            m.owner_of("Ctrl+Alt+Left".parse().unwrap(), ShortcutAction::Pause),
            Some(ShortcutAction::Move(Direction::Left))
        );
    }

    #[test]
    fn the_old_modifier_carries_over_only_when_it_was_a_win_chord() {
        assert!(ShortcutMap::migrated_from(ShortcutModifier::CtrlAltShift).is_default());
        assert!(ShortcutMap::migrated_from(ShortcutModifier::CtrlAlt).is_default());
        let m = ShortcutMap::migrated_from(ShortcutModifier::WinAlt);
        assert_eq!(
            m.chord(ShortcutAction::Zone(3)).unwrap().to_string(),
            "Win+Alt+3"
        );
        assert_eq!(
            m.chord(ShortcutAction::Switcher).unwrap().to_string(),
            "Win+Alt+Space"
        );
        assert_eq!(
            m.chord(ShortcutAction::Move(Direction::Down))
                .unwrap()
                .to_string(),
            "Ctrl+Alt+Down",
            "a shortcut that did not exist then keeps its new default"
        );
    }

    #[test]
    fn alternatives_keep_the_key_and_skip_the_chord_itself() {
        let c: Chord = "Ctrl+Alt+Left".parse().unwrap();
        let alts = alternatives(c);
        assert!(!alts.contains(&c));
        assert!(alts.iter().all(|a| a.key == Key::Left));
        assert_eq!(alts[0].to_string(), "Ctrl+Alt+Shift+Left");
    }
}
