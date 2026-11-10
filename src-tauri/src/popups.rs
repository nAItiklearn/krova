//! The two small windows besides the editor: `panel` (the quick panel) and
//! `switcher` (the layout switcher and the cheat sheet).
//!
//! Each is created the first time it is asked for and then **kept warm**: closing
//! it, pressing Esc in it or clicking outside it only hides it, so the next open is
//! a show and a `navigate` event, not a new WebView. The editor window (`main`) is
//! the opposite: destroyed on close, because it is big and rarely open.
//!
//! The UI tells the views apart by the window label (`getCurrentWindow().label`)
//! and the `navigate` payload: `panel` for the panel, `switcher` or `cheat_sheet`
//! in the switcher window. The page should call `hide_popup` on Esc.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use tauri::webview::PageLoadEvent;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewWindow,
    WebviewWindowBuilder, Wry,
};

use crate::model::{lock, Shared};

pub const PANEL: &str = "panel";
pub const SWITCHER: &str = "switcher";

/// After "page loaded" the page needs a moment to register its `navigate` listener.
const FIRST_DELIVERY_DELAY: Duration = Duration::from_millis(250);

/// Readiness, the view to deliver, and how long the last open took, per popup.
#[derive(Default)]
pub struct PopupState {
    inner: Mutex<HashMap<&'static str, Slot>>,
}

#[derive(Default)]
struct Slot {
    ready: bool,
    pending: Option<&'static str>,
    requested: Option<Instant>,
    /// Milliseconds from the request to the window being shown, last open.
    last_open_ms: Option<u128>,
    building: bool,
}

/// Which window shows a view, or `None` if the view is not a popup view.
pub fn window_for(view: &str) -> Option<(&'static str, &'static str)> {
    match view {
        "panel" => Some((PANEL, "panel")),
        "switcher" => Some((SWITCHER, "switcher")),
        "cheat_sheet" => Some((SWITCHER, "cheat_sheet")),
        _ => None,
    }
}

pub fn is_popup(label: &str) -> bool {
    label == PANEL || label == SWITCHER
}

impl PopupState {
    /// Milliseconds the last open of `label` took (request to shown), if measured.
    pub fn last_open_ms(&self, label: &str) -> Option<u128> {
        lock(&self.inner).get(label).and_then(|s| s.last_open_ms)
    }
}

/// Show a popup with a view, creating it on first use.
pub fn show(app: &AppHandle, view: &str) {
    let Some((label, view)) = window_for(view) else {
        return;
    };
    let state = app.state::<PopupState>();
    let warm = {
        let mut slots = lock(&state.inner);
        let slot = slots.entry(label).or_default();
        slot.pending = Some(view);
        slot.requested = Some(Instant::now());
        slot.ready
    };
    if let Some(window) = app.get_webview_window(label) {
        if warm {
            place(app, &window, label);
            reveal(app, &window, label);
            deliver(app, label);
        }
        // Still loading: the load handler shows it.
        return;
    }
    {
        let mut slots = lock(&state.inner);
        let slot = slots.entry(label).or_default();
        if slot.building {
            return;
        }
        slot.building = true;
    }
    // Building a WebView from an event handler deadlocks WebView2: own thread.
    let app = app.clone();
    let spawned = thread::Builder::new()
        .name("krova-popup".into())
        .spawn(move || create(&app, label));
    if let Err(e) = spawned {
        eprintln!("krova: could not start the popup thread: {e}");
    }
}

fn create(app: &AppHandle, label: &'static str) {
    let Some(config) = app.config().app.windows.iter().find(|w| w.label == label) else {
        eprintln!("krova: the {label} window is missing from tauri.conf.json");
        return;
    };
    let built = WebviewWindowBuilder::from_config(app, config).and_then(|builder| {
        builder
            .on_page_load(move |window, payload| {
                if payload.event() == PageLoadEvent::Finished {
                    loaded(&window, label);
                }
            })
            .build()
    });
    let state = app.state::<PopupState>();
    lock(&state.inner).entry(label).or_default().building = false;
    if let Err(e) = built {
        eprintln!("krova: could not create the {label} window: {e}");
    }
}

fn loaded(window: &WebviewWindow<Wry>, label: &'static str) {
    let app = window.app_handle().clone();
    lock(&app.state::<PopupState>().inner)
        .entry(label)
        .or_default()
        .ready = true;
    place(&app, window, label);
    reveal(&app, window, label);
    let spawned = thread::Builder::new()
        .name("krova-popup-navigate".into())
        .spawn(move || {
            thread::sleep(FIRST_DELIVERY_DELAY);
            deliver(&app, label);
        });
    if let Err(e) = spawned {
        eprintln!("krova: could not start the navigate thread: {e}");
    }
}

fn reveal(app: &AppHandle, window: &WebviewWindow<Wry>, label: &'static str) {
    let _ = window.show();
    let _ = window.set_focus();
    let state = app.state::<PopupState>();
    let mut slots = lock(&state.inner);
    let slot = slots.entry(label).or_default();
    if let Some(at) = slot.requested.take() {
        slot.last_open_ms = Some(at.elapsed().as_millis());
    }
}

fn deliver(app: &AppHandle, label: &'static str) {
    let view = lock(&app.state::<PopupState>().inner)
        .get_mut(label)
        .and_then(|s| s.pending.take());
    if let Some(view) = view {
        let _ = app.emit_to(label, "navigate", view);
    }
    if label == PANEL {
        let anchor = *lock(&app.state::<Arc<Shared>>().panel_anchor);
        if let Some(rect) = anchor {
            let _ = app.emit_to(PANEL, "panel-anchor", serde_json::json!({ "rect": rect }));
        }
    }
}

/// Where a popup goes: the switcher in the middle of the monitor it is for, the
/// panel near the bottom-right corner of the primary monitor's work area (where the
/// tray is), or at the rect a `panel-anchor` set (the notch).
fn place(app: &AppHandle, window: &WebviewWindow<Wry>, label: &'static str) {
    let shared = app.state::<Arc<Shared>>();
    let size = window.outer_size().unwrap_or(PhysicalSize::new(360, 520));
    let (w, h) = (
        i32::try_from(size.width).unwrap_or(360),
        i32::try_from(size.height).unwrap_or(520),
    );
    let monitors = lock(&shared.model).snapshots();
    let target = if label == SWITCHER {
        let id = crate::commands::switcher_monitor_of(&shared);
        monitors.iter().find(|m| Some(&m.profile_id) == id.as_ref())
    } else {
        monitors.iter().find(|m| m.primary)
    }
    .or(monitors.first());
    let Some(m) = target else {
        return;
    };
    let work = m.work;
    let anchor = *lock(&shared.panel_anchor);
    let (x, y) = match (label, anchor) {
        (PANEL, Some(a)) => {
            // Below the anchor (a notch at the top), centred on it, kept on screen.
            let x = (a.x + a.w / 2 - w / 2).clamp(work.x, (work.right() - w).max(work.x));
            let below = a.bottom() + 8;
            let y = if below + h <= work.bottom() {
                below
            } else {
                (a.y - 8 - h).max(work.y)
            };
            (x, y)
        }
        (PANEL, None) => {
            let margin = (12.0 * m.scale).round() as i32;
            (work.right() - w - margin, work.bottom() - h - margin)
        }
        _ => (work.x + (work.w - w) / 2, work.y + (work.h - h) / 3),
    };
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

/// Hide a popup (Esc, a click outside, the close button). It stays warm.
pub fn hide(app: &AppHandle, label: &str) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.hide();
    }
}

/// The page asks to hide its popup (Esc). Only the two popup labels are accepted.
#[tauri::command]
pub fn hide_popup(app: AppHandle, label: String) -> Result<(), String> {
    if !is_popup(&label) {
        return Err(format!("{label} is not a popup window"));
    }
    hide(&app, &label);
    Ok(())
}

/// Milliseconds the last open of a popup took, from the request to the window being
/// shown (`null` before the first open). For the latency measurement.
#[tauri::command]
pub fn popup_open_ms(state: tauri::State<'_, PopupState>, label: String) -> Option<u64> {
    state
        .last_open_ms(&label)
        .and_then(|ms| u64::try_from(ms).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn views_go_to_their_window() {
        assert_eq!(window_for("panel"), Some((PANEL, "panel")));
        assert_eq!(window_for("switcher"), Some((SWITCHER, "switcher")));
        assert_eq!(window_for("cheat_sheet"), Some((SWITCHER, "cheat_sheet")));
        assert_eq!(window_for("editor"), None);
        assert_eq!(window_for("../x"), None);
        assert!(is_popup("panel") && is_popup("switcher") && !is_popup("main"));
    }

    #[test]
    fn the_last_open_time_is_kept_per_popup() {
        let s = PopupState::default();
        assert_eq!(s.last_open_ms(PANEL), None);
        lock(&s.inner).entry(PANEL).or_default().last_open_ms = Some(42);
        assert_eq!(s.last_open_ms(PANEL), Some(42));
        assert_eq!(s.last_open_ms(SWITCHER), None);
    }
}
