//! Launch at login for macOS.
//!
//! STATUS: written without a Mac or a compiler (see `mac_ffi`).
//!
//! Two mechanisms, chosen by where the executable lives:
//! - Inside an `.app` bundle on macOS 13+: `SMAppService.mainApp`
//!   (`+mainAppService`, `-registerAndReturnError:`, `-unregisterAndReturnError:`,
//!   `-status`), called through the Objective-C runtime. There is no stable
//!   `objc2-service-management` release this code could rely on without a compiler
//!   to check it against, so no binding crate is used. Every selector is guarded
//!   with `respondsToSelector:`.
//! - Otherwise (a bare binary from `cargo run`, or macOS 12 and older): a
//!   LaunchAgent property list, `~/Library/LaunchAgents/dev.krova.desktop.plist`,
//!   with `RunAtLoad`. It is written but not loaded with `launchctl`, so it takes
//!   effect at the next login and never starts a second instance now.
//!
//! `SMAppService` may report "requires approval" when the user has to allow the
//! login item in System Settings > General > Login Items; that counts as not
//! enabled here.

#![allow(dead_code, unused_unsafe, unused_imports)]

use std::fs;
use std::path::PathBuf;
use std::ptr::null_mut;

use super::mac_ffi::*;
use crate::mac_math::{app_bundle_path, launch_agent_plist, LAUNCH_AGENT_LABEL};
use crate::PlatformError;

/// `SMAppServiceStatus`.
const STATUS_NOT_REGISTERED: isize = 0;
const STATUS_ENABLED: isize = 1;
const STATUS_REQUIRES_APPROVAL: isize = 2;
const STATUS_NOT_FOUND: isize = 3;

fn io_err(e: std::io::Error) -> PlatformError {
    PlatformError::Os(e.to_string())
}

/// Is the running executable inside a `.app` bundle?
fn in_app_bundle() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.to_str().map(|s| app_bundle_path(s).is_some()))
        .unwrap_or(false)
}

fn agent_path() -> Result<PathBuf, PlatformError> {
    let home =
        std::env::var_os("HOME").ok_or_else(|| PlatformError::Os("HOME is not set".into()))?;
    Ok(PathBuf::from(home)
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{LAUNCH_AGENT_LABEL}.plist")))
}

// -------------------------------------------------------- SMAppService

/// `[SMAppService mainAppService]`, or null when the class or selector is missing.
unsafe fn main_service() -> Id {
    let class = objc_class("SMAppService");
    if class.is_null() || !responds(class, "mainAppService") {
        return null_mut();
    }
    msg_id(class, sel("mainAppService"))
}

unsafe fn sm_status() -> Option<isize> {
    let service = main_service();
    if service.is_null() || !responds(service, "status") {
        return None;
    }
    Some(msg_isize(service, sel("status")))
}

/// Call `-registerAndReturnError:` or `-unregisterAndReturnError:`.
unsafe fn sm_call(service: Id, selector: &str) -> Result<(), PlatformError> {
    if !responds(service, selector) {
        return Err(PlatformError::Unsupported);
    }
    let mut error: Id = null_mut();
    if msg_bool_err(service, sel(selector), &mut error) != 0 {
        return Ok(());
    }
    let text = if error.is_null() {
        None
    } else {
        string_from_ns(msg_id(error, sel("localizedDescription")))
    };
    Err(PlatformError::Os(text.unwrap_or_else(|| {
        "the login item could not be changed".into()
    })))
}

/// `Some(result)` when `SMAppService` handled the request, `None` when it is not
/// available and the LaunchAgent fallback should be used.
fn sm_set(enabled: bool) -> Option<Result<(), PlatformError>> {
    with_pool(|| unsafe {
        let service = main_service();
        if service.is_null() {
            return None;
        }
        let status = sm_status().unwrap_or(STATUS_NOT_FOUND);
        Some(if enabled {
            if status == STATUS_ENABLED {
                Ok(())
            } else {
                sm_call(service, "registerAndReturnError:")
            }
        } else if status == STATUS_NOT_REGISTERED || status == STATUS_NOT_FOUND {
            Ok(())
        } else {
            sm_call(service, "unregisterAndReturnError:")
        })
    })
}

// --------------------------------------------------------- LaunchAgent

fn agent_enabled() -> Result<bool, PlatformError> {
    Ok(agent_path()?.is_file())
}

fn write_agent() -> Result<(), PlatformError> {
    let exe = std::env::current_exe().map_err(io_err)?;
    let exe = exe
        .to_str()
        .ok_or_else(|| PlatformError::Os("the executable path is not valid UTF-8".into()))?;
    let path = agent_path()?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(io_err)?;
    }
    // Write next to the target and rename, so a crash never leaves half a plist.
    let tmp = path.with_extension("plist.tmp");
    fs::write(&tmp, launch_agent_plist(LAUNCH_AGENT_LABEL, exe)).map_err(io_err)?;
    fs::rename(&tmp, &path).map_err(io_err)
}

fn remove_agent() -> Result<(), PlatformError> {
    match fs::remove_file(agent_path()?) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io_err(e)),
    }
}

// ------------------------------------------------------------- public

pub fn enabled() -> Result<bool, PlatformError> {
    if in_app_bundle() {
        let status = with_pool(|| unsafe { sm_status() });
        if status == Some(STATUS_ENABLED) {
            return Ok(true);
        }
    }
    agent_enabled()
}

pub fn set(enabled: bool) -> Result<(), PlatformError> {
    if in_app_bundle() {
        if let Some(result) = sm_set(enabled) {
            result?;
            // A plist left over from an unbundled run would start a second copy.
            return remove_agent();
        }
    }
    if enabled {
        write_agent()
    } else {
        remove_agent()
    }
}
