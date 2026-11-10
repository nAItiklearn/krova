//! End-to-end test of the reconnect memory against a real window.
//!
//! This machine has one monitor, so a second one is faked: a platform wrapper
//! reports two "monitors" (the left and right half of the real screen) and can drop
//! the second and bring it back. Everything else is real: the engine thread, the
//! registry, `place_window` on a real `notepad.exe`. The sequence is
//!
//! 1. snap Notepad into zone 2 of the fake second monitor (keyboard path),
//! 2. the fake monitor disappears and the user moves Notepad elsewhere (as Windows
//!    would when a monitor is unplugged),
//! 3. the monitor returns and a display-change event arrives: after the debounce
//!    the engine puts Notepad back into zone 2.
//!
//! `#[ignore]`d: it needs an interactive desktop and takes the keyboard focus.
//!
//! ```text
//! cargo test --manifest-path src-tauri/Cargo.toml -- --ignored --nocapture --test-threads=1 desktop_reconnect
//! ```

use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::{Duration, Instant};

use krova_core::registry::{KeyAction, WindowKey};
use krova_core::{Config, MonitorDescriptor, MoveMode, PxRect};
use krova_platform::overlay::Overlay;
use krova_platform::win::WindowsPlatform;
use krova_platform::{
    Capabilities, EventSink, HotkeyEvent, HotkeyHandle, HotkeySpec, InputEvent, InputHandle,
    MonitorInfo, Permission, Platform, PlatformError, WatchEvent, WatchHandle, WindowId,
};

use crate::desktop_tests::{focus, near, wait_for_rect, Notepad};
use crate::engine::{self, Msg};
use crate::model::{lock, AppModel, Shared};

/// The real Windows platform, except that the list of monitors is whatever the test says.
struct FakeMonitors {
    real: WindowsPlatform,
    monitors: Mutex<Vec<MonitorInfo>>,
}

impl Platform for FakeMonitors {
    fn monitors(&self) -> Result<Vec<MonitorInfo>, PlatformError> {
        Ok(lock(&self.monitors).clone())
    }
    fn place_window(
        &self,
        window: WindowId,
        target: PxRect,
        work: PxRect,
        animate: bool,
    ) -> Result<PxRect, PlatformError> {
        self.real.place_window(window, target, work, animate)
    }
    fn start_input(
        &self,
        modifier: krova_core::Modifier,
        sink: Sender<InputEvent>,
    ) -> Result<InputHandle, PlatformError> {
        self.real.start_input(modifier, sink)
    }
    fn autostart_enabled(&self) -> Result<bool, PlatformError> {
        self.real.autostart_enabled()
    }
    fn set_autostart(&self, enabled: bool) -> Result<(), PlatformError> {
        self.real.set_autostart(enabled)
    }
    fn create_overlay(&self) -> Result<Box<dyn Overlay>, PlatformError> {
        self.real.create_overlay()
    }
    fn permission(&self) -> Permission {
        self.real.permission()
    }
    fn capabilities(&self) -> Capabilities {
        // Window memory, but no real hotkeys or watcher: the test drives the engine itself.
        Capabilities {
            shortcuts: false,
            window_memory: false,
            auto_arrange: false,
        }
    }
    fn window_exists(&self, window: WindowId) -> bool {
        self.real.window_exists(window)
    }
    fn window_rect(&self, window: WindowId) -> Result<PxRect, PlatformError> {
        self.real.window_rect(window)
    }
    fn is_minimized(&self, window: WindowId) -> bool {
        self.real.is_minimized(window)
    }
    fn is_maximized(&self, window: WindowId) -> bool {
        self.real.is_maximized(window)
    }
    fn foreground_window(&self) -> Option<WindowId> {
        self.real.foreground_window()
    }
    fn is_movable_target(&self, window: WindowId) -> bool {
        self.real.is_movable_target(window)
    }
    fn start_hotkeys(
        &self,
        _spec: HotkeySpec,
        _sink: EventSink<HotkeyEvent>,
    ) -> Result<HotkeyHandle, PlatformError> {
        Err(PlatformError::Unsupported)
    }
    fn start_watcher(&self, _sink: EventSink<WatchEvent>) -> Result<WatchHandle, PlatformError> {
        Err(PlatformError::Unsupported)
    }
}

fn fake_monitor(hardware_id: &str, rect: PxRect) -> MonitorInfo {
    MonitorInfo {
        descriptor: MonitorDescriptor {
            hardware_id: Some(hardware_id.to_owned()),
            device_path: None,
            rect,
        },
        work: rect,
        scale: 1.0,
        label: hardware_id.to_owned(),
        primary: hardware_id == "A",
    }
}

fn send(shared: &Shared, msg: Msg) {
    let tx = lock(&shared.engine_tx).clone().expect("engine is running");
    tx.send(msg).expect("engine thread is alive");
}

#[test]
#[ignore = "spawns notepad and takes the keyboard focus"]
fn desktop_reconnect_puts_a_displaced_window_back_into_its_zone() {
    let real = WindowsPlatform::new();
    let screen = real
        .monitors()
        .expect("monitors")
        .into_iter()
        .find(|m| m.primary)
        .expect("a primary monitor")
        .work;
    // Monitor B gets 60% of the screen so that half of it is wider than Notepad's
    // minimum width (about 400 px); otherwise the window refuses the zone size.
    let half = screen.w * 2 / 5;
    let a = fake_monitor("A", PxRect::new(screen.x, screen.y, half, screen.h));
    let b = fake_monitor(
        "B",
        PxRect::new(screen.x + half, screen.y, screen.w - half, screen.h),
    );
    let platform = Arc::new(FakeMonitors {
        real,
        monitors: Mutex::new(vec![a.clone(), b.clone()]),
    });

    let mut model = AppModel::new(Config::default(), PathBuf::from("unused.json"), false);
    model.config.settings.move_mode = MoveMode::Instant;
    model.refresh_monitors(vec![a.clone(), b.clone()]).unwrap();
    let shared = Arc::new(Shared::new(model, platform.clone()));
    engine::start_headless(&shared);

    let Some(np) = Notepad::spawn() else {
        println!("SKIPPED: no Notepad window appeared");
        return;
    };
    // Start on the fake monitor B so the keyboard path resolves it as the monitor.
    platform
        .place_window(
            np.id,
            PxRect::new(b.work.x + 20, b.work.y + 100, 420, 400),
            b.work,
            false,
        )
        .expect("place");
    if !focus(np.id) {
        println!("SKIPPED: the session did not let this test take the focus");
        return;
    }

    // 1. Snap into zone 2 of B's "halves" layout.
    send(&shared, Msg::Hotkey(KeyAction::Zone(2)));
    let layout_zone2 = PxRect::new(
        b.work.x + b.work.w / 2,
        b.work.y,
        b.work.w - b.work.w / 2,
        b.work.h,
    );
    let got = wait_for_rect(&*platform, np.id, layout_zone2);
    println!("zone 2 of B: want {layout_zone2:?}, got {got:?}");
    assert!(
        got.is_some(),
        "the keyboard move reached zone 2 of monitor B"
    );
    let record = lock(&shared.registry)
        .get(WindowKey(np.id.0))
        .cloned()
        .unwrap();
    println!("registry: {record:?}");
    assert_eq!(record.zone_id, 2);

    // 2. Monitor B is unplugged; Windows would shove the window onto A.
    *lock(&platform.monitors) = vec![a.clone()];
    send(&shared, Msg::Watch(WatchEvent::DisplayChanged));
    sleep(engine::DISPLAY_DEBOUNCE + Duration::from_millis(1500));
    let displaced = PxRect::new(a.work.x + 40, a.work.y + 120, 400, 380);
    platform
        .place_window(np.id, displaced, a.work, false)
        .expect("displace");
    assert!(
        near(platform.window_rect(np.id).unwrap(), displaced),
        "the window now sits on monitor A"
    );
    assert!(
        lock(&shared.registry).contains(WindowKey(np.id.0)),
        "the registry keeps the entry while the monitor is away"
    );

    // 3. B returns; after the debounce the engine puts the window back.
    *lock(&platform.monitors) = vec![a.clone(), b.clone()];
    let returned_at = Instant::now();
    send(&shared, Msg::Watch(WatchEvent::DisplayChanged));
    let back = wait_for_rect(&*platform, np.id, layout_zone2);
    println!(
        "after the monitor returned: want {layout_zone2:?}, got {back:?}, took {:?}",
        returned_at.elapsed()
    );
    assert!(
        back.is_some(),
        "the window went back into zone 2 of monitor B"
    );
    assert!(
        returned_at.elapsed() >= engine::DISPLAY_DEBOUNCE,
        "not before the debounce time"
    );

    // 4. Idempotent: another display event changes nothing.
    send(&shared, Msg::Watch(WatchEvent::DisplayChanged));
    sleep(engine::DISPLAY_DEBOUNCE + Duration::from_millis(1000));
    assert!(near(platform.window_rect(np.id).unwrap(), layout_zone2));

    // 5. A user move forgets the window: simulate by dropping the entry as a drag does,
    //    then unplug and replug: nothing is restored.
    lock(&shared.registry).remove(WindowKey(np.id.0));
    *lock(&platform.monitors) = vec![a.clone()];
    send(&shared, Msg::Watch(WatchEvent::DisplayChanged));
    sleep(engine::DISPLAY_DEBOUNCE + Duration::from_millis(1000));
    platform
        .place_window(np.id, displaced, a.work, false)
        .unwrap();
    *lock(&platform.monitors) = vec![a, b];
    send(&shared, Msg::Watch(WatchEvent::DisplayChanged));
    sleep(engine::DISPLAY_DEBOUNCE + Duration::from_millis(1500));
    assert!(
        near(platform.window_rect(np.id).unwrap(), displaced),
        "a window the user placed elsewhere is left alone"
    );
}
