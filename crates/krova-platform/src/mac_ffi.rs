//! Hand-written declarations of the system APIs the macOS layer calls.
//!
//! STATUS: written without a Mac or a compiler; the first real build is CI on
//! macos-latest. Treat every signature below as unverified until that run is green.
//!
//! There is deliberately no binding crate: each declaration comes from the public
//! C headers (CoreFoundation, CoreGraphics, ApplicationServices, libdispatch) or
//! the Objective-C runtime, so no third-party API can drift under us. The two
//! private/undocumented symbols (`_AXUIElementGetWindow`,
//! `CGDisplayCreateUUIDFromDisplayID`) are resolved with `dlsym` at run time so a
//! missing export degrades a feature instead of breaking the link.
//!
//! Objective-C messages go through `objc_msgSend`, declared once per argument /
//! return shape with `#[link_name]` (hence `clashing_extern_declarations`).
//! `objc_msgSend_stret` is used for 32-byte struct returns on x86_64 only.

// Loosened on purpose until the first green macOS CI run; tighten afterwards.
#![allow(
    non_snake_case,
    non_upper_case_globals,
    non_camel_case_types,
    dead_code,
    unused_unsafe,
    unused_imports,
    improper_ctypes,
    clashing_extern_declarations,
    clippy::missing_safety_doc,
    clippy::too_many_arguments,
    clippy::new_without_default
)]

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr::{addr_of, null, null_mut};
use std::sync::mpsc;
use std::time::Duration;

use crate::PlatformError;

// ---------------------------------------------------------------- types

pub type CFTypeRef = *const c_void;
pub type CFStringRef = *const c_void;
pub type CFIndex = isize;
/// C `Boolean` (unsigned char), as returned by CoreFoundation / HIServices.
pub type Boolean = u8;
pub type AXError = i32;
pub type AXUIElementRef = *const c_void;
/// Objective-C `id` / `Class`.
pub type Id = *mut c_void;
/// Objective-C `SEL`.
pub type Sel = *const c_void;
/// `CGEventTapCallBack`.
pub type TapCallback =
    unsafe extern "C" fn(*mut c_void, u32, *const c_void, *mut c_void) -> *const c_void;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CGPoint {
    pub x: f64,
    pub y: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CGSize {
    pub width: f64,
    pub height: f64,
}

/// Also `NSRect` on 64-bit macOS.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CGRect {
    pub origin: CGPoint,
    pub size: CGSize,
}

impl CGRect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self {
            origin: CGPoint { x, y },
            size: CGSize {
                width: w,
                height: h,
            },
        }
    }
}

// ------------------------------------------------------------ constants

/// `kCFStringEncodingUTF8`.
pub const ENCODING_UTF8: u32 = 0x0800_0100;
/// `kCFNumberSInt32Type`, `kCFNumberDoubleType`.
pub const NUMBER_SINT32: CFIndex = 3;
pub const NUMBER_DOUBLE: CFIndex = 13;
/// `kAXValueCGPointType`, `kAXValueCGSizeType`.
pub const AX_VALUE_CGPOINT: u32 = 1;
pub const AX_VALUE_CGSIZE: u32 = 2;

/// `CGEventType`.
pub const EVENT_LEFT_MOUSE_DOWN: u32 = 1;
pub const EVENT_LEFT_MOUSE_UP: u32 = 2;
pub const EVENT_LEFT_MOUSE_DRAGGED: u32 = 6;
pub const EVENT_TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;
pub const EVENT_TAP_DISABLED_BY_USER_INPUT: u32 = 0xFFFF_FFFF;
/// `kCGSessionEventTap`, `kCGHeadInsertEventTap`, `kCGEventTapOptionListenOnly`.
pub const TAP_SESSION: u32 = 1;
pub const TAP_HEAD_INSERT: u32 = 0;
pub const TAP_LISTEN_ONLY: u32 = 1;
/// `kCGEventSourceStateCombinedSessionState`.
pub const EVENT_SOURCE_COMBINED_SESSION: i32 = 0;

/// `CGWindowListOption` bits.
pub const LIST_ON_SCREEN_ONLY: u32 = 1;
pub const LIST_INCLUDING_WINDOW: u32 = 8;
pub const LIST_EXCLUDE_DESKTOP: u32 = 16;

// ----------------------------------------------------------- frameworks

// One block per framework: the link attributes are identical in effect to stacking
// them on a single block, but clippy flags stacked `#[link(kind = ..)]` as duplicates.
#[link(name = "AppKit", kind = "framework")]
extern "C" {}

#[link(name = "Foundation", kind = "framework")]
extern "C" {}

#[link(name = "ServiceManagement", kind = "framework")]
extern "C" {}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    pub static kCFRunLoopCommonModes: CFStringRef;
    pub static kCFRunLoopDefaultMode: CFStringRef;
    pub static kCFBooleanTrue: CFTypeRef;
    // Only their addresses are used; the real types are structs.
    pub static kCFTypeDictionaryKeyCallBacks: u8;
    pub static kCFTypeDictionaryValueCallBacks: u8;

    pub fn CFRelease(cf: CFTypeRef);
    pub fn CFRetain(cf: CFTypeRef) -> CFTypeRef;
    pub fn CFGetTypeID(cf: CFTypeRef) -> usize;
    pub fn CFNumberGetTypeID() -> usize;
    pub fn CFBooleanGetTypeID() -> usize;
    pub fn CFNumberGetValue(
        number: CFTypeRef,
        the_type: CFIndex,
        value_ptr: *mut c_void,
    ) -> Boolean;
    pub fn CFBooleanGetValue(boolean: CFTypeRef) -> Boolean;
    pub fn CFArrayGetCount(array: CFTypeRef) -> CFIndex;
    pub fn CFArrayGetValueAtIndex(array: CFTypeRef, index: CFIndex) -> *const c_void;
    pub fn CFDictionaryGetValue(dict: CFTypeRef, key: *const c_void) -> *const c_void;
    pub fn CFDictionaryCreate(
        allocator: *const c_void,
        keys: *const *const c_void,
        values: *const *const c_void,
        count: CFIndex,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> CFTypeRef;
    pub fn CFStringCreateWithBytes(
        allocator: *const c_void,
        bytes: *const u8,
        num_bytes: CFIndex,
        encoding: u32,
        is_external_representation: Boolean,
    ) -> CFStringRef;
    pub fn CFStringGetCString(
        string: CFStringRef,
        buffer: *mut c_char,
        buffer_size: CFIndex,
        encoding: u32,
    ) -> Boolean;
    pub fn CFUUIDCreateString(allocator: *const c_void, uuid: *const c_void) -> CFStringRef;
    pub fn CFDataCreate(allocator: *const c_void, bytes: *const u8, length: CFIndex) -> CFTypeRef;
    pub fn CFRunLoopGetCurrent() -> *const c_void;
    pub fn CFRunLoopAddSource(run_loop: *const c_void, source: *const c_void, mode: CFStringRef);
    pub fn CFRunLoopRemoveSource(run_loop: *const c_void, source: *const c_void, mode: CFStringRef);
    pub fn CFRunLoopRunInMode(
        mode: CFStringRef,
        seconds: f64,
        return_after_source_handled: Boolean,
    ) -> i32;
    pub fn CFRunLoopStop(run_loop: *const c_void);
    pub fn CFMachPortCreateRunLoopSource(
        allocator: *const c_void,
        port: *const c_void,
        order: CFIndex,
    ) -> *const c_void;
    pub fn CFMachPortInvalidate(port: *const c_void);
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    pub static kCGWindowNumber: CFStringRef;
    pub static kCGWindowOwnerPID: CFStringRef;
    pub static kCGWindowLayer: CFStringRef;
    pub static kCGWindowBounds: CFStringRef;
    pub static kCGWindowAlpha: CFStringRef;

    pub fn CGWindowListCopyWindowInfo(option: u32, relative_to_window: u32) -> CFTypeRef;
    pub fn CGRectMakeWithDictionaryRepresentation(dict: CFTypeRef, rect: *mut CGRect) -> bool;

    pub fn CGDisplayVendorNumber(display: u32) -> u32;
    pub fn CGDisplayModelNumber(display: u32) -> u32;
    pub fn CGDisplaySerialNumber(display: u32) -> u32;

    pub fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: TapCallback,
        user_info: *mut c_void,
    ) -> *const c_void;
    pub fn CGEventTapEnable(tap: *const c_void, enable: bool);
    pub fn CGEventGetLocation(event: *const c_void) -> CGPoint;
    pub fn CGEventSourceFlagsState(state_id: i32) -> u64;
    pub fn CGEventSourceKeyState(state_id: i32, key: u16) -> bool;
    pub fn CGPreflightListenEventAccess() -> bool;
    pub fn CGRequestListenEventAccess() -> bool;

    pub fn CGColorSpaceCreateDeviceRGB() -> CFTypeRef;
    pub fn CGDataProviderCreateWithCFData(data: CFTypeRef) -> CFTypeRef;
    pub fn CGImageCreate(
        width: usize,
        height: usize,
        bits_per_component: usize,
        bits_per_pixel: usize,
        bytes_per_row: usize,
        space: CFTypeRef,
        bitmap_info: u32,
        provider: CFTypeRef,
        decode: *const f64,
        should_interpolate: bool,
        intent: i32,
    ) -> CFTypeRef;
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    pub static kAXTrustedCheckOptionPrompt: CFStringRef;

    pub fn AXIsProcessTrusted() -> Boolean;
    pub fn AXIsProcessTrustedWithOptions(options: CFTypeRef) -> Boolean;
    pub fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    pub fn AXUIElementSetMessagingTimeout(element: AXUIElementRef, seconds: f32) -> AXError;
    pub fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    pub fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> AXError;
    pub fn AXValueCreate(value_type: u32, value: *const c_void) -> CFTypeRef;
    pub fn AXValueGetValue(value: CFTypeRef, value_type: u32, out: *mut c_void) -> Boolean;
}

// libSystem (no link attribute needed).
extern "C" {
    pub fn pthread_main_np() -> c_int;
    pub fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    pub fn dispatch_async_f(
        queue: *mut c_void,
        context: *mut c_void,
        work: extern "C" fn(*mut c_void),
    );
    // `dispatch_get_main_queue()` is `&_dispatch_main_q`; only the address is used.
    pub static _dispatch_main_q: u8;
}

// ------------------------------------------------------- objective-c

#[link(name = "objc")]
extern "C" {
    pub fn objc_getClass(name: *const c_char) -> Id;
    pub fn sel_registerName(name: *const c_char) -> Sel;
    pub fn objc_autoreleasePoolPush() -> *mut c_void;
    pub fn objc_autoreleasePoolPop(pool: *mut c_void);
}

// One declaration per message shape. All of them are the same symbol.
extern "C" {
    #[link_name = "objc_msgSend"]
    pub fn msg_id(obj: Id, sel: Sel) -> Id;
    #[link_name = "objc_msgSend"]
    pub fn msg_void(obj: Id, sel: Sel);
    #[link_name = "objc_msgSend"]
    pub fn msg_usize(obj: Id, sel: Sel) -> usize;
    #[link_name = "objc_msgSend"]
    pub fn msg_isize(obj: Id, sel: Sel) -> isize;
    #[link_name = "objc_msgSend"]
    pub fn msg_u32(obj: Id, sel: Sel) -> u32;
    #[link_name = "objc_msgSend"]
    pub fn msg_f64(obj: Id, sel: Sel) -> f64;
    /// `BOOL` result.
    #[link_name = "objc_msgSend"]
    pub fn msg_bool(obj: Id, sel: Sel) -> u8;
    #[link_name = "objc_msgSend"]
    pub fn msg_id_arg(obj: Id, sel: Sel, a: Id) -> Id;
    #[link_name = "objc_msgSend"]
    pub fn msg_id_index(obj: Id, sel: Sel, index: usize) -> Id;
    /// `-respondsToSelector:`
    #[link_name = "objc_msgSend"]
    pub fn msg_bool_sel(obj: Id, sel: Sel, a: Sel) -> u8;
    /// `-registerAndReturnError:` and friends: `BOOL` result, `NSError **` argument.
    #[link_name = "objc_msgSend"]
    pub fn msg_bool_err(obj: Id, sel: Sel, error: *mut Id) -> u8;
    /// `-setFoo:(BOOL)`
    #[link_name = "objc_msgSend"]
    pub fn msg_set_bool(obj: Id, sel: Sel, value: u8);
    #[link_name = "objc_msgSend"]
    pub fn msg_set_id(obj: Id, sel: Sel, value: Id);
    #[link_name = "objc_msgSend"]
    pub fn msg_set_isize(obj: Id, sel: Sel, value: isize);
    #[link_name = "objc_msgSend"]
    pub fn msg_set_usize(obj: Id, sel: Sel, value: usize);
    #[link_name = "objc_msgSend"]
    pub fn msg_set_rect(obj: Id, sel: Sel, rect: CGRect);
    /// `-setFrame:display:`
    #[link_name = "objc_msgSend"]
    pub fn msg_set_rect_flag(obj: Id, sel: Sel, rect: CGRect, flag: u8);
    /// `-initWithContentRect:styleMask:backing:defer:`
    #[link_name = "objc_msgSend"]
    pub fn msg_init_window(
        obj: Id,
        sel: Sel,
        rect: CGRect,
        style: usize,
        backing: usize,
        defer: u8,
    ) -> Id;
}

// A 32-byte struct comes back in registers (d0-d3) on arm64, but through a hidden
// pointer on x86_64, which needs the _stret entry point.
#[cfg(target_arch = "aarch64")]
extern "C" {
    #[link_name = "objc_msgSend"]
    fn msg_rect_raw(obj: Id, sel: Sel) -> CGRect;
}

#[cfg(target_arch = "x86_64")]
extern "C" {
    #[link_name = "objc_msgSend_stret"]
    fn msg_rect_stret(out: *mut CGRect, obj: Id, sel: Sel);
}

/// Send a message that returns an `NSRect` / `CGRect`.
#[cfg(target_arch = "aarch64")]
pub unsafe fn msg_rect(obj: Id, sel: Sel) -> CGRect {
    msg_rect_raw(obj, sel)
}

/// Send a message that returns an `NSRect` / `CGRect`.
#[cfg(target_arch = "x86_64")]
pub unsafe fn msg_rect(obj: Id, sel: Sel) -> CGRect {
    let mut out = CGRect::default();
    msg_rect_stret(&mut out, obj, sel);
    out
}

// -------------------------------------------------------------- helpers

fn c_name(name: &str) -> Vec<u8> {
    let mut buf = Vec::with_capacity(name.len() + 1);
    buf.extend_from_slice(name.as_bytes());
    buf.push(0);
    buf
}

/// Register / look up a selector by name.
pub fn sel(name: &str) -> Sel {
    let buf = c_name(name);
    unsafe { sel_registerName(buf.as_ptr().cast()) }
}

/// Look up an Objective-C class by name; null when it does not exist (for
/// example `SMAppService` before macOS 13).
pub fn objc_class(name: &str) -> Id {
    let buf = c_name(name);
    unsafe { objc_getClass(buf.as_ptr().cast()) }
}

/// `-respondsToSelector:`. Used before every call whose selector spelling is not
/// certain, so a wrong guess degrades a feature instead of raising an
/// uncatchable Objective-C exception.
pub unsafe fn responds(obj: Id, name: &str) -> bool {
    !obj.is_null() && msg_bool_sel(obj, sel("respondsToSelector:"), sel(name)) != 0
}

/// Autoreleased `NSString` from UTF-8 (null if `s` contains a NUL byte).
pub fn ns_string(s: &str) -> Id {
    let Ok(c) = CString::new(s) else {
        return null_mut();
    };
    unsafe {
        msg_id_arg(
            objc_class("NSString"),
            sel("stringWithUTF8String:"),
            c.as_ptr().cast::<c_void>().cast_mut(),
        )
    }
}

/// Copy an `NSString` into a Rust string. Only ever used for display names and
/// error descriptions, never for window titles.
pub unsafe fn string_from_ns(s: Id) -> Option<String> {
    if s.is_null() {
        return None;
    }
    let p = msg_id(s, sel("UTF8String")) as *const c_char;
    if p.is_null() {
        return None;
    }
    Some(CStr::from_ptr(p).to_string_lossy().into_owned())
}

/// Look a symbol up in every loaded image.
pub fn dlsym_default(name: &str) -> *mut c_void {
    // RTLD_DEFAULT on macOS is ((void *) -2).
    let rtld_default = -2isize as *mut c_void;
    let buf = c_name(name);
    unsafe { dlsym(rtld_default, buf.as_ptr().cast()) }
}

// ------------------------------------------------------ core foundation

/// An owned (+1) CoreFoundation object, released on drop.
pub struct CfOwned(CFTypeRef);

impl CfOwned {
    /// Take ownership of a "Create"/"Copy" result; `None` for null.
    pub fn new(p: CFTypeRef) -> Option<Self> {
        if p.is_null() {
            None
        } else {
            Some(Self(p))
        }
    }

    pub fn ptr(&self) -> CFTypeRef {
        self.0
    }
}

impl Drop for CfOwned {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0) };
    }
}

pub fn cf_string(s: &str) -> Option<CfOwned> {
    let p = unsafe {
        CFStringCreateWithBytes(null(), s.as_ptr(), s.len() as CFIndex, ENCODING_UTF8, 0)
    };
    CfOwned::new(p)
}

pub unsafe fn cf_string_to_rust(s: CFStringRef) -> Option<String> {
    let mut buf: [c_char; 256] = [0; 256];
    if CFStringGetCString(s, buf.as_mut_ptr(), 256, ENCODING_UTF8) == 0 {
        return None;
    }
    Some(CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned())
}

pub unsafe fn cf_i32(v: CFTypeRef) -> Option<i32> {
    if v.is_null() || CFGetTypeID(v) != CFNumberGetTypeID() {
        return None;
    }
    let mut out: i32 = 0;
    let ok = CFNumberGetValue(v, NUMBER_SINT32, (&mut out as *mut i32).cast::<c_void>());
    (ok != 0).then_some(out)
}

pub unsafe fn cf_f64(v: CFTypeRef) -> Option<f64> {
    if v.is_null() || CFGetTypeID(v) != CFNumberGetTypeID() {
        return None;
    }
    let mut out: f64 = 0.0;
    let ok = CFNumberGetValue(v, NUMBER_DOUBLE, (&mut out as *mut f64).cast::<c_void>());
    (ok != 0).then_some(out)
}

pub unsafe fn cf_bool(v: CFTypeRef) -> Option<bool> {
    if v.is_null() || CFGetTypeID(v) != CFBooleanGetTypeID() {
        return None;
    }
    Some(CFBooleanGetValue(v) != 0)
}

pub unsafe fn dict_i32(dict: CFTypeRef, key: CFStringRef) -> Option<i32> {
    cf_i32(CFDictionaryGetValue(dict, key))
}

pub unsafe fn dict_f64(dict: CFTypeRef, key: CFStringRef) -> Option<f64> {
    cf_f64(CFDictionaryGetValue(dict, key))
}

// ------------------------------------------------- threads and run loops

/// An autorelease pool for the current scope.
pub struct Pool(*mut c_void);

impl Pool {
    pub fn new() -> Self {
        Self(unsafe { objc_autoreleasePoolPush() })
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        unsafe { objc_autoreleasePoolPop(self.0) };
    }
}

/// Run `f` inside its own autorelease pool.
pub fn with_pool<T>(f: impl FnOnce() -> T) -> T {
    let _pool = Pool::new();
    f()
}

pub fn on_main_thread() -> bool {
    unsafe { pthread_main_np() != 0 }
}

/// Queue `job` on the main dispatch queue and return immediately.
pub fn dispatch_main(job: Box<dyn FnOnce() + Send>) {
    extern "C" fn trampoline(context: *mut c_void) {
        // SAFETY: `context` is the pointer produced by `Box::into_raw` below and
        // libdispatch hands it to us exactly once.
        let job = unsafe { Box::from_raw(context.cast::<Box<dyn FnOnce() + Send>>()) };
        // A panic must not unwind into libdispatch.
        let _ = catch_unwind(AssertUnwindSafe(move || {
            let job: Box<dyn FnOnce() + Send> = *job;
            with_pool(job);
        }));
    }

    let context = Box::into_raw(Box::new(job)).cast::<c_void>();
    unsafe {
        let queue = addr_of!(_dispatch_main_q).cast::<c_void>().cast_mut();
        dispatch_async_f(queue, context, trampoline);
    }
}

/// Run `f` on the main thread and wait for its result. AppKit objects
/// (`NSScreen`, `NSWindow`, ...) may only be used there. When already on the main
/// thread `f` runs inline, so this never deadlocks against itself. When the main
/// thread does not get to it within a few seconds (no run loop, for example in a
/// test binary) an error is returned instead of hanging forever.
pub fn run_on_main<T: Send + 'static>(
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, PlatformError> {
    if on_main_thread() {
        return Ok(with_pool(f));
    }
    let (tx, rx) = mpsc::channel::<T>();
    dispatch_main(Box::new(move || {
        let value = f();
        let _ = tx.send(value);
    }));
    rx.recv_timeout(Duration::from_secs(3))
        .map_err(|_| PlatformError::Os("the main thread did not respond".into()))
}
