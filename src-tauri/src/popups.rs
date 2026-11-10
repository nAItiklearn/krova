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

use krova_core::PxRect;
use serde::{Deserialize, Serialize};

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
    label == PANEL || label == SWITCHER || label == PICKER
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
    match built {
        Ok(window) => track_capture(&window),
        Err(e) => eprintln!("krova: could not create the {label} window: {e}"),
    }
}

// --- integration --- share-safe: the popups join the capture-exclusion registry
/// While share-safe is on, a popup is excluded from screen capture
/// (`WDA_EXCLUDEFROMCAPTURE`), like the notch, the strips and the zone overlays.
/// The popups stay warm (hidden, never destroyed) until the app quits; a destroyed
/// one leaves the registry.
fn track_capture(window: &WebviewWindow<Wry>) {
    #[cfg(windows)]
    if let Ok(hwnd) = window.hwnd() {
        let raw = hwnd.0 as isize;
        krova_platform::desk::track_capture_window(raw);
        window.on_window_event(move |event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                krova_platform::desk::untrack_capture_window(raw);
            }
        });
    }
    #[cfg(not(windows))]
    let _ = window;
}
// --- integration ---

fn loaded(window: &WebviewWindow<Wry>, label: &'static str) {
    let app = window.app_handle().clone();
    lock(&app.state::<PopupState>().inner)
        .entry(label)
        .or_default()
        .ready = true;
    backdrop(&app, window);
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
        let anchor = lock(&app.state::<Arc<Shared>>().panel_anchor).clone();
        if let Some(anchor) = anchor {
            let _ = app.emit_to(PANEL, "panel-anchor", anchor);
        }
    }
}

/// Where the quick panel was opened from: the notch's rect (physical pixels), the
/// screen corner it sits in and the monitor (profile id). Sent to the panel as the
/// `panel-anchor` event, unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanelAnchor {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    /// `"top_center"`, `"top_left"`, `"top_right"`, `"bottom_left"`, `"bottom_right"`.
    pub corner: String,
    pub monitor_id: String,
}

/// The panel's top-left corner for a panel of `w` x `h` on a monitor with work area
/// `work`: next to the anchor (below a top notch, above a bottom one, centred on it,
/// kept on screen), or near the tray (bottom right, `margin` from the edges).
pub fn panel_position(
    work: PxRect,
    w: i32,
    h: i32,
    anchor: Option<&PanelAnchor>,
    margin: i32,
) -> (i32, i32) {
    let Some(a) = anchor else {
        return (work.right() - w - margin, work.bottom() - h - margin);
    };
    let max_x = (work.right() - w).max(work.x);
    let max_y = (work.bottom() - h).max(work.y);
    let x = match a.corner.as_str() {
        "top_left" | "bottom_left" => a.x,
        "top_right" | "bottom_right" => a.x + a.w - w,
        _ => a.x + a.w / 2 - w / 2,
    }
    .clamp(work.x, max_x);
    let y = if a.corner.starts_with("bottom") {
        a.y - margin - h
    } else {
        a.y + a.h + margin
    }
    .clamp(work.y, max_y);
    (x, y)
}

/// Where a popup goes. The switcher covers the whole monitor it is for (the page
/// draws its own backdrop and deck); the panel opens next to its anchor (the notch)
/// or near the tray on the primary monitor.
fn place(app: &AppHandle, window: &WebviewWindow<Wry>, label: &'static str) {
    let shared = app.state::<Arc<Shared>>();
    let monitors = lock(&shared.model).snapshots();
    let anchor = lock(&shared.panel_anchor).clone();
    let target = match label {
        SWITCHER => {
            let id = crate::commands::switcher_monitor_of(&shared);
            monitors.iter().find(|m| Some(&m.profile_id) == id.as_ref())
        }
        _ => anchor
            .as_ref()
            .and_then(|a| monitors.iter().find(|m| m.profile_id == a.monitor_id))
            .or_else(|| monitors.iter().find(|m| m.primary)),
    }
    .or(monitors.first());
    let Some(m) = target else {
        return;
    };
    if label == SWITCHER {
        let r = m.rect;
        let _ = window.set_size(PhysicalSize::new(
            u32::try_from(r.w).unwrap_or(1),
            u32::try_from(r.h).unwrap_or(1),
        ));
        let _ = window.set_position(PhysicalPosition::new(r.x, r.y));
        return;
    }
    let size = window.outer_size().unwrap_or(PhysicalSize::new(360, 520));
    let (w, h) = (
        i32::try_from(size.width).unwrap_or(360),
        i32::try_from(size.height).unwrap_or(520),
    );
    let margin = (12.0 * m.scale).round() as i32;
    let (x, y) = panel_position(m.work, w, h, anchor.as_ref(), margin);
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

/// The blurred system backdrop for a popup, once, after its page loaded.
fn backdrop(app: &AppHandle, window: &WebviewWindow<Wry>) {
    #[cfg(windows)]
    {
        let shared = app.state::<Arc<Shared>>();
        let dark = match lock(&shared.model).config.settings.theme {
            krova_core::Theme::Dark => true,
            krova_core::Theme::Light => false,
            krova_core::Theme::System => shared.platform.prefers_dark(),
        };
        if let Ok(hwnd) = window.hwnd() {
            let _ = krova_platform::apply_backdrop(hwnd.0 as isize, dark);
        }
    }
    #[cfg(not(windows))]
    let _ = (app, window);
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

    // --- integration ---
    #[test]
    fn the_picker_sits_by_the_cursor_inside_the_work_area() {
        let work = PxRect::new(0, 0, 1920, 1040);
        assert_eq!(picker_position(work, 240, 160, (100, 100), 12), (112, 112));
        // Near the right and bottom edges it stays inside, above the cursor.
        assert_eq!(
            picker_position(work, 240, 160, (1900, 1000), 12),
            (1680, 828)
        );
        assert!(is_popup(PICKER));
    }
    // --- end integration ---

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

    fn anchor(corner: &str, x: i32, y: i32) -> PanelAnchor {
        PanelAnchor {
            x,
            y,
            w: 36,
            h: 10,
            corner: corner.into(),
            monitor_id: "monitor-1".into(),
        }
    }

    const WORK: PxRect = PxRect::new(0, 0, 1920, 1040);

    #[test]
    fn without_an_anchor_the_panel_sits_by_the_tray() {
        assert_eq!(panel_position(WORK, 360, 520, None, 12), (1548, 508));
    }

    #[test]
    fn a_top_notch_opens_the_panel_below_it_centred() {
        let a = anchor("top_center", 942, 0);
        assert_eq!(panel_position(WORK, 360, 520, Some(&a), 12), (780, 22));
        // Kept on screen at the left and right.
        let a = anchor("top_left", 0, 0);
        assert_eq!(panel_position(WORK, 360, 520, Some(&a), 12).0, 0);
        let a = anchor("top_right", 1884, 0);
        assert_eq!(panel_position(WORK, 360, 520, Some(&a), 12).0, 1920 - 360);
    }

    #[test]
    fn a_bottom_notch_opens_the_panel_above_it() {
        let a = anchor("bottom_right", 1884, 1030);
        let (x, y) = panel_position(WORK, 360, 520, Some(&a), 12);
        assert_eq!(y, 1030 - 12 - 520);
        assert_eq!(x, 1884 + 36 - 360);
    }

    #[test]
    fn the_anchor_travels_to_the_panel_as_is() {
        let json = serde_json::to_value(anchor("top_center", 1, 2)).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"x":1,"y":2,"w":36,"h":10,"corner":"top_center","monitor_id":"monitor-1"})
        );
    }
}

// --- integration ---
// The hover zone picker (wave 4): a third warm popup, `picker` (`picker.html`),
// shown near the cursor on `hover-picker` and hidden on `hover-picker-end`. It
// never takes focus (the window under the cursor keeps it); the page picks a
// zone with `place_hovered` and may hide itself with `hide_popup("picker")`.

pub const PICKER: &str = "picker";

/// The picker's state: the last `hover-picker` payload (re-sent once the page
/// has loaded) and whether the page is ready.
#[derive(Default)]
struct PickerSlot {
    payload: Option<serde_json::Value>,
    ready: bool,
    building: bool,
}

static PICKER_SLOT: Mutex<PickerSlot> = Mutex::new(PickerSlot {
    payload: None,
    ready: false,
    building: false,
});

/// Where the picker's top-left corner goes: just below and right of the cursor,
/// kept inside the work area. Physical px.
pub fn picker_position(work: PxRect, w: i32, h: i32, cursor: (i32, i32), gap: i32) -> (i32, i32) {
    let max_x = (work.right() - w).max(work.x);
    let max_y = (work.bottom() - h).max(work.y);
    let x = (cursor.0 + gap).clamp(work.x, max_x);
    // Below the cursor when it fits, else above it.
    let y = if cursor.1 + gap + h <= work.bottom() {
        cursor.1 + gap
    } else {
        cursor.1 - gap - h
    }
    .clamp(work.y, max_y);
    (x, y)
}

fn cursor_of(payload: &serde_json::Value) -> Option<(i32, i32)> {
    let c = payload.get("cursor")?;
    let x = i32::try_from(c.get(0)?.as_i64()?).ok()?;
    let y = i32::try_from(c.get(1)?.as_i64()?).ok()?;
    Some((x, y))
}

fn place_picker(app: &AppHandle, window: &WebviewWindow<Wry>, payload: &serde_json::Value) {
    let Some(cursor) = cursor_of(payload) else {
        return;
    };
    let shared = app.state::<Arc<Shared>>();
    let monitors = lock(&shared.model).snapshots();
    let Some(m) = monitors
        .iter()
        .find(|m| m.rect.contains(cursor.0, cursor.1))
        .or(monitors.first())
    else {
        return;
    };
    let size = window.outer_size().unwrap_or(PhysicalSize::new(240, 160));
    let (w, h) = (
        i32::try_from(size.width).unwrap_or(240),
        i32::try_from(size.height).unwrap_or(160),
    );
    let gap = (12.0 * m.scale).round() as i32;
    let (x, y) = picker_position(m.work, w, h, cursor, gap);
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

fn reveal_picker(app: &AppHandle, window: &WebviewWindow<Wry>, payload: &serde_json::Value) {
    place_picker(app, window, payload);
    // `show` without `set_focus`: the window config has `focus: false`.
    let _ = window.show();
    let _ = app.emit_to(PICKER, "hover-picker", payload);
}

/// `hover-picker` fired: show the picker next to the cursor (creating it the
/// first time; the page gets the payload once loaded).
pub fn show_picker(app: &AppHandle, payload: &serde_json::Value) {
    let (ready, build) = {
        let mut slot = lock(&PICKER_SLOT);
        slot.payload = Some(payload.clone());
        let build = !slot.ready && !slot.building && app.get_webview_window(PICKER).is_none();
        if build {
            slot.building = true;
        }
        (slot.ready, build)
    };
    if ready {
        if let Some(window) = app.get_webview_window(PICKER) {
            reveal_picker(app, &window, payload);
        }
        return;
    }
    if !build {
        return; // still loading: the load handler shows it
    }
    let app = app.clone();
    let spawned = thread::Builder::new()
        .name("krova-picker".into())
        .spawn(move || create_picker(&app));
    if let Err(e) = spawned {
        lock(&PICKER_SLOT).building = false;
        eprintln!("krova: could not start the picker thread: {e}");
    }
}

fn create_picker(app: &AppHandle) {
    let Some(config) = app.config().app.windows.iter().find(|w| w.label == PICKER) else {
        lock(&PICKER_SLOT).building = false;
        eprintln!("krova: the picker window is missing from tauri.conf.json");
        return;
    };
    let built = WebviewWindowBuilder::from_config(app, config).and_then(|builder| {
        builder
            .on_page_load(|window, payload| {
                if payload.event() == PageLoadEvent::Finished {
                    picker_loaded(&window);
                }
            })
            .build()
    });
    lock(&PICKER_SLOT).building = false;
    match built {
        Ok(window) => track_capture(&window),
        Err(e) => eprintln!("krova: could not create the picker window: {e}"),
    }
}

fn picker_loaded(window: &WebviewWindow<Wry>) {
    let app = window.app_handle().clone();
    let window = window.clone();
    // Like the other popups: give the page a moment to register its listeners.
    let spawned = thread::Builder::new()
        .name("krova-picker-show".into())
        .spawn(move || {
            thread::sleep(FIRST_DELIVERY_DELAY);
            let payload = {
                let mut slot = lock(&PICKER_SLOT);
                slot.ready = true;
                slot.payload.clone()
            };
            if let Some(payload) = payload {
                reveal_picker(&app, &window, &payload);
            }
        });
    if let Err(e) = spawned {
        eprintln!("krova: could not start the picker thread: {e}");
    }
}

/// `hover-picker-end` fired (or a zone was picked): hide the picker.
pub fn hide_picker(app: &AppHandle) {
    lock(&PICKER_SLOT).payload = None;
    hide(app, PICKER);
}
// --- end integration ---
