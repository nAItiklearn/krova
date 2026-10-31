//! The engine: one thread that turns OS events into overlay updates and window
//! moves.
//!
//! It handles four kinds of message, all arriving on one channel:
//! - drag events from the mouse hook (zones overlay, snapping, the window memory
//!   registry: a drag of a registered window removes its entry);
//! - global keyboard shortcuts (move the focused window to a zone);
//! - window events (a registered window was resized: linked dividers; the display
//!   configuration may have changed: debounce, then put windows back in their zones);
//! - the debounce timer itself.
//!
//! `plan` is pure (snap-session actions in, effects out) and unit-tested; the
//! decisions behind the other features are pure too and live in `actions`.
//! Nothing here ever reads a window title. The registry holds window handles, zone
//! ids and rectangles, in memory only.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use krova_core::linked::{plan_linked_resize, MIN_LINKED_SIDE_PX};
use krova_core::registry::{plan_reconnect, KeyAction, WindowKey, WindowSnapshot};
use krova_core::{
    Action, DragEvent, MonitorView, MoveMode, PxRect, Session, Settings, SnapSettings, SnapTarget,
};
use krova_platform::overlay::{Overlay, OverlayScene, OverlayStyle};
use krova_platform::{
    EventSink, HotkeyEvent, HotkeySpec, InputEvent, PlatformError, WatchEvent, WindowId,
};

use crate::actions::{classify_change, plan_key_move, same_place, Change, Slot};
use crate::model::{lock, AppModel, Shared, ShortcutStatus};

/// How long the monitor set must stay quiet before the engine looks at it. A dock
/// or a wake from sleep makes displays flicker for a second or so.
pub const DISPLAY_DEBOUNCE: Duration = Duration::from_millis(1500);

/// A resize event for a window Krova itself just moved is ignored for this long.
const OWN_MOVE_GRACE: Duration = Duration::from_millis(400);

/// Messages the engine thread handles.
#[derive(Debug)]
pub enum Msg {
    Input(InputEvent),
    Hotkey(KeyAction),
    Watch(WatchEvent),
}

/// Which zone of which connected monitor a snap targets (monitor = index into the
/// connected monitors, as in the snap session).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZoneRef {
    pub monitor: usize,
    pub zone_id: u32,
}

/// What the overlay currently shows, accumulated from snap-session actions.
#[derive(Debug, Default)]
pub struct SceneState {
    zones_monitor: Option<usize>,
    zones: Vec<(u32, PxRect)>,
    highlight: Option<u32>,
    preview: Option<(usize, PxRect)>,
}

impl SceneState {
    fn scene(&self, monitors: &[MonitorView<'_>], settings: &Settings) -> Option<OverlayScene> {
        let index = self.zones_monitor.or(self.preview.map(|(m, _)| m))?;
        let monitor = monitors.get(index)?.rect;
        Some(OverlayScene {
            monitor,
            zones: self.zones.clone(),
            highlight: self.highlight,
            preview: self.preview.filter(|(m, _)| *m == index).map(|(_, r)| r),
            style: OverlayStyle::from_settings(
                &settings.highlight_color,
                settings.highlight_opacity,
            ),
            // The zone rects already carry the gap: draw them exactly.
            gapped: settings.zone_gap_px > 0,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Show this scene on the overlay (`None` hides it).
    Scene(Option<OverlayScene>),
    Place {
        window: WindowId,
        target: PxRect,
        work: PxRect,
        animate: bool,
        /// Set when the target is a zone (not an edge snap): the window is then
        /// remembered in the registry.
        zone: Option<ZoneRef>,
    },
}

/// Translate snap-session `actions` into effects. Scene changes within one batch
/// are merged into a single update, flushed before any window move.
pub fn plan(
    state: &mut SceneState,
    actions: &[Action],
    window: Option<WindowId>,
    monitors: &[MonitorView<'_>],
    settings: &Settings,
) -> Vec<Effect> {
    let mut out = Vec::new();
    let mut dirty = false;
    for action in actions {
        match *action {
            Action::ShowZones { monitor } => {
                let Some(m) = monitors.get(monitor) else {
                    continue;
                };
                // The overlay shows the gapped rects: what you see is what you get.
                let gap = settings.zone_gap_px;
                state.zones = m
                    .layout
                    .map(|l| {
                        l.zones
                            .iter()
                            .filter_map(|z| {
                                l.zone_pixels_gapped(z.id, m.work, gap).map(|r| (z.id, r))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                state.zones_monitor = Some(monitor);
                state.highlight = None;
                dirty = true;
            }
            Action::Highlight { zone, .. } => {
                state.highlight = zone;
                dirty = true;
            }
            Action::HideZones => {
                state.zones_monitor = None;
                state.zones.clear();
                state.highlight = None;
                dirty = true;
            }
            Action::EdgePreview { monitor, rect } => {
                state.preview = rect.map(|r| (monitor, r));
                dirty = true;
            }
            Action::Snap {
                monitor,
                rect,
                target,
            } => {
                if dirty {
                    out.push(Effect::Scene(state.scene(monitors, settings)));
                    dirty = false;
                }
                if let (Some(window), Some(m)) = (window, monitors.get(monitor)) {
                    out.push(Effect::Place {
                        window,
                        target: rect,
                        work: m.work,
                        animate: settings.move_mode == MoveMode::Animate,
                        zone: match target {
                            SnapTarget::Zone(zone_id) => Some(ZoneRef { monitor, zone_id }),
                            SnapTarget::Edge => None,
                        },
                    });
                }
            }
        }
    }
    if dirty {
        out.push(Effect::Scene(state.scene(monitors, settings)));
    }
    out
}

/// Start the engine thread, the mouse hook, the window watcher and the keyboard
/// shortcuts. Each part is optional: without the mouse hook (unsupported OS, missing
/// permission) the app still runs for editing layouts, and a platform that cannot
/// watch windows or register shortcuts simply goes without those features.
pub fn start(shared: &Arc<Shared>) {
    start_parts(shared, true);
}

/// Everything except the global mouse hook and the overlay window: what the
/// desktop integration tests need, without hooking the tester's mouse.
/// Gated like its only callers (the Windows-only desktop tests), so a macOS test build has no
/// unused function.
#[cfg(all(test, windows))]
pub fn start_headless(shared: &Arc<Shared>) {
    start_parts(shared, false);
}

fn start_parts(shared: &Arc<Shared>, with_mouse: bool) {
    let (tx, rx) = mpsc::channel::<Msg>();
    let overlay = if with_mouse {
        match shared.platform.create_overlay() {
            Ok(o) => Some(o),
            Err(e) => {
                eprintln!("krova: overlay is unavailable: {e}");
                None
            }
        }
    } else {
        None
    };
    let engine_shared = Arc::clone(shared);
    let spawned = thread::Builder::new()
        .name("krova-engine".into())
        .spawn(move || run(&rx, &engine_shared, overlay.as_deref()));
    if let Err(e) = spawned {
        eprintln!("krova: could not start the engine thread: {e}");
        return;
    }
    *lock(&shared.engine_tx) = Some(tx.clone());

    if with_mouse {
        start_input(shared, &tx);
    }
    start_watcher(shared, &tx);
    apply_shortcuts(shared);
}

/// Try to (re)start the mouse hook, for example after a permission was granted.
pub fn retry_input(shared: &Arc<Shared>) {
    let tx = lock(&shared.engine_tx).clone();
    if let Some(tx) = tx {
        start_input(shared, &tx);
    }
}

fn start_input(shared: &Shared, tx: &Sender<Msg>) {
    let (input_tx, input_rx) = mpsc::channel::<InputEvent>();
    let modifier = lock(&shared.model).config.settings.modifier;
    match shared.platform.start_input(modifier, input_tx) {
        Ok(handle) => *lock(&shared.input) = Some(handle),
        Err(e) => {
            eprintln!("krova: drag input is unavailable: {e}");
            return;
        }
    }
    // The platform sends plain `InputEvent`s; tag them for the engine's channel.
    let tx = tx.clone();
    let spawned = thread::Builder::new()
        .name("krova-input-fwd".into())
        .spawn(move || {
            for ev in input_rx {
                if tx.send(Msg::Input(ev)).is_err() {
                    break;
                }
            }
        });
    if let Err(e) = spawned {
        eprintln!("krova: could not start the input forwarder: {e}");
    }
}

fn start_watcher(shared: &Shared, tx: &Sender<Msg>) {
    if !shared.platform.capabilities().window_memory {
        return;
    }
    let tx = tx.clone();
    let sink: EventSink<WatchEvent> = Arc::new(move |ev| {
        let _ = tx.send(Msg::Watch(ev));
    });
    match shared.platform.start_watcher(sink) {
        Ok(handle) => lock(&shared.features).watcher = Some(handle),
        Err(e) => eprintln!("krova: window events are unavailable: {e}"),
    }
}

/// (Re)register the global shortcuts to match the settings: on only while Krova is
/// enabled and `keyboard_shortcuts` is set, with the chosen modifier. Safe to call
/// whenever settings change; the old registration is released first. The result
/// (and any chords the OS refused) is stored for the UI.
pub fn apply_shortcuts(shared: &Shared) {
    let _one_at_a_time = lock(&shared.shortcut_gate);
    let (wanted, modifier) = {
        let model = lock(&shared.model);
        let s = &model.config.settings;
        (s.enabled && s.keyboard_shortcuts, s.shortcut_modifier)
    };
    // Release the old chords before taking the new ones (they may overlap).
    let old = {
        let mut features = lock(&shared.features);
        features.shortcuts = ShortcutStatus::default();
        features.hotkeys.take()
    };
    drop(old);
    if !wanted || !shared.platform.capabilities().shortcuts {
        return;
    }
    let Some(tx) = lock(&shared.engine_tx).clone() else {
        return;
    };
    let sink: EventSink<HotkeyEvent> = Arc::new(move |ev| {
        let _ = tx.send(Msg::Hotkey(ev.action));
    });
    match shared.platform.start_hotkeys(HotkeySpec { modifier }, sink) {
        Ok(handle) => {
            let status = ShortcutStatus {
                active: handle.registered() > 0,
                failed: handle.failed().to_vec(),
            };
            let mut features = lock(&shared.features);
            features.shortcuts = status;
            features.hotkeys = Some(handle);
        }
        Err(e) => {
            eprintln!("krova: keyboard shortcuts are unavailable: {e}");
            lock(&shared.features).shortcuts = ShortcutStatus {
                active: false,
                failed: vec![format!("Keyboard shortcuts could not be started: {e}")],
            };
        }
    }
}

fn run(rx: &Receiver<Msg>, shared: &Shared, overlay: Option<&dyn Overlay>) {
    let mut engine = Engine::new(shared, overlay);
    loop {
        let message = match engine.display_deadline {
            Some(deadline) => {
                match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                    Ok(m) => Some(m),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
            None => match rx.recv() {
                Ok(m) => Some(m),
                Err(_) => break,
            },
        };
        match message {
            Some(Msg::Input(ev)) => engine.on_input(ev),
            Some(Msg::Hotkey(action)) => engine.on_hotkey(action),
            Some(Msg::Watch(ev)) => engine.on_watch(ev),
            None => {
                engine.display_deadline = None;
                engine.reconcile_monitors();
            }
        }
    }
}

struct Engine<'a> {
    shared: &'a Shared,
    overlay: Option<&'a dyn Overlay>,
    session: Session,
    scene: SceneState,
    /// Profile ids of the monitors connected when we last looked.
    connected: Vec<String>,
    /// When set, the monitor set is re-examined at this time (debounce).
    display_deadline: Option<Instant>,
    /// Windows Krova moved itself a moment ago; their own move/size events are ignored.
    placed_at: HashMap<WindowKey, Instant>,
}

impl<'a> Engine<'a> {
    fn new(shared: &'a Shared, overlay: Option<&'a dyn Overlay>) -> Self {
        let connected = lock(&shared.model).connected_ids();
        Self {
            shared,
            overlay,
            session: Session::new(SnapSettings::default()),
            scene: SceneState::default(),
            connected,
            display_deadline: None,
            placed_at: HashMap::new(),
        }
    }

    // ------------------------------------------------------------ drag input

    fn on_input(&mut self, ev: InputEvent) {
        let shared = self.shared;
        // A fresh press is a cheap moment to notice monitors that were plugged in
        // or out; the heavier work waits for the debounce.
        if matches!(ev.event, DragEvent::Press { .. }) {
            self.notice_monitor_changes();
        }
        let planned = {
            let model = lock(&shared.model);
            let settings = &model.config.settings;
            self.session.set_settings(SnapSettings {
                enabled: settings.enabled,
                edge_snapping: settings.edge_snapping,
                edge_px: settings.edge_size_px,
                gap_px: settings.zone_gap_px,
            });
            let views = model.views();
            let actions = self.session.handle(ev.event, &views);
            let effects = plan(&mut self.scene, &actions, ev.window, &views, settings);
            let profiles = model.connected_ids();
            effects
                .into_iter()
                .map(|effect| {
                    let slot = match &effect {
                        Effect::Place { zone: Some(z), .. } => views
                            .get(z.monitor)
                            .and_then(|v| v.layout)
                            .and_then(|layout| {
                                Some(Slot {
                                    profile_id: profiles.get(z.monitor)?.clone(),
                                    layout_id: layout.id.clone(),
                                    zone_id: z.zone_id,
                                })
                            }),
                        _ => None,
                    };
                    (effect, slot)
                })
                .collect::<Vec<_>>()
        };

        // The user started dragging this window: it is no longer where Krova put it.
        if self.session.is_dragging() {
            if let Some(window) = ev.window {
                lock(&shared.registry).remove(WindowKey(window.0));
            }
        }

        // The model lock is released: moving a window can take ~120 ms when animated.
        for (effect, slot) in planned {
            match effect {
                Effect::Scene(s) => {
                    if let Some(o) = self.overlay {
                        o.set_scene(s);
                    }
                }
                Effect::Place {
                    window,
                    target,
                    work,
                    animate,
                    ..
                } => match shared.platform.place_window(window, target, work, animate) {
                    Ok(done) => {
                        if let Some(slot) = slot {
                            self.remember(window, slot, done);
                        }
                    }
                    Err(e) => eprintln!("krova: could not move the window: {e}"),
                },
            }
        }
    }

    // ------------------------------------------------------------ registry

    /// Record that `window` now occupies `rect` in `slot`.
    fn remember(&mut self, window: WindowId, slot: Slot, rect: PxRect) {
        let shared = self.shared;
        let key = WindowKey(window.0);
        {
            let mut registry = lock(&shared.registry);
            // Windows that have been closed since are dropped now and then.
            registry.retain(|k, _| shared.platform.window_exists(WindowId(k.0)));
            registry.insert(key, slot.record(rect));
        }
        self.placed_at.insert(key, Instant::now());
        self.placed_at
            .retain(|_, at| at.elapsed() < OWN_MOVE_GRACE * 10);
    }

    fn placed_by_us_just_now(&self, key: WindowKey) -> bool {
        self.placed_at
            .get(&key)
            .is_some_and(|at| at.elapsed() < OWN_MOVE_GRACE)
    }

    // ------------------------------------------------------------ keyboard

    fn on_hotkey(&mut self, action: KeyAction) {
        let shared = self.shared;
        let (active, gap, animate) = {
            let model = lock(&shared.model);
            let s = &model.config.settings;
            (
                s.enabled && s.keyboard_shortcuts,
                s.zone_gap_px,
                s.move_mode == MoveMode::Animate,
            )
        };
        if !active {
            return;
        }
        let platform = &shared.platform;
        // The focused window; anything we must not or cannot move is left alone
        // (the desktop, the taskbar, elevated programs, minimized windows and
        // Krova's own windows).
        let Some(window) = platform.foreground_window() else {
            return;
        };
        if !platform.is_movable_target(window) {
            return;
        }
        let Ok(rect) = platform.window_rect(window) else {
            return;
        };
        let record = lock(&shared.registry).get(WindowKey(window.0)).cloned();
        let planned = {
            let model = lock(&shared.model);
            let views = model.views();
            plan_key_move(
                action,
                &views,
                &model.connected_ids(),
                rect,
                record.as_ref(),
                gap,
            )
        };
        let Some(planned) = planned else {
            return;
        };
        match platform.place_window(window, planned.target, planned.work, animate) {
            Ok(done) => self.remember(window, planned.slot, done),
            Err(e) => eprintln!("krova: could not move the window: {e}"),
        }
    }

    // ------------------------------------------------- window events / monitors

    fn on_watch(&mut self, ev: WatchEvent) {
        match ev {
            WatchEvent::DisplayChanged => {
                // Restart the quiet period: a dock flickers for a while.
                self.display_deadline = Some(Instant::now() + DISPLAY_DEBOUNCE);
            }
            // The decision is made at the end, by comparing with the registry.
            WatchEvent::MoveSizeStart(_) => {}
            WatchEvent::MoveSizeEnd(window) => self.on_move_size_end(window),
        }
    }

    /// Cheap check on a mouse press: if the set of monitors changed and no check is
    /// pending, schedule one. Also keeps the model's monitor list fresh, as before.
    fn notice_monitor_changes(&mut self) {
        let shared = self.shared;
        let Ok(infos) = shared.platform.monitors() else {
            return;
        };
        let ids = {
            let mut model = lock(&shared.model);
            if let Err(e) = model.refresh_monitors(infos) {
                eprintln!("krova: could not save the config: {e}");
            }
            model.connected_ids()
        };
        if ids != self.connected && self.display_deadline.is_none() {
            self.display_deadline = Some(Instant::now() + DISPLAY_DEBOUNCE);
        }
    }

    /// The monitor set may have changed and has been quiet for the debounce time:
    /// adopt it, and put windows back into the zones of monitors that came back.
    fn reconcile_monitors(&mut self) {
        let shared = self.shared;
        let Ok(infos) = shared.platform.monitors() else {
            return;
        };
        let previous = std::mem::take(&mut self.connected);
        let (settings, current) = {
            let mut model = lock(&shared.model);
            if let Err(e) = model.refresh_monitors(infos) {
                eprintln!("krova: could not save the config: {e}");
            }
            self.connected = model.connected_ids();
            (model.config.settings.clone(), model.connected())
        };
        if !settings.enabled || !settings.restore_on_reconnect {
            return;
        }

        let registry = lock(&shared.registry).clone();
        let returned: HashSet<&str> = current
            .iter()
            .map(|m| m.profile_id.as_str())
            .filter(|id| !previous.iter().any(|p| p == id))
            .collect();
        if returned.is_empty() || registry.is_empty() {
            return;
        }

        let platform = &shared.platform;
        let plan = {
            let model = lock(&shared.model);
            let mut windows = HashMap::new();
            for (key, record) in registry.iter() {
                if !returned.contains(record.monitor_profile_id.as_str()) {
                    continue;
                }
                windows.insert(key, snapshot(&**platform, &model, WindowId(key.0)));
            }
            plan_reconnect(
                &registry,
                &previous,
                &current,
                &model.config,
                &windows,
                settings.zone_gap_px,
            )
        };

        {
            let mut live = lock(&shared.registry);
            for key in &plan.forget {
                live.remove(*key);
            }
        }
        let animate = settings.move_mode == MoveMode::Animate;
        for p in plan.placements {
            let window = WindowId(p.key.0);
            match platform.place_window(window, p.target, p.work, animate) {
                Ok(done) => {
                    lock(&shared.registry).set_rect(p.key, done);
                    self.placed_at.insert(p.key, Instant::now());
                }
                Err(PlatformError::WindowGone) => {
                    lock(&shared.registry).remove(p.key);
                }
                Err(e) => eprintln!("krova: could not restore a window: {e}"),
            }
        }
    }

    /// The user finished moving or resizing a window (the OS move/size loop ended).
    fn on_move_size_end(&mut self, window: WindowId) {
        let shared = self.shared;
        let key = WindowKey(window.0);
        let (active, linked, gap) = {
            let model = lock(&shared.model);
            let s = &model.config.settings;
            (s.enabled, s.linked_dividers, s.zone_gap_px)
        };
        if !active || self.placed_by_us_just_now(key) {
            return;
        }
        let Some(record) = lock(&shared.registry).get(key).cloned() else {
            return;
        };
        let platform = &shared.platform;
        let forget = || {
            lock(&shared.registry).remove(key);
        };
        if !platform.window_exists(window) || platform.is_minimized(window) {
            forget();
            return;
        }
        let Ok(now) = platform.window_rect(window) else {
            forget();
            return;
        };
        match classify_change(record.rect, now) {
            Change::Same => {}
            // Placed somewhere else (for example with Alt+Space, Move).
            Change::Moved => forget(),
            Change::Resized => {
                // The maximize button is not a resize of the zone: forget it.
                if platform.is_maximized(window) || !linked {
                    forget();
                } else {
                    self.follow_resize(
                        key,
                        &record.monitor_profile_id,
                        &record.layout_id,
                        record.rect,
                        now,
                        gap,
                    );
                }
            }
        }
    }

    /// Linked dividers: the window `key` was resized from `old` to `now`; move the
    /// shared edge of the neighbours snapped on the same monitor and layout.
    fn follow_resize(
        &mut self,
        key: WindowKey,
        profile_id: &str,
        layout_id: &str,
        old: PxRect,
        now: PxRect,
        gap: i32,
    ) {
        let shared = self.shared;
        let platform = &shared.platform;

        // Neighbours as they are now. One that vanished is forgotten; one that was
        // moved or resized behind our back no longer sits in its zone, so it is
        // forgotten too and left out; minimized ones cannot follow.
        let candidates: Vec<(WindowKey, PxRect)> = lock(&shared.registry)
            .on_layout(profile_id, layout_id)
            .filter(|(k, _)| *k != key)
            .map(|(k, r)| (k, r.rect))
            .collect();
        let mut peers = Vec::new();
        for (peer, recorded) in candidates {
            let id = WindowId(peer.0);
            if !platform.window_exists(id) {
                lock(&shared.registry).remove(peer);
                continue;
            }
            if platform.is_minimized(id) || platform.is_maximized(id) {
                continue;
            }
            let Ok(rect) = platform.window_rect(id) else {
                continue;
            };
            if classify_change(recorded, rect) == Change::Same {
                peers.push((peer, rect));
            } else {
                lock(&shared.registry).remove(peer);
            }
        }

        let work = lock(&shared.model)
            .connected()
            .into_iter()
            .find(|m| m.profile_id == profile_id)
            .map(|m| m.work);
        let Some(work) = work else {
            lock(&shared.registry).remove(key);
            return;
        };
        let Some(plan) = plan_linked_resize(key, old, now, &peers, gap, MIN_LINKED_SIDE_PX) else {
            return;
        };

        for (peer, target) in plan.follows {
            match platform.place_window(WindowId(peer.0), target, work, false) {
                // Whatever the neighbour really ended up with is what we remember.
                Ok(done) => {
                    lock(&shared.registry).set_rect(peer, done);
                    self.placed_at.insert(peer, Instant::now());
                }
                Err(PlatformError::WindowGone) => {
                    lock(&shared.registry).remove(peer);
                }
                Err(e) => eprintln!("krova: could not move a neighbouring window: {e}"),
            }
        }
        let window = WindowId(key.0);
        let final_rect = if same_place(plan.resized, now) {
            now
        } else {
            self.placed_at.insert(key, Instant::now());
            platform
                .place_window(window, plan.resized, work, false)
                .unwrap_or(now)
        };
        lock(&shared.registry).set_rect(key, final_rect);
    }
}

/// What is known about a registered window right now.
fn snapshot(
    platform: &dyn krova_platform::Platform,
    model: &AppModel,
    window: WindowId,
) -> WindowSnapshot {
    if !platform.window_exists(window) {
        return WindowSnapshot {
            exists: false,
            minimized: false,
            maximized: false,
            rect: None,
            monitor_profile_id: None,
        };
    }
    let rect = platform.window_rect(window).ok();
    WindowSnapshot {
        exists: true,
        minimized: platform.is_minimized(window),
        maximized: platform.is_maximized(window),
        rect,
        monitor_profile_id: rect.and_then(|r| model.profile_at(r)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use krova_core::{templates, Layout, SnapTarget};

    fn views(layout: &Layout) -> [MonitorView<'_>; 1] {
        [MonitorView {
            rect: PxRect::new(0, 0, 1920, 1080),
            work: PxRect::new(0, 0, 1920, 1040),
            layout: Some(layout),
        }]
    }

    #[test]
    fn showing_and_highlighting_make_one_scene_update() {
        let layouts = templates();
        let v = views(&layouts[0]);
        let mut st = SceneState::default();
        let fx = plan(
            &mut st,
            &[
                Action::ShowZones { monitor: 0 },
                Action::Highlight {
                    monitor: 0,
                    zone: Some(2),
                },
            ],
            None,
            &v,
            &Settings::default(),
        );
        assert_eq!(fx.len(), 1);
        let Effect::Scene(Some(scene)) = &fx[0] else {
            panic!("expected a scene, got {:?}", fx[0]);
        };
        assert_eq!(scene.monitor, v[0].rect);
        assert_eq!(
            scene.zones,
            vec![
                (1, PxRect::new(0, 0, 960, 1040)),
                (2, PxRect::new(960, 0, 960, 1040))
            ]
        );
        assert_eq!(scene.highlight, Some(2));
        assert_eq!(scene.preview, None);
    }

    #[test]
    fn hiding_clears_the_scene() {
        let layouts = templates();
        let v = views(&layouts[0]);
        let mut st = SceneState::default();
        let s = Settings::default();
        plan(&mut st, &[Action::ShowZones { monitor: 0 }], None, &v, &s);
        let fx = plan(&mut st, &[Action::HideZones], None, &v, &s);
        assert_eq!(fx, vec![Effect::Scene(None)]);
    }

    #[test]
    fn edge_preview_shows_and_clears() {
        let layouts = templates();
        let v = views(&layouts[0]);
        let mut st = SceneState::default();
        let s = Settings::default();
        let r = PxRect::new(0, 0, 960, 1040);
        let fx = plan(
            &mut st,
            &[Action::EdgePreview {
                monitor: 0,
                rect: Some(r),
            }],
            None,
            &v,
            &s,
        );
        let Effect::Scene(Some(scene)) = &fx[0] else {
            panic!("expected a scene");
        };
        assert!(scene.zones.is_empty());
        assert_eq!(scene.preview, Some(r));
        let fx = plan(
            &mut st,
            &[Action::EdgePreview {
                monitor: 0,
                rect: None,
            }],
            None,
            &v,
            &s,
        );
        assert_eq!(fx, vec![Effect::Scene(None)]);
    }

    #[test]
    fn snap_hides_the_overlay_before_moving_the_window() {
        let layouts = templates();
        let v = views(&layouts[0]);
        let mut st = SceneState::default();
        let s = Settings::default();
        plan(&mut st, &[Action::ShowZones { monitor: 0 }], None, &v, &s);
        let target = PxRect::new(0, 0, 960, 1040);
        let fx = plan(
            &mut st,
            &[
                Action::HideZones,
                Action::Snap {
                    monitor: 0,
                    rect: target,
                    target: SnapTarget::Zone(1),
                },
            ],
            Some(WindowId(7)),
            &v,
            &s,
        );
        assert_eq!(
            fx,
            vec![
                Effect::Scene(None),
                Effect::Place {
                    window: WindowId(7),
                    target,
                    work: v[0].work,
                    animate: true,
                    zone: Some(ZoneRef {
                        monitor: 0,
                        zone_id: 1
                    })
                }
            ]
        );
    }

    #[test]
    fn an_edge_snap_is_not_a_zone_and_is_not_remembered() {
        let layouts = templates();
        let v = views(&layouts[0]);
        let mut st = SceneState::default();
        let fx = plan(
            &mut st,
            &[Action::Snap {
                monitor: 0,
                rect: PxRect::new(0, 0, 960, 1040),
                target: SnapTarget::Edge,
            }],
            Some(WindowId(3)),
            &v,
            &Settings::default(),
        );
        assert!(matches!(fx.as_slice(), [Effect::Place { zone: None, .. }]));
    }

    #[test]
    fn the_overlay_shows_the_gapped_rects() {
        let layouts = templates();
        let v = views(&layouts[0]);
        let mut st = SceneState::default();
        let s = Settings {
            zone_gap_px: 10,
            ..Settings::default()
        };
        let fx = plan(&mut st, &[Action::ShowZones { monitor: 0 }], None, &v, &s);
        let Effect::Scene(Some(scene)) = &fx[0] else {
            panic!("expected a scene");
        };
        assert_eq!(
            scene.zones,
            vec![
                (1, PxRect::new(10, 10, 945, 1020)),
                (2, PxRect::new(965, 10, 945, 1020))
            ]
        );
    }

    #[test]
    fn instant_mode_turns_animation_off() {
        let layouts = templates();
        let v = views(&layouts[0]);
        let mut st = SceneState::default();
        let s = Settings {
            move_mode: MoveMode::Instant,
            ..Settings::default()
        };
        let fx = plan(
            &mut st,
            &[Action::Snap {
                monitor: 0,
                rect: PxRect::new(0, 0, 960, 1040),
                target: SnapTarget::Edge,
            }],
            Some(WindowId(1)),
            &v,
            &s,
        );
        assert!(matches!(
            fx.as_slice(),
            [Effect::Place { animate: false, .. }]
        ));
    }

    #[test]
    fn a_snap_without_a_window_moves_nothing() {
        let layouts = templates();
        let v = views(&layouts[0]);
        let mut st = SceneState::default();
        let fx = plan(
            &mut st,
            &[Action::Snap {
                monitor: 0,
                rect: PxRect::new(0, 0, 960, 1040),
                target: SnapTarget::Zone(1),
            }],
            None,
            &v,
            &Settings::default(),
        );
        assert!(fx.is_empty());
    }

    #[test]
    fn actions_for_a_vanished_monitor_are_ignored() {
        let layouts = templates();
        let v = views(&layouts[0]);
        let mut st = SceneState::default();
        let fx = plan(
            &mut st,
            &[Action::ShowZones { monitor: 5 }],
            None,
            &v,
            &Settings::default(),
        );
        assert!(fx.is_empty());
    }
}
