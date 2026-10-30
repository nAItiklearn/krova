//! OS abstraction for Krova.
//!
//! `krova-core` decides *what* should happen (zones, snapping, placement math);
//! this crate does the OS-specific *doing*: enumerate monitors, move windows,
//! watch the mouse for window drags, manage autostart. Everything the app needs
//! goes through the [`Platform`] trait so the shell stays OS-independent.

use std::path::Path;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

use krova_core::{DragEvent, Modifier, MonitorDescriptor, PxRect};

pub mod ids;
pub mod input;
#[cfg(target_os = "macos")]
pub mod mac;
pub mod mac_math;
pub mod overlay;
#[cfg(windows)]
pub mod win;
#[cfg(windows)]
pub mod win_overlay;

/// A connected monitor with everything the shell needs to lay zones on it.
#[derive(Debug, Clone, PartialEq)]
pub struct MonitorInfo {
    /// Identity and full rect, as used by `krova_core::Config::resolve_monitors`.
    pub descriptor: MonitorDescriptor,
    /// Work area: excludes taskbar / menu bar / dock.
    pub work: PxRect,
    /// Device pixels per logical pixel (1.0 = 96 dpi / 1x).
    pub scale: f64,
    pub label: String,
    pub primary: bool,
}

/// Opaque handle of a top-level window (HWND / CGWindowID-based).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(pub u64);

#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("not supported on this platform")]
    Unsupported,
    #[error("the window no longer exists")]
    WindowGone,
    /// The OS refuses to let us touch this window (e.g. it runs elevated).
    #[error("cannot move this window: {0}")]
    Refused(&'static str),
    #[error("os error: {0}")]
    Os(String),
}

/// State of the permission Krova needs to move other apps' windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    /// Nothing to grant (Windows).
    NotRequired,
    Granted,
    /// macOS Accessibility not yet granted: show onboarding.
    Denied,
}

/// A drag-related event plus the window it concerns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputEvent {
    pub event: DragEvent,
    /// The window being dragged (always set for events a drag produces).
    pub window: Option<WindowId>,
}

pub fn modifier_code(m: Modifier) -> u8 {
    match m {
        Modifier::Shift => 0,
        Modifier::Ctrl => 1,
        Modifier::Alt => 2,
        Modifier::Super => 3,
    }
}

pub fn modifier_from_code(code: u8) -> Modifier {
    match code {
        1 => Modifier::Ctrl,
        2 => Modifier::Alt,
        3 => Modifier::Super,
        _ => Modifier::Shift,
    }
}

/// Command line stored in the Windows `Run` key: the quoted executable path.
pub fn autostart_command(exe: &Path) -> String {
    format!("\"{}\"", exe.display())
}

/// Keeps the input monitor alive; dropping it stops the OS hook and its threads.
pub struct InputHandle {
    modifier: Arc<AtomicU8>,
    stop: Option<Box<dyn FnOnce() + Send>>,
}

impl InputHandle {
    pub fn new(modifier: Arc<AtomicU8>, stop: impl FnOnce() + Send + 'static) -> Self {
        Self {
            modifier,
            stop: Some(Box::new(stop)),
        }
    }

    /// Change the activation modifier of the running monitor.
    pub fn set_modifier(&self, m: Modifier) {
        self.modifier.store(modifier_code(m), Ordering::Relaxed);
    }
}

impl Drop for InputHandle {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            stop();
        }
    }
}

pub trait Platform: Send + Sync {
    /// Currently connected monitors, in OS enumeration order.
    fn monitors(&self) -> Result<Vec<MonitorInfo>, PlatformError>;

    /// Move/resize `window` so its *visible* frame fills `target`. `work` is the
    /// work area of the destination monitor; the window is kept inside it when
    /// its size limits prevent an exact fit. Restores a maximized window first.
    /// Blocks for the animation (about 120 ms) when `animate` is set, so call it
    /// off the UI thread. Returns the visible frame that was actually achieved.
    fn place_window(
        &self,
        window: WindowId,
        target: PxRect,
        work: PxRect,
        animate: bool,
    ) -> Result<PxRect, PlatformError>;

    /// Start watching for window drags. Events go to `sink` until the returned
    /// handle is dropped. Only one monitor may run at a time.
    fn start_input(
        &self,
        modifier: Modifier,
        sink: Sender<InputEvent>,
    ) -> Result<InputHandle, PlatformError>;

    fn autostart_enabled(&self) -> Result<bool, PlatformError>;
    fn set_autostart(&self, enabled: bool) -> Result<(), PlatformError>;

    /// Create the drag-highlight overlay; it stays hidden until it is given a scene.
    fn create_overlay(&self) -> Result<Box<dyn overlay::Overlay>, PlatformError>;

    fn permission(&self) -> Permission;
}

/// Platform for OSes Krova does not support; every operation fails cleanly.
/// Lets the workspace build and test on Linux CI.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullPlatform;

impl Platform for NullPlatform {
    fn monitors(&self) -> Result<Vec<MonitorInfo>, PlatformError> {
        Err(PlatformError::Unsupported)
    }

    fn place_window(
        &self,
        _window: WindowId,
        _target: PxRect,
        _work: PxRect,
        _animate: bool,
    ) -> Result<PxRect, PlatformError> {
        Err(PlatformError::Unsupported)
    }

    fn start_input(
        &self,
        _modifier: Modifier,
        _sink: Sender<InputEvent>,
    ) -> Result<InputHandle, PlatformError> {
        Err(PlatformError::Unsupported)
    }

    fn autostart_enabled(&self) -> Result<bool, PlatformError> {
        Err(PlatformError::Unsupported)
    }

    fn set_autostart(&self, _enabled: bool) -> Result<(), PlatformError> {
        Err(PlatformError::Unsupported)
    }

    fn create_overlay(&self) -> Result<Box<dyn overlay::Overlay>, PlatformError> {
        Err(PlatformError::Unsupported)
    }

    fn permission(&self) -> Permission {
        Permission::NotRequired
    }
}

#[cfg(windows)]
pub fn default_platform() -> Box<dyn Platform> {
    Box::new(win::WindowsPlatform::new())
}

#[cfg(target_os = "macos")]
pub fn default_platform() -> Box<dyn Platform> {
    Box::new(mac::MacPlatform::new())
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn default_platform() -> Box<dyn Platform> {
    Box::new(NullPlatform)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn modifier_codes_round_trip() {
        for m in [
            Modifier::Shift,
            Modifier::Ctrl,
            Modifier::Alt,
            Modifier::Super,
        ] {
            assert_eq!(modifier_from_code(modifier_code(m)), m);
        }
        assert_eq!(modifier_from_code(200), Modifier::Shift);
    }

    #[test]
    fn handle_runs_stop_exactly_once_on_drop() {
        let stops = Arc::new(AtomicU8::new(0));
        let s = Arc::clone(&stops);
        let handle = InputHandle::new(Arc::new(AtomicU8::new(0)), move || {
            s.fetch_add(1, Ordering::SeqCst);
        });
        assert_eq!(stops.load(Ordering::SeqCst), 0);
        drop(handle);
        assert_eq!(stops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn handle_updates_the_shared_modifier() {
        let cell = Arc::new(AtomicU8::new(modifier_code(Modifier::Shift)));
        let handle = InputHandle::new(Arc::clone(&cell), || {});
        handle.set_modifier(Modifier::Alt);
        assert_eq!(
            modifier_from_code(cell.load(Ordering::Relaxed)),
            Modifier::Alt
        );
    }

    #[test]
    fn autostart_command_quotes_paths_with_spaces() {
        let exe = Path::new(r"C:\Program Files\Krova\krova.exe");
        assert_eq!(
            autostart_command(exe),
            r#""C:\Program Files\Krova\krova.exe""#
        );
    }

    #[test]
    fn null_platform_fails_cleanly() {
        let p = NullPlatform;
        assert!(matches!(p.monitors(), Err(PlatformError::Unsupported)));
        assert!(matches!(
            p.autostart_enabled(),
            Err(PlatformError::Unsupported)
        ));
        let (tx, _rx) = mpsc::channel();
        assert!(matches!(
            p.start_input(Modifier::Shift, tx),
            Err(PlatformError::Unsupported)
        ));
    }
}
