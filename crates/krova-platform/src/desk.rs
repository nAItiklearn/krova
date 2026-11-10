//! Wave 3 OS seam: workspaces, calm desk, focus dim and window highlight.
//!
//! Kept apart from [`crate::Platform`] so the wave-3 features add a trait instead
//! of growing the shared one. Every method has a default that says "not
//! supported"; a platform overrides only what it really does. macOS currently
//! overrides nothing, so its capabilities are all `false`.
//!
//! Privacy: a window is described only by its handle, its frame and the full path
//! of its executable. Titles are never read.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use krova_core::focus::VisualsConfig;
use krova_core::PxRect;

use crate::{PlatformError, WindowId};

/// Which wave-3 features this OS build supports. The UI hides the others.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DeskCapabilities {
    pub workspaces: bool,
    pub calm_desk: bool,
    pub focus_dim: bool,
    pub highlight: bool,
    /// Windows 11 can colour window borders itself (no overlay needed).
    pub dwm_borders: bool,
}

/// A top-level window an app could be arranged with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeskWindow {
    pub id: WindowId,
    /// Full path of the executable (`QueryFullProcessImageNameW`); `None` when the
    /// process cannot be inspected.
    pub exe: Option<String>,
    /// Visible frame (minimised windows: their restored frame is not known, so the
    /// last visible frame or an empty rect).
    pub rect: PxRect,
    pub minimized: bool,
}

/// The running focus-dim / highlight overlays. Dropping it removes every overlay,
/// resets every border colour Krova set and ends the thread.
pub trait Visuals: Send {
    /// Apply new settings (colour, width, opacity, mode) without restarting.
    fn set_config(&self, config: VisualsConfig);
}

pub trait Desk: Send + Sync {
    fn capabilities(&self) -> DeskCapabilities {
        DeskCapabilities::default()
    }

    /// The OS build number (Windows: 22000 and up is Windows 11). 0 if unknown.
    fn os_build(&self) -> u32 {
        0
    }

    /// Visible, arrangeable top-level windows of other programs, topmost first.
    /// With `include_minimized`, minimised ones too.
    fn arrangeable_windows(&self, _include_minimized: bool) -> Vec<DeskWindow> {
        Vec::new()
    }

    /// Minimise without activating anything else; never closes.
    fn minimize(&self, _window: WindowId) -> Result<(), PlatformError> {
        Err(PlatformError::Unsupported)
    }

    /// Restore a minimised window to its previous frame without stealing focus.
    fn unminimize(&self, _window: WindowId) -> Result<(), PlatformError> {
        Err(PlatformError::Unsupported)
    }

    fn is_minimized(&self, _window: WindowId) -> bool {
        false
    }

    /// Folders programs are never launched from (the user and system temp folders).
    fn temp_dirs(&self) -> Vec<String> {
        Vec::new()
    }

    /// Whether a regular file exists at `path`.
    fn file_exists(&self, path: &str) -> bool {
        std::path::Path::new(path).is_file()
    }

    /// Start a program from its executable path, with no arguments, in its own
    /// folder. The caller has already checked the path with
    /// `krova_core::workspace::check_launch_path`.
    fn launch(&self, _exe: &str) -> Result<(), PlatformError> {
        Err(PlatformError::Unsupported)
    }

    /// Wait (event-driven, no polling) until each of `exes` has a new arrangeable
    /// window that is not in `known`, or until `timeout`. Returns one slot per entry
    /// of `exes`, in order; two equal paths get two different windows.
    fn wait_for_windows(
        &self,
        exes: &[String],
        _known: &[WindowId],
        _timeout: Duration,
    ) -> Vec<Option<WindowId>> {
        vec![None; exes.len()]
    }

    /// Start the focus-dim / highlight overlays. Only call while
    /// [`VisualsConfig::is_active`]; drop the handle to stop.
    fn start_visuals(&self, _config: VisualsConfig) -> Result<Box<dyn Visuals>, PlatformError> {
        Err(PlatformError::Unsupported)
    }
}

/// A platform without wave-3 support.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullDesk;

impl Desk for NullDesk {}

#[cfg(windows)]
pub fn default_desk() -> Box<dyn Desk> {
    Box::new(crate::win_desk::WinDesk::new())
}

#[cfg(not(windows))]
pub fn default_desk() -> Box<dyn Desk> {
    Box::new(NullDesk)
}

// ------------------------------------------------------------ share-safe

static SHARE_SAFE: AtomicBool = AtomicBool::new(false);

/// Mirror of the `share_safe` setting for native overlays, set by the app whenever
/// the settings change. While true, no Krova overlay draws an app name, an icon or
/// window contents: only zone shapes.
pub fn set_share_safe(on: bool) {
    SHARE_SAFE.store(on, Ordering::Relaxed);
}

/// See [`set_share_safe`].
pub fn share_safe() -> bool {
    SHARE_SAFE.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_null_desk_supports_nothing() {
        let d = NullDesk;
        assert_eq!(d.capabilities(), DeskCapabilities::default());
        assert!(d.arrangeable_windows(true).is_empty());
        assert!(d.minimize(WindowId(1)).is_err());
        assert!(d.launch("C:\\a.exe").is_err());
        assert_eq!(
            d.wait_for_windows(&["a".into(), "b".into()], &[], Duration::ZERO),
            vec![None, None]
        );
        let cfg = VisualsConfig::from_settings(
            &krova_core::Settings::default(),
            &krova_core::focus::FocusSettings::default(),
            &krova_core::focus::HighlightSettings::default(),
        );
        assert!(d.start_visuals(cfg).is_err());
    }

    #[test]
    fn share_safe_is_a_shared_switch() {
        set_share_safe(true);
        assert!(share_safe());
        set_share_safe(false);
        assert!(!share_safe());
    }
}
