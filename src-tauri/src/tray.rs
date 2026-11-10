//! Tray / menu-bar icon: Enable, Pause arranging, Quick panel, Edit layouts,
//! Settings, Launch at login, Quit. A left click opens the quick panel; while
//! auto-arrange is paused the icon is drawn dimmed.

use std::sync::Arc;

use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::commands::{change_settings, commit};
use crate::engine::{self, Msg};
use crate::model::{AppState, Shared};
use crate::{popups, ui_window};

/// Handles to the checkable entries and the two icons, so they can follow the state.
pub struct TrayItems {
    enabled: CheckMenuItem<Wry>,
    paused: CheckMenuItem<Wry>,
    autostart: CheckMenuItem<Wry>,
    tray: TrayIcon<Wry>,
    normal: Option<Image<'static>>,
    dimmed: Option<Image<'static>>,
}

/// The icon at 40 % opacity: the "paused" look.
pub fn dim(rgba: &[u8]) -> Vec<u8> {
    rgba.chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2], (u16::from(p[3]) * 2 / 5) as u8])
        .collect()
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
    let paused = CheckMenuItem::with_id(
        app,
        "pause",
        "Pause arranging",
        true,
        state.paused,
        None::<&str>,
    )?;
    let panel = MenuItem::with_id(app, "panel", "Quick panel", true, None::<&str>)?;
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
            &paused,
            &sep_top,
            &panel,
            &edit,
            &settings,
            &autostart,
            &sep_bottom,
            &quit,
        ],
    )?;

    let normal = app.default_window_icon().map(|i| i.clone().to_owned());
    let dimmed = normal
        .as_ref()
        .map(|i| Image::new_owned(dim(i.rgba()), i.width(), i.height()));
    let mut tray = TrayIconBuilder::with_id("krova")
        .tooltip("Krova")
        .menu(&menu)
        // The left click is the panel; the menu stays on the right click.
        .show_menu_on_left_click(false)
        .on_menu_event(handle_menu_event)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                popups::show(tray.app_handle(), "panel");
            }
        });
    if let Some(icon) = normal.clone() {
        tray = tray.icon(icon);
    }
    let tray = tray.build(app)?;

    app.manage(TrayItems {
        enabled,
        paused,
        autostart,
        tray,
        normal,
        dimmed,
    });
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
        "pause" => engine::notify(&shared, Msg::SetPaused(!shared.is_paused())),
        "autostart" => {
            let result = change_settings(&shared, |s| s.launch_at_login = !s.launch_at_login)
                .and_then(|()| commit(app, &shared).map(|_| ()));
            if let Err(e) = result {
                eprintln!("krova: {e}");
            }
        }
        "panel" => popups::show(app, "panel"),
        "edit" => ui_window::show(app, Some("editor")),
        "settings" => ui_window::show(app, Some("settings")),
        // An explicit exit request: lib.rs lets `Some(code)` through and only
        // vetoes the implicit "last window closed" exit.
        "quit" => app.exit(0),
        _ => {}
    }
}

/// Keep the checkmarks and the icon in step with the real state.
pub fn sync(app: &AppHandle, state: &AppState) {
    if let Some(items) = app.try_state::<TrayItems>() {
        let _ = items.enabled.set_checked(state.config.settings.enabled);
        let _ = items.paused.set_checked(state.paused);
        let _ = items.autostart.set_checked(state.autostart);
        let icon = if state.paused {
            items.dimmed.clone()
        } else {
            items.normal.clone()
        };
        if icon.is_some() {
            let _ = items.tray.set_icon(icon);
        }
        let _ = items.tray.set_tooltip(Some(if state.paused {
            "Krova (paused)"
        } else {
            "Krova"
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::dim;

    #[test]
    fn the_dimmed_icon_keeps_the_colour_and_lowers_the_alpha() {
        let px = [10, 20, 30, 255, 1, 2, 3, 0];
        assert_eq!(dim(&px), [10, 20, 30, 102, 1, 2, 3, 0]);
    }
}
