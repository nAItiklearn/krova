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
use krova_core::registry::{plan_reconnect, KeyAction, WindowKey, WindowSnapshot, MAX_AGE};
use krova_core::{
    Action, DragEvent, MonitorView, MoveMode, PxRect, Session, Settings, SnapSettings, SnapTarget,
};
mod arrange;

use krova_platform::overlay::{Overlay, OverlayScene, OverlayStyle};
use krova_platform::{
    ChordReport, EventSink, HotkeyEvent, HotkeyHandle, HotkeySpec, InputEvent, PlatformError,
    StripEvent, StripHost, WatchEvent, WindowId,
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
    /// A tab of a stack strip was clicked, or the wheel turned over a strip.
    Strip(StripEvent),
    /// Settings or layouts were changed (by the UI or the tray): follow them.
    ConfigChanged,
    /// Arrange every ordinary window on this monitor (profile id) now.
    ArrangeMonitor(String),
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
    // The stack tab strips are native windows like the overlay: not in headless runs.
    let strips = if with_mouse && shared.platform.capabilities().auto_arrange {
        let strip_tx = tx.clone();
        let sink: EventSink<StripEvent> = Arc::new(move |ev| {
            let _ = strip_tx.send(Msg::Strip(ev));
        });
        match shared.platform.create_strip_host(sink) {
            Ok(s) => Some(s),
            Err(e) => {
                eprintln!("krova: stack tab strips are unavailable: {e}");
                None
            }
        }
    } else {
        None
    };
    let engine_shared = Arc::clone(shared);
    let spawned = thread::Builder::new()
        .name("krova-engine".into())
        .spawn(move || run(&rx, &engine_shared, overlay.as_deref(), strips.as_deref()));
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

/// Hand a message to the engine thread (no-op before it has started).
pub fn notify(shared: &Shared, msg: Msg) {
    if let Some(tx) = lock(&shared.engine_tx).as_ref() {
        let _ = tx.send(msg);
    }
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

/// Which shortcut groups the settings ask for: the twelve that move the focused
/// window (`keyboard_shortcuts`) and `<mods>+Space` for the layout switcher
/// (`layout_switcher_shortcut`), each only while Krova is enabled, both with the
/// chosen modifier. `None` when neither is wanted.
pub fn hotkey_request(settings: &Settings) -> Option<HotkeySpec> {
    let window_moves = settings.enabled && settings.keyboard_shortcuts;
    let switcher = settings.enabled && settings.layout_switcher_shortcut;
    (window_moves || switcher).then_some(HotkeySpec {
        modifier: settings.shortcut_modifier,
        window_moves,
        switcher,
    })
}

/// What the UI is told after the OS answered: the status of the window-move
/// shortcuts and of the switcher's shortcut. A group that was not requested has
/// nothing registered and nothing refused, so it reads as inactive and empty.
pub fn shortcut_statuses(handle: &HotkeyHandle) -> (ShortcutStatus, ShortcutStatus) {
    let status = |report: ChordReport| ShortcutStatus {
        active: report.registered > 0,
        failed: report.failed,
    };
    (status(handle.window_moves()), status(handle.switcher()))
}

/// Statuses for the case that the shortcut thread could not even be started.
fn unstarted_statuses(spec: HotkeySpec, why: &str) -> (ShortcutStatus, ShortcutStatus) {
    let failed = |wanted: bool, what: &str| ShortcutStatus {
        active: false,
        failed: if wanted {
            vec![format!("{what} could not be started: {why}")]
        } else {
            Vec::new()
        },
    };
    (
        failed(spec.window_moves, "Keyboard shortcuts"),
        failed(spec.switcher, "The layout switcher shortcut"),
    )
}

/// (Re)register the global shortcuts to match the settings (see [`hotkey_request`]).
/// Safe to call whenever settings change; the old registration is released first.
/// The result (and any chords the OS refused) is stored for the UI.
pub fn apply_shortcuts(shared: &Shared) {
    let _one_at_a_time = lock(&shared.shortcut_gate);
    let spec = hotkey_request(&lock(&shared.model).config.settings);
    // Release the old chords before taking the new ones (they may overlap).
    let old = {
        let mut features = lock(&shared.features);
        features.shortcuts = ShortcutStatus::default();
        features.layout_switcher = ShortcutStatus::default();
        features.hotkeys.take()
    };
    drop(old);
    let Some(spec) = spec else {
        return;
    };
    if !shared.platform.capabilities().shortcuts {
        return;
    }
    let Some(tx) = lock(&shared.engine_tx).clone() else {
        return;
    };
    let sink: EventSink<HotkeyEvent> = Arc::new(move |ev| {
        let _ = tx.send(Msg::Hotkey(ev.action));
    });
    match shared.platform.start_hotkeys(spec, sink) {
        Ok(handle) => {
            let (shortcuts, layout_switcher) = shortcut_statuses(&handle);
            let mut features = lock(&shared.features);
            features.shortcuts = shortcuts;
            features.layout_switcher = layout_switcher;
            features.hotkeys = Some(handle);
        }
        Err(e) => {
            eprintln!("krova: keyboard shortcuts are unavailable: {e}");
            let (shortcuts, layout_switcher) = unstarted_statuses(spec, &e.to_string());
            let mut features = lock(&shared.features);
            features.shortcuts = shortcuts;
            features.layout_switcher = layout_switcher;
        }
    }
}

fn run(
    rx: &Receiver<Msg>,
    shared: &Shared,
    overlay: Option<&dyn Overlay>,
    strips: Option<&dyn StripHost>,
) {
    let mut engine = Engine::new(shared, overlay, strips);
    loop {
        // Sleep until a message arrives or the earliest timer is due; with no timer
        // set, block: nothing runs while nothing happens.
        let deadline = [engine.display_deadline, engine.arrange.deadline()]
            .into_iter()
            .flatten()
            .min();
        let message = match deadline {
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
            Some(Msg::Strip(ev)) => engine.on_strip(ev),
            Some(Msg::ConfigChanged) => engine.on_config_changed(),
            Some(Msg::ArrangeMonitor(profile)) => engine.arrange_monitor(&profile),
            None => {}
        }
        let now = Instant::now();
        if engine.display_deadline.is_some_and(|d| d <= now) {
            engine.display_deadline = None;
            engine.reconcile_monitors();
        }
        if engine.arrange.deadline().is_some_and(|d| d <= now) {
            engine.arrange_due();
        }
    }
}

struct Engine<'a> {
    shared: &'a Shared,
    overlay: Option<&'a dyn Overlay>,
    strips: Option<&'a dyn StripHost>,
    session: Session,
    scene: SceneState,
    /// Profile ids of the monitors connected when we last looked.
    connected: Vec<String>,
    /// When set, the monitor set is re-examined at this time (debounce).
    display_deadline: Option<Instant>,
    /// Windows Krova moved itself a moment ago; their own move/size events are ignored.
    placed_at: HashMap<WindowKey, Instant>,
    /// Auto-arrange: stacks per monitor, windows waiting to settle, strips shown.
    arrange: arrange::ArrangeState,
}

impl<'a> Engine<'a> {
    fn new(
        shared: &'a Shared,
        overlay: Option<&'a dyn Overlay>,
        strips: Option<&'a dyn StripHost>,
    ) -> Self {
        let connected = lock(&shared.model).connected_ids();
        Self {
            shared,
            overlay,
            strips,
            arrange: arrange::ArrangeState::default(),
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
                    Ok(done) => match slot {
                        Some(slot) => {
                            self.remember(window, slot.clone(), done);
                            // A drop on an occupied zone swaps tracked windows.
                            self.after_manual_place(window, &slot, true);
                        }
                        None => self.after_free_place(window),
                    },
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
            // Windows that have been closed since are dropped now and then, and so
            // are entries nobody has refreshed for hours.
            registry.retain(|k, _| shared.platform.window_exists(WindowId(k.0)));
            registry.expire(Instant::now(), MAX_AGE);
            let owner = shared.platform.window_owner(window);
            registry.insert(key, slot.record(rect, owner));
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
        if action == KeyAction::OpenSwitcher {
            self.open_switcher();
            return;
        }
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
            Ok(done) => {
                self.remember(window, planned.slot.clone(), done);
                self.after_manual_place(window, &planned.slot, false);
            }
            Err(e) => eprintln!("krova: could not move the window: {e}"),
        }
    }

    /// `<mods>+Space`: remember the monitor under the cursor and ask the shell to
    /// show the layout switcher. Nothing happens while the feature is off, and
    /// with no monitor connected there is nothing to switch.
    fn open_switcher(&mut self) {
        let shared = self.shared;
        let wanted = {
            let model = lock(&shared.model);
            let s = &model.config.settings;
            s.enabled && s.layout_switcher_shortcut
        };
        if !wanted {
            return;
        }
        // A monitor plugged in a moment ago must count: refresh the list first.
        self.notice_monitor_changes();
        let cursor = shared.platform.cursor_position();
        let Some(target) = lock(&shared.model).monitor_for_switcher(cursor) else {
            return;
        };
        *lock(&shared.switcher_monitor) = Some(target);
        // Take the opener out of its lock first: opening can create the window.
        let open = lock(&shared.open_view).clone();
        if let Some(open) = open {
            open("switcher");
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
            WatchEvent::WindowShown(window) | WatchEvent::WindowRestored(window) => {
                self.on_window_shown(window);
            }
            WatchEvent::WindowGone(window) => self.on_window_left(window, true),
            WatchEvent::WindowMinimized(window) => self.on_window_left(window, false),
            WatchEvent::Foreground(window) => self.on_focus(window),
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
        self.arrange_monitors_changed();
        if !settings.enabled || !settings.restore_on_reconnect {
            return;
        }

        let registry = {
            let mut live = lock(&shared.registry);
            live.expire(Instant::now(), MAX_AGE);
            live.clone()
        };
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
        // Auto-arrange looks again in a moment: a snap may still be on its way.
        self.on_user_moved(window);
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
///
/// A handle that is not a movable window any more (an elevated program, a window
/// that lost its title bar, a tooltip the OS gave the number to) counts as gone: it
/// is forgotten and never moved. A minimized window is the exception, because
/// `is_movable_target` is false for it only on account of being minimized.
fn snapshot(
    platform: &dyn krova_platform::Platform,
    model: &AppModel,
    window: WindowId,
) -> WindowSnapshot {
    let gone = WindowSnapshot {
        exists: false,
        minimized: false,
        maximized: false,
        rect: None,
        monitor_profile_id: None,
        owner: None,
    };
    if !platform.window_exists(window) {
        return gone;
    }
    let minimized = platform.is_minimized(window);
    if !minimized && !platform.is_movable_target(window) {
        return gone;
    }
    let rect = platform.window_rect(window).ok();
    WindowSnapshot {
        exists: true,
        minimized,
        maximized: platform.is_maximized(window),
        rect,
        monitor_profile_id: rect.and_then(|r| model.profile_at(r)),
        owner: platform.window_owner(window),
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

#[cfg(test)]
mod switcher_tests {
    use super::*;
    use crate::model::AppModel;
    use krova_core::{Config, Modifier, MonitorDescriptor, ShortcutModifier};
    use krova_platform::{Capabilities, InputHandle, MonitorInfo, Permission, Platform};
    use std::path::PathBuf;
    use std::sync::Mutex;

    /// A desk with monitors and a cursor that tests move around, and shortcuts that
    /// are "registered" by recording what was asked for.
    struct Desk {
        monitors: Mutex<Vec<MonitorInfo>>,
        cursor: Mutex<Option<(i32, i32)>>,
        hotkey_requests: Mutex<Vec<HotkeySpec>>,
        refuse_space: bool,
    }

    impl Platform for Desk {
        fn monitors(&self) -> Result<Vec<MonitorInfo>, PlatformError> {
            Ok(lock(&self.monitors).clone())
        }
        fn place_window(
            &self,
            _: WindowId,
            _: PxRect,
            _: PxRect,
            _: bool,
        ) -> Result<PxRect, PlatformError> {
            Err(PlatformError::Unsupported)
        }
        fn start_input(
            &self,
            _: Modifier,
            _: Sender<InputEvent>,
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
            Capabilities {
                shortcuts: true,
                window_memory: false,
                auto_arrange: false,
            }
        }
        fn cursor_position(&self) -> Option<(i32, i32)> {
            *lock(&self.cursor)
        }
        fn start_hotkeys(
            &self,
            spec: HotkeySpec,
            _: EventSink<HotkeyEvent>,
        ) -> Result<HotkeyHandle, PlatformError> {
            lock(&self.hotkey_requests).push(spec);
            let mut accepted: Vec<KeyAction> = Vec::new();
            let mut refused = Vec::new();
            for def in spec.defs() {
                if self.refuse_space && def.action == KeyAction::OpenSwitcher {
                    refused.push((def.action, "Ctrl+Alt+Shift+Space is taken".to_owned()));
                } else {
                    accepted.push(def.action);
                }
            }
            Ok(HotkeyHandle::new(accepted, refused, || {}))
        }
    }

    fn monitor(hw: &str, x: i32, primary: bool) -> MonitorInfo {
        MonitorInfo {
            descriptor: MonitorDescriptor {
                hardware_id: Some(hw.to_owned()),
                device_path: None,
                rect: PxRect::new(x, 0, 1920, 1080),
            },
            work: PxRect::new(x, 0, 1920, 1040),
            scale: 1.0,
            label: format!("Display {hw}"),
            primary,
        }
    }

    fn two() -> Vec<MonitorInfo> {
        vec![monitor("A", 0, true), monitor("B", 1920, false)]
    }

    fn desk(
        monitors: Vec<MonitorInfo>,
        cursor: Option<(i32, i32)>,
        refuse_space: bool,
    ) -> Arc<Desk> {
        Arc::new(Desk {
            monitors: Mutex::new(monitors),
            cursor: Mutex::new(cursor),
            hotkey_requests: Mutex::new(Vec::new()),
            refuse_space,
        })
    }

    type Opened = Arc<Mutex<Vec<&'static str>>>;

    /// Shared state whose model already knows `known`, plus a recorder for the
    /// views the engine asks the shell to open.
    fn shared_on(desk: &Arc<Desk>, known: Vec<MonitorInfo>) -> (Shared, Opened) {
        let mut model = AppModel::new(Config::default(), PathBuf::from("unused.json"), false);
        model.refresh_monitors(known).unwrap();
        let shared = Shared::new(model, desk.clone());
        let opened: Opened = Arc::new(Mutex::new(Vec::new()));
        let record = Arc::clone(&opened);
        *lock(&shared.open_view) = Some(Arc::new(move |view| lock(&record).push(view)));
        (shared, opened)
    }

    fn press(shared: &Shared) {
        Engine::new(shared, None, None).on_hotkey(KeyAction::OpenSwitcher);
    }

    #[test]
    fn the_shortcut_opens_the_switcher_on_the_monitor_under_the_cursor() {
        let desk = desk(two(), Some((2500, 300)), false);
        let (shared, opened) = shared_on(&desk, two());
        press(&shared);
        assert_eq!(lock(&shared.switcher_monitor).as_deref(), Some("monitor-2"));
        assert_eq!(*lock(&opened), ["switcher"]);

        *lock(&desk.cursor) = Some((100, 100));
        press(&shared);
        assert_eq!(lock(&shared.switcher_monitor).as_deref(), Some("monitor-1"));
        assert_eq!(*lock(&opened), ["switcher", "switcher"]);
    }

    #[test]
    fn an_unreadable_cursor_means_the_primary_monitor() {
        let desk = desk(two(), None, false);
        let (shared, opened) = shared_on(&desk, two());
        press(&shared);
        assert_eq!(lock(&shared.switcher_monitor).as_deref(), Some("monitor-1"));
        assert_eq!(lock(&opened).len(), 1);
    }

    #[test]
    fn a_monitor_plugged_in_a_moment_ago_counts() {
        // The model only knows A; the desk now also has B with the cursor on it.
        let desk = desk(two(), Some((3000, 10)), false);
        let (shared, opened) = shared_on(&desk, vec![monitor("A", 0, true)]);
        press(&shared);
        assert_eq!(lock(&shared.switcher_monitor).as_deref(), Some("monitor-2"));
        assert_eq!(lock(&opened).len(), 1);
    }

    #[test]
    fn nothing_happens_while_the_feature_or_krova_is_off() {
        let desk = desk(two(), Some((100, 100)), false);
        let (shared, opened) = shared_on(&desk, two());
        lock(&shared.model).config.settings.layout_switcher_shortcut = false;
        press(&shared);
        lock(&shared.model).config.settings.layout_switcher_shortcut = true;
        lock(&shared.model).config.settings.enabled = false;
        press(&shared);
        assert!(lock(&opened).is_empty());
        assert_eq!(*lock(&shared.switcher_monitor), None);
    }

    #[test]
    fn the_switcher_does_not_depend_on_the_window_move_shortcuts() {
        let desk = desk(two(), Some((100, 100)), false);
        let (shared, opened) = shared_on(&desk, two());
        lock(&shared.model).config.settings.keyboard_shortcuts = false;
        press(&shared);
        assert_eq!(lock(&opened).len(), 1);
    }

    #[test]
    fn with_no_monitor_there_is_nothing_to_switch() {
        let desk = desk(vec![], Some((100, 100)), false);
        let (shared, opened) = shared_on(&desk, vec![]);
        press(&shared);
        assert!(lock(&opened).is_empty());
        assert_eq!(*lock(&shared.switcher_monitor), None);
    }

    #[test]
    fn without_a_shell_the_target_is_still_recorded() {
        let desk = desk(two(), Some((100, 100)), false);
        let (shared, _) = shared_on(&desk, two());
        *lock(&shared.open_view) = None;
        press(&shared);
        assert_eq!(lock(&shared.switcher_monitor).as_deref(), Some("monitor-1"));
    }

    #[test]
    fn the_settings_decide_which_shortcut_groups_are_requested() {
        let ask = |enabled, moves, switcher| {
            hotkey_request(&Settings {
                enabled,
                keyboard_shortcuts: moves,
                layout_switcher_shortcut: switcher,
                shortcut_modifier: ShortcutModifier::CtrlAlt,
                ..Settings::default()
            })
        };
        let spec = |window_moves, switcher| {
            Some(HotkeySpec {
                modifier: ShortcutModifier::CtrlAlt,
                window_moves,
                switcher,
            })
        };
        assert_eq!(ask(true, true, true), spec(true, true));
        assert_eq!(ask(true, true, false), spec(true, false));
        assert_eq!(ask(true, false, true), spec(false, true));
        assert_eq!(ask(true, false, false), None);
        // Krova disabled switches both off.
        assert_eq!(ask(false, true, true), None);
        assert_eq!(
            hotkey_request(&Settings::default()),
            Some(HotkeySpec {
                modifier: ShortcutModifier::CtrlAltShift,
                window_moves: true,
                switcher: true,
            }),
            "the defaults ask for both, with Ctrl+Alt+Shift"
        );
    }

    #[test]
    fn registration_results_are_reported_per_group() {
        let desk = desk(two(), None, true);
        let (shared, _) = shared_on(&desk, two());
        *lock(&shared.engine_tx) = Some(mpsc::channel::<Msg>().0);

        apply_shortcuts(&shared);
        {
            let f = lock(&shared.features);
            assert_eq!(
                f.shortcuts,
                ShortcutStatus {
                    active: true,
                    failed: vec![]
                }
            );
            assert_eq!(
                f.layout_switcher,
                ShortcutStatus {
                    active: false,
                    failed: vec!["Ctrl+Alt+Shift+Space is taken".to_owned()]
                },
                "the refused switcher chord does not disturb the move shortcuts"
            );
        }

        // Only the switcher wanted: the move group is not requested at all.
        {
            let mut model = lock(&shared.model);
            model.config.settings.keyboard_shortcuts = false;
            model.config.settings.layout_switcher_shortcut = true;
        }
        apply_shortcuts(&shared);
        let asked = lock(&desk.hotkey_requests).clone();
        assert_eq!(asked.len(), 2);
        assert!(asked[0].window_moves && asked[0].switcher);
        assert!(!asked[1].window_moves && asked[1].switcher);
        assert_eq!(lock(&shared.features).shortcuts, ShortcutStatus::default());

        // Both off: everything released, nothing requested again.
        lock(&shared.model).config.settings.layout_switcher_shortcut = false;
        apply_shortcuts(&shared);
        assert_eq!(lock(&desk.hotkey_requests).len(), 2);
        let f = lock(&shared.features);
        assert!(f.hotkeys.is_none());
        assert_eq!(f.layout_switcher, ShortcutStatus::default());
    }

    #[test]
    fn a_working_switcher_chord_reads_as_active_with_nothing_failed() {
        let desk = desk(two(), None, false);
        let (shared, _) = shared_on(&desk, two());
        *lock(&shared.engine_tx) = Some(mpsc::channel::<Msg>().0);
        apply_shortcuts(&shared);
        let f = lock(&shared.features);
        assert!(f.shortcuts.active && f.shortcuts.failed.is_empty());
        assert!(f.layout_switcher.active && f.layout_switcher.failed.is_empty());
    }

    #[test]
    fn a_shortcut_thread_that_cannot_start_is_reported_for_the_requested_groups() {
        let spec = HotkeySpec {
            modifier: ShortcutModifier::CtrlAltShift,
            window_moves: false,
            switcher: true,
        };
        let (moves, switcher) = unstarted_statuses(spec, "no thread");
        assert_eq!(moves, ShortcutStatus::default());
        assert!(!switcher.active);
        assert_eq!(
            switcher.failed,
            ["The layout switcher shortcut could not be started: no thread"]
        );
        let both = HotkeySpec {
            window_moves: true,
            ..spec
        };
        let (moves, _) = unstarted_statuses(both, "x");
        assert_eq!(moves.failed, ["Keyboard shortcuts could not be started: x"]);
    }
}
