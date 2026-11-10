//! The resting-cursor watcher on Windows, shared by focus follows mouse and the
//! hover zone picker.
//!
//! It reuses Krova's existing `WH_MOUSE_LL` hook (`win.rs`): the hook calls
//! [`on_hook`], which stores the cursor position in two atomics and pokes a
//! capacity-1 channel, nothing more. A worker thread sleeps on that channel. After
//! a movement it starts one one-shot wait per enabled feature
//! (`krova_core::ffm::rest_waits`); any further movement restarts them. So while
//! the cursor is still, or while both features are off, nothing runs at all.
//!
//! The hook only exists while Krova is enabled (it is the drag hook), so both
//! features pause with Krova's master switch.

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use krova_core::ffm::{hover_wanted, rest_waits, RestAction};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    SendMessageTimeoutW, HTCAPTION, SMTO_ABORTIFHUNG, WM_NCHITTEST,
};

use crate::pointer::{PointerConfig, PointerEvent};
use crate::win::{
    hwnd, is_own_process_window, modifier_down, snappable, target_is_protected, visible_rect,
    window_at, window_id,
};
use crate::win_ffm::{any_button_down, focus_at, FocusOutcome};
use crate::{EventSink, PlatformError, WatchHandle, WindowId};

/// How long a window may take to answer `WM_NCHITTEST`; a hung window is skipped.
const HIT_TEST_TIMEOUT_MS: u32 = 20;

const WM_MOUSEMOVE: u32 = 0x0200;
/// `WM_LBUTTONDOWN` ..= `WM_MBUTTONDBLCLK` and `WM_XBUTTONDOWN` ..= `WM_XBUTTONDBLCLK`.
const BUTTON_MESSAGES: [std::ops::RangeInclusive<u32>; 2] = [0x0201..=0x0209, 0x020B..=0x020D];

static ACTIVE: AtomicBool = AtomicBool::new(false);
static CLICKED: AtomicBool = AtomicBool::new(false);
static X: AtomicI32 = AtomicI32::new(0);
static Y: AtomicI32 = AtomicI32::new(0);
static POKE: Mutex<Option<SyncSender<()>>> = Mutex::new(None);

/// Called by the mouse hook for every mouse message. Must return at once: two
/// atomic stores and a non-blocking send, and nothing while the watcher is off.
pub(crate) fn on_hook(message: u32, x: i32, y: i32) {
    if !ACTIVE.load(Ordering::Relaxed) {
        return;
    }
    if message == WM_MOUSEMOVE {
        X.store(x, Ordering::Relaxed);
        Y.store(y, Ordering::Relaxed);
        if let Ok(slot) = POKE.try_lock() {
            if let Some(tx) = slot.as_ref() {
                // Full means the worker already knows the cursor moved.
                let _ = tx.try_send(());
            }
        }
    } else if BUTTON_MESSAGES.iter().any(|r| r.contains(&message)) {
        CLICKED.store(true, Ordering::Relaxed);
    }
}

/// `MAKELPARAM(x, y)` for screen coordinates (each a signed 16-bit value).
fn point_lparam(x: i32, y: i32) -> LPARAM {
    let lo = (x as i16 as u16) as u32;
    let hi = (y as i16 as u16) as u32;
    LPARAM(((hi << 16) | lo) as i32 as isize)
}

/// Is `(x, y)` on the title bar of `h`? Asked of the window itself, with a short
/// timeout so a hung program cannot stall the watcher.
fn on_caption(h: HWND, x: i32, y: i32) -> bool {
    let mut result = 0usize;
    let answered = unsafe {
        SendMessageTimeoutW(
            h,
            WM_NCHITTEST,
            WPARAM(0),
            point_lparam(x, y),
            SMTO_ABORTIFHUNG,
            HIT_TEST_TIMEOUT_MS,
            Some(&mut result),
        )
    };
    answered.0 != 0 && result == HTCAPTION as usize
}

/// `movable && hit()`, with `hit` (the cross-process hit test) only run for a
/// movable window.
fn caption_hit(movable: bool, hit: impl FnOnce() -> bool) -> bool {
    movable && hit()
}

fn hover_at(config: &PointerConfig, x: i32, y: i32) -> Option<PointerEvent> {
    let modifier = modifier_down(config.hover_modifier());
    let button = any_button_down();
    // Cheap checks first: most rests happen without the modifier held.
    if !hover_wanted(config.hover(), modifier, button, true, true) {
        return None;
    }
    let h = window_at(x, y)?;
    let movable = snappable(h) && !is_own_process_window(h) && !target_is_protected(h);
    // F-28: a rejected window (protected, elevated, Krova's own) never receives
    // WM_NCHITTEST; the hit test only runs for a window we could move.
    let caption = caption_hit(movable, || on_caption(h, x, y));
    if !hover_wanted(true, true, false, caption, movable) {
        return None;
    }
    Some(PointerEvent::HoverCaption {
        window: window_id(h),
        rect: visible_rect(h)?,
        cursor: (x, y),
    })
}

struct Worker {
    config: Arc<PointerConfig>,
    sink: EventSink<PointerEvent>,
    picker_open: bool,
    /// The window the OS refused last time (a `WindowId`: an `HWND` is not `Send`).
    refused: Option<WindowId>,
}

impl Worker {
    fn act(&mut self, action: RestAction) {
        let (x, y) = (X.load(Ordering::Relaxed), Y.load(Ordering::Relaxed));
        match action {
            RestAction::Hover => match hover_at(&self.config, x, y) {
                Some(ev) => {
                    self.picker_open = true;
                    (self.sink)(ev);
                }
                // Resting on Krova's own picker keeps it open: that is where the
                // user is about to click.
                None if self.picker_open && !window_at(x, y).is_some_and(is_own_process_window) => {
                    self.picker_open = false;
                    (self.sink)(PointerEvent::HoverEnd);
                }
                None => {}
            },
            RestAction::Focus => {
                let clicked = CLICKED.load(Ordering::Relaxed);
                match focus_at(x, y, clicked, self.refused.map(hwnd)) {
                    FocusOutcome::Skipped => {}
                    FocusOutcome::Focused(h) => {
                        self.refused = None;
                        (self.sink)(PointerEvent::Focused(window_id(h)));
                    }
                    FocusOutcome::Refused(h) => {
                        self.refused = Some(window_id(h));
                        (self.sink)(PointerEvent::FocusRefused(window_id(h)));
                    }
                }
            }
        }
    }

    fn run(mut self, rx: Receiver<()>) {
        let mut moved = false;
        loop {
            if !moved && rx.recv().is_err() {
                return;
            }
            moved = false;
            CLICKED.store(false, Ordering::Relaxed);
            let waits = rest_waits(self.config.focus_delay(), self.config.hover());
            let start = Instant::now();
            for (at, action) in waits {
                let remaining =
                    Duration::from_millis(u64::from(at)).saturating_sub(start.elapsed());
                match rx.recv_timeout(remaining) {
                    // Still moving: start over from the new resting place.
                    Ok(()) => {
                        moved = true;
                        break;
                    }
                    Err(RecvTimeoutError::Timeout) => self.act(action),
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
        }
    }
}

/// Start the watcher. Only one runs at a time; dropping the handle stops it.
pub(crate) fn start(
    config: Arc<PointerConfig>,
    sink: EventSink<PointerEvent>,
) -> Result<WatchHandle, PlatformError> {
    let (tx, rx) = sync_channel::<()>(1);
    {
        let mut slot = POKE
            .lock()
            .map_err(|_| PlatformError::Os("pointer watcher lock poisoned".into()))?;
        if slot.is_some() {
            return Err(PlatformError::Os(
                "the pointer watcher is already running".into(),
            ));
        }
        *slot = Some(tx);
    }
    let worker = Worker {
        config,
        sink,
        picker_open: false,
        refused: None,
    };
    let thread = thread::Builder::new()
        .name("krova-pointer".into())
        .spawn(move || worker.run(rx))
        .map_err(|e| PlatformError::Os(e.to_string()))?;
    ACTIVE.store(true, Ordering::Relaxed);
    Ok(WatchHandle::new(move || {
        ACTIVE.store(false, Ordering::Relaxed);
        // Dropping the only sender wakes the worker with `Disconnected`.
        if let Ok(mut slot) = POKE.lock() {
            *slot = None;
        }
        let _ = thread.join();
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn rejected_windows_are_never_hit_tested() {
        let mut asked = false;
        assert!(!caption_hit(false, || {
            asked = true;
            true
        }));
        assert!(!asked, "WM_NCHITTEST must not reach a rejected window");
        assert!(caption_hit(true, || true));
        assert!(!caption_hit(true, || false));
    }

    #[test]
    fn lparam_packs_negative_screen_coordinates() {
        assert_eq!(point_lparam(10, 20).0, (20 << 16) | 10);
        let p = point_lparam(-1, -2).0 as u32;
        assert_eq!(p & 0xFFFF, 0xFFFF);
        assert_eq!(p >> 16, 0xFFFE);
    }

    /// Starts and stops the real watcher thread (no window is focused: both
    /// features are off, so a rest asks nothing of the desktop). One test, because
    /// the hook state is process-wide.
    #[test]
    fn the_watcher_starts_once_stops_and_restarts() {
        // No watcher yet: the hook stores nothing.
        on_hook(WM_MOUSEMOVE, 123_456, 7);
        assert_ne!(X.load(Ordering::Relaxed), 123_456);
        let (tx, rx) = mpsc::channel();
        let sink: EventSink<PointerEvent> = Arc::new(move |e| {
            let _ = tx.send(e);
        });
        let config = Arc::new(PointerConfig::default());
        let handle = start(Arc::clone(&config), Arc::clone(&sink)).unwrap();
        assert!(start(Arc::clone(&config), Arc::clone(&sink)).is_err());
        on_hook(WM_MOUSEMOVE, 5, 6);
        assert_eq!(
            (X.load(Ordering::Relaxed), Y.load(Ordering::Relaxed)),
            (5, 6)
        );
        drop(handle);
        assert!(rx.try_recv().is_err(), "nothing is enabled: no events");
        let again = start(config, sink).unwrap();
        drop(again);
    }

    /// Real desktop: put the cursor on the title bar of an ordinary window that
    /// does not have the focus (for example Notepad), hold Shift, then run
    /// `cargo test -p krova-platform -- --ignored --nocapture rest_at_the_cursor`.
    /// It prints what the hover picker and focus follows mouse would do there. It
    /// never prints a window title.
    #[test]
    #[ignore = "needs a real desktop and a human to place the cursor"]
    fn rest_at_the_cursor_reports_what_would_happen() {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
        let mut p = POINT::default();
        unsafe { GetCursorPos(&mut p) }.unwrap();
        let config = PointerConfig::default();
        config.set(true, 350, true, krova_core::Modifier::Shift);
        println!("hover: {:?}", hover_at(&config, p.x, p.y));
        let outcome = match focus_at(p.x, p.y, false, None) {
            FocusOutcome::Skipped => "skipped".to_owned(),
            FocusOutcome::Focused(h) => format!("focused {:?}", window_id(h)),
            FocusOutcome::Refused(h) => format!("refused by Windows {:?}", window_id(h)),
        };
        println!("focus: {outcome}");
    }
}
