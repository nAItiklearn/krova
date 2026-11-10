//! Share-safe for native overlays (F-29): while `share_safe` is on, every Krova
//! overlay window is excluded from screen capture (`WDA_EXCLUDEFROMCAPTURE`), so
//! a screen share or recording never shows it. Turning it off restores
//! `WDA_NONE`.
//!
//! Overlay windows register themselves right after `CreateWindowExW` and
//! unregister before `DestroyWindow`. This is defence in depth: an overlay that
//! ever draws an app name, an icon or window contents must still check
//! [`crate::desk::share_safe`] and draw only zone shapes while it is on.

use std::collections::HashSet;
use std::ffi::c_void;
use std::sync::Mutex;

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WDA_MONITOR, WDA_NONE,
    WINDOW_DISPLAY_AFFINITY,
};

/// Overlay windows of this process, as raw handle values (an `HWND` is not `Send`).
static OVERLAYS: Mutex<Option<HashSet<isize>>> = Mutex::new(None);

fn with_overlays<R>(f: impl FnOnce(&mut HashSet<isize>) -> R) -> R {
    let mut guard = OVERLAYS.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(HashSet::new))
}

fn to_hwnd(raw: isize) -> HWND {
    HWND(raw as *mut c_void)
}

pub(crate) fn affinity_for(share_safe: bool) -> WINDOW_DISPLAY_AFFINITY {
    if share_safe {
        WDA_EXCLUDEFROMCAPTURE
    } else {
        WDA_NONE
    }
}

fn apply(h: HWND, share_safe: bool) {
    // SAFETY: `h` is a window of this process (only our overlays register).
    let ok = unsafe { SetWindowDisplayAffinity(h, affinity_for(share_safe)) }.is_ok();
    if !ok && share_safe {
        // Before Windows 10 2004 only WDA_MONITOR exists: the overlay then shows as
        // black in a capture, which still hides what it draws.
        // SAFETY: as above.
        let _ = unsafe { SetWindowDisplayAffinity(h, WDA_MONITOR) };
    }
}

/// Call right after creating an overlay window: it is tracked and gets the
/// current share-safe affinity.
pub(crate) fn register_overlay(h: HWND) {
    if h.is_invalid() {
        return;
    }
    with_overlays(|set| set.insert(h.0 as isize));
    apply(h, crate::desk::share_safe());
}

/// Call before destroying an overlay window.
pub(crate) fn unregister_overlay(h: HWND) {
    with_overlays(|set| set.remove(&(h.0 as isize)));
}

/// Re-apply the affinity to every tracked overlay (share-safe switched).
pub(crate) fn apply_all(share_safe: bool) {
    let all: Vec<isize> = with_overlays(|set| set.iter().copied().collect());
    for raw in all {
        apply(to_hwnd(raw), share_safe);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::core::PCWSTR;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, GetWindowDisplayAffinity, WS_EX_TOOLWINDOW, WS_POPUP,
    };

    fn affinity(h: HWND) -> u32 {
        let mut a = 0u32;
        // SAFETY: plain query on our own window.
        let _ = unsafe { GetWindowDisplayAffinity(h, &mut a) };
        a
    }

    #[test]
    fn share_safe_hides_overlays_from_capture_and_restores_them() {
        assert_eq!(affinity_for(true), WDA_EXCLUDEFROMCAPTURE);
        assert_eq!(affinity_for(false), WDA_NONE);
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        // SAFETY: a hidden popup of the system STATIC class, destroyed below.
        let h = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW,
                PCWSTR(class.as_ptr()),
                PCWSTR::null(),
                WS_POPUP,
                0,
                0,
                1,
                1,
                HWND::default(),
                None,
                None,
                None,
            )
        }
        .expect("a test window");
        register_overlay(h);
        apply_all(true);
        assert_ne!(
            affinity(h),
            WDA_NONE.0,
            "excluded from capture while share-safe"
        );
        apply_all(false);
        assert_eq!(affinity(h), WDA_NONE.0);
        unregister_overlay(h);
        assert!(!with_overlays(|s| s.contains(&(h.0 as isize))));
        // SAFETY: our own window.
        let _ = unsafe { DestroyWindow(h) };
    }
}
