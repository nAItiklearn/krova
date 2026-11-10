//! Focus follows mouse on Windows: what is under the resting cursor, and the
//! activation itself.
//!
//! Activation uses only `SetForegroundWindow`, the documented API, and nothing
//! that works around its rules (no `AttachThreadInput`, no injected key presses,
//! no changing the user's system-wide "activate window by hovering" setting).
//! Documented limit: Windows lets a background process take the foreground only
//! in some situations (the foreground-lock rules of `SetForegroundWindow`). When
//! it refuses, Windows flashes the window's taskbar button instead; Krova then
//! reports the refusal and does not try that same window again until the cursor
//! has rested on another one, so a refusal flashes once, not over and over.

use std::mem::size_of;

use krova_core::ffm::{decide, Decision, Observation};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONULL,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    IsWindowEnabled, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON, VK_XBUTTON1, VK_XBUTTON2,
};
use windows::Win32::UI::Shell::{
    SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetGUIThreadInfo, GetWindowLongW, GetWindowRect, GetWindowThreadProcessId,
    SetForegroundWindow, GUITHREADINFO, GUI_INMENUMODE, GUI_INMOVESIZE, GUI_POPUPMENUMODE,
    GUI_SYSTEMMENUMODE, GWL_EXSTYLE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};

use crate::win::{
    class_name, is_own_process_window, key_down, snappable, target_is_protected, window_at,
};

/// Any mouse button held right now.
pub(crate) fn any_button_down() -> bool {
    [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON, VK_XBUTTON1, VK_XBUTTON2]
        .into_iter()
        .any(key_down)
}

/// Desktop and taskbar windows: never activated, never a "full-screen app".
fn is_shell(h: HWND) -> bool {
    matches!(
        class_name(h).as_str(),
        "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
    )
}

/// A window that can be activated by an ordinary click: captioned, enabled (a
/// window with an open modal dialog is disabled), not a tool palette and not one
/// that asks never to be activated.
fn activatable(h: HWND) -> bool {
    if !snappable(h) || !unsafe { IsWindowEnabled(h) }.as_bool() {
        return false;
    }
    let ex = unsafe { GetWindowLongW(h, GWL_EXSTYLE) } as u32;
    ex & (WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0) == 0
}

/// A menu (or popup menu, or the system menu) is open, or a move/size loop is
/// running, in the thread that owns the foreground window.
fn menu_open(foreground: HWND) -> bool {
    if foreground.0.is_null() {
        return false;
    }
    let thread = unsafe { GetWindowThreadProcessId(foreground, None) };
    let mut info = GUITHREADINFO {
        cbSize: size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };
    if unsafe { GetGUIThreadInfo(thread, &mut info) }.is_err() {
        return false;
    }
    let busy = GUI_INMENUMODE.0 | GUI_POPUPMENUMODE.0 | GUI_SYSTEMMENUMODE.0 | GUI_INMOVESIZE.0;
    info.flags.0 & busy != 0 || !info.hwndMenuOwner.0.is_null()
}

/// A full-screen app is in front: the shell says so (games, presentations,
/// "busy"), or the foreground window covers its whole monitor.
fn fullscreen_in_front(foreground: HWND) -> bool {
    if let Ok(state) = unsafe { SHQueryUserNotificationState() } {
        if state == QUNS_BUSY
            || state == QUNS_RUNNING_D3D_FULL_SCREEN
            || state == QUNS_PRESENTATION_MODE
        {
            return true;
        }
    }
    if foreground.0.is_null() || is_shell(foreground) {
        return false;
    }
    let monitor = unsafe { MonitorFromWindow(foreground, MONITOR_DEFAULTTONULL) };
    if monitor.is_invalid() {
        return false;
    }
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        return false;
    }
    let mut r = RECT::default();
    if unsafe { GetWindowRect(foreground, &mut r) }.is_err() {
        return false;
    }
    let m = info.rcMonitor;
    r.left <= m.left && r.top <= m.top && r.right >= m.right && r.bottom >= m.bottom
}

/// What happened when the cursor came to rest at `(x, y)`.
pub(crate) enum FocusOutcome {
    Skipped,
    Focused(HWND),
    Refused(HWND),
}

/// Look at the window under the resting cursor and activate it when every rule
/// allows it (`krova_core::ffm::decide`). `refused_before` is the window the OS
/// refused last time.
pub(crate) fn focus_at(
    x: i32,
    y: i32,
    clicked_during_rest: bool,
    refused_before: Option<HWND>,
) -> FocusOutcome {
    let target = window_at(x, y);
    let foreground = unsafe { GetForegroundWindow() };
    let observation = match target {
        None => Observation::default(),
        Some(h) => Observation {
            has_window: true,
            button_down: any_button_down(),
            clicked_during_rest,
            arrangeable: !is_shell(h) && activatable(h),
            own_window: is_own_process_window(h),
            menu_open: menu_open(foreground),
            // Only asked when it matters: it opens the target's process.
            elevated: false,
            fullscreen_in_front: fullscreen_in_front(foreground),
            already_foreground: h == foreground,
            refused_before: refused_before == Some(h),
        },
    };
    if decide(&observation) != Decision::Focus {
        return FocusOutcome::Skipped;
    }
    let Some(h) = target else {
        return FocusOutcome::Skipped;
    };
    let observation = Observation {
        elevated: target_is_protected(h),
        ..observation
    };
    if decide(&observation) != Decision::Focus {
        return FocusOutcome::Skipped;
    }
    // Last look at the buttons: the checks above take a moment.
    if any_button_down() {
        return FocusOutcome::Skipped;
    }
    let _ = unsafe { SetForegroundWindow(h) };
    if unsafe { GetForegroundWindow() } == h {
        FocusOutcome::Focused(h)
    } else {
        FocusOutcome::Refused(h)
    }
}
