//! Wiring of the notch: feeds the native notch window (`krova_platform::notch`)
//! from the app state, acts on its clicks and menu, and owns the small `panel`
//! web window that a click opens next to it.
//!
//! The notch costs nothing while idle: it follows the app through the
//! `state-changed` event it already emits, and its own events arrive on a
//! channel served by one sleeping thread. The panel window is created on the
//! first click only and then kept warm (hidden) so later clicks are instant.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use krova_core::notch::{
    panel_rect, parse_hex, to_px, NotchCorner, DEFAULT_ACCENT, DEFAULT_RADIUS,
};
use krova_core::Settings;
use krova_platform::notch::{
    start_notch, Notch, NotchAnchor, NotchEvent, NotchLayout, NotchMenu, NotchModel,
    NotchMonitorInfo, SUPPORTED,
};
use serde::Serialize;
use tauri::webview::PageLoadEvent;
use tauri::{
    AppHandle, Emitter, Listener, Manager, PhysicalPosition, PhysicalSize, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder, Window, WindowEvent, Wry,
};

use crate::commands::{build_state, change_settings, commit};
use crate::model::{lock, Shared};
use crate::ui_window;

/// Label of the quick panel window (also in the capability file).
pub const PANEL: &str = "panel";
/// Panel size, logical px.
const PANEL_SIZE: (f64, f64) = (360.0, 440.0);
/// A click this soon after the panel lost focus is the click that closed it
/// (the blur comes first), so it must not reopen it.
const REOPEN_GUARD: Duration = Duration::from_millis(400);
/// Like `ui_window`: the page registers its listeners shortly after "loaded".
const DELIVERY_DELAY: Duration = Duration::from_millis(250);

/// Payload of the `panel-anchor` event: where the notch is (physical screen px),
/// which position it sits at and which monitor (`MonitorProfile.id`) it is on.
/// The panel window has already been placed next to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PanelAnchor {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub corner: NotchCorner,
    pub monitor_id: String,
}

impl From<&NotchAnchor> for PanelAnchor {
    fn from(a: &NotchAnchor) -> Self {
        Self {
            x: a.rect.x,
            y: a.rect.y,
            w: a.rect.w,
            h: a.rect.h,
            corner: a.corner,
            monitor_id: a.monitor_id.clone(),
        }
    }
}

#[derive(Default)]
pub struct NotchState {
    notch: Mutex<Option<Box<dyn Notch>>>,
    /// The last anchor, for a panel page that is still loading.
    anchor: Mutex<Option<PanelAnchor>>,
    panel_ready: AtomicBool,
    hidden_at: Mutex<Option<Instant>>,
    creating: Mutex<()>,
}

/// Accent (`0xrrggbb`) and corner radius (logical px) from `Settings.appearance`
/// when that block exists, else Krova's defaults (`#ff5b1f`, 10 px). Read
/// through JSON so it works with or without the `appearance` field.
pub fn appearance(settings: &Settings) -> (u32, f64) {
    serde_json::to_value(settings).map_or((DEFAULT_ACCENT, DEFAULT_RADIUS), |v| appearance_of(&v))
}

fn appearance_of(settings: &serde_json::Value) -> (u32, f64) {
    let block = settings.get("appearance");
    let accent = block
        .and_then(|a| a.get("accent"))
        .and_then(serde_json::Value::as_str)
        .and_then(parse_hex)
        .unwrap_or(DEFAULT_ACCENT);
    let radius = block
        .and_then(|a| a.get("corner_radius_px"))
        .and_then(serde_json::Value::as_f64)
        .filter(|r| r.is_finite() && (0.0..=32.0).contains(r))
        .unwrap_or(DEFAULT_RADIUS);
    (accent, radius)
}

/// Share-safe (`Settings.share_safe`, or `Settings.appearance.share_safe`), false
/// while no such setting exists.
pub fn share_safe(settings: &Settings) -> bool {
    serde_json::to_value(settings).is_ok_and(|v| share_safe_of(&v))
}

fn share_safe_of(settings: &serde_json::Value) -> bool {
    settings
        .get("share_safe")
        .or_else(|| settings.get("appearance").and_then(|a| a.get("share_safe")))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

/// What the notch should show, from the app model.
fn model_of(shared: &Shared) -> (NotchModel, bool) {
    let model = lock(&shared.model);
    let settings = &model.config.settings;
    let (accent, radius) = appearance(settings);
    let monitors = model
        .snapshots()
        .into_iter()
        .filter_map(|snap| {
            let profile = model
                .config
                .monitors
                .iter()
                .find(|p| p.id == snap.profile_id)?;
            Some(NotchMonitorInfo {
                id: snap.profile_id.clone(),
                rect: snap.rect,
                work: snap.work,
                scale: snap.scale,
                primary: snap.primary,
                layouts: profile
                    .layouts
                    .iter()
                    .map(|l| NotchLayout {
                        id: l.id.clone(),
                        name: l.name.clone(),
                        zones: l.zones.len(),
                    })
                    .collect(),
                active: profile.active_layout.clone(),
            })
        })
        .collect();
    (
        NotchModel {
            settings: settings.notch,
            monitors,
            paused: !settings.enabled,
            accent,
            radius,
        },
        share_safe(settings),
    )
}

/// Send the current state to the notch.
fn push(app: &AppHandle) {
    let (Some(shared), Some(state)) = (
        app.try_state::<Arc<Shared>>(),
        app.try_state::<NotchState>(),
    ) else {
        return;
    };
    let (model, safe) = model_of(&shared);
    let guard = lock(&state.notch);
    if let Some(notch) = guard.as_ref() {
        notch.set_share_safe(safe);
        notch.update(model);
    }
}

/// Start the notch (Windows only; elsewhere this does nothing).
pub fn start(app: &AppHandle) {
    app.manage(NotchState::default());
    if !SUPPORTED {
        return;
    }
    let (tx, rx) = mpsc::channel::<NotchEvent>();
    let notch = match start_notch(Arc::new(move |e| {
        let _ = tx.send(e);
    })) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("krova: the notch is not available: {e}");
            return;
        }
    };
    *lock(&app.state::<NotchState>().notch) = Some(notch);
    push(app);

    let handle = app.clone();
    app.listen_any("state-changed", move |_| push(&handle));

    let handle = app.clone();
    let spawned = thread::Builder::new()
        .name("krova-notch-events".into())
        .spawn(move || {
            for event in rx {
                handle_event(&handle, event);
            }
        });
    if let Err(e) = spawned {
        eprintln!("krova: could not start the notch event thread: {e}");
    }
}

fn report(result: Result<(), String>) {
    if let Err(e) = result {
        eprintln!("krova: {e}");
    }
}

fn settings_then_commit(app: &AppHandle, shared: &Shared, edit: impl FnOnce(&mut Settings)) {
    report(change_settings(shared, edit).and_then(|()| commit(app, shared).map(|_| ())));
}

fn handle_event(app: &AppHandle, event: NotchEvent) {
    let shared = app.state::<Arc<Shared>>();
    match event {
        NotchEvent::Clicked(anchor) => request_panel(app, &anchor),
        NotchEvent::CornerChanged(corner) => {
            settings_then_commit(app, &shared, |s| s.notch.corner = corner);
        }
        NotchEvent::DisplayChanged => {
            if let Ok(infos) = shared.platform.monitors() {
                if let Err(e) = lock(&shared.model).refresh_monitors(infos) {
                    eprintln!("krova: could not save the config: {e}");
                }
            }
            let _ = app.emit("state-changed", build_state(&shared));
        }
        NotchEvent::Menu { monitor_id, item } => match item {
            NotchMenu::TogglePause => {
                settings_then_commit(app, &shared, |s| s.enabled = !s.enabled);
            }
            NotchMenu::Layout(layout_id) => {
                let set = lock(&shared.model)
                    .config
                    .set_active_layout(&monitor_id, &layout_id)
                    .map_err(|e| e.to_string());
                report(set.and_then(|()| commit(app, &shared).map(|_| ())));
            }
            NotchMenu::Switcher => {
                *lock(&shared.switcher_monitor) = Some(monitor_id);
                ui_window::show(app, Some("switcher"));
            }
            NotchMenu::CheatSheet => ui_window::show(app, Some("cheatsheet")),
            NotchMenu::OpenApp => ui_window::show(app, Some("editor")),
            NotchMenu::Hide => settings_then_commit(app, &shared, |s| s.notch.enabled = false),
        },
    }
}

/// Where the panel goes for this anchor (physical px).
fn panel_place(anchor: &NotchAnchor) -> krova_core::PxRect {
    let scale = anchor.scale;
    panel_rect(
        anchor.rect,
        anchor.corner,
        anchor.work,
        to_px(PANEL_SIZE.0, scale),
        to_px(PANEL_SIZE.1, scale),
        to_px(krova_core::notch::PANEL_GAP, scale),
    )
}

/// Open (or close, if it is showing) the quick panel next to the notch, and
/// emit `panel-anchor`. The single entry point for the panel.
// --- notch: replace with ui_window::show_panel after engine merge ---
// The engine branch owns the `panel` window (it loads `ui/panel.html`). Until
// that merges, this builds a window with the same settings when none exists
// (loading `index.html`, which receives `navigate("panel")`), so the notch
// works standalone. An existing `panel` window is always reused as is.
pub fn request_panel(app: &AppHandle, anchor: &NotchAnchor) {
    let state = app.state::<NotchState>();
    let payload = PanelAnchor::from(anchor);
    *lock(&state.anchor) = Some(payload.clone());
    if let Some(window) = app.get_webview_window(PANEL) {
        if window.is_visible().unwrap_or(false) {
            hide_window(&window);
            return;
        }
        let just_closed = lock(&state.hidden_at).is_some_and(|t| t.elapsed() < REOPEN_GUARD);
        if just_closed {
            return;
        }
        place_and_show(&window, anchor);
        if state.panel_ready.load(Ordering::SeqCst) {
            deliver(app);
        }
        return;
    }
    let _creating = lock(&state.creating);
    if app.get_webview_window(PANEL).is_some() {
        return;
    }
    let mut builder = WebviewWindowBuilder::new(app, PANEL, WebviewUrl::App("index.html".into()))
        .title("Krova")
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .visible(false)
        .on_page_load(|window, payload| {
            if payload.event() == PageLoadEvent::Finished {
                panel_loaded(&window);
            }
        });
    #[cfg(not(target_os = "macos"))]
    {
        builder = builder.transparent(true);
    }
    match builder.build() {
        Ok(window) => {
            // Placed now, shown once the page has loaded (no white flash).
            let r = panel_place(anchor);
            let _ = window.set_size(PhysicalSize::new(r.w.max(1) as u32, r.h.max(1) as u32));
            let _ = window.set_position(PhysicalPosition::new(r.x, r.y));
        }
        Err(e) => eprintln!("krova: could not create the panel: {e}"),
    }
}

fn place_and_show(window: &WebviewWindow<Wry>, anchor: &NotchAnchor) {
    let r = panel_place(anchor);
    let _ = window.set_size(PhysicalSize::new(r.w.max(1) as u32, r.h.max(1) as u32));
    let _ = window.set_position(PhysicalPosition::new(r.x, r.y));
    let _ = window.show();
    let _ = window.set_focus();
}

fn panel_loaded(window: &WebviewWindow<Wry>) {
    let app = window.app_handle().clone();
    app.state::<NotchState>()
        .panel_ready
        .store(true, Ordering::SeqCst);
    let _ = window.show();
    let _ = window.set_focus();
    let spawned = thread::Builder::new()
        .name("krova-panel".into())
        .spawn(move || {
            thread::sleep(DELIVERY_DELAY);
            deliver(&app);
        });
    if let Err(e) = spawned {
        eprintln!("krova: could not start the panel thread: {e}");
    }
}

/// Tell the panel page where it is and which view to show.
fn deliver(app: &AppHandle) {
    let anchor = lock(&app.state::<NotchState>().anchor).clone();
    if let Some(anchor) = anchor {
        let _ = app.emit_to(PANEL, "panel-anchor", &anchor);
    }
    let _ = app.emit_to(PANEL, "navigate", "panel");
}

fn hide_window(window: &WebviewWindow<Wry>) {
    let _ = window.hide();
    if let Some(state) = window.app_handle().try_state::<NotchState>() {
        *lock(&state.hidden_at) = Some(Instant::now());
    }
}

/// Window events of the panel: closing or losing focus hides it (it stays warm).
/// Returns true when the event was the panel's, so the caller skips its own
/// handling (which is for the editor window).
pub fn on_window_event(window: &Window<Wry>, event: &WindowEvent) -> bool {
    if window.label() != PANEL {
        return false;
    }
    match event {
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            if let Some(w) = window.app_handle().get_webview_window(PANEL) {
                hide_window(&w);
            }
        }
        WindowEvent::Focused(false) => {
            if let Some(w) = window.app_handle().get_webview_window(PANEL) {
                hide_window(&w);
            }
        }
        WindowEvent::Destroyed => {
            if let Some(state) = window.app_handle().try_state::<NotchState>() {
                state.panel_ready.store(false, Ordering::SeqCst);
            }
        }
        _ => {}
    }
    true
}

/// The panel asks to close (Esc, or after it ran an action).
#[tauri::command]
pub fn hide_panel(app: AppHandle) {
    if let Some(w) = app.get_webview_window(PANEL) {
        hide_window(&w);
    }
}

/// The last `panel-anchor`, for a panel page that missed the event.
#[tauri::command]
pub fn panel_anchor(state: tauri::State<'_, NotchState>) -> Option<PanelAnchor> {
    lock(&state.anchor).clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn appearance_falls_back_to_the_defaults() {
        assert_eq!(appearance_of(&json!({})), (DEFAULT_ACCENT, DEFAULT_RADIUS));
        assert_eq!(
            appearance_of(&json!({"appearance": {"accent": "#00AA11", "corner_radius_px": 6}})),
            (0x00aa11, 6.0)
        );
        assert_eq!(
            appearance_of(&json!({"appearance": {"accent": "red", "corner_radius_px": 900}})),
            (DEFAULT_ACCENT, DEFAULT_RADIUS)
        );
        assert_eq!(
            appearance_of(&json!({"appearance": 3})),
            (DEFAULT_ACCENT, DEFAULT_RADIUS)
        );
    }

    #[test]
    fn share_safe_is_off_unless_a_setting_says_so() {
        assert!(!share_safe_of(&json!({})));
        assert!(share_safe_of(&json!({"share_safe": true})));
        assert!(share_safe_of(&json!({"appearance": {"share_safe": true}})));
        assert!(!share_safe_of(&json!({"share_safe": "yes"})));
    }

    #[test]
    fn the_anchor_payload_has_the_contract_field_names() {
        let a = NotchAnchor {
            rect: krova_core::PxRect::new(850, 4, 220, 28),
            corner: NotchCorner::TopCenter,
            monitor_id: "monitor-1".into(),
            work: krova_core::PxRect::new(0, 0, 1920, 1032),
            scale: 1.0,
        };
        let v = serde_json::to_value(PanelAnchor::from(&a)).unwrap();
        assert_eq!(
            v,
            json!({"x": 850, "y": 4, "w": 220, "h": 28, "corner": "top_center", "monitor_id": "monitor-1"})
        );
        assert_eq!(panel_place(&a), krova_core::PxRect::new(780, 38, 360, 440));
    }
}
