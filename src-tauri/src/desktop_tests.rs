//! End-to-end tests of the engine against real windows on the interactive desktop.
//!
//! They start the real engine (without the global mouse hook and the overlay),
//! register the real global shortcuts, press keys with `SendInput` and watch where
//! a spawned `notepad.exe` ends up. Because they take over the keyboard and the
//! mouse for a few seconds they are all `#[ignore]`; run them with
//!
//! ```text
//! cargo test --manifest-path src-tauri/Cargo.toml -- --ignored --nocapture --test-threads=1 desktop
//! ```
//!
//! Window titles are never read: Notepad is found by window class and by the
//! handles that appeared after it was started.

use std::ffi::c_void;
use std::mem::size_of;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Arc;
use std::thread::sleep;
use std::time::{Duration, Instant};

use krova_core::registry::WindowKey;
use krova_core::{Config, MoveMode, PxRect, ShortcutModifier};
use krova_platform::{Platform, WindowId};
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, TRUE, WPARAM};
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYBD_EVENT_FLAGS,
    KEYEVENTF_KEYUP, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEINPUT, MOUSE_EVENT_FLAGS,
    VIRTUAL_KEY, VK_LEFT, VK_LWIN, VK_MENU, VK_RIGHT, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetForegroundWindow, IsWindow, IsWindowVisible, PostMessageW,
    SendMessageW, SetCursorPos, SetForegroundWindow, WM_CLOSE, WM_NCHITTEST,
};

use crate::engine;
use crate::model::{lock, AppModel, Shared};

fn hwnd(id: WindowId) -> HWND {
    HWND(id.0 as usize as *mut c_void)
}

unsafe extern "system" fn collect(h: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: lparam is the address of the Vec that `top_level_windows` keeps alive.
    let list = unsafe { &mut *(lparam.0 as *mut Vec<usize>) };
    list.push(h.0 as usize);
    TRUE
}

fn notepad_windows() -> Vec<usize> {
    let mut all: Vec<usize> = Vec::new();
    // SAFETY: the callback only pushes into `all`, which outlives the call.
    let _ = unsafe { EnumWindows(Some(collect), LPARAM(&mut all as *mut Vec<usize> as isize)) };
    all.into_iter()
        .filter(|&w| {
            let h = HWND(w as *mut c_void);
            let mut buf = [0u16; 64];
            // SAFETY: the buffer is valid for the length passed.
            let n = unsafe { GetClassNameW(h, &mut buf) };
            let class = String::from_utf16_lossy(&buf[..usize::try_from(n).unwrap_or(0)]);
            // SAFETY: querying a handle that may have died is harmless.
            class == "Notepad" && unsafe { IsWindowVisible(h) }.as_bool()
        })
        .collect()
}

/// A real Notepad window, closed again when dropped.
pub(crate) struct Notepad {
    pub(crate) id: WindowId,
    child: Child,
}

impl Notepad {
    pub(crate) fn spawn() -> Option<Notepad> {
        // SAFETY: plain process-wide setting; fails harmlessly if already set.
        let _ =
            unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        let before = notepad_windows();
        let mut child = Command::new("notepad.exe").spawn().ok()?;
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(15) {
            if let Some(&new) = notepad_windows().iter().find(|w| !before.contains(w)) {
                sleep(Duration::from_millis(600));
                return Some(Notepad {
                    id: WindowId(new as u64),
                    child,
                });
            }
            sleep(Duration::from_millis(100));
        }
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

fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
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

fn mouse(flags: MOUSE_EVENT_FLAGS) -> INPUT {
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

/// Win+Alt+`vk`.
fn win_alt(vk: VIRTUAL_KEY) {
    send(&[
        key(VK_LWIN, false),
        key(VK_MENU, false),
        key(vk, false),
        key(vk, true),
        key(VK_MENU, true),
        key(VK_LWIN, true),
    ]);
}

fn digit(n: u8) -> VIRTUAL_KEY {
    VIRTUAL_KEY(u16::from(b'0') + u16::from(n))
}

pub(crate) fn focus(id: WindowId) -> bool {
    let h = hwnd(id);
    send(&[key(VK_MENU, false), key(VK_MENU, true)]);
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

pub(crate) fn near(a: PxRect, b: PxRect) -> bool {
    [
        (a.x, b.x),
        (a.y, b.y),
        (a.right(), b.right()),
        (a.bottom(), b.bottom()),
    ]
    .iter()
    .all(|&(p, q)| (p - q).abs() <= 3)
}

pub(crate) fn wait_for_rect(platform: &dyn Platform, id: WindowId, want: PxRect) -> Option<PxRect> {
    let started = Instant::now();
    let mut last = None;
    while started.elapsed() < Duration::from_secs(4) {
        if let Ok(r) = platform.window_rect(id) {
            last = Some(r);
            if near(r, want) {
                return Some(r);
            }
        }
        sleep(Duration::from_millis(100));
    }
    println!("never reached {want:?}; last seen {last:?}");
    None
}

/// A running engine on this machine's real monitors, with instant moves.
fn start_engine(modifier: ShortcutModifier) -> Arc<Shared> {
    let platform: Arc<dyn Platform> = Arc::from(krova_platform::default_platform());
    let mut model = AppModel::new(Config::default(), PathBuf::from("unused.json"), false);
    model.config.settings.move_mode = MoveMode::Instant;
    model.config.settings.shortcut_modifier = modifier;
    model
        .refresh_monitors(platform.monitors().expect("monitors"))
        .unwrap();
    let shared = Arc::new(Shared::new(model, platform));
    engine::start_headless(&shared);
    shared
}

/// The primary monitor's work area and its active layout.
fn primary(shared: &Shared) -> (PxRect, krova_core::Layout) {
    let model = lock(&shared.model);
    let views = model.views();
    let monitors = shared.platform.monitors().unwrap();
    let i = monitors.iter().position(|m| m.primary).unwrap_or(0);
    (views[i].work, views[i].layout.unwrap().clone())
}

#[test]
#[ignore = "spawns notepad, registers global shortcuts and injects keystrokes"]
fn desktop_keyboard_shortcuts_move_notepad_to_the_right_zones() {
    let shared = start_engine(ShortcutModifier::WinAlt);
    let status = lock(&shared.features).shortcuts.clone();
    println!(
        "SHORTCUTS active={} refused={:?}",
        status.active, status.failed
    );
    assert!(status.active, "some shortcuts must register");
    let refused = |chord_suffix: &str| {
        status
            .failed
            .iter()
            .any(|m| m.starts_with(&format!("Win+Alt+{chord_suffix} ")))
    };

    let Some(np) = Notepad::spawn() else {
        println!("SKIPPED: no Notepad window appeared");
        return;
    };
    let platform = Arc::clone(&shared.platform);
    let (work, layout) = primary(&shared);
    println!(
        "work area {work:?}, layout {} with {} zones",
        layout.id,
        layout.zones.len()
    );
    let start = PxRect::new(work.x + 150, work.y + 120, 520, 420);
    platform
        .place_window(np.id, start, work, false)
        .expect("place");
    if !focus(np.id) {
        println!("SKIPPED: the session did not let this test take the focus");
        return;
    }

    // Win+Alt+2: zone number 2.
    if !refused("2") {
        win_alt(digit(2));
        let want = layout
            .zone_pixels_gapped(zone_number(&layout, 2), work, 0)
            .unwrap();
        let got = wait_for_rect(&*platform, np.id, want);
        println!("Win+Alt+2 -> want {want:?}, got {got:?}");
        assert!(got.is_some(), "notepad did not reach zone 2");
        let record = lock(&shared.registry)
            .get(WindowKey(np.id.0))
            .cloned()
            .unwrap();
        assert_eq!(record.zone_id, zone_number(&layout, 2));
        assert_eq!(record.layout_id, layout.id);
    }

    // Win+Alt+1 then Right: zone 1, then the next zone (2).
    if !refused("1") {
        win_alt(digit(1));
        let want1 = layout
            .zone_pixels_gapped(zone_number(&layout, 1), work, 0)
            .unwrap();
        assert!(wait_for_rect(&*platform, np.id, want1).is_some(), "zone 1");
        if !status
            .failed
            .iter()
            .any(|m| m.starts_with("Win+Alt+Right "))
        {
            win_alt(VK_RIGHT);
            let want2 = layout
                .zone_pixels_gapped(zone_number(&layout, 2), work, 0)
                .unwrap();
            assert!(
                wait_for_rect(&*platform, np.id, want2).is_some(),
                "Right -> zone 2"
            );
        }
        if !status.failed.iter().any(|m| m.starts_with("Win+Alt+Left ")) {
            win_alt(VK_LEFT);
            assert!(
                wait_for_rect(&*platform, np.id, want1).is_some(),
                "Left -> zone 1"
            );
        }
    }

    // Up with a single monitor does nothing.
    if platform.monitors().unwrap().len() == 1
        && !status.failed.iter().any(|m| m.starts_with("Win+Alt+Up "))
    {
        let before = platform.window_rect(np.id).unwrap();
        win_alt(VK_UP);
        sleep(Duration::from_millis(600));
        assert!(
            near(platform.window_rect(np.id).unwrap(), before),
            "Up with one monitor is a no-op"
        );
    }

    // Disabling the shortcuts in the settings releases the keys.
    crate::commands::change_settings(&shared, |s| s.keyboard_shortcuts = false).unwrap();
    assert!(!lock(&shared.features).shortcuts.active);
    assert!(lock(&shared.features).hotkeys.is_none());
    // And re-enabling registers them again, with a different modifier.
    crate::commands::change_settings(&shared, |s| {
        s.keyboard_shortcuts = true;
        s.shortcut_modifier = ShortcutModifier::CtrlAltShift;
    })
    .unwrap();
    let again = lock(&shared.features).shortcuts.clone();
    println!(
        "after re-enabling with Ctrl+Alt+Shift: active={} refused={:?}",
        again.active, again.failed
    );
    assert!(again.active);
    drop(lock(&shared.features).hotkeys.take());
}

fn zone_number(layout: &krova_core::Layout, n: usize) -> u32 {
    krova_core::registry::zone_by_number(layout, n).expect("zone exists")
}

/// Two notepads side by side; dragging the seam between them with the real mouse
/// makes the other one follow.
#[test]
#[ignore = "spawns two notepads, injects keystrokes and drags a window edge with the real mouse"]
fn desktop_linked_dividers_move_the_neighbours_edge() {
    let shared = start_engine(ShortcutModifier::WinAlt);
    let status = lock(&shared.features).shortcuts.clone();
    if status
        .failed
        .iter()
        .any(|m| m.starts_with("Win+Alt+1 ") || m.starts_with("Win+Alt+2 "))
    {
        println!("SKIPPED: Win+Alt+1 or 2 is taken on this machine");
        return;
    }
    let (Some(left), Some(right)) = (Notepad::spawn(), Notepad::spawn()) else {
        println!("SKIPPED: could not start two Notepad windows");
        return;
    };
    let platform = Arc::clone(&shared.platform);
    let (work, layout) = primary(&shared);
    for (np, n) in [(&left, 1u8), (&right, 2u8)] {
        platform
            .place_window(
                np.id,
                PxRect::new(work.x + 100, work.y + 100, 520, 420),
                work,
                false,
            )
            .unwrap();
        if !focus(np.id) {
            println!("SKIPPED: the session did not let this test take the focus");
            return;
        }
        win_alt(digit(n));
        let want = layout
            .zone_pixels_gapped(zone_number(&layout, usize::from(n)), work, 0)
            .unwrap();
        assert!(
            wait_for_rect(&*platform, np.id, want).is_some(),
            "notepad {n} in its zone"
        );
    }
    assert_eq!(lock(&shared.registry).len(), 2);

    let before_left = platform.window_rect(left.id).unwrap();
    let before_right = platform.window_rect(right.id).unwrap();
    println!("before: left {before_left:?}, right {before_right:?}");

    // Find the left window's right resize border by asking the window itself.
    let y = before_left.y + before_left.h / 2;
    let hit = |x: i32| -> isize {
        let lparam = ((y as u16 as isize) << 16) | (x as u16 as isize & 0xFFFF);
        // SAFETY: WM_NCHITTEST is a read-only query.
        unsafe { SendMessageW(hwnd(left.id), WM_NCHITTEST, WPARAM(0), LPARAM(lparam)) }.0
    };
    let Some(x) = (before_left.right() - 4..=before_left.right() + 10).find(|&x| hit(x) == 11)
    else {
        println!("SKIPPED: no right-edge resize border found on the left window");
        return;
    };

    // SAFETY: moves the real cursor for the duration of the test.
    unsafe {
        let _ = SetCursorPos(x, y);
    }
    sleep(Duration::from_millis(250));
    send(&[mouse(MOUSEEVENTF_LEFTDOWN)]);
    sleep(Duration::from_millis(150));
    for step in 1..=8 {
        // SAFETY: as above.
        unsafe {
            let _ = SetCursorPos(x - step * 12, y);
        }
        sleep(Duration::from_millis(40));
    }
    send(&[mouse(MOUSEEVENTF_LEFTUP)]);

    // The engine reacts to MOVESIZEEND: the right window's left edge follows.
    let started = Instant::now();
    let mut moved = false;
    let (mut now_left, mut now_right) = (before_left, before_right);
    while started.elapsed() < Duration::from_secs(5) {
        now_left = platform.window_rect(left.id).unwrap();
        now_right = platform.window_rect(right.id).unwrap();
        if now_right.x != before_right.x {
            moved = true;
            break;
        }
        sleep(Duration::from_millis(150));
    }
    println!("after: left {now_left:?}, right {now_right:?}");
    assert!(
        now_left.w < before_left.w,
        "the drag really narrowed the left window"
    );
    assert!(moved, "the right window did not follow the dragged seam");
    assert!(
        (now_right.x - now_left.right()).abs() <= 3,
        "the seam stays closed"
    );
    assert_eq!(
        now_right.right(),
        before_right.right(),
        "the far edge stays put"
    );
    let reg = lock(&shared.registry);
    assert!(near(reg.get(WindowKey(left.id.0)).unwrap().rect, now_left));
    assert!(near(
        reg.get(WindowKey(right.id.0)).unwrap().rect,
        now_right
    ));
}
