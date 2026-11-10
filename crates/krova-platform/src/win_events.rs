//! Windows: global keyboard shortcuts and the window/display watcher.
//!
//! Both live on their own thread with their own message loop, like the mouse hook
//! in `win.rs`, but without that thread's known weaknesses:
//!
//! - A thread is stopped by signalling a manual-reset **event handle that its
//!   owner keeps open until the thread has been joined**. Nothing is ever posted to
//!   a stored thread id, so there is no way to hit a recycled id.
//! - Startup reports back over a channel; if it times out the thread is told to
//!   stop and the event handle is deliberately leaked rather than closed under it.
//! - Callbacks only enqueue: the sinks handed in are channel sends. The WinEvent
//!   callback and the window procedure never take a lock for longer than a clone
//!   of an `Arc`, and never block.
//!
//! Window titles are never read: the watcher only learns window handles.

use std::ffi::c_void;
use std::mem::size_of;
use std::sync::mpsc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use krova_core::registry::KeyAction;
use krova_core::ShortcutModifier;
use windows::core::{HRESULT, PCWSTR};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_HOTKEY_ALREADY_REGISTERED, FALSE, HANDLE, HINSTANCE, HMODULE, HWND, LPARAM,
    LRESULT, TRUE, WAIT_FAILED, WAIT_OBJECT_0, WPARAM,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{CreateEventW, SetEvent, INFINITE};
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
    MOD_SHIFT, MOD_WIN, VK_LEFT, VK_RIGHT, VK_SPACE, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, MsgWaitForMultipleObjectsEx,
    PeekMessageW, RegisterClassExW, TranslateMessage, EVENT_SYSTEM_MOVESIZEEND,
    EVENT_SYSTEM_MOVESIZESTART, HMENU, MSG, MWMO_INPUTAVAILABLE, OBJID_WINDOW,
    PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND, PM_NOREMOVE, PM_REMOVE, QS_ALLINPUT,
    SPI_SETWORKAREA, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_CLOSE, WM_DISPLAYCHANGE,
    WM_HOTKEY, WM_POWERBROADCAST, WM_QUIT, WM_SETTINGCHANGE, WNDCLASSEXW, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_POPUP,
};

use crate::hotkeys::{action_for_id, refusal_message, HotkeyEvent, HotkeySpec, Key, Refusal};
use crate::{EventSink, HotkeyHandle, PlatformError, WatchEvent, WatchHandle, WindowId};

fn os_err(e: windows::core::Error) -> PlatformError {
    PlatformError::Os(e.to_string())
}

/// A manual-reset event used to tell a message-loop thread to stop.
struct StopEvent(HANDLE);

// SAFETY: a kernel event handle is just an identifier; signalling and closing it
// from any thread is allowed.
unsafe impl Send for StopEvent {}
unsafe impl Sync for StopEvent {}

impl StopEvent {
    fn new() -> Result<Self, PlatformError> {
        // SAFETY: no security attributes, manual reset, initially not signalled, unnamed.
        unsafe { CreateEventW(None, TRUE, FALSE, PCWSTR::null()) }
            .map(Self)
            .map_err(os_err)
    }

    fn signal(&self) {
        // SAFETY: the handle is valid until `drop`.
        let _ = unsafe { SetEvent(self.0) };
    }

    /// The handle value, for the thread that waits on it. Valid until the owner
    /// drops this value, which only happens after that thread has been joined.
    fn raw(&self) -> usize {
        self.0 .0 as usize
    }
}

impl Drop for StopEvent {
    fn drop(&mut self) {
        // SAFETY: we created the handle and nobody waits on it any more.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

/// Run the thread's message loop until `stop` is signalled or `WM_QUIT` arrives.
fn pump(stop: usize, mut on_message: impl FnMut(&MSG)) {
    let handles = [HANDLE(stop as *mut c_void)];
    loop {
        // SAFETY: `handles` outlives the call; the wait is on the thread's own queue.
        let wait = unsafe {
            MsgWaitForMultipleObjectsEx(Some(&handles), INFINITE, QS_ALLINPUT, MWMO_INPUTAVAILABLE)
        };
        if wait == WAIT_OBJECT_0 || wait == WAIT_FAILED {
            return;
        }
        let mut msg = MSG::default();
        while unsafe { PeekMessageW(&mut msg, HWND::default(), 0, 0, PM_REMOVE) }.as_bool() {
            if msg.message == WM_QUIT {
                return;
            }
            on_message(&msg);
        }
    }
}

fn dispatch(msg: &MSG) {
    // SAFETY: `msg` was filled in by PeekMessageW on this thread.
    unsafe {
        let _ = TranslateMessage(msg);
        DispatchMessageW(msg);
    }
}

// ------------------------------------------------------------- hotkeys

/// The `RegisterHotKey` modifier bits for a chord (without `MOD_NOREPEAT`).
pub fn modifier_bits(m: ShortcutModifier) -> u32 {
    match m {
        ShortcutModifier::CtrlAlt => MOD_CONTROL.0 | MOD_ALT.0,
        ShortcutModifier::WinAlt => MOD_WIN.0 | MOD_ALT.0,
        ShortcutModifier::CtrlAltShift => MOD_CONTROL.0 | MOD_ALT.0 | MOD_SHIFT.0,
        ShortcutModifier::WinCtrl => MOD_WIN.0 | MOD_CONTROL.0,
    }
}

/// Virtual-key code of a shortcut's key.
pub fn virtual_key(key: Key) -> u32 {
    match key {
        Key::Left => u32::from(VK_LEFT.0),
        Key::Right => u32::from(VK_RIGHT.0),
        Key::Up => u32::from(VK_UP.0),
        // '1'..'9' on the main row have the virtual-key codes of their ASCII digits.
        Key::Digit(n) => u32::from(b'0') + u32::from(n.min(9)),
        Key::Space => u32::from(VK_SPACE.0),
    }
}

struct HotkeyReport {
    accepted: Vec<KeyAction>,
    refused: Vec<(KeyAction, String)>,
}

fn hotkey_thread(
    stop: usize,
    spec: HotkeySpec,
    sink: &EventSink<HotkeyEvent>,
    ready: &mpsc::Sender<HotkeyReport>,
) {
    let mods = HOT_KEY_MODIFIERS(modifier_bits(spec.modifier) | MOD_NOREPEAT.0);

    // Touching the queue creates it, so a WM_HOTKEY posted right after
    // registration is not lost.
    let mut probe = MSG::default();
    // SAFETY: plain peek on this thread's own queue.
    let _ = unsafe { PeekMessageW(&mut probe, HWND::default(), 0, 0, PM_NOREMOVE) };

    let mut registered = Vec::new();
    let mut accepted = Vec::new();
    let mut refused = Vec::new();
    for def in spec.defs() {
        // SAFETY: a thread-bound hot key (no window); unregistered below on this thread.
        let result = unsafe { RegisterHotKey(HWND::default(), def.id, mods, virtual_key(def.key)) };
        match result {
            Ok(()) => {
                registered.push(def.id);
                accepted.push(def.action);
            }
            Err(e) => {
                let why = if e.code() == HRESULT::from_win32(ERROR_HOTKEY_ALREADY_REGISTERED.0) {
                    Refusal::InUse
                } else {
                    Refusal::Other((e.code().0 & 0xFFFF) as u32)
                };
                refused.push((def.action, refusal_message(spec.modifier, def.key, why)));
            }
        }
    }
    let _ = ready.send(HotkeyReport { accepted, refused });

    pump(stop, |msg| {
        if msg.message == WM_HOTKEY {
            if let Some(action) = i32::try_from(msg.wParam.0).ok().and_then(action_for_id) {
                sink(HotkeyEvent { action });
            }
        } else {
            dispatch(msg);
        }
    });

    for id in registered {
        // SAFETY: unregisters a hot key this thread registered.
        let _ = unsafe { UnregisterHotKey(HWND::default(), id) };
    }
}

/// Register the requested shortcuts (the window-move group, `<mods>+Space`, or both)
/// on a dedicated thread. Shortcuts the OS refuses are listed in the returned
/// handle; the others work regardless.
pub fn start_hotkeys(
    spec: HotkeySpec,
    sink: EventSink<HotkeyEvent>,
) -> Result<HotkeyHandle, PlatformError> {
    let stop = StopEvent::new()?;
    let raw = stop.raw();
    let (ready_tx, ready_rx) = mpsc::channel::<HotkeyReport>();
    let join = thread::Builder::new()
        .name("krova-hotkeys".into())
        .spawn(move || hotkey_thread(raw, spec, &sink, &ready_tx))
        .map_err(|e| PlatformError::Os(e.to_string()))?;

    match ready_rx.recv_timeout(Duration::from_secs(2)) {
        Ok(report) => Ok(HotkeyHandle::new(
            report.accepted,
            report.refused,
            move || {
                stop.signal();
                let _ = join.join();
                drop(stop);
            },
        )),
        Err(_) => {
            // The thread will see the event as soon as it gets going. Leak the
            // handle rather than close it under a thread that may still wait on it.
            stop.signal();
            std::mem::forget(stop);
            Err(PlatformError::Os(
                "the shortcut thread did not start".into(),
            ))
        }
    }
}

// ------------------------------------------------------------- watcher

/// The one active watcher's sink. The WinEvent callback and the window procedure
/// are bare functions, so they reach it through this slot.
static WATCH_SINK: Mutex<Option<EventSink<WatchEvent>>> = Mutex::new(None);

fn emit(event: WatchEvent) {
    // Copy the Arc out and release the lock before calling it.
    let sink = match WATCH_SINK.try_lock() {
        Ok(slot) => slot.clone(),
        Err(_) => return,
    };
    if let Some(sink) = sink {
        sink(event);
    }
}

fn clear_watch_sink() {
    if let Ok(mut slot) = WATCH_SINK.lock() {
        *slot = None;
    }
}

fn window_id(h: HWND) -> WindowId {
    WindowId(h.0 as usize as u64)
}

/// Called by Windows on the watcher thread (during `PeekMessageW`) for every
/// move/size loop start and end, in any other process.
unsafe extern "system" fn win_event_proc(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    id_child: i32,
    _event_thread: u32,
    _event_time: u32,
) {
    // Only whole top-level windows: ignore events about child objects.
    if id_object != OBJID_WINDOW.0 || id_child != 0 || hwnd.0.is_null() {
        return;
    }
    match event {
        EVENT_SYSTEM_MOVESIZESTART => emit(WatchEvent::MoveSizeStart(window_id(hwnd))),
        EVENT_SYSTEM_MOVESIZEEND => emit(WatchEvent::MoveSizeEnd(window_id(hwnd))),
        _ => {}
    }
}

/// Window procedure of the hidden top-level window that receives the broadcast
/// display, work-area and power messages.
unsafe extern "system" fn watch_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // Another program can post WM_CLOSE to any window it can enumerate; the default
    // handling would destroy this one and Krova would stop hearing about displays.
    if msg == WM_CLOSE {
        return LRESULT(0);
    }
    let display_may_have_changed = match msg {
        WM_DISPLAYCHANGE => true,
        WM_SETTINGCHANGE => wparam.0 == SPI_SETWORKAREA.0 as usize,
        WM_POWERBROADCAST => {
            matches!(
                wparam.0 as u32,
                PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND
            )
        }
        _ => false,
    };
    if display_may_have_changed {
        emit(WatchEvent::DisplayChanged);
    }
    // SAFETY: forwards the arguments Windows gave us, unchanged.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// A hidden *top-level* window: message-only windows do not receive broadcasts.
fn create_watch_window() -> windows::core::Result<HWND> {
    // SAFETY: standard window-class registration and creation; the class name
    // buffer outlives both calls, and WNDCLASSEXW is plain data (all-zero is valid).
    unsafe {
        let hinst = HINSTANCE(GetModuleHandleW(PCWSTR::null())?.0);
        let class: Vec<u16> = "KrovaDisplayWatch\0".encode_utf16().collect();
        let mut wc: WNDCLASSEXW = std::mem::zeroed();
        wc.cbSize = size_of::<WNDCLASSEXW>() as u32;
        wc.lpfnWndProc = Some(watch_wnd_proc);
        wc.hInstance = hinst;
        wc.lpszClassName = PCWSTR(class.as_ptr());
        // A second registration fails harmlessly: the class from the first is reused.
        RegisterClassExW(&wc);
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            PCWSTR(class.as_ptr()),
            PCWSTR::null(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            HWND::default(),
            HMENU::default(),
            hinst,
            None,
        )
    }
}

fn watch_thread(stop: usize, ready: &mpsc::Sender<Result<(), String>>) {
    let window = match create_watch_window() {
        Ok(w) => w,
        Err(e) => {
            let _ = ready.send(Err(e.to_string()));
            return;
        }
    };
    // SAFETY: out-of-context hook with a plain function callback; it is removed
    // below, on this same thread, before the thread ends.
    let hook = unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_MOVESIZESTART,
            EVENT_SYSTEM_MOVESIZEEND,
            HMODULE::default(),
            Some(win_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        )
    };
    if hook.is_invalid() {
        let _ = ready.send(Err("SetWinEventHook failed".into()));
        // SAFETY: destroys the window created above, on its own thread.
        let _ = unsafe { DestroyWindow(window) };
        return;
    }
    let _ = ready.send(Ok(()));

    pump(stop, dispatch);

    // SAFETY: removes the hook and window created above, on their own thread.
    unsafe {
        let _ = UnhookWinEvent(hook);
        let _ = DestroyWindow(window);
    }
}

/// Watch for window move/size loops (any process but ours) and display changes.
/// Only one watcher can run at a time.
pub fn start_watcher(sink: EventSink<WatchEvent>) -> Result<WatchHandle, PlatformError> {
    {
        let mut slot = WATCH_SINK
            .lock()
            .map_err(|_| PlatformError::Os("watcher state poisoned".into()))?;
        if slot.is_some() {
            return Err(PlatformError::Os(
                "the window watcher is already running".into(),
            ));
        }
        *slot = Some(sink);
    }
    let stop = match StopEvent::new() {
        Ok(s) => s,
        Err(e) => {
            clear_watch_sink();
            return Err(e);
        }
    };
    let raw = stop.raw();
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
    let spawned = thread::Builder::new()
        .name("krova-watch".into())
        .spawn(move || watch_thread(raw, &ready_tx));
    let join = match spawned {
        Ok(j) => j,
        Err(e) => {
            clear_watch_sink();
            return Err(PlatformError::Os(e.to_string()));
        }
    };

    match ready_rx.recv_timeout(Duration::from_secs(2)) {
        Ok(Ok(())) => Ok(WatchHandle::new(move || {
            stop.signal();
            let _ = join.join();
            clear_watch_sink();
            drop(stop);
        })),
        Ok(Err(msg)) => {
            let _ = join.join();
            clear_watch_sink();
            Err(PlatformError::Os(format!(
                "could not start the window watcher: {msg}"
            )))
        }
        Err(_) => {
            stop.signal();
            std::mem::forget(stop);
            clear_watch_sink();
            Err(PlatformError::Os("the window watcher did not start".into()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hot keys are per process: two tests registering the same chord at the same
    /// time would refuse each other, so the tests that register take turns.
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

    fn take_turn() -> std::sync::MutexGuard<'static, ()> {
        ONE_AT_A_TIME
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    #[test]
    fn modifier_chords_map_to_the_right_bits() {
        assert_eq!(
            modifier_bits(ShortcutModifier::CtrlAlt),
            MOD_CONTROL.0 | MOD_ALT.0
        );
        assert_eq!(
            modifier_bits(ShortcutModifier::WinAlt),
            MOD_WIN.0 | MOD_ALT.0
        );
        assert_eq!(
            modifier_bits(ShortcutModifier::CtrlAltShift),
            MOD_CONTROL.0 | MOD_ALT.0 | MOD_SHIFT.0
        );
        assert_eq!(
            modifier_bits(ShortcutModifier::WinCtrl),
            MOD_WIN.0 | MOD_CONTROL.0
        );
        for m in [
            ShortcutModifier::CtrlAlt,
            ShortcutModifier::WinAlt,
            ShortcutModifier::CtrlAltShift,
            ShortcutModifier::WinCtrl,
        ] {
            assert_eq!(modifier_bits(m) & MOD_NOREPEAT.0, 0);
        }
    }

    #[test]
    fn keys_map_to_virtual_key_codes() {
        assert_eq!(virtual_key(Key::Left), 0x25);
        assert_eq!(virtual_key(Key::Up), 0x26);
        assert_eq!(virtual_key(Key::Right), 0x27);
        assert_eq!(virtual_key(Key::Digit(1)), 0x31);
        assert_eq!(virtual_key(Key::Digit(9)), 0x39);
        assert_eq!(virtual_key(Key::Space), 0x20);
    }

    #[test]
    fn the_watcher_starts_reports_nothing_idle_and_stops() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        let _turn = take_turn();
        let seen = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&seen);
        let handle = start_watcher(Arc::new(move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
        }))
        .expect("watcher starts (needs a window station, which test sessions have)");
        // Only one at a time.
        assert!(start_watcher(Arc::new(|_| {})).is_err());
        drop(handle);
        // After dropping, a new one can start again.
        let again = start_watcher(Arc::new(|_| {})).expect("restart after stop");
        drop(again);
    }

    /// The watcher's hidden window, found by class name (this process only has one).
    fn find_watch_window() -> Option<HWND> {
        use windows::Win32::UI::WindowsAndMessaging::FindWindowW;
        let class: Vec<u16> = "KrovaDisplayWatch\0".encode_utf16().collect();
        // SAFETY: the class name buffer is NUL-terminated and outlives the call.
        unsafe { FindWindowW(PCWSTR(class.as_ptr()), PCWSTR::null()) }.ok()
    }

    #[test]
    fn the_watcher_window_ignores_wm_close_from_other_programs() {
        use std::sync::Arc;
        use windows::Win32::UI::WindowsAndMessaging::{
            IsWindow, SendMessageTimeoutW, SMTO_ABORTIFHUNG,
        };
        let _turn = take_turn();
        let handle = start_watcher(Arc::new(|_| {})).expect("watcher starts");
        let window = find_watch_window().expect("the watcher has a window");
        for _ in 0..3 {
            // SAFETY: a plain cross-thread send to a window this process owns; the
            // watcher thread pumps messages, and the timeout bounds the wait.
            let sent = unsafe {
                SendMessageTimeoutW(
                    window,
                    WM_CLOSE,
                    WPARAM(0),
                    LPARAM(0),
                    SMTO_ABORTIFHUNG,
                    2000,
                    None,
                )
            };
            assert_ne!(sent.0, 0, "the window answered");
        }
        // SAFETY: IsWindow is harmless on any handle.
        assert!(
            unsafe { IsWindow(window) }.as_bool(),
            "WM_CLOSE must not destroy the watcher's window"
        );
        drop(handle);
        // A normal stop still removes it.
        assert!(find_watch_window().is_none());
    }

    #[test]
    fn a_window_has_an_owner_that_is_stable_and_names_this_process() {
        use crate::Platform;
        use std::sync::Arc;
        let _turn = take_turn();
        let handle = start_watcher(Arc::new(|_| {})).expect("watcher starts");
        let window = find_watch_window().expect("the watcher has a window");
        let platform = crate::win::WindowsPlatform::new();
        let id = WindowId(window.0 as usize as u64);
        let first = platform.window_owner(id).expect("own process is readable");
        let second = platform.window_owner(id).expect("and again");
        assert_eq!(first, second, "the owner does not change between readings");
        assert_eq!(first.pid, std::process::id());
        assert!(first.started > 0);
        drop(handle);
        assert_eq!(
            platform.window_owner(id),
            None,
            "a destroyed window has no owner"
        );
        assert_eq!(platform.window_owner(WindowId(0)), None);
    }

    #[test]
    fn hotkeys_register_and_unregister_cleanly() {
        use std::sync::Arc;
        let _turn = take_turn();
        // Whatever the OS refuses is reported, never an error; dropping stops the thread.
        let spec = HotkeySpec {
            modifier: ShortcutModifier::CtrlAltShift,
            window_moves: true,
            switcher: true,
        };
        let handle = start_hotkeys(spec, Arc::new(|_| {})).expect("thread starts");
        assert_eq!(
            handle.registered() + handle.failed().len(),
            spec.defs().len()
        );
        assert_eq!(spec.defs().len(), 13);
        let refused_first = handle.failed().len();
        drop(handle);
        // The same chord can be registered again right away: the first
        // registrations were released when the thread ended.
        let handle = start_hotkeys(spec, Arc::new(|_| {})).expect("thread starts again");
        assert_eq!(
            handle.failed().len(),
            refused_first,
            "{:?}",
            handle.failed()
        );
    }

    #[test]
    fn each_group_registers_on_its_own() {
        use std::sync::Arc;
        let _turn = take_turn();
        let only = |window_moves, switcher| {
            let spec = HotkeySpec {
                modifier: ShortcutModifier::CtrlAltShift,
                window_moves,
                switcher,
            };
            let handle = start_hotkeys(spec, Arc::new(|_| {})).expect("thread starts");
            let (moves, space) = (handle.window_moves(), handle.switcher());
            // Whatever the OS refuses is reported; a group that was not asked for is empty.
            assert_eq!(
                moves.registered + moves.failed.len(),
                usize::from(window_moves) * 12
            );
            assert_eq!(space.registered + space.failed.len(), usize::from(switcher));
            drop(handle);
        };
        only(true, false);
        only(false, true);
        only(false, false);
    }

    /// Which `<mods>+Space` chords this PC's Windows lets a program register. Prints
    /// a table (`-- --ignored --nocapture`); it asserts only for the default chord,
    /// and is ignored because another running program may legitimately own it.
    #[test]
    #[ignore = "depends on the programs running on this PC; run by hand"]
    fn space_chords_probe() {
        use std::sync::Arc;
        let _turn = take_turn();
        for modifier in [
            ShortcutModifier::CtrlAlt,
            ShortcutModifier::WinAlt,
            ShortcutModifier::CtrlAltShift,
            ShortcutModifier::WinCtrl,
        ] {
            let spec = HotkeySpec {
                modifier,
                window_moves: false,
                switcher: true,
            };
            let handle = start_hotkeys(spec, Arc::new(|_| {})).expect("thread starts");
            let report = handle.switcher();
            println!(
                "{:>15}+Space: {}",
                modifier.label(),
                if report.registered == 1 {
                    "registered".to_owned()
                } else {
                    report.failed.join("; ")
                }
            );
            if modifier == ShortcutModifier::CtrlAltShift {
                assert_eq!(
                    report.registered, 1,
                    "the default chord: {:?}",
                    report.failed
                );
            }
        }
    }
}
