//! The engine's auto-arrange flow against a fake desk: windows appear, are
//! classified, placed, stacked, swapped and re-laid out.

use super::*;
use crate::model::{AppModel, Shared};
use krova_core::app_rules::{AppRule, RuleAction};
use krova_core::{Config, Modifier, MonitorDescriptor};
use krova_platform::overlay::Overlay;
use krova_platform::{Capabilities, InputEvent, InputHandle, MonitorInfo, Permission, Platform};
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

type DeskWindow = (PxRect, WindowKind, &'static str);

#[derive(Default)]
struct Desk {
    windows: Mutex<HashMap<u64, DeskWindow>>,
    placed: Mutex<Vec<(u64, PxRect)>>,
    raised: Mutex<Vec<u64>>,
    foreground: Mutex<Option<u64>>,
}

impl Platform for Desk {
    fn monitors(&self) -> Result<Vec<MonitorInfo>, PlatformError> {
        Ok(vec![monitor()])
    }
    fn place_window(
        &self,
        w: WindowId,
        target: PxRect,
        _: PxRect,
        _: bool,
    ) -> Result<PxRect, PlatformError> {
        let mut windows = lock(&self.windows);
        let Some(entry) = windows.get_mut(&w.0) else {
            return Err(PlatformError::WindowGone);
        };
        entry.0 = target;
        lock(&self.placed).push((w.0, target));
        Ok(target)
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
            shortcuts: false,
            window_memory: true,
            auto_arrange: true,
        }
    }
    fn window_exists(&self, w: WindowId) -> bool {
        lock(&self.windows).contains_key(&w.0)
    }
    fn window_rect(&self, w: WindowId) -> Result<PxRect, PlatformError> {
        lock(&self.windows)
            .get(&w.0)
            .map(|e| e.0)
            .ok_or(PlatformError::WindowGone)
    }
    fn foreground_window(&self) -> Option<WindowId> {
        lock(&self.foreground).map(WindowId)
    }
    fn probe_window(&self, w: WindowId) -> Option<WindowProbe> {
        lock(&self.windows).get(&w.0).map(|e| WindowProbe {
            kind: e.1,
            exe: Some(e.2.to_owned()),
            exe_path: None,
            owner: None,
        })
    }
    fn raise_window(&self, w: WindowId) -> Result<(), PlatformError> {
        lock(&self.raised).push(w.0);
        Ok(())
    }
}

fn monitor() -> MonitorInfo {
    MonitorInfo {
        descriptor: MonitorDescriptor {
            hardware_id: Some("A".into()),
            device_path: None,
            rect: PxRect::new(0, 0, 1920, 1080),
        },
        work: PxRect::new(0, 0, 1920, 1040),
        scale: 1.0,
        label: "Display A".into(),
        primary: true,
    }
}

fn setup() -> (Arc<Desk>, Shared) {
    let desk = Arc::new(Desk::default());
    let mut model = AppModel::new(Config::default(), PathBuf::from("unused.json"), false);
    model.refresh_monitors(vec![monitor()]).unwrap();
    let shared = Shared::new(model, desk.clone());
    (desk, shared)
}

fn open(desk: &Desk, id: u64, kind: WindowKind, exe: &'static str) {
    lock(&desk.windows).insert(id, (PxRect::new(300, 300, 500, 400), kind, exe));
}

const LEFT: PxRect = PxRect::new(0, 0, 960, 1040);
const RIGHT: PxRect = PxRect::new(960, 0, 960, 1040);

fn rect_of(desk: &Desk, id: u64) -> PxRect {
    lock(&desk.windows)[&id].0
}

#[test]
fn new_windows_fill_the_zones_then_stack() {
    let (desk, shared) = setup();
    let mut e = Engine::new(&shared, None, None);
    for id in 1..=3 {
        open(&desk, id, WindowKind::Normal, "app.exe");
        e.on_window_shown(WindowId(id));
    }
    assert_eq!(e.arrange.pending.len(), 3, "they wait to settle first");
    assert!(e.arrange.deadline().is_some());
    assert!(lock(&desk.placed).is_empty());
    for id in 1..=3 {
        e.arrange_new(WindowId(id));
    }
    assert_eq!(rect_of(&desk, 1), LEFT);
    assert_eq!(rect_of(&desk, 2), RIGHT);
    // The third joins a stack (every zone is taken) and is raised there.
    let stacks = &e.arrange.stacks["monitor-1"];
    assert_eq!(stacks.len(), 3);
    let zone = stacks.zone_of(3).unwrap();
    assert_eq!(stacks.depth(zone), 2);
    assert_eq!(stacks.visible(zone), Some(3));
    assert!(lock(&desk.raised).contains(&3));
    // The registry remembers them all, so a reconnect puts them back.
    assert_eq!(lock(&shared.registry).len(), 3);
}

#[test]
fn dialogs_rules_and_switched_off_monitors_are_left_alone() {
    let (desk, shared) = setup();
    let mut e = Engine::new(&shared, None, None);
    open(&desk, 1, WindowKind::Dialog, "app.exe");
    e.arrange_new(WindowId(1));
    open(&desk, 2, WindowKind::Normal, "calc.exe");
    lock(&shared.model)
        .config
        .settings
        .app_rules
        .set(AppRule::new("calc.exe", RuleAction::Float))
        .unwrap();
    e.arrange_new(WindowId(2));
    assert!(lock(&desk.placed).is_empty());

    // A preferred zone is honoured.
    open(&desk, 3, WindowKind::Normal, "spotify.exe");
    lock(&shared.model)
        .config
        .settings
        .app_rules
        .set(AppRule::new("spotify.exe", RuleAction::PreferredZone(2)))
        .unwrap();
    e.arrange_new(WindowId(3));
    assert_eq!(rect_of(&desk, 3), RIGHT);

    // Paused, or auto-arrange off for the monitor: nothing moves.
    open(&desk, 4, WindowKind::Normal, "app.exe");
    shared.set_paused(true);
    e.arrange_new(WindowId(4));
    shared.set_paused(false);
    lock(&shared.model).config.monitors[0].auto_arrange.enabled = false;
    e.arrange_new(WindowId(4));
    assert_eq!(lock(&desk.placed).len(), 1);
}

#[test]
fn closing_a_window_untracks_it_and_the_next_tab_shows() {
    let (desk, shared) = setup();
    let mut e = Engine::new(&shared, None, None);
    for id in 1..=3 {
        open(&desk, id, WindowKind::Normal, "app.exe");
        e.arrange_new(WindowId(id));
    }
    let zone = e.arrange.stacks["monitor-1"].zone_of(3).unwrap();
    lock(&desk.windows).remove(&3);
    e.on_window_left(WindowId(3), true);
    let stacks = &e.arrange.stacks["monitor-1"];
    assert!(!stacks.contains(3));
    assert_eq!(stacks.depth(zone), 1);
    assert!(stacks.visible(zone).is_some());
}

#[test]
fn dropping_a_tracked_window_on_an_occupied_zone_swaps_the_two() {
    let (desk, shared) = setup();
    let mut e = Engine::new(&shared, None, None);
    for id in 1..=2 {
        open(&desk, id, WindowKind::Normal, "app.exe");
        e.arrange_new(WindowId(id));
    }
    // The user drags window 1 onto the right zone; the snap already moved it.
    lock(&desk.windows).get_mut(&1).unwrap().0 = RIGHT;
    let slot = Slot {
        profile_id: "monitor-1".into(),
        layout_id: "halves".into(),
        zone_id: 2,
    };
    e.after_manual_place(WindowId(1), &slot, true);
    assert_eq!(
        rect_of(&desk, 2),
        LEFT,
        "the other window took the free seat"
    );
    let stacks = &e.arrange.stacks["monitor-1"];
    assert_eq!(stacks.zone_of(1), Some(2));
    assert_eq!(stacks.zone_of(2), Some(1));
}

#[test]
fn the_keyboard_swap_trades_places_with_the_neighbour() {
    let (desk, shared) = setup();
    let mut e = Engine::new(&shared, None, None);
    for id in 1..=2 {
        open(&desk, id, WindowKind::Normal, "app.exe");
        e.arrange_new(WindowId(id));
    }
    *lock(&desk.foreground) = Some(1);
    assert!(e.swap_focused(Direction::Right));
    assert_eq!(rect_of(&desk, 1), RIGHT);
    assert_eq!(rect_of(&desk, 2), LEFT);
    assert!(!e.swap_focused(Direction::Up), "no neighbour above");
}

#[test]
fn a_new_active_layout_re_tiles_the_tracked_windows() {
    let (desk, shared) = setup();
    let mut e = Engine::new(&shared, None, None);
    for id in 1..=2 {
        open(&desk, id, WindowKind::Normal, "app.exe");
        e.arrange_new(WindowId(id));
    }
    lock(&shared.model)
        .config
        .set_active_layout("monitor-1", "thirds")
        .unwrap();
    e.on_config_changed();
    assert_eq!(rect_of(&desk, 1), PxRect::new(0, 0, 640, 1040));
    assert_eq!(rect_of(&desk, 2), PxRect::new(640, 0, 640, 1040));

    // Turning auto-arrange off forgets the stacks; nothing moves.
    let moves = lock(&desk.placed).len();
    lock(&shared.model).config.monitors[0].auto_arrange.enabled = false;
    e.on_config_changed();
    assert!(!e.arrange.stacks.contains_key("monitor-1"));
    assert_eq!(lock(&desk.placed).len(), moves);
}

#[test]
fn a_window_moved_away_by_hand_floats() {
    let (desk, shared) = setup();
    let mut e = Engine::new(&shared, None, None);
    open(&desk, 1, WindowKind::Normal, "app.exe");
    e.arrange_new(WindowId(1));
    // Still in its zone: kept.
    e.check_moved(WindowId(1));
    assert!(e.arrange.tracked(1));
    // Dragged into the middle of the screen without a snap: no longer tracked.
    lock(&desk.windows).get_mut(&1).unwrap().0 = PxRect::new(500, 300, 600, 400);
    e.check_moved(WindowId(1));
    assert!(!e.arrange.tracked(1));
}

#[test]
fn the_front_app_says_why_it_is_not_arranged() {
    let (desk, shared) = setup();
    let mut e = Engine::new(&shared, None, None);
    open(&desk, 1, WindowKind::Dialog, "app.exe");
    e.on_focus(WindowId(1));
    let front = lock(&shared.front_app).clone().unwrap();
    assert!(!front.arranged);
    assert_eq!(front.reason.as_deref(), Some("dialog"));
    assert_eq!(front.exe.as_deref(), Some("app.exe"));
    open(&desk, 2, WindowKind::Normal, "app.exe");
    e.arrange_new(WindowId(2));
    e.on_focus(WindowId(2));
    let front = lock(&shared.front_app).clone().unwrap();
    assert!(front.arranged && front.reason.is_none());
}

#[test]
fn a_shown_window_already_tracked_or_pending_is_not_queued_twice() {
    let (desk, shared) = setup();
    let mut e = Engine::new(&shared, None, None);
    open(&desk, 1, WindowKind::Normal, "app.exe");
    e.on_window_shown(WindowId(1));
    e.on_window_shown(WindowId(1));
    assert_eq!(e.arrange.pending.len(), 1);
    e.arrange.pending.clear();
    e.arrange_new(WindowId(1));
    e.on_window_shown(WindowId(1));
    assert!(e.arrange.pending.is_empty());
}
