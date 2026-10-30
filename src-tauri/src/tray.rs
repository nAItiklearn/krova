//! Tray / menu-bar icon: Enable, Edit layouts, Settings, Launch at login, Quit.

use std::sync::Arc;

use tauri::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, Wry};

use crate::commands::{change_settings, commit};
use crate::model::{AppState, Shared};

/// Handles to the two checkable entries, so they can follow the real state.
pub struct TrayItems {
    enabled: CheckMenuItem<Wry>,
    autostart: CheckMenuItem<Wry>,
}

pub fn build(app: &AppHandle, state: &AppState) -> tauri::Result<()> {
    let enabled = CheckMenuItem::with_id(
        app,
        "enabled",
        "Enabled",
        true,
        state.config.settings.enabled,
        None::<&str>,
    )?;
    let edit = MenuItem::with_id(app, "edit", "Edit layouts…", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let autostart = CheckMenuItem::with_id(
        app,
        "autostart",
        "Launch at login",
        true,
        state.autostart,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Quit Krova", true, None::<&str>)?;
    let sep_top = PredefinedMenuItem::separator(app)?;
    let sep_bottom = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[
            &enabled,
            &sep_top,
            &edit,
            &settings,
            &autostart,
            &sep_bottom,
            &quit,
        ],
    )?;

    let mut tray = TrayIconBuilder::with_id("krova")
        .tooltip("Krova")
        .menu(&menu)
        .on_menu_event(handle_menu_event);
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;

    app.manage(TrayItems { enabled, autostart });
    Ok(())
}

fn handle_menu_event(app: &AppHandle, event: MenuEvent) {
    let shared = app.state::<Arc<Shared>>();
    match event.id.as_ref() {
        "enabled" => {
            let result = change_settings(&shared, |s| s.enabled = !s.enabled)
                .and_then(|()| commit(app, &shared).map(|_| ()));
            if let Err(e) = result {
                eprintln!("krova: {e}");
            }
        }
        "autostart" => {
            let result = change_settings(&shared, |s| s.launch_at_login = !s.launch_at_login)
                .and_then(|()| commit(app, &shared).map(|_| ()));
            if let Err(e) = result {
                eprintln!("krova: {e}");
            }
        }
        "edit" => show_window(app, Some("editor")),
        "settings" => show_window(app, Some("settings")),
        "quit" => app.exit(0),
        _ => {}
    }
}

/// Keep the checkmarks in step with the real state (also after UI changes).
pub fn sync(app: &AppHandle, state: &AppState) {
    if let Some(items) = app.try_state::<TrayItems>() {
        let _ = items.enabled.set_checked(state.config.settings.enabled);
        let _ = items.autostart.set_checked(state.autostart);
    }
}

/// Bring the main window forward, optionally switching its view.
pub fn show_window(app: &AppHandle, view: Option<&str>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        if let Some(view) = view {
            let _ = app.emit("navigate", view);
        }
    }
}
