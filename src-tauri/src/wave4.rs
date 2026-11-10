//! Wave 4 ("fit and finish") commands and glue: reserved edges, focus follows
//! mouse, the hover zone picker, "Check for updates" and the native string table.
//!
//! Everything lives here so the shared files (`engine.rs`, `commands.rs`,
//! `lib.rs`, `tray.rs`) only carry small `// --- wave4 ---` blocks that call in.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::thread;

use krova_core::edges::ReservedEdges;
use krova_core::registry::{SnapRecord, WindowKey};
use krova_core::{MoveMode, PxRect, Settings};
use krova_platform::pointer::{PointerConfig, PointerEvent, Wave4Capabilities};
use krova_platform::{EventSink, WatchHandle, WindowId};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::commands::commit;
use crate::model::{lock, AppModel, AppState, Shared};

// ------------------------------------------------------------ reserved edges

/// Store the reserved edges of one monitor profile (sanitised: each side
/// `0..=400` logical px). The new work area applies at once to everything that
/// reads it (zones, snapping, keyboard moves, reconnect restore).
pub fn set_reserved(
    shared: &Shared,
    monitor_id: &str,
    reserved: ReservedEdges,
) -> Result<(), String> {
    let mut model = lock(&shared.model);
    let profile = model
        .config
        .monitors
        .iter_mut()
        .find(|m| m.id == monitor_id)
        .ok_or_else(|| format!("monitor {monitor_id} not found"))?;
    profile.reserved = reserved.sanitized();
    Ok(())
}

#[tauri::command]
pub fn set_reserved_edges(
    app: AppHandle,
    shared: State<'_, Arc<Shared>>,
    monitor_id: String,
    reserved: ReservedEdges,
) -> Result<AppState, String> {
    set_reserved(&shared, &monitor_id, reserved)?;
    commit(&app, &shared)
}

// ------------------------------------------------- focus follows mouse, hover

/// Process-wide wave 4 glue, set once by [`start`]. Absent in unit tests, where
/// every function below that needs it is a no-op.
struct Wave4 {
    /// Sends a Tauri event. A plain closure, so code the unit tests reach links
    /// none of the webview stack (see `[profile.test.package.krova-app]`).
    emit: EmitFn,
    shared: Arc<Shared>,
    pointer: Arc<PointerConfig>,
    watch: Mutex<Option<WatchHandle>>,
}

/// Sends one Tauri event (name, JSON payload).
type EmitFn = Arc<dyn Fn(&'static str, serde_json::Value) + Send + Sync>;

static WAVE4: OnceLock<Wave4> = OnceLock::new();
/// The window the hover picker was last opened for. Only its handle and monitor,
/// never its title.
static HOVERED: Mutex<Option<Hovered>> = Mutex::new(None);
/// How often Windows refused to let focus follows mouse activate a window.
static FOCUS_REFUSED: AtomicU32 = AtomicU32::new(0);

/// The window under an open hover picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hovered {
    pub window: WindowId,
    pub monitor_id: String,
}

/// Payload of the `hover-picker` event: the zones the hovered window can go to.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HoverPicker {
    /// `MonitorProfile.id` of the monitor under the cursor.
    pub monitor_id: String,
    /// The window's visible frame, in virtual-screen pixels.
    pub window_rect: PxRect,
    /// Where the cursor rests, `[x, y]` in virtual-screen pixels.
    pub cursor: [i32; 2],
    pub layout_id: String,
    /// The active layout's zones, in pixels, with the zone gap applied: exactly
    /// where `place_hovered` would put the window.
    pub zones: Vec<HoverZone>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HoverZone {
    pub id: u32,
    pub rect: PxRect,
}

/// Wave 4 additions to `AppState` (flattened into it).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Wave4State {
    pub wave4_capabilities: Wave4CapabilitiesState,
    pub pointer: PointerStatus,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Wave4CapabilitiesState {
    pub focus_follows_mouse: bool,
    pub hover_picker: bool,
    pub open_releases_page: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct PointerStatus {
    /// The resting-cursor watcher runs (focus follows mouse or the hover picker
    /// is on, and Krova is enabled).
    pub active: bool,
    /// Times Windows refused to activate a window since Krova started.
    pub focus_refused: u32,
}

/// The wave 4 part of the state the UI renders from.
pub fn state_extras(shared: &Shared) -> Wave4State {
    let caps = shared.platform.wave4_capabilities();
    let active = WAVE4.get().is_some_and(|w| lock(&w.watch).is_some());
    Wave4State {
        wave4_capabilities: Wave4CapabilitiesState {
            focus_follows_mouse: caps.focus_follows_mouse,
            hover_picker: caps.hover_picker,
            open_releases_page: caps.open_url,
        },
        pointer: PointerStatus {
            active,
            focus_refused: FOCUS_REFUSED.load(Ordering::Relaxed),
        },
    }
}

/// Which pointer features should run: `(focus follows mouse, hover picker)`.
pub fn pointer_wanted(settings: &Settings, caps: Wave4Capabilities) -> (bool, bool) {
    (
        settings.enabled && settings.focus_follows_mouse && caps.focus_follows_mouse,
        settings.enabled && settings.hover_picker && caps.hover_picker,
    )
}

/// Start the wave 4 glue. Called once from the Tauri setup, after the engine.
pub fn start(app: &AppHandle, shared: &Arc<Shared>) {
    let wave4 = Wave4 {
        emit: {
            let app = app.clone();
            Arc::new(move |name, payload| {
                let _ = app.emit(name, payload);
            })
        },
        shared: Arc::clone(shared),
        pointer: Arc::new(PointerConfig::default()),
        watch: Mutex::new(None),
    };
    if WAVE4.set(wave4).is_err() {
        return;
    }
    apply(shared);
}

/// Follow the settings: update the watcher's live config, start it when a
/// feature needs it and stop it (so nothing runs) when none does.
pub fn apply(shared: &Shared) {
    let Some(w) = WAVE4.get() else {
        return;
    };
    let settings = lock(&shared.model).config.settings.clone();
    let (focus, hover) = pointer_wanted(&settings, shared.platform.wave4_capabilities());
    w.pointer.set(
        focus,
        settings.focus_follows_mouse_delay_ms,
        hover,
        settings.modifier,
    );
    let mut watch = lock(&w.watch);
    if !(focus || hover) {
        *watch = None;
        *lock(&HOVERED) = None;
        return;
    }
    if watch.is_some() {
        return;
    }
    let (tx, rx) = mpsc::channel::<PointerEvent>();
    let sink: EventSink<PointerEvent> = Arc::new(move |e| {
        let _ = tx.send(e);
    });
    match shared
        .platform
        .start_pointer_watch(Arc::clone(&w.pointer), sink)
    {
        Ok(handle) => {
            *watch = Some(handle);
            // Ends by itself when the watcher (and with it the sender) is dropped.
            let _ = thread::Builder::new()
                .name("krova-pointer-events".into())
                .spawn(move || {
                    while let Ok(ev) = rx.recv() {
                        on_pointer_event(ev);
                    }
                });
        }
        Err(e) => eprintln!("krova: could not watch the cursor: {e}"),
    }
}

/// Called after every settings change (UI or tray).
pub fn settings_changed(shared: &Shared) {
    apply(shared);
}

fn on_pointer_event(ev: PointerEvent) {
    let Some(w) = WAVE4.get() else {
        return;
    };
    match ev {
        PointerEvent::HoverCaption {
            window,
            rect,
            cursor,
        } => {
            let picker = hover_payload(&lock(&w.shared.model), rect, cursor);
            if let Some(picker) = picker {
                *lock(&HOVERED) = Some(Hovered {
                    window,
                    monitor_id: picker.monitor_id.clone(),
                });
                (w.emit)(
                    "hover-picker",
                    serde_json::to_value(&picker).unwrap_or_default(),
                );
            }
        }
        PointerEvent::HoverEnd => {
            (w.emit)("hover-picker-end", serde_json::Value::Null);
        }
        PointerEvent::Focused(_) => {}
        PointerEvent::FocusRefused(_) => {
            FOCUS_REFUSED.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// The picker for a window whose title bar the cursor rests on: the monitor under
/// the cursor and its active layout's zones. `None` off every monitor.
pub fn hover_payload(
    model: &AppModel,
    window_rect: PxRect,
    cursor: (i32, i32),
) -> Option<HoverPicker> {
    let views = model.views();
    let ids = model.connected_ids();
    let i = views
        .iter()
        .position(|v| v.rect.contains(cursor.0, cursor.1))?;
    let layout = views[i].layout?;
    let gap = model.config.settings.zone_gap_px;
    let zones = layout
        .zones
        .iter()
        .filter_map(|z| {
            layout
                .zone_pixels_gapped(z.id, views[i].work, gap)
                .map(|rect| HoverZone { id: z.id, rect })
        })
        .collect();
    Some(HoverPicker {
        monitor_id: ids.get(i)?.clone(),
        window_rect,
        cursor: [cursor.0, cursor.1],
        layout_id: layout.id.clone(),
        zones,
    })
}

/// Where `place_hovered` puts the window: `(target, work area, registry record
/// with the planned rect)`.
fn plan_hover_place(
    model: &AppModel,
    monitor_id: &str,
    zone_id: u32,
) -> Result<(PxRect, PxRect, SnapRecord), String> {
    let views = model.views();
    let ids = model.connected_ids();
    let i = ids
        .iter()
        .position(|id| id == monitor_id)
        .ok_or_else(|| format!("monitor {monitor_id} is not connected"))?;
    let view = &views[i];
    let layout = view
        .layout
        .ok_or_else(|| format!("monitor {monitor_id} has no active layout"))?;
    let target = layout
        .zone_pixels_gapped(zone_id, view.work, model.config.settings.zone_gap_px)
        .ok_or_else(|| format!("zone {zone_id} is not in the active layout"))?;
    let record = SnapRecord {
        monitor_profile_id: monitor_id.to_owned(),
        layout_id: layout.id.clone(),
        zone_id,
        rect: target,
    };
    Ok((target, view.work, record))
}

/// Move the hovered window into `zone_id` of its monitor's active layout and
/// remember it like a drag snap. Returns the frame the window ended up with.
pub fn place_hovered_on(
    shared: &Shared,
    hovered: &Hovered,
    zone_id: u32,
) -> Result<PxRect, String> {
    let (target, work, mut record, animate) = {
        let model = lock(&shared.model);
        let (t, w, r) = plan_hover_place(&model, &hovered.monitor_id, zone_id)?;
        (
            t,
            w,
            r,
            model.config.settings.move_mode == MoveMode::Animate,
        )
    };
    if !shared.platform.is_movable_target(hovered.window) {
        return Err("that window can no longer be moved".to_owned());
    }
    let done = shared
        .platform
        .place_window(hovered.window, target, work, animate)
        .map_err(|e| format!("could not move the window: {e}"))?;
    record.rect = done;
    lock(&shared.registry).insert(WindowKey(hovered.window.0), record);
    Ok(done)
}

/// Put the window under the hover picker into a zone. Async, so the move (about
/// 120 ms when animated) does not block the UI thread.
#[tauri::command]
pub async fn place_hovered(
    app: AppHandle,
    shared: State<'_, Arc<Shared>>,
    zone_id: u32,
) -> Result<(), String> {
    let hovered = lock(&HOVERED)
        .clone()
        .ok_or_else(|| "no window is under the picker".to_owned())?;
    place_hovered_on(&shared, &hovered, zone_id)?;
    *lock(&HOVERED) = None;
    let _ = app.emit("hover-picker-end", ());
    Ok(())
}

// ------------------------------------------------------- check for updates

/// Where "Check for updates" goes. Fixed at compile time; Krova never fetches it
/// itself (no network in the app): the browser opens it, because the user asked.
pub const RELEASES_URL: &str = "https://github.com/nAItiklearn/krova/releases";

/// Open the releases page in the default browser.
pub fn open_releases(shared: &Shared) -> Result<(), String> {
    shared
        .platform
        .open_url(RELEASES_URL)
        .map_err(|e| format!("could not open the releases page: {e}"))
}

#[tauri::command]
pub fn open_releases_page(shared: State<'_, Arc<Shared>>) -> Result<(), String> {
    open_releases(&shared)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use krova_core::{Config, MonitorDescriptor, PxRect};
    use krova_platform::{MonitorInfo, NullPlatform};

    use super::*;
    use crate::model::AppModel;

    pub(crate) fn monitor(hw: &str, x: i32, scale: f64) -> MonitorInfo {
        MonitorInfo {
            descriptor: MonitorDescriptor {
                hardware_id: Some(hw.to_owned()),
                device_path: None,
                rect: PxRect::new(x, 0, 1920, 1080),
            },
            work: PxRect::new(x, 0, 1920, 1040),
            scale,
            label: format!("Display {hw}"),
            primary: x == 0,
        }
    }

    pub(crate) fn shared_with(infos: Vec<MonitorInfo>) -> Shared {
        let mut model = AppModel::new(Config::default(), PathBuf::from("unused.json"), false);
        model.refresh_monitors(infos).unwrap();
        Shared::new(model, Arc::new(NullPlatform))
    }

    #[test]
    fn reserved_edges_shrink_the_work_area_everything_reads() {
        let shared = shared_with(vec![monitor("A", 0, 1.0), monitor("B", 1920, 2.0)]);
        set_reserved(&shared, "monitor-2", ReservedEdges::new(0, 0, 0, 30)).unwrap();
        let model = lock(&shared.model);
        // Monitor 2 is at 200 %: 30 logical px are 60 physical.
        let expected = PxRect::new(1980, 0, 1860, 1040);
        assert_eq!(model.views()[1].work, expected);
        assert_eq!(model.snapshots()[1].work, expected);
        assert_eq!(model.connected()[1].work, expected);
        // Monitor 1 is untouched.
        assert_eq!(model.views()[0].work, PxRect::new(0, 0, 1920, 1040));
    }

    #[test]
    fn reserved_edges_are_sanitised_and_unknown_monitors_refused() {
        let shared = shared_with(vec![monitor("A", 0, 1.0)]);
        set_reserved(&shared, "monitor-1", ReservedEdges::new(9000, 0, 0, 0)).unwrap();
        assert_eq!(
            lock(&shared.model).config.monitors[0].reserved,
            ReservedEdges::new(400, 0, 0, 0)
        );
        let err = set_reserved(&shared, "monitor-7", ReservedEdges::default()).unwrap_err();
        assert_eq!(err, "monitor monitor-7 not found");
        let json = serde_json::to_value(crate::commands::build_state(&shared)).unwrap();
        assert_eq!(json["config"]["monitors"][0]["reserved"]["top"], 400);
    }

    #[test]
    fn pointer_features_need_the_setting_the_capability_and_krova_enabled() {
        let caps = Wave4Capabilities {
            focus_follows_mouse: true,
            hover_picker: true,
            open_url: true,
        };
        let mut s = Settings::default();
        assert_eq!(pointer_wanted(&s, caps), (false, false), "both are opt-in");
        s.focus_follows_mouse = true;
        s.hover_picker = true;
        assert_eq!(pointer_wanted(&s, caps), (true, true));
        assert_eq!(
            pointer_wanted(&s, Wave4Capabilities::default()),
            (false, false),
            "macOS today"
        );
        s.enabled = false;
        assert_eq!(pointer_wanted(&s, caps), (false, false));
    }

    #[test]
    fn the_hover_picker_lists_the_gapped_zones_of_the_monitor_under_the_cursor() {
        let shared = shared_with(vec![monitor("A", 0, 1.0), monitor("B", 1920, 1.0)]);
        let mut model = lock(&shared.model);
        model.config.settings.zone_gap_px = 10;
        let window = PxRect::new(2000, 100, 800, 600);
        let picker = hover_payload(&model, window, (2100, 110)).unwrap();
        assert_eq!(picker.monitor_id, "monitor-2");
        assert_eq!(picker.window_rect, window);
        assert_eq!(picker.cursor, [2100, 110]);
        let active = model.config.monitors[1].active().unwrap().clone();
        assert_eq!(picker.layout_id, active.id);
        assert_eq!(picker.zones.len(), active.zones.len());
        let work = model.views()[1].work;
        for z in &picker.zones {
            assert_eq!(
                Some(z.rect),
                active.zone_pixels_gapped(z.id, work, 10),
                "zone {}",
                z.id
            );
        }
        assert!(hover_payload(&model, window, (99_999, 0)).is_none());
        let json = serde_json::to_value(&picker).unwrap();
        assert_eq!(json["monitor_id"], "monitor-2");
        assert!(json["zones"][0]["rect"]["w"].is_number());
    }

    #[test]
    fn placing_the_hovered_window_checks_monitor_zone_and_window() {
        let shared = shared_with(vec![monitor("A", 0, 1.0)]);
        let first_zone = {
            let model = lock(&shared.model);
            let (target, work, record) = plan_hover_place(&model, "monitor-1", 1).unwrap();
            assert_eq!(work, PxRect::new(0, 0, 1920, 1040));
            assert_eq!(record.zone_id, 1);
            assert_eq!(record.rect, target);
            assert!(plan_hover_place(&model, "monitor-1", 999)
                .unwrap_err()
                .contains("zone 999"));
            assert!(plan_hover_place(&model, "monitor-5", 1)
                .unwrap_err()
                .contains("not connected"));
            target
        };
        assert!(first_zone.w > 0);
        // The null platform can move nothing: refused before any move is tried.
        let hovered = Hovered {
            window: WindowId(42),
            monitor_id: "monitor-1".into(),
        };
        let err = place_hovered_on(&shared, &hovered, 1).unwrap_err();
        assert_eq!(err, "that window can no longer be moved");
        assert!(lock(&shared.registry).get(WindowKey(42)).is_none());
    }

    #[test]
    fn the_releases_page_is_a_fixed_https_address() {
        assert!(krova_platform::pointer::is_safe_https_url(RELEASES_URL));
        assert!(RELEASES_URL.starts_with("https://github.com/"));
        let shared = shared_with(vec![]);
        // The null platform cannot open a browser: a readable error, no panic.
        let err = open_releases(&shared).unwrap_err();
        assert!(err.starts_with("could not open the releases page"), "{err}");
    }

    #[test]
    fn the_state_carries_the_wave4_fields_at_the_top_level() {
        let shared = shared_with(vec![monitor("A", 0, 1.0)]);
        let json = serde_json::to_value(crate::commands::build_state(&shared)).unwrap();
        assert_eq!(json["wave4_capabilities"]["focus_follows_mouse"], false);
        assert_eq!(json["pointer"]["active"], false);
        assert!(json["pointer"]["focus_refused"].is_number());
        let s = &json["config"]["settings"];
        assert_eq!(s["focus_follows_mouse"], false);
        assert_eq!(s["focus_follows_mouse_delay_ms"], 350);
        assert_eq!(s["hover_picker"], false);
        // Settings changes are harmless before `start` (no watcher to follow them).
        crate::commands::change_settings(&shared, |s| {
            s.focus_follows_mouse = true;
            s.focus_follows_mouse_delay_ms = 5;
        })
        .unwrap();
        let s = lock(&shared.model).config.settings.clone();
        assert!(s.focus_follows_mouse);
        assert_eq!(s.focus_follows_mouse_delay_ms, 150);
    }
}
