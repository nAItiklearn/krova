//! Integration tests that drive real windows on the interactive desktop.
//!
//! They spawn a real `notepad.exe`, move it with `place_window`, press keys with
//! `SendInput` and drag its title bar with the real mouse, so they only make sense
//! in a logged-in session and they briefly take over the keyboard and the mouse.
//! They are therefore all `#[ignore]`; run them with
//!
//! ```text
//! cargo test -p krova-platform --test windows_desktop -- --ignored --nocapture --test-threads=1
//! ```
//!
//! Window titles are never read: Notepad is found by its window *class* and the
//! window handles that appear after it was started.

#![cfg(windows)]

use std::ffi::c_void;
use std::mem::size_of;
use std::process::{Child, Command};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::sleep;
use std::time::{Duration, Instant};

use krova_core::{PxRect, ShortcutModifier};
use krova_platform::hotkeys::{chord_label, HOTKEYS};
use krova_platform::win::WindowsPlatform;
use krova_platform::{HotkeySpec, Platform, WatchEvent, WindowId};
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, TRUE, WPARAM};
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYBD_EVENT_FLAGS,
    KEYEVENTF_KEYUP, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEINPUT, MOUSE_EVENT_FLAGS,
    VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetForegroundWindow, IsWindow, IsWindowVisible, PostMessageW,
    SendMessageW, SetCursorPos, SetForegroundWindow, WM_CLOSE, WM_NCHITTEST,
};

fn hwnd(id: WindowId) -> HWND {
    HWND(id.0 as usize as *mut c_void)
}

fn make_dpi_aware() {
    // Same awareness as the app, or window rects would be virtualised. Fails
    // harmlessly if the process already has an awareness.
    // SAFETY: plain process-wide setting.
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
}

unsafe extern "system" fn collect(h: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: lparam is the address of the Vec that `top_level_windows` keeps alive.
    let list = unsafe { &mut *(lparam.0 as *mut Vec<usize>) };
    list.push(h.0 as usize);
    TRUE
}

fn top_level_windows() -> Vec<usize> {
    let mut list: Vec<usize> = Vec::new();
    // SAFETY: the callback only pushes into `list`, which outlives the call.
    let _ = unsafe { EnumWindows(Some(collect), LPARAM(&mut list as *mut Vec<usize> as isize)) };
    list
}

fn class_name(h: HWND) -> String {
    let mut buf = [0u16; 128];
    // SAFETY: the buffer is valid for the length passed.
    let n = unsafe { GetClassNameW(h, &mut buf) };
    String::from_utf16_lossy(&buf[..usize::try_from(n).unwrap_or(0)])
}

fn notepad_windows() -> Vec<usize> {
    top_level_windows()
        .into_iter()
        .filter(|&w| {
            let h = HWND(w as *mut c_void);
            // SAFETY: querying a handle that may have died is harmless.
            class_name(h) == "Notepad" && unsafe { IsWindowVisible(h) }.as_bool()
        })
        .collect()
}

/// A real Notepad window that is closed again when dropped.
struct Notepad {
    id: WindowId,
    child: Child,
}

impl Notepad {
    fn spawn() -> Option<Notepad> {
        make_dpi_aware();
        let before = notepad_windows();
        let child = Command::new("notepad.exe").spawn().ok()?;
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(15) {
            if let Some(&new) = notepad_windows().iter().find(|w| !before.contains(w)) {
                sleep(Duration::from_millis(600)); // let it finish creating its UI
                return Some(Notepad {
                    id: WindowId(new as u64),
                    child,
                });
            }
            sleep(Duration::from_millis(100));
        }
        let mut child = child;
        let _ = child.kill();
        None
    }
}

impl Drop for Notepad {
    fn drop(&mut self) {
        let h = hwnd(self.id);
        // SAFETY: posting a close request to a window we created.
        let _ = unsafe { PostMessageW(h, WM_CLOSE, WPARAM(0), LPARAM(0)) };
        let started = Instant::now();
        // SAFETY: IsWindow on a possibly dead handle is harmless.
        while unsafe { IsWindow(h) }.as_bool() && started.elapsed() < Duration::from_secs(5) {
            sleep(Duration::from_millis(100));
        }
        let _ = self.child.kill();
    }
}

fn key_input(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if up {
                    KEYEVENTF_KEYUP
                } else {
                    KEYBD_EVENT_FLAGS(0)
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn mouse_input(flags: MOUSE_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn send(inputs: &[INPUT]) {
    // SAFETY: `inputs` is a valid slice of fully initialised INPUT structs.
    let sent = unsafe { SendInput(inputs, size_of::<INPUT>() as i32) };
    assert_eq!(sent as usize, inputs.len(), "SendInput was blocked");
}

/// Press `mods` (down in order), tap `key`, release `mods` in reverse.
fn press_chord(mods: &[VIRTUAL_KEY], key: VIRTUAL_KEY) {
    let mut seq: Vec<INPUT> = mods.iter().map(|&m| key_input(m, false)).collect();
    seq.push(key_input(key, false));
    seq.push(key_input(key, true));
    seq.extend(mods.iter().rev().map(|&m| key_input(m, true)));
    send(&seq);
}

fn modifier_keys(m: ShortcutModifier) -> Vec<VIRTUAL_KEY> {
    match m {
        ShortcutModifier::CtrlAlt => vec![VK_CONTROL, VK_MENU],
        ShortcutModifier::WinAlt => vec![VK_LWIN, VK_MENU],
        ShortcutModifier::CtrlAltShift => vec![VK_CONTROL, VK_MENU, VK_SHIFT],
        ShortcutModifier::WinCtrl => vec![VK_LWIN, VK_CONTROL],
    }
}

/// Bring `id` to the foreground. Returns whether it really got the focus.
fn focus(id: WindowId) -> bool {
    let h = hwnd(id);
    // A tap of Alt lets SetForegroundWindow through the foreground lock.
    send(&[key_input(VK_MENU, false), key_input(VK_MENU, true)]);
    // SAFETY: plain focus request for a window we own.
    let _ = unsafe { SetForegroundWindow(h) };
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(3) {
        // SAFETY: reads the foreground window handle.
        if unsafe { GetForegroundWindow() } == h {
            return true;
        }
        sleep(Duration::from_millis(50));
    }
    false
}

fn near(a: PxRect, b: PxRect, tolerance: i32) -> bool {
    (a.x - b.x).abs() <= tolerance
        && (a.y - b.y).abs() <= tolerance
        && (a.right() - b.right()).abs() <= tolerance
        && (a.bottom() - b.bottom()).abs() <= tolerance
}

#[test]
#[ignore = "spawns a real notepad.exe window on the interactive desktop"]
fn place_window_and_the_window_queries_work_on_a_real_notepad() {
    let platform = WindowsPlatform::new();
    let Some(np) = Notepad::spawn() else {
        println!("SKIPPED: no Notepad window appeared (Notepad may have opened a tab elsewhere)");
        return;
    };
    assert!(platform.window_exists(np.id));
    assert!(!platform.is_minimized(np.id));
    assert!(!platform.is_maximized(np.id));
    assert!(
        platform.is_movable_target(np.id),
        "notepad is an ordinary movable window"
    );

    let monitors = platform.monitors().expect("monitors");
    let work = monitors
        .iter()
        .find(|m| m.primary)
        .unwrap_or(&monitors[0])
        .work;

    // Instant placement.
    let target = PxRect::new(work.x + 120, work.y + 100, 700, 520);
    let done = platform
        .place_window(np.id, target, work, false)
        .expect("place_window");
    let seen = platform.window_rect(np.id).expect("window_rect");
    println!("target {target:?} -> achieved {done:?}, window_rect {seen:?}");
    assert!(near(done, target, 2), "visible frame lands on the target");
    assert!(
        near(seen, done, 2),
        "window_rect agrees with what place_window returned"
    );

    // Animated placement lands on the target as well.
    let target2 = PxRect::new(work.x + 300, work.y + 200, 800, 600);
    let done2 = platform
        .place_window(np.id, target2, work, true)
        .expect("animated place_window");
    assert!(near(done2, target2, 2));

    // Minimized windows are reported as such and are not movable targets.
    let id = np.id;
    drop(np);
    let started = Instant::now();
    while platform.window_exists(id) && started.elapsed() < Duration::from_secs(5) {
        sleep(Duration::from_millis(100));
    }
    assert!(
        !platform.window_exists(id),
        "a closed window no longer exists"
    );
    assert!(platform.window_rect(id).is_err());
    assert!(!platform.is_movable_target(id));
}

/// Which combos RegisterHotKey accepts here, and (for the chords it accepts) that
/// pressing them really delivers the event. Chords whose OS meaning is dangerous
/// to trigger (anything with the Windows key and Ctrl) are only registered, never pressed.
#[test]
#[ignore = "registers global hotkeys and injects keystrokes into the interactive session"]
fn global_hotkeys_register_and_deliver_for_every_modifier() {
    make_dpi_aware();
    let platform = WindowsPlatform::new();
    let all = [
        ShortcutModifier::WinAlt,
        ShortcutModifier::CtrlAlt,
        ShortcutModifier::CtrlAltShift,
        ShortcutModifier::WinCtrl,
    ];
    for modifier in all {
        let (tx, rx) = mpsc::channel();
        let sink = Arc::new(move |ev: krova_platform::HotkeyEvent| {
            let _ = tx.send(ev.action);
        });
        let handle = platform
            .start_hotkeys(HotkeySpec { modifier }, sink)
            .expect("hotkey thread starts");
        println!(
            "REGISTRATION {}: {} of {} accepted; refused: {:?}",
            modifier.label(),
            handle.registered(),
            HOTKEYS.len(),
            handle.failed()
        );
        assert_eq!(handle.registered() + handle.failed().len(), HOTKEYS.len());

        let safe_to_press = !matches!(modifier, ShortcutModifier::WinCtrl);
        if !safe_to_press {
            continue;
        }
        for def in HOTKEYS {
            let chord = chord_label(modifier, def.key);
            if handle.failed().iter().any(|m| m.starts_with(&chord)) {
                continue; // refused by the OS: pressing it would trigger someone else's action
            }
            let vk = VIRTUAL_KEY(krova_platform::win_events::virtual_key(def.key) as u16);
            press_chord(&modifier_keys(modifier), vk);
            match rx.recv_timeout(Duration::from_secs(2)) {
                Ok(action) => assert_eq!(action, def.action, "{chord}"),
                Err(_) => panic!("{chord} was registered but no event arrived"),
            }
        }
        drop(handle);
        assert!(rx.try_recv().is_err(), "nothing queued after unregistering");
    }
}

/// Real mouse drags on a real window produce the move/size events the linked
/// dividers rely on, and the registered window can be told apart from the rest.
#[test]
#[ignore = "drags a real notepad window with the real mouse"]
fn the_watcher_reports_a_title_bar_drag() {
    let platform = WindowsPlatform::new();
    let Some(np) = Notepad::spawn() else {
        println!("SKIPPED: no Notepad window appeared");
        return;
    };
    let monitors = platform.monitors().expect("monitors");
    let work = monitors
        .iter()
        .find(|m| m.primary)
        .unwrap_or(&monitors[0])
        .work;
    let start = PxRect::new(work.x + 200, work.y + 160, 700, 500);
    platform
        .place_window(np.id, start, work, false)
        .expect("place");

    let (tx, rx) = mpsc::channel();
    let watcher = platform
        .start_watcher(Arc::new(move |ev| {
            let _ = tx.send(ev);
        }))
        .expect("watcher starts");

    // Grab the title bar (near the top, left of the caption buttons) and move it.
    let grab = (start.x + 200, start.y + 14);
    // SAFETY: moves the real cursor for the duration of the test.
    unsafe {
        let _ = SetCursorPos(grab.0, grab.1);
    }
    sleep(Duration::from_millis(200));
    send(&[mouse_input(MOUSEEVENTF_LEFTDOWN)]);
    sleep(Duration::from_millis(150));
    for step in 1..=8 {
        // SAFETY: as above.
        unsafe {
            let _ = SetCursorPos(grab.0 + step * 12, grab.1 + step * 8);
        }
        sleep(Duration::from_millis(40));
    }
    send(&[mouse_input(MOUSEEVENTF_LEFTUP)]);

    let mut saw_start = false;
    let mut saw_end = false;
    let deadline = Instant::now() + Duration::from_secs(4);
    while Instant::now() < deadline && !(saw_start && saw_end) {
        if let Ok(ev) = rx.recv_timeout(Duration::from_millis(200)) {
            match ev {
                WatchEvent::MoveSizeStart(w) if w == np.id => saw_start = true,
                WatchEvent::MoveSizeEnd(w) if w == np.id => saw_end = true,
                _ => {}
            }
        }
    }
    drop(watcher);
    let moved = platform.window_rect(np.id).expect("rect");
    println!("start {start:?} -> after drag {moved:?}; saw start {saw_start}, end {saw_end}");
    assert!(
        saw_start && saw_end,
        "MOVESIZESTART and MOVESIZEEND for the dragged window"
    );
    assert!(moved.x > start.x, "the drag really moved the window");
}

/// Dragging the right edge reports a resize that the engine can compare with the
/// remembered rect. Skips when the window does not expose a resizable right edge.
#[test]
#[ignore = "resizes a real notepad window with the real mouse"]
fn the_watcher_reports_an_edge_resize() {
    let platform = WindowsPlatform::new();
    let Some(np) = Notepad::spawn() else {
        println!("SKIPPED: no Notepad window appeared");
        return;
    };
    let monitors = platform.monitors().expect("monitors");
    let work = monitors
        .iter()
        .find(|m| m.primary)
        .unwrap_or(&monitors[0])
        .work;
    let start = PxRect::new(work.x + 200, work.y + 160, 700, 500);
    platform
        .place_window(np.id, start, work, false)
        .expect("place");

    // Find a point on the right resize border by asking the window itself.
    let hit = |x: i32, y: i32| -> isize {
        let lparam = ((y as u16 as isize) << 16) | (x as u16 as isize & 0xFFFF);
        // SAFETY: WM_NCHITTEST is a read-only query.
        unsafe { SendMessageW(hwnd(np.id), WM_NCHITTEST, WPARAM(0), LPARAM(lparam)) }.0
    };
    let y = start.y + start.h / 2;
    let Some(x) = (start.right() - 4..=start.right() + 10).find(|&x| hit(x, y) == 11) else {
        println!("SKIPPED: no right-edge resize border found at the visible edge");
        return;
    };

    let (tx, rx) = mpsc::channel();
    let watcher = platform
        .start_watcher(Arc::new(move |ev| {
            let _ = tx.send(ev);
        }))
        .expect("watcher starts");

    // SAFETY: moves the real cursor for the duration of the test.
    unsafe {
        let _ = SetCursorPos(x, y);
    }
    sleep(Duration::from_millis(200));
    send(&[mouse_input(MOUSEEVENTF_LEFTDOWN)]);
    sleep(Duration::from_millis(150));
    for step in 1..=8 {
        // SAFETY: as above.
        unsafe {
            let _ = SetCursorPos(x + step * 15, y);
        }
        sleep(Duration::from_millis(40));
    }
    send(&[mouse_input(MOUSEEVENTF_LEFTUP)]);

    let mut saw_end = false;
    let deadline = Instant::now() + Duration::from_secs(4);
    while Instant::now() < deadline && !saw_end {
        if let Ok(WatchEvent::MoveSizeEnd(w)) = rx.recv_timeout(Duration::from_millis(200)) {
            saw_end = w == np.id;
        }
    }
    drop(watcher);
    let after = platform.window_rect(np.id).expect("rect");
    println!("start {start:?} -> after resize {after:?}; saw end {saw_end}");
    assert!(saw_end, "MOVESIZEEND for the resized window");
    assert!(after.w > start.w, "the edge drag really widened the window");
}

#[test]
#[ignore = "needs a window that can take the keyboard focus"]
fn the_foreground_window_is_the_one_we_focused() {
    let platform = WindowsPlatform::new();
    let Some(np) = Notepad::spawn() else {
        println!("SKIPPED: no Notepad window appeared");
        return;
    };
    if !focus(np.id) {
        println!("SKIPPED: the session did not let this test take the focus");
        return;
    }
    assert_eq!(platform.foreground_window(), Some(np.id));
    assert!(platform.is_movable_target(np.id));
}
