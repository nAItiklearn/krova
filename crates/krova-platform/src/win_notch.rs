//! Windows backend of the notch: a layered, topmost, never-activated tool window
//! on its own thread.
//!
//! Idle cost is zero: the thread sleeps in `GetMessageW` and only wakes for mouse
//! input over the notch, a model update, a display change or a foreground-window
//! change (to hide behind fullscreen apps). A 16 ms timer runs only while the
//! spring is moving and is killed as soon as it rests.
//!
//! Everything is drawn by `krova_core::notch::render_pill` (anti-aliased, per-pixel
//! alpha, handed to `UpdateLayeredWindow`); the label's glyphs come from GDI with
//! grayscale anti-aliasing (`ANTIALIASED_QUALITY`, no ClearType: a layered window
//! has no background to filter sub-pixels against). Window titles are never read.

use std::cell::RefCell;
use std::ffi::c_void;
use std::mem::size_of;
use std::ptr::null_mut;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use krova_core::notch::{
    self as core, nearest_corner, notch_rect, pick_monitor, render_pill, to_px, PillFrame, Pointer,
    PointerInput, PointerOutput, RectSpring, COLLAPSED, EXPANDED, PAUSED_DOT,
};
use krova_core::PxRect;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{
    COLORREF, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, CreateFontW, DeleteDC, DeleteObject, DrawTextW, GetDC,
    GetTextFaceW, MonitorFromPoint, ReleaseDC, SelectObject, SetBkMode, SetTextColor, BITMAPINFO,
    BITMAPINFOHEADER, BLENDFUNCTION, DIB_RGB_COLORS, DT_END_ELLIPSIS, DT_LEFT, DT_NOPREFIX,
    DT_SINGLELINE, DT_VCENTER, HDC, HFONT, MONITOR_DEFAULTTONEAREST, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TrackMouseEvent, TME_HOVER, TME_LEAVE, TRACKMOUSEEVENT,
};
use windows::Win32::UI::Shell::{
    SHQueryUserNotificationState, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    DispatchMessageW, GetClassNameW, GetCursorPos, GetForegroundWindow, GetMessageW,
    GetWindowLongW, GetWindowRect, GetWindowThreadProcessId, IsZoomed, KillTimer, PeekMessageW,
    PostMessageW, PostThreadMessageW, RegisterClassExW, SetForegroundWindow, SetTimer,
    SetWindowPos, ShowWindow, TrackPopupMenu, TranslateMessage, UpdateLayeredWindow,
    EVENT_OBJECT_LOCATIONCHANGE, EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_MINIMIZEEND,
    EVENT_SYSTEM_MOVESIZEEND, GWL_STYLE, HMENU, HWND_TOPMOST, MA_NOACTIVATE, MF_CHECKED, MF_POPUP,
    MF_SEPARATOR, MF_STRING, MSG, PM_NOREMOVE, PM_REMOVE, SPI_SETWORKAREA, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SW_HIDE, SW_SHOWNOACTIVATE, TPM_BOTTOMALIGN, TPM_LEFTALIGN,
    TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, TPM_TOPALIGN, ULW_ALPHA, WINEVENT_OUTOFCONTEXT,
    WINEVENT_SKIPOWNPROCESS, WM_APP, WM_CAPTURECHANGED, WM_DISPLAYCHANGE, WM_DPICHANGED,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_NULL, WM_QUIT, WM_RBUTTONUP,
    WM_SETTINGCHANGE, WM_TIMER, WNDCLASSEXW, WS_CAPTION, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use crate::notch::{label, Notch, NotchAnchor, NotchEvent, NotchMenu, NotchModel};
use crate::{EventSink, PlatformError};

/// Posted to the notch thread when the inbox changed.
const WM_NOTCH_UPDATE: u32 = WM_APP + 21;
const TIMER_ID: usize = 1;
/// From `TrackMouseEvent` (winuser.h); not exported by this `windows` version.
const WM_MOUSEHOVER: u32 = 0x02A1;
const WM_MOUSELEAVE: u32 = 0x02A3;
const FRAME_MS: u32 = 16;

/// Menu command ids. Layouts take `ID_LAYOUT..ID_LAYOUT + n`.
const ID_PAUSE: usize = 1;
const ID_SWITCHER: usize = 2;
const ID_CHEATS: usize = 3;
const ID_OPEN: usize = 4;
const ID_HIDE: usize = 5;
const ID_LAYOUT: usize = 100;
/// Never list more than this many layouts in the menu.
const MAX_MENU_LAYOUTS: usize = 64;

#[derive(Default)]
struct Inbox {
    model: Option<NotchModel>,
    share_safe: Option<bool>,
}

type SharedInbox = Arc<Mutex<Inbox>>;

pub struct WinNotch {
    inbox: SharedInbox,
    thread_id: u32,
    join: Mutex<Option<JoinHandle<()>>>,
}

impl WinNotch {
    pub fn start(sink: EventSink<NotchEvent>) -> Result<Self, PlatformError> {
        let inbox: SharedInbox = Arc::default();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<u32, String>>();
        let shared = Arc::clone(&inbox);
        let join = thread::Builder::new()
            .name("krova-notch".into())
            .spawn(move || run(shared, sink, ready_tx))
            .map_err(|e| PlatformError::Os(e.to_string()))?;
        match ready_rx.recv_timeout(Duration::from_secs(2)) {
            Ok(Ok(thread_id)) => Ok(Self {
                inbox,
                thread_id,
                join: Mutex::new(Some(join)),
            }),
            Ok(Err(msg)) => {
                let _ = join.join();
                Err(PlatformError::Os(format!(
                    "could not create the notch: {msg}"
                )))
            }
            Err(_) => Err(PlatformError::Os("the notch thread did not start".into())),
        }
    }

    fn poke(&self) {
        let _ =
            unsafe { PostThreadMessageW(self.thread_id, WM_NOTCH_UPDATE, WPARAM(0), LPARAM(0)) };
    }
}

impl Notch for WinNotch {
    fn update(&self, model: NotchModel) {
        if let Ok(mut inbox) = self.inbox.lock() {
            inbox.model = Some(model);
        }
        self.poke();
    }

    fn set_share_safe(&self, on: bool) {
        if let Ok(mut inbox) = self.inbox.lock() {
            inbox.share_safe = Some(on);
        }
        self.poke();
    }
}

impl Drop for WinNotch {
    fn drop(&mut self) {
        let _ = unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
        if let Some(join) = self.join.lock().ok().and_then(|mut j| j.take()) {
            let _ = join.join();
        }
    }
}

// ---------------------------------------------------------------------------
// Thread state
// ---------------------------------------------------------------------------

/// The rendered label: grayscale coverage at the expanded size.
struct TextMask {
    text: String,
    w: i32,
    h: i32,
    coverage: Vec<u8>,
}

struct State {
    hwnd: HWND,
    sink: EventSink<NotchEvent>,
    model: NotchModel,
    share_safe: bool,
    pointer: Pointer,
    spring: RectSpring,
    /// Index into `model.monitors` of the monitor the notch is on.
    monitor: Option<usize>,
    /// Device px per logical px on that monitor.
    scale: f64,
    shown: bool,
    tracking: bool,
    animating: bool,
    last_tick: Instant,
    /// The window under a fullscreen check, and the location hook on its process.
    location_hook: Option<(HWINEVENTHOOK, u32)>,
    fullscreen: bool,
    text: Option<TextMask>,
    /// The last frame pushed to the window, to skip identical repaints.
    painted: Option<(PxRect, u64)>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// Run `f` on the thread state unless it is already borrowed (a re-entrant
/// message sent while we are inside a handler goes to `DefWindowProc` instead).
fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|cell| match cell.try_borrow_mut() {
        Ok(mut slot) => slot.as_mut().map(f),
        Err(_) => None,
    })
}

fn cursor() -> Option<(i32, i32)> {
    let mut p = POINT::default();
    unsafe { GetCursorPos(&mut p) }.ok().map(|()| (p.x, p.y))
}

fn monitor_scale(rect: PxRect, fallback: f64) -> f64 {
    let centre = POINT {
        x: rect.x + rect.w / 2,
        y: rect.y + rect.h / 2,
    };
    let (mut dx, mut dy) = (0u32, 0u32);
    let hmon = unsafe { MonitorFromPoint(centre, MONITOR_DEFAULTTONEAREST) };
    if unsafe { GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI, &mut dx, &mut dy) }.is_ok() && dx > 0 {
        f64::from(dx) / 96.0
    } else if fallback.is_finite() && fallback > 0.0 {
        fallback
    } else {
        1.0
    }
}

fn window_rect(hwnd: HWND) -> Option<PxRect> {
    let mut r = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut r) }.ok()?;
    Some(PxRect::new(
        r.left,
        r.top,
        r.right - r.left,
        r.bottom - r.top,
    ))
}

/// The desktop and the taskbar cover the monitor without being "fullscreen".
fn is_shell_window(hwnd: HWND) -> bool {
    let mut buf = [0u16; 64];
    let n = unsafe { GetClassNameW(hwnd, &mut buf) };
    let class = String::from_utf16_lossy(&buf[..usize::try_from(n).unwrap_or(0)]);
    matches!(
        class.as_str(),
        "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
    )
}

/// Is an app fullscreen in front of `monitor`? The shell's own answer
/// (`SHQueryUserNotificationState`) for D3D games and presentations, otherwise
/// a foreground window covering the whole monitor without being an ordinary
/// maximized window.
fn fullscreen_on(monitor: PxRect, own: HWND) -> bool {
    let fg = unsafe { GetForegroundWindow() };
    if fg.0.is_null() || fg == own || is_shell_window(fg) {
        return false;
    }
    let Some(r) = window_rect(fg) else {
        return false;
    };
    let on_this = monitor.contains(r.x + r.w / 2, r.y + r.h / 2);
    if !on_this {
        return false;
    }
    if let Ok(state) = unsafe { SHQueryUserNotificationState() } {
        if state == QUNS_RUNNING_D3D_FULL_SCREEN || state == QUNS_PRESENTATION_MODE {
            return true;
        }
    }
    let style = unsafe { GetWindowLongW(fg, GWL_STYLE) } as u32;
    let captioned = style & WS_CAPTION.0 == WS_CAPTION.0;
    let maximized = unsafe { IsZoomed(fg) }.as_bool();
    core::covers_monitor(r, monitor) && !(maximized && captioned)
}

impl State {
    fn current(&self) -> Option<&crate::notch::NotchMonitorInfo> {
        self.monitor.and_then(|i| self.model.monitors.get(i))
    }

    fn size(&self, expanded: bool) -> (i32, i32) {
        let (w, h) = if expanded { EXPANDED } else { COLLAPSED };
        (to_px(w, self.scale), to_px(h, self.scale))
    }

    /// The rect the notch rests at for the current state.
    fn target(&self, expanded: bool) -> Option<PxRect> {
        let m = self.current()?;
        let (w, h) = self.size(expanded);
        Some(notch_rect(
            self.model.settings.corner,
            m.rect,
            m.work,
            w,
            h,
            to_px(core::INSET, self.scale),
        ))
    }

    /// Re-pick the monitor, re-read its DPI, decide visibility, retarget the spring.
    fn relayout(&mut self, animate: bool) {
        let rects: Vec<(PxRect, bool)> = self
            .model
            .monitors
            .iter()
            .map(|m| (m.rect, m.primary))
            .collect();
        let picked = pick_monitor(self.model.settings.monitor, &rects, cursor());
        let moved_monitor = picked != self.monitor;
        self.monitor = picked;
        let Some(m) = self.current() else {
            self.hide();
            return;
        };
        let (rect, fallback) = (m.rect, m.scale);
        let scale = monitor_scale(rect, fallback);
        let rescaled = (scale - self.scale).abs() > 1e-6;
        self.scale = scale;
        self.pointer.threshold = to_px(core::DRAG_THRESHOLD, scale);
        self.fullscreen = self.model.settings.hide_in_fullscreen && fullscreen_on(rect, self.hwnd);
        if !self.model.settings.enabled || self.fullscreen {
            self.hide();
            return;
        }
        if self.pointer.dragging() {
            // The drag owns the position; only the size follows.
            let (w, h) = self.size(false);
            self.spring.w.target = f64::from(w);
            self.spring.h.target = f64::from(h);
        } else if let Some(target) = self.target(self.pointer.expanded()) {
            if !self.shown || moved_monitor || rescaled || !animate {
                self.spring.jump(target);
            } else {
                self.spring.set_target(target);
            }
        }
        if rescaled {
            self.text = None;
        }
        self.show();
        self.kick();
    }

    fn hide(&mut self) {
        if self.shown {
            let _ = unsafe { ShowWindow(self.hwnd, SW_HIDE) };
            self.shown = false;
            self.painted = None;
        }
        self.stop_timer();
    }

    fn show(&mut self) {
        if !self.shown {
            self.paint();
            let _ = unsafe { ShowWindow(self.hwnd, SW_SHOWNOACTIVATE) };
            let _ = unsafe {
                SetWindowPos(
                    self.hwnd,
                    HWND_TOPMOST,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                )
            };
            self.shown = true;
        }
    }

    /// Paint now, and keep a frame timer running while the spring moves.
    fn kick(&mut self) {
        self.paint();
        if self.spring.at_rest() {
            self.stop_timer();
        } else if !self.animating {
            self.last_tick = Instant::now();
            self.animating = unsafe { SetTimer(self.hwnd, TIMER_ID, FRAME_MS, None) } != 0;
        }
    }

    fn stop_timer(&mut self) {
        if self.animating {
            let _ = unsafe { KillTimer(self.hwnd, TIMER_ID) };
            self.animating = false;
        }
    }

    fn tick(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f64().min(0.05);
        self.last_tick = now;
        self.spring.step(dt);
        self.kick();
    }

    fn label(&self) -> String {
        self.current()
            .map(|m| label(m, self.model.paused, self.share_safe))
            .unwrap_or_default()
    }

    /// The label's coverage at the expanded size, cached until text or DPI change.
    fn text_mask(&mut self) -> Option<&TextMask> {
        let text = self.label();
        let (w, h) = self.size(true);
        let fresh = matches!(&self.text, Some(t) if t.text == text && t.w == w && t.h == h);
        let stale = !fresh;
        if stale {
            let left = to_px(22.0, self.scale);
            let right = to_px(12.0, self.scale);
            let font_px = to_px(12.0, self.scale);
            self.text = render_text(&text, w, h, left, right, font_px).map(|coverage| TextMask {
                text,
                w,
                h,
                coverage,
            });
        }
        self.text.as_ref()
    }

    fn paint(&mut self) {
        let rect = self.spring.rect();
        let (cw, _) = self.size(false);
        let (ew, _) = self.size(true);
        let e = core::expansion(f64::from(rect.w), f64::from(cw), f64::from(ew));
        let scale = self.scale;
        let full = f64::from(rect.h) / 2.0;
        let radius = full + ((self.model.radius * scale).min(full) - full) * e;
        let dot_r = (2.5 + 0.5 * e) * scale;
        let dot_x = full + (12.0 * scale - full) * e;
        let dot = if self.model.paused {
            PAUSED_DOT
        } else {
            self.model.accent
        };
        let text_alpha = ((e - 0.5) / 0.5).clamp(0.0, 1.0);
        // Cut the cached expanded-size mask down to this frame: the label keeps
        // its distance from the left edge and stays vertically centred.
        let mut cut = None;
        if text_alpha > 0.0 {
            if let Some(mask) = self.text_mask() {
                let mut out = vec![0u8; usize::try_from(rect.w * rect.h).unwrap_or(0)];
                let oy = (mask.h - rect.h) / 2;
                for y in 0..rect.h {
                    let sy = y + oy;
                    if sy < 0 || sy >= mask.h {
                        continue;
                    }
                    for x in 0..rect.w.min(mask.w) {
                        let s = usize::try_from(sy * mask.w + x).unwrap_or(0);
                        let d = usize::try_from(y * rect.w + x).unwrap_or(0);
                        out[d] = mask.coverage[s];
                    }
                }
                cut = Some(out);
            }
        }
        let frame = PillFrame {
            w: rect.w,
            h: rect.h,
            radius,
            dot,
            dot_at: (dot_x, f64::from(rect.h) / 2.0, dot_r),
            text: cut.as_deref(),
            text_alpha,
        };
        // Skip a repaint that would push the very same pixels.
        let key = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (rect.w, rect.h, radius.to_bits(), dot, dot_x.to_bits()).hash(&mut h);
            text_alpha.to_bits().hash(&mut h);
            self.text.as_ref().map(|t| &t.text).hash(&mut h);
            h.finish()
        };
        if self.painted == Some((rect, key)) {
            return;
        }
        let pixels = render_pill(&frame);
        push(self.hwnd, rect, &pixels);
        self.painted = Some((rect, key));
    }

    fn anchor(&self) -> Option<NotchAnchor> {
        let m = self.current()?;
        Some(NotchAnchor {
            rect: self.target(true)?,
            corner: self.model.settings.corner,
            monitor_id: m.id.clone(),
            work: m.work,
            scale: self.scale,
        })
    }

    fn on_pointer(&mut self, input: PointerInput) {
        match self.pointer.on(input) {
            PointerOutput::Nothing => {}
            PointerOutput::Expand | PointerOutput::Collapse => self.relayout(true),
            PointerOutput::Click => {
                if let Some(a) = self.anchor() {
                    (self.sink)(NotchEvent::Clicked(a));
                }
            }
            PointerOutput::Drag { .. } => {
                // The collapsed pill rides centred under the cursor.
                if let Some((x, y)) = cursor() {
                    let (w, h) = self.size(false);
                    self.spring.x.jump(f64::from(x - w / 2));
                    self.spring.y.jump(f64::from(y - h / 2));
                    self.spring.w.target = f64::from(w);
                    self.spring.h.target = f64::from(h);
                }
                self.kick();
            }
            PointerOutput::Drop { x, y } => {
                if let Some(m) = self.current() {
                    let (w, h) = self.size(false);
                    let inset = to_px(core::INSET, self.scale);
                    let corner = nearest_corner(m.rect, m.work, w, h, inset, (x, y));
                    if corner != self.model.settings.corner {
                        self.model.settings.corner = corner;
                        (self.sink)(NotchEvent::CornerChanged(corner));
                    }
                }
                self.tracking = false;
                self.relayout(true);
            }
        }
    }

    /// Re-hook location changes for the process of the new foreground window, so
    /// an app that goes fullscreen without a focus change (F11) is noticed too.
    fn follow_foreground(&mut self) {
        let fg = unsafe { GetForegroundWindow() };
        let mut pid = 0u32;
        if !fg.0.is_null() {
            unsafe { GetWindowThreadProcessId(fg, Some(&mut pid)) };
        }
        if self.location_hook.map(|(_, p)| p) == Some(pid) {
            return;
        }
        if let Some((hook, _)) = self.location_hook.take() {
            let _ = unsafe { UnhookWinEvent(hook) };
        }
        if pid != 0 && self.model.settings.hide_in_fullscreen {
            let hook = unsafe {
                SetWinEventHook(
                    EVENT_OBJECT_LOCATIONCHANGE,
                    EVENT_OBJECT_LOCATIONCHANGE,
                    None,
                    Some(win_event_proc),
                    pid,
                    0,
                    WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
                )
            };
            if !hook.0.is_null() {
                self.location_hook = Some((hook, pid));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// GDI helpers
// ---------------------------------------------------------------------------

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn create_font(dc: HDC, px: i32) -> Option<HFONT> {
    for face in ["Segoe UI Variable Text", "Segoe UI"] {
        let name = wide(face);
        let font = unsafe {
            CreateFontW(
                -px,
                0,
                0,
                0,
                600, // FW_SEMIBOLD
                0,
                0,
                0,
                1, // DEFAULT_CHARSET
                0, // OUT_DEFAULT_PRECIS
                0, // CLIP_DEFAULT_PRECIS
                4, // ANTIALIASED_QUALITY: grayscale, no ClearType
                0, // DEFAULT_PITCH | FF_DONTCARE
                PCWSTR(name.as_ptr()),
            )
        };
        if font.0.is_null() {
            continue;
        }
        let old = unsafe { SelectObject(dc, font) };
        let mut got = [0u16; 64];
        let n = unsafe { GetTextFaceW(dc, Some(&mut got)) };
        unsafe { SelectObject(dc, old) };
        let got = String::from_utf16_lossy(&got[..usize::try_from(n.max(1) - 1).unwrap_or(0)]);
        if got.eq_ignore_ascii_case(face) || face == "Segoe UI" {
            return Some(font);
        }
        let _ = unsafe { DeleteObject(font) };
    }
    None
}

/// Draw `text` white on black into a `w` x `h` DIB and return its grayscale
/// coverage, one byte per pixel.
fn render_text(text: &str, w: i32, h: i32, left: i32, right: i32, font_px: i32) -> Option<Vec<u8>> {
    if w <= 0 || h <= 0 || text.is_empty() {
        return None;
    }
    let len = usize::try_from(w).ok()? * usize::try_from(h).ok()?;
    unsafe {
        let screen = GetDC(HWND::default());
        let dc = CreateCompatibleDC(screen);
        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = w;
        bmi.bmiHeader.biHeight = -h;
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        let mut bits: *mut c_void = null_mut();
        let mut out = None;
        if let Ok(bitmap) = CreateDIBSection(
            screen,
            &bmi,
            DIB_RGB_COLORS,
            &mut bits,
            HANDLE::default(),
            0,
        ) {
            if !bits.is_null() {
                let old_bmp = SelectObject(dc, bitmap);
                if let Some(font) = create_font(dc, font_px) {
                    let old_font = SelectObject(dc, font);
                    SetBkMode(dc, TRANSPARENT);
                    SetTextColor(dc, COLORREF(0x00ff_ffff));
                    let mut buf: Vec<u16> = text.encode_utf16().collect();
                    let mut rc = RECT {
                        left,
                        top: 0,
                        right: (w - right).max(left),
                        bottom: h,
                    };
                    DrawTextW(
                        dc,
                        &mut buf,
                        &mut rc,
                        DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
                    );
                    SelectObject(dc, old_font);
                    let _ = DeleteObject(font);
                    // SAFETY: the DIB holds w * h 32-bit pixels, top-down.
                    let px = std::slice::from_raw_parts(bits.cast::<u32>(), len);
                    out = Some(
                        px.iter()
                            .map(|p| {
                                let r = (p >> 16) & 0xff;
                                let g = (p >> 8) & 0xff;
                                let b = p & 0xff;
                                r.max(g).max(b) as u8
                            })
                            .collect(),
                    );
                }
                SelectObject(dc, old_bmp);
            }
            let _ = DeleteObject(bitmap);
        }
        let _ = DeleteDC(dc);
        ReleaseDC(HWND::default(), screen);
        out
    }
}

/// Push premultiplied ARGB `pixels` (`rect.w * rect.h`) to the layered window,
/// moving and resizing it to `rect` in the same call.
fn push(hwnd: HWND, rect: PxRect, pixels: &[u32]) {
    let expected = usize::try_from(rect.w)
        .unwrap_or(0)
        .saturating_mul(usize::try_from(rect.h).unwrap_or(0));
    if expected == 0 || pixels.len() != expected {
        return;
    }
    unsafe {
        let screen = GetDC(HWND::default());
        let mem = CreateCompatibleDC(screen);
        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = rect.w;
        bmi.bmiHeader.biHeight = -rect.h;
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        let mut bits: *mut c_void = null_mut();
        if let Ok(bitmap) = CreateDIBSection(
            screen,
            &bmi,
            DIB_RGB_COLORS,
            &mut bits,
            HANDLE::default(),
            0,
        ) {
            if !bits.is_null() {
                // SAFETY: the DIB holds exactly rect.w * rect.h pixels (checked above).
                std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits.cast::<u32>(), pixels.len());
                let old = SelectObject(mem, bitmap);
                let dst = POINT {
                    x: rect.x,
                    y: rect.y,
                };
                let size = SIZE {
                    cx: rect.w,
                    cy: rect.h,
                };
                let src = POINT { x: 0, y: 0 };
                let blend = BLENDFUNCTION {
                    BlendOp: 0,
                    BlendFlags: 0,
                    SourceConstantAlpha: 255,
                    AlphaFormat: 1,
                };
                let _ = UpdateLayeredWindow(
                    hwnd,
                    screen,
                    Some(&dst),
                    Some(&size),
                    mem,
                    Some(&src),
                    COLORREF(0),
                    Some(&blend),
                    ULW_ALPHA,
                );
                SelectObject(mem, old);
            }
            let _ = DeleteObject(bitmap);
        }
        let _ = DeleteDC(mem);
        ReleaseDC(HWND::default(), screen);
    }
}

// ---------------------------------------------------------------------------
// Menu
// ---------------------------------------------------------------------------

/// Show the right-click menu. Runs a modal loop, so it is called with the state
/// *not* borrowed; `snapshot` carries what it needs.
fn context_menu(hwnd: HWND, snapshot: MenuSnapshot) -> Option<NotchMenu> {
    unsafe {
        let menu = CreatePopupMenu().ok()?;
        let sub = CreatePopupMenu().ok()?;
        let mut strings: Vec<Vec<u16>> = Vec::new();
        let mut add = |m: HMENU, flags, id: usize, text: &str| {
            strings.push(wide(text));
            let ptr = PCWSTR(strings.last().map_or(std::ptr::null(), |s| s.as_ptr()));
            let _ = AppendMenuW(m, flags, id, ptr);
        };
        add(
            menu,
            MF_STRING,
            ID_PAUSE,
            if snapshot.paused {
                "Resume Krova"
            } else {
                "Pause Krova"
            },
        );
        for (i, (name, active)) in snapshot.layouts.iter().enumerate().take(MAX_MENU_LAYOUTS) {
            let flags = if *active {
                MF_STRING | MF_CHECKED
            } else {
                MF_STRING
            };
            add(sub, flags, ID_LAYOUT + i, name);
        }
        add(menu, MF_POPUP, sub.0 as usize, "Layouts");
        add(menu, MF_STRING, ID_SWITCHER, "Layout switcher");
        add(menu, MF_STRING, ID_CHEATS, "Cheat sheet");
        add(menu, MF_SEPARATOR, 0, "");
        add(menu, MF_STRING, ID_OPEN, "Open Krova");
        add(menu, MF_STRING, ID_HIDE, "Hide notch");

        let (x, y) = snapshot.at;
        let vertical = if snapshot.top {
            TPM_TOPALIGN
        } else {
            TPM_BOTTOMALIGN
        };
        // The menu only dismisses on an outside click when its owner is foreground.
        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY | TPM_LEFTALIGN | vertical,
            x,
            y,
            0,
            hwnd,
            None,
        )
        .0 as usize;
        let _ = PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu); // also destroys the submenu
        drop(strings);
        match cmd {
            ID_PAUSE => Some(NotchMenu::TogglePause),
            ID_SWITCHER => Some(NotchMenu::Switcher),
            ID_CHEATS => Some(NotchMenu::CheatSheet),
            ID_OPEN => Some(NotchMenu::OpenApp),
            ID_HIDE => Some(NotchMenu::Hide),
            n if n >= ID_LAYOUT => snapshot
                .layout_ids
                .get(n - ID_LAYOUT)
                .cloned()
                .map(NotchMenu::Layout),
            _ => None,
        }
    }
}

struct MenuSnapshot {
    paused: bool,
    /// `(menu text, active)`.
    layouts: Vec<(String, bool)>,
    layout_ids: Vec<String>,
    at: (i32, i32),
    top: bool,
    monitor_id: String,
}

impl State {
    fn menu_snapshot(&self) -> Option<MenuSnapshot> {
        let m = self.current()?;
        let rect = self.spring.rect();
        let top = self.model.settings.corner.is_top();
        Some(MenuSnapshot {
            paused: self.model.paused,
            layouts: m
                .layouts
                .iter()
                .enumerate()
                .map(|(i, l)| {
                    // Share-safe: the names may be private; number them instead.
                    let text = if self.share_safe {
                        format!("Layout {}", i + 1)
                    } else {
                        // `&` would underline the next letter in a menu.
                        l.name.replace('&', "&&")
                    };
                    (text, l.id == m.active)
                })
                .collect(),
            layout_ids: m.layouts.iter().map(|l| l.id.clone()).collect(),
            at: (rect.x, if top { rect.bottom() } else { rect.y }),
            top,
            monitor_id: m.id.clone(),
        })
    }
}

// ---------------------------------------------------------------------------
// Window and message loop
// ---------------------------------------------------------------------------

unsafe extern "system" fn win_event_proc(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    // The system hook covers a range of events; only these can change what the
    // notch shows (the others, such as mouse capture, fire on every click).
    if !matches!(
        event,
        EVENT_SYSTEM_FOREGROUND
            | EVENT_SYSTEM_MOVESIZEEND
            | EVENT_SYSTEM_MINIMIZEEND
            | EVENT_OBJECT_LOCATIONCHANGE
    ) {
        return;
    }
    if event == EVENT_OBJECT_LOCATIONCHANGE {
        // Only the foreground window itself moving or resizing matters.
        if id_object != 0 || hwnd != unsafe { GetForegroundWindow() } {
            return;
        }
    }
    with_state(|s| {
        if event == EVENT_SYSTEM_FOREGROUND {
            s.follow_foreground();
        }
        let was = s.fullscreen;
        let monitor = s.monitor;
        // A focus change can also move the notch (monitor = cursor).
        if event != EVENT_OBJECT_LOCATIONCHANGE || was != fullscreen_now(s) || monitor.is_none() {
            s.relayout(true);
        }
    });
}

fn fullscreen_now(s: &State) -> bool {
    s.model.settings.hide_in_fullscreen
        && s.current().is_some_and(|m| fullscreen_on(m.rect, s.hwnd))
}

fn track_mouse(hwnd: HWND) {
    let mut tme = TRACKMOUSEEVENT {
        cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
        dwFlags: TME_HOVER | TME_LEAVE,
        hwndTrack: hwnd,
        dwHoverTime: core::HOVER_MS,
    };
    let _ = unsafe { TrackMouseEvent(&mut tme) };
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let handled = match msg {
        WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
        WM_MOUSEMOVE => with_state(|s| {
            if !s.tracking {
                s.tracking = true;
                track_mouse(hwnd);
                s.on_pointer(PointerInput::Enter);
            }
            if let Some((x, y)) = cursor() {
                s.on_pointer(PointerInput::Move { x, y });
            }
        }),
        WM_MOUSEHOVER => with_state(|s| s.on_pointer(PointerInput::Dwell)),
        WM_MOUSELEAVE => with_state(|s| {
            s.tracking = false;
            s.on_pointer(PointerInput::Leave);
        }),
        WM_LBUTTONDOWN => with_state(|s| {
            if let Some((x, y)) = cursor() {
                let _ = unsafe { SetCapture(hwnd) };
                s.on_pointer(PointerInput::Down { x, y });
            }
        }),
        WM_LBUTTONUP => {
            let r = with_state(|s| {
                if let Some((x, y)) = cursor() {
                    s.on_pointer(PointerInput::Up { x, y });
                }
            });
            // Outside the borrow: releasing sends WM_CAPTURECHANGED.
            let _ = unsafe { ReleaseCapture() };
            r
        }
        WM_CAPTURECHANGED => with_state(|s| {
            // Someone else took the mouse mid-press: forget the gesture.
            if s.pointer.dragging() || matches!(s.pointer.phase, core::Phase::Pressed { .. }) {
                s.pointer.cancel();
                s.tracking = false;
                s.relayout(true);
            }
        }),
        WM_RBUTTONUP => {
            let snapshot = with_state(|s| s.menu_snapshot()).flatten();
            if let Some(snap) = snapshot {
                let monitor_id = snap.monitor_id.clone();
                let item = context_menu(hwnd, snap);
                with_state(|s| {
                    s.pointer.cancel();
                    s.tracking = false;
                    if let Some(item) = item {
                        (s.sink)(NotchEvent::Menu {
                            monitor_id: monitor_id.clone(),
                            item,
                        });
                    }
                    s.relayout(true);
                });
            }
            Some(())
        }
        WM_TIMER if wparam.0 == TIMER_ID => with_state(State::tick),
        WM_DISPLAYCHANGE | WM_DPICHANGED => with_state(|s| {
            (s.sink)(NotchEvent::DisplayChanged);
            s.relayout(false);
        }),
        WM_SETTINGCHANGE if wparam.0 == SPI_SETWORKAREA.0 as usize => with_state(|s| {
            (s.sink)(NotchEvent::DisplayChanged);
            s.relayout(false);
        }),
        _ => None,
    };
    if handled.is_some() && msg != WM_DPICHANGED {
        return LRESULT(0);
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn create_window() -> windows::core::Result<HWND> {
    unsafe {
        let hinst = HINSTANCE(GetModuleHandleW(PCWSTR::null())?.0);
        let class = wide("KrovaNotch");
        let mut wc: WNDCLASSEXW = std::mem::zeroed();
        wc.cbSize = size_of::<WNDCLASSEXW>() as u32;
        wc.lpfnWndProc = Some(wnd_proc);
        wc.hInstance = hinst;
        wc.lpszClassName = PCWSTR(class.as_ptr());
        RegisterClassExW(&wc);
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
            PCWSTR(class.as_ptr()),
            PCWSTR::null(),
            WS_POPUP,
            0,
            0,
            1,
            1,
            HWND::default(),
            HMENU::default(),
            hinst,
            None,
        )
    }
}

fn run(inbox: SharedInbox, sink: EventSink<NotchEvent>, ready: mpsc::Sender<Result<u32, String>>) {
    let hwnd = match create_window() {
        Ok(h) => h,
        Err(e) => {
            let _ = ready.send(Err(e.to_string()));
            return;
        }
    };
    // --- integration ---
    // Share-safe hides the notch from screen capture like the other overlays (F-29).
    crate::win_capture::register_overlay(hwnd);
    // --- end integration ---
    let fg_hook = unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_MINIMIZEEND,
            None,
            Some(win_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        )
    };
    let mut msg = MSG::default();
    let _ = unsafe { PeekMessageW(&mut msg, HWND::default(), 0, 0, PM_NOREMOVE) };
    STATE.with(|cell| {
        *cell.borrow_mut() = Some(State {
            hwnd,
            sink,
            model: NotchModel::default(),
            // --- integration --- (start from the mirror; the app pushes updates)
            share_safe: crate::desk::share_safe(),
            pointer: Pointer::new(4),
            spring: RectSpring::at(PxRect::new(0, 0, 1, 1)),
            monitor: None,
            scale: 1.0,
            shown: false,
            tracking: false,
            animating: false,
            last_tick: Instant::now(),
            location_hook: None,
            fullscreen: false,
            text: None,
            painted: None,
        });
    });
    let _ = ready.send(Ok(unsafe { GetCurrentThreadId() }));

    while unsafe { GetMessageW(&mut msg, HWND::default(), 0, 0) }.0 > 0 {
        if msg.message == WM_NOTCH_UPDATE {
            while unsafe {
                PeekMessageW(
                    &mut msg,
                    HWND::default(),
                    WM_NOTCH_UPDATE,
                    WM_NOTCH_UPDATE,
                    PM_REMOVE,
                )
            }
            .as_bool()
            {}
            let taken = inbox
                .lock()
                .map(|mut i| (i.model.take(), i.share_safe.take()))
                .unwrap_or_default();
            with_state(|s| {
                let mut changed = false;
                if let Some(on) = taken.1 {
                    changed |= on != s.share_safe;
                    s.share_safe = on;
                }
                if let Some(model) = taken.0 {
                    changed |= model != s.model;
                    s.model = model;
                }
                if changed || !s.shown {
                    s.follow_foreground();
                    s.relayout(true);
                }
            });
            continue;
        }
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    STATE.with(|cell| {
        if let Some(s) = cell.borrow_mut().take() {
            if let Some((hook, _)) = s.location_hook {
                let _ = unsafe { UnhookWinEvent(hook) };
            }
        }
    });
    if !fg_hook.0.is_null() {
        let _ = unsafe { UnhookWinEvent(fg_hook) };
    }
    // --- integration ---
    crate::win_capture::unregister_overlay(hwnd);
    // --- end integration ---
    let _ = unsafe { DestroyWindow(hwnd) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notch::{NotchLayout, NotchMonitorInfo};

    #[test]
    #[ignore = "creates a real window; needs an interactive desktop session"]
    fn the_notch_starts_shows_updates_and_stops() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&events);
        let notch = WinNotch::start(Arc::new(move |e| {
            if let Ok(mut l) = log.lock() {
                l.push(e);
            }
        }))
        .expect("notch window");
        let model = NotchModel {
            monitors: vec![NotchMonitorInfo {
                id: "monitor-1".into(),
                rect: PxRect::new(0, 0, 1920, 1080),
                work: PxRect::new(0, 0, 1920, 1032),
                scale: 1.0,
                primary: true,
                layouts: vec![NotchLayout {
                    id: "halves".into(),
                    name: "Halves".into(),
                    zones: 2,
                }],
                active: "halves".into(),
            }],
            ..NotchModel::default()
        };
        notch.update(model.clone());
        thread::sleep(Duration::from_millis(300));
        notch.set_share_safe(true);
        let mut paused = model;
        paused.paused = true;
        paused.settings.corner = krova_core::notch::NotchCorner::BottomRight;
        notch.update(paused);
        thread::sleep(Duration::from_millis(500));
        drop(notch);
    }

    #[test]
    #[ignore = "draws text with GDI; needs a desktop session"]
    fn text_renders_some_coverage() {
        let mask = render_text("Halves", 220, 28, 22, 12, 12).expect("mask");
        assert_eq!(mask.len(), 220 * 28);
        assert!(mask.iter().any(|&c| c > 128));
        // Nothing is drawn left of the label's start.
        assert!(mask
            .chunks(220)
            .all(|row| row[..20].iter().all(|&c| c == 0)));
    }
}
