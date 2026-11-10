//! Windows: the focus-dim and window-highlight overlays (wave 3).
//!
//! One thread, started only while focus dim or a highlight is on, owns:
//! - WinEvent hooks (foreground, minimise, show/hide/destroy, cloak, and location
//!   changes of the focused program only, or of every program in "all borders"
//!   mode). The callbacks only post a message to the thread; all work happens in
//!   its message loop.
//! - One dim window per monitor: layered, click-through, no-activate, topmost,
//!   black at the configured opacity, with a rounded hole (a window region) where
//!   the focused window is.
//! - The highlight: Windows 11 draws thin borders itself (`DWMWA_BORDER_COLOR`);
//!   otherwise a click-through ring window follows the focused window (or one ring
//!   per window in "all" mode), and the glow is a per-pixel-alpha window.
//! - A 16 ms thread timer, alive **only while a spring animation runs**. When
//!   nothing moves, the thread sleeps in `GetMessageW`: zero idle CPU.
//!
//! The thread is per-monitor DPI aware, so every rect is in physical pixels and
//! widths are scaled by the monitor's DPI. Window titles are never read.

use std::collections::HashMap;
use std::ffi::c_void;
use std::mem::size_of;
use std::ptr::null_mut;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use krova_core::focus::{
    alpha_byte, dim_hole, glow_alpha, hex_rgb, highlight_backend, is_fullscreen, outset,
    rounded_rect_distance, scaled, spring_mix, spring_rect, window_corner_radius, HighlightBackend,
    VisualsConfig, SPRING_MS,
};
use krova_core::PxRect;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{
    COLORREF, HANDLE, HINSTANCE, HMODULE, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM,
};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_COLOR_DEFAULT,
};
use windows::Win32::Graphics::Gdi::{
    CombineRgn, CreateCompatibleDC, CreateDIBSection, CreateRectRgn, CreateRoundRectRgn,
    CreateSolidBrush, DeleteDC, DeleteObject, EnumDisplayMonitors, FillRect, GetDC,
    GetMonitorInfoW, ReleaseDC, SelectObject, SetWindowRgn, BITMAPINFO, BITMAPINFOHEADER,
    BLENDFUNCTION, DIB_RGB_COLORS, HDC, HMONITOR, HRGN, MONITORINFO, RGN_DIFF,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::HiDpi::{
    GetDpiForMonitor, SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    MDT_EFFECTIVE_DPI,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetForegroundWindow,
    GetMessageW, GetWindow, GetWindowLongPtrW, GetWindowThreadProcessId, IsIconic, IsWindow,
    KillTimer, PeekMessageW, PostThreadMessageW, RegisterClassExW, SetLayeredWindowAttributes,
    SetTimer, SetWindowLongPtrW, SetWindowPos, ShowWindow, TranslateMessage, UpdateLayeredWindow,
    EVENT_OBJECT_CLOAKED, EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE, EVENT_OBJECT_LOCATIONCHANGE,
    EVENT_OBJECT_UNCLOAKED, EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_MINIMIZEEND,
    EVENT_SYSTEM_MINIMIZESTART, GWLP_USERDATA, GW_HWNDPREV, HMENU, HTTRANSPARENT, HWND_TOP,
    HWND_TOPMOST, LWA_ALPHA, MSG, OBJID_WINDOW, PM_NOREMOVE, SWP_NOACTIVATE, SWP_NOOWNERZORDER,
    SWP_SHOWWINDOW, SW_HIDE, ULW_ALPHA, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_APP,
    WM_DISPLAYCHANGE, WM_DPICHANGED, WM_ERASEBKGND, WM_NCHITTEST, WM_QUIT, WM_SETTINGCHANGE,
    WM_TIMER, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_EX_TRANSPARENT, WS_POPUP,
};

use crate::desk::Visuals;
use crate::win_desk::{
    frame, is_cloaked, ordinary_window as arrangeable, os_build, top_level_windows,
};
use crate::PlatformError;

/// A WinEvent arrived: wParam = window, lParam = event id.
const WM_EVENT: u32 = WM_APP + 31;
/// The shared config changed.
const WM_CONFIG: u32 = WM_APP + 32;
/// Monitors or DPI changed: rebuild the dim windows.
const WM_REBUILD: u32 = WM_APP + 33;
/// Frame interval while something animates.
const FRAME_MS: u32 = 16;
/// Most windows that get their own ring in "all borders" overlay mode.
const MAX_RINGS: usize = 48;
/// Ring alpha for windows that are not focused, in "all borders" mode.
const UNFOCUSED_RING_ALPHA: u8 = 140;

type SharedConfig = Arc<Mutex<VisualsConfig>>;

pub struct WinVisuals {
    config: SharedConfig,
    thread_id: u32,
    join: Option<JoinHandle<()>>,
}

impl WinVisuals {
    pub fn start(config: VisualsConfig) -> Result<Self, PlatformError> {
        let shared: SharedConfig = Arc::new(Mutex::new(config));
        let (ready_tx, ready_rx) = mpsc::channel::<Result<u32, String>>();
        let for_thread = Arc::clone(&shared);
        let join = thread::Builder::new()
            .name("krova-visuals".into())
            .spawn(move || run(&for_thread, &ready_tx))
            .map_err(|e| PlatformError::Os(e.to_string()))?;
        match ready_rx.recv_timeout(Duration::from_secs(2)) {
            Ok(Ok(thread_id)) => Ok(Self {
                config: shared,
                thread_id,
                join: Some(join),
            }),
            Ok(Err(msg)) => {
                let _ = join.join();
                Err(PlatformError::Os(format!(
                    "could not start the overlays: {msg}"
                )))
            }
            Err(_) => Err(PlatformError::Os("the overlay thread did not start".into())),
        }
    }
}

impl Visuals for WinVisuals {
    fn set_config(&self, config: VisualsConfig) {
        if let Ok(mut slot) = self.config.lock() {
            *slot = config;
        }
        // SAFETY: the thread is alive until `drop` joins it, so the id is ours.
        let _ = unsafe { PostThreadMessageW(self.thread_id, WM_CONFIG, WPARAM(0), LPARAM(0)) };
    }
}

impl Drop for WinVisuals {
    fn drop(&mut self) {
        // SAFETY: as above.
        let _ = unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

// ---------------------------------------------------------------- windows

fn colorref((r, g, b): (u8, u8, u8)) -> COLORREF {
    COLORREF(u32::from(r) | (u32::from(g) << 8) | (u32::from(b) << 16))
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
        WM_ERASEBKGND => {
            // The fill colour is kept in the window's user data as a COLORREF.
            // SAFETY: wparam is the HDC Windows hands us for erasing.
            unsafe {
                let colour = COLORREF(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as u32);
                let brush = CreateSolidBrush(colour);
                let mut r = RECT::default();
                let _ = windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut r);
                FillRect(HDC(wparam.0 as *mut c_void), &r, brush);
                let _ = DeleteObject(brush);
            }
            LRESULT(1)
        }
        WM_DISPLAYCHANGE | WM_DPICHANGED | WM_SETTINGCHANGE => {
            // SAFETY: posting to our own thread.
            let _ = unsafe {
                PostThreadMessageW(GetCurrentThreadId(), WM_REBUILD, WPARAM(0), LPARAM(0))
            };
            LRESULT(0)
        }
        // SAFETY: forwards the arguments unchanged.
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn class() -> windows::core::Result<(HINSTANCE, Vec<u16>)> {
    // SAFETY: standard class registration; the name buffer outlives the call.
    unsafe {
        let hinst = HINSTANCE(GetModuleHandleW(PCWSTR::null())?.0);
        let name: Vec<u16> = "KrovaVisual\0".encode_utf16().collect();
        let mut wc: WNDCLASSEXW = std::mem::zeroed();
        wc.cbSize = size_of::<WNDCLASSEXW>() as u32;
        wc.lpfnWndProc = Some(wnd_proc);
        wc.hInstance = hinst;
        wc.lpszClassName = PCWSTR(name.as_ptr());
        // A second registration fails harmlessly; the first class is reused.
        RegisterClassExW(&wc);
        Ok((hinst, name))
    }
}

/// A hidden overlay window. `topmost` for dim / focused ring / glow.
fn overlay_window(topmost: bool) -> Option<HWND> {
    let (hinst, name) = class().ok()?;
    let mut ex = WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
    if topmost {
        ex |= WS_EX_TOPMOST;
    }
    // SAFETY: creates a window of the class registered above on this thread.
    unsafe {
        CreateWindowExW(
            ex,
            PCWSTR(name.as_ptr()),
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
    .ok()
}

fn set_fill(h: HWND, colour: COLORREF, alpha: u8) {
    // SAFETY: our own window.
    unsafe {
        SetWindowLongPtrW(h, GWLP_USERDATA, colour.0 as isize);
        let _ = SetLayeredWindowAttributes(h, COLORREF(0), alpha, LWA_ALPHA);
    }
}

fn place(h: HWND, r: PxRect, after: HWND) {
    // SAFETY: our own window.
    let _ = unsafe {
        SetWindowPos(
            h,
            after,
            r.x,
            r.y,
            r.w.max(1),
            r.h.max(1),
            SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_NOOWNERZORDER,
        )
    };
}

fn hide(h: HWND) {
    // SAFETY: our own window.
    let _ = unsafe { ShowWindow(h, SW_HIDE) };
}

fn destroy(h: HWND) {
    // SAFETY: our own window, on its own thread.
    let _ = unsafe { DestroyWindow(h) };
}

/// Hand a region to a window (the system owns it afterwards).
fn set_region(h: HWND, rgn: HRGN) {
    // SAFETY: on success the system owns `rgn`; on failure we delete it.
    unsafe {
        if SetWindowRgn(h, rgn, true) == 0 {
            let _ = DeleteObject(rgn);
        }
    }
}

/// `w x h` minus a rounded hole (local coordinates).
fn region_with_hole(w: i32, h: i32, hole: Option<PxRect>, radius: i32) -> HRGN {
    // SAFETY: GDI region calls; temporaries are deleted.
    unsafe {
        let full = CreateRectRgn(0, 0, w, h);
        if let Some(r) = hole {
            let d = radius * 2;
            let cut = CreateRoundRectRgn(r.x, r.y, r.right() + 1, r.bottom() + 1, d, d);
            CombineRgn(full, full, cut, RGN_DIFF);
            let _ = DeleteObject(cut);
        }
        full
    }
}

/// A ring `width` thick inside a `w x h` box, inner radius `inner`.
fn ring_region(w: i32, h: i32, width: i32, inner: i32) -> HRGN {
    let outer = inner + width;
    // SAFETY: GDI region calls; the temporary is deleted.
    unsafe {
        let o = CreateRoundRectRgn(0, 0, w + 1, h + 1, outer * 2, outer * 2);
        let i = CreateRoundRectRgn(
            width,
            width,
            w - width + 1,
            h - width + 1,
            inner * 2,
            inner * 2,
        );
        CombineRgn(o, o, i, RGN_DIFF);
        let _ = DeleteObject(i);
        o
    }
}

fn set_dwm_border(h: HWND, colour: u32) {
    // SAFETY: `colour` outlives the call; other processes' windows accept this
    // attribute on Windows 11 (it is ignored elsewhere).
    let _ = unsafe {
        DwmSetWindowAttribute(
            h,
            DWMWA_BORDER_COLOR,
            (&colour as *const u32).cast::<c_void>(),
            size_of::<u32>() as u32,
        )
    };
}

// --------------------------------------------------------------- monitors

#[derive(Debug, Clone, Copy)]
struct Mon {
    rect: PxRect,
    scale: f64,
}

unsafe extern "system" fn collect_monitor(
    h: HMONITOR,
    _: HDC,
    _: *mut RECT,
    data: LPARAM,
) -> windows::Win32::Foundation::BOOL {
    // SAFETY: `data` is the Vec `monitors()` keeps alive for the enumeration.
    let list = unsafe { &mut *(data.0 as *mut Vec<HMONITOR>) };
    list.push(h);
    windows::Win32::Foundation::TRUE
}

fn monitor_scale(h: HMONITOR) -> f64 {
    let (mut x, mut y) = (96u32, 96u32);
    // SAFETY: valid out-pointers.
    if unsafe { GetDpiForMonitor(h, MDT_EFFECTIVE_DPI, &mut x, &mut y) }.is_ok() {
        f64::from(x) / 96.0
    } else {
        1.0
    }
}

fn monitors() -> Vec<Mon> {
    let mut handles: Vec<HMONITOR> = Vec::new();
    // SAFETY: the callback only pushes into `handles`.
    let _ = unsafe {
        EnumDisplayMonitors(
            HDC::default(),
            None,
            Some(collect_monitor),
            LPARAM(&mut handles as *mut Vec<HMONITOR> as isize),
        )
    };
    handles
        .into_iter()
        .filter_map(|h| {
            let mut mi = MONITORINFO {
                cbSize: size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            // SAFETY: valid out-pointer of the declared size.
            unsafe { GetMonitorInfoW(h, &mut mi) }
                .as_bool()
                .then(|| Mon {
                    rect: crate::win_desk::px(mi.rcMonitor),
                    scale: monitor_scale(h),
                })
        })
        .collect()
}

fn monitor_of(mons: &[Mon], r: PxRect) -> Option<Mon> {
    let rects: Vec<PxRect> = mons.iter().map(|m| m.rect).collect();
    krova_core::registry::monitor_for_rect(&rects, r).map(|i| mons[i])
}

// --------------------------------------------------------------- the state

#[derive(Debug, Clone, Copy)]
struct Spring<T: Copy> {
    from: T,
    to: T,
    start: Instant,
}

impl Spring<f64> {
    fn now(&self) -> f64 {
        spring_mix(self.from, self.to, elapsed_ms(self.start))
    }
}

impl Spring<PxRect> {
    fn now(&self) -> PxRect {
        spring_rect(self.from, self.to, elapsed_ms(self.start))
    }
}

fn elapsed_ms(start: Instant) -> u32 {
    u32::try_from(start.elapsed().as_millis()).unwrap_or(u32::MAX)
}

fn done<T: Copy>(s: &Spring<T>) -> bool {
    elapsed_ms(s.start) >= SPRING_MS
}

struct Dim {
    hwnd: HWND,
    mon: Mon,
    shown_hole: Option<Option<PxRect>>,
}

struct State {
    cfg: VisualsConfig,
    backend: HighlightBackend,
    build: u32,
    mons: Vec<Mon>,
    fg: Option<HWND>,
    dims: Vec<Dim>,
    dim_alpha: Spring<f64>,
    hole: Option<Spring<PxRect>>,
    ring: Option<HWND>,
    ring_rect: Option<Spring<PxRect>>,
    ring_shape: Option<(i32, i32)>,
    glow: Option<HWND>,
    glow_size: Option<(i32, i32)>,
    rings: HashMap<isize, HWND>,
    dwm_coloured: Vec<isize>,
    loc_hook: Option<HWINEVENTHOOK>,
    loc_pid: u32,
    timer: usize,
    refresh_all: bool,
}

fn rgb(cfg: &VisualsConfig) -> (u8, u8, u8) {
    hex_rgb(&cfg.color).unwrap_or((0xff, 0x5b, 0x1f))
}

impl State {
    fn new(cfg: VisualsConfig) -> Self {
        let build = os_build();
        let backend = highlight_backend(&cfg.highlight, build);
        Self {
            cfg,
            backend,
            build,
            mons: monitors(),
            fg: None,
            dims: Vec::new(),
            dim_alpha: Spring {
                from: 0.0,
                to: 0.0,
                start: Instant::now(),
            },
            hole: None,
            ring: None,
            ring_rect: None,
            ring_shape: None,
            glow: None,
            glow_size: None,
            rings: HashMap::new(),
            dwm_coloured: Vec::new(),
            loc_hook: None,
            loc_pid: 0,
            timer: 0,
            refresh_all: false,
        }
    }

    /// The focused window's frame, if it is one we decorate.
    fn focused_rect(&self) -> Option<PxRect> {
        let h = self.fg?;
        if !arrangeable(h, false) {
            return None;
        }
        frame(h)
    }

    fn build_windows(&mut self) {
        if self.cfg.dim {
            self.dims = self
                .mons
                .iter()
                .filter_map(|&mon| {
                    let hwnd = overlay_window(true)?;
                    set_fill(hwnd, COLORREF(0), 0);
                    Some(Dim {
                        hwnd,
                        mon,
                        shown_hole: None,
                    })
                })
                .collect();
        }
        match self.backend {
            HighlightBackend::OverlayActive => {
                self.ring = overlay_window(true);
                if let Some(r) = self.ring {
                    set_fill(r, colorref(rgb(&self.cfg)), 255);
                }
            }
            HighlightBackend::Glow => self.glow = overlay_window(true),
            _ => {}
        }
    }

    fn teardown(&mut self) {
        for d in self.dims.drain(..) {
            destroy(d.hwnd);
        }
        for h in [self.ring.take(), self.glow.take()].into_iter().flatten() {
            destroy(h);
        }
        for (_, h) in self.rings.drain() {
            destroy(h);
        }
        for h in self.dwm_coloured.drain(..) {
            let w = HWND(h as *mut c_void);
            // SAFETY: plain query.
            if unsafe { IsWindow(w) }.as_bool() {
                set_dwm_border(w, DWMWA_COLOR_DEFAULT);
            }
        }
        self.ring_rect = None;
        self.ring_shape = None;
        self.glow_size = None;
        self.hole = None;
        self.unhook_location();
        self.stop_timer();
    }

    fn unhook_location(&mut self) {
        if let Some(h) = self.loc_hook.take() {
            // SAFETY: a hook this thread installed.
            let _ = unsafe { UnhookWinEvent(h) };
        }
        self.loc_pid = 0;
    }

    /// Location changes: of the focused program only, except in "all" overlay mode.
    fn hook_location(&mut self) {
        let wants_all = self.backend == HighlightBackend::OverlayAll;
        let needs = self.cfg.dim
            || matches!(
                self.backend,
                HighlightBackend::OverlayActive
                    | HighlightBackend::Glow
                    | HighlightBackend::OverlayAll
            );
        if !needs {
            self.unhook_location();
            return;
        }
        let pid = if wants_all {
            0
        } else {
            let mut pid = 0u32;
            if let Some(h) = self.fg {
                // SAFETY: valid out-pointer.
                unsafe { GetWindowThreadProcessId(h, Some(&mut pid)) };
            }
            if pid == 0 {
                self.unhook_location();
                return;
            }
            pid
        };
        if self.loc_hook.is_some() && self.loc_pid == pid {
            return;
        }
        self.unhook_location();
        // SAFETY: out-of-context hook, removed on this thread.
        let hook = unsafe {
            SetWinEventHook(
                EVENT_OBJECT_LOCATIONCHANGE,
                EVENT_OBJECT_LOCATIONCHANGE,
                HMODULE::default(),
                Some(event_proc),
                pid,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            )
        };
        if !hook.is_invalid() {
            self.loc_hook = Some(hook);
            self.loc_pid = pid;
        }
    }

    fn start_timer(&mut self) {
        if self.timer == 0 {
            // SAFETY: a thread timer; killed in `stop_timer`.
            self.timer = unsafe { SetTimer(HWND::default(), 0, FRAME_MS, None) };
        }
    }

    fn stop_timer(&mut self) {
        if self.timer != 0 {
            // SAFETY: the timer this thread created.
            let _ = unsafe { KillTimer(HWND::default(), self.timer) };
            self.timer = 0;
        }
    }

    // ---- focus

    /// Focus moved (or the config changed): retarget the springs.
    fn on_focus(&mut self, h: Option<HWND>) {
        let old = self.fg;
        self.fg = h;
        self.hook_location();
        let target = self.focused_rect();

        // Dim fades in when there is a decorated focused window, out otherwise.
        if self.cfg.dim {
            let want = if target.is_some() {
                f64::from(self.cfg.dim_opacity)
            } else {
                0.0
            };
            let current = self.dim_alpha.now();
            if (want - self.dim_alpha.to).abs() > f64::EPSILON {
                self.dim_alpha = Spring {
                    from: current,
                    to: want,
                    start: Instant::now(),
                };
            }
            self.hole = match (self.hole, target) {
                (Some(s), Some(t)) if current > 0.01 => Some(Spring {
                    from: s.now(),
                    to: t,
                    start: Instant::now(),
                }),
                (_, Some(t)) => Some(Spring {
                    from: t,
                    to: t,
                    start: Instant::now(),
                }),
                (s, None) => s,
            };
            // Keep the dim above whatever just came forward.
            for d in &self.dims {
                if current > 0.0 || want > 0.0 {
                    // SAFETY: our own window.
                    let _ = unsafe {
                        SetWindowPos(
                            d.hwnd,
                            HWND_TOPMOST,
                            0,
                            0,
                            0,
                            0,
                            SWP_NOACTIVATE
                                | windows::Win32::UI::WindowsAndMessaging::SWP_NOMOVE
                                | windows::Win32::UI::WindowsAndMessaging::SWP_NOSIZE,
                        )
                    };
                }
            }
        }

        match self.backend {
            HighlightBackend::DwmActive => {
                if let Some(o) = old.filter(|&o| Some(o) != h) {
                    set_dwm_border(o, DWMWA_COLOR_DEFAULT);
                    self.dwm_coloured.retain(|&x| x != o.0 as isize);
                }
                if let (Some(n), Some(_)) = (h, target) {
                    set_dwm_border(n, colorref(rgb(&self.cfg)).0);
                    if !self.dwm_coloured.contains(&(n.0 as isize)) {
                        self.dwm_coloured.push(n.0 as isize);
                    }
                }
            }
            HighlightBackend::DwmAll | HighlightBackend::OverlayAll => self.refresh_all = true,
            HighlightBackend::OverlayActive | HighlightBackend::Glow => {
                let ring = self.ring_target(target);
                self.ring_rect = match (self.ring_rect, ring) {
                    (Some(s), Some(t)) if old.is_some() && old != h => Some(Spring {
                        from: s.now(),
                        to: t,
                        start: Instant::now(),
                    }),
                    (_, Some(t)) => Some(Spring {
                        from: t,
                        to: t,
                        start: Instant::now(),
                    }),
                    (_, None) => None,
                };
            }
            HighlightBackend::Off => {}
        }
        self.frame();
    }

    /// The focused window moved or resized: follow at once (no spring).
    fn on_moved(&mut self, h: HWND) {
        if Some(h) == self.fg {
            let target = self.focused_rect();
            if let (Some(t), true) = (target, self.cfg.dim) {
                self.hole = Some(Spring {
                    from: t,
                    to: t,
                    start: Instant::now(),
                });
            }
            if matches!(
                self.backend,
                HighlightBackend::OverlayActive | HighlightBackend::Glow
            ) {
                self.ring_rect = self.ring_target(target).map(|t| Spring {
                    from: t,
                    to: t,
                    start: Instant::now(),
                });
            }
            if target.is_none() && self.cfg.dim {
                // Minimised, cloaked or gone: fade the dim out.
                self.on_focus(self.fg);
                return;
            }
            self.frame();
        } else if self.backend == HighlightBackend::OverlayAll {
            self.refresh_all = true;
            self.start_timer();
        }
    }

    /// Where the highlight goes for a focused frame; `None` hides it (full screen).
    fn ring_target(&self, target: Option<PxRect>) -> Option<PxRect> {
        let t = target?;
        let mon = monitor_of(&self.mons, t)?;
        if is_fullscreen(t, mon.rect) {
            return None;
        }
        let width = scaled(self.cfg.highlight.width_px, mon.scale);
        let by = if self.backend == HighlightBackend::Glow {
            width * 4
        } else {
            width
        };
        Some(outset(t, by))
    }

    // ---- drawing

    /// Draw one frame; keeps the timer running only while a spring is moving.
    fn frame(&mut self) {
        let mut moving = false;
        if self.cfg.dim {
            moving |= !done(&self.dim_alpha);
            if let Some(s) = &self.hole {
                moving |= !done(s);
            }
            self.draw_dim();
        }
        match self.backend {
            HighlightBackend::OverlayActive => {
                if let Some(s) = &self.ring_rect {
                    moving |= !done(s);
                }
                self.draw_ring();
            }
            HighlightBackend::Glow => {
                if let Some(s) = &self.ring_rect {
                    moving |= !done(s);
                }
                self.draw_glow();
            }
            HighlightBackend::OverlayAll | HighlightBackend::DwmAll if self.refresh_all => {
                self.refresh_all = false;
                self.draw_all();
            }
            _ => {}
        }
        if moving || self.refresh_all {
            self.start_timer();
        } else {
            self.stop_timer();
        }
    }

    fn draw_dim(&mut self) {
        let alpha = alpha_byte(self.dim_alpha.now());
        let hole = self.hole.map(|s| s.now());
        for d in &mut self.dims {
            if alpha == 0 {
                hide(d.hwnd);
                d.shown_hole = None;
                continue;
            }
            let local = dim_hole(d.mon.rect, hole);
            if d.shown_hole != Some(local) {
                let radius = scaled(self.cfg.corner_radius_px, d.mon.scale);
                set_region(
                    d.hwnd,
                    region_with_hole(d.mon.rect.w, d.mon.rect.h, local, radius),
                );
                d.shown_hole = Some(local);
            }
            set_fill(d.hwnd, COLORREF(0), alpha);
            place(d.hwnd, d.mon.rect, HWND_TOPMOST);
        }
    }

    fn draw_ring(&mut self) {
        let Some(ring) = self.ring else { return };
        let Some(r) = self.ring_rect.map(|s| s.now()) else {
            hide(ring);
            return;
        };
        let mon = monitor_of(&self.mons, r).map_or(1.0, |m| m.scale);
        let width = scaled(self.cfg.highlight.width_px, mon);
        let inner = window_corner_radius(self.build, mon);
        if self.ring_shape != Some((r.w, r.h)) {
            set_region(ring, ring_region(r.w, r.h, width, inner));
            self.ring_shape = Some((r.w, r.h));
        }
        place(ring, r, HWND_TOPMOST);
    }

    fn draw_glow(&mut self) {
        let Some(glow) = self.glow else { return };
        let Some(r) = self.ring_rect.map(|s| s.now()) else {
            hide(glow);
            return;
        };
        // Render at the target size once; the spring only moves it.
        let target = self.ring_rect.map_or(r, |s| s.to);
        let mon = monitor_of(&self.mons, target).map_or(1.0, |m| m.scale);
        let reach = scaled(self.cfg.highlight.width_px, mon) * 4;
        let inner = window_corner_radius(self.build, mon);
        let at = PxRect::new(r.x, r.y, target.w, target.h);
        if self.glow_size != Some((target.w, target.h)) {
            paint_glow(glow, at, reach, inner, rgb(&self.cfg));
            self.glow_size = Some((target.w, target.h));
        }
        place(glow, at, HWND_TOPMOST);
    }

    /// "All borders": colour (DWM) or ring (overlay) every arrangeable window.
    fn draw_all(&mut self) {
        let colour = colorref(rgb(&self.cfg));
        let windows: Vec<HWND> = top_level_windows()
            .into_iter()
            .filter(|&h| arrangeable(h, false))
            .take(MAX_RINGS)
            .collect();
        if self.backend == HighlightBackend::DwmAll {
            for h in &windows {
                let key = h.0 as isize;
                if !self.dwm_coloured.contains(&key) {
                    set_dwm_border(*h, colour.0);
                    self.dwm_coloured.push(key);
                }
            }
            self.dwm_coloured.retain(|&k| {
                // SAFETY: plain query.
                unsafe { IsWindow(HWND(k as *mut c_void)) }.as_bool()
            });
            return;
        }
        // Overlay rings: one per window, just above it in the z-order.
        let keep: Vec<isize> = windows.iter().map(|h| h.0 as isize).collect();
        let stale: Vec<isize> = self
            .rings
            .keys()
            .filter(|k| !keep.contains(k))
            .copied()
            .collect();
        for k in stale {
            if let Some(h) = self.rings.remove(&k) {
                destroy(h);
            }
        }
        for target in windows {
            let Some(f) = frame(target) else { continue };
            let Some(mon) = monitor_of(&self.mons, f) else {
                continue;
            };
            let key = target.0 as isize;
            let ring = match self.rings.get(&key) {
                Some(&r) => r,
                None => match overlay_window(false) {
                    Some(r) => {
                        self.rings.insert(key, r);
                        r
                    }
                    None => continue,
                },
            };
            if is_fullscreen(f, mon.rect) {
                hide(ring);
                continue;
            }
            let width = scaled(self.cfg.highlight.width_px, mon.scale);
            let r = outset(f, width);
            set_region(
                ring,
                ring_region(r.w, r.h, width, window_corner_radius(self.build, mon.scale)),
            );
            let alpha = if Some(target) == self.fg {
                255
            } else {
                UNFOCUSED_RING_ALPHA
            };
            set_fill(ring, colour, alpha);
            // SAFETY: plain z-order query.
            let above = unsafe { GetWindow(target, GW_HWNDPREV) }
                .ok()
                .filter(|p| !p.0.is_null())
                .unwrap_or(HWND_TOP);
            place(ring, r, above);
        }
    }

    fn apply_config(&mut self, cfg: VisualsConfig) {
        self.teardown();
        self.backend = highlight_backend(&cfg.highlight, self.build);
        self.cfg = cfg;
        self.mons = monitors();
        self.build_windows();
        self.refresh_all = true;
        self.dim_alpha = Spring {
            from: 0.0,
            to: 0.0,
            start: Instant::now(),
        };
        // SAFETY: plain query.
        let fg = unsafe { GetForegroundWindow() };
        self.on_focus((!fg.0.is_null()).then_some(fg));
    }
}

/// Paint the glow bitmap (premultiplied ARGB) into a per-pixel-alpha window.
fn paint_glow(h: HWND, at: PxRect, reach: i32, inner: i32, (r, g, b): (u8, u8, u8)) {
    let (w, ht) = (at.w.max(1), at.h.max(1));
    let Ok(count) = usize::try_from(i64::from(w) * i64::from(ht)) else {
        return;
    };
    let mut pixels = vec![0u32; count];
    let reach_f = f64::from(reach);
    let (iw, ih) = (f64::from(w - 2 * reach), f64::from(ht - 2 * reach));
    let radius = f64::from(inner);
    // Only the band within `reach` of the edge can be non-zero.
    for y in 0..ht {
        let in_band_y = y < 2 * reach || y >= ht - 2 * reach;
        for x in 0..w {
            if !in_band_y && x >= 2 * reach && x < w - 2 * reach {
                continue;
            }
            let d = rounded_rect_distance(
                f64::from(x - reach) + 0.5,
                f64::from(y - reach) + 0.5,
                iw,
                ih,
                radius,
            );
            if d <= 0.0 {
                continue;
            }
            let a = glow_alpha(d, reach_f);
            if a == 0 {
                continue;
            }
            let pm = |c: u8| (u32::from(c) * u32::from(a) / 255) & 0xff;
            pixels[(y * w + x) as usize] =
                (u32::from(a) << 24) | (pm(r) << 16) | (pm(g) << 8) | pm(b);
        }
    }
    // SAFETY: standard DIB section + UpdateLayeredWindow, like win_overlay::paint;
    // the DIB holds exactly w * ht pixels, which is pixels.len().
    unsafe {
        let screen = GetDC(HWND::default());
        let mem = CreateCompatibleDC(screen);
        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = w;
        bmi.bmiHeader.biHeight = -ht;
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
                std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits.cast::<u32>(), pixels.len());
                let old = SelectObject(mem, bitmap);
                let dst = POINT { x: at.x, y: at.y };
                let size = SIZE { cx: w, cy: ht };
                let src = POINT { x: 0, y: 0 };
                let blend = BLENDFUNCTION {
                    BlendOp: 0,
                    BlendFlags: 0,
                    SourceConstantAlpha: 255,
                    AlphaFormat: 1,
                };
                let _ = UpdateLayeredWindow(
                    h,
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

// ------------------------------------------------------------ the thread

unsafe extern "system" fn event_proc(
    _hook: HWINEVENTHOOK,
    event: u32,
    h: HWND,
    id_object: i32,
    id_child: i32,
    _thread: u32,
    _time: u32,
) {
    if id_object != OBJID_WINDOW.0 || id_child != 0 || h.0.is_null() {
        return;
    }
    // SAFETY: posting to our own queue (out-of-context callbacks run on the
    // installing thread).
    let _ = unsafe {
        PostThreadMessageW(
            GetCurrentThreadId(),
            WM_EVENT,
            WPARAM(h.0 as usize),
            LPARAM(event as isize),
        )
    };
}

fn install(min: u32, max: u32) -> Option<HWINEVENTHOOK> {
    // Krova's own windows count as focus targets (its overlays never take focus),
    // so its own process is not skipped here.
    // SAFETY: out-of-context hook, removed on this thread before it ends.
    let hook = unsafe {
        SetWinEventHook(
            min,
            max,
            HMODULE::default(),
            Some(event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        )
    };
    (!hook.is_invalid()).then_some(hook)
}

fn run(shared: &SharedConfig, ready: &mpsc::Sender<Result<u32, String>>) {
    // SAFETY: affects only this thread: every rect it sees is in physical pixels.
    let _ = unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    let mut msg = MSG::default();
    // SAFETY: creates this thread's queue before anything posts to it.
    let _ = unsafe { PeekMessageW(&mut msg, HWND::default(), 0, 0, PM_NOREMOVE) };
    let Some(cfg) = shared.lock().ok().map(|c| c.clone()) else {
        let _ = ready.send(Err("config unavailable".into()));
        return;
    };
    // A hidden top-level window so display changes reach this thread.
    let Some(control) = overlay_window(false) else {
        let _ = ready.send(Err("could not create a window".into()));
        return;
    };
    let hooks: Vec<HWINEVENTHOOK> = [
        (EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND),
        (EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZEEND),
        (EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE),
        (EVENT_OBJECT_CLOAKED, EVENT_OBJECT_UNCLOAKED),
    ]
    .into_iter()
    .filter_map(|(a, b)| install(a, b))
    .collect();

    // SAFETY: plain query.
    let _ = ready.send(Ok(unsafe { GetCurrentThreadId() }));

    let mut state = State::new(cfg.clone());
    state.apply_config(cfg);

    // SAFETY: standard message loop on this thread's own queue.
    while unsafe { GetMessageW(&mut msg, HWND::default(), 0, 0) }.0 > 0 {
        match msg.message {
            WM_EVENT => {
                let h = HWND(msg.wParam.0 as *mut c_void);
                let event = msg.lParam.0 as u32;
                match event {
                    EVENT_SYSTEM_FOREGROUND => state.on_focus(Some(h)),
                    EVENT_OBJECT_LOCATIONCHANGE => state.on_moved(h),
                    _ => {
                        // Minimise, show/hide, destroy, cloak.
                        if Some(h) == state.fg {
                            // SAFETY: plain queries.
                            let gone = !unsafe { IsWindow(h) }.as_bool()
                                || unsafe { IsIconic(h) }.as_bool()
                                || is_cloaked(h);
                            if gone || event == EVENT_SYSTEM_MINIMIZEEND {
                                state.on_focus(state.fg);
                            }
                        }
                        if matches!(
                            state.backend,
                            HighlightBackend::DwmAll | HighlightBackend::OverlayAll
                        ) {
                            state.refresh_all = true;
                            state.start_timer();
                        }
                    }
                }
            }
            WM_TIMER if msg.hwnd.0.is_null() => state.frame(),
            WM_CONFIG => {
                if let Some(cfg) = shared.lock().ok().map(|c| c.clone()) {
                    state.apply_config(cfg);
                }
            }
            WM_REBUILD => {
                // Collapse a burst of display messages into one rebuild.
                let cfg = state.cfg.clone();
                state.apply_config(cfg);
            }
            _ => {
                // SAFETY: dispatching a message this thread received.
                unsafe {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }
    }

    state.teardown();
    // SAFETY: removing what this thread installed and created.
    unsafe {
        for h in hooks {
            let _ = UnhookWinEvent(h);
        }
    }
    destroy(control);
}

#[cfg(test)]
mod tests {
    use super::*;
    use krova_core::focus::{FocusSettings, HighlightMode, HighlightSettings};

    fn config(dim: bool, mode: HighlightMode, width_px: u32) -> VisualsConfig {
        VisualsConfig::from_settings(
            &krova_core::Settings::default(),
            &FocusSettings {
                enabled: dim,
                opacity: 0.45,
            },
            &HighlightSettings {
                mode,
                color: None,
                width_px,
            },
        )
    }

    #[test]
    fn colours_are_bgr() {
        assert_eq!(colorref((0x11, 0x22, 0x33)).0, 0x0033_2211);
    }

    #[test]
    #[ignore = "creates real overlay windows; needs an interactive desktop session"]
    fn visuals_start_switch_modes_and_stop() {
        let v = WinVisuals::start(config(true, HighlightMode::None, 2)).expect("overlays start");
        thread::sleep(Duration::from_millis(400));
        for (mode, width) in [
            (HighlightMode::Active, 2),
            (HighlightMode::Active, 5),
            (HighlightMode::All, 1),
            (HighlightMode::All, 4),
            (HighlightMode::Glow, 3),
        ] {
            v.set_config(config(true, mode, width));
            thread::sleep(Duration::from_millis(400));
        }
        drop(v);
    }
}
