//! Window lookup (CGWindowList) and placement (Accessibility API) for macOS.
//!
//! STATUS: written without a Mac or a compiler (see `mac_ffi`).
//!
//! How a [`WindowId`] maps to an AX element: the id IS the `CGWindowID` (widened
//! to u64). There is no registry. At placement time
//! 1. `CGWindowListCopyWindowInfo(kCGWindowListOptionIncludingWindow, id)` gives
//!    the owner pid and the window's frame (this also tells us the window is gone);
//! 2. `AXUIElementCreateApplication(pid)` + `kAXWindowsAttribute` lists the app's
//!    AX windows;
//! 3. `_AXUIElementGetWindow` (private but used by every window manager; resolved
//!    with `dlsym`) tells which AX window carries that `CGWindowID`. If the symbol
//!    is missing the window is matched by frame instead (`mac_math::pick_ax_window`).
//!
//! Window titles are never requested: neither the Accessibility title attribute
//! nor the window-list name key is read anywhere in this crate.

#![allow(dead_code, unused_unsafe, unused_imports)]

use std::ffi::c_void;
use std::mem::transmute;
use std::ptr::null;
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};

use krova_core::window::ANIMATION_MS;
use krova_core::{animate_step, clamp_to_limits, PxRect};

use super::mac_ffi::*;
use crate::mac_math::{
    classify_ax_error, infer_limits, pick_ax_window, round_rect, AxCandidate, AxFailure,
    WindowEntry, SIZE_TOLERANCE,
};
use crate::{PlatformError, WindowId};

// ------------------------------------------------------- window list

/// Read a `CGWindowListCopyWindowInfo` result. Front-to-back order is preserved.
pub fn list_windows(option: u32, relative_to: u32) -> Vec<WindowEntry> {
    let mut out = Vec::new();
    unsafe {
        let Some(array) = CfOwned::new(CGWindowListCopyWindowInfo(option, relative_to)) else {
            return out;
        };
        let count = CFArrayGetCount(array.ptr());
        for i in 0..count {
            let dict = CFArrayGetValueAtIndex(array.ptr(), i);
            if dict.is_null() {
                continue;
            }
            if let Some(entry) = read_window(dict) {
                out.push(entry);
            }
        }
    }
    out
}

unsafe fn read_window(dict: CFTypeRef) -> Option<WindowEntry> {
    let id = u32::try_from(dict_i32(dict, kCGWindowNumber)?).ok()?;
    let pid = dict_i32(dict, kCGWindowOwnerPID).unwrap_or(0);
    // A window without a layer is treated as a system layer and never hit.
    let layer = dict_i32(dict, kCGWindowLayer).unwrap_or(i32::MAX);
    let alpha = dict_f64(dict, kCGWindowAlpha).unwrap_or(1.0);
    let bounds = CFDictionaryGetValue(dict, kCGWindowBounds);
    if bounds.is_null() {
        return None;
    }
    let mut r = CGRect::default();
    if !CGRectMakeWithDictionaryRepresentation(bounds, &mut r) {
        return None;
    }
    Some(WindowEntry {
        id,
        pid,
        layer,
        alpha,
        bounds: round_rect(r.origin.x, r.origin.y, r.size.width, r.size.height),
    })
}

/// The normal on-screen window under a screen point (current Space only).
pub fn window_at(x: i32, y: i32) -> Option<(u32, PxRect)> {
    let list = list_windows(LIST_ON_SCREEN_ONLY | LIST_EXCLUDE_DESKTOP, 0);
    let i = crate::mac_math::window_hit((x, y), &list)?;
    Some((list[i].id, list[i].bounds))
}

/// Current frame of one window, `None` when it no longer exists.
pub fn window_frame(id: u32) -> Option<PxRect> {
    list_windows(LIST_INCLUDING_WINDOW, id)
        .into_iter()
        .find(|w| w.id == id)
        .map(|w| w.bounds)
}

// ------------------------------------------------------- AX helpers

/// Attribute names. The `kAX*Attribute` constants are C macros, not symbols, so
/// the strings are spelled out here.
struct AxNames {
    position: CfOwned,
    size: CfOwned,
    windows: CfOwned,
    minimized: CfOwned,
    full_screen: CfOwned,
}

impl AxNames {
    fn new() -> Option<Self> {
        Some(Self {
            position: cf_string("AXPosition")?,
            size: cf_string("AXSize")?,
            windows: cf_string("AXWindows")?,
            minimized: cf_string("AXMinimized")?,
            full_screen: cf_string("AXFullScreen")?,
        })
    }
}

fn os_error(msg: &str) -> PlatformError {
    PlatformError::Os(msg.to_string())
}

fn ax_error(code: i32) -> PlatformError {
    match classify_ax_error(code) {
        AxFailure::WindowGone => PlatformError::WindowGone,
        AxFailure::PermissionDenied => {
            PlatformError::Refused("Accessibility permission is not granted")
        }
        AxFailure::Unsupported => PlatformError::Refused("the window cannot be moved or resized"),
        AxFailure::Other(c) => PlatformError::Os(format!("accessibility error {c}")),
    }
}

/// `AXUIElementCopyAttributeValue`; the error is the raw `AXError`.
unsafe fn ax_copy(element: AXUIElementRef, attribute: &CfOwned) -> Result<CfOwned, i32> {
    let mut out: CFTypeRef = null();
    let err = AXUIElementCopyAttributeValue(element, attribute.ptr(), &mut out);
    if err != 0 {
        return Err(err);
    }
    CfOwned::new(out).ok_or(crate::mac_math::AX_ERROR_NO_VALUE)
}

unsafe fn ax_bool(element: AXUIElementRef, attribute: &CfOwned) -> Option<bool> {
    let value = ax_copy(element, attribute).ok()?;
    cf_bool(value.ptr())
}

unsafe fn ax_frame(element: AXUIElementRef, names: &AxNames) -> Option<PxRect> {
    let position = ax_copy(element, &names.position).ok()?;
    let size = ax_copy(element, &names.size).ok()?;
    let mut p = CGPoint::default();
    let mut s = CGSize::default();
    if AXValueGetValue(
        position.ptr(),
        AX_VALUE_CGPOINT,
        (&mut p as *mut CGPoint).cast(),
    ) == 0
    {
        return None;
    }
    if AXValueGetValue(size.ptr(), AX_VALUE_CGSIZE, (&mut s as *mut CGSize).cast()) == 0 {
        return None;
    }
    Some(round_rect(p.x, p.y, s.width, s.height))
}

/// `AXIllegalArgument`, reported when an `AXValue` cannot even be built.
const AX_ERROR_ILLEGAL_ARGUMENT: i32 = -25201;

/// Set a CGPoint / CGSize attribute; the error is the raw `AXError`.
unsafe fn ax_set(
    element: AXUIElementRef,
    attribute: &CfOwned,
    kind: u32,
    value: *const c_void,
) -> Result<(), i32> {
    let boxed = CfOwned::new(AXValueCreate(kind, value)).ok_or(AX_ERROR_ILLEGAL_ARGUMENT)?;
    match AXUIElementSetAttributeValue(element, attribute.ptr(), boxed.ptr()) {
        0 => Ok(()),
        code => Err(code),
    }
}

/// Position, then size; with `verify` also position again when the window did not
/// accept the size (it may have grown past a screen edge and been pushed back).
///
/// A window that refuses a size change (fixed-size panels) has still moved, so
/// that is not an error here: the caller reads the frame back and infers limits.
/// A vanished window or a missing permission is.
unsafe fn set_frame(
    element: AXUIElementRef,
    names: &AxNames,
    r: PxRect,
    verify: bool,
) -> Result<(), PlatformError> {
    let p = CGPoint {
        x: f64::from(r.x),
        y: f64::from(r.y),
    };
    let s = CGSize {
        width: f64::from(r.w),
        height: f64::from(r.h),
    };
    let set_position = || {
        ax_set(
            element,
            &names.position,
            AX_VALUE_CGPOINT,
            (&p as *const CGPoint).cast(),
        )
        .map_err(ax_error)
    };
    set_position()?;
    let sized = ax_set(
        element,
        &names.size,
        AX_VALUE_CGSIZE,
        (&s as *const CGSize).cast(),
    );
    if let Err(code) = sized {
        if matches!(
            classify_ax_error(code),
            AxFailure::WindowGone | AxFailure::PermissionDenied
        ) {
            return Err(ax_error(code));
        }
    }
    if verify {
        if let Some(now) = ax_frame(element, names) {
            if (now.w, now.h) != (r.w, r.h) {
                set_position()?;
            }
        }
    }
    Ok(())
}

unsafe fn animate_move(element: AXUIElementRef, names: &AxNames, from: PxRect, to: PxRect) {
    let start = Instant::now();
    loop {
        let elapsed = start.elapsed().as_millis().min(u128::from(ANIMATION_MS)) as u32;
        if elapsed >= ANIMATION_MS {
            break;
        }
        if set_frame(element, names, animate_step(from, to, elapsed), false).is_err() {
            break;
        }
        thread::sleep(Duration::from_millis(8));
    }
}

// ------------------------------------------------ CGWindowID <-> AX window

type AxGetWindowFn = unsafe extern "C" fn(AXUIElementRef, *mut u32) -> AXError;

/// `_AXUIElementGetWindow(AXUIElementRef, CGWindowID *)`: private, but exported
/// by HIServices and relied on by Rectangle, yabai and others.
fn ax_get_window_fn() -> Option<AxGetWindowFn> {
    static ADDR: OnceLock<usize> = OnceLock::new();
    let addr = *ADDR.get_or_init(|| dlsym_default("_AXUIElementGetWindow") as usize);
    if addr == 0 {
        None
    } else {
        // SAFETY: the symbol has this C signature.
        Some(unsafe { transmute::<usize, AxGetWindowFn>(addr) })
    }
}

/// The AX window element for `wanted_id`. The returned pointer is borrowed from
/// `windows` and valid only while that array is alive.
unsafe fn find_window(
    windows: &CfOwned,
    wanted_id: u32,
    wanted: PxRect,
    names: &AxNames,
) -> Result<AXUIElementRef, PlatformError> {
    let get_window = ax_get_window_fn();
    let count = CFArrayGetCount(windows.ptr());
    let mut elements: Vec<AXUIElementRef> = Vec::new();
    let mut candidates: Vec<AxCandidate> = Vec::new();
    for i in 0..count {
        let element = CFArrayGetValueAtIndex(windows.ptr(), i);
        if element.is_null() {
            continue;
        }
        let window_id = get_window.and_then(|f| {
            let mut id = 0u32;
            (f(element, &mut id) == 0).then_some(id)
        });
        elements.push(element);
        candidates.push(AxCandidate {
            window_id,
            frame: ax_frame(element, names),
        });
    }
    let index = pick_ax_window(wanted_id, wanted, &candidates).ok_or(PlatformError::Refused(
        "the app does not expose this window to Accessibility",
    ))?;
    Ok(elements[index])
}

// ------------------------------------------------------------ placement

/// See `Platform::place_window`. Mirrors `win.rs`: set, check what the window
/// really did, infer its size limits from the difference, place once more.
pub fn place_window(
    window: WindowId,
    target: PxRect,
    work: PxRect,
    animate: bool,
) -> Result<PxRect, PlatformError> {
    let id = u32::try_from(window.0).map_err(|_| PlatformError::WindowGone)?;
    if !super::accessibility_granted() {
        return Err(PlatformError::Refused(
            "Accessibility permission is not granted",
        ));
    }
    let entry = list_windows(LIST_INCLUDING_WINDOW, id)
        .into_iter()
        .find(|w| w.id == id)
        .ok_or(PlatformError::WindowGone)?;
    let names =
        AxNames::new().ok_or_else(|| os_error("could not create accessibility attribute names"))?;

    unsafe {
        let app = CfOwned::new(AXUIElementCreateApplication(entry.pid))
            .ok_or(PlatformError::WindowGone)?;
        // A hung app must not hang the engine thread for the default 6 seconds.
        let _ = AXUIElementSetMessagingTimeout(app.ptr(), 1.0);
        let windows = ax_copy(app.ptr(), &names.windows).map_err(ax_error)?;
        let element = find_window(&windows, id, entry.bounds, &names)?;

        if ax_bool(element, &names.minimized) == Some(true) {
            return Err(PlatformError::Refused("the window is minimized"));
        }
        if ax_bool(element, &names.full_screen) == Some(true) {
            return Err(PlatformError::Refused("the window is full screen"));
        }

        let start = ax_frame(element, &names).unwrap_or(entry.bounds);
        if animate {
            animate_move(element, &names, start, target);
        }
        set_frame(element, &names, target, true)?;

        // If the window refused the size (min/max limits), infer the limits from
        // what it did and place it again: centred in the zone, kept on screen.
        let got = ax_frame(element, &names).unwrap_or(target);
        if let Some(limits) = infer_limits(target, got, SIZE_TOLERANCE) {
            set_frame(element, &names, clamp_to_limits(target, limits, work), true)?;
        }
        Ok(ax_frame(element, &names).unwrap_or(got))
    }
}
