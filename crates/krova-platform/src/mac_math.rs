//! OS-independent logic of the macOS layer.
//!
//! Nothing in here touches a macOS API, so Windows and Linux CI run these tests
//! too. The macOS-only modules (`mac_ffi`, `mac_ax`, ...) stay thin wrappers
//! around the system calls and delegate every decision to this file.
//!
//! Conventions:
//! - Krova's "px" on macOS are points, not device pixels.
//! - AppKit (`NSScreen`, `NSWindow`) has a bottom-left origin; CGWindowList,
//!   CGEvent and the Accessibility API, like Krova itself, a top-left one.
//!   [`flip_rect`] / [`unflip_rect`] convert against the primary screen height.

use krova_core::{DragEvent, Modifier, PxRect, SizeLimits};

use crate::input::{ChangeDetector, RawPointer};
use crate::{InputEvent, WindowId};

// ------------------------------------------------------------- constants

/// `NSStatusWindowLevel`: the overlay floats above normal windows and the Dock.
pub const OVERLAY_WINDOW_LEVEL: isize = 25;
/// `NSWindowCollectionBehaviorCanJoinAllSpaces` (1 << 0).
pub const COLLECTION_CAN_JOIN_ALL_SPACES: usize = 1;
/// `NSWindowCollectionBehaviorIgnoresCycle` (1 << 6).
pub const COLLECTION_IGNORES_CYCLE: usize = 1 << 6;
/// `NSWindowCollectionBehaviorFullScreenAuxiliary` (1 << 8).
pub const COLLECTION_FULL_SCREEN_AUXILIARY: usize = 1 << 8;
/// Collection behaviour of the overlay window.
pub const OVERLAY_COLLECTION_BEHAVIOR: usize =
    COLLECTION_CAN_JOIN_ALL_SPACES | COLLECTION_IGNORES_CYCLE | COLLECTION_FULL_SCREEN_AUXILIARY;

/// `kCGImageAlphaPremultipliedFirst` (2) | `kCGBitmapByteOrder32Little` (2 << 12):
/// a little-endian `0xAARRGGBB` word with premultiplied alpha, which is exactly
/// what `overlay::render` produces.
pub const BITMAP_INFO_BGRA_PREMULTIPLIED: u32 = 2 | (2 << 12);

/// `kVK_Escape`.
pub const KEY_ESCAPE: u16 = 0x35;

/// `CGEventFlags` masks (`kCGEventFlagMask*`).
pub const FLAG_SHIFT: u64 = 1 << 17;
pub const FLAG_CONTROL: u64 = 1 << 18;
pub const FLAG_ALTERNATE: u64 = 1 << 19;
pub const FLAG_COMMAND: u64 = 1 << 20;

/// Window size may differ from the request by this many points (the window
/// server snaps sizes to the backing grid) without counting as "limited".
pub const SIZE_TOLERANCE: i32 = 1;

/// A frame read through Accessibility matches a CGWindowList frame within this.
pub const FRAME_MATCH_TOLERANCE: i32 = 2;

/// Label of the LaunchAgent fallback; also the app's bundle identifier.
pub const LAUNCH_AGENT_LABEL: &str = "dev.krova.app";

// ------------------------------------------------------------- geometry

/// Round a floating point rect to whole points. Each edge is rounded on its own
/// so that neighbouring rects keep tiling without gaps.
pub fn round_rect(x: f64, y: f64, w: f64, h: f64) -> PxRect {
    let left = x.round() as i32;
    let top = y.round() as i32;
    let right = (x + w).round() as i32;
    let bottom = (y + h).round() as i32;
    PxRect::new(left, top, right - left, bottom - top)
}

/// AppKit global rect (bottom-left origin) to Krova's top-left virtual screen.
/// `primary_height` is the height of the primary screen (`NSScreen.screens[0]`),
/// whose bottom-left corner is the AppKit origin.
pub fn flip_rect(x: f64, y: f64, w: f64, h: f64, primary_height: f64) -> PxRect {
    round_rect(x, primary_height - (y + h), w, h)
}

/// Inverse of [`flip_rect`]: Krova rect to AppKit `(x, y, w, h)`.
pub fn unflip_rect(r: PxRect, primary_height: f64) -> (f64, f64, f64, f64) {
    (
        f64::from(r.x),
        primary_height - f64::from(r.y) - f64::from(r.h),
        f64::from(r.w),
        f64::from(r.h),
    )
}

/// Pixel cell containing a cursor position given in fractional points.
pub fn cursor_px(x: f64, y: f64) -> (i32, i32) {
    (x.floor() as i32, y.floor() as i32)
}

// ------------------------------------------------------------- monitors

const UNKNOWN_DISPLAY_FIELD: u32 = 0xFFFF_FFFF;

fn known(v: u32) -> u32 {
    if v == UNKNOWN_DISPLAY_FIELD {
        0
    } else {
        v
    }
}

/// `VVVV-MMMM-SSSSSSSS` from `CGDisplayVendorNumber` / `ModelNumber` /
/// `SerialNumber`. `None` when the OS reports nothing at all (every field is 0
/// or "unknown"). A missing serial alone still yields an id, with a zero serial.
pub fn display_hardware_id(vendor: u32, model: u32, serial: u32) -> Option<String> {
    let (v, m, s) = (known(vendor), known(model), known(serial));
    if v == 0 && m == 0 && s == 0 {
        return None;
    }
    Some(format!("{v:04X}-{m:04X}-{s:08X}"))
}

// -------------------------------------------------------------- windows

/// One row of `CGWindowListCopyWindowInfo`. No title is ever read.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowEntry {
    pub id: u32,
    pub pid: i32,
    pub layer: i32,
    pub alpha: f64,
    pub bounds: PxRect,
}

/// Index of the window a click at `point` landed on. `windows` is the window
/// server's list, front to back. Only layer 0 (normal application windows) and
/// visible ones count; menu bar, Dock and floating layers are skipped, so a click
/// on them falls through to the window below. That is harmless: such a press
/// never moves the window, and a drag needs the window origin to move.
pub fn window_hit(point: (i32, i32), windows: &[WindowEntry]) -> Option<usize> {
    windows
        .iter()
        .position(|w| w.layer == 0 && w.alpha > 0.0 && w.bounds.contains(point.0, point.1))
}

/// A window of an application as seen through Accessibility.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxCandidate {
    /// `CGWindowID` from `_AXUIElementGetWindow`, when that call worked.
    pub window_id: Option<u32>,
    pub frame: Option<PxRect>,
}

fn frame_distance(a: PxRect, b: PxRect) -> i32 {
    (a.x - b.x)
        .abs()
        .max((a.y - b.y).abs())
        .max((a.w - b.w).abs())
        .max((a.h - b.h).abs())
}

/// Which accessibility window is the CGWindow `wanted_id` with frame `wanted`?
/// An exact window-number match always wins. Otherwise only windows whose number
/// is unknown are considered, and the one whose frame is closest (within
/// [`FRAME_MATCH_TOLERANCE`]) is taken; ties go to the first (front-most).
pub fn pick_ax_window(wanted_id: u32, wanted: PxRect, candidates: &[AxCandidate]) -> Option<usize> {
    if let Some(i) = candidates
        .iter()
        .position(|c| c.window_id == Some(wanted_id))
    {
        return Some(i);
    }
    candidates
        .iter()
        .enumerate()
        .filter(|(_, c)| c.window_id.is_none())
        .filter_map(|(i, c)| c.frame.map(|f| (i, frame_distance(f, wanted))))
        .filter(|(_, d)| *d <= FRAME_MATCH_TOLERANCE)
        .min_by_key(|(_, d)| *d)
        .map(|(i, _)| i)
}

/// Size limits a window revealed by not reaching the size we asked for, or
/// `None` when it got there (within `tolerance` points). Mirrors the inference
/// in `win.rs`.
pub fn infer_limits(want: PxRect, got: PxRect, tolerance: i32) -> Option<SizeLimits> {
    let dw = got.w - want.w;
    let dh = got.h - want.h;
    if dw.abs() <= tolerance && dh.abs() <= tolerance {
        return None;
    }
    Some(SizeLimits {
        min_w: (dw > tolerance).then_some(got.w),
        max_w: (dw < -tolerance).then_some(got.w),
        min_h: (dh > tolerance).then_some(got.h),
        max_h: (dh < -tolerance).then_some(got.h),
    })
}

// ------------------------------------------------------- accessibility

pub const AX_ERROR_INVALID_UI_ELEMENT: i32 = -25202;
pub const AX_ERROR_API_DISABLED: i32 = -25211;
pub const AX_ERROR_ATTRIBUTE_UNSUPPORTED: i32 = -25205;
pub const AX_ERROR_ACTION_UNSUPPORTED: i32 = -25206;
pub const AX_ERROR_NOT_IMPLEMENTED: i32 = -25208;
pub const AX_ERROR_NO_VALUE: i32 = -25212;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxFailure {
    /// The element or its app no longer exists.
    WindowGone,
    /// Accessibility is switched off for this process.
    PermissionDenied,
    /// The window or app does not allow this attribute to be read / set.
    Unsupported,
    Other(i32),
}

/// Sort an `AXError` code into what the platform layer reports to the app.
pub fn classify_ax_error(code: i32) -> AxFailure {
    match code {
        AX_ERROR_INVALID_UI_ELEMENT => AxFailure::WindowGone,
        AX_ERROR_API_DISABLED => AxFailure::PermissionDenied,
        AX_ERROR_ATTRIBUTE_UNSUPPORTED
        | AX_ERROR_ACTION_UNSUPPORTED
        | AX_ERROR_NOT_IMPLEMENTED
        | AX_ERROR_NO_VALUE => AxFailure::Unsupported,
        other => AxFailure::Other(other),
    }
}

// ---------------------------------------------------------------- keys

/// `CGEventFlags` bit of the activation modifier. Super is Command, Alt is Option.
pub fn modifier_mask(m: Modifier) -> u64 {
    match m {
        Modifier::Shift => FLAG_SHIFT,
        Modifier::Ctrl => FLAG_CONTROL,
        Modifier::Alt => FLAG_ALTERNATE,
        Modifier::Super => FLAG_COMMAND,
    }
}

pub fn modifier_held(flags: u64, m: Modifier) -> bool {
    flags & modifier_mask(m) != 0
}

// ------------------------------------------------------------ autostart

/// Escape the five XML special characters.
pub fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// LaunchAgent property list that starts `exe` at login.
pub fn launch_agent_plist(label: &str, exe: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
         \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n\
         <dict>\n\
         \t<key>Label</key>\n\
         \t<string>{label}</string>\n\
         \t<key>ProgramArguments</key>\n\
         \t<array>\n\
         \t\t<string>{exe}</string>\n\
         \t</array>\n\
         \t<key>RunAtLoad</key>\n\
         \t<true/>\n\
         \t<key>ProcessType</key>\n\
         \t<string>Interactive</string>\n\
         </dict>\n\
         </plist>\n",
        label = xml_escape(label),
        exe = xml_escape(exe),
    )
}

/// The `.app` bundle an executable path lives in
/// (`/Applications/Krova.app/Contents/MacOS/krova` gives `/Applications/Krova.app`),
/// or `None` for a bare binary such as `target/debug/krova-app`.
pub fn app_bundle_path(exe: &str) -> Option<&str> {
    const MARK: &str = ".app/Contents/MacOS/";
    let i = exe.find(MARK)?;
    Some(&exe[..i + ".app".len()])
}

// ------------------------------------------------------- drag tracking

/// What the drag tracker needs from the OS. The real implementation lives in
/// `mac_input`; tests use a fake.
pub trait DragProbe {
    /// The normal on-screen window under a screen point: its id and frame.
    fn window_at(&mut self, x: i32, y: i32) -> Option<(u32, PxRect)>;
    /// Current frame of a window, `None` once it is gone.
    fn window_frame(&mut self, id: u32) -> Option<PxRect>;
    /// Is the configured activation modifier held right now?
    fn modifier_down(&mut self) -> bool;
    fn escape_down(&mut self) -> bool;
    /// Give the OS time to finish its own window move before we act on it.
    fn settle(&mut self);
}

#[derive(Debug, Clone, Copy)]
struct Press {
    id: u32,
    moved: bool,
}

fn input_event(event: DragEvent, id: u32) -> InputEvent {
    InputEvent {
        event,
        window: Some(WindowId(u64::from(id))),
    }
}

/// Turns raw left-button events into [`InputEvent`]s with the same rules as the
/// Windows worker: a press on a normal window starts a gesture; every move
/// reports the window's current frame (the snap session decides whether it is a
/// drag by "origin moved, size unchanged"); a release after a real move waits for
/// the OS to settle; modifier and Escape are polled while a drag is live.
#[derive(Debug, Default)]
pub struct DragTracker {
    press: Option<Press>,
    modifier_watch: ChangeDetector,
    escape_watch: ChangeDetector,
}

impl DragTracker {
    /// Is a left-button gesture on a window in progress?
    pub fn is_pressed(&self) -> bool {
        self.press.is_some()
    }

    pub fn on_pointer<P: DragProbe>(
        &mut self,
        probe: &mut P,
        ev: RawPointer,
        out: &mut Vec<InputEvent>,
    ) {
        match ev {
            RawPointer::Down(x, y) => {
                self.press = None;
                let Some((id, rect)) = probe.window_at(x, y) else {
                    return;
                };
                self.modifier_watch.reset(probe.modifier_down());
                self.escape_watch.reset(probe.escape_down());
                out.push(input_event(
                    DragEvent::Press {
                        cursor: (x, y),
                        window: rect,
                    },
                    id,
                ));
                self.press = Some(Press { id, moved: false });
            }
            RawPointer::Move(x, y) => {
                let Some(p) = self.press.as_mut() else {
                    return;
                };
                let id = p.id;
                let Some(rect) = probe.window_frame(id) else {
                    self.press = None;
                    return;
                };
                p.moved = true;
                out.push(input_event(
                    DragEvent::Move {
                        cursor: (x, y),
                        window: rect,
                        modifier_down: probe.modifier_down(),
                    },
                    id,
                ));
            }
            RawPointer::Up => {
                if let Some(p) = self.press.take() {
                    if p.moved {
                        probe.settle();
                    }
                    out.push(input_event(DragEvent::Release, p.id));
                }
            }
        }
    }

    /// Keys do not produce mouse events, so poll them while a drag is live.
    pub fn poll_keys<P: DragProbe>(&mut self, probe: &mut P, out: &mut Vec<InputEvent>) {
        let id = match self.press {
            Some(p) if p.moved => p.id,
            _ => return,
        };
        if let Some(down) = self.modifier_watch.update(probe.modifier_down()) {
            out.push(input_event(DragEvent::Modifier { down }, id));
        }
        if self.escape_watch.update(probe.escape_down()) == Some(true) {
            out.push(input_event(DragEvent::Cancel, id));
            self.press = None;
        }
    }

    /// The OS stopped delivering events (event tap disabled): abandon the gesture
    /// so no overlay is left on screen.
    pub fn abort(&mut self, out: &mut Vec<InputEvent>) {
        if let Some(p) = self.press.take() {
            out.push(input_event(DragEvent::Cancel, p.id));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRIMARY_H: f64 = 1080.0;

    #[test]
    fn primary_screen_flips_onto_itself() {
        let r = flip_rect(0.0, 0.0, 1920.0, 1080.0, PRIMARY_H);
        assert_eq!(r, PxRect::new(0, 0, 1920, 1080));
    }

    #[test]
    fn a_screen_above_the_primary_gets_negative_y() {
        // AppKit: its bottom edge sits on the primary's top edge (y = 1080).
        let r = flip_rect(0.0, 1080.0, 2560.0, 1440.0, PRIMARY_H);
        assert_eq!(r, PxRect::new(0, -1440, 2560, 1440));
    }

    #[test]
    fn a_screen_to_the_right_and_lower_flips_correctly() {
        // AppKit y = -200 is below the primary's bottom edge by 200 points.
        let r = flip_rect(1920.0, -200.0, 1280.0, 1024.0, PRIMARY_H);
        assert_eq!(r, PxRect::new(1920, 256, 1280, 1024));
    }

    #[test]
    fn visible_frame_excludes_menu_bar_and_dock() {
        // 25 pt menu bar on top, 70 pt Dock at the bottom.
        let work = flip_rect(0.0, 70.0, 1920.0, 985.0, PRIMARY_H);
        assert_eq!(work, PxRect::new(0, 25, 1920, 985));
        assert_eq!(work.bottom(), 1010);
    }

    #[test]
    fn unflip_is_the_inverse() {
        let top_left = PxRect::new(0, 25, 1920, 985);
        assert_eq!(unflip_rect(top_left, PRIMARY_H), (0.0, 70.0, 1920.0, 985.0));
        let (x, y, w, h) = unflip_rect(PxRect::new(1920, 256, 1280, 1024), PRIMARY_H);
        assert_eq!(
            flip_rect(x, y, w, h, PRIMARY_H),
            PxRect::new(1920, 256, 1280, 1024)
        );
    }

    #[test]
    fn fractional_edges_round_individually() {
        // Edges 0.4 and 100.8 round to 0 and 101, not to 0 and 100.
        let r = round_rect(0.4, 0.4, 100.4, 100.4);
        assert_eq!(r, PxRect::new(0, 0, 101, 101));
    }

    #[test]
    fn cursor_cells_use_floor() {
        assert_eq!(cursor_px(100.9, 50.0), (100, 50));
        assert_eq!(cursor_px(-0.5, -1.2), (-1, -2));
    }

    #[test]
    fn hardware_id_format() {
        assert_eq!(
            display_hardware_id(0x0610, 0xA050, 0x1234).as_deref(),
            Some("0610-A050-00001234")
        );
        assert_eq!(
            display_hardware_id(0x10AC, 0x4141, 0).as_deref(),
            Some("10AC-4141-00000000")
        );
    }

    #[test]
    fn hardware_id_is_none_when_nothing_is_known() {
        assert_eq!(display_hardware_id(0, 0, 0), None);
        assert_eq!(display_hardware_id(0xFFFF_FFFF, 0xFFFF_FFFF, 0), None);
        assert_eq!(
            display_hardware_id(0xFFFF_FFFF, 0xFFFF_FFFF, 0xFFFF_FFFF),
            None
        );
    }

    fn win(id: u32, layer: i32, alpha: f64, b: PxRect) -> WindowEntry {
        WindowEntry {
            id,
            pid: 1,
            layer,
            alpha,
            bounds: b,
        }
    }

    #[test]
    fn hit_test_takes_the_front_most_normal_window() {
        let list = [
            win(1, 0, 1.0, PxRect::new(0, 0, 500, 500)),
            win(2, 0, 1.0, PxRect::new(100, 100, 800, 600)),
        ];
        assert_eq!(window_hit((200, 200), &list), Some(0));
        assert_eq!(window_hit((700, 500), &list), Some(1));
        assert_eq!(window_hit((2000, 2000), &list), None);
        assert_eq!(window_hit((200, 200), &[]), None);
    }

    #[test]
    fn hit_test_skips_system_layers_and_invisible_windows() {
        let list = [
            win(10, 24, 1.0, PxRect::new(0, 0, 1920, 25)), // menu bar
            win(11, 0, 0.0, PxRect::new(0, 0, 1920, 1080)), // fully transparent
            win(12, 0, 1.0, PxRect::new(0, 0, 1000, 800)),
        ];
        assert_eq!(window_hit((10, 10), &list), Some(2));
    }

    #[test]
    fn hit_test_is_half_open() {
        let list = [win(1, 0, 1.0, PxRect::new(0, 0, 100, 100))];
        assert_eq!(window_hit((99, 99), &list), Some(0));
        assert_eq!(window_hit((100, 50), &list), None);
    }

    fn cand(id: Option<u32>, f: Option<PxRect>) -> AxCandidate {
        AxCandidate {
            window_id: id,
            frame: f,
        }
    }

    const FRAME: PxRect = PxRect::new(100, 100, 800, 600);

    #[test]
    fn exact_window_number_wins_even_with_a_different_frame() {
        let c = [
            cand(Some(5), Some(FRAME)),
            cand(Some(9), Some(PxRect::new(0, 0, 10, 10))),
        ];
        assert_eq!(pick_ax_window(9, FRAME, &c), Some(1));
    }

    #[test]
    fn frames_decide_only_when_window_numbers_are_unavailable() {
        let c = [
            cand(None, Some(PxRect::new(100, 100, 802, 600))),
            cand(None, Some(PxRect::new(101, 100, 800, 600))),
            cand(None, None),
        ];
        // Distances: 2 and 1, so the second one is closest.
        assert_eq!(pick_ax_window(3, FRAME, &c), Some(1));
    }

    #[test]
    fn windows_with_another_known_number_are_never_picked_by_frame() {
        let c = [cand(Some(5), Some(FRAME))];
        assert_eq!(pick_ax_window(9, FRAME, &c), None);
    }

    #[test]
    fn distant_frames_do_not_match() {
        let c = [cand(None, Some(PxRect::new(100, 100, 810, 600)))];
        assert_eq!(pick_ax_window(3, FRAME, &c), None);
        assert_eq!(pick_ax_window(3, FRAME, &[]), None);
    }

    #[test]
    fn equal_frame_ties_go_to_the_first_candidate() {
        let c = [cand(None, Some(FRAME)), cand(None, Some(FRAME))];
        assert_eq!(pick_ax_window(3, FRAME, &c), Some(0));
    }

    #[test]
    fn limits_are_inferred_per_axis() {
        let want = PxRect::new(0, 0, 960, 1040);
        assert_eq!(infer_limits(want, want, SIZE_TOLERANCE), None);
        assert_eq!(
            infer_limits(want, PxRect::new(0, 0, 961, 1039), SIZE_TOLERANCE),
            None
        );
        let min = infer_limits(want, PxRect::new(0, 0, 1200, 1040), SIZE_TOLERANCE);
        assert_eq!(
            min,
            Some(SizeLimits {
                min_w: Some(1200),
                ..SizeLimits::default()
            })
        );
        let max = infer_limits(want, PxRect::new(0, 0, 500, 400), SIZE_TOLERANCE);
        assert_eq!(
            max,
            Some(SizeLimits {
                max_w: Some(500),
                max_h: Some(400),
                ..SizeLimits::default()
            })
        );
    }

    #[test]
    fn accessibility_errors_are_classified() {
        assert_eq!(classify_ax_error(-25202), AxFailure::WindowGone);
        assert_eq!(classify_ax_error(-25211), AxFailure::PermissionDenied);
        assert_eq!(classify_ax_error(-25205), AxFailure::Unsupported);
        assert_eq!(classify_ax_error(-25212), AxFailure::Unsupported);
        assert_eq!(classify_ax_error(-25204), AxFailure::Other(-25204));
    }

    #[test]
    fn modifier_flags() {
        let flags = FLAG_SHIFT | FLAG_COMMAND;
        assert!(modifier_held(flags, Modifier::Shift));
        assert!(modifier_held(flags, Modifier::Super));
        assert!(!modifier_held(flags, Modifier::Ctrl));
        assert!(!modifier_held(flags, Modifier::Alt));
        assert!(!modifier_held(0, Modifier::Shift));
    }

    #[test]
    fn system_constants_have_the_documented_values() {
        assert_eq!(FLAG_SHIFT, 0x0002_0000);
        assert_eq!(FLAG_CONTROL, 0x0004_0000);
        assert_eq!(FLAG_ALTERNATE, 0x0008_0000);
        assert_eq!(FLAG_COMMAND, 0x0010_0000);
        assert_eq!(BITMAP_INFO_BGRA_PREMULTIPLIED, 0x2002);
        assert_eq!(OVERLAY_COLLECTION_BEHAVIOR, 1 | 64 | 256);
        assert_eq!(OVERLAY_WINDOW_LEVEL, 25);
        assert_eq!(KEY_ESCAPE, 53);
    }

    #[test]
    fn plist_escapes_the_executable_path() {
        let xml = launch_agent_plist("dev.krova.app", "/Apps/R&D <x>/krova");
        assert!(xml.contains("<string>dev.krova.app</string>"));
        assert!(xml.contains("<string>/Apps/R&amp;D &lt;x&gt;/krova</string>"));
        assert!(xml.contains("<key>RunAtLoad</key>"));
        assert!(xml.starts_with("<?xml"));
        assert!(xml.trim_end().ends_with("</plist>"));
    }

    #[test]
    fn xml_escaping() {
        assert_eq!(
            xml_escape(r#"a&b<c>"d"'e'"#),
            "a&amp;b&lt;c&gt;&quot;d&quot;&apos;e&apos;"
        );
        assert_eq!(xml_escape("plain"), "plain");
    }

    #[test]
    fn bundle_path_detection() {
        assert_eq!(
            app_bundle_path("/Applications/Krova.app/Contents/MacOS/krova-app"),
            Some("/Applications/Krova.app")
        );
        assert_eq!(
            app_bundle_path("/Users/me/krova/target/debug/krova-app"),
            None
        );
        assert_eq!(app_bundle_path(""), None);
    }

    // ---------------------------------------------------------- tracker

    struct Fake {
        hit: Option<(u32, PxRect)>,
        frame: Option<PxRect>,
        modifier: bool,
        escape: bool,
        settled: u32,
    }

    impl Fake {
        fn over(id: u32, r: PxRect) -> Self {
            Self {
                hit: Some((id, r)),
                frame: Some(r),
                modifier: false,
                escape: false,
                settled: 0,
            }
        }
    }

    impl DragProbe for Fake {
        fn window_at(&mut self, _x: i32, _y: i32) -> Option<(u32, PxRect)> {
            self.hit
        }
        fn window_frame(&mut self, _id: u32) -> Option<PxRect> {
            self.frame
        }
        fn modifier_down(&mut self) -> bool {
            self.modifier
        }
        fn escape_down(&mut self) -> bool {
            self.escape
        }
        fn settle(&mut self) {
            self.settled += 1;
        }
    }

    fn ev(event: DragEvent, id: u32) -> InputEvent {
        input_event(event, id)
    }

    #[test]
    fn press_move_release_follows_the_windows_rules() {
        let mut probe = Fake::over(7, FRAME);
        let mut t = DragTracker::default();
        let mut out = Vec::new();

        t.on_pointer(&mut probe, RawPointer::Down(500, 110), &mut out);
        assert_eq!(
            std::mem::take(&mut out),
            vec![ev(
                DragEvent::Press {
                    cursor: (500, 110),
                    window: FRAME
                },
                7
            )]
        );

        let moved = PxRect::new(140, 130, 800, 600);
        probe.frame = Some(moved);
        probe.modifier = true;
        t.on_pointer(&mut probe, RawPointer::Move(540, 130), &mut out);
        assert_eq!(
            std::mem::take(&mut out),
            vec![ev(
                DragEvent::Move {
                    cursor: (540, 130),
                    window: moved,
                    modifier_down: true
                },
                7
            )]
        );

        t.on_pointer(&mut probe, RawPointer::Up, &mut out);
        assert_eq!(out, vec![ev(DragEvent::Release, 7)]);
        assert_eq!(probe.settled, 1, "a real drag waits for the OS to settle");
        assert!(!t.is_pressed());
    }

    #[test]
    fn a_click_without_motion_releases_without_waiting() {
        let mut probe = Fake::over(7, FRAME);
        let mut t = DragTracker::default();
        let mut out = Vec::new();
        t.on_pointer(&mut probe, RawPointer::Down(500, 110), &mut out);
        out.clear();
        t.on_pointer(&mut probe, RawPointer::Up, &mut out);
        assert_eq!(out, vec![ev(DragEvent::Release, 7)]);
        assert_eq!(probe.settled, 0);
    }

    #[test]
    fn presses_outside_any_window_and_stray_moves_do_nothing() {
        let mut probe = Fake::over(7, FRAME);
        probe.hit = None;
        let mut t = DragTracker::default();
        let mut out = Vec::new();
        t.on_pointer(&mut probe, RawPointer::Down(5, 5), &mut out);
        t.on_pointer(&mut probe, RawPointer::Move(9, 9), &mut out);
        t.on_pointer(&mut probe, RawPointer::Up, &mut out);
        assert!(out.is_empty());
        assert!(!t.is_pressed());
    }

    #[test]
    fn a_window_that_disappears_ends_the_gesture_silently() {
        let mut probe = Fake::over(7, FRAME);
        let mut t = DragTracker::default();
        let mut out = Vec::new();
        t.on_pointer(&mut probe, RawPointer::Down(500, 110), &mut out);
        out.clear();
        probe.frame = None;
        t.on_pointer(&mut probe, RawPointer::Move(520, 120), &mut out);
        t.on_pointer(&mut probe, RawPointer::Up, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn a_new_press_supersedes_a_lost_release() {
        let mut probe = Fake::over(7, FRAME);
        let mut t = DragTracker::default();
        let mut out = Vec::new();
        t.on_pointer(&mut probe, RawPointer::Down(500, 110), &mut out);
        out.clear();
        probe.hit = Some((8, PxRect::new(0, 0, 300, 300)));
        t.on_pointer(&mut probe, RawPointer::Down(10, 10), &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].window, Some(WindowId(8)));
    }

    #[test]
    fn keys_are_only_polled_while_a_drag_is_live() {
        let mut probe = Fake::over(7, FRAME);
        let mut t = DragTracker::default();
        let mut out = Vec::new();
        t.on_pointer(&mut probe, RawPointer::Down(500, 110), &mut out);
        out.clear();

        probe.modifier = true;
        t.poll_keys(&mut probe, &mut out);
        assert!(out.is_empty(), "not moved yet");

        t.on_pointer(&mut probe, RawPointer::Move(520, 120), &mut out);
        out.clear();
        t.poll_keys(&mut probe, &mut out);
        assert_eq!(out, vec![ev(DragEvent::Modifier { down: true }, 7)]);
        out.clear();
        t.poll_keys(&mut probe, &mut out);
        assert!(out.is_empty(), "no change, no event");

        probe.modifier = false;
        t.poll_keys(&mut probe, &mut out);
        assert_eq!(out, vec![ev(DragEvent::Modifier { down: false }, 7)]);
    }

    #[test]
    fn escape_cancels_once_and_ends_the_gesture() {
        let mut probe = Fake::over(7, FRAME);
        let mut t = DragTracker::default();
        let mut out = Vec::new();
        t.on_pointer(&mut probe, RawPointer::Down(500, 110), &mut out);
        t.on_pointer(&mut probe, RawPointer::Move(520, 120), &mut out);
        out.clear();

        probe.escape = true;
        t.poll_keys(&mut probe, &mut out);
        assert_eq!(out, vec![ev(DragEvent::Cancel, 7)]);
        assert!(!t.is_pressed());
        out.clear();
        t.on_pointer(&mut probe, RawPointer::Up, &mut out);
        assert!(out.is_empty(), "the cancelled gesture has no release");
    }

    #[test]
    fn a_held_key_at_press_time_is_not_a_change() {
        let mut probe = Fake::over(7, FRAME);
        probe.modifier = true;
        probe.escape = true;
        let mut t = DragTracker::default();
        let mut out = Vec::new();
        t.on_pointer(&mut probe, RawPointer::Down(500, 110), &mut out);
        t.on_pointer(&mut probe, RawPointer::Move(520, 120), &mut out);
        out.clear();
        t.poll_keys(&mut probe, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn abort_cancels_only_an_active_gesture() {
        let mut probe = Fake::over(7, FRAME);
        let mut t = DragTracker::default();
        let mut out = Vec::new();
        t.abort(&mut out);
        assert!(out.is_empty());
        t.on_pointer(&mut probe, RawPointer::Down(500, 110), &mut out);
        out.clear();
        t.abort(&mut out);
        assert_eq!(out, vec![ev(DragEvent::Cancel, 7)]);
        assert!(!t.is_pressed());
    }
}
