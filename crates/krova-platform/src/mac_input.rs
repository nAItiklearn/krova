//! Drag input for macOS: a listen-only `CGEventTap` plus a worker thread.
//!
//! STATUS: written without a Mac or a compiler (see `mac_ffi`).
//!
//! Same shape as `win.rs`:
//! - the tap thread owns a `CFRunLoop` with the tap's run-loop source and only
//!   forwards left-button events (`RawPointer`) into a channel, so the callback
//!   stays tiny (the system disables a tap whose callback is slow);
//! - a worker thread feeds those events to `mac_math::DragTracker`, which finds
//!   the window under the cursor (CGWindowList, on-screen, layer 0), samples its
//!   frame and polls the modifier (`CGEventSourceFlagsState`) and Escape
//!   (`CGEventSourceKeyState`) while a drag is live.
//!
//! Permissions: the tap needs Accessibility (checked up front, because moving
//! windows needs it anyway) and, on recent macOS, may also need Input Monitoring;
//! `CGEventTapCreate` returns NULL when it is refused, which is reported as an
//! error mentioning both. Window titles are never read.

#![allow(dead_code, unused_unsafe, unused_imports)]

use std::ffi::c_void;
use std::ptr::null;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use krova_core::{Modifier, PxRect};

use super::mac_ffi::*;
use crate::input::{coalesce_moves, RawPointer};
use crate::mac_math::{cursor_px, modifier_held, DragProbe, DragTracker, KEY_ESCAPE};
use crate::{modifier_code, modifier_from_code, InputEvent, InputHandle, PlatformError};

/// Only one monitor may run at a time (as on Windows).
static RUNNING: AtomicBool = AtomicBool::new(false);

/// `CGEventMaskBit(type)` is `1 << type`.
const TAP_MASK: u64 = (1u64 << EVENT_LEFT_MOUSE_DOWN)
    | (1u64 << EVENT_LEFT_MOUSE_UP)
    | (1u64 << EVENT_LEFT_MOUSE_DRAGGED);

const NO_TAP_MESSAGE: &str = "could not create the mouse event tap; grant Krova access in \
System Settings > Privacy & Security > Accessibility and Input Monitoring, then restart it";

// ----------------------------------------------------------- tap thread

/// Lives on the tap thread's stack for as long as the tap exists; the callback
/// reaches it through the tap's `userInfo` pointer.
struct TapContext {
    tx: Sender<RawPointer>,
    /// The tap's CFMachPort (as an integer), so the callback can re-enable it.
    port: AtomicUsize,
    /// Set when the system disabled the tap and events may have been lost.
    lost: Arc<AtomicBool>,
}

unsafe extern "C" fn tap_callback(
    _proxy: *mut c_void,
    kind: u32,
    event: *const c_void,
    info: *mut c_void,
) -> *const c_void {
    if info.is_null() {
        return event;
    }
    let ctx = &*(info as *const TapContext);
    match kind {
        EVENT_LEFT_MOUSE_DOWN | EVENT_LEFT_MOUSE_UP | EVENT_LEFT_MOUSE_DRAGGED => {
            let at = CGEventGetLocation(event);
            let (x, y) = cursor_px(at.x, at.y);
            let ev = match kind {
                EVENT_LEFT_MOUSE_DOWN => RawPointer::Down(x, y),
                EVENT_LEFT_MOUSE_DRAGGED => RawPointer::Move(x, y),
                _ => RawPointer::Up,
            };
            let _ = ctx.tx.send(ev);
        }
        EVENT_TAP_DISABLED_BY_TIMEOUT | EVENT_TAP_DISABLED_BY_USER_INPUT => {
            ctx.lost.store(true, Ordering::SeqCst);
            let port = ctx.port.load(Ordering::SeqCst);
            if port != 0 {
                CGEventTapEnable(port as *const c_void, true);
            }
        }
        _ => {}
    }
    event
}

/// Create the tap and run its run loop until `stop` is set. Reports the run
/// loop (retained, as an integer) or an error through `ready`.
fn tap_thread(
    tx: Sender<RawPointer>,
    lost: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    ready: Sender<Result<usize, String>>,
) {
    let ctx = Box::new(TapContext {
        tx,
        port: AtomicUsize::new(0),
        lost,
    });
    let info = (&*ctx as *const TapContext).cast_mut().cast::<c_void>();

    unsafe {
        let port = CGEventTapCreate(
            TAP_SESSION,
            TAP_HEAD_INSERT,
            TAP_LISTEN_ONLY,
            TAP_MASK,
            tap_callback,
            info,
        );
        if port.is_null() {
            let _ = ready.send(Err(NO_TAP_MESSAGE.to_string()));
            return;
        }
        ctx.port.store(port as usize, Ordering::SeqCst);

        let source = CFMachPortCreateRunLoopSource(null(), port, 0);
        if source.is_null() {
            CFRelease(port);
            let _ = ready.send(Err("could not create the event tap run loop source".into()));
            return;
        }
        let run_loop = CFRunLoopGetCurrent();
        CFRetain(run_loop);
        CFRunLoopAddSource(run_loop, source, kCFRunLoopCommonModes);
        CGEventTapEnable(port, true);
        let _ = ready.send(Ok(run_loop as usize));

        // A timed run loop instead of CFRunLoopRun(): a CFRunLoopStop() that
        // arrives before the loop started would otherwise be lost for good.
        while !stop.load(Ordering::SeqCst) {
            let result = CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.25, 0);
            if result == 1 {
                // kCFRunLoopRunFinished: nothing left to watch; do not spin.
                thread::sleep(Duration::from_millis(50));
            }
        }

        CGEventTapEnable(port, false);
        CFRunLoopRemoveSource(run_loop, source, kCFRunLoopCommonModes);
        CFMachPortInvalidate(port);
        CFRelease(source);
        CFRelease(port);
    }
    // `ctx` is dropped here, which drops the sender and lets the worker exit.
}

// -------------------------------------------------------------- worker

/// The OS side of `DragTracker`.
struct MacProbe {
    modifier: Arc<AtomicU8>,
}

impl DragProbe for MacProbe {
    fn window_at(&mut self, x: i32, y: i32) -> Option<(u32, PxRect)> {
        super::mac_ax::window_at(x, y)
    }

    fn window_frame(&mut self, id: u32) -> Option<PxRect> {
        super::mac_ax::window_frame(id)
    }

    fn modifier_down(&mut self) -> bool {
        let m = modifier_from_code(self.modifier.load(Ordering::Relaxed));
        let flags = unsafe { CGEventSourceFlagsState(EVENT_SOURCE_COMBINED_SESSION) };
        modifier_held(flags, m)
    }

    fn escape_down(&mut self) -> bool {
        unsafe { CGEventSourceKeyState(EVENT_SOURCE_COMBINED_SESSION, KEY_ESCAPE) }
    }

    fn settle(&mut self) {
        // Let the window server finish its own move before the app repositions
        // the window (30 ms, as on Windows).
        thread::sleep(Duration::from_millis(30));
    }
}

fn flush(out: &mut Vec<InputEvent>, sink: &Sender<InputEvent>) {
    for ev in out.drain(..) {
        let _ = sink.send(ev);
    }
}

fn run_worker(
    rx: Receiver<RawPointer>,
    sink: Sender<InputEvent>,
    modifier: Arc<AtomicU8>,
    stop: Arc<AtomicBool>,
    lost: Arc<AtomicBool>,
) {
    let mut probe = MacProbe { modifier };
    let mut tracker = DragTracker::default();
    let mut out: Vec<InputEvent> = Vec::new();

    while !stop.load(Ordering::Relaxed) {
        let mut batch = Vec::new();
        match rx.recv_timeout(Duration::from_millis(15)) {
            Ok(ev) => batch.push(ev),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        while let Ok(ev) = rx.try_recv() {
            batch.push(ev);
        }
        coalesce_moves(&mut batch);

        if lost.swap(false, Ordering::SeqCst) {
            tracker.abort(&mut out);
            flush(&mut out, &sink);
        }
        for ev in batch {
            tracker.on_pointer(&mut probe, ev, &mut out);
            flush(&mut out, &sink);
        }
        tracker.poll_keys(&mut probe, &mut out);
        flush(&mut out, &sink);
    }
}

// ---------------------------------------------------------------- start

pub fn start(modifier: Modifier, sink: Sender<InputEvent>) -> Result<InputHandle, PlatformError> {
    if !super::accessibility_granted() {
        return Err(PlatformError::Os(
            "Accessibility permission is not granted (System Settings > Privacy & Security > \
             Accessibility)"
                .into(),
        ));
    }
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Err(PlatformError::Os("input monitor is already running".into()));
    }
    start_inner(modifier, sink).map_err(|e| {
        RUNNING.store(false, Ordering::SeqCst);
        e
    })
}

fn io_err(e: std::io::Error) -> PlatformError {
    PlatformError::Os(e.to_string())
}

fn start_inner(modifier: Modifier, sink: Sender<InputEvent>) -> Result<InputHandle, PlatformError> {
    let (tx, rx) = mpsc::channel::<RawPointer>();
    let (ready_tx, ready_rx) = mpsc::channel::<Result<usize, String>>();
    let modifier_cell = Arc::new(AtomicU8::new(modifier_code(modifier)));
    let stop = Arc::new(AtomicBool::new(false));
    let lost = Arc::new(AtomicBool::new(false));

    let tap_join = {
        let lost = Arc::clone(&lost);
        let stop = Arc::clone(&stop);
        thread::Builder::new()
            .name("krova-tap".into())
            .spawn(move || tap_thread(tx, lost, stop, ready_tx))
            .map_err(io_err)?
    };

    let run_loop = match ready_rx.recv_timeout(Duration::from_secs(2)) {
        Ok(Ok(run_loop)) => run_loop,
        Ok(Err(msg)) => {
            let _ = tap_join.join();
            return Err(PlatformError::Os(msg));
        }
        Err(_) => {
            // The thread may still come up; make it exit as soon as it does.
            stop.store(true, Ordering::SeqCst);
            return Err(PlatformError::Os(
                "the event tap thread did not start".into(),
            ));
        }
    };

    let worker = {
        let modifier_cell = Arc::clone(&modifier_cell);
        let stop = Arc::clone(&stop);
        let lost = Arc::clone(&lost);
        thread::Builder::new()
            .name("krova-input".into())
            .spawn(move || run_worker(rx, sink, modifier_cell, stop, lost))
    };
    let worker_join = match worker {
        Ok(j) => j,
        Err(e) => {
            stop.store(true, Ordering::SeqCst);
            unsafe { CFRunLoopStop(run_loop as *const c_void) };
            let _ = tap_join.join();
            unsafe { CFRelease(run_loop as *const c_void) };
            return Err(io_err(e));
        }
    };

    Ok(InputHandle::new(modifier_cell, move || {
        stop.store(true, Ordering::SeqCst);
        // Wake the run loop now; the 0.25 s timeout would catch it anyway.
        unsafe { CFRunLoopStop(run_loop as *const c_void) };
        let _ = tap_join.join();
        let _ = worker_join.join();
        unsafe { CFRelease(run_loop as *const c_void) };
        RUNNING.store(false, Ordering::SeqCst);
    }))
}
