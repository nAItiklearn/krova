//! Krova desktop app: tray, settings/editor window and the drag-to-snap engine.
//!
//! Privacy by construction: no network access, no telemetry, and window titles
//! are never read, logged or stored.

mod commands;
mod engine;
mod model;
mod tray;

use std::sync::{Arc, Mutex};

use krova_core::{Config, LoadStatus};
use krova_platform::Platform;
use tauri::{Manager, WindowEvent};

use model::{AppModel, Shared};

pub fn run() {
    tauri::Builder::default()
        // A second launch just brings the first one forward; two input hooks
        // would snap every window twice.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_window(app, Some("editor"));
        }))
        .setup(setup)
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::save_settings,
            commands::save_layout,
            commands::delete_layout,
            commands::set_active_layout,
            commands::list_templates,
            commands::edit_split_zone,
            commands::edit_remove_zone,
            commands::edit_move_divider,
        ])
        .on_window_event(|window, event| {
            // The tray keeps the app alive: closing the window only hides it.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Krova");
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

    let shared = Arc::new(Shared {
        model: Mutex::new(model),
        platform,
        input: Mutex::new(None),
    });
    app.manage(Arc::clone(&shared));

    let initial = commands::build_state(&shared);
    tray::build(app.handle(), &initial)?;
    engine::start(&shared);
    Ok(())
}
