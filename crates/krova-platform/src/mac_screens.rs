//! `Platform::monitors` for macOS: `NSScreen.screens` plus CoreGraphics ids.
//!
//! STATUS: written without a Mac or a compiler (see `mac_ffi`).
//!
//! AppKit uses a bottom-left origin whose zero is the bottom-left corner of the
//! primary screen (`screens[0]`, the one with the menu bar). Krova uses a top-left
//! virtual screen, so every frame is flipped against the primary screen's height
//! (`mac_math::flip_rect`). Everything is in points: Krova's px on macOS are
//! points, and `scale` (`backingScaleFactor`) says how many device pixels one
//! point is.
//!
//! `NSScreen` is only used on the main thread (`run_on_main`).

#![allow(dead_code, unused_unsafe, unused_imports)]

use std::ffi::c_void;
use std::ptr::null;
use std::sync::OnceLock;

use krova_core::MonitorDescriptor;

use super::mac_ffi::*;
use crate::mac_math::{display_hardware_id, flip_rect};
use crate::{MonitorInfo, PlatformError};

/// Currently connected monitors, primary first (AppKit order).
pub fn monitors() -> Result<Vec<MonitorInfo>, PlatformError> {
    run_on_main(|| unsafe { collect() })
}

/// Height in points of the primary screen. Main thread only. 0.0 if unknown.
pub unsafe fn primary_height() -> f64 {
    let screens = msg_id(objc_class("NSScreen"), sel("screens"));
    if screens.is_null() || msg_usize(screens, sel("count")) == 0 {
        return 0.0;
    }
    let first = msg_id_index(screens, sel("objectAtIndex:"), 0);
    if first.is_null() {
        return 0.0;
    }
    msg_rect(first, sel("frame")).size.height
}

struct RawScreen {
    frame: CGRect,
    visible: CGRect,
    scale: f64,
    display_id: Option<u32>,
    name: Option<String>,
}

unsafe fn collect() -> Vec<MonitorInfo> {
    let screens = msg_id(objc_class("NSScreen"), sel("screens"));
    if screens.is_null() {
        return Vec::new();
    }
    let count = msg_usize(screens, sel("count"));
    let mut raws = Vec::with_capacity(count);
    for i in 0..count {
        let screen = msg_id_index(screens, sel("objectAtIndex:"), i);
        if !screen.is_null() {
            raws.push(read_screen(screen));
        }
    }
    let Some(primary_h) = raws.first().map(|r| r.frame.size.height) else {
        return Vec::new();
    };

    raws.iter()
        .enumerate()
        .map(|(i, r)| {
            let (hardware_id, device_path) = match r.display_id {
                Some(id) => (
                    display_hardware_id(
                        CGDisplayVendorNumber(id),
                        CGDisplayModelNumber(id),
                        CGDisplaySerialNumber(id),
                    ),
                    display_uuid(id),
                ),
                None => (None, None),
            };
            let f = r.frame;
            let v = r.visible;
            MonitorInfo {
                descriptor: MonitorDescriptor {
                    hardware_id,
                    device_path,
                    rect: flip_rect(
                        f.origin.x,
                        f.origin.y,
                        f.size.width,
                        f.size.height,
                        primary_h,
                    ),
                },
                work: flip_rect(
                    v.origin.x,
                    v.origin.y,
                    v.size.width,
                    v.size.height,
                    primary_h,
                ),
                scale: if r.scale.is_finite() && r.scale >= 1.0 {
                    r.scale
                } else {
                    1.0
                },
                label: r
                    .name
                    .clone()
                    .filter(|n| !n.trim().is_empty())
                    .unwrap_or_else(|| format!("Display {}", i + 1)),
                primary: i == 0,
            }
        })
        .collect()
}

unsafe fn read_screen(screen: Id) -> RawScreen {
    let frame = msg_rect(screen, sel("frame"));
    let visible = msg_rect(screen, sel("visibleFrame"));
    let scale = msg_f64(screen, sel("backingScaleFactor"));
    // `localizedName` is the display's product name ("LG UltraFine"), not a
    // window title. It exists from macOS 10.15; guard anyway.
    let name = if responds(screen, "localizedName") {
        string_from_ns(msg_id(screen, sel("localizedName")))
    } else {
        None
    };
    RawScreen {
        frame,
        visible,
        scale,
        display_id: display_id(screen),
        name,
    }
}

/// `CGDirectDisplayID` of a screen: `deviceDescription["NSScreenNumber"]`.
unsafe fn display_id(screen: Id) -> Option<u32> {
    let description = msg_id(screen, sel("deviceDescription"));
    if description.is_null() {
        return None;
    }
    let number = msg_id_arg(
        description,
        sel("objectForKey:"),
        ns_string("NSScreenNumber"),
    );
    if number.is_null() {
        return None;
    }
    Some(msg_u32(number, sel("unsignedIntValue")))
}

type DisplayUuidFn = unsafe extern "C" fn(u32) -> *const c_void;

/// `CGDisplayCreateUUIDFromDisplayID` lives in ColorSync (re-exported by
/// ApplicationServices); resolve it at run time instead of risking the link.
fn display_uuid_fn() -> Option<DisplayUuidFn> {
    static ADDR: OnceLock<usize> = OnceLock::new();
    let addr = *ADDR.get_or_init(|| dlsym_default("CGDisplayCreateUUIDFromDisplayID") as usize);
    if addr == 0 {
        None
    } else {
        // SAFETY: the symbol has this C signature: CFUUIDRef f(uint32_t).
        Some(unsafe { std::mem::transmute::<usize, DisplayUuidFn>(addr) })
    }
}

/// The display's UUID string (stable per display, survives reconnects).
unsafe fn display_uuid(id: u32) -> Option<String> {
    let make = display_uuid_fn()?;
    let uuid = CfOwned::new(make(id))?;
    let text = CfOwned::new(CFUUIDCreateString(null(), uuid.ptr()))?;
    cf_string_to_rust(text.ptr())
}
