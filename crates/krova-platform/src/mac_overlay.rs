//! Drag-highlight overlay for macOS: one borderless, transparent, click-through
//! `NSWindow` at status-bar level that shows the bitmap `overlay::render` makes.
//!
//! STATUS: written without a Mac or a compiler (see `mac_ffi`). Of the six macOS
//! features this is the one with the most message sends that were never run.
//!
//! Threading: AppKit objects live on the main thread only. `set_scene` can be
//! called from any thread; it rasterises the scene on the caller's thread (pure
//! Rust), stores the newest frame and queues at most one main-queue job to apply
//! it, so a burst of updates collapses into a single repaint. Building and
//! destroying the window also run on the main thread (`run_on_main`).
//!
//! Drawing: the content view is layer-backed and gets one sublayer whose
//! `contents` is a `CGImage` of the premultiplied BGRA bitmap. The bitmap has one
//! pixel per point (Krova's px are points), so it is scaled up on Retina screens;
//! the "nearest" magnification filter keeps the edges crisp. Implicit CALayer
//! animations are switched off for the update (no cross-fade).

#![allow(dead_code, unused_unsafe, unused_imports)]

use std::ffi::c_void;
use std::ptr::{null, null_mut};
use std::sync::{Arc, Mutex};

use krova_core::PxRect;

use super::mac_ffi::*;
use crate::mac_math::{
    unflip_rect, BITMAP_INFO_BGRA_PREMULTIPLIED, OVERLAY_COLLECTION_BEHAVIOR, OVERLAY_WINDOW_LEVEL,
};
use crate::overlay::{render, Overlay, OverlayScene};
use crate::PlatformError;

/// `NSBackingStoreBuffered`; `NSWindowStyleMaskBorderless` is 0.
const BACKING_BUFFERED: usize = 2;

/// A rasterised scene waiting to be shown.
struct Frame {
    rect: PxRect,
    pixels: Vec<u32>,
}

#[derive(Default)]
enum Pending {
    #[default]
    Nothing,
    Hide,
    Show(Frame),
}

#[derive(Default)]
struct State {
    /// The scene last requested; identical requests are dropped.
    requested: Option<OverlayScene>,
    pending: Pending,
    /// A main-queue job is already on its way.
    queued: bool,
    /// The window has been destroyed.
    closed: bool,
}

struct Inner {
    /// `NSWindow *` and the `CALayer *` that carries the bitmap, as integers so
    /// the struct is Send + Sync. They are only dereferenced on the main thread.
    window: usize,
    layer: usize,
    state: Mutex<State>,
}

pub struct MacOverlay {
    inner: Arc<Inner>,
}

impl MacOverlay {
    /// Create the (initially hidden) overlay window.
    pub fn create() -> Result<Self, PlatformError> {
        let built = run_on_main(|| unsafe { build_window() })?;
        let (window, layer) = built?;
        Ok(Self {
            inner: Arc::new(Inner {
                window,
                layer,
                state: Mutex::new(State::default()),
            }),
        })
    }
}

impl Overlay for MacOverlay {
    fn set_scene(&self, scene: Option<OverlayScene>) {
        if let Ok(st) = self.inner.state.lock() {
            if st.requested == scene {
                return;
            }
        }
        // Rasterise outside the lock; the main thread only needs it to swap frames.
        let pending = match &scene {
            Some(s) => Pending::Show(Frame {
                rect: s.monitor,
                pixels: render(s),
            }),
            None => Pending::Hide,
        };
        let queue_job = {
            let Ok(mut st) = self.inner.state.lock() else {
                return;
            };
            if st.closed {
                return;
            }
            st.requested = scene;
            st.pending = pending;
            !std::mem::replace(&mut st.queued, true)
        };
        if queue_job {
            let inner = Arc::clone(&self.inner);
            dispatch_main(Box::new(move || flush(&inner)));
        }
    }
}

impl Drop for MacOverlay {
    fn drop(&mut self) {
        let inner = Arc::clone(&self.inner);
        let job = move || unsafe { destroy(&inner) };
        if on_main_thread() {
            with_pool(job);
        } else {
            dispatch_main(Box::new(job));
        }
    }
}

/// Main thread: show or hide whatever was requested last.
fn flush(inner: &Inner) {
    let pending = {
        let Ok(mut st) = inner.state.lock() else {
            return;
        };
        st.queued = false;
        if st.closed {
            return;
        }
        std::mem::take(&mut st.pending)
    };
    unsafe {
        match pending {
            Pending::Nothing => {}
            Pending::Hide => hide(inner),
            Pending::Show(frame) => show(inner, &frame),
        }
    }
}

// ------------------------------------------------------------- AppKit

/// Main thread: create the window and its bitmap layer.
unsafe fn build_window() -> Result<(usize, usize), PlatformError> {
    let class = objc_class("NSWindow");
    if class.is_null() {
        return Err(PlatformError::Os("NSWindow is not available".into()));
    }
    let blank = CGRect::new(0.0, 0.0, 1.0, 1.0);
    let window = msg_init_window(
        msg_id(class, sel("alloc")),
        sel("initWithContentRect:styleMask:backing:defer:"),
        blank,
        0,
        BACKING_BUFFERED,
        0,
    );
    if window.is_null() {
        return Err(PlatformError::Os(
            "could not create the overlay window".into(),
        ));
    }

    // Transparent, shadowless, click-through, on every Space, above full screen.
    msg_set_bool(window, sel("setReleasedWhenClosed:"), 0);
    msg_set_bool(window, sel("setOpaque:"), 0);
    msg_set_id(
        window,
        sel("setBackgroundColor:"),
        msg_id(objc_class("NSColor"), sel("clearColor")),
    );
    msg_set_bool(window, sel("setHasShadow:"), 0);
    msg_set_bool(window, sel("setIgnoresMouseEvents:"), 1);
    msg_set_bool(window, sel("setHidesOnDeactivate:"), 0);
    msg_set_isize(window, sel("setLevel:"), OVERLAY_WINDOW_LEVEL);
    msg_set_usize(
        window,
        sel("setCollectionBehavior:"),
        OVERLAY_COLLECTION_BEHAVIOR,
    );

    let view = msg_id(window, sel("contentView"));
    if view.is_null() {
        return Err(PlatformError::Os(
            "the overlay window has no content view".into(),
        ));
    }
    msg_set_bool(view, sel("setWantsLayer:"), 1);
    let root = msg_id(view, sel("layer"));
    let layer = msg_id(objc_class("CALayer"), sel("layer"));
    if root.is_null() || layer.is_null() {
        return Err(PlatformError::Os(
            "could not create the overlay layer".into(),
        ));
    }
    msg_set_id(layer, sel("setMagnificationFilter:"), ns_string("nearest"));
    // The root layer retains the sublayer, which therefore lives as long as the window.
    msg_set_id(root, sel("addSublayer:"), layer);
    Ok((window as usize, layer as usize))
}

unsafe fn hide(inner: &Inner) {
    msg_set_id(inner.window as Id, sel("orderOut:"), null_mut());
}

unsafe fn show(inner: &Inner, frame: &Frame) {
    let (w, h) = (frame.rect.w, frame.rect.h);
    let expected = usize::try_from(w).unwrap_or(0) * usize::try_from(h).unwrap_or(0);
    if w <= 0 || h <= 0 || frame.pixels.len() != expected {
        hide(inner);
        return;
    }
    let Some(image) = make_image(w, h, &frame.pixels) else {
        hide(inner);
        return;
    };

    // The scene is in Krova's top-left space; AppKit wants bottom-left.
    let (x, y, fw, fh) = unflip_rect(frame.rect, super::mac_screens::primary_height());
    let window = inner.window as Id;
    msg_set_rect_flag(
        window,
        sel("setFrame:display:"),
        CGRect::new(x, y, fw, fh),
        1,
    );

    let transaction = objc_class("CATransaction");
    msg_void(transaction, sel("begin"));
    msg_set_bool(transaction, sel("setDisableActions:"), 1);
    let layer = inner.layer as Id;
    msg_set_rect(layer, sel("setFrame:"), CGRect::new(0.0, 0.0, fw, fh));
    msg_set_id(layer, sel("setContents:"), image.ptr().cast_mut());
    msg_void(transaction, sel("commit"));

    msg_void(window, sel("orderFrontRegardless"));
}

/// Main thread: take the window down for good.
unsafe fn destroy(inner: &Inner) {
    if let Ok(mut st) = inner.state.lock() {
        st.closed = true;
    }
    let window = inner.window as Id;
    msg_set_id(window, sel("orderOut:"), null_mut());
    msg_void(window, sel("close"));
    msg_void(window, sel("release"));
}

/// A `CGImage` over a copy of the premultiplied `0xAARRGGBB` pixels.
unsafe fn make_image(w: i32, h: i32, pixels: &[u32]) -> Option<CfOwned> {
    let (width, height) = (usize::try_from(w).ok()?, usize::try_from(h).ok()?);
    let data = CfOwned::new(CFDataCreate(
        null(),
        pixels.as_ptr().cast::<u8>(),
        (pixels.len() * 4) as CFIndex,
    ))?;
    let provider = CfOwned::new(CGDataProviderCreateWithCFData(data.ptr()))?;
    let space = CfOwned::new(CGColorSpaceCreateDeviceRGB())?;
    CfOwned::new(CGImageCreate(
        width,
        height,
        8,
        32,
        width * 4,
        space.ptr(),
        BITMAP_INFO_BGRA_PREMULTIPLIED,
        provider.ptr(),
        null(),
        false,
        0,
    ))
}
