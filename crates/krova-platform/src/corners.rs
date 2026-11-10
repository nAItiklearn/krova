//! Hot corners: the OS-independent half of the platform API.
//!
//! The rules live in `krova_core::corners`; a platform watches the cursor near
//! the outer screen corners, runs the state machine and reports each fire as a
//! [`CornerHit`]. Only Windows implements it (see `win_corners.rs`); elsewhere
//! [`crate::Platform::start_hot_corners`] answers `Unsupported`.

use krova_core::corners::{Corner, HotCornerAction, HotCornerSettings};
use krova_core::PxRect;

/// What the hot-corner watcher should do. Pushed again whenever it changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CornerConfig {
    /// Which corners do what; a corner without an action is not watched at all.
    pub settings: HotCornerSettings,
    /// Colour of the ripple feedback.
    pub accent: (u8, u8, u8),
}

impl CornerConfig {
    /// Nothing watched: the state while Krova is switched off.
    pub fn off(accent: (u8, u8, u8)) -> Self {
        Self {
            settings: HotCornerSettings {
                top_left: None,
                top_right: None,
                bottom_left: None,
                bottom_right: None,
                ..HotCornerSettings::default()
            },
            accent,
        }
    }
}

/// A hot corner fired.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CornerHit {
    pub corner: Corner,
    pub action: HotCornerAction,
    /// Full rect of the monitor the corner belongs to (match it against
    /// `MonitorInfo::descriptor.rect`).
    pub monitor: PxRect,
}

/// Keeps the hot-corner watcher alive; dropping it stops it.
pub struct HotCornersHandle {
    update: Box<dyn Fn(CornerConfig) + Send + Sync>,
    stop: Option<Box<dyn FnOnce() + Send>>,
}

impl HotCornersHandle {
    pub fn new(
        update: impl Fn(CornerConfig) + Send + Sync + 'static,
        stop: impl FnOnce() + Send + 'static,
    ) -> Self {
        Self {
            update: Box::new(update),
            stop: Some(Box::new(stop)),
        }
    }

    /// Apply new settings (also re-reads the monitors).
    pub fn update(&self, config: CornerConfig) {
        (self.update)(config);
    }
}

impl Drop for HotCornersHandle {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[test]
    fn the_handle_forwards_updates_and_stops_once() {
        let updates = Arc::new(AtomicU32::new(0));
        let stops = Arc::new(AtomicU32::new(0));
        let (u, s) = (Arc::clone(&updates), Arc::clone(&stops));
        let handle = HotCornersHandle::new(
            move |c: CornerConfig| {
                assert!(!c.settings.any());
                u.fetch_add(1, Ordering::SeqCst);
            },
            move || {
                s.fetch_add(1, Ordering::SeqCst);
            },
        );
        handle.update(CornerConfig::off((1, 2, 3)));
        assert_eq!(updates.load(Ordering::SeqCst), 1);
        drop(handle);
        assert_eq!(stops.load(Ordering::SeqCst), 1);
    }
}
