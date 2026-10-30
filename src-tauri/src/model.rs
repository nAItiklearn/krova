//! App state shared by the tray, the Tauri commands and the drag engine.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use krova_core::{Config, ConfigError, MonitorDescriptor, MonitorView, PxRect, Settings};
use krova_platform::{InputHandle, MonitorInfo, Permission, Platform};
use serde::Serialize;

/// Everything the app threads share.
pub struct Shared {
    pub model: Mutex<AppModel>,
    pub platform: Arc<dyn Platform>,
    /// Keeps the OS input hook alive; `None` if it could not be started.
    pub input: Mutex<Option<InputHandle>>,
}

/// Lock a mutex, carrying on with the data if another thread panicked.
pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A currently connected monitor, as shown in the UI.
#[derive(Debug, Clone, Serialize)]
pub struct MonitorSnapshot {
    pub profile_id: String,
    pub label: String,
    pub rect: PxRect,
    pub work: PxRect,
    pub scale: f64,
    pub primary: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionState {
    NotRequired,
    Granted,
    Denied,
}

impl From<Permission> for PermissionState {
    fn from(p: Permission) -> Self {
        match p {
            Permission::NotRequired => Self::NotRequired,
            Permission::Granted => Self::Granted,
            Permission::Denied => Self::Denied,
        }
    }
}

/// The single JSON document the UI renders from.
#[derive(Debug, Clone, Serialize)]
pub struct AppState {
    pub config: Config,
    pub monitors: Vec<MonitorSnapshot>,
    pub autostart: bool,
    pub permission: PermissionState,
    pub version: String,
}

pub struct AppModel {
    pub config: Config,
    path: PathBuf,
    /// False when the config on disk could not be used (for example it was
    /// written by a newer version): we then run on defaults and never overwrite it.
    persist: bool,
    monitors: Vec<MonitorInfo>,
    /// Profile index in `config.monitors` for each entry of `monitors`.
    profile_idx: Vec<usize>,
}

impl AppModel {
    pub fn new(config: Config, path: PathBuf, persist: bool) -> Self {
        Self {
            config,
            path,
            persist,
            monitors: Vec::new(),
            profile_idx: Vec::new(),
        }
    }

    /// Adopt the currently connected monitors, matching them to saved profiles
    /// (creating profiles for unknown ones). Returns whether the config changed;
    /// if so it has been saved.
    pub fn refresh_monitors(&mut self, infos: Vec<MonitorInfo>) -> Result<bool, ConfigError> {
        let before = self.config.clone();
        let descriptors: Vec<MonitorDescriptor> =
            infos.iter().map(|m| m.descriptor.clone()).collect();
        self.profile_idx = self.config.resolve_monitors(&descriptors);
        self.monitors = infos;
        let changed = self.config != before;
        if changed {
            self.persist()?;
        }
        Ok(changed)
    }

    pub fn persist(&self) -> Result<(), ConfigError> {
        if self.persist {
            self.config.save(&self.path)
        } else {
            Ok(())
        }
    }

    /// Monitors with their active layouts, in the shape the snap session wants.
    pub fn views(&self) -> Vec<MonitorView<'_>> {
        self.monitors
            .iter()
            .zip(&self.profile_idx)
            .map(|(m, &p)| MonitorView {
                rect: m.descriptor.rect,
                work: m.work,
                layout: self.config.monitors[p].active(),
            })
            .collect()
    }

    pub fn snapshots(&self) -> Vec<MonitorSnapshot> {
        self.monitors
            .iter()
            .zip(&self.profile_idx)
            .map(|(m, &p)| MonitorSnapshot {
                profile_id: self.config.monitors[p].id.clone(),
                label: m.label.clone(),
                rect: m.descriptor.rect,
                work: m.work,
                scale: m.scale,
                primary: m.primary,
            })
            .collect()
    }

    pub fn state(&self, autostart: bool, permission: Permission) -> AppState {
        AppState {
            config: self.config.clone(),
            monitors: self.snapshots(),
            autostart,
            permission: permission.into(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
        }
    }
}

/// Force settings coming from the UI (or the tray) into their valid ranges. The
/// rules live in `krova_core` so the config loader applies the same ones.
pub fn sanitize_settings(s: Settings) -> Settings {
    s.sanitized()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(hw: &str, x: i32) -> MonitorInfo {
        MonitorInfo {
            descriptor: MonitorDescriptor {
                hardware_id: Some(hw.to_owned()),
                device_path: None,
                rect: PxRect::new(x, 0, 1920, 1080),
            },
            work: PxRect::new(x, 0, 1920, 1040),
            scale: 1.0,
            label: format!("Display {hw}"),
            primary: x == 0,
        }
    }

    #[test]
    fn refresh_creates_profiles_persists_and_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut m = AppModel::new(Config::default(), path.clone(), true);
        assert!(m
            .refresh_monitors(vec![info("A", 0), info("B", 1920)])
            .unwrap());
        assert!(path.exists());
        let (loaded, _) = Config::load(&path).unwrap();
        assert_eq!(loaded.monitors.len(), 2);
        assert!(!m
            .refresh_monitors(vec![info("A", 0), info("B", 1920)])
            .unwrap());
    }

    #[test]
    fn views_and_snapshots_follow_the_connected_monitors() {
        let dir = tempfile::tempdir().unwrap();
        let mut m = AppModel::new(Config::default(), dir.path().join("c.json"), true);
        m.refresh_monitors(vec![info("A", 0), info("B", 1920)])
            .unwrap();

        let views = m.views();
        assert_eq!(views.len(), 2);
        assert_eq!(views[1].rect.x, 1920);
        assert_eq!(views[1].layout.unwrap().id, "halves");

        let snaps = m.snapshots();
        assert_eq!(snaps[0].label, "Display A");
        assert_eq!(snaps[0].profile_id, "monitor-1");
        assert!(snaps[0].primary && !snaps[1].primary);
    }

    #[test]
    fn disconnected_monitors_keep_their_profile_but_leave_the_views() {
        let dir = tempfile::tempdir().unwrap();
        let mut m = AppModel::new(Config::default(), dir.path().join("c.json"), true);
        m.refresh_monitors(vec![info("A", 0), info("B", 1920)])
            .unwrap();
        m.config.monitors[1].active_layout = "grid".to_owned();
        m.refresh_monitors(vec![info("A", 0)]).unwrap();
        assert_eq!(m.views().len(), 1);
        assert_eq!(m.config.monitors.len(), 2);
        m.refresh_monitors(vec![info("B", 0), info("A", 1920)])
            .unwrap();
        let views = m.views();
        assert_eq!(views[0].layout.unwrap().id, "grid");
        assert_eq!(views[1].layout.unwrap().id, "halves");
    }

    #[test]
    fn nothing_is_written_when_persistence_is_off() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut m = AppModel::new(Config::default(), path.clone(), false);
        m.refresh_monitors(vec![info("A", 0)]).unwrap();
        m.persist().unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn state_carries_version_and_permission() {
        let m = AppModel::new(Config::default(), PathBuf::from("unused.json"), false);
        let s = m.state(true, Permission::Denied);
        assert!(s.autostart);
        assert_eq!(s.permission, PermissionState::Denied);
        assert_eq!(s.version, env!("CARGO_PKG_VERSION"));
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["permission"], "denied");
        assert_eq!(json["config"]["settings"]["modifier"], "shift");
    }

    #[test]
    fn settings_are_forced_into_range() {
        let s = sanitize_settings(Settings {
            highlight_opacity: 9.0,
            edge_size_px: 1000,
            highlight_color: "  #FF8800 ".to_owned(),
            ..Settings::default()
        });
        assert_eq!(s.highlight_opacity, 1.0);
        assert_eq!(s.edge_size_px, 64);
        assert_eq!(s.highlight_color, "#ff8800");

        let s = sanitize_settings(Settings {
            highlight_opacity: f32::NAN,
            edge_size_px: -3,
            highlight_color: "red".to_owned(),
            ..Settings::default()
        });
        assert_eq!(s.highlight_opacity, Settings::default().highlight_opacity);
        assert_eq!(s.edge_size_px, 2);
        assert_eq!(s.highlight_color, Settings::default().highlight_color);
    }
}
