//! Wave 4 platform types: the resting-cursor watcher behind focus follows mouse
//! and the hover zone picker, and what each OS layer supports of it.
//!
//! Not cfg-gated, so every platform (and the tests) sees the same types.

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU8, Ordering};

use krova_core::{Modifier, PxRect};

use crate::{modifier_code, modifier_from_code, WindowId};

/// What the wave 4 OS features can do on this platform. All `false` on macOS for
/// now; the UI hides the settings of features that are `false`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Wave4Capabilities {
    pub focus_follows_mouse: bool,
    pub hover_picker: bool,
    /// Can open an https URL in the default browser ("Check for updates").
    pub open_url: bool,
}

/// Live settings of the pointer watcher, shared with its thread. Changing them
/// takes effect at the next cursor stop, without restarting anything.
#[derive(Debug)]
pub struct PointerConfig {
    focus: AtomicBool,
    focus_delay_ms: AtomicI32,
    hover: AtomicBool,
    hover_modifier: AtomicU8,
}

impl Default for PointerConfig {
    fn default() -> Self {
        Self {
            focus: AtomicBool::new(false),
            focus_delay_ms: AtomicI32::new(krova_core::ffm::DEFAULT_DELAY_MS),
            hover: AtomicBool::new(false),
            hover_modifier: AtomicU8::new(modifier_code(Modifier::Shift)),
        }
    }
}

impl PointerConfig {
    pub fn set(&self, focus: bool, focus_delay_ms: i32, hover: bool, modifier: Modifier) {
        self.focus.store(focus, Ordering::Relaxed);
        self.focus_delay_ms.store(
            krova_core::ffm::clamp_delay(focus_delay_ms),
            Ordering::Relaxed,
        );
        self.hover.store(hover, Ordering::Relaxed);
        self.hover_modifier
            .store(modifier_code(modifier), Ordering::Relaxed);
    }

    /// The focus delay when focus follows mouse is on.
    pub fn focus_delay(&self) -> Option<i32> {
        self.focus
            .load(Ordering::Relaxed)
            .then(|| self.focus_delay_ms.load(Ordering::Relaxed))
    }

    pub fn hover(&self) -> bool {
        self.hover.load(Ordering::Relaxed)
    }

    pub fn hover_modifier(&self) -> Modifier {
        modifier_from_code(self.hover_modifier.load(Ordering::Relaxed))
    }

    /// Whether anything needs the watcher at all.
    pub fn any(&self) -> bool {
        self.focus_delay().is_some() || self.hover()
    }
}

/// What the pointer watcher reports. Never carries a window title.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerEvent {
    /// The cursor rests on the title bar of `window` with the modifier held.
    HoverCaption {
        window: WindowId,
        /// The window's visible frame.
        rect: PxRect,
        cursor: (i32, i32),
    },
    /// A picker was shown and the cursor came to rest somewhere else (or the
    /// modifier was released): close it.
    HoverEnd,
    /// Focus follows mouse activated this window.
    Focused(WindowId),
    /// The OS refused to activate this window (foreground-lock rules).
    FocusRefused(WindowId),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_round_trips_and_clamps() {
        let c = PointerConfig::default();
        assert!(!c.any());
        c.set(true, 9999, false, Modifier::Alt);
        assert_eq!(c.focus_delay(), Some(800));
        assert!(!c.hover());
        assert!(c.any());
        c.set(false, 200, true, Modifier::Ctrl);
        assert_eq!(c.focus_delay(), None);
        assert!(c.hover());
        assert_eq!(c.hover_modifier(), Modifier::Ctrl);
    }
}
