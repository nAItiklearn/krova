//! Hot corners in the app: start the platform watcher, keep it in step with the
//! settings, and dispatch every fire.
//!
//! Each fire emits the Tauri event `hot-corner` (`{ corner, action, monitor_id }`)
//! and then runs the action through [`dispatch`]: `switcher` and `panel` open
//! their views here; the other actions (`cheat_sheet`, `focus`, `calm_desk`,
//! `pause`, `show_zones`) belong to other parts of the app, which either listen
//! to the event or plug a handler in with [`register`].
//!
//! Kept out of `Shared` on purpose (a process-wide slot instead), so this feature
//! touches the shared files only with a few `// --- corners ---` lines.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

use krova_core::corners::{accent_rgb, HotCornerAction};
use krova_core::Settings;
use krova_platform::corners::{CornerConfig, CornerHit, HotCornersHandle};
use krova_platform::{EventSink, PlatformError};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::model::{lock, Shared};

/// Keeps the platform watcher alive while the app runs.
static WATCHER: Mutex<Option<HotCornersHandle>> = Mutex::new(None);
/// The platform can watch corners (false on macOS for now).
static SUPPORTED: AtomicBool = AtomicBool::new(false);

/// An in-process handler for one action: gets the profile id of the monitor
/// whose corner fired (if it is known).
pub type Handler = Arc<dyn Fn(Option<&str>) + Send + Sync>;

static HANDLERS: Mutex<Vec<(HotCornerAction, Handler)>> = Mutex::new(Vec::new());

/// Payload of the `hot-corner` event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HotCornerEvent {
    /// `"top_left"`, `"top_right"`, `"bottom_left"` or `"bottom_right"`.
    pub corner: &'static str,
    /// A `HotCornerAction` name, such as `"switcher"`.
    pub action: &'static str,
    /// `MonitorProfile.id` of the monitor whose corner it was.
    pub monitor_id: Option<String>,
}

/// What `hot_corners_status` returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct HotCornerStatus {
    /// This OS build can watch the screen corners.
    pub supported: bool,
    /// Corners are being watched right now: supported, Krova on, at least one
    /// corner has an action.
    pub active: bool,
}

/// Where an action goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    /// Bring the UI window forward on this view.
    View(&'static str),
    /// A registered handler, or only the event.
    Elsewhere,
}

fn route(action: HotCornerAction) -> Route {
    match action {
        HotCornerAction::Switcher => Route::View("switcher"),
        HotCornerAction::Panel => Route::View("settings"),
        HotCornerAction::CheatSheet
        | HotCornerAction::Focus
        | HotCornerAction::CalmDesk
        | HotCornerAction::Pause
        | HotCornerAction::ShowZones => Route::Elsewhere,
    }
}

/// What the watcher should do for these settings: nothing while Krova is off.
pub fn config_for(settings: &Settings) -> CornerConfig {
    let accent = accent_rgb(settings);
    if settings.enabled {
        CornerConfig {
            settings: settings.hot_corners.clone(),
            accent,
        }
    } else {
        CornerConfig::off(accent)
    }
}

/// Plug in the code that runs `action` (the engine's calm desk, focus, ...). A
/// later registration for the same action replaces the earlier one.
#[allow(dead_code)] // Called by the engine / wave 3 branches once they land.
pub fn register(action: HotCornerAction, handler: Handler) {
    let mut handlers = lock(&HANDLERS);
    handlers.retain(|(a, _)| *a != action);
    handlers.push((action, handler));
}

fn handler_for(action: HotCornerAction) -> Option<Handler> {
    lock(&HANDLERS)
        .iter()
        .find(|(a, _)| *a == action)
        .map(|(_, h)| Arc::clone(h))
}

/// Run a hot-corner action. `monitor_id` is the monitor whose corner fired.
pub fn dispatch(shared: &Shared, action: HotCornerAction, monitor_id: Option<String>) {
    match route(action) {
        Route::View(view) => {
            if view == "switcher" {
                // The switcher is for the monitor whose corner was used.
                let target = monitor_id.or_else(|| lock(&shared.model).monitor_for_switcher(None));
                let Some(target) = target else { return };
                *lock(&shared.switcher_monitor) = Some(target);
            }
            // Take the opener out of its lock first: opening can create the window.
            let open = lock(&shared.open_view).clone();
            if let Some(open) = open {
                open(view);
            }
        }
        Route::Elsewhere => {
            if let Some(h) = handler_for(action) {
                h(monitor_id.as_deref());
            }
        }
    }
}

fn on_hit(app: &AppHandle, shared: &Shared, hit: CornerHit) {
    let monitor_id = lock(&shared.model).profile_at(hit.monitor);
    let event = HotCornerEvent {
        corner: hit.corner.as_str(),
        action: hit.action.as_str(),
        monitor_id: monitor_id.clone(),
    };
    let _ = app.emit("hot-corner", &event);
    dispatch(shared, hit.action, monitor_id);
}

/// Start watching the corners. Call once, after the engine (whose mouse hook the
/// watcher rides on) has started.
pub fn start(app: &AppHandle, shared: &Arc<Shared>) {
    let config = config_for(&lock(&shared.model).config.settings);
    // The platform sink must return at once: hand hits to a small thread that
    // sleeps in `recv` until a corner fires.
    let (tx, rx) = mpsc::channel::<CornerHit>();
    let app = app.clone();
    let worker_shared = Arc::clone(shared);
    let spawned = thread::Builder::new()
        .name("krova-corners-dispatch".into())
        .spawn(move || {
            for hit in rx {
                on_hit(&app, &worker_shared, hit);
            }
        });
    if let Err(e) = spawned {
        eprintln!("krova: could not start the hot corner dispatcher: {e}");
        return;
    }
    let tx = Mutex::new(tx);
    let sink: EventSink<CornerHit> = Arc::new(move |hit| {
        let _ = lock(&tx).send(hit);
    });
    match shared.platform.start_hot_corners(config, sink) {
        Ok(handle) => {
            SUPPORTED.store(true, Ordering::SeqCst);
            *lock(&WATCHER) = Some(handle);
        }
        Err(PlatformError::Unsupported) => {}
        Err(e) => {
            SUPPORTED.store(true, Ordering::SeqCst);
            eprintln!("krova: hot corners are unavailable: {e}");
        }
    }
}

/// Settings changed: tell the watcher if anything it uses did.
pub fn settings_changed(old: &Settings, new: &Settings) {
    let wanted = config_for(new);
    if config_for(old) == wanted {
        return;
    }
    if let Some(handle) = lock(&WATCHER).as_ref() {
        handle.update(wanted);
    }
}

fn status(settings: &Settings) -> HotCornerStatus {
    let supported = SUPPORTED.load(Ordering::SeqCst);
    let running = lock(&WATCHER).is_some();
    HotCornerStatus {
        supported,
        active: running && config_for(settings).settings.any(),
    }
}

#[tauri::command]
pub fn hot_corners_status(shared: State<'_, Arc<Shared>>) -> HotCornerStatus {
    status(&lock(&shared.model).config.settings)
}

// Tests that drop a `HotCornerEvent` (an `Option<String>`) or a `Handler` (an
// `Arc<dyn Fn>`) are left out: their drop glue resolves to tauri's copy and the
// test binary then fails to start (see the note in Cargo.toml). The event's JSON
// shape is covered by the core name tests and the UI mock tests.
#[cfg(test)]
mod tests {
    use super::*;
    use krova_core::corners::HotCornerSettings;

    #[test]
    fn switcher_and_panel_open_views_the_rest_goes_elsewhere() {
        assert_eq!(route(HotCornerAction::Switcher), Route::View("switcher"));
        assert_eq!(route(HotCornerAction::Panel), Route::View("settings"));
        for a in [
            HotCornerAction::CheatSheet,
            HotCornerAction::Focus,
            HotCornerAction::CalmDesk,
            HotCornerAction::Pause,
            HotCornerAction::ShowZones,
        ] {
            assert_eq!(route(a), Route::Elsewhere);
        }
    }

    #[test]
    fn nothing_is_watched_while_krova_is_off() {
        let mut s = Settings::default();
        assert!(config_for(&s).settings.any());
        assert_eq!(config_for(&s).accent, (0xff, 0x5b, 0x1f));
        s.enabled = false;
        let off = config_for(&s);
        assert!(!off.settings.any());
        assert_eq!(off.accent, (0xff, 0x5b, 0x1f));
    }

    #[test]
    fn unrelated_setting_changes_do_not_reconfigure() {
        let old = Settings::default();
        let mut new = old.clone();
        new.zone_gap_px = 8;
        assert_eq!(config_for(&old), config_for(&new));
        new.hot_corners = HotCornerSettings {
            dwell_ms: 500,
            ..HotCornerSettings::default()
        };
        assert_ne!(config_for(&old), config_for(&new));
    }

    #[test]
    fn status_is_inactive_without_a_watcher() {
        let s = status(&Settings::default());
        assert!(!s.active);
    }
}
