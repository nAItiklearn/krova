//! Tauri commands: the typed seam between the Svelte UI and the Rust core.
//!
//! Every command returns the new `AppState` (or the edited `Layout`) and reports
//! failure as a plain string the UI can show. JS passes camelCase argument names;
//! Tauri maps them to the snake_case parameters below.

use std::sync::Arc;

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
    lock(&shared.model).state(autostart, permission)
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
    if old.launch_at_login != new.launch_at_login {
        if let Err(e) = shared.platform.set_autostart(new.launch_at_login) {
            lock(&shared.model).config.settings.launch_at_login = old.launch_at_login;
            return Err(format!("could not change launch at login: {e}"));
        }
    }
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
        engine::start(&shared);
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

#[tauri::command]
pub fn list_templates() -> Vec<Layout> {
    templates()
}

#[tauri::command]
pub fn edit_split_zone(
    mut layout: Layout,
    zone_id: u32,
    axis: AxisArg,
    at: f64,
) -> Result<Layout, String> {
    layout
        .split_zone(zone_id, axis.into(), at)
        .map_err(|e| e.to_string())?;
    Ok(layout)
}

#[tauri::command]
pub fn edit_remove_zone(mut layout: Layout, zone_id: u32) -> Result<Layout, String> {
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
    layout
        .move_divider(axis.into(), from, to)
        .map_err(|e| e.to_string())?;
    Ok(layout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use krova_core::Rect;

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
}
