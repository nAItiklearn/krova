//! What each global shortcut does (Wave 2). The chords themselves are in
//! `krova_core::shortcuts`; this file turns a pressed action into engine work.

use std::time::Instant;

use krova_core::registry::{KeyAction, WindowKey};
use krova_core::shortcuts::ShortcutAction;
use krova_core::MoveMode;
use krova_platform::WindowId;

use super::Engine;
use crate::actions::Slot;
use crate::hooks::EngineEvent;
use crate::model::lock;

impl Engine<'_> {
    pub(super) fn on_hotkey(&mut self, action: ShortcutAction) {
        match action {
            ShortcutAction::Switcher => self.open_switcher(),
            ShortcutAction::CheatSheet => self.open_view("cheat_sheet"),
            ShortcutAction::Pause => {
                let paused = !self.shared.is_paused();
                self.set_paused(paused);
            }
            ShortcutAction::Focus => self.shared.hooks.emit(&EngineEvent::Action("focus")),
            ShortcutAction::ApplyLayout(n) => self.apply_layout_under_cursor(n),
            _ if !self.window_keys_active() => {}
            ShortcutAction::Move(dir) => self.key_move(KeyAction::Move(dir)),
            ShortcutAction::Zone(n) => self.key_move(KeyAction::Zone(n)),
            ShortcutAction::Swap(dir) => {
                // A window auto-arrange does not track simply moves that way.
                if !self.swap_focused(dir) {
                    self.key_move(KeyAction::Move(dir));
                }
            }
            ShortcutAction::StackPrev => self.cycle_focused(false),
            ShortcutAction::StackNext => self.cycle_focused(true),
            ShortcutAction::MaximizeInZone => self.toggle_fill(),
        }
    }

    fn window_keys_active(&self) -> bool {
        let model = lock(&self.shared.model);
        model.config.settings.enabled && model.config.settings.keyboard_shortcuts
    }

    /// Ask the shell to show a view (`"cheat_sheet"`, `"panel"`, ...).
    pub(super) fn open_view(&self, view: &'static str) {
        // Take the opener out of its lock first: opening can create a window.
        let open = lock(&self.shared.open_view).clone();
        if let Some(open) = open {
            open(view);
        }
    }

    /// Tell the UI and the tray that the state changed.
    pub(super) fn push_state(&self) {
        let notify = lock(&self.shared.state_changed).clone();
        if let Some(notify) = notify {
            notify();
        }
    }

    /// Pause or resume auto-arrange. Windows stay where they are either way.
    pub(super) fn set_paused(&mut self, paused: bool) {
        if self.shared.is_paused() == paused {
            return;
        }
        self.shared.set_paused(paused);
        if paused {
            // Nothing waiting may arrange itself later.
            self.arrange.clear_pending();
        }
        self.shared.hooks.emit(&EngineEvent::PauseChanged(paused));
        self.push_state();
    }

    /// `Ctrl+Alt+Shift+N`: apply the monitor's layout N on the monitor under the
    /// cursor. Auto-arrange re-tiles the windows it looks after.
    fn apply_layout_under_cursor(&mut self, n: u8) {
        if !lock(&self.shared.model).config.settings.enabled {
            return;
        }
        self.notice_monitor_changes();
        let cursor = self.shared.platform.cursor_position();
        let applied = {
            let mut model = lock(&self.shared.model);
            let Some(profile) = model.monitor_for_switcher(cursor) else {
                return;
            };
            let layout_id = model
                .config
                .monitors
                .iter()
                .find(|m| m.id == profile)
                .and_then(|m| m.layouts.get(usize::from(n).saturating_sub(1)))
                .filter(|_| n >= 1)
                .map(|l| l.id.clone());
            let Some(layout_id) = layout_id else {
                return;
            };
            let ok = model.config.set_active_layout(&profile, &layout_id).is_ok();
            if ok {
                if let Err(e) = model.persist() {
                    eprintln!("krova: could not save the config: {e}");
                }
            }
            ok
        };
        if applied {
            self.on_config_changed();
            self.shared.hooks.emit(&EngineEvent::SettingsChanged);
            self.push_state();
        }
    }

    /// `Ctrl+Alt+Enter`: a window in a zone fills its monitor's work area; pressed
    /// again it goes back into its zone. A window in no zone is left alone.
    fn toggle_fill(&mut self) {
        let platform = &self.shared.platform;
        let Some(window) = platform.foreground_window() else {
            return;
        };
        if !platform.is_movable_target(window) {
            return;
        }
        let key = WindowKey(window.0);
        let animate = lock(&self.shared.model).config.settings.move_mode == MoveMode::Animate;
        if let Some(slot) = self.filled.remove(&key) {
            let ctx = lock(&self.shared.model).monitor_ctx(&slot.profile_id);
            let gap = lock(&self.shared.model).config.settings.zone_gap_px;
            let Some(m) = ctx.filter(|m| m.layout.id == slot.layout_id) else {
                return;
            };
            let Some(target) = m.layout.zone_pixels_gapped(slot.zone_id, m.work, gap) else {
                return;
            };
            if let Ok(done) = platform.place_window(window, target, m.work, animate) {
                self.remember(window, slot, done);
            }
            return;
        }
        let Some(record) = lock(&self.shared.registry).get(key).cloned() else {
            return;
        };
        let Some(m) = lock(&self.shared.model).monitor_ctx(&record.monitor_profile_id) else {
            return;
        };
        if platform
            .place_window(window, m.work, m.work, animate)
            .is_ok()
        {
            self.placed_at.insert(key, Instant::now());
            self.filled.insert(
                key,
                Slot {
                    profile_id: record.monitor_profile_id,
                    layout_id: record.layout_id,
                    zone_id: record.zone_id,
                },
            );
            self.shared.hooks.emit(&EngineEvent::WindowMoved {
                window: window.0,
                rect: m.work,
            });
        }
    }

    /// A filled window that was closed or moved is no longer "filled".
    pub(super) fn forget_filled(&mut self, window: WindowId) {
        self.filled.remove(&WindowKey(window.0));
    }
}
