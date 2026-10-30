//! macOS platform layer.
//!
//! STATUS: written on a Windows machine with no Mac and no compiler. NOTHING IN
//! THIS LAYER HAS BEEN COMPILED, RUN OR TESTED. The first real build is CI on
//! macos-latest; treat all of it as unverified until that run is green. Only the
//! pure logic in `mac_math` (not cfg-gated) is exercised by tests on every OS.
//!
//! Design (docs/PORTING-MAP.md, ADR-001):
//!
//! | Platform method   | Module           | How                                              |
//! |-------------------|------------------|--------------------------------------------------|
//! | `permission`      | here             | `AXIsProcessTrusted`                              |
//! | `monitors`        | `mac_screens`    | `NSScreen`, flipped to a top-left origin          |
//! | `place_window`    | `mac_ax`         | Accessibility API (`AXPosition` / `AXSize`)       |
//! | `start_input`     | `mac_input`      | listen-only `CGEventTap` + worker thread          |
//! | autostart         | `mac_autostart`  | `SMAppService.mainApp`, else a LaunchAgent plist  |
//! | `create_overlay`  | `mac_overlay`    | borderless click-through `NSWindow`               |
//!
//! There are no third-party crates. `mac_ffi` declares the C and Objective-C
//! runtime signatures by hand, so the macOS build cannot be broken by a binding
//! crate's API changing. All macOS-specific modules are declared here with
//! `#[path]`: a plain `mod x;` inside `mac.rs` would look in `src/mac/x.rs`.
//!
//! Units: Krova's "px" on macOS are points (what NSScreen, CGWindowList, CGEvent
//! and Accessibility all use), not device pixels. `MonitorInfo::scale` carries the
//! backing scale factor.
//!
//! Window ids: `WindowId` is the `CGWindowID`; see `mac_ax` for how it is turned
//! into an Accessibility element at placement time.
//!
//! Permissions: moving other apps' windows needs Accessibility; the event tap may
//! additionally need Input Monitoring. `permission()` reports Accessibility only.
//! After the user grants it in System Settings the app may have to be restarted
//! before the event tap can be created, and `start_input` is not retried by
//! itself: the caller should call it again once `permission()` turns `Granted`.
//!
//! Privacy: window titles are never read, logged or stored.

use std::process::Command;
use std::ptr::null;
use std::sync::mpsc::Sender;

use krova_core::{Modifier, PxRect};

use crate::{InputEvent, InputHandle, MonitorInfo, Permission, Platform, PlatformError, WindowId};

#[cfg(target_os = "macos")]
#[path = "mac_ffi.rs"]
mod mac_ffi;

#[cfg(target_os = "macos")]
#[path = "mac_screens.rs"]
mod mac_screens;

#[cfg(target_os = "macos")]
#[path = "mac_ax.rs"]
mod mac_ax;

#[cfg(target_os = "macos")]
#[path = "mac_input.rs"]
mod mac_input;

#[cfg(target_os = "macos")]
#[path = "mac_overlay.rs"]
mod mac_overlay;

#[cfg(target_os = "macos")]
#[path = "mac_autostart.rs"]
mod mac_autostart;

const ACCESSIBILITY_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";
const INPUT_MONITORING_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent";

// ------------------------------------------------------- permissions

/// Is Accessibility granted to this process? Never shows a prompt.
pub fn accessibility_granted() -> bool {
    unsafe { mac_ffi::AXIsProcessTrusted() != 0 }
}

/// Ask macOS to show its "Krova would like to control this computer" prompt (with
/// the button that opens System Settings) and return whether access is already
/// granted. The prompt appears at most once per app version; after that use
/// [`open_accessibility_settings`].
pub fn prompt_for_accessibility() -> bool {
    use mac_ffi::*;
    unsafe {
        let keys = [kAXTrustedCheckOptionPrompt];
        let values = [kCFBooleanTrue];
        let options = CfOwned::new(CFDictionaryCreate(
            null(),
            keys.as_ptr(),
            values.as_ptr(),
            1,
            std::ptr::addr_of!(kCFTypeDictionaryKeyCallBacks).cast(),
            std::ptr::addr_of!(kCFTypeDictionaryValueCallBacks).cast(),
        ));
        match options {
            Some(o) => AXIsProcessTrustedWithOptions(o.ptr()) != 0,
            None => AXIsProcessTrusted() != 0,
        }
    }
}

/// Open System Settings > Privacy & Security > Accessibility.
pub fn open_accessibility_settings() -> Result<(), PlatformError> {
    open_url(ACCESSIBILITY_SETTINGS_URL)
}

/// Is Input Monitoring granted (listen-only event taps)? Never prompts.
pub fn input_monitoring_granted() -> bool {
    unsafe { mac_ffi::CGPreflightListenEventAccess() }
}

/// Show the system prompt for Input Monitoring; returns whether it is granted.
pub fn request_input_monitoring() -> bool {
    unsafe { mac_ffi::CGRequestListenEventAccess() }
}

/// Open System Settings > Privacy & Security > Input Monitoring.
pub fn open_input_monitoring_settings() -> Result<(), PlatformError> {
    open_url(INPUT_MONITORING_SETTINGS_URL)
}

fn open_url(url: &str) -> Result<(), PlatformError> {
    let status = Command::new("/usr/bin/open")
        .arg(url)
        .status()
        .map_err(|e| PlatformError::Os(e.to_string()))?;
    if status.success() {
        Ok(())
    } else {
        Err(PlatformError::Os(format!("open exited with {status}")))
    }
}

// ----------------------------------------------------------- platform

#[derive(Debug, Default, Clone, Copy)]
pub struct MacPlatform;

impl MacPlatform {
    pub fn new() -> Self {
        Self
    }
}

impl Platform for MacPlatform {
    fn monitors(&self) -> Result<Vec<MonitorInfo>, PlatformError> {
        mac_screens::monitors()
    }

    fn place_window(
        &self,
        window: WindowId,
        target: PxRect,
        work: PxRect,
        animate: bool,
    ) -> Result<PxRect, PlatformError> {
        mac_ax::place_window(window, target, work, animate)
    }

    fn start_input(
        &self,
        modifier: Modifier,
        sink: Sender<InputEvent>,
    ) -> Result<InputHandle, PlatformError> {
        mac_input::start(modifier, sink)
    }

    fn autostart_enabled(&self) -> Result<bool, PlatformError> {
        mac_autostart::enabled()
    }

    fn set_autostart(&self, enabled: bool) -> Result<(), PlatformError> {
        mac_autostart::set(enabled)
    }

    fn create_overlay(&self) -> Result<Box<dyn crate::overlay::Overlay>, PlatformError> {
        Ok(Box::new(mac_overlay::MacOverlay::create()?))
    }

    fn permission(&self) -> Permission {
        if accessibility_granted() {
            Permission::Granted
        } else {
            Permission::Denied
        }
    }
}
