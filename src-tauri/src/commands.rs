//! Tauri commands: the typed seam between the Svelte UI and the Rust core.
//!
//! Every command returns the new `AppState` (or the edited `Layout`) and reports
//! failure as a plain string the UI can show. JS passes camelCase argument names;
//! Tauri maps them to the snake_case parameters below.

use std::sync::Arc;

use krova_core::layout_io;
use krova_core::{templates, Axis, Layout, Settings};
use krova_platform::Permission;
use serde::Deserialize;
use tauri::{AppHandle, Emitter, State};

use crate::engine;
use crate::model::{lock, sanitize_settings, AppState, Shared};
use crate::tray;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AxisArg {
    Vertical,
    Horizontal,
}

impl From<AxisArg> for Axis {
    fn from(a: AxisArg) -> Self {
        match a {
            AxisArg::Vertical => Axis::Vertical,
            AxisArg::Horizontal => Axis::Horizontal,
        }
    }
}

pub fn build_state(shared: &Shared) -> AppState {
    let autostart = shared.platform.autostart_enabled().unwrap_or(false);
    let permission = shared.platform.permission();
    let (shortcuts, layout_switcher) = {
        let features = lock(&shared.features);
        (features.shortcuts.clone(), features.layout_switcher.clone())
    };
    let capabilities = shared.platform.capabilities();
    let mut state = lock(&shared.model).state(
        autostart,
        permission,
        shortcuts,
        layout_switcher,
        capabilities,
    );
    // --- wave3 ---
    crate::wave3::fill_state(shared, &mut state);
    // --- end wave3 ---
    // --- wave4 ---
    state.wave4 = crate::wave4::state_extras(shared);
    // --- end wave4 ---
    state
}

/// Persist the config, refresh the tray, tell every listener, return the state.
pub fn commit(app: &AppHandle, shared: &Shared) -> Result<AppState, String> {
    lock(&shared.model)
        .persist()
        .map_err(|e| format!("could not save the config: {e}"))?;
    let state = build_state(shared);
    tray::sync(app, &state);
    let _ = app.emit("state-changed", &state);
    Ok(state)
}

/// Change settings and apply their side effects (modifier on the live input
/// monitor, launch at login). Used by both the UI and the tray.
pub fn change_settings(shared: &Shared, edit: impl FnOnce(&mut Settings)) -> Result<(), String> {
    let (old, new) = {
        let mut model = lock(&shared.model);
        let old = model.config.settings.clone();
        let mut new = old.clone();
        edit(&mut new);
        let new = sanitize_settings(new);
        model.config.settings = new.clone();
        (old, new)
    };
    if old.modifier != new.modifier {
        if let Some(input) = lock(&shared.input).as_ref() {
            input.set_modifier(new.modifier);
        }
    }
    // The shortcuts follow the master switch, their own switches and the chord.
    if (
        old.enabled,
        old.keyboard_shortcuts,
        old.layout_switcher_shortcut,
        old.shortcut_modifier,
    ) != (
        new.enabled,
        new.keyboard_shortcuts,
        new.layout_switcher_shortcut,
        new.shortcut_modifier,
    ) {
        engine::apply_shortcuts(shared);
    }
    // --- wave3 ---
    crate::wave3::on_settings_changed(shared, &old, &new);
    // --- end wave3 ---
    if old.launch_at_login != new.launch_at_login {
        if let Err(e) = shared.platform.set_autostart(new.launch_at_login) {
            lock(&shared.model).config.settings.launch_at_login = old.launch_at_login;
            return Err(format!("could not change launch at login: {e}"));
        }
    }
    // --- wave4 ---
    crate::wave4::settings_changed(shared);
    // --- end wave4 ---
    Ok(())
}

#[tauri::command]
pub fn get_state(shared: State<'_, Arc<Shared>>) -> Result<AppState, String> {
    if let Ok(infos) = shared.platform.monitors() {
        lock(&shared.model)
            .refresh_monitors(infos)
            .map_err(|e| format!("could not save the config: {e}"))?;
    }
    // macOS: the input monitor cannot start before Accessibility is granted, so
    // try again now that it is (a restart may still be needed on some systems).
    if lock(&shared.input).is_none() && shared.platform.permission() == Permission::Granted {
        engine::retry_input(&shared);
    }
    Ok(build_state(&shared))
}

#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    shared: State<'_, Arc<Shared>>,
    settings: Settings,
) -> Result<AppState, String> {
    change_settings(&shared, |s| *s = settings)?;
    commit(&app, &shared)
}

#[tauri::command]
pub fn save_layout(
    app: AppHandle,
    shared: State<'_, Arc<Shared>>,
    monitor_id: String,
    layout: Layout,
) -> Result<AppState, String> {
    lock(&shared.model)
        .config
        .save_layout(&monitor_id, layout)
        .map_err(|e| e.to_string())?;
    commit(&app, &shared)
}

#[tauri::command]
pub fn delete_layout(
    app: AppHandle,
    shared: State<'_, Arc<Shared>>,
    monitor_id: String,
    layout_id: String,
) -> Result<AppState, String> {
    lock(&shared.model)
        .config
        .delete_layout(&monitor_id, &layout_id)
        .map_err(|e| e.to_string())?;
    commit(&app, &shared)
}

#[tauri::command]
pub fn set_active_layout(
    app: AppHandle,
    shared: State<'_, Arc<Shared>>,
    monitor_id: String,
    layout_id: String,
) -> Result<AppState, String> {
    lock(&shared.model)
        .config
        .set_active_layout(&monitor_id, &layout_id)
        .map_err(|e| e.to_string())?;
    commit(&app, &shared)
}

/// Reorder a monitor's layouts: the layout ends up at `to_index`. The active layout
/// is unchanged. A refused move (unknown ids, index out of range) changes nothing.
#[tauri::command]
pub fn move_layout(
    app: AppHandle,
    shared: State<'_, Arc<Shared>>,
    monitor_id: String,
    layout_id: String,
    to_index: usize,
) -> Result<AppState, String> {
    lock(&shared.model)
        .config
        .move_layout(&monitor_id, &layout_id, to_index)
        .map_err(|e| e.to_string())?;
    commit(&app, &shared)
}

/// Put a layout back at `index` (the undo of `delete_layout`), optionally making it
/// the active layout in the same step. Never replaces a layout with the same id.
#[tauri::command]
pub fn insert_layout(
    app: AppHandle,
    shared: State<'_, Arc<Shared>>,
    monitor_id: String,
    layout: Layout,
    index: usize,
    activate: bool,
) -> Result<AppState, String> {
    lock(&shared.model)
        .config
        .insert_layout(&monitor_id, layout, index, activate)
        .map_err(|e| e.to_string())?;
    commit(&app, &shared)
}

/// The monitor the layout switcher is for: the one under the cursor when its
/// shortcut was last pressed, if that monitor is still connected; otherwise the
/// primary (or first connected) monitor. `None` if the switcher was never opened or
/// no monitor is connected.
pub fn switcher_monitor_of(shared: &Shared) -> Option<String> {
    let stored = lock(&shared.switcher_monitor).clone()?;
    let model = lock(&shared.model);
    if model.connected_ids().contains(&stored) {
        Some(stored)
    } else {
        model.monitor_for_switcher(None)
    }
}

#[tauri::command]
pub fn switcher_monitor(shared: State<'_, Arc<Shared>>) -> Option<String> {
    switcher_monitor_of(&shared)
}

#[tauri::command]
pub fn list_templates() -> Vec<Layout> {
    templates()
}

/// The layout as a portable file (pretty JSON envelope). The layout is validated
/// first, so an invalid one is never exported. The UI decides where to save it.
#[tauri::command]
pub fn export_layout(layout: Layout) -> Result<String, String> {
    layout_io::export_json(&layout).map_err(|e| e.to_string())
}

/// Parse, validate and store a layout file on a monitor. A layout id that is
/// already taken on that monitor gets a numeric suffix; the 64-layouts cap and all
/// layout limits apply exactly as for `save_layout`.
pub fn import_into(shared: &Shared, monitor_id: &str, json: &str) -> Result<String, String> {
    let mut model = lock(&shared.model);
    let profile = model
        .config
        .monitors
        .iter()
        .find(|m| m.id == monitor_id)
        .ok_or_else(|| format!("monitor {monitor_id} not found"))?;
    let taken: Vec<&str> = profile.layouts.iter().map(|l| l.id.as_str()).collect();
    let layout = layout_io::import_json(json, &taken).map_err(|e| e.to_string())?;
    let id = layout.id.clone();
    model
        .config
        .save_layout(monitor_id, layout)
        .map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub fn import_layout(
    app: AppHandle,
    shared: State<'_, Arc<Shared>>,
    monitor_id: String,
    json: String,
) -> Result<AppState, String> {
    import_into(&shared, &monitor_id, &json)?;
    commit(&app, &shared)
}

#[tauri::command]
pub fn edit_split_zone(
    mut layout: Layout,
    zone_id: u32,
    axis: AxisArg,
    at: f64,
) -> Result<Layout, String> {
    // The webview is not trusted: check the incoming layout before editing it.
    layout.validate().map_err(|e| e.to_string())?;
    layout
        .split_zone(zone_id, axis.into(), at)
        .map_err(|e| e.to_string())?;
    Ok(layout)
}

#[tauri::command]
pub fn edit_remove_zone(mut layout: Layout, zone_id: u32) -> Result<Layout, String> {
    layout.validate().map_err(|e| e.to_string())?;
    layout.remove_zone(zone_id).map_err(|e| e.to_string())?;
    Ok(layout)
}

#[tauri::command]
pub fn edit_move_divider(
    mut layout: Layout,
    axis: AxisArg,
    from: f64,
    to: f64,
) -> Result<Layout, String> {
    layout.validate().map_err(|e| e.to_string())?;
    layout
        .move_divider(axis.into(), from, to)
        .map_err(|e| e.to_string())?;
    Ok(layout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AppModel;
    use krova_core::{Config, MonitorDescriptor, PxRect, Rect};
    use krova_platform::NullPlatform;
    use std::path::PathBuf;

    #[test]
    fn axis_arguments_use_the_documented_spelling() {
        let v: AxisArg = serde_json::from_str("\"vertical\"").unwrap();
        assert!(matches!(Axis::from(v), Axis::Vertical));
        let h: AxisArg = serde_json::from_str("\"horizontal\"").unwrap();
        assert!(matches!(Axis::from(h), Axis::Horizontal));
        assert!(serde_json::from_str::<AxisArg>("\"diagonal\"").is_err());
    }

    #[test]
    fn edit_commands_return_the_edited_layout_or_a_readable_error() {
        let halves = templates().remove(0);
        let split = edit_split_zone(halves.clone(), 1, AxisArg::Horizontal, 0.5).unwrap();
        assert_eq!(split.zones.len(), 3);

        let err = edit_split_zone(halves.clone(), 1, AxisArg::Vertical, 0.01).unwrap_err();
        assert!(err.contains("minimum size"), "{err}");

        let merged = edit_remove_zone(templates().remove(2), 1).unwrap();
        assert_eq!(merged.zones.len(), 3);
        let one = Layout::new("one", "One", &[Rect::new(0.0, 0.0, 1.0, 1.0)]);
        assert!(edit_remove_zone(one, 1)
            .unwrap_err()
            .contains("at least one zone"));

        let moved = edit_move_divider(halves, AxisArg::Vertical, 0.5, 0.3).unwrap();
        assert!((moved.zones[0].rect.w - 0.3).abs() < 1e-9);
    }

    #[test]
    fn layouts_arrive_as_the_ui_sends_them() {
        let json = r#"{"id":"x","name":"X","zones":[{"id":1,"rect":{"x":0,"y":0,"w":1,"h":1}}]}"#;
        let layout: Layout = serde_json::from_str(json).unwrap();
        assert!(layout.validate().is_ok());
    }

    fn shared_with_one_monitor() -> Shared {
        let mut model = AppModel::new(Config::default(), PathBuf::from("unused.json"), false);
        model.config.resolve_monitors(&[MonitorDescriptor {
            hardware_id: Some("A".to_owned()),
            device_path: None,
            rect: PxRect::new(0, 0, 1920, 1080),
        }]);
        Shared::new(model, Arc::new(NullPlatform))
    }

    fn layout_ids(shared: &Shared) -> Vec<String> {
        lock(&shared.model).config.monitors[0]
            .layouts
            .iter()
            .map(|l| l.id.clone())
            .collect()
    }

    #[test]
    fn export_wraps_a_valid_layout_in_the_versioned_envelope() {
        let text = export_layout(templates().remove(1)).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["krova_layout"], 1);
        assert_eq!(v["layout"]["id"], "thirds");
        assert!(text.len() < layout_io::MAX_LAYOUT_JSON_BYTES);
    }

    #[test]
    fn export_refuses_an_invalid_layout_with_a_readable_message() {
        let mut bad = templates().remove(0);
        bad.zones[0].rect = Rect::new(0.9, 0.0, 0.5, 1.0);
        let err = export_layout(bad).unwrap_err();
        assert!(err.contains("not valid"), "{err}");
        let empty = Layout {
            id: "e".into(),
            name: "e".into(),
            zones: vec![],
        };
        assert!(export_layout(empty).unwrap_err().contains("no zones"));
    }

    #[test]
    fn import_stores_the_layout_on_the_monitor() {
        let shared = shared_with_one_monitor();
        let json = export_layout(Layout::new(
            "mine",
            "Mine",
            &[Rect::new(0.0, 0.0, 0.3, 1.0), Rect::new(0.3, 0.0, 0.7, 1.0)],
        ))
        .unwrap();
        let before = layout_ids(&shared).len();
        let id = import_into(&shared, "monitor-1", &json).unwrap();
        assert_eq!(id, "mine");
        let ids = layout_ids(&shared);
        assert_eq!(ids.len(), before + 1);
        assert!(ids.contains(&"mine".to_owned()));
    }

    #[test]
    fn importing_a_taken_id_adds_a_suffix_instead_of_replacing() {
        let shared = shared_with_one_monitor();
        // "halves" is a built-in layout of every new monitor.
        let json = export_layout(templates().remove(0)).unwrap();
        let before = layout_ids(&shared).len();
        let id = import_into(&shared, "monitor-1", &json).unwrap();
        assert_eq!(id, "halves-2");
        let id = import_into(&shared, "monitor-1", &json).unwrap();
        assert_eq!(id, "halves-3");
        assert_eq!(layout_ids(&shared).len(), before + 2);
        // The original is untouched.
        assert_eq!(
            lock(&shared.model).config.monitors[0]
                .layouts
                .iter()
                .find(|l| l.id == "halves")
                .unwrap()
                .name,
            "Halves"
        );
    }

    #[test]
    fn import_refuses_bad_input_and_changes_nothing() {
        let shared = shared_with_one_monitor();
        let before = lock(&shared.model).config.clone();
        let good = export_layout(templates().remove(1)).unwrap();
        assert!(import_into(&shared, "monitor-9", &good)
            .unwrap_err()
            .contains("not found"));
        for bad in [
            "".to_owned(),
            "not json".to_owned(),
            r#"{"krova_layout":2,"layout":{}}"#.to_owned(),
            good.replace("\"krova_layout\": 1", "\"krova_layout\": 7"),
            " ".repeat(layout_io::MAX_LAYOUT_JSON_BYTES + 1),
        ] {
            assert!(
                import_into(&shared, "monitor-1", &bad).is_err(),
                "{bad:.40}"
            );
        }
        assert_eq!(lock(&shared.model).config, before);
    }

    #[test]
    fn import_respects_the_layouts_per_monitor_cap() {
        let shared = shared_with_one_monitor();
        let json = export_layout(Layout::new(
            "extra",
            "Extra",
            &[Rect::new(0.0, 0.0, 1.0, 1.0)],
        ))
        .unwrap();
        let mut accepted = 0;
        for _ in 0..krova_core::config::MAX_LAYOUTS_PER_MONITOR + 5 {
            if import_into(&shared, "monitor-1", &json).is_ok() {
                accepted += 1;
            }
        }
        let total = layout_ids(&shared).len();
        assert_eq!(total, krova_core::config::MAX_LAYOUTS_PER_MONITOR);
        assert_eq!(accepted, total - templates().len());
        let err = import_into(&shared, "monitor-1", &json).unwrap_err();
        assert!(err.contains("at most 64 layouts"), "{err}");
    }

    #[test]
    fn changing_shortcut_settings_is_harmless_without_platform_support() {
        // The null platform has no shortcuts: the change is stored, nothing registers.
        let shared = shared_with_one_monitor();
        change_settings(&shared, |s| {
            s.shortcut_modifier = krova_core::ShortcutModifier::CtrlAlt;
            s.keyboard_shortcuts = false;
            s.zone_gap_px = 999;
        })
        .unwrap();
        let s = lock(&shared.model).config.settings.clone();
        assert_eq!(s.shortcut_modifier, krova_core::ShortcutModifier::CtrlAlt);
        assert!(!s.keyboard_shortcuts);
        assert_eq!(s.zone_gap_px, 32, "sanitised on the way in");
        assert!(!lock(&shared.features).shortcuts.active);
        assert!(build_state(&shared).shortcuts.failed.is_empty());
        let caps = build_state(&shared).capabilities;
        assert!(!caps.shortcuts && !caps.window_memory);
    }

    #[test]
    fn edit_commands_refuse_layouts_that_were_never_valid() {
        // Duplicate ids, an out-of-range rectangle and a hostile id near u32::MAX.
        let mut dup = templates().remove(0);
        dup.zones[1].id = 1;
        let mut off = templates().remove(0);
        off.zones[0].rect = Rect::new(0.9, 0.0, 0.5, 1.0);
        let mut max_id = Layout::new("m", "m", &[Rect::new(0.0, 0.0, 1.0, 1.0)]);
        max_id.zones[0].id = u32::MAX;

        for bad in [dup, off] {
            assert!(edit_split_zone(bad.clone(), 1, AxisArg::Vertical, 0.25).is_err());
            assert!(edit_remove_zone(bad.clone(), 1).is_err());
            assert!(edit_move_divider(bad, AxisArg::Vertical, 0.5, 0.4).is_err());
        }
        // A valid layout whose ids are exhausted is refused cleanly, not with a panic.
        let err = edit_split_zone(max_id, u32::MAX, AxisArg::Vertical, 0.5).unwrap_err();
        assert!(err.contains("another zone"), "{err}");
    }

    fn connected(hw: &str, x: i32, primary: bool) -> krova_platform::MonitorInfo {
        krova_platform::MonitorInfo {
            descriptor: MonitorDescriptor {
                hardware_id: Some(hw.to_owned()),
                device_path: None,
                rect: PxRect::new(x, 0, 1920, 1080),
            },
            work: PxRect::new(x, 0, 1920, 1040),
            scale: 1.0,
            label: format!("Display {hw}"),
            primary,
        }
    }

    fn shared_with_monitors(infos: Vec<krova_platform::MonitorInfo>) -> Shared {
        let mut model = AppModel::new(Config::default(), PathBuf::from("unused.json"), false);
        model.refresh_monitors(infos).unwrap();
        Shared::new(model, Arc::new(NullPlatform))
    }

    #[test]
    fn the_switcher_monitor_is_known_once_opened_and_follows_what_is_connected() {
        let shared =
            shared_with_monitors(vec![connected("A", 0, true), connected("B", 1920, false)]);
        assert_eq!(switcher_monitor_of(&shared), None, "never opened");

        *lock(&shared.switcher_monitor) = Some("monitor-2".to_owned());
        assert_eq!(switcher_monitor_of(&shared).as_deref(), Some("monitor-2"));

        // The monitor it was opened for has been unplugged since: the primary one.
        lock(&shared.model)
            .refresh_monitors(vec![connected("A", 0, true)])
            .unwrap();
        assert_eq!(switcher_monitor_of(&shared).as_deref(), Some("monitor-1"));

        lock(&shared.model).refresh_monitors(vec![]).unwrap();
        assert_eq!(switcher_monitor_of(&shared), None, "nothing is connected");
    }

    #[test]
    fn reordering_and_undoing_a_delete_go_through_the_same_validation_as_saving() {
        // The commands wrap these calls and turn the error into the string asserted here.
        let shared = shared_with_monitors(vec![connected("A", 0, true)]);
        let mut model = lock(&shared.model);
        let text = |r: Result<(), krova_core::ProfileError>| r.unwrap_err().to_string();
        assert_eq!(
            text(model.config.move_layout("monitor-1", "halves", 9)),
            "position 9 is out of range (the monitor has 4 layouts)"
        );
        assert_eq!(
            text(model.config.move_layout("monitor-9", "halves", 0)),
            "monitor monitor-9 not found"
        );
        let dup = templates().remove(0);
        assert_eq!(
            text(model.config.insert_layout("monitor-1", dup, 0, true)),
            "layout halves already exists"
        );
        let deleted = templates().remove(1);
        model.config.delete_layout("monitor-1", "thirds").unwrap();
        model
            .config
            .insert_layout("monitor-1", deleted, 1, false)
            .unwrap();
        let ids: Vec<&str> = model.config.monitors[0]
            .layouts
            .iter()
            .map(|l| l.id.as_str())
            .collect();
        assert_eq!(ids, ["halves", "thirds", "grid", "priority"]);
    }

    #[test]
    fn the_switcher_shortcut_setting_is_applied_without_platform_support() {
        // The null platform has no shortcuts: the switch is stored, nothing registers,
        // and the switcher's status stays "inactive, nothing failed".
        let shared = shared_with_monitors(vec![connected("A", 0, true)]);
        change_settings(&shared, |s| s.layout_switcher_shortcut = false).unwrap();
        assert!(!lock(&shared.model).config.settings.layout_switcher_shortcut);
        let state = build_state(&shared);
        assert!(!state.layout_switcher.active && state.layout_switcher.failed.is_empty());
        let json = serde_json::to_value(&state).unwrap();
        assert_eq!(json["layout_switcher"]["active"], false);
        assert_eq!(
            json["config"]["settings"]["layout_switcher_shortcut"],
            false
        );
    }
}
