//! Auto-arrange, stacks, swaps and the tab strips: the engine side of Wave 1.
//!
//! The decisions are made by `krova_core::arrange` / `swap` / `app_rules`; this file
//! feeds them OS events and applies their plans with the same animated move as a
//! drag snap. Per connected monitor (profile id) one `Stacks` says which window is
//! in which zone. Runtime state only: nothing here is written to disk, and nothing
//! reads a window title (tabs show the program's file name).
//!
//! Event flow:
//! - a window is shown (or restored): wait [`SETTLE`] (programs move and resize
//!   their window right after showing it), classify it, ask the app rules, pick the
//!   monitor, then `arrange::plan(WindowOpened)`;
//! - a window is destroyed, hidden or minimized: `WindowClosed`;
//! - the focus moves to a tracked window: `Focused` (a hidden tab comes forward);
//! - a tracked window is dragged onto an occupied zone: `swap::drop_on_zone`;
//! - a tracked window is moved away by hand: it stops being tracked;
//! - the active layout changes: windows follow their zone ids, the ones whose zone
//!   is gone are arranged again.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};

use krova_core::app_rules::{Decision, SkipReason, WindowKind};
use krova_core::arrange::{self, ArrangeContext, ArrangeEvent, Plan};
use krova_core::registry::WindowKey;
use krova_core::stack::{CycleDirection, Stacks};
use krova_core::swap::{self, Direction};
use krova_core::{MoveMode, PxRect, Theme};
use krova_platform::strips::short_name;
use krova_platform::{
    PlatformError, StripEvent, StripScene, StripTab, StripTheme, WindowId, WindowProbe,
};

use super::Engine;
use crate::actions::Slot;
use crate::model::{lock, FrontApp, MonitorCtx};

/// How long a newly shown window is left alone before it is arranged.
pub const SETTLE: Duration = Duration::from_millis(350);
/// How long after the user's own move or resize a tracked window is looked at again
/// (the snap that ends a drag arrives a moment after the OS move loop ended).
pub const MOVE_CHECK: Duration = Duration::from_millis(400);
/// Most windows waiting to be arranged at once.
const MAX_PENDING: usize = 256;
/// Most windows arranged by one "arrange this monitor now".
const MAX_ARRANGE_ALL: usize = 64;
/// Most probes cached.
const MAX_PROBES: usize = 1024;
/// A window this far (per edge) from its zone still counts as in the zone.
const IN_ZONE_PX: i32 = 8;

/// Engine-side auto-arrange state.
#[derive(Default)]
pub struct ArrangeState {
    /// Per monitor profile id.
    pub stacks: HashMap<String, Stacks>,
    /// The layout each monitor's stacks were built for.
    layout_of: HashMap<String, String>,
    /// The arranged window that has the focus.
    focused: Option<u64>,
    probes: HashMap<u64, WindowProbe>,
    /// Shown windows waiting for [`SETTLE`].
    pending: Vec<(Instant, WindowId)>,
    /// Tracked windows the user moved or resized, looked at after [`MOVE_CHECK`].
    checks: Vec<(Instant, WindowId)>,
    /// What the strips show now, so an unchanged list is not sent again.
    shown: Vec<StripScene>,
}

impl ArrangeState {
    /// The earliest moment something here has to be looked at.
    pub fn deadline(&self) -> Option<Instant> {
        self.pending
            .iter()
            .chain(&self.checks)
            .map(|(at, _)| *at)
            .min()
    }

    fn profile_of(&self, window: u64) -> Option<String> {
        self.stacks
            .iter()
            .find(|(_, s)| s.contains(window))
            .map(|(p, _)| p.clone())
    }

    fn tracked(&self, window: u64) -> bool {
        self.stacks.values().any(|s| s.contains(window))
    }

    /// Forget the windows waiting to be arranged (on pause).
    pub fn clear_pending(&mut self) {
        self.pending.clear();
    }
}

/// Stable key of a strip: the monitor profile and the zone.
pub fn strip_key(profile_id: &str, zone: u32) -> u64 {
    let mut h = DefaultHasher::new();
    profile_id.hash(&mut h);
    zone.hash(&mut h);
    h.finish()
}

/// `#rrggbb` to RGB; the default accent if it does not parse.
pub fn parse_accent(hex: &str) -> [u8; 3] {
    let h = hex.trim().trim_start_matches('#');
    let byte = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
    match (h.len(), byte(0), byte(2), byte(4)) {
        (6, Some(r), Some(g), Some(b)) => [r, g, b],
        _ => [0xff, 0x5b, 0x1f],
    }
}

/// Why a window is not arranged, as the short code `FrontApp.reason` carries.
pub fn skip_code(reason: SkipReason) -> &'static str {
    match reason {
        SkipReason::NeverTouch => "never_touch",
        SkipReason::Float => "float",
        SkipReason::Kind(kind) => match kind {
            WindowKind::Dialog => "dialog",
            WindowKind::Tool => "tool",
            WindowKind::OwnedPopup => "popup",
            WindowKind::Elevated => "elevated",
            _ => "other",
        },
    }
}

/// How the strips look: palette, accent, corner radius and share-safe.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StripLook {
    pub gap: i32,
    pub theme: StripTheme,
    pub accent: [u8; 3],
    pub radius_px: i32,
    pub hide_names: bool,
}

/// Build the strip scenes: one per zone that holds two or more windows.
pub fn strip_scenes(
    monitors: &[MonitorCtx],
    stacks: &HashMap<String, Stacks>,
    probes: &HashMap<u64, WindowProbe>,
    look: &StripLook,
) -> Vec<StripScene> {
    let mut out = Vec::new();
    for m in monitors {
        let Some(stacks) = stacks.get(&m.profile_id) else {
            continue;
        };
        let mut zones: Vec<u32> = stacks
            .occupied_zones()
            .filter(|&z| stacks.depth(z) >= 2)
            .collect();
        zones.sort_unstable();
        for zone in zones {
            let Some(rect) = m.layout.zone_pixels_gapped(zone, m.work, look.gap) else {
                continue;
            };
            let visible = stacks.visible(zone);
            let tabs = stacks
                .members(zone)
                .iter()
                .map(|&w| {
                    let probe = probes.get(&w);
                    StripTab {
                        window: WindowId(w),
                        label: probe
                            .and_then(|p| p.exe.as_deref())
                            .map(short_name)
                            .unwrap_or_else(|| "Window".to_owned()),
                        icon_path: probe.and_then(|p| p.exe_path.clone()),
                        active: visible == Some(w),
                    }
                })
                .collect();
            out.push(StripScene {
                key: strip_key(&m.profile_id, zone),
                zone: rect,
                tabs,
                theme: look.theme,
                accent: look.accent,
                radius_px: look.radius_px,
                scale: m.scale,
                hide_names: look.hide_names,
            });
        }
    }
    out
}

fn near(a: PxRect, b: PxRect) -> bool {
    (a.x - b.x).abs() <= IN_ZONE_PX
        && (a.y - b.y).abs() <= IN_ZONE_PX
        && (a.right() - b.right()).abs() <= IN_ZONE_PX
        && (a.bottom() - b.bottom()).abs() <= IN_ZONE_PX
}

impl Engine<'_> {
    fn arrange_available(&self) -> bool {
        self.shared.platform.capabilities().auto_arrange
    }

    /// Auto-arrange may act: Krova is on and not paused.
    fn arrange_active(&self) -> bool {
        lock(&self.shared.model).config.settings.enabled && !self.shared.is_paused()
    }

    fn probe(&mut self, window: WindowId) -> Option<WindowProbe> {
        if let Some(p) = self.arrange.probes.get(&window.0) {
            // A reused handle has another owner: probe again.
            if p.owner.is_some() && p.owner == self.shared.platform.window_owner(window) {
                return Some(p.clone());
            }
        }
        let p = self.shared.platform.probe_window(window)?;
        if self.arrange.probes.len() >= MAX_PROBES {
            self.arrange.probes.clear();
        }
        self.arrange.probes.insert(window.0, p.clone());
        Some(p)
    }

    // ---------------------------------------------------------------- events

    /// A window was shown or restored: arrange it after a moment.
    pub(super) fn on_window_shown(&mut self, window: WindowId) {
        if !self.arrange_available() || self.arrange.tracked(window.0) {
            return;
        }
        let a = &mut self.arrange;
        if a.pending.iter().any(|(_, w)| *w == window) || a.pending.len() >= MAX_PENDING {
            return;
        }
        a.pending.push((Instant::now() + SETTLE, window));
    }

    /// Run whatever is due: settled windows and move checks.
    pub(super) fn arrange_due(&mut self) {
        let now = Instant::now();
        let (due, rest): (Vec<_>, Vec<_>) = self
            .arrange
            .pending
            .drain(..)
            .partition(|(at, _)| *at <= now);
        self.arrange.pending = rest;
        for (_, w) in due {
            self.arrange_new(w);
        }
        let (due, rest): (Vec<_>, Vec<_>) = self
            .arrange
            .checks
            .drain(..)
            .partition(|(at, _)| *at <= now);
        self.arrange.checks = rest;
        for (_, w) in due {
            self.check_moved(w);
        }
    }

    /// Arrange one new window, if the rules and the monitor allow it.
    fn arrange_new(&mut self, window: WindowId) {
        if !self.arrange_active() || self.arrange.tracked(window.0) {
            return;
        }
        let platform = &self.shared.platform;
        if !platform.window_exists(window) || platform.is_minimized(window) {
            return;
        }
        let Some(probe) = self.probe(window) else {
            return;
        };
        let rules = lock(&self.shared.model).config.settings.app_rules.clone();
        let (preferred_zone, start_monitor) = match rules.decide(probe.kind, probe.exe.as_deref()) {
            Decision::Skip(_) => return,
            Decision::Arrange {
                preferred_zone,
                start_monitor,
            } => (preferred_zone, start_monitor.map(str::to_owned)),
        };
        let Some(m) = self.monitor_for_new(window, start_monitor.as_deref()) else {
            return;
        };
        if !m.arrange.enabled {
            return;
        }
        let stacks = self.arrange.stacks.entry(m.profile_id.clone()).or_default();
        let plan = arrange::plan(
            &ArrangeContext {
                layout: &m.layout,
                stacks,
                focused: self.arrange.focused,
                settings: m.arrange,
            },
            ArrangeEvent::WindowOpened {
                id: window.0,
                preferred_zone,
            },
        );
        self.arrange
            .layout_of
            .insert(m.profile_id.clone(), m.layout.id.clone());
        self.apply_plan(&m, &plan, None);
    }

    /// The monitor a new window goes to: the one an app rule names, else the one it
    /// appeared on, else the one under the cursor.
    fn monitor_for_new(&self, window: WindowId, rule: Option<&str>) -> Option<MonitorCtx> {
        let platform = &self.shared.platform;
        let model = lock(&self.shared.model);
        if let Some(m) = rule.and_then(|id| model.monitor_ctx(id)) {
            return Some(m);
        }
        let on = platform
            .window_rect(window)
            .ok()
            .and_then(|r| model.profile_at(r))
            .or_else(|| {
                platform
                    .cursor_position()
                    .and_then(|(x, y)| model.profile_at_point(x, y))
            })?;
        model.monitor_ctx(&on)
    }

    /// A window was destroyed, hidden or minimized.
    pub(super) fn on_window_left(&mut self, window: WindowId, destroyed: bool) {
        self.arrange.pending.retain(|(_, w)| *w != window);
        self.arrange.checks.retain(|(_, w)| *w != window);
        if destroyed {
            self.arrange.probes.remove(&window.0);
        }
        self.untrack(window);
    }

    /// Stop tracking a window; the others fill in if the monitor says so.
    fn untrack(&mut self, window: WindowId) {
        let id = window.0;
        if self.arrange.focused == Some(id) {
            self.arrange.focused = None;
        }
        let Some(profile) = self.arrange.profile_of(id) else {
            return;
        };
        let m = lock(&self.shared.model).monitor_ctx(&profile);
        let plan = match &m {
            Some(m) if self.arrange_active() => arrange::plan(
                &ArrangeContext {
                    layout: &m.layout,
                    stacks: &self.arrange.stacks[&profile],
                    focused: self.arrange.focused,
                    settings: m.arrange,
                },
                ArrangeEvent::WindowClosed { id },
            ),
            _ => Plan::default(),
        };
        match (&m, plan.is_empty()) {
            (Some(m), false) => self.apply_plan(m, &plan, Some(id)),
            _ => {
                if let Some(s) = self.arrange.stacks.get_mut(&profile) {
                    s.remove(id);
                }
                self.refresh_strips();
            }
        }
        if let Some(s) = self.arrange.stacks.get_mut(&profile) {
            s.remove(id);
        }
    }

    /// The focus moved.
    pub(super) fn on_focus(&mut self, window: WindowId) {
        let id = window.0;
        self.update_front_app(window);
        let Some(profile) = self.arrange.profile_of(id) else {
            return;
        };
        let Some(m) = lock(&self.shared.model).monitor_ctx(&profile) else {
            return;
        };
        let plan = arrange::plan(
            &ArrangeContext {
                layout: &m.layout,
                stacks: &self.arrange.stacks[&profile],
                focused: self.arrange.focused,
                settings: m.arrange,
            },
            ArrangeEvent::Focused { id },
        );
        self.arrange.focused = Some(id);
        // The OS already raised the focused window: only the stack order changes.
        if let Some(s) = self.arrange.stacks.get_mut(&profile) {
            let _ = plan.apply(s);
        }
        self.refresh_strips();
    }

    fn update_front_app(&mut self, window: WindowId) {
        let tracked = self.arrange.tracked(window.0);
        let probe = self.probe(window);
        let (paused, rules) = {
            let model = lock(&self.shared.model);
            (
                self.shared.is_paused(),
                model.config.settings.app_rules.clone(),
            )
        };
        let front = probe.map(|p| {
            let reason = if tracked {
                None
            } else if paused {
                Some("paused")
            } else {
                match rules.decide(p.kind, p.exe.as_deref()) {
                    Decision::Skip(r) => Some(skip_code(r)),
                    Decision::Arrange { .. } => Some("auto_arrange_off"),
                }
            };
            FrontApp {
                exe: p.exe.clone(),
                arranged: tracked,
                reason: reason.map(str::to_owned),
            }
        });
        *lock(&self.shared.front_app) = front;
    }

    /// The user finished moving or resizing a tracked window: look again shortly.
    pub(super) fn on_user_moved(&mut self, window: WindowId) {
        if self.arrange.tracked(window.0) && !self.arrange.checks.iter().any(|(_, w)| *w == window)
        {
            self.arrange
                .checks
                .push((Instant::now() + MOVE_CHECK, window));
        }
    }

    /// A tracked window that is no longer in its zone (and was not snapped to
    /// another one) floats from now on.
    fn check_moved(&mut self, window: WindowId) {
        let key = WindowKey(window.0);
        let record = lock(&self.shared.registry).get(key).cloned();
        let now = self.shared.platform.window_rect(window).ok();
        let still_in_zone = match (record, now) {
            (Some(r), Some(now)) => near(r.rect, now),
            _ => false,
        };
        if !still_in_zone {
            self.untrack(window);
        }
    }

    // ------------------------------------------------------- manual placement

    /// A window was put into a zone by hand (a drag snap or a shortcut). With `swap`
    /// a tracked window dropped on an occupied zone trades places with the window
    /// there; otherwise it simply joins that zone.
    pub(super) fn after_manual_place(&mut self, window: WindowId, slot: &Slot, swap_on_drop: bool) {
        if !self.arrange_available() {
            return;
        }
        let id = window.0;
        let m = lock(&self.shared.model).monitor_ctx(&slot.profile_id);
        let Some(m) = m.filter(|m| m.arrange.enabled && m.layout.id == slot.layout_id) else {
            self.untrack(window);
            return;
        };
        let current = self.arrange.profile_of(id);
        if current.as_deref() == Some(m.profile_id.as_str()) && swap_on_drop {
            let plan = swap::drop_on_zone(
                &m.layout,
                &self.arrange.stacks[&m.profile_id],
                id,
                slot.zone_id,
            );
            if !plan.is_empty() {
                self.apply_plan(&m, &plan, Some(id));
                return;
            }
        }
        if let Some(other) = current {
            if other != m.profile_id {
                self.untrack(window);
            } else if let Some(s) = self.arrange.stacks.get_mut(&other) {
                s.remove(id);
            }
        }
        let stacks = self.arrange.stacks.entry(m.profile_id.clone()).or_default();
        if stacks.push(slot.zone_id, id).is_ok() {
            self.arrange
                .layout_of
                .insert(m.profile_id.clone(), m.layout.id.clone());
        }
        self.refresh_strips();
    }

    /// A window was edge-snapped or otherwise placed outside any zone.
    pub(super) fn after_free_place(&mut self, window: WindowId) {
        self.untrack(window);
    }

    /// Swap the focused window with its neighbour in `dir` (a keyboard shortcut).
    /// Into an empty neighbour it simply moves. Returns whether anything moved.
    pub(super) fn swap_focused(&mut self, dir: Direction) -> bool {
        let Some(window) = self.shared.platform.foreground_window() else {
            return false;
        };
        let Some(profile) = self.arrange.profile_of(window.0) else {
            return false;
        };
        let Some(m) = lock(&self.shared.model).monitor_ctx(&profile) else {
            return false;
        };
        let plan =
            swap::swap_with_neighbour(&m.layout, &self.arrange.stacks[&profile], window.0, dir);
        if plan.is_empty() {
            return false;
        }
        self.apply_plan(&m, &plan, None);
        true
    }

    /// Step through the stack of the focused window's zone.
    pub(super) fn cycle_focused(&mut self, forward: bool) {
        let Some(window) = self.shared.platform.foreground_window() else {
            return;
        };
        let Some(profile) = self.arrange.profile_of(window.0) else {
            return;
        };
        let Some(zone) = self.arrange.stacks[&profile].zone_of(window.0) else {
            return;
        };
        self.cycle(&profile, zone, forward);
    }

    fn cycle(&mut self, profile: &str, zone: u32, forward: bool) {
        let dir = if forward {
            CycleDirection::Next
        } else {
            CycleDirection::Prev
        };
        let Some(next) = self
            .arrange
            .stacks
            .get_mut(profile)
            .and_then(|s| s.cycle(zone, dir))
        else {
            return;
        };
        self.arrange.focused = Some(next);
        let _ = self.shared.platform.activate_window(WindowId(next));
        self.refresh_strips();
    }

    /// A tab was clicked or the wheel turned over a strip.
    pub(super) fn on_strip(&mut self, event: StripEvent) {
        match event {
            StripEvent::Select { window } => {
                let found = self
                    .arrange
                    .stacks
                    .values_mut()
                    .any(|s| s.bring_to_front(window.0));
                if found {
                    self.arrange.focused = Some(window.0);
                    let _ = self.shared.platform.activate_window(window);
                    self.refresh_strips();
                }
            }
            StripEvent::Cycle { key, forward } => {
                let target = self.arrange.stacks.iter().find_map(|(p, s)| {
                    s.occupied_zones()
                        .find(|&z| strip_key(p, z) == key)
                        .map(|z| (p.clone(), z))
                });
                if let Some((profile, zone)) = target {
                    self.cycle(&profile, zone, forward);
                }
            }
        }
    }

    // ------------------------------------------------------- layout / config

    /// Settings or layouts changed: follow the active layouts and the switches.
    pub(super) fn on_config_changed(&mut self) {
        if !self.arrange_available() {
            return;
        }
        let monitors = lock(&self.shared.model).monitor_ctxs();
        let connected: Vec<&str> = monitors.iter().map(|m| m.profile_id.as_str()).collect();
        self.arrange
            .stacks
            .retain(|p, _| connected.contains(&p.as_str()));
        self.arrange
            .layout_of
            .retain(|p, _| connected.contains(&p.as_str()));
        for m in &monitors {
            if !m.arrange.enabled {
                self.arrange.stacks.remove(&m.profile_id);
                self.arrange.layout_of.remove(&m.profile_id);
                continue;
            }
            let previous = self.arrange.layout_of.get(&m.profile_id).cloned();
            if previous.as_deref().is_some_and(|l| l != m.layout.id) {
                self.relayout(m);
            }
        }
        self.refresh_strips();
    }

    /// The active layout of a monitor changed: every tracked window goes to the zone
    /// with the same id in the new layout; windows whose zone is gone are arranged
    /// again from scratch.
    fn relayout(&mut self, m: &MonitorCtx) {
        self.arrange
            .layout_of
            .insert(m.profile_id.clone(), m.layout.id.clone());
        let Some(stacks) = self.arrange.stacks.get_mut(&m.profile_id) else {
            return;
        };
        let lost = stacks.retain_zones(|z| m.layout.zones.iter().any(|zone| zone.id == z));
        let kept = stacks.assignments();
        for (w, zone) in kept {
            self.place_in_zone(m, w, zone);
        }
        for w in lost {
            self.arrange_new(WindowId(w));
        }
    }

    /// "Arrange this monitor now": every ordinary window on it that is not tracked
    /// yet is arranged, back to front.
    pub(super) fn arrange_monitor(&mut self, profile: &str) {
        if !self.arrange_available() {
            return;
        }
        let windows = self.shared.platform.top_level_windows();
        let on_monitor: Vec<WindowId> = {
            let model = lock(&self.shared.model);
            windows
                .into_iter()
                .filter(|w| !self.arrange.tracked(w.0))
                .filter(|w| {
                    self.shared
                        .platform
                        .window_rect(*w)
                        .ok()
                        .and_then(|r| model.profile_at(r))
                        .as_deref()
                        == Some(profile)
                })
                .collect()
        };
        for w in on_monitor.into_iter().rev().take(MAX_ARRANGE_ALL) {
            self.arrange_new(w);
        }
    }

    // ---------------------------------------------------------------- applying

    fn move_settings(&self) -> (i32, bool) {
        let model = lock(&self.shared.model);
        let s = &model.config.settings;
        (s.zone_gap_px, s.move_mode == MoveMode::Animate)
    }

    /// Move window `w` into `zone` of monitor `m` and remember it there.
    fn place_in_zone(&mut self, m: &MonitorCtx, w: u64, zone: u32) -> bool {
        let (gap, animate) = self.move_settings();
        let Some(target) = m.layout.zone_pixels_gapped(zone, m.work, gap) else {
            return false;
        };
        let window = WindowId(w);
        match self
            .shared
            .platform
            .place_window(window, target, m.work, animate)
        {
            Ok(done) => {
                let slot = Slot {
                    profile_id: m.profile_id.clone(),
                    layout_id: m.layout.id.clone(),
                    zone_id: zone,
                };
                self.remember(window, slot, done);
                true
            }
            Err(PlatformError::WindowGone) => false,
            Err(e) => {
                eprintln!("krova: could not arrange a window: {e}");
                false
            }
        }
    }

    /// Apply a plan on monitor `m`: move the windows (all but `skip`, which is
    /// already where it belongs), update the stacks, raise the revealed tabs.
    fn apply_plan(&mut self, m: &MonitorCtx, plan: &Plan, skip: Option<u64>) {
        if plan.is_empty() {
            return;
        }
        let mut gone = Vec::new();
        for p in &plan.placements {
            if Some(p.window) == skip {
                continue;
            }
            if !self.place_in_zone(m, p.window, p.zone)
                && !self.shared.platform.window_exists(WindowId(p.window))
            {
                gone.push(p.window);
            }
        }
        let stacks = self.arrange.stacks.entry(m.profile_id.clone()).or_default();
        if plan.apply(stacks).is_err() {
            eprintln!("krova: an arrange plan did not fit the stacks; skipped");
        }
        for w in gone {
            stacks.remove(w);
        }
        for &w in &plan.raise {
            let _ = self.shared.platform.raise_window(WindowId(w));
        }
        self.refresh_strips();
    }

    /// Send the strips that match the stacks now (only when they changed).
    pub(super) fn refresh_strips(&mut self) {
        let Some(host) = self.strips else {
            return;
        };
        let (monitors, gap, theme, accent) = {
            let model = lock(&self.shared.model);
            let s = &model.config.settings;
            (
                model.monitor_ctxs(),
                s.zone_gap_px,
                s.theme,
                parse_accent(&s.highlight_color),
            )
        };
        let dark = match theme {
            Theme::Dark => true,
            Theme::Light => false,
            Theme::System => self.shared.platform.prefers_dark(),
        };
        let scenes = strip_scenes(
            &monitors,
            &self.arrange.stacks,
            &self.arrange.probes,
            &StripLook {
                gap,
                theme: if dark {
                    StripTheme::Dark
                } else {
                    StripTheme::Light
                },
                accent,
                radius_px: 10,
                hide_names: self.shared.hide_app_names(),
            },
        );
        if scenes != self.arrange.shown {
            self.arrange.shown = scenes.clone();
            host.set_strips(scenes);
        }
    }

    /// Monitors were connected or disconnected: forget the stacks of the gone ones.
    pub(super) fn arrange_monitors_changed(&mut self) {
        self.on_config_changed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use krova_core::{templates, Layout};

    fn ctx(id: &str, layout: Layout) -> MonitorCtx {
        MonitorCtx {
            profile_id: id.to_owned(),
            work: PxRect::new(0, 0, 1920, 1040),
            scale: 1.25,
            layout,
            arrange: Default::default(),
        }
    }

    fn probe(exe: &str) -> WindowProbe {
        WindowProbe {
            kind: WindowKind::Normal,
            exe: Some(exe.to_owned()),
            exe_path: Some(format!("C:\\Apps\\{exe}")),
            owner: None,
        }
    }

    #[test]
    fn accents_parse_and_fall_back() {
        assert_eq!(parse_accent("#3b82f6"), [0x3b, 0x82, 0xf6]);
        assert_eq!(parse_accent("ff5b1f"), [0xff, 0x5b, 0x1f]);
        assert_eq!(parse_accent("nope"), [0xff, 0x5b, 0x1f]);
        assert_eq!(parse_accent("#12345"), [0xff, 0x5b, 0x1f]);
    }

    #[test]
    fn strip_keys_tell_monitors_and_zones_apart() {
        assert_eq!(strip_key("monitor-1", 2), strip_key("monitor-1", 2));
        assert_ne!(strip_key("monitor-1", 2), strip_key("monitor-1", 3));
        assert_ne!(strip_key("monitor-1", 2), strip_key("monitor-2", 2));
    }

    #[test]
    fn only_zones_with_two_or_more_windows_get_a_strip() {
        let m = ctx("monitor-1", templates()[0].clone());
        let mut s = Stacks::new();
        s.push(1, 10).unwrap();
        s.push(2, 20).unwrap();
        s.push(2, 21).unwrap();
        let stacks = HashMap::from([("monitor-1".to_owned(), s)]);
        let probes = HashMap::from([(20, probe("chrome.exe")), (21, probe("code.exe"))]);
        let scenes = strip_scenes(
            &[m],
            &stacks,
            &probes,
            &StripLook {
                gap: 0,
                theme: StripTheme::Dark,
                accent: [1, 2, 3],
                radius_px: 10,
                hide_names: false,
            },
        );
        assert_eq!(scenes.len(), 1);
        let sc = &scenes[0];
        assert_eq!(sc.key, strip_key("monitor-1", 2));
        assert_eq!(sc.zone, PxRect::new(960, 0, 960, 1040));
        assert_eq!(sc.scale, 1.25);
        let labels: Vec<&str> = sc.tabs.iter().map(|t| t.label.as_str()).collect();
        assert!(labels.contains(&"Chrome") && labels.contains(&"Code"));
        assert_eq!(sc.tabs.iter().filter(|t| t.active).count(), 1);
    }

    #[test]
    fn a_window_without_a_known_program_is_labelled_neutrally() {
        let m = ctx("monitor-1", templates()[0].clone());
        let mut s = Stacks::new();
        s.push(1, 10).unwrap();
        s.push(1, 11).unwrap();
        let stacks = HashMap::from([("monitor-1".to_owned(), s)]);
        let scenes = strip_scenes(
            &[m],
            &stacks,
            &HashMap::new(),
            &StripLook {
                gap: 0,
                theme: StripTheme::Light,
                accent: [1, 2, 3],
                radius_px: 10,
                hide_names: true,
            },
        );
        assert!(scenes[0]
            .tabs
            .iter()
            .all(|t| t.label == "Window" && t.icon_path.is_none()));
        assert!(scenes[0].hide_names);
    }

    #[test]
    fn skip_reasons_have_short_codes() {
        assert_eq!(skip_code(SkipReason::Float), "float");
        assert_eq!(skip_code(SkipReason::NeverTouch), "never_touch");
        assert_eq!(skip_code(SkipReason::Kind(WindowKind::Dialog)), "dialog");
        assert_eq!(skip_code(SkipReason::Kind(WindowKind::Splash)), "other");
    }
}

#[cfg(test)]
mod flow_tests;
