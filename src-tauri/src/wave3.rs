//! Wave 3 in the app: workspaces, focus dim, calm desk, share-safe and the window
//! highlight. The pure logic lives in `krova_core::{workspace, focus, calm}`, the
//! OS work behind `krova_platform::desk::Desk`; this file wires them to the shared
//! state and exposes the Tauri commands.
//!
//! Kept in one file so merging with the engine rewrite stays cheap: the shared
//! files only gain small `// --- wave3 ---` blocks (a field in `Shared`, a field in
//! `AppState`, one call in `change_settings`, one at startup, the command list).
//!
//! Privacy: windows are known by handle, frame and executable path only; titles are
//! never read, logged or stored. Workspaces store executable paths, monitor ids and
//! zone ids, nothing else. Nothing here uses the network.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use krova_core::calm::{calm_targets, CalmDesk, CalmMonitor, CalmWindow};
use krova_core::focus::{self as kfocus, VisualsConfig};
use krova_core::registry::{SnapRecord, WindowKey};
use krova_core::workspace::{
    self as kws, check_launch_path, EntryOutcome, EntryResult, RestoreReport, Workspace,
    LAUNCH_WAIT_MS,
};
use krova_core::{MoveMode, PxRect, Settings};
use krova_platform::desk::{self, Desk, DeskCapabilities, Visuals};
use krova_platform::WindowId;
use serde::Serialize;

use crate::commands::change_settings;
use crate::model::{lock, AppState, Shared};

/// The wave-3 runtime state, one field of [`Shared`].
pub struct Wave3 {
    /// The OS seam. Replaceable so tests can use a fake desk.
    pub desk: Mutex<Arc<dyn Desk>>,
    /// The running dim / highlight overlays; `None` while both are off.
    visuals: Mutex<Option<Box<dyn Visuals>>>,
    visuals_error: Mutex<Option<String>>,
    /// Which windows calm desk minimised. In memory only.
    calm: Mutex<CalmDesk>,
    /// One workspace restore at a time; holds the id being restored.
    restoring: Mutex<Option<String>>,
}

impl Default for Wave3 {
    fn default() -> Self {
        Self {
            desk: Mutex::new(Arc::from(desk::default_desk())),
            visuals: Mutex::new(None),
            visuals_error: Mutex::new(None),
            calm: Mutex::new(CalmDesk::default()),
            restoring: Mutex::new(None),
        }
    }
}

impl Wave3 {
    pub fn desk(&self) -> Arc<dyn Desk> {
        Arc::clone(&lock(&self.desk))
    }
}

/// What the UI is told (`AppState.wave3`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Wave3State {
    pub capabilities: Wave3Capabilities,
    pub calm_desk: CalmDeskState,
    /// The focus dim is showing (on, and its overlays started).
    pub focus_active: bool,
    /// Mirror of `settings.share_safe`, so overlays and the quick panel can read it.
    pub share_safe: bool,
    /// Why the dim / highlight overlays could not start, if they could not.
    pub visuals_error: Option<String>,
    /// Id of the workspace being restored right now.
    pub restoring: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Wave3Capabilities {
    pub workspaces: bool,
    pub calm_desk: bool,
    pub focus_dim: bool,
    pub highlight: bool,
    pub dwm_borders: bool,
}

impl From<DeskCapabilities> for Wave3Capabilities {
    fn from(c: DeskCapabilities) -> Self {
        Self {
            workspaces: c.workspaces,
            calm_desk: c.calm_desk,
            focus_dim: c.focus_dim,
            highlight: c.highlight,
            dwm_borders: c.dwm_borders,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct CalmDeskState {
    pub active: bool,
    /// How many windows calm desk is holding minimised.
    pub count: usize,
}

pub fn status(shared: &Shared) -> Wave3State {
    let w = &shared.wave3;
    let settings = lock(&shared.model).config.settings.clone();
    let calm = lock(&w.calm);
    Wave3State {
        capabilities: w.desk().capabilities().into(),
        calm_desk: CalmDeskState {
            active: calm.is_active(),
            count: calm.count(),
        },
        focus_active: settings.focus.enabled && lock(&w.visuals).is_some(),
        share_safe: share_safe(shared),
        visuals_error: lock(&w.visuals_error).clone(),
        restoring: lock(&w.restoring).clone(),
    }
}

/// Fill in `AppState.wave3` (called from `commands::build_state`).
pub fn fill_state(shared: &Shared, state: &mut AppState) {
    state.wave3 = status(shared);
}

// ------------------------------------------------------------ share-safe

/// The one question every Krova overlay asks before showing an app name, an icon
/// or window contents: when true, show only zone shapes.
pub fn share_safe(shared: &Shared) -> bool {
    kfocus::share_safe(&lock(&shared.model).config.settings)
}

// ------------------------------------------------------- settings / visuals

fn visuals_config(settings: &Settings) -> VisualsConfig {
    VisualsConfig::from_settings(settings, &settings.focus, &settings.highlight)
}

/// Bring the overlays and the share-safe mirror in line with the settings. Called
/// at startup and after every settings change; cheap when nothing changed. Starts
/// the overlay thread only while focus dim or a highlight is on.
pub fn apply_settings(shared: &Shared) {
    let settings = lock(&shared.model).config.settings.clone();
    desk::set_share_safe(kfocus::share_safe(&settings));
    // --- integration ---
    crate::notch::set_share_safe(kfocus::share_safe(&settings));
    // --- end integration ---
    let want = (settings.enabled).then(|| visuals_config(&settings));
    let want = want.filter(VisualsConfig::is_active);
    let w = &shared.wave3;
    let desk = w.desk();
    let caps = desk.capabilities();
    let want = want.filter(|_| caps.focus_dim || caps.highlight);
    let mut running = lock(&w.visuals);
    match (want, running.as_ref()) {
        (None, _) => {
            *running = None;
            *lock(&w.visuals_error) = None;
        }
        (Some(cfg), Some(v)) => v.set_config(cfg),
        (Some(cfg), None) => match desk.start_visuals(cfg) {
            Ok(v) => {
                *running = Some(v);
                *lock(&w.visuals_error) = None;
            }
            Err(e) => *lock(&w.visuals_error) = Some(e.to_string()),
        },
    }
}

/// Hook for `commands::change_settings`: re-apply when a wave-3 relevant field changed.
pub fn on_settings_changed(shared: &Shared, old: &Settings, new: &Settings) {
    let relevant = |s: &Settings| {
        (
            s.enabled,
            s.focus,
            s.highlight.clone(),
            s.share_safe,
            kfocus::appearance(s),
        )
    };
    if relevant(old) != relevant(new) {
        apply_settings(shared);
    }
}

/// Start what the saved settings ask for (called once at startup).
pub fn start(shared: &Shared) {
    apply_settings(shared);
}

/// Flip focus dim. Returns the new state. For the command and, later, Ctrl+Alt+F.
pub fn toggle_focus(shared: &Shared) -> Result<bool, String> {
    let mut on = false;
    change_settings(shared, |s| {
        s.focus.enabled = !s.focus.enabled;
        on = s.focus.enabled;
    })?;
    lock(&shared.model)
        .persist()
        .map_err(|e| format!("could not save the config: {e}"))?;
    Ok(on)
}

// ------------------------------------------------------------ calm desk

/// Toggle calm desk: minimise every arrangeable window that is not in a zone of
/// its monitor's active layout, or restore exactly those that are still minimised.
/// Returns whether calm desk is now on.
pub fn toggle_calm_desk(shared: &Shared) -> Result<bool, String> {
    let w = &shared.wave3;
    let desk = w.desk();
    if !desk.capabilities().calm_desk {
        return Err("calm desk is not available on this system".into());
    }
    let mut calm = lock(&w.calm);
    if calm.is_active() {
        let back = calm.release(|id| desk.is_minimized(WindowId(id)));
        for id in back {
            let _ = desk.unminimize(WindowId(id));
        }
        return Ok(false);
    }

    let windows = desk.arrangeable_windows(false);
    let model = lock(&shared.model);
    let settings = &model.config.settings;
    let snapshots = model.snapshots();
    let cursor_monitor = shared
        .platform
        .cursor_position()
        .and_then(|(x, y)| model.profile_at_point(x, y));
    let layouts: Vec<Option<&krova_core::Layout>> = snapshots
        .iter()
        .map(|m| {
            model
                .config
                .monitors
                .iter()
                .find(|p| p.id == m.profile_id)
                .and_then(|p| p.active())
        })
        .collect();
    let monitors: Vec<CalmMonitor<'_>> = snapshots
        .iter()
        .zip(&layouts)
        .map(|(m, layout)| CalmMonitor {
            rect: m.rect,
            work: m.work,
            layout: *layout,
            gap: settings.zone_gap_px,
            included: settings.calm_desk.all_monitors
                || cursor_monitor
                    .as_deref()
                    .map_or(true, |c| c == m.profile_id),
        })
        .collect();
    let registry = lock(&shared.registry);
    let calm_windows: Vec<CalmWindow> = windows
        .iter()
        .map(|dw| CalmWindow {
            id: dw.id.0,
            rect: dw.rect,
            snapped: registry.get(WindowKey(dw.id.0)).is_some_and(|r| {
                model
                    .config
                    .monitors
                    .iter()
                    .find(|p| p.id == r.monitor_profile_id)
                    .is_some_and(|p| p.active_layout == r.layout_id)
            }),
        })
        .collect();
    let targets = calm_targets(&calm_windows, &monitors);
    drop(registry);
    drop(model);

    let done: Vec<u64> = targets
        .into_iter()
        .filter(|&id| desk.minimize(WindowId(id)).is_ok())
        .collect();
    calm.engage(done);
    Ok(true)
}

// ------------------------------------------------------------ workspaces

/// Save the windows on screen as a workspace. With `id`, replaces that workspace
/// (keeping its id); without, creates a new one.
pub fn capture_workspace(
    shared: &Shared,
    name: &str,
    id: Option<&str>,
) -> Result<Workspace, String> {
    let name = kws::normalize_name(name).map_err(|e| e.to_string())?;
    let desk = shared.wave3.desk();
    if !desk.capabilities().workspaces {
        return Err("workspaces are not available on this system".into());
    }
    let windows = desk.arrangeable_windows(false);
    let mut model = lock(&shared.model);
    let id = match id {
        Some(id) => {
            if !model.config.workspaces.iter().any(|w| w.id == id) {
                return Err(kws::WorkspaceError::Unknown.to_string());
            }
            id.to_owned()
        }
        None => kws::next_workspace_id(&model.config.workspaces),
    };
    let snapshots = model.snapshots();
    let registry = lock(&shared.registry);
    let captured = {
        let monitors: Vec<kws::CaptureMonitor<'_>> = snapshots
            .iter()
            .map(|m| kws::CaptureMonitor {
                monitor_id: &m.profile_id,
                rect: m.rect,
                work: m.work,
                layout: model
                    .config
                    .monitors
                    .iter()
                    .find(|p| p.id == m.profile_id)
                    .and_then(|p| p.active()),
            })
            .collect();
        let capture_windows: Vec<kws::CaptureWindow> = windows
            .iter()
            .filter_map(|dw| {
                Some(kws::CaptureWindow {
                    exe: dw.exe.clone()?,
                    rect: dw.rect,
                    snapped: registry
                        .get(WindowKey(dw.id.0))
                        .map(|r| (r.monitor_profile_id.clone(), r.layout_id.clone(), r.zone_id)),
                })
            })
            .collect();
        kws::capture(&id, &name, &monitors, &capture_windows)
    };
    drop(registry);
    kws::upsert_workspace(&mut model.config.workspaces, captured.clone())
        .map_err(|e| e.to_string())?;
    Ok(captured)
}

pub fn rename_workspace(shared: &Shared, id: &str, name: &str) -> Result<(), String> {
    let name = kws::normalize_name(name).map_err(|e| e.to_string())?;
    let mut model = lock(&shared.model);
    let ws = model
        .config
        .workspaces
        .iter_mut()
        .find(|w| w.id == id)
        .ok_or_else(|| kws::WorkspaceError::Unknown.to_string())?;
    ws.name = name;
    Ok(())
}

pub fn delete_workspace(shared: &Shared, id: &str) -> Result<(), String> {
    let mut model = lock(&shared.model);
    let before = model.config.workspaces.len();
    model.config.workspaces.retain(|w| w.id != id);
    if model.config.workspaces.len() == before {
        return Err(kws::WorkspaceError::Unknown.to_string());
    }
    Ok(())
}

fn program_name(exe: &str) -> String {
    krova_core::app_rules::exe_file_name(exe).to_owned()
}

/// Where a workspace entry goes now: `(target rect, work area, layout id)`.
fn entry_target(shared: &Shared, entry: &kws::WorkspaceEntry) -> Option<(PxRect, PxRect, String)> {
    let model = lock(&shared.model);
    let snap = model
        .snapshots()
        .into_iter()
        .find(|m| m.profile_id == entry.monitor_id)?;
    let layout = model
        .config
        .monitors
        .iter()
        .find(|p| p.id == entry.monitor_id)?
        .active()?;
    let rect =
        layout.zone_pixels_gapped(entry.zone_id, snap.work, model.config.settings.zone_gap_px)?;
    Some((rect, snap.work, layout.id.clone()))
}

/// Restore a workspace: activate its layouts, put an open window of each program
/// into its zone (launching missing programs when `launch` is set, waiting up to
/// 10 s for each first window), then minimise every other arrangeable window. Never
/// closes anything. Blocks for the animations; call it off the UI thread.
pub fn restore_workspace(shared: &Shared, id: &str, launch: bool) -> Result<RestoreReport, String> {
    let w = &shared.wave3;
    {
        let mut busy = lock(&w.restoring);
        if busy.is_some() {
            return Err("a workspace is already being restored".into());
        }
        *busy = Some(id.to_owned());
    }
    let result = restore_inner(shared, id, launch);
    *lock(&w.restoring) = None;
    result
}

fn restore_inner(shared: &Shared, id: &str, launch: bool) -> Result<RestoreReport, String> {
    let desk = shared.wave3.desk();
    if !desk.capabilities().workspaces {
        return Err("workspaces are not available on this system".into());
    }
    let (ws, animate) = {
        let mut model = lock(&shared.model);
        let ws = model
            .config
            .workspaces
            .iter()
            .find(|w| w.id == id)
            .cloned()
            .ok_or_else(|| kws::WorkspaceError::Unknown.to_string())?;
        // The layouts the workspace was captured with become active again (when they
        // still exist), so the zone ids mean what they meant.
        for l in &ws.layouts {
            let _ = model.config.set_active_layout(&l.monitor_id, &l.layout_id);
        }
        (ws, model.config.settings.move_mode == MoveMode::Animate)
    };

    let windows = desk.arrangeable_windows(true);
    // F-26: temp folders, Downloads, Desktop and AppData\Roaming.
    let blocked = desk.blocked_dirs();
    let pairs: Vec<(u64, Option<String>)> =
        windows.iter().map(|d| (d.id.0, d.exe.clone())).collect();
    let plan = kws::plan_restore(&ws, &pairs, launch, |p| {
        check_launch_path(p, &blocked).is_ok() && desk.file_exists(p)
    });

    let mut outcomes: Vec<Option<EntryOutcome>> = vec![None; ws.entries.len()];
    // The full path each refused entry was refused for (F-26).
    let mut refused_paths: Vec<Option<String>> = vec![None; ws.entries.len()];
    let mut assigned: Vec<(usize, WindowId, EntryOutcome)> = plan
        .reuse
        .iter()
        .map(|&(i, win)| (i, WindowId(win), EntryOutcome::Placed))
        .collect();

    // Launch every missing program first, then wait for all of them together.
    // The platform launches the final path of the file it checked; wait for that one.
    let mut started: Vec<(usize, String)> = Vec::new();
    for &i in &plan.launch {
        match desk.launch(&ws.entries[i].exe, &blocked) {
            Ok(path) => started.push((i, path)),
            Err(f) => {
                outcomes[i] = Some(EntryOutcome::Refused);
                refused_paths[i] = Some(f.path.unwrap_or_else(|| ws.entries[i].exe.clone()));
            }
        }
    }
    if !started.is_empty() {
        let exes: Vec<String> = started.iter().map(|(_, p)| p.clone()).collect();
        let known: Vec<WindowId> = windows.iter().map(|d| d.id).collect();
        let found = desk.wait_for_windows(&exes, &known, Duration::from_millis(LAUNCH_WAIT_MS));
        for (&(i, _), got) in started.iter().zip(found) {
            match got {
                Some(win) => assigned.push((i, win, EntryOutcome::Launched)),
                None => outcomes[i] = Some(EntryOutcome::TimedOut),
            }
        }
    }
    for &i in &plan.missing {
        let refused = launch && !ws.entries[i].exe.is_empty();
        if refused {
            refused_paths[i] = Some(ws.entries[i].exe.clone());
        }
        outcomes[i] = Some(if refused {
            EntryOutcome::Refused
        } else {
            EntryOutcome::NotRunning
        });
    }

    // Place each window in turn, in entry order, so they land one after another.
    assigned.sort_by_key(|(i, _, _)| *i);
    let minimized_now: HashSet<u64> = windows
        .iter()
        .filter(|d| d.minimized)
        .map(|d| d.id.0)
        .collect();
    for (i, win, ok) in assigned {
        let entry = &ws.entries[i];
        let Some((target, work, layout_id)) = entry_target(shared, entry) else {
            outcomes[i] = Some(EntryOutcome::NoZone);
            continue;
        };
        if minimized_now.contains(&win.0) || desk.is_minimized(win) {
            let _ = desk.unminimize(win);
        }
        match shared.platform.place_window(win, target, work, animate) {
            Ok(done) => {
                lock(&shared.registry).insert(
                    WindowKey(win.0),
                    SnapRecord {
                        monitor_profile_id: entry.monitor_id.clone(),
                        layout_id,
                        zone_id: entry.zone_id,
                        rect: done,
                    },
                );
                outcomes[i] = Some(ok);
            }
            Err(_) => outcomes[i] = Some(EntryOutcome::Failed),
        }
    }

    // Everything else steps aside (minimised, never closed).
    let mut minimized = 0;
    for id in &plan.minimize {
        if minimized_now.contains(id) {
            continue;
        }
        if desk.minimize(WindowId(*id)).is_ok() {
            minimized += 1;
        }
    }

    Ok(RestoreReport {
        workspace_id: ws.id.clone(),
        entries: ws
            .entries
            .iter()
            .zip(outcomes)
            .zip(refused_paths)
            .map(|((e, o), path)| {
                let outcome = o.unwrap_or(EntryOutcome::Failed);
                EntryResult {
                    program: program_name(&e.exe),
                    monitor_id: e.monitor_id.clone(),
                    zone_id: e.zone_id,
                    outcome,
                    path: path.filter(|_| outcome == EntryOutcome::Refused),
                }
            })
            .collect(),
        minimized,
    })
}

// ------------------------------------------------------------ commands

/// The Tauri commands (registered in `lib.rs`). Their names are the command names
/// the UI invokes; JS passes camelCase argument names.
pub mod cmd {
    use std::sync::Arc;

    use krova_core::workspace::RestoreReport;
    use tauri::{AppHandle, State};

    use crate::commands::commit;
    use crate::model::{AppState, Shared};

    /// Turn focus dim on or off.
    #[tauri::command]
    pub fn toggle_focus_dim(
        app: AppHandle,
        shared: State<'_, Arc<Shared>>,
    ) -> Result<AppState, String> {
        super::toggle_focus(&shared)?;
        commit(&app, &shared)
    }

    /// Calm desk on (minimise the strays) or off (bring them back).
    #[tauri::command]
    pub fn toggle_calm_desk(
        app: AppHandle,
        shared: State<'_, Arc<Shared>>,
    ) -> Result<AppState, String> {
        super::toggle_calm_desk(&shared)?;
        commit(&app, &shared)
    }

    /// Save the windows on screen as a workspace (new, or replacing `id`).
    #[tauri::command]
    pub fn capture_workspace(
        app: AppHandle,
        shared: State<'_, Arc<Shared>>,
        name: String,
        id: Option<String>,
    ) -> Result<AppState, String> {
        super::capture_workspace(&shared, &name, id.as_deref())?;
        commit(&app, &shared)
    }

    #[tauri::command]
    pub fn rename_workspace(
        app: AppHandle,
        shared: State<'_, Arc<Shared>>,
        id: String,
        name: String,
    ) -> Result<AppState, String> {
        super::rename_workspace(&shared, &id, &name)?;
        commit(&app, &shared)
    }

    #[tauri::command]
    pub fn delete_workspace(
        app: AppHandle,
        shared: State<'_, Arc<Shared>>,
        id: String,
    ) -> Result<AppState, String> {
        super::delete_workspace(&shared, &id)?;
        commit(&app, &shared)
    }

    /// Restore a workspace. Runs on a worker thread (it waits for launched programs
    /// and animates each window), then pushes the new state (`state-changed`).
    #[tauri::command(async)]
    pub fn restore_workspace(
        app: AppHandle,
        shared: State<'_, Arc<Shared>>,
        id: String,
        launch: bool,
    ) -> Result<RestoreReport, String> {
        let report = super::restore_workspace(&shared, &id, launch)?;
        commit(&app, &shared)?;
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use krova_core::focus::HighlightMode;
    use krova_core::{Config, MonitorDescriptor};
    use krova_platform::desk::DeskWindow;
    use krova_platform::overlay::Overlay;
    use krova_platform::{
        Capabilities, InputEvent, InputHandle, MonitorInfo, Permission, Platform, PlatformError,
    };

    use crate::model::AppModel;

    #[derive(Default)]
    struct FakeDesk {
        windows: Mutex<Vec<DeskWindow>>,
        minimized: Mutex<HashSet<u64>>,
        launched: Mutex<Vec<String>>,
        /// What `wait_for_windows` "finds" for a launched exe.
        appears: Mutex<Vec<(String, u64)>>,
        visuals_started: AtomicUsize,
        configs: Arc<Mutex<Vec<VisualsConfig>>>,
    }

    struct FakeVisuals(Arc<Mutex<Vec<VisualsConfig>>>);
    impl Visuals for FakeVisuals {
        fn set_config(&self, c: VisualsConfig) {
            lock(&self.0).push(c);
        }
    }

    impl Desk for FakeDesk {
        fn capabilities(&self) -> DeskCapabilities {
            DeskCapabilities {
                workspaces: true,
                calm_desk: true,
                focus_dim: true,
                highlight: true,
                dwm_borders: false,
            }
        }
        fn arrangeable_windows(&self, include_minimized: bool) -> Vec<DeskWindow> {
            let min = lock(&self.minimized).clone();
            lock(&self.windows)
                .iter()
                .cloned()
                .map(|mut w| {
                    w.minimized = min.contains(&w.id.0);
                    w
                })
                .filter(|w| include_minimized || !w.minimized)
                .collect()
        }
        fn minimize(&self, w: WindowId) -> Result<(), PlatformError> {
            lock(&self.minimized).insert(w.0);
            Ok(())
        }
        fn unminimize(&self, w: WindowId) -> Result<(), PlatformError> {
            lock(&self.minimized).remove(&w.0);
            Ok(())
        }
        fn is_minimized(&self, w: WindowId) -> bool {
            lock(&self.minimized).contains(&w.0)
        }
        fn file_exists(&self, path: &str) -> bool {
            !path.contains("gone")
        }
        fn launch(
            &self,
            exe: &str,
            blocked_dirs: &[String],
        ) -> Result<String, krova_platform::desk::LaunchFailure> {
            // Like the real desk: a path that resolves into a blocked folder is
            // refused with that final path.
            if let Some(real) = exe.strip_prefix("C:\\Apps\\link-") {
                let fin = format!("C:\\Users\\me\\Downloads\\{real}");
                if check_launch_path(&fin, blocked_dirs).is_err() {
                    return Err(krova_platform::desk::LaunchFailure::new(
                        Some(fin),
                        PlatformError::Os("refused".into()),
                    ));
                }
            }
            lock(&self.launched).push(exe.to_owned());
            Ok(exe.to_owned())
        }
        fn blocked_dirs(&self) -> Vec<String> {
            vec!["C:\\Users\\me\\Downloads".into()]
        }
        fn wait_for_windows(
            &self,
            exes: &[String],
            _known: &[WindowId],
            _timeout: Duration,
        ) -> Vec<Option<WindowId>> {
            let appears = lock(&self.appears).clone();
            exes.iter()
                .map(|e| {
                    appears
                        .iter()
                        .find(|(x, _)| x == e)
                        .map(|(_, id)| WindowId(*id))
                })
                .collect()
        }
        fn start_visuals(&self, c: VisualsConfig) -> Result<Box<dyn Visuals>, PlatformError> {
            self.visuals_started.fetch_add(1, Ordering::SeqCst);
            lock(&self.configs).push(c);
            Ok(Box::new(FakeVisuals(Arc::clone(&self.configs))))
        }
    }

    #[derive(Default)]
    struct FakePlatform {
        placed: Mutex<Vec<(u64, PxRect)>>,
    }

    impl Platform for FakePlatform {
        fn monitors(&self) -> Result<Vec<MonitorInfo>, PlatformError> {
            Ok(vec![monitor()])
        }
        fn place_window(
            &self,
            w: WindowId,
            target: PxRect,
            _work: PxRect,
            _animate: bool,
        ) -> Result<PxRect, PlatformError> {
            lock(&self.placed).push((w.0, target));
            Ok(target)
        }
        fn start_input(
            &self,
            _: krova_core::Modifier,
            _: std::sync::mpsc::Sender<InputEvent>,
        ) -> Result<InputHandle, PlatformError> {
            Err(PlatformError::Unsupported)
        }
        fn autostart_enabled(&self) -> Result<bool, PlatformError> {
            Ok(false)
        }
        fn set_autostart(&self, _: bool) -> Result<(), PlatformError> {
            Ok(())
        }
        fn create_overlay(&self) -> Result<Box<dyn Overlay>, PlatformError> {
            Err(PlatformError::Unsupported)
        }
        fn permission(&self) -> Permission {
            Permission::NotRequired
        }
        fn capabilities(&self) -> Capabilities {
            Capabilities::default()
        }
    }

    fn monitor() -> MonitorInfo {
        MonitorInfo {
            descriptor: MonitorDescriptor {
                hardware_id: Some("A".into()),
                device_path: None,
                rect: PxRect::new(0, 0, 2000, 1000),
            },
            work: PxRect::new(0, 0, 2000, 1000),
            scale: 1.0,
            label: "Display 1".into(),
            primary: true,
        }
    }

    fn win(id: u64, exe: &str, rect: PxRect) -> DeskWindow {
        DeskWindow {
            id: WindowId(id),
            exe: Some(exe.into()),
            rect,
            minimized: false,
        }
    }

    /// The share-safe mirror is one process-wide switch and every test here calls
    /// `apply_settings`, so they run one at a time: `setup` takes this lock and the
    /// test thread holds it until it ends.
    static SERIAL: Mutex<()> = Mutex::new(());
    thread_local! {
        static HELD: std::cell::RefCell<Option<std::sync::MutexGuard<'static, ()>>> =
            const { std::cell::RefCell::new(None) };
    }

    fn setup() -> (Shared, Arc<FakeDesk>, Arc<FakePlatform>) {
        let guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        HELD.with(|h| *h.borrow_mut() = Some(guard));
        let dir = tempfile::tempdir().unwrap();
        let mut model = AppModel::new(Config::default(), dir.path().join("c.json"), false);
        model.refresh_monitors(vec![monitor()]).unwrap();
        let platform = Arc::new(FakePlatform::default());
        let shared = Shared::new(model, Arc::clone(&platform) as Arc<dyn Platform>);
        let desk = Arc::new(FakeDesk::default());
        *lock(&shared.wave3.desk) = Arc::clone(&desk) as Arc<dyn Desk>;
        (shared, desk, platform)
    }

    fn monitor_id(shared: &Shared) -> String {
        lock(&shared.model).connected_ids()[0].clone()
    }

    #[test]
    fn capture_then_restore_reuses_launches_and_minimises() {
        let (shared, desk, platform) = setup();
        let left = PxRect::new(0, 0, 1000, 1000);
        let right = PxRect::new(1000, 0, 1000, 1000);
        *lock(&desk.windows) = vec![
            win(1, "C:\\Apps\\code.exe", left),
            win(2, "C:\\Apps\\mail.exe", right),
        ];
        let ws = capture_workspace(&shared, "  Coding ", None).unwrap();
        assert_eq!(ws.name, "Coding");
        assert_eq!(ws.entries.len(), 2);
        assert_eq!(lock(&shared.model).config.workspaces.len(), 1);

        // Later: code.exe is open but somewhere else, mail.exe is closed, chat.exe is open.
        *lock(&desk.windows) = vec![
            win(5, "C:\\apps\\CODE.exe", PxRect::new(300, 300, 400, 400)),
            win(6, "C:\\Apps\\chat.exe", left),
        ];
        *lock(&desk.appears) = vec![("C:\\Apps\\mail.exe".into(), 9)];
        let report = restore_workspace(&shared, &ws.id, true).unwrap();
        let outcomes: Vec<(String, EntryOutcome)> = report
            .entries
            .iter()
            .map(|e| (e.program.clone(), e.outcome))
            .collect();
        assert_eq!(
            outcomes,
            vec![
                ("code.exe".into(), EntryOutcome::Placed),
                ("mail.exe".into(), EntryOutcome::Launched),
            ]
        );
        assert_eq!(*lock(&desk.launched), vec!["C:\\Apps\\mail.exe".to_owned()]);
        assert_eq!(*lock(&platform.placed), vec![(5, left), (9, right)]);
        assert_eq!(report.minimized, 1);
        assert!(desk.is_minimized(WindowId(6)));
        // The placed windows are known to the snap registry.
        let reg = lock(&shared.registry);
        assert_eq!(reg.get(WindowKey(9)).map(|r| r.zone_id), Some(2));
        assert_eq!(
            reg.get(WindowKey(5)).map(|r| r.monitor_profile_id.clone()),
            Some(monitor_id(&shared))
        );
    }

    #[test]
    fn a_launch_whose_real_path_is_blocked_is_refused_with_that_path() {
        let (shared, desk, _) = setup();
        let rect = PxRect::new(0, 0, 1000, 1000);
        *lock(&desk.windows) = vec![
            win(1, "C:\\Apps\\link-tool.exe", rect),
            win(2, "C:\\Users\\me\\Downloads\\setup.exe", rect),
        ];
        let ws = capture_workspace(&shared, "A", None).unwrap();
        *lock(&desk.windows) = vec![];
        let r = restore_workspace(&shared, &ws.id, true).unwrap();
        // The stored path looked fine; its final path is in Downloads.
        assert_eq!(r.entries[0].outcome, EntryOutcome::Refused);
        assert_eq!(
            r.entries[0].path.as_deref(),
            Some("C:\\Users\\me\\Downloads\\tool.exe")
        );
        // A stored path in Downloads is never even tried.
        assert_eq!(r.entries[1].outcome, EntryOutcome::Refused);
        assert_eq!(
            r.entries[1].path.as_deref(),
            Some("C:\\Users\\me\\Downloads\\setup.exe")
        );
        assert!(lock(&desk.launched).is_empty());
    }

    #[test]
    fn restore_without_launching_reports_what_is_not_running() {
        let (shared, desk, _) = setup();
        *lock(&desk.windows) = vec![win(
            1,
            "C:\\Apps\\gone\\x.exe",
            PxRect::new(0, 0, 1000, 1000),
        )];
        let ws = capture_workspace(&shared, "A", None).unwrap();
        *lock(&desk.windows) = vec![];
        let r = restore_workspace(&shared, &ws.id, false).unwrap();
        assert_eq!(r.entries[0].outcome, EntryOutcome::NotRunning);
        // With launching on, a path that is gone is refused, never started.
        let r = restore_workspace(&shared, &ws.id, true).unwrap();
        assert_eq!(r.entries[0].outcome, EntryOutcome::Refused);
        assert_eq!(r.entries[0].path.as_deref(), Some("C:\\Apps\\gone\\x.exe"));
        assert!(lock(&desk.launched).is_empty());
        assert!(restore_workspace(&shared, "nope", true).is_err());
        assert!(lock(&shared.wave3.restoring).is_none());
    }

    #[test]
    fn a_launched_program_that_never_shows_times_out() {
        let (shared, desk, _) = setup();
        *lock(&desk.windows) = vec![win(1, "C:\\Apps\\slow.exe", PxRect::new(0, 0, 1000, 1000))];
        let ws = capture_workspace(&shared, "A", None).unwrap();
        *lock(&desk.windows) = vec![];
        let r = restore_workspace(&shared, &ws.id, true).unwrap();
        assert_eq!(r.entries[0].outcome, EntryOutcome::TimedOut);
    }

    #[test]
    fn workspaces_can_be_renamed_replaced_and_deleted() {
        let (shared, desk, _) = setup();
        *lock(&desk.windows) = vec![win(1, "C:\\a.exe", PxRect::new(0, 0, 1000, 1000))];
        let ws = capture_workspace(&shared, "One", None).unwrap();
        assert!(capture_workspace(&shared, "x\u{202E}y", None).is_err());
        rename_workspace(&shared, &ws.id, "Uno").unwrap();
        assert!(rename_workspace(&shared, &ws.id, " ").is_err());
        *lock(&desk.windows) = vec![];
        let replaced = capture_workspace(&shared, "Uno", Some(&ws.id)).unwrap();
        assert!(replaced.entries.is_empty());
        assert_eq!(lock(&shared.model).config.workspaces.len(), 1);
        assert!(capture_workspace(&shared, "Uno", Some("ws-99")).is_err());
        delete_workspace(&shared, &ws.id).unwrap();
        assert!(delete_workspace(&shared, &ws.id).is_err());
    }

    #[test]
    fn calm_desk_minimises_strays_and_restores_only_those_still_minimised() {
        let (shared, desk, _) = setup();
        *lock(&desk.windows) = vec![
            win(1, "C:\\a.exe", PxRect::new(0, 0, 1000, 1000)), // left half zone
            win(2, "C:\\b.exe", PxRect::new(400, 300, 500, 400)), // floating
            win(3, "C:\\c.exe", PxRect::new(100, 100, 300, 300)), // floating
        ];
        assert!(toggle_calm_desk(&shared).unwrap());
        assert!(!desk.is_minimized(WindowId(1)));
        assert!(desk.is_minimized(WindowId(2)) && desk.is_minimized(WindowId(3)));
        assert_eq!(
            status(&shared).calm_desk,
            CalmDeskState {
                active: true,
                count: 2
            }
        );
        // The user brought 3 back by hand and minimised 1 themselves.
        desk.unminimize(WindowId(3)).unwrap();
        desk.minimize(WindowId(1)).unwrap();
        assert!(!toggle_calm_desk(&shared).unwrap());
        assert!(!desk.is_minimized(WindowId(2)));
        assert!(desk.is_minimized(WindowId(1)), "not ours to restore");
        assert_eq!(status(&shared).calm_desk, CalmDeskState::default());
    }

    #[test]
    fn overlays_start_only_while_needed_and_follow_the_settings() {
        let (shared, desk, _) = setup();
        start(&shared);
        assert_eq!(desk.visuals_started.load(Ordering::SeqCst), 0);
        assert!(toggle_focus(&shared).unwrap());
        assert_eq!(desk.visuals_started.load(Ordering::SeqCst), 1);
        assert!(status(&shared).focus_active);
        change_settings(&shared, |s| s.highlight.mode = HighlightMode::Active).unwrap();
        let last = lock(&desk.configs).last().cloned().unwrap();
        assert_eq!(last.highlight.mode, HighlightMode::Active);
        assert_eq!(last.color, kfocus::DEFAULT_ACCENT);
        assert_eq!(
            desk.visuals_started.load(Ordering::SeqCst),
            1,
            "reconfigured, not restarted"
        );
        assert!(!toggle_focus(&shared).unwrap());
        assert!(
            lock(&shared.wave3.visuals).is_some(),
            "the highlight still needs them"
        );
        change_settings(&shared, |s| s.highlight.mode = HighlightMode::None).unwrap();
        assert!(lock(&shared.wave3.visuals).is_none());
        // Krova switched off: nothing runs.
        change_settings(&shared, |s| {
            s.focus.enabled = true;
            s.enabled = false;
        })
        .unwrap();
        assert!(lock(&shared.wave3.visuals).is_none());
    }

    #[test]
    fn share_safe_is_one_switch_for_every_overlay() {
        let (shared, _, _) = setup();
        assert!(!share_safe(&shared));
        change_settings(&shared, |s| s.share_safe = true).unwrap();
        assert!(share_safe(&shared));
        assert!(desk::share_safe());
        assert!(status(&shared).share_safe);
        change_settings(&shared, |s| s.share_safe = false).unwrap();
        assert!(!desk::share_safe());
    }
}
