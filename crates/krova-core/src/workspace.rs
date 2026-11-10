//! Workspaces (wave 3): a saved set of programs, each in a zone of a monitor.
//!
//! A workspace remembers, per entry, only the **full path of the executable**, the
//! monitor profile id and the zone id. Never window titles, never command-line
//! arguments, never documents. Restoring reuses an open window of each program, or
//! (when asked) launches the program again from its captured path, places every
//! window in its zone and minimises (never closes) the windows that are not part of
//! the workspace.
//!
//! This module is the pure part: the model with its limits, the capture and
//! restore planners and the rules for which paths may be launched. The platform
//! layer does the enumerating, launching and moving.

use serde::{Deserialize, Serialize};

use crate::geometry::PxRect;
use crate::registry::select::zone_for_rect;
use crate::zone::Layout;

/// Longest workspace id or name, in characters (the same limit as layout names).
pub const MAX_WORKSPACE_NAME_LEN: usize = 64;
/// Most workspaces that are kept.
pub const MAX_WORKSPACES: usize = 32;
/// Most programs in one workspace.
pub const MAX_WORKSPACE_ENTRIES: usize = 64;
/// Longest stored executable path, in UTF-16-ish characters (Windows long paths).
pub const MAX_EXE_PATH_LEN: usize = 1024;
/// How long a restore waits for a launched program's first window.
pub const LAUNCH_WAIT_MS: u64 = 10_000;

/// One program in a workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceEntry {
    /// Full path of the executable, as the OS reported it at capture time.
    pub exe: String,
    /// Profile id of the monitor (`MonitorProfile::id`).
    pub monitor_id: String,
    /// Zone id in the layout that was active on that monitor at capture time
    /// (recorded in [`Workspace::layouts`]).
    pub zone_id: u32,
}

/// The layout that was active on one monitor when the workspace was captured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceLayout {
    pub monitor_id: String,
    pub layout_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub layouts: Vec<WorkspaceLayout>,
    #[serde(default)]
    pub entries: Vec<WorkspaceEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WorkspaceError {
    #[error("give the workspace a name")]
    EmptyName,
    #[error("the workspace name is too long (at most 64 characters)")]
    NameTooLong,
    #[error("the workspace name contains hidden or control characters")]
    HiddenCharacter,
    #[error("a workspace has at most 64 programs")]
    TooManyEntries,
    #[error("at most 32 workspaces can be saved")]
    TooManyWorkspaces,
    #[error("an entry has an unusable program path")]
    BadPath,
    #[error("an entry names no monitor")]
    BadMonitor,
    #[error("no workspace with this id")]
    Unknown,
}

/// Control characters, line and paragraph separators, zero-width characters, the
/// direction overrides, embeddings and isolates and the other invisible format
/// characters (the same set the app rules refuse).
pub fn is_hidden_char(c: char) -> bool {
    c.is_control()
        || (c.is_whitespace() && c != ' ')
        || matches!(
            c,
            '\u{00AD}'
                | '\u{061C}'
                | '\u{180E}'
                | '\u{200B}'..='\u{200F}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{2069}'
                | '\u{206A}'..='\u{206F}'
                | '\u{FEFF}'
                | '\u{FFF9}'..='\u{FFFB}'
                | '\u{E0000}'..='\u{E007F}'
        )
}

/// Check a workspace name (or id) exactly as written.
pub fn validate_name(name: &str) -> Result<(), WorkspaceError> {
    if name.trim().is_empty() {
        return Err(WorkspaceError::EmptyName);
    }
    if name.chars().take(MAX_WORKSPACE_NAME_LEN + 1).count() > MAX_WORKSPACE_NAME_LEN {
        return Err(WorkspaceError::NameTooLong);
    }
    if name.chars().any(is_hidden_char) {
        return Err(WorkspaceError::HiddenCharacter);
    }
    Ok(())
}

/// Trim a typed name and check it.
pub fn normalize_name(raw: &str) -> Result<String, WorkspaceError> {
    let name = raw.trim_matches(' ');
    validate_name(name)?;
    Ok(name.to_owned())
}

fn valid_path_text(path: &str) -> bool {
    !path.is_empty()
        && path.chars().take(MAX_EXE_PATH_LEN + 1).count() <= MAX_EXE_PATH_LEN
        && !path.chars().any(is_hidden_char)
}

impl Workspace {
    pub fn validate(&self) -> Result<(), WorkspaceError> {
        validate_name(&self.id)?;
        validate_name(&self.name)?;
        if self.entries.len() > MAX_WORKSPACE_ENTRIES || self.layouts.len() > MAX_WORKSPACE_ENTRIES
        {
            return Err(WorkspaceError::TooManyEntries);
        }
        for e in &self.entries {
            if !valid_path_text(&e.exe) {
                return Err(WorkspaceError::BadPath);
            }
            if validate_name(&e.monitor_id).is_err() {
                return Err(WorkspaceError::BadMonitor);
            }
        }
        for l in &self.layouts {
            if validate_name(&l.monitor_id).is_err() || validate_name(&l.layout_id).is_err() {
                return Err(WorkspaceError::BadMonitor);
            }
        }
        Ok(())
    }

    /// The layout id recorded for a monitor.
    pub fn layout_for(&self, monitor_id: &str) -> Option<&str> {
        self.layouts
            .iter()
            .find(|l| l.monitor_id == monitor_id)
            .map(|l| l.layout_id.as_str())
    }
}

/// Drop invalid workspaces and keep at most [`MAX_WORKSPACES`]; duplicate ids keep
/// the first. Applied when the config is loaded.
pub fn sanitize_workspaces(list: &mut Vec<Workspace>) {
    let mut seen = std::collections::HashSet::new();
    list.retain(|w| w.validate().is_ok() && seen.insert(w.id.clone()));
    list.truncate(MAX_WORKSPACES);
}

/// A fresh id that is not used yet: `ws-1`, `ws-2`, ...
pub fn next_workspace_id(list: &[Workspace]) -> String {
    (1..)
        .map(|n| format!("ws-{n}"))
        .find(|id| list.iter().all(|w| &w.id != id))
        .unwrap_or_else(|| "ws".to_owned())
}

/// Save (insert or replace by id) a workspace into a list, enforcing the limits.
pub fn upsert_workspace(list: &mut Vec<Workspace>, ws: Workspace) -> Result<(), WorkspaceError> {
    ws.validate()?;
    if let Some(slot) = list.iter_mut().find(|w| w.id == ws.id) {
        *slot = ws;
        return Ok(());
    }
    if list.len() >= MAX_WORKSPACES {
        return Err(WorkspaceError::TooManyWorkspaces);
    }
    list.push(ws);
    Ok(())
}

// ------------------------------------------------------------------ capture

/// A window seen at capture time. `zone` is set when the snap registry knows the
/// window's zone; otherwise its rect decides.
#[derive(Debug, Clone, PartialEq)]
pub struct CaptureWindow {
    pub exe: String,
    pub rect: PxRect,
    /// `(monitor_id, layout_id, zone_id)` from the snap registry, if any.
    pub snapped: Option<(String, String, u32)>,
}

/// A connected monitor at capture time.
#[derive(Debug, Clone, Copy)]
pub struct CaptureMonitor<'a> {
    pub monitor_id: &'a str,
    pub rect: PxRect,
    pub work: PxRect,
    pub layout: Option<&'a Layout>,
}

/// Build a workspace from the windows on screen. Windows are taken in the order
/// given (topmost first); windows in no zone are skipped; the entry list is cut at
/// [`MAX_WORKSPACE_ENTRIES`]. Snap records of a layout that is no longer active are
/// ignored in favour of the rect.
pub fn capture(
    id: &str,
    name: &str,
    monitors: &[CaptureMonitor<'_>],
    windows: &[CaptureWindow],
) -> Workspace {
    let rects: Vec<PxRect> = monitors.iter().map(|m| m.rect).collect();
    let mut entries = Vec::new();
    for w in windows {
        if entries.len() >= MAX_WORKSPACE_ENTRIES || !valid_path_text(&w.exe) {
            continue;
        }
        let from_registry = w.snapped.as_ref().and_then(|(mon, layout, zone)| {
            let m = monitors.iter().find(|m| m.monitor_id == mon)?;
            let active = m.layout?;
            (active.id == *layout && active.zones.iter().any(|z| z.id == *zone))
                .then_some((m.monitor_id, *zone))
        });
        let placed = from_registry.or_else(|| {
            let m = &monitors[crate::registry::select::monitor_for_rect(&rects, w.rect)?];
            let zone = zone_for_rect(m.layout?, m.work, w.rect)?;
            Some((m.monitor_id, zone))
        });
        if let Some((monitor_id, zone_id)) = placed {
            entries.push(WorkspaceEntry {
                exe: w.exe.clone(),
                monitor_id: monitor_id.to_owned(),
                zone_id,
            });
        }
    }
    let layouts = monitors
        .iter()
        .filter_map(|m| {
            Some(WorkspaceLayout {
                monitor_id: m.monitor_id.to_owned(),
                layout_id: m.layout?.id.clone(),
            })
        })
        .take(MAX_WORKSPACE_ENTRIES)
        .collect();
    Workspace {
        id: id.to_owned(),
        name: name.to_owned(),
        layouts,
        entries,
    }
}

// ------------------------------------------------------------------ restore

/// Case-insensitive comparison of two executable paths (Windows paths are not case
/// sensitive), with `/` and `\` treated alike.
pub fn same_exe_path(a: &str, b: &str) -> bool {
    let norm = |c: char| if c == '/' { '\\' } else { c };
    a.chars()
        .map(norm)
        .flat_map(char::to_lowercase)
        .eq(b.chars().map(norm).flat_map(char::to_lowercase))
}

/// What to do for one restore.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RestorePlan {
    /// `(entry index, window)`: an open window that goes into the entry's zone.
    pub reuse: Vec<(usize, u64)>,
    /// Entries with no open window that should be launched.
    pub launch: Vec<usize>,
    /// Entries with no open window that are not launched (launching off, or the
    /// path may not be launched).
    pub missing: Vec<usize>,
    /// Open windows that are not part of the workspace: minimise them.
    pub minimize: Vec<u64>,
}

/// Match the workspace's entries to the open windows `(id, exe path)`. Each window
/// serves at most one entry, in entry order; windows with no exe path are never
/// matched and are minimised like any other stranger. `launchable` decides, per
/// entry, whether its path may be launched.
pub fn plan_restore(
    ws: &Workspace,
    windows: &[(u64, Option<String>)],
    launch: bool,
    launchable: impl Fn(&str) -> bool,
) -> RestorePlan {
    let mut used = vec![false; windows.len()];
    let mut plan = RestorePlan::default();
    for (i, entry) in ws.entries.iter().enumerate() {
        let found = windows.iter().enumerate().position(|(j, (_, exe))| {
            !used[j] && exe.as_deref().is_some_and(|e| same_exe_path(e, &entry.exe))
        });
        match found {
            Some(j) => {
                used[j] = true;
                plan.reuse.push((i, windows[j].0));
            }
            None if launch && launchable(&entry.exe) => plan.launch.push(i),
            None => plan.missing.push(i),
        }
    }
    plan.minimize = windows
        .iter()
        .zip(&used)
        .filter(|(_, &u)| !u)
        .map(|((id, _), _)| *id)
        .collect();
    plan
}

/// Why a captured path may not be launched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LaunchRefusal {
    #[error("the path is not a full local path")]
    NotAbsolute,
    #[error("only programs ending in .exe are started")]
    NotExe,
    #[error("programs in a temporary folder are never started")]
    InTempDir,
    #[error("network paths are never started")]
    Network,
    #[error("the path contains hidden characters")]
    Hidden,
}

/// Whether a stored path may be launched: an absolute local drive path (`C:\...`)
/// ending in `.exe`, with no `..` component, not on a network share and not inside
/// one of `temp_dirs`. The platform additionally checks that the file exists.
pub fn check_launch_path(path: &str, temp_dirs: &[String]) -> Result<(), LaunchRefusal> {
    if path.chars().any(is_hidden_char) {
        return Err(LaunchRefusal::Hidden);
    }
    let p = path.replace('/', "\\");
    if p.starts_with("\\\\") {
        return Err(LaunchRefusal::Network);
    }
    let bytes = p.as_bytes();
    let drive =
        bytes.len() > 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'\\';
    if !drive || p.split('\\').any(|c| c == ".." || c == ".") {
        return Err(LaunchRefusal::NotAbsolute);
    }
    if !p.to_ascii_lowercase().ends_with(".exe") {
        return Err(LaunchRefusal::NotExe);
    }
    let lower = p.to_lowercase();
    for t in temp_dirs {
        let mut t = t.replace('/', "\\").to_lowercase();
        while t.ends_with('\\') {
            t.pop();
        }
        if !t.is_empty() && lower.starts_with(&format!("{t}\\")) {
            return Err(LaunchRefusal::InTempDir);
        }
    }
    // Belt and braces: any folder literally called Temp or Tmp.
    if lower
        .split('\\')
        .any(|c| c == "temp" || c == "tmp" || c == "$recycle.bin")
    {
        return Err(LaunchRefusal::InTempDir);
    }
    Ok(())
}

/// Result of restoring one entry, for the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryOutcome {
    /// An open window was moved into the zone.
    Placed,
    /// The program was started and its window placed.
    Launched,
    /// The program was started but no window appeared within 10 s.
    TimedOut,
    /// No open window, and it was not started (launching off).
    NotRunning,
    /// The path may not be launched (temp folder, not an .exe, gone, ...).
    Refused,
    /// The monitor or zone of the entry is not available now.
    NoZone,
    /// Moving the window failed (for example it belongs to an elevated program).
    Failed,
}

/// One line of a restore report. `program` is the file name only (no folders), so
/// the UI never has to show a full path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryResult {
    pub program: String,
    pub monitor_id: String,
    pub zone_id: u32,
    pub outcome: EntryOutcome,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreReport {
    pub workspace_id: String,
    pub entries: Vec<EntryResult>,
    /// How many other windows were minimised.
    pub minimized: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Rect;

    fn ws(entries: &[(&str, &str, u32)]) -> Workspace {
        Workspace {
            id: "ws-1".into(),
            name: "Writing".into(),
            layouts: vec![WorkspaceLayout {
                monitor_id: "monitor-1".into(),
                layout_id: "halves".into(),
            }],
            entries: entries
                .iter()
                .map(|(exe, m, z)| WorkspaceEntry {
                    exe: (*exe).into(),
                    monitor_id: (*m).into(),
                    zone_id: *z,
                })
                .collect(),
        }
    }

    #[test]
    fn names_follow_the_layout_rules() {
        assert!(validate_name("Writing").is_ok());
        assert!(validate_name("Schreiben & Lesen 日本").is_ok());
        assert_eq!(validate_name("  "), Err(WorkspaceError::EmptyName));
        assert_eq!(
            validate_name(&"n".repeat(65)),
            Err(WorkspaceError::NameTooLong)
        );
        assert!(validate_name(&"é".repeat(64)).is_ok());
        for c in [
            '\u{202A}', '\u{202B}', '\u{202C}', '\u{202D}', '\u{202E}', '\u{2066}', '\u{2067}',
            '\u{2068}', '\u{2069}', '\u{200B}', '\u{200C}', '\u{200D}', '\u{2060}', '\u{FEFF}',
            '\u{2028}', '\u{2029}', '\n', '\t', '\u{7}',
        ] {
            assert_eq!(
                validate_name(&format!("a{c}b")),
                Err(WorkspaceError::HiddenCharacter),
                "{:?}",
                c
            );
        }
        assert_eq!(normalize_name("  Mail  ").unwrap(), "Mail");
    }

    #[test]
    fn limits_are_enforced() {
        let mut w = ws(&[]);
        w.entries = (0..65)
            .map(|i| WorkspaceEntry {
                exe: format!("C:\\a{i}.exe"),
                monitor_id: "monitor-1".into(),
                zone_id: 1,
            })
            .collect();
        assert_eq!(w.validate(), Err(WorkspaceError::TooManyEntries));
        w.entries.truncate(64);
        assert!(w.validate().is_ok());

        let mut list = Vec::new();
        for n in 0..MAX_WORKSPACES {
            let mut w = ws(&[]);
            w.id = format!("ws-{n}");
            upsert_workspace(&mut list, w).unwrap();
        }
        let mut extra = ws(&[]);
        extra.id = "extra".into();
        assert_eq!(
            upsert_workspace(&mut list, extra),
            Err(WorkspaceError::TooManyWorkspaces)
        );
        // Replacing an existing one is still fine at the limit.
        let mut again = ws(&[]);
        again.id = "ws-3".into();
        again.name = "Renamed".into();
        upsert_workspace(&mut list, again).unwrap();
        assert_eq!(list[3].name, "Renamed");
    }

    #[test]
    fn sanitizing_drops_bad_and_duplicate_workspaces() {
        let good = ws(&[("C:\\x.exe", "monitor-1", 1)]);
        let mut bad = good.clone();
        bad.id = "ws-2".into();
        bad.name = "a\u{202E}b".into();
        let mut list = vec![good.clone(), bad, good.clone()];
        sanitize_workspaces(&mut list);
        assert_eq!(list, vec![good]);
        let mut long: Vec<Workspace> = (0..40)
            .map(|n| {
                let mut w = ws(&[]);
                w.id = format!("ws-{n}");
                w
            })
            .collect();
        sanitize_workspaces(&mut long);
        assert_eq!(long.len(), MAX_WORKSPACES);
    }

    #[test]
    fn ids_are_fresh() {
        assert_eq!(next_workspace_id(&[]), "ws-1");
        let list = vec![ws(&[])];
        assert_eq!(next_workspace_id(&list), "ws-2");
    }

    #[test]
    fn capture_maps_windows_to_zones_and_never_keeps_titles() {
        let halves = Layout::new(
            "halves",
            "Halves",
            &[Rect::new(0.0, 0.0, 0.5, 1.0), Rect::new(0.5, 0.0, 0.5, 1.0)],
        );
        let mon = PxRect::new(0, 0, 2000, 1000);
        let monitors = [CaptureMonitor {
            monitor_id: "monitor-1",
            rect: mon,
            work: mon,
            layout: Some(&halves),
        }];
        let windows = vec![
            CaptureWindow {
                exe: "C:\\Apps\\code.exe".into(),
                rect: PxRect::new(1100, 50, 800, 900),
                snapped: None,
            },
            CaptureWindow {
                exe: "C:\\Apps\\mail.exe".into(),
                rect: PxRect::new(10, 10, 100, 100),
                // The registry wins over the rect.
                snapped: Some(("monitor-1".into(), "halves".into(), 2)),
            },
            CaptureWindow {
                // A registry entry of a layout no longer active falls back to the rect.
                exe: "C:\\Apps\\old.exe".into(),
                rect: PxRect::new(10, 10, 100, 100),
                snapped: Some(("monitor-1".into(), "thirds".into(), 3)),
            },
            CaptureWindow {
                // Off every zone (no layout overlap at all): skipped.
                exe: "C:\\Apps\\far.exe".into(),
                rect: PxRect::new(5000, 5000, 10, 10),
                snapped: None,
            },
        ];
        let w = capture("ws-1", "Coding", &monitors, &windows);
        let got: Vec<(&str, u32)> = w
            .entries
            .iter()
            .map(|e| (e.exe.as_str(), e.zone_id))
            .collect();
        // far.exe is nearest monitor-1 but overlaps no zone.
        assert_eq!(
            got,
            vec![
                ("C:\\Apps\\code.exe", 2),
                ("C:\\Apps\\mail.exe", 2),
                ("C:\\Apps\\old.exe", 1),
            ]
        );
        assert_eq!(w.layout_for("monitor-1"), Some("halves"));
        assert!(w.validate().is_ok());
        // The serialised form holds only paths, monitor ids, layout ids and zone ids.
        let json = serde_json::to_string(&w).unwrap();
        assert!(!json.contains("title"));
    }

    #[test]
    fn restore_reuses_launches_and_minimises() {
        let w = ws(&[
            ("C:\\Apps\\Code.exe", "monitor-1", 1),
            ("C:\\Apps\\code.exe", "monitor-1", 2),
            ("C:\\Apps\\mail.exe", "monitor-1", 2),
            ("C:\\Temp\\x.exe", "monitor-1", 1),
        ]);
        let windows = vec![
            (10, Some("c:/apps/CODE.EXE".to_owned())),
            (11, Some("C:\\Other\\chat.exe".to_owned())),
            (12, None),
        ];
        let plan = plan_restore(&w, &windows, true, |p| check_launch_path(p, &[]).is_ok());
        assert_eq!(plan.reuse, vec![(0, 10)]);
        assert_eq!(plan.launch, vec![1, 2]);
        assert_eq!(plan.missing, vec![3]);
        assert_eq!(plan.minimize, vec![11, 12]);

        let no_launch = plan_restore(&w, &windows, false, |_| true);
        assert!(no_launch.launch.is_empty());
        assert_eq!(no_launch.missing, vec![1, 2, 3]);
    }

    #[test]
    fn only_safe_paths_may_be_launched() {
        let temps = vec!["C:\\Users\\me\\AppData\\Local\\Temp\\".to_owned()];
        assert!(check_launch_path("C:\\Program Files\\App\\app.exe", &temps).is_ok());
        assert!(check_launch_path("d:/tools/App.EXE", &temps).is_ok());
        assert_eq!(
            check_launch_path("C:\\Users\\me\\AppData\\Local\\Temp\\x\\a.exe", &temps),
            Err(LaunchRefusal::InTempDir)
        );
        assert_eq!(
            check_launch_path("C:\\Windows\\Temp\\a.exe", &[]),
            Err(LaunchRefusal::InTempDir)
        );
        assert_eq!(
            check_launch_path("C:\\app\\run.bat", &temps),
            Err(LaunchRefusal::NotExe)
        );
        assert_eq!(
            check_launch_path("C:\\app\\run.exe.lnk", &temps),
            Err(LaunchRefusal::NotExe)
        );
        assert_eq!(
            check_launch_path("app.exe", &temps),
            Err(LaunchRefusal::NotAbsolute)
        );
        assert_eq!(
            check_launch_path("C:\\a\\..\\Temp\\b.exe", &temps),
            Err(LaunchRefusal::NotAbsolute)
        );
        assert_eq!(
            check_launch_path("\\\\server\\share\\a.exe", &temps),
            Err(LaunchRefusal::Network)
        );
        assert_eq!(
            check_launch_path("C:\\a\u{202E}exe.txt", &temps),
            Err(LaunchRefusal::Hidden)
        );
    }

    #[test]
    fn exe_paths_compare_without_case() {
        assert!(same_exe_path("C:\\A\\b.exe", "c:/a/B.EXE"));
        assert!(!same_exe_path("C:\\A\\b.exe", "C:\\A\\c.exe"));
    }

    #[test]
    fn a_workspace_round_trips_and_old_files_without_lists_load() {
        let w = ws(&[("C:\\x.exe", "monitor-1", 1)]);
        let back: Workspace = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
        assert_eq!(back, w);
        let bare: Workspace = serde_json::from_str(r#"{"id":"a","name":"b"}"#).unwrap();
        assert!(bare.entries.is_empty() && bare.layouts.is_empty());
    }

    #[test]
    fn the_config_keeps_valid_workspaces_and_old_configs_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, r#"{"schema_version":1,"settings":{},"monitors":[]}"#).unwrap();
        let (cfg, _) = crate::Config::load(&path).unwrap();
        assert!(cfg.workspaces.is_empty());
        assert!(!cfg.settings.focus.enabled && !cfg.settings.share_safe);

        let good = ws(&[("C:\\x.exe", "monitor-1", 1)]);
        let mut bad = good.clone();
        bad.id = "ws-2".into();
        bad.name = "\u{2066}x".into();
        let cfg = crate::Config {
            workspaces: vec![good.clone(), bad],
            ..crate::Config::default()
        };
        cfg.save(&path).unwrap();
        let (back, _) = crate::Config::load(&path).unwrap();
        assert_eq!(back.workspaces, vec![good]);
        assert_eq!(back.schema_version, 1);
    }
}
