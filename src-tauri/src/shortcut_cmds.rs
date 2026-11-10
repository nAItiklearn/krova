//! Shortcut sheet (Wave 2): the rows the UI renders, rebinding, the live conflict
//! probe with suggestions, reset, and pause.

use std::sync::Arc;

use krova_core::shortcuts::{alternatives, Chord, ShortcutAction, ShortcutGroup, ShortcutMap};
use krova_core::Settings;
use krova_platform::{HotkeyHandle, Platform, PlatformError};
use serde::Serialize;
use tauri::{AppHandle, State};

use crate::commands::{change_settings, commit};
use crate::engine::{self, Msg};
use crate::model::{lock, AppState, Shared, ShortcutActionState};

fn group_name(g: ShortcutGroup) -> &'static str {
    match g {
        ShortcutGroup::Windows => "Windows",
        ShortcutGroup::Layouts => "Layouts",
        ShortcutGroup::Krova => "Krova",
    }
}

/// Whether the settings let this action's shortcut be registered at all.
fn feature_on(settings: &Settings, action: ShortcutAction) -> bool {
    settings.enabled
        && if action == ShortcutAction::Switcher {
            settings.layout_switcher_shortcut
        } else {
            settings.keyboard_shortcuts
        }
}

/// The rows of the shortcut sheet, in sheet order.
pub fn shortcut_rows(
    settings: &Settings,
    handle: Option<&HotkeyHandle>,
    supported: bool,
) -> Vec<ShortcutActionState> {
    settings
        .shortcuts
        .effective()
        .into_iter()
        .map(|(action, chord)| {
            let (status, message) = if !supported {
                ("unsupported", None)
            } else if chord.is_none() || !feature_on(settings, action) {
                ("off", None)
            } else {
                match handle {
                    Some(h) if h.is_registered(action) => ("active", None),
                    Some(h) => match h.refusal(action) {
                        Some(why) if why.contains("already used") => {
                            ("in_use", Some(why.to_owned()))
                        }
                        Some(why) => ("error", Some(why.to_owned())),
                        None => ("off", None),
                    },
                    None => ("off", None),
                }
            };
            ShortcutActionState {
                id: action.id(),
                label: action.label(),
                group: group_name(action.group()).to_owned(),
                chord: chord.map(|c| c.to_string()),
                default_chord: action.default_chord().map(|c| c.to_string()),
                status: status.to_owned(),
                message,
            }
        })
        .collect()
}

/// The answer of `probe_shortcut`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProbeResult {
    /// `"free"`, `"current"` (it is this action's chord already), `"conflict"` (another
    /// Krova action has it), `"in_use"` (another program has it), `"invalid"`
    /// (not a chord), `"unknown"` (this OS cannot check).
    pub status: String,
    /// The chord, normalised (`"Ctrl+Alt+Left"`), when it could be read.
    pub chord: Option<String>,
    /// The Krova action that has it, for `"conflict"`.
    pub conflict_with: Option<String>,
    /// One sentence for the UI.
    pub message: String,
    /// Up to three free chords with the same key.
    pub suggestions: Vec<String>,
}

/// Is `chord` free for `action`? Checks Krova's own map first, then asks the OS.
pub fn probe(
    platform: &dyn Platform,
    map: &ShortcutMap,
    action: ShortcutAction,
    text: &str,
) -> ProbeResult {
    let chord: Chord = match text.parse() {
        Ok(c) => c,
        Err(e) => {
            return ProbeResult {
                status: "invalid".into(),
                chord: None,
                conflict_with: None,
                message: e.to_string(),
                suggestions: Vec::new(),
            }
        }
    };
    let taken_by_krova = |c: Chord| map.owner_of(c, action).is_some();
    let is_current = |c: Chord| map.chord(action) == Some(c);
    // Krova holds its own registered chords, so the OS cannot tell "ours" from
    // "someone else's" for them: the current chord counts as free.
    let os_free = |c: Chord| -> Option<bool> {
        if is_current(c) {
            return Some(true);
        }
        match platform.probe_hotkey(c) {
            Ok(free) => Some(free),
            Err(PlatformError::Unsupported) => None,
            Err(_) => Some(false),
        }
    };
    let suggestions = || -> Vec<String> {
        alternatives(chord)
            .into_iter()
            .filter(|&c| !taken_by_krova(c) && os_free(c) == Some(true))
            .take(3)
            .map(|c| c.to_string())
            .collect()
    };
    let shown = chord.to_string();
    if let Some(other) = map.owner_of(chord, action) {
        return ProbeResult {
            status: "conflict".into(),
            chord: Some(shown.clone()),
            conflict_with: Some(other.id()),
            message: format!("{shown} is already {}", other.label()),
            suggestions: suggestions(),
        };
    }
    if is_current(chord) {
        return ProbeResult {
            status: "current".into(),
            chord: Some(shown.clone()),
            conflict_with: None,
            message: format!("{shown} is set"),
            suggestions: Vec::new(),
        };
    }
    match os_free(chord) {
        Some(true) => ProbeResult {
            status: "free".into(),
            chord: Some(shown.clone()),
            conflict_with: None,
            message: format!("{shown} is free"),
            suggestions: Vec::new(),
        },
        Some(false) => ProbeResult {
            status: "in_use".into(),
            chord: Some(shown.clone()),
            conflict_with: None,
            message: format!("{shown} is already used by another app"),
            suggestions: suggestions(),
        },
        None => ProbeResult {
            status: "unknown".into(),
            chord: Some(shown),
            conflict_with: None,
            message: "This system cannot check shortcuts".into(),
            suggestions: Vec::new(),
        },
    }
}

fn action_of(id: &str) -> Result<ShortcutAction, String> {
    ShortcutAction::from_id(id).ok_or_else(|| format!("there is no shortcut \"{id}\""))
}

/// Check a chord for an action before it is saved.
#[tauri::command]
pub fn probe_shortcut(
    shared: State<'_, Arc<Shared>>,
    action: String,
    chord: String,
) -> Result<ProbeResult, String> {
    let action = action_of(&action)?;
    let map = lock(&shared.model).config.settings.shortcuts.clone();
    Ok(probe(&*shared.platform, &map, action, &chord))
}

/// Give an action a chord, or switch it off with `null`. A chord another Krova
/// action has is refused (probe first to offer the user a choice).
#[tauri::command]
pub fn set_shortcut(
    app: AppHandle,
    shared: State<'_, Arc<Shared>>,
    action: String,
    chord: Option<String>,
) -> Result<AppState, String> {
    let action = action_of(&action)?;
    let chord = match chord {
        Some(text) => Some(text.parse::<Chord>().map_err(|e| e.to_string())?),
        None => None,
    };
    if let Some(c) = chord {
        let map = lock(&shared.model).config.settings.shortcuts.clone();
        if let Some(other) = map.owner_of(c, action) {
            return Err(format!("{c} is already {}", other.label()));
        }
    }
    change_settings(&shared, |s| s.shortcuts.set(action, chord))?;
    commit(&app, &shared)
}

/// Every shortcut back to its default.
#[tauri::command]
pub fn reset_shortcuts(app: AppHandle, shared: State<'_, Arc<Shared>>) -> Result<AppState, String> {
    change_settings(&shared, |s| s.shortcuts.reset())?;
    commit(&app, &shared)
}

/// Pause or resume auto-arrange (the same as `Ctrl+Alt+T` and the tray item).
#[tauri::command]
pub fn set_paused(shared: State<'_, Arc<Shared>>, paused: bool) -> Result<(), String> {
    engine::notify(&shared, Msg::SetPaused(paused));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use krova_core::swap::Direction;
    use krova_platform::NullPlatform;

    /// A platform where only the chords in `taken` are held by other programs.
    struct Desk {
        taken: Vec<&'static str>,
    }

    impl Platform for Desk {
        fn monitors(&self) -> Result<Vec<krova_platform::MonitorInfo>, PlatformError> {
            Ok(Vec::new())
        }
        fn place_window(
            &self,
            _: krova_platform::WindowId,
            _: krova_core::PxRect,
            _: krova_core::PxRect,
            _: bool,
        ) -> Result<krova_core::PxRect, PlatformError> {
            Err(PlatformError::Unsupported)
        }
        fn start_input(
            &self,
            _: krova_core::Modifier,
            _: std::sync::mpsc::Sender<krova_platform::InputEvent>,
        ) -> Result<krova_platform::InputHandle, PlatformError> {
            Err(PlatformError::Unsupported)
        }
        fn autostart_enabled(&self) -> Result<bool, PlatformError> {
            Ok(false)
        }
        fn set_autostart(&self, _: bool) -> Result<(), PlatformError> {
            Ok(())
        }
        fn create_overlay(
            &self,
        ) -> Result<Box<dyn krova_platform::overlay::Overlay>, PlatformError> {
            Err(PlatformError::Unsupported)
        }
        fn permission(&self) -> krova_platform::Permission {
            krova_platform::Permission::NotRequired
        }
        fn probe_hotkey(&self, chord: Chord) -> Result<bool, PlatformError> {
            Ok(!self.taken.contains(&chord.to_string().as_str()))
        }
    }

    const PAUSE: ShortcutAction = ShortcutAction::Pause;

    #[test]
    fn a_free_chord_is_free_and_the_current_one_is_current() {
        let desk = Desk { taken: vec![] };
        let map = ShortcutMap::new();
        let r = probe(&desk, &map, PAUSE, "ctrl+shift+p");
        assert_eq!(r.status, "free");
        assert_eq!(r.chord.as_deref(), Some("Ctrl+Shift+P"));
        let r = probe(&desk, &map, PAUSE, "Ctrl+Alt+T");
        assert_eq!(r.status, "current");
    }

    #[test]
    fn a_chord_of_another_action_is_a_conflict_with_free_suggestions() {
        let desk = Desk {
            taken: vec!["Ctrl+Alt+Shift+Left"],
        };
        let map = ShortcutMap::new();
        let r = probe(&desk, &map, PAUSE, "Ctrl+Alt+Left");
        assert_eq!(r.status, "conflict");
        assert_eq!(r.conflict_with.as_deref(), Some("move_left"));
        assert_eq!(r.message, "Ctrl+Alt+Left is already Move left");
        // Ctrl+Alt+Shift+Left is Krova's own swap chord (and also taken): skipped.
        assert!(!r.suggestions.iter().any(|s| s == "Ctrl+Alt+Shift+Left"));
        assert!(!r.suggestions.is_empty() && r.suggestions.len() <= 3);
        assert!(r.suggestions.iter().all(|s| s.ends_with("Left")));
    }

    #[test]
    fn a_chord_another_program_holds_is_in_use() {
        let desk = Desk {
            taken: vec!["Ctrl+Shift+P"],
        };
        let r = probe(&desk, &ShortcutMap::new(), PAUSE, "Ctrl+Shift+P");
        assert_eq!(r.status, "in_use");
        assert_eq!(r.message, "Ctrl+Shift+P is already used by another app");
        assert!(r.suggestions.iter().all(|s| s != "Ctrl+Shift+P"));
    }

    #[test]
    fn nonsense_is_invalid_and_an_os_without_shortcuts_is_unknown() {
        let desk = Desk { taken: vec![] };
        let r = probe(&desk, &ShortcutMap::new(), PAUSE, "Shift+P");
        assert_eq!(r.status, "invalid");
        assert!(r.message.contains("Ctrl, Alt or Win"));
        let r = probe(&NullPlatform, &ShortcutMap::new(), PAUSE, "Ctrl+Shift+P");
        assert_eq!(r.status, "unknown");
    }

    #[test]
    fn rows_follow_the_settings_and_the_registration() {
        let mut s = Settings::default();
        s.shortcuts.set(PAUSE, None);
        let handle = HotkeyHandle::new(
            vec![ShortcutAction::Move(Direction::Left)],
            vec![(
                ShortcutAction::Move(Direction::Right),
                "Ctrl+Alt+Right is already used by another app".into(),
            )],
            || {},
        );
        let rows = shortcut_rows(&s, Some(&handle), true);
        assert_eq!(rows.len(), 33);
        let row = |id: &str| rows.iter().find(|r| r.id == id).unwrap().clone();
        assert_eq!(row("move_left").status, "active");
        assert_eq!(row("move_left").group, "Windows");
        assert_eq!(row("move_left").chord.as_deref(), Some("Ctrl+Alt+Left"));
        assert_eq!(row("move_right").status, "in_use");
        assert!(row("move_right").message.unwrap().contains("already used"));
        assert_eq!(row("pause").status, "off");
        assert_eq!(row("pause").chord, None);
        assert_eq!(row("pause").default_chord.as_deref(), Some("Ctrl+Alt+T"));
        assert_eq!(row("switcher").group, "Layouts");
        assert_eq!(row("cheat_sheet").group, "Krova");
        // macOS today: nothing is registered.
        assert!(shortcut_rows(&s, None, false)
            .iter()
            .all(|r| r.status == "unsupported"));
        // Shortcuts switched off in settings read as off.
        s.keyboard_shortcuts = false;
        let rows = shortcut_rows(&s, Some(&handle), true);
        assert_eq!(
            rows.iter().find(|r| r.id == "move_left").unwrap().status,
            "off"
        );
    }

    #[test]
    fn unknown_action_ids_are_refused() {
        assert!(action_of("teleport").is_err());
        assert_eq!(action_of("zone_3"), Ok(ShortcutAction::Zone(3)));
    }
}
