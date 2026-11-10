use std::fs;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::app_rules::AppRules;
use crate::arrange::ArrangeSettings;
use crate::geometry::PxRect;
use crate::monitor::{match_monitors, MonitorDescriptor};
use crate::presets::{MainSize, PresetFamily, PresetSpec, PresetVariant};
use crate::shortcuts::ShortcutMap;
use crate::zone::{has_hidden_char, strip_hidden, templates, Layout};

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
    /// What would be written is larger than a file Krova is willing to read back, so
    /// the next start would set it aside as corrupt and lose it. Nothing is written.
    #[error("the config is too large to save ({size} bytes; the limit is {limit})")]
    TooLarge { size: u64, limit: u64 },
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

/// Modifier chord for the keyboard zone shortcuts (`<mods>+Left`, `+1` ... `+9`).
/// Ctrl+Alt+Left/Right is taken by many IDEs (back / forward). Win+Alt looked like
/// the natural default but was measured on Windows 11 (10.0.26300): `RegisterHotKey`
/// refuses all twelve Win+Alt chords there (error 1409, Windows keeps them), and
/// Win+Ctrl for eleven of twelve. Ctrl+Alt+Shift was accepted for all twelve, so it
/// is the default. Serialised as snake_case strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShortcutModifier {
    CtrlAlt,
    WinAlt,
    #[default]
    CtrlAltShift,
    WinCtrl,
}

impl ShortcutModifier {
    /// Human-readable prefix such as `Win+Alt`, used in messages and the UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::CtrlAlt => "Ctrl+Alt",
            Self::WinAlt => "Win+Alt",
            Self::CtrlAltShift => "Ctrl+Alt+Shift",
            Self::WinCtrl => "Win+Ctrl",
        }
    }
}

/// Largest gap (in pixels) between zones and around the screen edge.
pub const MAX_ZONE_GAP_PX: i32 = 32;


/// Default accent colour of every native overlay (zones, stack tabs, the notch).
pub const DEFAULT_ACCENT: &str = "#ff5b1f";
/// Largest corner radius, in logical pixels.
pub const MAX_CORNER_RADIUS_PX: i32 = 16;
/// Border width range of the zone overlay, in logical pixels.
pub const MIN_BORDER_WIDTH_PX: i32 = 1;
pub const MAX_BORDER_WIDTH_PX: i32 = 6;

/// How Krova's native overlays look. `highlight_opacity` stays the opacity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    /// `#rrggbb`, lower case.
    pub accent: String,
    /// `0..=16`.
    pub corner_radius_px: i32,
    /// `1..=6`; the highlighted zone uses twice this.
    pub border_width_px: i32,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            accent: DEFAULT_ACCENT.to_owned(),
            corner_radius_px: 10,
            border_width_px: 2,
        }
    }
}

impl Appearance {
    pub fn sanitized(mut self) -> Self {
        self.accent = normalize_hex_color(&self.accent).unwrap_or_else(|| DEFAULT_ACCENT.to_owned());
        self.corner_radius_px = self.corner_radius_px.clamp(0, MAX_CORNER_RADIUS_PX);
        self.border_width_px = self
            .border_width_px
            .clamp(MIN_BORDER_WIDTH_PX, MAX_BORDER_WIDTH_PX);
        self
    }

    /// The accent as RGB.
    pub fn accent_rgb(&self) -> [u8; 3] {
        let hex = normalize_hex_color(&self.accent).unwrap_or_else(|| DEFAULT_ACCENT.to_owned());
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0);
        [byte(1), byte(3), byte(5)]
    }
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
    /// Global keyboard shortcuts that move the focused window between zones.
    pub keyboard_shortcuts: bool,
    pub shortcut_modifier: ShortcutModifier,
    /// Put windows back into their zones when their monitor is reconnected.
    pub restore_on_reconnect: bool,
    /// Resizing a snapped window by an edge moves the neighbours' shared edge too.
    pub linked_dividers: bool,
    /// Space between zones and around the work area, in pixels (0..=32).
    pub zone_gap_px: i32,
    /// `<shortcut_modifier>+Space` opens the layout switcher on the monitor under
    /// the cursor. Independent of `keyboard_shortcuts`.
    pub layout_switcher_shortcut: bool,
    /// Per-program rules (by executable file name, never a title). A plain array.
    pub app_rules: AppRules,
    /// Changes to the default shortcut map (`action id -> chord | null`). Replaces
    /// `shortcut_modifier`, which is kept only so an older file can be migrated.
    pub shortcuts: ShortcutMap,
    /// Accent, corner radius and border width of the native overlays.
    pub appearance: Appearance,
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
        self.zone_gap_px = self.zone_gap_px.clamp(0, MAX_ZONE_GAP_PX);
        self.highlight_color =
            normalize_hex_color(&self.highlight_color).unwrap_or(defaults.highlight_color);
        self.app_rules = self.app_rules.sanitized();
        self.shortcuts = self.shortcuts.sanitized();
        self.appearance = self.appearance.sanitized();
        self
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            modifier: Modifier::Shift,
            highlight_color: DEFAULT_ACCENT.to_owned(),
            highlight_opacity: 0.35,
            move_mode: MoveMode::Animate,
            edge_snapping: false,
            edge_size_px: 8,
            launch_at_login: false,
            theme: Theme::System,
            keyboard_shortcuts: true,
            shortcut_modifier: ShortcutModifier::CtrlAltShift,
            restore_on_reconnect: true,
            linked_dividers: true,
            zone_gap_px: 0,
            layout_switcher_shortcut: true,
            app_rules: AppRules::default(),
            shortcuts: ShortcutMap::default(),
            appearance: Appearance::default(),
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
    /// Auto-arrange on this monitor. On by default (missing in older files: on).
    #[serde(default)]
    pub auto_arrange: ArrangeSettings,
    /// The first-run preset was offered to this profile (see
    /// [`MonitorProfile::seed_preset`]); it is never offered again.
    #[serde(default)]
    pub preset_seeded: bool,
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
            auto_arrange: ArrangeSettings::default(),
            preset_seeded: false,
        }
    }

    /// The preset a monitor of this size starts with: main + side for three windows,
    /// in the shape of the screen (landscape, ultrawide or portrait).
    pub fn first_run_preset(width: i32, height: i32) -> PresetSpec {
        PresetSpec::new(
            PresetFamily::MainSide,
            3,
            PresetVariant::for_size(width, height),
            MainSize::Sixty,
        )
    }

    /// First run: a profile that still holds exactly the built-in layouts, with the
    /// first one active, gets the preset that fits the monitor's shape added in front
    /// and made active, so Krova tiles out of the box. Done once per profile; a
    /// profile the user has already changed is only marked. Returns whether a layout
    /// was added.
    pub fn seed_preset(&mut self, width: i32, height: i32) -> bool {
        if self.preset_seeded {
            return false;
        }
        self.preset_seeded = true;
        if self.layouts != templates() || self.active_layout != "halves" {
            return false;
        }
        let Ok(layout) = Self::first_run_preset(width, height).layout() else {
            return false;
        };
        self.active_layout = layout.id.clone();
        self.layouts.insert(0, layout);
        true
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
        // Invisible characters in a name are removed rather than costing the user
        // the whole layout; a layout that stays invalid after that is dropped.
        for l in &mut self.layouts {
            if has_hidden_char(&l.id) || has_hidden_char(&l.name) {
                l.id = strip_hidden(&l.id);
                l.name = strip_hidden(&l.name);
            }
        }
        let mut seen = std::collections::HashSet::new();
        self.layouts
            .retain(|l| l.validate().is_ok() && seen.insert(l.id.clone()));
        if has_hidden_char(&self.active_layout) {
            self.active_layout = strip_hidden(&self.active_layout);
        }
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
        // Before the shortcut map existed, one modifier applied to every shortcut.
        let had_shortcut_map = value
            .get("settings")
            .and_then(|s| s.get("shortcuts"))
            .is_some();
        let value_had_appearance = value
            .get("settings")
            .and_then(|s| s.get("appearance"))
            .is_some();
        let mut cfg: Config = match serde_json::from_value(value) {
            Ok(c) => c,
            Err(_) => return Self::quarantine(path),
        };
        // Migrations from older versions would run here, keyed on `found`.
        // Before `appearance`, the zone overlay used `highlight_color`: a colour the
        // user picked then (anything but the old blue default) becomes the accent.
        let had_appearance = value_had_appearance;
        if !had_appearance && !cfg.settings.highlight_color.eq_ignore_ascii_case("#3b82f6") {
            if let Some(c) = normalize_hex_color(&cfg.settings.highlight_color) {
                cfg.settings.appearance.accent = c;
            }
        }
        if !had_shortcut_map {
            cfg.settings.shortcuts = ShortcutMap::migrated_from(cfg.settings.shortcut_modifier);
        }
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
        // Never write what `load` would refuse to read: that would lose the config.
        if json.len() as u64 > MAX_CONFIG_BYTES {
            return Err(ConfigError::TooLarge {
                size: json.len() as u64,
                limit: MAX_CONFIG_BYTES,
            });
        }
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
        let mut matches = match_monitors(current, &stored);
        // Profiles in use right now: never evicted to make room for a new one.
        let mut in_use: Vec<usize> = matches.iter().flatten().copied().collect();
        let mut out = Vec::with_capacity(current.len());
        for (k, d) in current.iter().enumerate() {
            let idx = match matches[k] {
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
                    // At the cap, the oldest profile of a monitor that is not
                    // connected makes room. With nothing to evict (more monitors
                    // are connected than the cap allows) the rest get no profile.
                    if self.monitors.len() >= MAX_MONITORS {
                        let Some(evict) = (0..self.monitors.len()).find(|i| !in_use.contains(i))
                        else {
                            break;
                        };
                        self.monitors.remove(evict);
                        // Every index above the removed profile moves down by one.
                        for used in in_use.iter_mut().chain(out.iter_mut()) {
                            if *used > evict {
                                *used -= 1;
                            }
                        }
                        for slot in matches.iter_mut().flatten() {
                            if *slot > evict {
                                *slot -= 1;
                            }
                        }
                    }
                    let id = self.next_monitor_id();
                    self.monitors.push(MonitorProfile::from_descriptor(id, d));
                    let new = self.monitors.len() - 1;
                    in_use.push(new);
                    new
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

    /// Reorder: the layout ends up at `to_index` (a position in the current list,
    /// so `to_index < len`) and the ones in between shift by one. Moving a layout
    /// to where it already is succeeds and changes nothing. The active layout is
    /// tracked by id, so it stays active. An index outside the list is refused: the
    /// caller's picture of the list is stale.
    pub fn move_layout(
        &mut self,
        monitor_id: &str,
        layout_id: &str,
        to_index: usize,
    ) -> Result<(), ProfileError> {
        let p = self.profile_mut(monitor_id)?;
        let from = p
            .layouts
            .iter()
            .position(|l| l.id == layout_id)
            .ok_or_else(|| ProfileError::NoSuchLayout(layout_id.to_owned()))?;
        if to_index >= p.layouts.len() {
            return Err(ProfileError::OutOfRange {
                index: to_index,
                len: p.layouts.len(),
            });
        }
        let layout = p.layouts.remove(from);
        p.layouts.insert(to_index, layout);
        Ok(())
    }

    /// Put a layout at `index` (`index <= len`; `len` appends), optionally making it
    /// the active one in the same step. This is the undo of [`Config::delete_layout`]:
    /// the caller keeps the deleted layout and its old position. Unlike
    /// [`Config::save_layout`] it never replaces: a layout id the monitor already has
    /// is refused. The layout is validated and the layouts-per-monitor cap applies.
    /// On any error nothing changes.
    pub fn insert_layout(
        &mut self,
        monitor_id: &str,
        layout: Layout,
        index: usize,
        activate: bool,
    ) -> Result<(), ProfileError> {
        layout.validate()?;
        let p = self.profile_mut(monitor_id)?;
        if p.layouts.iter().any(|l| l.id == layout.id) {
            return Err(ProfileError::LayoutExists(layout.id));
        }
        if p.layouts.len() >= MAX_LAYOUTS_PER_MONITOR {
            return Err(ProfileError::TooManyLayouts);
        }
        if index > p.layouts.len() {
            return Err(ProfileError::OutOfRange {
                index,
                len: p.layouts.len(),
            });
        }
        if activate {
            p.active_layout.clone_from(&layout.id);
        }
        p.layouts.insert(index, layout);
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

    /// Replace a monitor's auto-arrange settings.
    pub fn set_auto_arrange(
        &mut self,
        monitor_id: &str,
        settings: ArrangeSettings,
    ) -> Result<(), ProfileError> {
        self.profile_mut(monitor_id)?.auto_arrange = settings;
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
    #[error("layout {0} already exists")]
    LayoutExists(String),
    #[error("position {index} is out of range (the monitor has {len} layouts)")]
    OutOfRange { index: usize, len: usize },
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

    fn order(c: &Config) -> Vec<String> {
        c.monitors[0].layouts.iter().map(|l| l.id.clone()).collect()
    }

    fn one(id: &str) -> Layout {
        Layout::new(id, id, &[Rect::new(0.0, 0.0, 1.0, 1.0)])
    }

    #[test]
    fn move_layout_puts_the_layout_at_the_final_index() {
        let mut c = config_with_one_monitor();
        let m = id(&c);
        assert_eq!(order(&c), ["halves", "thirds", "grid", "priority"]);
        c.move_layout(&m, "priority", 0).unwrap();
        assert_eq!(order(&c), ["priority", "halves", "thirds", "grid"]);
        c.move_layout(&m, "priority", 3).unwrap();
        assert_eq!(order(&c), ["halves", "thirds", "grid", "priority"]);
        c.move_layout(&m, "halves", 2).unwrap();
        assert_eq!(order(&c), ["thirds", "grid", "halves", "priority"]);
        c.move_layout(&m, "priority", 1).unwrap();
        assert_eq!(order(&c), ["thirds", "priority", "grid", "halves"]);
    }

    #[test]
    fn move_layout_keeps_every_layout_and_the_active_one() {
        let mut c = config_with_one_monitor();
        let m = id(&c);
        c.set_active_layout(&m, "grid").unwrap();
        let mut before = c.monitors[0].layouts.clone();
        c.move_layout(&m, "grid", 0).unwrap();
        c.move_layout(&m, "halves", 3).unwrap();
        assert_eq!(c.monitors[0].active_layout, "grid");
        let mut after = c.monitors[0].layouts.clone();
        before.sort_by(|a, b| a.id.cmp(&b.id));
        after.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(before, after, "reordering loses and changes nothing");
    }

    #[test]
    fn moving_a_layout_to_where_it_is_succeeds_and_changes_nothing() {
        let mut c = config_with_one_monitor();
        let m = id(&c);
        let before = c.clone();
        for (i, l) in before.monitors[0].layouts.iter().enumerate() {
            c.move_layout(&m, &l.id, i).unwrap();
            assert_eq!(c, before);
        }
    }

    #[test]
    fn move_layout_refuses_bad_input_and_changes_nothing() {
        let mut c = config_with_one_monitor();
        let m = id(&c);
        let before = c.clone();
        let len = before.monitors[0].layouts.len();
        assert_eq!(
            c.move_layout("nope", "halves", 0),
            Err(ProfileError::NoSuchMonitor("nope".to_owned()))
        );
        assert_eq!(
            c.move_layout(&m, "ghost", 0),
            Err(ProfileError::NoSuchLayout("ghost".to_owned()))
        );
        for bad in [len, len + 1, usize::MAX] {
            assert_eq!(
                c.move_layout(&m, "halves", bad),
                Err(ProfileError::OutOfRange { index: bad, len })
            );
        }
        assert_eq!(c, before);
        let err = c.move_layout(&m, "halves", 9).unwrap_err().to_string();
        assert_eq!(
            err,
            "position 9 is out of range (the monitor has 4 layouts)"
        );
    }

    #[test]
    fn move_layout_only_touches_the_named_monitor() {
        let mut c = config_with_one_monitor();
        let at = |hw: &str, x: i32| MonitorDescriptor {
            hardware_id: Some(hw.to_owned()),
            device_path: None,
            rect: PxRect::new(x, 0, 1920, 1080),
        };
        c.resolve_monitors(&[at("A", 0), at("B", 1920)]);
        assert_eq!(c.monitors.len(), 2);
        let first = c.monitors[0].id.clone();
        c.move_layout(&first, "grid", 0).unwrap();
        assert_eq!(c.monitors[0].layouts[0].id, "grid");
        assert_eq!(c.monitors[1].layouts[0].id, "halves");
    }

    #[test]
    fn a_delete_is_undone_by_insert_layout_at_the_old_index() {
        let mut c = config_with_one_monitor();
        let m = id(&c);
        c.set_active_layout(&m, "thirds").unwrap();
        let before = c.clone();
        let index = 1;
        let deleted = before.monitors[0].layouts[index].clone();
        c.delete_layout(&m, "thirds").unwrap();
        assert_eq!(
            c.monitors[0].active_layout, "halves",
            "fell back to the first"
        );
        c.insert_layout(&m, deleted, index, true).unwrap();
        assert_eq!(c, before, "layouts, order and the active layout are back");
    }

    #[test]
    fn insert_layout_without_activate_keeps_the_active_layout() {
        let mut c = config_with_one_monitor();
        let m = id(&c);
        c.insert_layout(&m, one("mine"), 0, false).unwrap();
        assert_eq!(order(&c), ["mine", "halves", "thirds", "grid", "priority"]);
        assert_eq!(c.monitors[0].active_layout, "halves");
    }

    #[test]
    fn insert_layout_accepts_the_end_of_the_list_and_nothing_past_it() {
        let mut c = config_with_one_monitor();
        let m = id(&c);
        c.insert_layout(&m, one("last"), 4, false).unwrap();
        assert_eq!(c.monitors[0].layouts.last().unwrap().id, "last");
        let before = c.clone();
        assert_eq!(
            c.insert_layout(&m, one("far"), 6, false),
            Err(ProfileError::OutOfRange { index: 6, len: 5 })
        );
        assert_eq!(
            c.insert_layout(&m, one("max"), usize::MAX, true),
            Err(ProfileError::OutOfRange {
                index: usize::MAX,
                len: 5
            })
        );
        assert_eq!(c, before, "a refused insert does not even activate");
    }

    #[test]
    fn insert_layout_never_replaces_and_validates() {
        let mut c = config_with_one_monitor();
        let m = id(&c);
        let before = c.clone();
        let same_id = Layout::new("halves", "Other", &[Rect::new(0.0, 0.0, 1.0, 1.0)]);
        assert_eq!(
            c.insert_layout(&m, same_id, 0, true),
            Err(ProfileError::LayoutExists("halves".to_owned()))
        );
        let bad = Layout::new("bad", "Bad", &[Rect::new(0.9, 0.0, 0.5, 1.0)]);
        assert!(matches!(
            c.insert_layout(&m, bad, 0, false),
            Err(ProfileError::Invalid(_))
        ));
        assert_eq!(
            c.insert_layout("nope", one("ok"), 0, false),
            Err(ProfileError::NoSuchMonitor("nope".to_owned()))
        );
        assert_eq!(c, before);
    }

    #[test]
    fn insert_layout_respects_the_layouts_per_monitor_cap() {
        let mut c = config_with_one_monitor();
        let m = id(&c);
        for i in c.monitors[0].layouts.len()..MAX_LAYOUTS_PER_MONITOR {
            c.insert_layout(&m, one(&format!("l{i}")), 0, false)
                .unwrap();
        }
        assert_eq!(c.monitors[0].layouts.len(), MAX_LAYOUTS_PER_MONITOR);
        let before = c.clone();
        assert_eq!(
            c.insert_layout(&m, one("one-too-many"), 0, true),
            Err(ProfileError::TooManyLayouts)
        );
        assert_eq!(c, before);
    }

    #[test]
    fn duplicate_layout_ids_from_a_hand_edited_file_move_the_first_match() {
        // Ids are not forced unique on load; the first one wins everywhere.
        let mut c = config_with_one_monitor();
        let m = id(&c);
        let dup = c.monitors[0].layouts[0].clone();
        c.monitors[0].layouts.push(dup);
        c.move_layout(&m, "halves", 2).unwrap();
        assert_eq!(
            order(&c),
            ["thirds", "grid", "halves", "priority", "halves"]
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
        assert_eq!(c.settings.highlight_color, "#ff5b1f");
    }

    #[test]
    fn new_settings_default_to_the_documented_values() {
        let d = Settings::default();
        assert!(d.keyboard_shortcuts);
        assert_eq!(d.shortcut_modifier, ShortcutModifier::CtrlAltShift);
        assert!(d.restore_on_reconnect);
        assert!(d.linked_dividers);
        assert_eq!(d.zone_gap_px, 0);
        assert!(d.layout_switcher_shortcut);
        // Settings that are already valid are left alone by sanitising.
        assert_eq!(d.clone().sanitized(), d);
    }

    #[test]
    fn a_v01_config_without_the_new_fields_loads_with_their_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.json");
        fs::write(
            &p,
            r##"{"schema_version":1,"settings":{"enabled":false,"modifier":"alt","highlight_color":"#112233","highlight_opacity":0.5,"move_mode":"instant","edge_snapping":true,"edge_size_px":10,"launch_at_login":true,"theme":"dark"},"monitors":[]}"##,
        )
        .unwrap();
        let (c, st) = Config::load(&p).unwrap();
        assert_eq!(st, LoadStatus::Loaded);
        assert_eq!(c.schema_version, SCHEMA_VERSION);
        assert!(!c.settings.enabled);
        assert_eq!(c.settings.modifier, Modifier::Alt);
        assert_eq!(c.settings.theme, Theme::Dark);
        let d = Settings::default();
        assert_eq!(c.settings.keyboard_shortcuts, d.keyboard_shortcuts);
        assert_eq!(c.settings.shortcut_modifier, d.shortcut_modifier);
        assert_eq!(c.settings.restore_on_reconnect, d.restore_on_reconnect);
        assert_eq!(c.settings.linked_dividers, d.linked_dividers);
        assert_eq!(c.settings.zone_gap_px, d.zone_gap_px);
        assert_eq!(
            c.settings.layout_switcher_shortcut,
            d.layout_switcher_shortcut
        );
    }

    #[test]
    fn the_switcher_shortcut_is_its_own_switch() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.json");
        fs::write(
            &p,
            r#"{"schema_version":1,"settings":{"layout_switcher_shortcut":false}}"#,
        )
        .unwrap();
        let (c, _) = Config::load(&p).unwrap();
        assert!(!c.settings.layout_switcher_shortcut);
        assert!(
            c.settings.keyboard_shortcuts,
            "the move shortcuts are unaffected"
        );
        let s = Settings {
            keyboard_shortcuts: false,
            ..Settings::default()
        }
        .sanitized();
        assert!(s.layout_switcher_shortcut, "and the other way round");
    }

    #[test]
    fn shortcut_modifiers_use_snake_case_strings() {
        for (m, s) in [
            (ShortcutModifier::CtrlAlt, "\"ctrl_alt\""),
            (ShortcutModifier::WinAlt, "\"win_alt\""),
            (ShortcutModifier::CtrlAltShift, "\"ctrl_alt_shift\""),
            (ShortcutModifier::WinCtrl, "\"win_ctrl\""),
        ] {
            assert_eq!(serde_json::to_string(&m).unwrap(), s);
            assert_eq!(serde_json::from_str::<ShortcutModifier>(s).unwrap(), m);
        }
        assert!(serde_json::from_str::<ShortcutModifier>("\"super\"").is_err());
        let json = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(json["shortcut_modifier"], "ctrl_alt_shift");
        assert_eq!(json["keyboard_shortcuts"], true);
        assert_eq!(json["zone_gap_px"], 0);
        assert_eq!(json["layout_switcher_shortcut"], true);
    }

    #[test]
    fn shortcut_labels_read_like_key_chords() {
        assert_eq!(ShortcutModifier::WinAlt.label(), "Win+Alt");
        assert_eq!(ShortcutModifier::CtrlAlt.label(), "Ctrl+Alt");
        assert_eq!(ShortcutModifier::CtrlAltShift.label(), "Ctrl+Alt+Shift");
        assert_eq!(ShortcutModifier::WinCtrl.label(), "Win+Ctrl");
    }

    #[test]
    fn the_zone_gap_is_clamped_to_zero_through_thirty_two() {
        for (given, want) in [
            (-5, 0),
            (0, 0),
            (7, 7),
            (32, 32),
            (33, 32),
            (i32::MAX, 32),
            (i32::MIN, 0),
        ] {
            let s = Settings {
                zone_gap_px: given,
                ..Settings::default()
            }
            .sanitized();
            assert_eq!(s.zone_gap_px, want, "gap {given}");
        }
    }

    #[test]
    fn a_hand_edited_gap_is_clamped_on_load() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.json");
        fs::write(
            &p,
            r#"{"schema_version":1,"settings":{"zone_gap_px":99999,"shortcut_modifier":"win_ctrl","keyboard_shortcuts":false}}"#,
        )
        .unwrap();
        let (c, _) = Config::load(&p).unwrap();
        assert_eq!(c.settings.zone_gap_px, MAX_ZONE_GAP_PX);
        assert_eq!(c.settings.shortcut_modifier, ShortcutModifier::WinCtrl);
        assert!(!c.settings.keyboard_shortcuts);
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
        for i in 0..MAX_MONITORS {
            c.resolve_monitors(&[one_monitor(&format!("HW{i}"))]);
        }
        // `resolve_monitors` keeps to the cap itself, so an oversized file (written by
        // hand or by another version) is built directly.
        for i in 0..5 {
            let mut extra = c.monitors[0].clone();
            extra.id = format!("extra-{i}");
            c.monitors.push(extra);
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

    #[test]
    fn profiles_are_capped_and_the_oldest_disconnected_one_makes_room() {
        let mut c = Config::default();
        // Fill the cap with monitors one after the other (each unplugged later).
        for n in 0..MAX_MONITORS {
            c.resolve_monitors(&[desc(&format!("HW{n}"), 0)]);
        }
        assert_eq!(c.monitors.len(), MAX_MONITORS);
        assert_eq!(c.monitors[0].hardware_id.as_deref(), Some("HW0"));

        // A brand-new monitor: the oldest profile (HW0) is dropped, the newest stays.
        let idx = c.resolve_monitors(&[desc("NEW", 0)]);
        assert_eq!(c.monitors.len(), MAX_MONITORS, "never above the cap");
        assert_eq!(idx.len(), 1);
        assert_eq!(c.monitors[idx[0]].hardware_id.as_deref(), Some("NEW"));
        assert!(c
            .monitors
            .iter()
            .all(|m| m.hardware_id.as_deref() != Some("HW0")));
        assert!(c
            .monitors
            .iter()
            .any(|m| m.hardware_id.as_deref() == Some("HW1")));
    }

    #[test]
    fn a_connected_monitor_is_never_evicted_to_make_room() {
        let mut c = Config::default();
        for n in 0..MAX_MONITORS {
            c.resolve_monitors(&[desc(&format!("HW{n}"), 0)]);
        }
        // HW0 is the oldest, but it is connected right now together with a new one.
        let idx = c.resolve_monitors(&[desc("HW0", 0), desc("NEW", 1920)]);
        assert_eq!(c.monitors.len(), MAX_MONITORS);
        assert_eq!(idx.len(), 2);
        assert_eq!(c.monitors[idx[0]].hardware_id.as_deref(), Some("HW0"));
        assert_eq!(c.monitors[idx[1]].hardware_id.as_deref(), Some("NEW"));
        // HW1, the next oldest, was the one that left.
        assert!(c
            .monitors
            .iter()
            .all(|m| m.hardware_id.as_deref() != Some("HW1")));
    }

    #[test]
    fn eviction_keeps_every_returned_index_pointing_at_the_right_profile() {
        let mut c = Config::default();
        for n in 0..MAX_MONITORS {
            c.resolve_monitors(&[desc(&format!("HW{n}"), 0)]);
        }
        // Two matched monitors that sit after the evicted one, then a new monitor.
        let idx = c.resolve_monitors(&[desc("HW10", 0), desc("HW20", 1920), desc("NEW", 3840)]);
        let hw: Vec<_> = idx
            .iter()
            .map(|&i| c.monitors[i].hardware_id.clone().unwrap())
            .collect();
        assert_eq!(hw, ["HW10", "HW20", "NEW"]);
        assert_eq!(c.monitors.len(), MAX_MONITORS);
    }

    #[test]
    fn more_connected_monitors_than_the_cap_get_no_profile_instead_of_breaking_it() {
        let mut c = Config::default();
        let all: Vec<_> = (0..MAX_MONITORS + 5)
            .map(|n| desc(&format!("HW{n}"), (n as i32) * 100))
            .collect();
        let idx = c.resolve_monitors(&all);
        assert_eq!(c.monitors.len(), MAX_MONITORS);
        assert_eq!(
            idx.len(),
            MAX_MONITORS,
            "the rest are left without a profile"
        );
        for (n, &i) in idx.iter().enumerate() {
            assert_eq!(
                c.monitors[i].hardware_id.as_deref(),
                Some(format!("HW{n}").as_str())
            );
        }
    }

    #[test]
    fn saving_a_config_larger_than_the_load_cap_is_refused_and_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut c = Config::default();
        c.resolve_monitors(&[desc("A", 0)]);
        c.save(&path).unwrap();
        let before = std::fs::read(&path).unwrap();

        // Pad the settings until the JSON is past the cap.
        c.settings.highlight_color = "x".repeat(MAX_CONFIG_BYTES as usize + 10);
        let err = c.save(&path).unwrap_err();
        assert!(matches!(err, ConfigError::TooLarge { .. }), "{err}");
        assert!(err.to_string().contains("too large"));
        assert_eq!(
            std::fs::read(&path).unwrap(),
            before,
            "the old file is untouched"
        );
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn a_config_just_under_the_cap_still_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut c = Config::default();
        c.resolve_monitors(&[desc("A", 0)]);
        c.save(&path).unwrap();
        let (loaded, status) = Config::load(&path).unwrap();
        assert_eq!(status, LoadStatus::Loaded);
        assert_eq!(loaded.monitors.len(), 1);
        assert!(
            std::fs::metadata(&path).unwrap().len() < MAX_CONFIG_BYTES / 100,
            "a normal config is tiny next to the cap"
        );
    }

    #[test]
    fn invisible_characters_in_a_stored_layout_name_are_removed_on_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut c = Config::default();
        c.resolve_monitors(&[desc("A", 0)]);
        c.monitors[0].layouts[0].name = "Ha\u{202E}lves\u{200B}".to_owned();
        let json = serde_json::to_vec(&c).unwrap();
        std::fs::write(&path, json).unwrap();
        let (loaded, _) = Config::load(&path).unwrap();
        let halves = loaded.monitors[0]
            .layouts
            .iter()
            .find(|l| l.id == "halves")
            .expect("the layout survives");
        assert_eq!(halves.name, "Halves");
    }

    #[test]
    fn a_fresh_profile_gets_the_preset_for_its_shape_once() {
        let mut c = Config::default();
        let idx = c.resolve_monitors(&[desc("A", 0)]);
        let p = &mut c.monitors[idx[0]];
        assert!(p.seed_preset(1920, 1080));
        assert_eq!(p.active_layout, "main-side-3-60");
        assert_eq!(p.layouts[0].id, "main-side-3-60");
        assert_eq!(p.layouts.len(), templates().len() + 1);
        assert!(p.active().is_some());
        // Never twice, even if the user goes back to the old layouts.
        assert!(!p.seed_preset(1920, 1080));
        p.layouts = templates();
        p.active_layout = "halves".into();
        assert!(!p.seed_preset(1920, 1080));
    }

    #[test]
    fn the_preset_follows_the_monitor_shape() {
        for (w, h, id) in [
            (3440, 1440, "main-side-3-ultrawide-60"),
            (1080, 1920, "main-side-3-portrait-60"),
        ] {
            let mut c = Config::default();
            let idx = c.resolve_monitors(&[desc("A", 0)]);
            let p = &mut c.monitors[idx[0]];
            assert!(p.seed_preset(w, h));
            assert_eq!(p.active_layout, id);
        }
    }

    #[test]
    fn a_profile_the_user_changed_is_only_marked() {
        let mut c = Config::default();
        let idx = c.resolve_monitors(&[desc("A", 0)]);
        let p = &mut c.monitors[idx[0]];
        p.active_layout = "thirds".into();
        assert!(!p.seed_preset(1920, 1080));
        assert!(p.preset_seeded);
        assert_eq!(p.layouts, templates());
        assert_eq!(p.active_layout, "thirds");
    }

    #[test]
    fn an_old_profile_loads_with_auto_arrange_on_and_unseeded() {
        let json = r#"{"schema_version":1,"monitors":[{"id":"monitor-1","label":"x",
            "hardware_id":null,"device_path":null,"last_rect":{"x":0,"y":0,"w":10,"h":10},
            "layouts":[],"active_layout":"halves"}]}"#;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, json).unwrap();
        let (c, _) = Config::load(&path).unwrap();
        assert!(c.monitors[0].auto_arrange.enabled);
        assert!(!c.monitors[0].preset_seeded);
        assert!(c.settings.app_rules.is_empty());
    }

    #[test]
    fn auto_arrange_can_be_set_per_monitor() {
        let mut c = Config::default();
        c.resolve_monitors(&[desc("A", 0), desc("B", 1920)]);
        let off = ArrangeSettings {
            enabled: false,
            ..ArrangeSettings::default()
        };
        c.set_auto_arrange("monitor-2", off).unwrap();
        assert!(c.monitors[0].auto_arrange.enabled);
        assert!(!c.monitors[1].auto_arrange.enabled);
        assert!(c.set_auto_arrange("monitor-9", off).is_err());
    }

    #[test]
    fn app_rules_in_settings_are_sanitised() {
        use crate::app_rules::{AppRule, RuleAction};
        let json = r#"[{"exe":"a.exe","action":{"kind":"float"}},
            {"exe":"A.EXE","action":{"kind":"never_touch"}},
            {"exe":"dir/x.exe","action":{"kind":"float"}}]"#;
        let rules: AppRules = serde_json::from_str(json).unwrap();
        let s = Settings {
            app_rules: rules,
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(
            s.app_rules.as_slice(),
            &[AppRule::new("a.exe", RuleAction::Float)]
        );
    }

    #[test]
    fn an_old_win_alt_choice_becomes_shortcut_overrides_once() {
        use crate::shortcuts::ShortcutAction;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(
            &path,
            r#"{"schema_version":1,"settings":{"shortcut_modifier":"win_alt"},"monitors":[]}"#,
        )
        .unwrap();
        let (c, _) = Config::load(&path).unwrap();
        assert_eq!(
            c.settings
                .shortcuts
                .chord(ShortcutAction::Zone(2))
                .unwrap()
                .to_string(),
            "Win+Alt+2"
        );
        // Saved and loaded again, the map is the user's now: no second migration.
        let mut c = c;
        c.settings.shortcuts.reset();
        c.save(&path).unwrap();
        let (c, _) = Config::load(&path).unwrap();
        assert!(c.settings.shortcuts.is_default());
    }

    #[test]
    fn a_bad_shortcut_entry_does_not_cost_the_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(
            &path,
            r#"{"schema_version":1,"settings":{"zone_gap_px":4,"shortcuts":{"pause":"Hyper+T","focus":null}},"monitors":[]}"#,
        )
        .unwrap();
        let (c, status) = Config::load(&path).unwrap();
        assert_eq!(status, LoadStatus::Loaded);
        assert_eq!(c.settings.zone_gap_px, 4);
        use crate::shortcuts::ShortcutAction;
        assert_eq!(c.settings.shortcuts.chord(ShortcutAction::Focus), None);
        assert!(c.settings.shortcuts.chord(ShortcutAction::Pause).is_some());
    }

    #[test]
    fn appearance_defaults_and_limits() {
        let a = Appearance::default();
        assert_eq!(a.accent, "#ff5b1f");
        assert_eq!((a.corner_radius_px, a.border_width_px), (10, 2));
        assert_eq!(a.accent_rgb(), [0xff, 0x5b, 0x1f]);
        assert_eq!(Settings::default().highlight_color, "#ff5b1f");
        let wild = Appearance {
            accent: "  #ABCDEF ".into(),
            corner_radius_px: 99,
            border_width_px: 0,
        }
        .sanitized();
        assert_eq!(wild.accent, "#abcdef");
        assert_eq!((wild.corner_radius_px, wild.border_width_px), (16, 1));
        let bad = Appearance {
            accent: "orange".into(),
            corner_radius_px: -3,
            border_width_px: 40,
        }
        .sanitized();
        assert_eq!(bad.accent, "#ff5b1f");
        assert_eq!((bad.corner_radius_px, bad.border_width_px), (0, 6));
    }

    #[test]
    fn a_highlight_colour_picked_before_appearance_becomes_the_accent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(
            &path,
            r##"{"schema_version":1,"settings":{"highlight_color":"#22AA44"}}"##,
        )
        .unwrap();
        let (c, _) = Config::load(&path).unwrap();
        assert_eq!(c.settings.appearance.accent, "#22aa44");
        // The old blue default is not a choice: the new accent applies.
        std::fs::write(
            &path,
            r##"{"schema_version":1,"settings":{"highlight_color":"#3b82f6"}}"##,
        )
        .unwrap();
        let (c, _) = Config::load(&path).unwrap();
        assert_eq!(c.settings.appearance.accent, "#ff5b1f");
        // Once an appearance is stored it wins.
        std::fs::write(
            &path,
            r##"{"schema_version":1,"settings":{"highlight_color":"#22aa44","appearance":{"accent":"#0000ff"}}}"##,
        )
        .unwrap();
        let (c, _) = Config::load(&path).unwrap();
        assert_eq!(c.settings.appearance.accent, "#0000ff");
        assert_eq!(c.settings.appearance.corner_radius_px, 10);
    }
}
