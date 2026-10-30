use std::fs;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::geometry::PxRect;
use crate::monitor::{match_monitors, MonitorDescriptor};
use crate::zone::{templates, Layout};

pub const SCHEMA_VERSION: u32 = 1;

/// Largest config file that will be parsed; anything bigger is treated as corrupt.
pub const MAX_CONFIG_BYTES: u64 = 4 * 1024 * 1024;
/// Most monitor profiles kept (profiles of disconnected monitors are kept too).
pub const MAX_MONITORS: usize = 64;
/// Most layouts a single monitor profile may hold.
pub const MAX_LAYOUTS_PER_MONITOR: usize = 64;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("config schema {found} is newer than supported {supported}")]
    TooNew { found: u32, supported: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadStatus {
    Loaded,
    /// No file yet: defaults used.
    Missing,
    /// File was unreadable; it was moved to `*.bak` and defaults used.
    Recovered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Modifier {
    #[default]
    Shift,
    Ctrl,
    Alt,
    /// Windows key / Command.
    Super,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MoveMode {
    #[default]
    Animate,
    Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub enabled: bool,
    pub modifier: Modifier,
    pub highlight_color: String,
    pub highlight_opacity: f32,
    pub move_mode: MoveMode,
    pub edge_snapping: bool,
    pub edge_size_px: i32,
    pub launch_at_login: bool,
    pub theme: Theme,
}

/// `#rrggbb` (any case, optional `#`, surrounding spaces ignored) as lowercase `#rrggbb`.
pub fn normalize_hex_color(s: &str) -> Option<String> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!("#{}", h.to_ascii_lowercase()))
}

impl Settings {
    /// Force every field into its valid range. Applied to settings coming from the
    /// UI, the tray and the config file, so a hand-edited file cannot carry nonsense.
    pub fn sanitized(mut self) -> Self {
        let defaults = Settings::default();
        self.highlight_opacity = if self.highlight_opacity.is_finite() {
            self.highlight_opacity.clamp(0.0, 1.0)
        } else {
            defaults.highlight_opacity
        };
        self.edge_size_px = self.edge_size_px.clamp(2, 64);
        self.highlight_color =
            normalize_hex_color(&self.highlight_color).unwrap_or(defaults.highlight_color);
        self
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            modifier: Modifier::Shift,
            highlight_color: "#3b82f6".to_owned(),
            highlight_opacity: 0.35,
            move_mode: MoveMode::Animate,
            edge_snapping: false,
            edge_size_px: 8,
            launch_at_login: false,
            theme: Theme::System,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonitorProfile {
    pub id: String,
    pub label: String,
    pub hardware_id: Option<String>,
    pub device_path: Option<String>,
    pub last_rect: PxRect,
    pub layouts: Vec<Layout>,
    pub active_layout: String,
}

impl MonitorProfile {
    fn from_descriptor(id: String, d: &MonitorDescriptor) -> Self {
        Self {
            label: id.clone(),
            id,
            hardware_id: d.hardware_id.clone(),
            device_path: d.device_path.clone(),
            last_rect: d.rect,
            layouts: templates(),
            active_layout: "halves".to_owned(),
        }
    }

    pub fn descriptor(&self) -> MonitorDescriptor {
        MonitorDescriptor {
            hardware_id: self.hardware_id.clone(),
            device_path: self.device_path.clone(),
            rect: self.last_rect,
        }
    }

    pub fn active(&self) -> Option<&Layout> {
        self.layouts.iter().find(|l| l.id == self.active_layout)
    }

    /// Drop invalid layouts, reseed if none are left, repair a dangling active id.
    fn sanitize(&mut self) {
        self.layouts.retain(|l| l.validate().is_ok());
        self.layouts.truncate(MAX_LAYOUTS_PER_MONITOR);
        if self.layouts.is_empty() {
            self.layouts = templates();
        }
        if self.active().is_none() {
            self.active_layout = self.layouts[0].id.clone();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub schema_version: u32,
    pub settings: Settings,
    pub monitors: Vec<MonitorProfile>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            settings: Settings::default(),
            monitors: Vec::new(),
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<(Config, LoadStatus), ConfigError> {
        // An absurdly large file is never a config we wrote: keep it aside, start fresh.
        if fs::metadata(path).is_ok_and(|m| m.len() > MAX_CONFIG_BYTES) {
            return Self::quarantine(path);
        }
        let text = match fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok((Config::default(), LoadStatus::Missing));
            }
            Err(e) => return Err(e.into()),
        };
        let value: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => return Self::quarantine(path),
        };
        // A version too large for u32 is certainly newer than ours, not "0".
        let found = value
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            .map_or(0, |v| u32::try_from(v).unwrap_or(u32::MAX));
        if found > SCHEMA_VERSION {
            return Err(ConfigError::TooNew {
                found,
                supported: SCHEMA_VERSION,
            });
        }
        let mut cfg: Config = match serde_json::from_value(value) {
            Ok(c) => c,
            Err(_) => return Self::quarantine(path),
        };
        // Migrations from older versions would run here, keyed on `found`.
        cfg.schema_version = SCHEMA_VERSION;
        cfg.settings = cfg.settings.sanitized();
        cfg.monitors.truncate(MAX_MONITORS);
        for m in &mut cfg.monitors {
            m.sanitize();
        }
        Ok((cfg, LoadStatus::Loaded))
    }

    fn quarantine(path: &Path) -> Result<(Config, LoadStatus), ConfigError> {
        fs::rename(path, path.with_extension("json.bak"))?;
        Ok((Config::default(), LoadStatus::Recovered))
    }

    /// Atomic write: temp file in the same directory, fsync, rename over target.
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("json.tmp");
        let json = serde_json::to_vec_pretty(self)?;
        {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(&json)?;
            f.sync_all()?;
        }
        fs::rename(&tmp, path)?;
        Ok(())
    }

    /// Match currently connected monitors to saved profiles (creating new
    /// profiles for unknown ones) and refresh their volatile fields.
    /// Profiles of disconnected monitors are kept so they restore on reconnect.
    /// Returns, per current monitor, its profile index.
    pub fn resolve_monitors(&mut self, current: &[MonitorDescriptor]) -> Vec<usize> {
        let stored: Vec<MonitorDescriptor> = self
            .monitors
            .iter()
            .map(MonitorProfile::descriptor)
            .collect();
        let matches = match_monitors(current, &stored);
        let mut out = Vec::with_capacity(current.len());
        for (d, m) in current.iter().zip(matches) {
            let idx = match m {
                Some(i) => {
                    let p = &mut self.monitors[i];
                    p.last_rect = d.rect;
                    if d.device_path.is_some() {
                        p.device_path = d.device_path.clone();
                    }
                    if p.hardware_id.is_none() {
                        p.hardware_id = d.hardware_id.clone();
                    }
                    i
                }
                None => {
                    let id = self.next_monitor_id();
                    self.monitors.push(MonitorProfile::from_descriptor(id, d));
                    self.monitors.len() - 1
                }
            };
            out.push(idx);
        }
        out
    }

    fn next_monitor_id(&self) -> String {
        (self.monitors.len() + 1..)
            .map(|n| format!("monitor-{n}"))
            .find(|id| self.monitors.iter().all(|m| &m.id != id))
            .expect("unbounded iterator")
    }

    fn profile_mut(&mut self, monitor_id: &str) -> Result<&mut MonitorProfile, ProfileError> {
        self.monitors
            .iter_mut()
            .find(|m| m.id == monitor_id)
            .ok_or_else(|| ProfileError::NoSuchMonitor(monitor_id.to_owned()))
    }

    /// Insert or replace (by layout id) a layout on a monitor profile.
    /// Invalid layouts are refused and nothing changes.
    pub fn save_layout(&mut self, monitor_id: &str, layout: Layout) -> Result<(), ProfileError> {
        layout.validate()?;
        let p = self.profile_mut(monitor_id)?;
        if let Some(i) = p.layouts.iter().position(|l| l.id == layout.id) {
            p.layouts[i] = layout;
        } else if p.layouts.len() >= MAX_LAYOUTS_PER_MONITOR {
            return Err(ProfileError::TooManyLayouts);
        } else {
            p.layouts.push(layout);
        }
        Ok(())
    }

    /// Remove a layout. A monitor always keeps at least one; deleting the active
    /// layout activates the first remaining one.
    pub fn delete_layout(&mut self, monitor_id: &str, layout_id: &str) -> Result<(), ProfileError> {
        let p = self.profile_mut(monitor_id)?;
        let idx = p
            .layouts
            .iter()
            .position(|l| l.id == layout_id)
            .ok_or_else(|| ProfileError::NoSuchLayout(layout_id.to_owned()))?;
        if p.layouts.len() == 1 {
            return Err(ProfileError::LastLayout);
        }
        p.layouts.remove(idx);
        if p.active_layout == layout_id {
            p.active_layout = p.layouts[0].id.clone();
        }
        Ok(())
    }

    pub fn set_active_layout(
        &mut self,
        monitor_id: &str,
        layout_id: &str,
    ) -> Result<(), ProfileError> {
        let p = self.profile_mut(monitor_id)?;
        if !p.layouts.iter().any(|l| l.id == layout_id) {
            return Err(ProfileError::NoSuchLayout(layout_id.to_owned()));
        }
        p.active_layout = layout_id.to_owned();
        Ok(())
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProfileError {
    #[error("monitor {0} not found")]
    NoSuchMonitor(String),
    #[error("layout {0} not found")]
    NoSuchLayout(String),
    #[error("a monitor needs at least one layout")]
    LastLayout,
    #[error("a monitor can hold at most 64 layouts")]
    TooManyLayouts,
    #[error("invalid layout: {0}")]
    Invalid(#[from] crate::zone::LayoutError),
}

#[cfg(test)]
mod profile_tests {
    use super::*;
    use crate::geometry::Rect;

    fn config_with_one_monitor() -> Config {
        let mut c = Config::default();
        c.resolve_monitors(&[MonitorDescriptor {
            hardware_id: Some("A".to_owned()),
            device_path: None,
            rect: PxRect::new(0, 0, 1920, 1080),
        }]);
        c
    }

    fn id(c: &Config) -> String {
        c.monitors[0].id.clone()
    }

    #[test]
    fn save_layout_adds_then_replaces_by_id() {
        let mut c = config_with_one_monitor();
        let m = id(&c);
        let before = c.monitors[0].layouts.len();
        let mut l = Layout::new("mine", "Mine", &[Rect::new(0.0, 0.0, 1.0, 1.0)]);
        c.save_layout(&m, l.clone()).unwrap();
        assert_eq!(c.monitors[0].layouts.len(), before + 1);
        l.name = "Renamed".to_owned();
        c.save_layout(&m, l).unwrap();
        assert_eq!(c.monitors[0].layouts.len(), before + 1);
        assert!(c.monitors[0]
            .layouts
            .iter()
            .any(|x| x.id == "mine" && x.name == "Renamed"));
    }

    #[test]
    fn invalid_layout_and_unknown_monitor_are_refused() {
        let mut c = config_with_one_monitor();
        let m = id(&c);
        let before = c.clone();
        let bad = Layout::new("bad", "Bad", &[Rect::new(0.9, 0.0, 0.5, 1.0)]);
        assert!(matches!(
            c.save_layout(&m, bad),
            Err(ProfileError::Invalid(_))
        ));
        let ok = Layout::new("ok", "Ok", &[Rect::new(0.0, 0.0, 1.0, 1.0)]);
        assert_eq!(
            c.save_layout("nope", ok),
            Err(ProfileError::NoSuchMonitor("nope".to_owned()))
        );
        assert_eq!(c, before);
    }

    #[test]
    fn deleting_the_active_layout_activates_another() {
        let mut c = config_with_one_monitor();
        let m = id(&c);
        assert_eq!(c.monitors[0].active_layout, "halves");
        c.delete_layout(&m, "halves").unwrap();
        assert!(c.monitors[0].layouts.iter().all(|l| l.id != "halves"));
        assert!(c.monitors[0].active().is_some());
    }

    #[test]
    fn the_last_layout_cannot_be_deleted() {
        let mut c = config_with_one_monitor();
        let m = id(&c);
        c.monitors[0].layouts.truncate(1);
        let only = c.monitors[0].layouts[0].id.clone();
        c.monitors[0].active_layout = only.clone();
        assert_eq!(c.delete_layout(&m, &only), Err(ProfileError::LastLayout));
        assert_eq!(c.monitors[0].layouts.len(), 1);
        assert_eq!(
            c.delete_layout(&m, "ghost"),
            Err(ProfileError::NoSuchLayout("ghost".to_owned()))
        );
    }

    #[test]
    fn set_active_requires_an_existing_layout() {
        let mut c = config_with_one_monitor();
        let m = id(&c);
        c.set_active_layout(&m, "grid").unwrap();
        assert_eq!(c.monitors[0].active_layout, "grid");
        assert_eq!(
            c.set_active_layout(&m, "ghost"),
            Err(ProfileError::NoSuchLayout("ghost".to_owned()))
        );
        assert_eq!(c.monitors[0].active_layout, "grid");
    }
}

#[cfg(test)]
mod hardening_tests {
    use super::*;
    use crate::geometry::Rect;

    fn one_monitor(hw: &str) -> MonitorDescriptor {
        MonitorDescriptor {
            hardware_id: Some(hw.to_owned()),
            device_path: None,
            rect: PxRect::new(0, 0, 1920, 1080),
        }
    }

    fn full(id: &str) -> Layout {
        Layout::new(id, "n", &[Rect::new(0.0, 0.0, 1.0, 1.0)])
    }

    #[test]
    fn hex_colours_are_normalized() {
        assert_eq!(normalize_hex_color("#FF8800").as_deref(), Some("#ff8800"));
        assert_eq!(normalize_hex_color("  3b82f6 ").as_deref(), Some("#3b82f6"));
        assert_eq!(normalize_hex_color("#fff"), None);
        assert_eq!(normalize_hex_color("#+f+f+f"), None);
        assert_eq!(normalize_hex_color("#ggg000"), None);
        assert_eq!(normalize_hex_color("#12345é"), None);
        assert_eq!(normalize_hex_color(""), None);
    }

    #[test]
    fn settings_are_forced_into_range() {
        let s = Settings {
            highlight_opacity: 9.0,
            edge_size_px: 1000,
            highlight_color: "  #FF8800 ".to_owned(),
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(s.highlight_opacity, 1.0);
        assert_eq!(s.edge_size_px, 64);
        assert_eq!(s.highlight_color, "#ff8800");

        let s = Settings {
            highlight_opacity: f32::NAN,
            edge_size_px: -3,
            highlight_color: "red".to_owned(),
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(s.highlight_opacity, Settings::default().highlight_opacity);
        assert_eq!(s.edge_size_px, 2);
        assert_eq!(s.highlight_color, Settings::default().highlight_color);

        let d = Settings::default();
        assert_eq!(d.clone().sanitized(), d, "valid settings are left alone");
    }

    #[test]
    fn hand_edited_settings_are_sanitized_on_load() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.json");
        fs::write(
            &p,
            r#"{"schema_version":1,"settings":{"highlight_opacity":7.0,"edge_size_px":100000,"highlight_color":"red"}}"#,
        )
        .unwrap();
        let (c, st) = Config::load(&p).unwrap();
        assert_eq!(st, LoadStatus::Loaded);
        assert_eq!(c.settings.highlight_opacity, 1.0);
        assert_eq!(c.settings.edge_size_px, 64);
        assert_eq!(c.settings.highlight_color, "#3b82f6");
    }

    #[test]
    fn an_oversized_file_is_set_aside_not_parsed() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.json");
        let f = fs::File::create(&p).unwrap();
        f.set_len(MAX_CONFIG_BYTES + 1).unwrap();
        drop(f);
        let (c, st) = Config::load(&p).unwrap();
        assert_eq!(st, LoadStatus::Recovered);
        assert_eq!(c, Config::default());
        assert!(dir.path().join("config.json.bak").exists());
        assert!(!p.exists());
    }

    #[test]
    fn a_schema_version_beyond_u32_counts_as_newer() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.json");
        fs::write(&p, r#"{"schema_version": 4294967296}"#).unwrap();
        assert!(matches!(
            Config::load(&p),
            Err(ConfigError::TooNew {
                found: u32::MAX,
                ..
            })
        ));
        assert!(p.exists(), "a newer config is left untouched");
    }

    #[test]
    fn profile_and_layout_counts_are_capped_on_load() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.json");
        let mut c = Config::default();
        for i in 0..MAX_MONITORS + 5 {
            c.resolve_monitors(&[one_monitor(&format!("HW{i}"))]);
        }
        assert!(c.monitors.len() > MAX_MONITORS);
        for i in 0..MAX_LAYOUTS_PER_MONITOR + 5 {
            c.monitors[0].layouts.push(full(&format!("extra{i}")));
        }
        c.save(&p).unwrap();
        let (loaded, _) = Config::load(&p).unwrap();
        assert_eq!(loaded.monitors.len(), MAX_MONITORS);
        assert_eq!(loaded.monitors[0].layouts.len(), MAX_LAYOUTS_PER_MONITOR);
        assert!(loaded.monitors[0].active().is_some());
    }

    #[test]
    fn the_layout_count_per_monitor_is_capped_when_saving() {
        let mut c = Config::default();
        c.resolve_monitors(&[one_monitor("A")]);
        let m = c.monitors[0].id.clone();
        let existing = c.monitors[0].layouts.len();
        for i in existing..MAX_LAYOUTS_PER_MONITOR {
            c.save_layout(&m, full(&format!("l{i}"))).unwrap();
        }
        assert_eq!(c.monitors[0].layouts.len(), MAX_LAYOUTS_PER_MONITOR);
        assert_eq!(
            c.save_layout(&m, full("one-too-many")),
            Err(ProfileError::TooManyLayouts)
        );
        assert!(
            c.save_layout(&m, full("l10")).is_ok(),
            "replacing an existing layout is still allowed"
        );
        assert_eq!(c.monitors[0].layouts.len(), MAX_LAYOUTS_PER_MONITOR);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Rect;

    fn desc(hw: &str, x: i32) -> MonitorDescriptor {
        MonitorDescriptor {
            hardware_id: Some(hw.to_owned()),
            device_path: Some(format!("path-{hw}")),
            rect: PxRect::new(x, 0, 1920, 1080),
        }
    }

    #[test]
    fn missing_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let (c, st) = Config::load(&dir.path().join("config.json")).unwrap();
        assert_eq!(st, LoadStatus::Missing);
        assert_eq!(c, Config::default());
    }

    #[test]
    fn save_load_roundtrip_and_no_tmp_left() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sub").join("config.json");
        let mut c = Config::default();
        c.settings.edge_snapping = true;
        c.resolve_monitors(&[desc("A", 0)]);
        c.save(&p).unwrap();
        c.save(&p).unwrap(); // overwrite works
        assert!(!p.with_extension("json.tmp").exists());
        let (loaded, st) = Config::load(&p).unwrap();
        assert_eq!(st, LoadStatus::Loaded);
        assert_eq!(loaded, c);
    }

    #[test]
    fn corrupt_file_is_quarantined() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.json");
        fs::write(&p, "{ not json").unwrap();
        let (c, st) = Config::load(&p).unwrap();
        assert_eq!(st, LoadStatus::Recovered);
        assert_eq!(c, Config::default());
        assert!(dir.path().join("config.json.bak").exists());
        assert!(!p.exists());
    }

    #[test]
    fn newer_schema_is_refused_and_file_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.json");
        fs::write(&p, r#"{"schema_version": 99}"#).unwrap();
        assert!(matches!(
            Config::load(&p),
            Err(ConfigError::TooNew { found: 99, .. })
        ));
        assert!(p.exists());
    }

    #[test]
    fn partial_and_unknown_fields_are_tolerated() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.json");
        fs::write(
            &p,
            r#"{"schema_version":1,"settings":{"modifier":"alt","future":1}}"#,
        )
        .unwrap();
        let (c, _) = Config::load(&p).unwrap();
        assert_eq!(c.settings.modifier, Modifier::Alt);
        assert!(c.settings.enabled);
    }

    #[test]
    fn invalid_layouts_are_dropped_on_load() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.json");
        let mut c = Config::default();
        c.resolve_monitors(&[desc("A", 0)]);
        c.monitors[0].layouts[0].zones[0].rect = Rect::new(0.9, 0.0, 0.9, 1.0);
        c.monitors[0].active_layout = "halves".to_owned();
        c.save(&p).unwrap();
        let (loaded, _) = Config::load(&p).unwrap();
        assert!(loaded.monitors[0].layouts.iter().all(|l| l.id != "halves"));
        assert!(loaded.monitors[0].active().is_some());
    }

    #[test]
    fn reconnect_restores_each_monitors_own_layouts() {
        let mut c = Config::default();
        let idx = c.resolve_monitors(&[desc("A", 0), desc("B", 1920)]);
        assert_eq!(idx, vec![0, 1]);
        c.monitors[0].active_layout = "thirds".to_owned();
        c.monitors[1].active_layout = "grid".to_owned();

        // B unplugged.
        let idx = c.resolve_monitors(&[desc("A", 0)]);
        assert_eq!(idx, vec![0]);
        assert_eq!(c.monitors.len(), 2);

        // B returns on the left this time, enumerated first.
        let idx = c.resolve_monitors(&[desc("B", -1920), desc("A", 0)]);
        assert_eq!(idx, vec![1, 0]);
        assert_eq!(c.monitors[1].active_layout, "grid");
        assert_eq!(c.monitors[0].active_layout, "thirds");
        assert_eq!(c.monitors[1].last_rect.x, -1920);
        assert_eq!(c.monitors.len(), 2);
    }

    #[test]
    fn unknown_monitor_gets_fresh_profile_with_unique_id() {
        let mut c = Config::default();
        c.resolve_monitors(&[desc("A", 0)]);
        c.monitors[0].id = "monitor-2".to_owned();
        c.resolve_monitors(&[desc("A", 0), desc("C", 1920)]);
        let ids: Vec<_> = c.monitors.iter().map(|m| m.id.clone()).collect();
        assert_eq!(ids.len(), 2);
        assert_ne!(ids[0], ids[1]);
        assert!(c.monitors[1].active().is_some());
    }
}
