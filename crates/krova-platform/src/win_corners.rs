//! Windows backend for hot corners.
//!
//! Why this design (cheapest correct option):
//! - Krova already keeps one `WH_MOUSE_LL` hook installed for as long as it runs
//!   (`win.rs`, started by the engine at launch, not only during drags). Its
//!   `mouse_proc` calls [`on_mouse_move`] for every move: a relaxed atomic load
//!   when no corner has an action, otherwise a per-axis distance test against at
//!   most [`MAX_SPOTS`] corner points held in static atomics. No lock, no
//!   allocation, no system call unless the cursor is inside a corner's wake band
//!   (or just left it), and then only one `PostThreadMessageW`.
//! - The alternative (four 1x1 layered topmost windows per monitor with
//!   `TrackMouseEvent` hover) would cost 4N windows, z-order fights with other
//!   topmost windows and full-screen apps, and still could not see "left by more
//!   than 24 px" without polling, so it is not used.
//! - A dedicated thread sleeps in `GetMessageW`: it wakes only for those posts,
//!   for the single one-shot dwell timer (`SetTimer`, re-armed rather than
//!   repeated) and, while the ripple plays (220 ms), for its frame timer. Zero
//!   idle CPU.
//! - The same thread owns the ripple's layered click-through window, which also
//!   receives `WM_DISPLAYCHANGE` / work-area changes, so the corner table follows
//!   monitor changes without the engine's help.
//!
//! The rules (outer corners, dwell, re-arm, cooldown) are `krova_core::corners`.
//! Hook coordinates may lie outside the desktop while the cursor is pinned in a
//! corner, so the worker reads the real (clamped) position with `GetCursorPos`.
//! Nothing here reads, logs or stores window titles; the full-screen check looks
//! only at the foreground window's rect and class name.

use std::ffi::c_void;
use std::mem::size_of;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use krova_core::corners::{
    clamp_to_monitors, covers_monitor, outer_spots, paint_ripple, ripple_frame, Corner,
    CornerMachine, CornerSpot, Output, RippleFrame,
};
use krova_core::PxRect;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{
    BOOL, COLORREF, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject,
    BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, DIB_RGB_COLORS, HBITMAP, HDC, HGDIOBJ,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VIRTUAL_KEY, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON, VK_XBUTTON1, VK_XBUTTON2,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClassNameW, GetCursorPos,
    GetForegroundWindow, GetMessageW, GetWindowRect, KillTimer, PeekMessageW, PostThreadMessageW,
    RegisterClassExW, SetTimer, SetWindowPos, ShowWindow, SystemParametersInfoW, TranslateMessage,
    UpdateLayeredWindow, HMENU, HWND_TOPMOST, MSG, PM_NOREMOVE, PM_REMOVE,
    SPI_GETCLIENTAREAANIMATION, SWP_NOACTIVATE, SWP_SHOWWINDOW, SW_HIDE,
    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, ULW_ALPHA, WM_APP, WM_DISPLAYCHANGE, WM_QUIT,
    WM_SETTINGCHANGE, WM_TIMER, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

use crate::corners::{CornerConfig, CornerHit, HotCornersHandle};
use crate::{EventSink, Platform, PlatformError};

/// Most corner points watched at once (4 per monitor, 16 monitors).
pub const MAX_SPOTS: usize = 64;

/// The cursor is in (or just left) a wake band: look at it.
const WM_CORNER_MOVE: u32 = WM_APP + 41;
/// Settings or monitors changed: rebuild the corner table.
const WM_CORNER_RECONFIG: u32 = WM_APP + 42;
const TIMER_DWELL: usize = 1;
const TIMER_RIPPLE: usize = 2;
const RIPPLE_FRAME_MS: u32 = 16;
const SPI_SETWORKAREA: usize = 0x002F;

// ------------------------------------------------- the hook's view (atomics)

#[allow(clippy::declare_interior_mutable_const)]
const ZERO_U64: AtomicU64 = AtomicU64::new(0);
#[allow(clippy::declare_interior_mutable_const)]
const ZERO_I32: AtomicI32 = AtomicI32::new(0);

/// Any corner watched at all? The only thing the hook reads when none is.
static ACTIVE: AtomicBool = AtomicBool::new(false);
static COUNT: AtomicUsize = AtomicUsize::new(0);
/// Corner pixel of each spot, `x` in the high and `y` in the low 32 bits.
static SPOT_XY: [AtomicU64; MAX_SPOTS] = [ZERO_U64; MAX_SPOTS];
/// Half-size of each spot's wake band, in device pixels.
static SPOT_BAND: [AtomicI32; MAX_SPOTS] = [ZERO_I32; MAX_SPOTS];
/// Was the last move inside a band? Lets exactly one "left" sample through.
static INSIDE: AtomicBool = AtomicBool::new(false);
/// The worker thread (0 when none runs).
static THREAD_ID: AtomicU32 = AtomicU32::new(0);

fn pack(x: i32, y: i32) -> u64 {
    (u64::from(x as u32) << 32) | u64::from(y as u32)
}

fn unpack(v: u64) -> (i32, i32) {
    ((v >> 32) as u32 as i32, v as u32 as i32)
}

/// Is `(x, y)` inside any spot's wake band? Allocation-free.
fn in_band(x: i32, y: i32) -> bool {
    let n = COUNT.load(Ordering::Acquire).min(MAX_SPOTS);
    (0..n).any(|i| {
        let (cx, cy) = unpack(SPOT_XY[i].load(Ordering::Relaxed));
        let band = SPOT_BAND[i].load(Ordering::Relaxed).max(0).unsigned_abs();
        x.abs_diff(cx) <= band && y.abs_diff(cy) <= band
    })
}

/// Called by the low-level mouse hook for every `WM_MOUSEMOVE`. Must stay tiny.
pub(crate) fn on_mouse_move(x: i32, y: i32) {
    if !ACTIVE.load(Ordering::Relaxed) {
        return;
    }
    let inside = in_band(x, y);
    let was = INSIDE.swap(inside, Ordering::Relaxed);
    if inside || was {
        let tid = THREAD_ID.load(Ordering::Relaxed);
        if tid != 0 {
            // The hook runs before the cursor actually moves, so the point travels
            // with the message (`GetCursorPos` would still say where it was).
            let _ = unsafe {
                PostThreadMessageW(
                    tid,
                    WM_CORNER_MOVE,
                    WPARAM(x as u32 as usize),
                    LPARAM(y as isize),
                )
            };
        }
    }
}

fn publish(spots: &[CornerSpot]) {
    ACTIVE.store(false, Ordering::Relaxed);
    let n = spots.len().min(MAX_SPOTS);
    for (i, s) in spots.iter().take(n).enumerate() {
        SPOT_XY[i].store(pack(s.x, s.y), Ordering::Relaxed);
        SPOT_BAND[i].store(s.wake_band(), Ordering::Relaxed);
    }
    COUNT.store(n, Ordering::Release);
    INSIDE.store(false, Ordering::Relaxed);
    ACTIVE.store(n > 0, Ordering::Relaxed);
}

fn unpublish() {
    ACTIVE.store(false, Ordering::Relaxed);
    COUNT.store(0, Ordering::Release);
    INSIDE.store(false, Ordering::Relaxed);
}

// ------------------------------------------------------------- small reads

fn key_down(vk: VIRTUAL_KEY) -> bool {
    (unsafe { GetAsyncKeyState(i32::from(vk.0)) }) < 0
}

fn any_button_down() -> bool {
    [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON, VK_XBUTTON1, VK_XBUTTON2]
        .into_iter()
        .any(key_down)
}

fn cursor() -> Option<(i32, i32)> {
    let mut p = POINT::default();
    unsafe { GetCursorPos(&mut p) }.ok()?;
    Some((p.x, p.y))
}

fn animations_on() -> bool {
    let mut on = BOOL(1);
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some((&mut on as *mut BOOL).cast::<c_void>()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    ok.is_err() || on.as_bool()
}

/// Is a full-screen app (game, video, slides) in front on this monitor? Looks at
/// the foreground window's rect and class only; never its title.
fn fullscreen_in_front(monitor: PxRect) -> bool {
    let h = unsafe { GetForegroundWindow() };
    if h.0.is_null() {
        return false;
    }
    let mut buf = [0u16; 64];
    let n = unsafe { GetClassNameW(h, &mut buf) };
    let class = String::from_utf16_lossy(&buf[..usize::try_from(n).unwrap_or(0)]);
    if matches!(
        class.as_str(),
        "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
    ) {
        return false;
    }
    let mut r = RECT::default();
    if unsafe { GetWindowRect(h, &mut r) }.is_err() {
        return false;
    }
    covers_monitor(
        PxRect::new(r.left, r.top, r.right - r.left, r.bottom - r.top),
        monitor,
    )
}

fn monitor_list() -> Vec<(PxRect, f64)> {
    crate::win::WindowsPlatform::new()
        .monitors()
        .map(|ms| ms.iter().map(|m| (m.descriptor.rect, m.scale)).collect())
        .unwrap_or_default()
}

// ------------------------------------------------------------------- start

/// Start the hot-corner watcher. Hits go to `sink` until the handle is dropped.
/// Needs Krova's mouse hook (`Platform::start_input`) to see the cursor; without
/// it the watcher runs but never fires.
pub fn start(
    config: CornerConfig,
    sink: EventSink<CornerHit>,
) -> Result<HotCornersHandle, PlatformError> {
    if THREAD_ID.load(Ordering::Acquire) != 0 {
        return Err(PlatformError::Os("hot corners are already running".into()));
    }
    let shared = Arc::new(Mutex::new(config));
    let (ready_tx, ready_rx) = mpsc::channel::<Result<u32, String>>();
    let worker_cfg = Arc::clone(&shared);
    let join: JoinHandle<()> = thread::Builder::new()
        .name("krova-corners".into())
        .spawn(move || run(worker_cfg, sink, ready_tx))
        .map_err(|e| PlatformError::Os(e.to_string()))?;
    let tid = match ready_rx.recv_timeout(Duration::from_secs(2)) {
        Ok(Ok(tid)) => tid,
        Ok(Err(msg)) => {
            let _ = join.join();
            return Err(PlatformError::Os(format!(
                "could not start hot corners: {msg}"
            )));
        }
        Err(_) => return Err(PlatformError::Os("hot corner thread did not start".into())),
    };
    let join = Mutex::new(Some(join));
    Ok(HotCornersHandle::new(
        move |c| {
            if let Ok(mut slot) = shared.lock() {
                *slot = c;
            }
            let _ = unsafe { PostThreadMessageW(tid, WM_CORNER_RECONFIG, WPARAM(0), LPARAM(0)) };
        },
        move || {
            let _ = unsafe { PostThreadMessageW(tid, WM_QUIT, WPARAM(0), LPARAM(0)) };
            if let Some(j) = join.lock().ok().and_then(|mut j| j.take()) {
                let _ = j.join();
            }
        },
    ))
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let monitors_changed =
        msg == WM_DISPLAYCHANGE || (msg == WM_SETTINGCHANGE && wparam.0 == SPI_SETWORKAREA);
    if monitors_changed {
        let tid = unsafe { GetCurrentThreadId() };
        let _ = unsafe { PostThreadMessageW(tid, WM_CORNER_RECONFIG, WPARAM(0), LPARAM(0)) };
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn create_window() -> windows::core::Result<HWND> {
    unsafe {
        let hinst = HINSTANCE(GetModuleHandleW(PCWSTR::null())?.0);
        let class: Vec<u16> = "KrovaHotCorner\0".encode_utf16().collect();
        // SAFETY: WNDCLASSEXW is plain data; all-zero is a valid starting value.
        let mut wc: WNDCLASSEXW = std::mem::zeroed();
        wc.cbSize = size_of::<WNDCLASSEXW>() as u32;
        wc.lpfnWndProc = Some(wnd_proc);
        wc.hInstance = hinst;
        wc.lpszClassName = PCWSTR(class.as_ptr());
        // Registering twice fails harmlessly; the first class is reused.
        RegisterClassExW(&wc);
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
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

// ------------------------------------------------------------------ worker

struct Worker {
    hwnd: HWND,
    config: Arc<Mutex<CornerConfig>>,
    current: CornerConfig,
    machine: CornerMachine,
    /// Rect and scale of every monitor, as of the last reconfigure.
    monitors: Vec<(PxRect, f64)>,
    sink: EventSink<CornerHit>,
    epoch: Instant,
    ripple: Option<Ripple>,
}

impl Worker {
    fn now(&self) -> u64 {
        u64::try_from(self.epoch.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    fn reconfigure(&mut self) {
        if let Ok(c) = self.config.lock() {
            self.current = c.clone();
        }
        self.monitors = monitor_list();
        let spots = if self.current.settings.any() {
            outer_spots(&self.monitors, &self.current.settings)
        } else {
            Vec::new()
        };
        let _ = unsafe { KillTimer(self.hwnd, TIMER_DWELL) };
        publish(&spots);
        self.machine
            .reconfigure(spots, self.current.settings.dwell_ms);
    }

    fn act(&mut self, out: Output) {
        match out {
            Output::Nothing => {}
            Output::ArmTimer { at_ms } => {
                let delay = at_ms.saturating_sub(self.now()).clamp(1, 60_000) as u32;
                unsafe { SetTimer(self.hwnd, TIMER_DWELL, delay, None) };
            }
            Output::CancelTimer => {
                let _ = unsafe { KillTimer(self.hwnd, TIMER_DWELL) };
            }
            Output::Fire(spot) => {
                let _ = unsafe { KillTimer(self.hwnd, TIMER_DWELL) };
                if self.current.settings.ignore_fullscreen && fullscreen_in_front(spot.monitor_rect)
                {
                    return;
                }
                self.start_ripple(&spot);
                (self.sink)(CornerHit {
                    corner: spot.corner,
                    action: spot.action,
                    monitor: spot.monitor_rect,
                });
            }
        }
    }

    fn on_move(&mut self, hook_pos: (i32, i32)) {
        // Hook points can lie past the desktop edge while the cursor is pinned
        // in a corner; the OS clamps them, and so do we.
        let pos = clamp_to_monitors(hook_pos, &self.monitors);
        let out = self.machine.pointer(pos, any_button_down(), self.now());
        self.act(out);
    }

    fn on_dwell_timer(&mut self) {
        // One-shot: the machine asks again if it needs more time.
        let _ = unsafe { KillTimer(self.hwnd, TIMER_DWELL) };
        let Some(pos) = cursor() else { return };
        let out = self.machine.timer(pos, any_button_down(), self.now());
        self.act(out);
    }

    fn start_ripple(&mut self, spot: &CornerSpot) {
        self.stop_ripple();
        let size = spot.ripple_size().clamp(8, 512);
        let Some(mut r) = Ripple::new(size, spot, self.current.accent, animations_on()) else {
            return;
        };
        let (x, y) = r.origin;
        let _ = unsafe {
            SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                x,
                y,
                size,
                size,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )
        };
        r.draw(self.hwnd, 0);
        unsafe { SetTimer(self.hwnd, TIMER_RIPPLE, RIPPLE_FRAME_MS, None) };
        self.ripple = Some(r);
    }

    fn ripple_tick(&mut self) {
        let Some(r) = self.ripple.as_mut() else {
            let _ = unsafe { KillTimer(self.hwnd, TIMER_RIPPLE) };
            return;
        };
        let t = u32::try_from(r.start.elapsed().as_millis()).unwrap_or(u32::MAX);
        if !r.draw(self.hwnd, t) {
            self.stop_ripple();
        }
    }

    fn stop_ripple(&mut self) {
        let _ = unsafe { KillTimer(self.hwnd, TIMER_RIPPLE) };
        if self.ripple.take().is_some() {
            let _ = unsafe { ShowWindow(self.hwnd, SW_HIDE) };
        }
    }
}

/// The ripple's bitmap, alive only while it plays.
struct Ripple {
    mem: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
    bits: *mut u32,
    size: i32,
    origin: (i32, i32),
    corner: Corner,
    rgb: (u8, u8, u8),
    start: Instant,
    animate: bool,
}

impl Ripple {
    fn new(size: i32, spot: &CornerSpot, rgb: (u8, u8, u8), animate: bool) -> Option<Self> {
        unsafe {
            let screen = GetDC(HWND::default());
            let mem = CreateCompatibleDC(screen);
            ReleaseDC(HWND::default(), screen);
            if mem.is_invalid() {
                return None;
            }
            // SAFETY: BITMAPINFO is plain data; all-zero is a valid starting value.
            let mut bmi: BITMAPINFO = std::mem::zeroed();
            bmi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = size;
            bmi.bmiHeader.biHeight = -size; // top-down rows
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32;
            let mut bits: *mut c_void = null_mut();
            let Ok(bitmap) =
                CreateDIBSection(mem, &bmi, DIB_RGB_COLORS, &mut bits, HANDLE::default(), 0)
            else {
                let _ = DeleteDC(mem);
                return None;
            };
            if bits.is_null() {
                let _ = DeleteObject(bitmap);
                let _ = DeleteDC(mem);
                return None;
            }
            let old = SelectObject(mem, bitmap);
            Some(Self {
                mem,
                bitmap,
                old,
                bits: bits.cast::<u32>(),
                size,
                origin: spot.ripple_origin(size),
                corner: spot.corner,
                rgb,
                start: Instant::now(),
                animate,
            })
        }
    }

    /// Paint the frame `t_ms` in; `false` once the ripple is over.
    fn draw(&mut self, hwnd: HWND, t_ms: u32) -> bool {
        let frame = if self.animate {
            ripple_frame(t_ms)
        } else {
            // Reduced motion: a still, half-strength quarter disc, then gone.
            (t_ms < krova_core::corners::RIPPLE_MS).then_some(RippleFrame {
                radius: 1.0,
                alpha: 0.6,
            })
        };
        let Some(frame) = frame else { return false };
        let n = usize::try_from(self.size).unwrap_or(0);
        // SAFETY: the DIB section holds size * size 32-bit pixels and lives as
        // long as `self`; only this thread touches it.
        let pixels = unsafe { std::slice::from_raw_parts_mut(self.bits, n * n) };
        paint_ripple(pixels, self.size, self.corner, self.rgb, frame);
        let dst = POINT {
            x: self.origin.0,
            y: self.origin.1,
        };
        let size = SIZE {
            cx: self.size,
            cy: self.size,
        };
        let src = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: 0,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: 1, // AC_SRC_ALPHA: premultiplied
        };
        unsafe {
            let screen = GetDC(HWND::default());
            let _ = UpdateLayeredWindow(
                hwnd,
                screen,
                Some(&dst),
                Some(&size),
                self.mem,
                Some(&src),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            );
            ReleaseDC(HWND::default(), screen);
        }
        true
    }
}

impl Drop for Ripple {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.mem, self.old);
            let _ = DeleteObject(self.bitmap);
            let _ = DeleteDC(self.mem);
        }
    }
}

fn run(
    config: Arc<Mutex<CornerConfig>>,
    sink: EventSink<CornerHit>,
    ready: mpsc::Sender<Result<u32, String>>,
) {
    let hwnd = match create_window() {
        Ok(h) => h,
        Err(e) => {
            let _ = ready.send(Err(e.to_string()));
            return;
        }
    };
    let mut msg = MSG::default();
    // Touching the queue creates it, so posts sent right away are not lost.
    let _ = unsafe { PeekMessageW(&mut msg, HWND::default(), 0, 0, PM_NOREMOVE) };
    let tid = unsafe { GetCurrentThreadId() };
    THREAD_ID.store(tid, Ordering::Release);

    let current = config
        .lock()
        .map(|c| c.clone())
        .unwrap_or_else(|_| CornerConfig::off((0xff, 0x5b, 0x1f)));
    let mut w = Worker {
        hwnd,
        config,
        machine: CornerMachine::new(Vec::new(), current.settings.dwell_ms),
        current,
        monitors: Vec::new(),
        sink,
        epoch: Instant::now(),
        ripple: None,
    };
    w.reconfigure();
    let _ = ready.send(Ok(tid));

    while unsafe { GetMessageW(&mut msg, HWND::default(), 0, 0) }.0 > 0 {
        match msg.message {
            WM_CORNER_MOVE => {
                // Only the latest position matters.
                let mut last = msg;
                while unsafe {
                    PeekMessageW(
                        &mut msg,
                        HWND::default(),
                        WM_CORNER_MOVE,
                        WM_CORNER_MOVE,
                        PM_REMOVE,
                    )
                }
                .as_bool()
                {
                    last = msg;
                }
                w.on_move((last.wParam.0 as u32 as i32, last.lParam.0 as i32));
            }
            WM_CORNER_RECONFIG => {
                while unsafe {
                    PeekMessageW(
                        &mut msg,
                        HWND::default(),
                        WM_CORNER_RECONFIG,
                        WM_CORNER_RECONFIG,
                        PM_REMOVE,
                    )
                }
                .as_bool()
                {}
                w.reconfigure();
            }
            WM_TIMER if msg.hwnd == hwnd && msg.wParam.0 == TIMER_DWELL => w.on_dwell_timer(),
            WM_TIMER if msg.hwnd == hwnd && msg.wParam.0 == TIMER_RIPPLE => w.ripple_tick(),
            _ => unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            },
        }
    }

    unpublish();
    THREAD_ID.store(0, Ordering::Release);
    w.stop_ripple();
    let _ = unsafe { KillTimer(hwnd, TIMER_DWELL) };
    let _ = unsafe { DestroyWindow(hwnd) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use krova_core::corners::{HotCornerAction, HotCornerSettings};

    #[test]
    fn points_pack_and_unpack() {
        for (x, y) in [(0, 0), (-1920, 1079), (i32::MAX, i32::MIN), (5759, -1)] {
            assert_eq!(unpack(pack(x, y)), (x, y));
        }
    }

    /// Serialises the tests that use the global corner table.
    static TABLE: Mutex<()> = Mutex::new(());

    #[test]
    fn the_band_test_matches_the_published_spots() {
        let _g = TABLE.lock().unwrap();
        let s = HotCornerSettings {
            top_left: Some(HotCornerAction::Focus),
            ..HotCornerSettings::default()
        };
        let m = PxRect::new(-1920, 0, 1920, 1080);
        let spots = outer_spots(&[(m, 1.0)], &s);
        publish(&spots);
        assert!(ACTIVE.load(Ordering::Relaxed));
        // Top-left at (-1920, 0), band 25; top-right at (-1, 0); bottom-right.
        assert!(in_band(-1920, 0));
        assert!(in_band(-1895, 25));
        assert!(in_band(-1925, -3), "hook points past the edge still count");
        assert!(!in_band(-1894, 0));
        assert!(in_band(-1, 1079));
        assert!(!in_band(-960, 540));
        unpublish();
        assert!(!ACTIVE.load(Ordering::Relaxed));
        assert!(!in_band(-1920, 0));
    }

    #[test]
    fn at_most_max_spots_are_published() {
        let _g = TABLE.lock().unwrap();
        let s = HotCornerSettings {
            top_left: Some(HotCornerAction::Focus),
            top_right: Some(HotCornerAction::Focus),
            bottom_left: Some(HotCornerAction::Focus),
            bottom_right: Some(HotCornerAction::Focus),
            ..HotCornerSettings::default()
        };
        // 20 separate monitors far apart: 80 corners.
        let monitors: Vec<(PxRect, f64)> = (0..20)
            .map(|i| (PxRect::new(i * 3000, 0, 1920, 1080), 1.0))
            .collect();
        let spots = outer_spots(&monitors, &s);
        assert_eq!(spots.len(), 80);
        publish(&spots);
        assert_eq!(COUNT.load(Ordering::Relaxed), MAX_SPOTS);
        unpublish();
    }

    /// Moves the real cursor into the primary monitor's top-left corner (with
    /// `SendInput`, so the low-level hook sees it) and waits for the hit.
    #[test]
    #[ignore = "moves the real mouse; needs an interactive desktop session"]
    fn a_real_cursor_in_a_corner_fires_once() {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_MOVE,
            MOUSEEVENTF_VIRTUALDESK, MOUSEINPUT,
        };
        use windows::Win32::UI::WindowsAndMessaging::{
            GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
            SM_YVIRTUALSCREEN,
        };

        let _g = TABLE.lock().unwrap();
        let p = crate::win::WindowsPlatform::new();
        let monitors = p.monitors().expect("monitors");
        let primary = monitors.iter().find(|m| m.primary).expect("a primary");
        let list: Vec<(PxRect, f64)> = monitors
            .iter()
            .map(|m| (m.descriptor.rect, m.scale))
            .collect();
        let index = monitors.iter().position(|m| m.primary).unwrap();
        if !krova_core::corners::is_outer(&list, index, Corner::TopLeft) {
            eprintln!("primary top-left corner touches another monitor; skipped");
            return;
        }
        let (input_tx, _input_rx) = mpsc::channel();
        let _input = p
            .start_input(krova_core::Modifier::Shift, input_tx)
            .expect("mouse hook");
        let (tx, rx) = mpsc::channel::<CornerHit>();
        let tx = Mutex::new(tx);
        let sink: EventSink<CornerHit> = Arc::new(move |hit| {
            let _ = tx.lock().unwrap().send(hit);
        });
        let settings = HotCornerSettings {
            top_left: Some(HotCornerAction::Focus),
            top_right: None,
            bottom_right: None,
            dwell_ms: 150,
            ignore_fullscreen: false,
            ..HotCornerSettings::default()
        };
        let handle = start(
            CornerConfig {
                settings,
                accent: (0xff, 0x5b, 0x1f),
            },
            sink,
        )
        .expect("hot corners start");
        let before = cursor();

        let move_to = |x: i32, y: i32| unsafe {
            let vx = GetSystemMetrics(SM_XVIRTUALSCREEN);
            let vy = GetSystemMetrics(SM_YVIRTUALSCREEN);
            let vw = GetSystemMetrics(SM_CXVIRTUALSCREEN).max(2);
            let vh = GetSystemMetrics(SM_CYVIRTUALSCREEN).max(2);
            let input = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx: (x - vx) * 65535 / (vw - 1),
                        dy: (y - vy) * 65535 / (vh - 1),
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            SendInput(&[input], size_of::<INPUT>() as i32);
        };
        let r = primary.descriptor.rect;
        move_to(r.x + r.w / 2, r.y + r.h / 2);
        thread::sleep(Duration::from_millis(50));
        move_to(r.x, r.y);
        let hit = rx.recv_timeout(Duration::from_secs(3));
        // Resting longer does not fire again.
        let again = rx.recv_timeout(Duration::from_millis(700));
        if let Some((x, y)) = before {
            move_to(x, y);
        }
        drop(handle);
        let hit = hit.expect("the corner fired");
        assert_eq!(hit.corner, Corner::TopLeft);
        assert_eq!(hit.action, HotCornerAction::Focus);
        assert_eq!(hit.monitor, r);
        assert!(again.is_err(), "fired twice");
        assert_eq!(THREAD_ID.load(Ordering::Relaxed), 0);
    }
}
