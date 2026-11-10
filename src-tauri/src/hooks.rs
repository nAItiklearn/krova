//! Engine extension seam: features built on other branches (focus dimming, calm
//! desk, share-safe, window highlight, reserved edges, focus follows mouse, ...)
//! register a hook here and hear about what the engine sees, without the engine
//! knowing about them.
//!
//! A hook runs on the engine thread, right after the event, and must return
//! quickly (send to its own channel or flip a flag; never block, never call back
//! into the engine synchronously). Hooks are called in registration order. No
//! event carries a window title.

use std::sync::{Arc, Mutex};

use krova_core::PxRect;

use crate::model::lock;

/// What the engine saw.
#[derive(Debug, Clone, PartialEq)]
pub enum EngineEvent {
    /// The foreground window changed (`window` is the platform window id).
    FocusChanged { window: u64 },
    /// Krova put a window at `rect` (a snap, a shortcut, auto-arrange, a restore).
    WindowMoved { window: u64, rect: PxRect },
    /// The set or the layout of the connected monitors changed (after debouncing).
    MonitorsChanged,
    /// Settings or layouts were changed by the UI or the tray.
    SettingsChanged,
    /// Auto-arrange was paused (`true`) or resumed.
    PauseChanged(bool),
    /// A shortcut, hot corner or notch menu asked for a feature the engine does not
    /// implement itself: `"focus"`, `"calm_desk"`, `"show_zones"`, ...
    Action(&'static str),
}

pub type Hook = Arc<dyn Fn(&EngineEvent) + Send + Sync>;

/// The registered hooks.
#[derive(Default)]
pub struct Hooks {
    hooks: Mutex<Vec<Hook>>,
}

impl Hooks {
    /// Add a hook; it stays for the life of the app. Called by the features of the
    /// wave 3 and wave 4 branches.
    #[allow(dead_code)]
    pub fn register(&self, hook: Hook) {
        lock(&self.hooks).push(hook);
    }

    /// Tell every hook. The list is copied first, so a hook may register another.
    pub fn emit(&self, event: &EngineEvent) {
        let hooks = lock(&self.hooks).clone();
        for hook in hooks {
            hook(event);
        }
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        lock(&self.hooks).len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hooks_hear_every_event_in_order() {
        let hooks = Hooks::default();
        let seen = Arc::new(Mutex::new(Vec::new()));
        for tag in ["a", "b"] {
            let seen = Arc::clone(&seen);
            hooks.register(Arc::new(move |e| lock(&seen).push((tag, e.clone()))));
        }
        assert_eq!(hooks.len(), 2);
        hooks.emit(&EngineEvent::Action("focus"));
        hooks.emit(&EngineEvent::PauseChanged(true));
        let seen = lock(&seen).clone();
        assert_eq!(seen.len(), 4);
        assert_eq!(seen[0], ("a", EngineEvent::Action("focus")));
        assert_eq!(seen[1], ("b", EngineEvent::Action("focus")));
        assert_eq!(seen[3], ("b", EngineEvent::PauseChanged(true)));
    }

    #[test]
    fn a_hook_may_register_another_while_being_called() {
        let hooks = Arc::new(Hooks::default());
        let inner = Arc::clone(&hooks);
        hooks.register(Arc::new(move |_| {
            if inner.len() < 3 {
                inner.register(Arc::new(|_| {}));
            }
        }));
        hooks.emit(&EngineEvent::MonitorsChanged);
        assert_eq!(hooks.len(), 2);
    }
}
