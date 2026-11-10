//! Krova desktop app: tray, settings/editor window and the drag-to-snap engine.
//!
//! Privacy by construction: no network access, no telemetry, and window titles
//! are never read, logged or stored.

mod actions;
mod commands;
#[cfg(all(test, windows))]
mod desktop_reconnect_tests;
#[cfg(all(test, windows))]
mod desktop_tests;
mod engine;
// --- wave4 ---
mod i18n;
// --- end wave4 ---
mod model;
// --- notch ---
mod notch;
// --- notch ---
mod tray;
mod ui_window;
// --- wave3 ---
mod wave3;
// --- end wave3 ---
// --- wave4 ---
mod wave4;
// --- end wave4 ---

use std::sync::Arc;

use krova_core::{Config, LoadStatus};
use krova_platform::Platform;
use tauri::{Manager, RunEvent, WindowEvent};

use model::{AppModel, Shared};

pub fn run() {
    let app = tauri::Builder::default()
        // A second launch just brings the first one forward; two input hooks
        // would snap every window twice.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            ui_window::show(app, Some(ui_window::view_from_args(&args)));
        }))
        .setup(setup)
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::save_settings,
            commands::save_layout,
            commands::delete_layout,
            commands::set_active_layout,
            commands::move_layout,
            commands::insert_layout,
            commands::switcher_monitor,
            commands::list_templates,
            commands::export_layout,
            commands::import_layout,
            commands::edit_split_zone,
            commands::edit_remove_zone,
            commands::edit_move_divider,
            ui_window::take_pending_view,
            // --- wave3 ---
            wave3::cmd::toggle_focus_dim,
            wave3::cmd::toggle_calm_desk,
            wave3::cmd::capture_workspace,
            wave3::cmd::rename_workspace,
            wave3::cmd::delete_workspace,
            wave3::cmd::restore_workspace,
            // --- end wave3 ---
            // --- wave4 ---
            wave4::set_reserved_edges,
            wave4::place_hovered,
            wave4::open_releases_page,
            // --- end wave4 ---
            // --- notch ---
            notch::hide_panel,
            notch::panel_anchor,
            // --- notch ---
        ])
        .on_window_event(|window, event| match event {
            // --- notch --- (the quick panel hides instead of closing; see notch.rs)
            _ if notch::on_window_event(window, event) => {}
            // --- notch ---
            // The tray keeps the app alive. By default closing the window destroys
            // it so the embedded browser exits; `show` rebuilds it on demand.
            WindowEvent::CloseRequested { api, .. } if !ui_window::DESTROY_ON_CLOSE => {
                api.prevent_close();
                let _ = window.hide();
            }
            WindowEvent::Destroyed => ui_window::destroyed(window.app_handle()),
            _ => {}
        })
        .build(tauri::generate_context!())
        .expect("error while building Krova");

    app.run(|_app, event| {
        // Closing the last window asks to exit with no code; the tray keeps the
        // app running. `Quit` calls `app.exit(0)`, which carries a code and passes.
        if let RunEvent::ExitRequested {
            api, code: None, ..
        } = event
        {
            api.prevent_exit();
        }
    });
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // Menu-bar app: no Dock icon on macOS.
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);

    let platform: Arc<dyn Platform> = Arc::from(krova_platform::default_platform());

    let path = app.path().app_data_dir()?.join("config.json");
    let (config, persist) = match Config::load(&path) {
        Ok((config, status)) => {
            if status == LoadStatus::Recovered {
                eprintln!("krova: the config was unreadable; it was kept as config.json.bak");
            }
            (config, true)
        }
        Err(e) => {
            // For example a config written by a newer version: run on defaults
            // but leave the file alone.
            eprintln!("krova: {e}; using defaults and not saving");
            (Config::default(), false)
        }
    };

    let mut model = AppModel::new(config, path, persist);
    match platform.monitors() {
        Ok(infos) => {
            if let Err(e) = model.refresh_monitors(infos) {
                eprintln!("krova: could not save the config: {e}");
            }
        }
        Err(e) => eprintln!("krova: could not list monitors: {e}"),
    }

    let shared = Arc::new(Shared::new(model, platform));
    app.manage(Arc::clone(&shared));
    app.manage(ui_window::UiState::default());

    // The layout switcher's shortcut asks the shell to bring its view forward. Set
    // before the engine starts so the first key press already finds it.
    let handle = app.handle().clone();
    *model::lock(&shared.open_view) = Some(Arc::new(move |view| {
        ui_window::show(&handle, Some(view));
    }));

    let initial = commands::build_state(&shared);
    tray::build(app.handle(), &initial)?;
    engine::start(&shared);
    // --- wave3 ---
    wave3::start(&shared);
    // --- end wave3 ---
    // --- wave4 ---
    wave4::start(app.handle(), &shared);
    // --- end wave4 ---
    // --- notch ---
    notch::start(app.handle());
    // --- notch ---
    Ok(())
}
