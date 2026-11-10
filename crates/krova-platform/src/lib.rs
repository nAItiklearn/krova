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

use krova_core::registry::KeyAction;
use krova_core::{DragEvent, Modifier, MonitorDescriptor, PxRect};

pub mod hotkeys;
pub mod ids;
pub mod input;
#[cfg(target_os = "macos")]
pub mod mac;
pub mod mac_math;
pub mod overlay;
#[cfg(windows)]
pub mod win;
#[cfg(windows)]
pub mod win_events;
#[cfg(windows)]
pub mod win_overlay;
// --- wave3 ---
pub mod desk;
#[cfg(windows)]
pub mod win_desk;
#[cfg(windows)]
pub mod win_visuals;
// --- end wave3 ---

pub use hotkeys::{HotkeyEvent, HotkeySpec};

/// Where an OS-level notification is delivered. The closure runs on the platform's
/// own thread and must only enqueue (a channel send) and return at once.
pub type EventSink<T> = Arc<dyn Fn(T) + Send + Sync>;

/// What this platform can do beyond moving windows by mouse. The UI hides the
/// settings of features that are `false`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Capabilities {
    /// Global keyboard shortcuts for moving windows between zones.
    pub shortcuts: bool,
    /// Reading where windows are (reconnect memory, linked dividers).
    pub window_memory: bool,
}

/// Something happened that the engine wants to know about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchEvent {
    /// The user started moving or resizing this window (the OS move/size loop).
    MoveSizeStart(WindowId),
    /// The user finished moving or resizing it.
    MoveSizeEnd(WindowId),
    /// The set or layout of the monitors may have changed (display change, work
    /// area change, resume from sleep). Debounce before acting on it.
    DisplayChanged,
}

/// What happened to one group of shortcuts when they were registered.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChordReport {
    /// How many the OS accepted.
    pub registered: usize,
    /// One sentence per shortcut the OS refused, ready for the settings page.
    pub failed: Vec<String>,
}

/// Keeps the global shortcuts registered; dropping it unregisters them and stops
/// their thread.
pub struct HotkeyHandle {
    /// What each accepted shortcut asks for.
    accepted: Vec<KeyAction>,
    /// What each refused shortcut asked for, and the sentence that says why.
    refused: Vec<(KeyAction, String)>,
    stop: Option<Box<dyn FnOnce() + Send>>,
}

impl HotkeyHandle {
    pub fn new(
        accepted: Vec<KeyAction>,
        refused: Vec<(KeyAction, String)>,
        stop: impl FnOnce() + Send + 'static,
    ) -> Self {
        Self {
            accepted,
            refused,
            stop: Some(Box::new(stop)),
        }
    }

    /// How many shortcuts the OS accepted, in both groups.
    pub fn registered(&self) -> usize {
        self.accepted.len()
    }

    /// One sentence per shortcut the OS refused, in both groups.
    pub fn failed(&self) -> Vec<String> {
        self.refused.iter().map(|(_, why)| why.clone()).collect()
    }

    fn report(&self, switcher: bool) -> ChordReport {
        let in_group = |action: &KeyAction| (*action == KeyAction::OpenSwitcher) == switcher;
        ChordReport {
            registered: self.accepted.iter().filter(|a| in_group(a)).count(),
            failed: self
                .refused
                .iter()
                .filter(|(a, _)| in_group(a))
                .map(|(_, why)| why.clone())
                .collect(),
        }
    }

    /// The twelve shortcuts that move the focused window.
    pub fn window_moves(&self) -> ChordReport {
        self.report(false)
    }

    /// `<mods>+Space`, which opens the layout switcher.
    pub fn switcher(&self) -> ChordReport {
        self.report(true)
    }
}

impl Drop for HotkeyHandle {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            stop();
        }
    }
}

/// Keeps the window-event watcher alive; dropping it removes the hooks.
pub struct WatchHandle {
    stop: Option<Box<dyn FnOnce() + Send>>,
}

impl WatchHandle {
    pub fn new(stop: impl FnOnce() + Send + 'static) -> Self {
        Self {
            stop: Some(Box::new(stop)),
        }
    }
}

impl Drop for WatchHandle {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            stop();
        }
    }
}

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

    // ---- Optional capabilities. Every method below has a default that says
    // "not supported", so a platform only overrides what it can really do.

    fn capabilities(&self) -> Capabilities {
        Capabilities::default()
    }

    /// Does the window still exist?
    fn window_exists(&self, _window: WindowId) -> bool {
        false
    }

    /// The window's visible frame right now (the same space `place_window` uses).
    fn window_rect(&self, _window: WindowId) -> Result<PxRect, PlatformError> {
        Err(PlatformError::Unsupported)
    }

    fn is_minimized(&self, _window: WindowId) -> bool {
        false
    }

    fn is_maximized(&self, _window: WindowId) -> bool {
        false
    }

    /// The window that has the keyboard focus, if any.
    fn foreground_window(&self) -> Option<WindowId> {
        None
    }

    /// Where the mouse cursor is, in the pixels of [`MonitorInfo::descriptor`]'s rect
    /// (virtual-screen coordinates). `None` when it cannot be read.
    fn cursor_position(&self) -> Option<(i32, i32)> {
        None
    }

    /// Whether Krova may move this window: an ordinary, visible, un-minimized
    /// application window of another program that is not protected (elevated).
    fn is_movable_target(&self, _window: WindowId) -> bool {
        false
    }

    /// Register the global shortcuts. They stay registered until the handle is
    /// dropped. Shortcuts the OS refuses are reported in the handle, not as an error.
    fn start_hotkeys(
        &self,
        _spec: HotkeySpec,
        _sink: EventSink<HotkeyEvent>,
    ) -> Result<HotkeyHandle, PlatformError> {
        Err(PlatformError::Unsupported)
    }

    /// Watch for window move/resize and display changes until the handle is dropped.
    fn start_watcher(&self, _sink: EventSink<WatchEvent>) -> Result<WatchHandle, PlatformError> {
        Err(PlatformError::Unsupported)
    }
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

    #[test]
    fn optional_capabilities_default_to_unsupported() {
        let p = NullPlatform;
        let w = WindowId(1);
        assert_eq!(p.capabilities(), Capabilities::default());
        assert!(!p.capabilities().shortcuts && !p.capabilities().window_memory);
        assert!(!p.window_exists(w));
        assert!(matches!(p.window_rect(w), Err(PlatformError::Unsupported)));
        assert!(!p.is_minimized(w));
        assert!(!p.is_maximized(w));
        assert_eq!(p.foreground_window(), None);
        assert_eq!(p.cursor_position(), None);
        assert!(!p.is_movable_target(w));
        let spec = HotkeySpec {
            modifier: krova_core::ShortcutModifier::WinAlt,
            window_moves: true,
            switcher: true,
        };
        assert!(matches!(
            p.start_hotkeys(spec, Arc::new(|_| {})),
            Err(PlatformError::Unsupported)
        ));
        assert!(matches!(
            p.start_watcher(Arc::new(|_| {})),
            Err(PlatformError::Unsupported)
        ));
    }

    #[test]
    fn handles_run_their_stop_action_exactly_once() {
        let stops = Arc::new(AtomicU8::new(0));
        let s = Arc::clone(&stops);
        let hotkeys = HotkeyHandle::new(
            vec![KeyAction::PrevZone, KeyAction::NextZone],
            vec![(
                KeyAction::Zone(3),
                "Win+Alt+3 is already used by another app".into(),
            )],
            move || {
                s.fetch_add(1, Ordering::SeqCst);
            },
        );
        assert_eq!(hotkeys.registered(), 2);
        assert_eq!(hotkeys.failed().len(), 1);
        drop(hotkeys);
        assert_eq!(stops.load(Ordering::SeqCst), 1);

        let s = Arc::clone(&stops);
        let watch = WatchHandle::new(move || {
            s.fetch_add(10, Ordering::SeqCst);
        });
        assert_eq!(stops.load(Ordering::SeqCst), 1);
        drop(watch);
        assert_eq!(stops.load(Ordering::SeqCst), 11);
    }

    #[test]
    fn a_hotkey_handle_reports_the_two_groups_apart() {
        let chord = |what: &str| format!("Ctrl+Alt+Shift+{what} is already used by another app");
        let handle = HotkeyHandle::new(
            vec![
                KeyAction::PrevZone,
                KeyAction::Zone(1),
                KeyAction::OpenSwitcher,
            ],
            vec![
                (KeyAction::Zone(2), chord("2")),
                (KeyAction::Zone(3), chord("3")),
            ],
            || {},
        );
        assert_eq!(handle.registered(), 3);
        assert_eq!(handle.failed(), [chord("2"), chord("3")]);
        assert_eq!(
            handle.window_moves(),
            ChordReport {
                registered: 2,
                failed: vec![chord("2"), chord("3")]
            }
        );
        assert_eq!(
            handle.switcher(),
            ChordReport {
                registered: 1,
                failed: vec![]
            }
        );

        // The switcher chord refused while the moves are fine.
        let handle = HotkeyHandle::new(
            vec![KeyAction::NextZone],
            vec![(KeyAction::OpenSwitcher, chord("Space"))],
            || {},
        );
        assert_eq!(handle.window_moves().failed, Vec::<String>::new());
        assert_eq!(handle.switcher().registered, 0);
        assert_eq!(handle.switcher().failed, [chord("Space")]);

        // Nothing requested for a group: its report is empty, not an error.
        let handle = HotkeyHandle::new(vec![KeyAction::OpenSwitcher], vec![], || {});
        assert_eq!(handle.window_moves(), ChordReport::default());
    }
}
