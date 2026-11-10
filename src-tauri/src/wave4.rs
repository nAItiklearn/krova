//! Wave 4 ("fit and finish") commands and glue: reserved edges, focus follows
//! mouse, the hover zone picker, "Check for updates" and the native string table.
//!
//! Everything lives here so the shared files (`engine.rs`, `commands.rs`,
//! `lib.rs`, `tray.rs`) only carry small `// --- wave4 ---` blocks that call in.

use std::sync::Arc;

use krova_core::edges::ReservedEdges;
use tauri::{AppHandle, State};

use crate::commands::commit;
use crate::model::{lock, AppState, Shared};

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
}
