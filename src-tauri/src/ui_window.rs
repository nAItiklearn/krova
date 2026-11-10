//! Lifecycle of the editor / settings window.
//!
//! The window is not created at start-up (`"create": false` in `tauri.conf.json`)
//! and closing it destroys it, so the WebView2 engine (about 190 MB in six
//! processes) only exists while the window is open. The tray, a second launch and
//! the commands recreate it on demand with the config entry of the `main` window.
//!
//! A "navigate" event emitted before the page has registered its listener would be
//! lost, so the requested view is parked in [`UiState`] and delivered once the page
//! has finished loading (after a short delay for the page's `listen()` call to
//! reach us). The UI can also pull the parked view with `take_pending_view`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow, WebviewWindowBuilder, Wry};

use crate::model::lock;

/// Label of the one UI window, as in `tauri.conf.json`.
pub const MAIN: &str = "main";

/// Closing the window destroys it, which makes WebView2 exit. Set to `false` to
/// go back to hiding it (the engine then stays alive) if destroy/recreate ever
/// proves unreliable on some machine; everything else keeps working unchanged.
pub const DESTROY_ON_CLOSE: bool = true;

/// How long after "page loaded" the parked view is delivered: the page registers
/// its `navigate` listener during mount, a couple of IPC round trips earlier.
const DELIVERY_DELAY: Duration = Duration::from_millis(250);

/// What the window should show next, and whether its page is ready for events.
#[derive(Default)]
pub struct UiState {
    // `&'static str`, not `String`: only the known views can ever be parked, and
    // a plain-data type keeps unit-test binaries from linking the whole webview
    // stack (a shared `Option<String>` drop-glue symbol from the `tauri` crate would).
    pending_view: Mutex<Option<&'static str>>,
    page_ready: AtomicBool,
    /// Serialises window creation (two quick tray clicks must build one window).
    creating: Mutex<()>,
}

/// The names a "navigate" can carry: the editor window's two views and `switcher`,
/// the layout switcher opened by its global shortcut. Anything else is not a view.
fn known_view(name: &str) -> Option<&'static str> {
    match name {
        "editor" => Some("editor"),
        "settings" => Some("settings"),
        "switcher" => Some("switcher"),
        _ => None,
    }
}

impl UiState {
    /// Remember which view to show next. Unknown names are ignored.
    pub fn park(&self, view: &str) {
        if let Some(view) = known_view(view) {
            *lock(&self.pending_view) = Some(view);
        }
    }

    pub fn take(&self) -> Option<&'static str> {
        lock(&self.pending_view).take()
    }

    pub fn is_ready(&self) -> bool {
        self.page_ready.load(Ordering::SeqCst)
    }

    pub fn set_ready(&self, ready: bool) {
        self.page_ready.store(ready, Ordering::SeqCst);
        if !ready {
            // A window that was destroyed must not hand a stale view to the next one.
            self.take();
        }
    }
}

/// The view a second launch asks for: `--settings` opens Settings, anything else
/// (including no argument at all) the editor. Arguments are untrusted input, so
/// they can only choose between these two fixed views.
pub fn view_from_args(args: &[String]) -> &'static str {
    // The first argument is the program itself.
    if args.iter().skip(1).any(|a| a == "--settings") {
        "settings"
    } else {
        "editor"
    }
}

/// Bring the window forward (creating it if needed), optionally switching its view.
pub fn show(app: &AppHandle, view: Option<&str>) {
    let ui = app.state::<UiState>();
    if let Some(view) = view {
        ui.park(view);
    }
    if let Some(window) = app.get_webview_window(MAIN) {
        reveal(&window);
        if ui.is_ready() {
            deliver_pending(app);
        }
        // Otherwise the page is still loading: its load handler delivers the view.
        return;
    }
    // WebView2 deadlocks when a window is built from an event handler (the tray
    // menu, the single-instance callback, a command), so build on its own thread.
    let app = app.clone();
    let spawned = thread::Builder::new()
        .name("krova-window".into())
        .spawn(move || create(&app));
    if let Err(e) = spawned {
        eprintln!("krova: could not start the window thread: {e}");
    }
}

fn create(app: &AppHandle) {
    let ui = app.state::<UiState>();
    let _building = lock(&ui.creating);
    if let Some(window) = app.get_webview_window(MAIN) {
        // Another request built it while this one waited.
        reveal(&window);
        if ui.is_ready() {
            deliver_pending(app);
        }
        return;
    }
    let Some(config) = app.config().app.windows.iter().find(|w| w.label == MAIN) else {
        eprintln!("krova: the main window is missing from tauri.conf.json");
        return;
    };
    let built = WebviewWindowBuilder::from_config(app, config).and_then(|builder| {
        builder
            .on_page_load(|window, payload| {
                if payload.event() == PageLoadEvent::Finished {
                    page_loaded(&window);
                }
            })
            .build()
    });
    if let Err(e) = built {
        eprintln!("krova: could not create the window: {e}");
    }
}

/// The page finished loading: show the (still hidden) window and, a moment later,
/// tell it which view was asked for.
fn page_loaded(window: &WebviewWindow<Wry>) {
    let app = window.app_handle().clone();
    app.state::<UiState>().set_ready(true);
    reveal(window);
    let spawned = thread::Builder::new()
        .name("krova-navigate".into())
        .spawn(move || {
            thread::sleep(DELIVERY_DELAY);
            deliver_pending(&app);
        });
    if let Err(e) = spawned {
        eprintln!("krova: could not start the navigate thread: {e}");
    }
}

fn reveal(window: &WebviewWindow<Wry>) {
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
}

fn deliver_pending(app: &AppHandle) {
    if let Some(view) = app.state::<UiState>().take() {
        let _ = app.emit_to(MAIN, "navigate", view);
    }
}

/// The window is going away: forget that its page was ready.
pub fn destroyed(app: &AppHandle) {
    app.state::<UiState>().set_ready(false);
}

/// The view a pending "navigate" asked for, if it has not been delivered yet.
/// Optional pull path for the UI: the push above already covers the common case.
#[tauri::command]
pub fn take_pending_view(ui: tauri::State<'_, UiState>) -> Option<&'static str> {
    ui.take()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_parked_view_is_delivered_once() {
        let ui = UiState::default();
        assert_eq!(ui.take(), None);
        ui.park("settings");
        assert_eq!(ui.take(), Some("settings"));
        assert_eq!(ui.take(), None);
    }

    #[test]
    fn the_latest_request_wins() {
        let ui = UiState::default();
        ui.park("settings");
        ui.park("editor");
        assert_eq!(ui.take(), Some("editor"));
    }

    #[test]
    fn only_the_known_views_can_be_parked() {
        let ui = UiState::default();
        ui.park("settings");
        ui.park("../../etc/passwd");
        ui.park("");
        ui.park("Settings");
        ui.park("Switcher");
        assert_eq!(ui.take(), Some("settings"), "unknown names change nothing");
        ui.park("<script>");
        assert_eq!(ui.take(), None);
    }

    #[test]
    fn the_switcher_can_be_parked_and_the_latest_request_wins() {
        let ui = UiState::default();
        ui.park("switcher");
        assert_eq!(ui.take(), Some("switcher"));
        ui.park("editor");
        ui.park("switcher");
        assert_eq!(ui.take(), Some("switcher"));
        ui.park("switcher");
        ui.park("settings");
        assert_eq!(ui.take(), Some("settings"));
    }

    #[test]
    fn a_second_launch_can_ask_for_the_settings_view_and_nothing_else() {
        let args = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(view_from_args(&args(&["krova-app.exe"])), "editor");
        assert_eq!(
            view_from_args(&args(&["krova-app.exe", "--settings"])),
            "settings"
        );
        assert_eq!(
            view_from_args(&args(&["krova-app.exe", "x", "--settings"])),
            "settings"
        );
        // The program name itself is not an option, and unknown words are ignored.
        assert_eq!(view_from_args(&args(&["--settings"])), "editor");
        assert_eq!(
            view_from_args(&args(&["krova-app.exe", "--evil", "settings"])),
            "editor"
        );
        assert_eq!(view_from_args(&[]), "editor");
    }

    #[test]
    fn a_destroyed_window_drops_its_pending_view_and_readiness() {
        let ui = UiState::default();
        ui.set_ready(true);
        ui.park("settings");
        assert!(ui.is_ready());
        ui.set_ready(false);
        assert!(!ui.is_ready());
        assert_eq!(ui.take(), None);
    }
}
