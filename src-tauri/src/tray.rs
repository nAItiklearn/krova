//! Tray / menu-bar icon: Enable, Edit layouts, Settings, Launch at login, Quit.

use std::sync::Arc;

use tauri::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};

use crate::commands::{change_settings, commit};
use crate::model::{AppState, Shared};
use crate::ui_window;

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
    // --- wave4 ---
    let updates = MenuItem::with_id(app, "updates", "Check for updates…", true, None::<&str>)?;
    // --- end wave4 ---
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
            &updates, // --- wave4 ---
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
        "edit" => ui_window::show(app, Some("editor")),
        "settings" => ui_window::show(app, Some("settings")),
        // --- wave4 ---
        "updates" => {
            if let Err(e) = crate::wave4::open_releases(&shared) {
                eprintln!("krova: {e}");
            }
        }
        // --- end wave4 ---
        // An explicit exit request: lib.rs lets `Some(code)` through and only
        // vetoes the implicit "last window closed" exit.
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
