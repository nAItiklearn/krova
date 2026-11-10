//! Windows platform layer.
//!
//! STATUS: written without a compiler available on the authoring machine (the
//! shell tool was down), against `windows` 0.58. The first real build is CI on
//! windows-latest; treat this file as unverified until that run is green.
//!
//! Design (docs/PORTING-MAP.md, ADR-001):
//! - A `WH_MOUSE_LL` hook on its own thread only forwards left-button events into
//!   a channel (the hook must return in well under a millisecond).
//! - A worker thread turns those into `DragEvent`s: it finds the window under the
//!   cursor, samples its rect, polls the modifier / Escape keys while a drag is
//!   live, and sends `InputEvent`s to the app.
//! - A drag is "the window's origin moved while its size stayed the same", so
//!   resizes and in-window drags never look like window drags.

use std::ffi::c_void;
use std::mem::size_of;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use krova_core::window::ANIMATION_MS;
use krova_core::{
    animate_step, clamp_to_limits, DragEvent, FrameInsets, Modifier, MonitorDescriptor, PxRect,
    SizeLimits,
};
use windows::core::{HRESULT, PCWSTR};
use windows::Win32::Foundation::{
    CloseHandle, BOOL, ERROR_ACCESS_DENIED, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT,
    TRUE, WPARAM,
};
use windows::Win32::Graphics::Dwm::{
    DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS,
};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayDevicesW, EnumDisplayMonitors, GetMonitorInfoW, DISPLAY_DEVICEW, HDC, HMONITOR,
    MONITORINFO, MONITORINFOEXW,
};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentProcessId, GetCurrentThreadId, OpenProcess, OpenProcessToken,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VIRTUAL_KEY, VK_CONTROL, VK_ESCAPE, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetAncestor, GetClassNameW, GetCursorPos,
    GetForegroundWindow, GetMessageW, GetWindowLongW, GetWindowRect, GetWindowThreadProcessId,
    IsIconic, IsWindow, IsWindowVisible, IsZoomed, PostThreadMessageW, SetWindowPos,
    SetWindowsHookExW, ShowWindow, TranslateMessage, UnhookWindowsHookEx, WindowFromPoint, GA_ROOT,
    GWL_STYLE, HHOOK, MSG, MSLLHOOKSTRUCT, SWP_NOACTIVATE, SWP_NOZORDER, SW_RESTORE, WH_MOUSE_LL,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_QUIT, WS_CAPTION,
};
use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

use crate::ids::monitor_hardware_id;
use crate::input::{coalesce_moves, ChangeDetector, RawPointer};
use crate::{
    autostart_command, modifier_code, modifier_from_code, Capabilities, EventSink, HotkeyEvent,
    HotkeyHandle, HotkeySpec, InputEvent, InputHandle, MonitorInfo, Permission, Platform,
    PlatformError, WatchEvent, WatchHandle, WindowId,
};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "Krova";
const MONITORINFOF_PRIMARY: u32 = 1;
const EDD_GET_DEVICE_INTERFACE_NAME: u32 = 1;
const ERROR_INVALID_WINDOW_HANDLE: u32 = 1400;

#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsPlatform;

impl WindowsPlatform {
    pub fn new() -> Self {
        Self
    }
}

// ---------------------------------------------------------------- helpers

fn os_err(e: windows::core::Error) -> PlatformError {
    PlatformError::Os(e.to_string())
}

fn io_err(e: std::io::Error) -> PlatformError {
    PlatformError::Os(e.to_string())
}

pub(crate) fn hwnd(id: WindowId) -> HWND {
    HWND(id.0 as usize as *mut c_void)
}

pub(crate) fn window_id(h: HWND) -> WindowId {
    WindowId(h.0 as usize as u64)
}

fn px(r: RECT) -> PxRect {
    PxRect::new(r.left, r.top, r.right - r.left, r.bottom - r.top)
}

fn wide_to_string(w: &[u16]) -> String {
    let len = w.iter().position(|&c| c == 0).unwrap_or(w.len());
    String::from_utf16_lossy(&w[..len])
}

/// Outer window rect (`GetWindowRect`), including the invisible resize borders.
fn outer_rect(h: HWND) -> Result<PxRect, PlatformError> {
    let mut r = RECT::default();
    unsafe { GetWindowRect(h, &mut r) }.map_err(os_err)?;
    Ok(px(r))
}

/// The frame the user actually sees (DWM extended frame bounds).
pub(crate) fn visible_rect(h: HWND) -> Option<PxRect> {
    let mut r = RECT::default();
    unsafe {
        DwmGetWindowAttribute(
            h,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&mut r as *mut RECT).cast::<c_void>(),
            size_of::<RECT>() as u32,
        )
    }
    .ok()?;
    Some(px(r))
}

/// Windows on other virtual desktops are cloaked.
fn is_cloaked(h: HWND) -> bool {
    let mut cloaked = 0u32;
    let ok = unsafe {
        DwmGetWindowAttribute(
            h,
            DWMWA_CLOAKED,
            (&mut cloaked as *mut u32).cast::<c_void>(),
            size_of::<u32>() as u32,
        )
    }
    .is_ok();
    ok && cloaked != 0
}

pub(crate) fn class_name(h: HWND) -> String {
    let mut buf = [0u16; 128];
    let n = unsafe { GetClassNameW(h, &mut buf) };
    String::from_utf16_lossy(&buf[..usize::try_from(n).unwrap_or(0)])
}

/// A normal, movable, captioned top-level window on the current desktop.
/// Borderless full-screen windows have no caption and are therefore left alone.
pub(crate) fn snappable(h: HWND) -> bool {
    let usable =
        unsafe { IsWindow(h).as_bool() && IsWindowVisible(h).as_bool() && !IsIconic(h).as_bool() };
    if !usable || is_cloaked(h) {
        return false;
    }
    if matches!(
        class_name(h).as_str(),
        "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
    ) {
        return false;
    }
    let style = unsafe { GetWindowLongW(h, GWL_STYLE) } as u32;
    (style & WS_CAPTION.0) == WS_CAPTION.0
}

fn process_elevated(process: HANDLE) -> Option<bool> {
    let mut token = HANDLE::default();
    unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) }.ok()?;
    let mut info = TOKEN_ELEVATION::default();
    let mut returned = 0u32;
    let queried = unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            Some((&mut info as *mut TOKEN_ELEVATION).cast::<c_void>()),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        )
    };
    let _ = unsafe { CloseHandle(token) };
    queried.ok()?;
    Some(info.TokenIsElevated != 0)
}

fn self_elevated() -> bool {
    static CACHE: OnceLock<bool> = OnceLock::new();
    *CACHE.get_or_init(|| process_elevated(unsafe { GetCurrentProcess() }).unwrap_or(false))
}

/// UIPI stops a normal process from moving an elevated process's windows, so
/// those are skipped up front (documented limitation). If the owning process
/// cannot be inspected we assume it is protected.
pub(crate) fn target_is_protected(h: HWND) -> bool {
    if self_elevated() {
        return false;
    }
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(h, Some(&mut pid)) };
    if pid == 0 {
        return true;
    }
    let Ok(process) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) })
    else {
        return true;
    };
    let elevated = process_elevated(process).unwrap_or(true);
    let _ = unsafe { CloseHandle(process) };
    elevated
}

/// Is this one of Krova's own windows (the settings window, the overlay)?
pub(crate) fn is_own_process_window(h: HWND) -> bool {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(h, Some(&mut pid)) };
    pid != 0 && pid == unsafe { GetCurrentProcessId() }
}

/// Top-level window under a screen point.
pub(crate) fn window_at(x: i32, y: i32) -> Option<HWND> {
    let hit = unsafe { WindowFromPoint(POINT { x, y }) };
    if hit.0.is_null() {
        return None;
    }
    let root = unsafe { GetAncestor(hit, GA_ROOT) };
    if root.0.is_null() {
        None
    } else {
        Some(root)
    }
}

pub(crate) fn key_down(vk: VIRTUAL_KEY) -> bool {
    // The high bit of GetAsyncKeyState is set while the key is held.
    (unsafe { GetAsyncKeyState(i32::from(vk.0)) }) < 0
}

pub(crate) fn modifier_down(m: Modifier) -> bool {
    match m {
        Modifier::Shift => key_down(VK_SHIFT),
        Modifier::Ctrl => key_down(VK_CONTROL),
        Modifier::Alt => key_down(VK_MENU),
        Modifier::Super => key_down(VK_LWIN) || key_down(VK_RWIN),
    }
}

// --------------------------------------------------------------- monitors

unsafe extern "system" fn collect_monitor(h: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
    // SAFETY: `data` is the address of the Vec that `monitors()` keeps alive for
    // the whole enumeration.
    let list = unsafe { &mut *(data.0 as *mut Vec<HMONITOR>) };
    list.push(h);
    TRUE
}

fn monitor_info(h: HMONITOR, index: usize) -> Option<MonitorInfo> {
    // SAFETY: MONITORINFOEXW is plain data; all-zero is a valid value.
    let mut mi: MONITORINFOEXW = unsafe { std::mem::zeroed() };
    mi.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
    let ok = unsafe { GetMonitorInfoW(h, (&mut mi as *mut MONITORINFOEXW).cast::<MONITORINFO>()) };
    if !ok.as_bool() {
        return None;
    }

    let (mut dpi_x, mut dpi_y) = (96u32, 96u32);
    let scale = if unsafe { GetDpiForMonitor(h, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) }.is_ok()
    {
        f64::from(dpi_x) / 96.0
    } else {
        1.0
    };

    // The monitor attached to this display adapter output; its device interface
    // name embeds the PnP id and the per-port instance path.
    // SAFETY: DISPLAY_DEVICEW is plain data; all-zero is a valid value.
    let mut dd: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
    dd.cb = size_of::<DISPLAY_DEVICEW>() as u32;
    let found = unsafe {
        EnumDisplayDevicesW(
            PCWSTR(mi.szDevice.as_ptr()),
            0,
            &mut dd,
            EDD_GET_DEVICE_INTERFACE_NAME,
        )
    }
    .as_bool();
    let interface = found.then(|| wide_to_string(&dd.DeviceID));

    Some(MonitorInfo {
        descriptor: MonitorDescriptor {
            hardware_id: interface.as_deref().and_then(monitor_hardware_id),
            device_path: interface.filter(|s| !s.is_empty()),
            rect: px(mi.monitorInfo.rcMonitor),
        },
        work: px(mi.monitorInfo.rcWork),
        scale,
        label: format!("Display {}", index + 1),
        primary: mi.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
    })
}

// ------------------------------------------------------------- placement

fn set_rect(h: HWND, r: PxRect) -> Result<(), PlatformError> {
    unsafe {
        SetWindowPos(
            h,
            HWND::default(),
            r.x,
            r.y,
            r.w,
            r.h,
            SWP_NOZORDER | SWP_NOACTIVATE,
        )
    }
    .map_err(|e| {
        let code = e.code();
        if code == HRESULT::from_win32(ERROR_ACCESS_DENIED.0) {
            PlatformError::Refused("the window belongs to an elevated process")
        } else if code == HRESULT::from_win32(ERROR_INVALID_WINDOW_HANDLE) {
            PlatformError::WindowGone
        } else {
            os_err(e)
        }
    })
}

fn animate_move(h: HWND, from: PxRect, to: PxRect) {
    let start = Instant::now();
    loop {
        let elapsed = start.elapsed().as_millis().min(u128::from(ANIMATION_MS)) as u32;
        if elapsed >= ANIMATION_MS {
            break;
        }
        if set_rect(h, animate_step(from, to, elapsed)).is_err() {
            break;
        }
        thread::sleep(Duration::from_millis(8));
    }
}

// ----------------------------------------------------------------- input

static HOOK_TX: Mutex<Option<Sender<RawPointer>>> = Mutex::new(None);
static LEFT_DOWN: AtomicBool = AtomicBool::new(false);

/// Low-level mouse hook. Must stay tiny: forward the event and return.
unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        // SAFETY: for WH_MOUSE_LL, lparam points to a valid MSLLHOOKSTRUCT for
        // the duration of the call.
        let pt = unsafe { (*(lparam.0 as *const MSLLHOOKSTRUCT)).pt };
        let ev = match wparam.0 as u32 {
            WM_LBUTTONDOWN => {
                LEFT_DOWN.store(true, Ordering::Relaxed);
                Some(RawPointer::Down(pt.x, pt.y))
            }
            WM_LBUTTONUP => {
                LEFT_DOWN.store(false, Ordering::Relaxed);
                Some(RawPointer::Up)
            }
            WM_MOUSEMOVE if LEFT_DOWN.load(Ordering::Relaxed) => Some(RawPointer::Move(pt.x, pt.y)),
            _ => None,
        };
        if let Some(ev) = ev {
            if let Ok(slot) = HOOK_TX.try_lock() {
                if let Some(tx) = slot.as_ref() {
                    let _ = tx.send(ev);
                }
            }
        }
        // --- wave4 ---
        // The resting-cursor watcher (focus follows mouse, hover picker); a no-op
        // unless it runs.
        crate::win_hover::on_hook(wparam.0 as u32, pt.x, pt.y);
        // --- end wave4 ---
    }
    unsafe { CallNextHookEx(HHOOK::default(), code, wparam, lparam) }
}

fn clear_hook_slot() {
    if let Ok(mut slot) = HOOK_TX.lock() {
        *slot = None;
    }
    LEFT_DOWN.store(false, Ordering::Relaxed);
}

fn quit_hook_thread(thread_id: u32) {
    let _ = unsafe { PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
}

/// Install the hook on a dedicated thread with its own message loop.
/// Returns that thread's id (to post `WM_QUIT`) and its join handle.
fn spawn_hook_thread(tx: Sender<RawPointer>) -> Result<(u32, JoinHandle<()>), PlatformError> {
    {
        let mut slot = HOOK_TX
            .lock()
            .map_err(|_| PlatformError::Os("input state poisoned".into()))?;
        if slot.is_some() {
            return Err(PlatformError::Os("input monitor is already running".into()));
        }
        *slot = Some(tx);
    }

    let (ready_tx, ready_rx) = mpsc::channel::<Result<u32, String>>();
    let spawned = thread::Builder::new()
        .name("krova-hook".into())
        .spawn(move || {
            let hook = (|| -> windows::core::Result<HHOOK> {
                unsafe {
                    let module = GetModuleHandleW(PCWSTR::null())?;
                    SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), HINSTANCE(module.0), 0)
                }
            })();
            match hook {
                Err(e) => {
                    let _ = ready_tx.send(Err(e.to_string()));
                }
                Ok(hook) => {
                    let _ = ready_tx.send(Ok(unsafe { GetCurrentThreadId() }));
                    let mut msg = MSG::default();
                    while unsafe { GetMessageW(&mut msg, HWND::default(), 0, 0) }.0 > 0 {
                        unsafe {
                            let _ = TranslateMessage(&msg);
                            DispatchMessageW(&msg);
                        }
                    }
                    let _ = unsafe { UnhookWindowsHookEx(hook) };
                }
            }
            // Dropping the sender lets the worker thread notice and exit.
            clear_hook_slot();
        });

    let join = match spawned {
        Ok(j) => j,
        Err(e) => {
            clear_hook_slot();
            return Err(io_err(e));
        }
    };
    match ready_rx.recv_timeout(Duration::from_secs(2)) {
        Ok(Ok(thread_id)) => Ok((thread_id, join)),
        Ok(Err(msg)) => {
            let _ = join.join();
            Err(PlatformError::Os(format!(
                "could not install mouse hook: {msg}"
            )))
        }
        Err(_) => {
            clear_hook_slot();
            Err(PlatformError::Os("mouse hook thread did not start".into()))
        }
    }
}

/// The left button went down on a window we may move.
struct Press {
    hwnd: HWND,
    id: WindowId,
    cursor: (i32, i32),
    /// Maximized windows change size when a drag starts (they restore), so
    /// their `Press` is deferred until the restored rect is known.
    was_zoomed: bool,
    moved: bool,
}

fn run_worker(
    rx: Receiver<RawPointer>,
    sink: Sender<InputEvent>,
    modifier: Arc<AtomicU8>,
    stop: Arc<AtomicBool>,
) {
    let current_modifier = || modifier_down(modifier_from_code(modifier.load(Ordering::Relaxed)));
    let emit = |event: DragEvent, id: WindowId| {
        let _ = sink.send(InputEvent {
            event,
            window: Some(id),
        });
    };
    let mut press: Option<Press> = None;
    let mut modifier_watch = ChangeDetector::default();
    let mut escape_watch = ChangeDetector::default();

    while !stop.load(Ordering::Relaxed) {
        let mut batch = Vec::new();
        match rx.recv_timeout(Duration::from_millis(15)) {
            Ok(ev) => batch.push(ev),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        while let Ok(ev) = rx.try_recv() {
            batch.push(ev);
        }
        coalesce_moves(&mut batch);

        for ev in batch {
            match ev {
                RawPointer::Down(x, y) => {
                    press = None;
                    let Some(h) = window_at(x, y) else { continue };
                    if !snappable(h) || target_is_protected(h) {
                        continue;
                    }
                    let Ok(rect) = outer_rect(h) else { continue };
                    let id = window_id(h);
                    let was_zoomed = unsafe { IsZoomed(h) }.as_bool();
                    modifier_watch.reset(current_modifier());
                    escape_watch.reset(key_down(VK_ESCAPE));
                    if !was_zoomed {
                        emit(
                            DragEvent::Press {
                                cursor: (x, y),
                                window: rect,
                            },
                            id,
                        );
                    }
                    press = Some(Press {
                        hwnd: h,
                        id,
                        cursor: (x, y),
                        was_zoomed,
                        moved: false,
                    });
                }
                RawPointer::Move(x, y) => {
                    let Some(p) = press.as_mut() else { continue };
                    let Ok(rect) = outer_rect(p.hwnd) else {
                        press = None;
                        continue;
                    };
                    if p.was_zoomed {
                        if unsafe { IsZoomed(p.hwnd) }.as_bool() {
                            continue;
                        }
                        // The window just left the maximized state: its restored
                        // rect is the baseline for this drag.
                        emit(
                            DragEvent::Press {
                                cursor: p.cursor,
                                window: rect,
                            },
                            p.id,
                        );
                        p.was_zoomed = false;
                    }
                    p.moved = true;
                    emit(
                        DragEvent::Move {
                            cursor: (x, y),
                            window: rect,
                            modifier_down: current_modifier(),
                        },
                        p.id,
                    );
                }
                RawPointer::Up => {
                    if let Some(p) = press.take() {
                        if p.moved {
                            // Let the OS finish its own move loop before the app
                            // repositions the window.
                            thread::sleep(Duration::from_millis(30));
                        }
                        if !p.was_zoomed {
                            emit(DragEvent::Release, p.id);
                        }
                    }
                }
            }
        }

        // Keys do not produce mouse events, so poll them while a drag is live.
        let live = press.as_ref().filter(|p| p.moved).map(|p| p.id);
        if let Some(id) = live {
            if let Some(down) = modifier_watch.update(current_modifier()) {
                emit(DragEvent::Modifier { down }, id);
            }
            if escape_watch.update(key_down(VK_ESCAPE)) == Some(true) {
                emit(DragEvent::Cancel, id);
                press = None;
            }
        }
    }
}

// ------------------------------------------------------------- Platform

impl Platform for WindowsPlatform {
    fn monitors(&self) -> Result<Vec<MonitorInfo>, PlatformError> {
        let mut handles: Vec<HMONITOR> = Vec::new();
        let ok = unsafe {
            EnumDisplayMonitors(
                HDC::default(),
                None,
                Some(collect_monitor),
                LPARAM(&mut handles as *mut Vec<HMONITOR> as isize),
            )
        };
        if !ok.as_bool() {
            return Err(PlatformError::Os("EnumDisplayMonitors failed".into()));
        }
        Ok(handles
            .into_iter()
            .enumerate()
            .filter_map(|(i, h)| monitor_info(h, i))
            .collect())
    }

    fn place_window(
        &self,
        window: WindowId,
        target: PxRect,
        work: PxRect,
        animate: bool,
    ) -> Result<PxRect, PlatformError> {
        let h = hwnd(window);
        if !unsafe { IsWindow(h) }.as_bool() {
            return Err(PlatformError::WindowGone);
        }
        if unsafe { IsZoomed(h) }.as_bool() {
            let _ = unsafe { ShowWindow(h, SW_RESTORE) };
        }

        let outer = outer_rect(h)?;
        let mut insets = visible_rect(h)
            .map(|v| FrameInsets::between(outer, v))
            .unwrap_or_default();
        if animate {
            animate_move(h, outer, insets.outer_for_visible(target));
        }
        set_rect(h, insets.outer_for_visible(target))?;

        // The invisible border scales with DPI, so it can differ after the window
        // crossed to another monitor: measure again and correct once.
        let now = outer_rect(h)?;
        if let Some(v) = visible_rect(h) {
            let measured = FrameInsets::between(now, v);
            if measured != insets {
                insets = measured;
                set_rect(h, insets.outer_for_visible(target))?;
            }
        }

        // If the window refused the size (min/max limits), infer the limits from
        // what it did and place it again: centred in the zone, kept on screen.
        let want = insets.outer_for_visible(target);
        let got = outer_rect(h)?;
        if (got.w, got.h) != (want.w, want.h) {
            let limits = SizeLimits {
                min_w: (got.w > want.w).then_some(got.w),
                max_w: (got.w < want.w).then_some(got.w),
                min_h: (got.h > want.h).then_some(got.h),
                max_h: (got.h < want.h).then_some(got.h),
            };
            let bounds = insets.outer_for_visible(work);
            set_rect(h, clamp_to_limits(want, limits, bounds))?;
        }

        let done = outer_rect(h)?;
        Ok(visible_rect(h).unwrap_or(done))
    }

    fn start_input(
        &self,
        modifier: Modifier,
        sink: Sender<InputEvent>,
    ) -> Result<InputHandle, PlatformError> {
        let (tx, rx) = mpsc::channel::<RawPointer>();
        let (hook_thread_id, hook_join) = spawn_hook_thread(tx)?;

        let modifier_cell = Arc::new(AtomicU8::new(modifier_code(modifier)));
        let stop = Arc::new(AtomicBool::new(false));
        let worker = {
            let modifier_cell = Arc::clone(&modifier_cell);
            let stop = Arc::clone(&stop);
            thread::Builder::new()
                .name("krova-input".into())
                .spawn(move || run_worker(rx, sink, modifier_cell, stop))
        };
        let worker_join = match worker {
            Ok(j) => j,
            Err(e) => {
                quit_hook_thread(hook_thread_id);
                let _ = hook_join.join();
                return Err(io_err(e));
            }
        };

        Ok(InputHandle::new(modifier_cell, move || {
            stop.store(true, Ordering::Relaxed);
            quit_hook_thread(hook_thread_id);
            let _ = hook_join.join();
            let _ = worker_join.join();
        }))
    }

    fn autostart_enabled(&self) -> Result<bool, PlatformError> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        match hkcu.open_subkey(RUN_KEY) {
            Ok(key) => Ok(key.get_value::<String, _>(RUN_VALUE).is_ok()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(io_err(e)),
        }
    }

    fn set_autostart(&self, enabled: bool) -> Result<(), PlatformError> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hkcu.create_subkey(RUN_KEY).map_err(io_err)?;
        if enabled {
            let exe = std::env::current_exe().map_err(io_err)?;
            key.set_value(RUN_VALUE, &autostart_command(&exe))
                .map_err(io_err)
        } else {
            match key.delete_value(RUN_VALUE) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(io_err(e)),
            }
        }
    }

    fn create_overlay(&self) -> Result<Box<dyn crate::overlay::Overlay>, PlatformError> {
        Ok(Box::new(crate::win_overlay::WinOverlay::start()?))
    }

    fn permission(&self) -> Permission {
        Permission::NotRequired
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            shortcuts: true,
            window_memory: true,
        }
    }

    fn window_exists(&self, window: WindowId) -> bool {
        unsafe { IsWindow(hwnd(window)) }.as_bool()
    }

    fn window_rect(&self, window: WindowId) -> Result<PxRect, PlatformError> {
        let h = hwnd(window);
        if !unsafe { IsWindow(h) }.as_bool() {
            return Err(PlatformError::WindowGone);
        }
        match visible_rect(h) {
            Some(v) => Ok(v),
            None => outer_rect(h),
        }
    }

    fn is_minimized(&self, window: WindowId) -> bool {
        unsafe { IsIconic(hwnd(window)) }.as_bool()
    }

    fn is_maximized(&self, window: WindowId) -> bool {
        unsafe { IsZoomed(hwnd(window)) }.as_bool()
    }

    fn foreground_window(&self) -> Option<WindowId> {
        let h = unsafe { GetForegroundWindow() };
        if h.0.is_null() {
            None
        } else {
            Some(window_id(h))
        }
    }

    fn cursor_position(&self) -> Option<(i32, i32)> {
        let mut p = POINT::default();
        // SAFETY: `p` is a valid out-pointer; the call fails (rather than faults)
        // when there is no input desktop.
        unsafe { GetCursorPos(&mut p) }.ok()?;
        Some((p.x, p.y))
    }

    fn is_movable_target(&self, window: WindowId) -> bool {
        let h = hwnd(window);
        unsafe { IsWindow(h) }.as_bool()
            && snappable(h)
            && !is_own_process_window(h)
            && !target_is_protected(h)
    }

    fn start_hotkeys(
        &self,
        spec: HotkeySpec,
        sink: EventSink<HotkeyEvent>,
    ) -> Result<HotkeyHandle, PlatformError> {
        crate::win_events::start_hotkeys(spec, sink)
    }

    fn start_watcher(&self, sink: EventSink<WatchEvent>) -> Result<WatchHandle, PlatformError> {
        crate::win_events::start_watcher(sink)
    }

    // --- wave4 ---
    fn wave4_capabilities(&self) -> crate::pointer::Wave4Capabilities {
        crate::pointer::Wave4Capabilities {
            focus_follows_mouse: true,
            hover_picker: true,
            open_url: true,
        }
    }

    fn start_pointer_watch(
        &self,
        config: Arc<crate::pointer::PointerConfig>,
        sink: EventSink<crate::pointer::PointerEvent>,
    ) -> Result<WatchHandle, PlatformError> {
        crate::win_hover::start(config, sink)
    }

    fn open_url(&self, url: &str) -> Result<(), PlatformError> {
        crate::win_shell::open_url(url)
    }

    fn ui_language(&self) -> Option<u16> {
        crate::win_shell::ui_language()
    }
    // --- end wave4 ---
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_strings_stop_at_the_first_nul() {
        let w: Vec<u16> = "DISPLAY1\0garbage".encode_utf16().collect();
        assert_eq!(wide_to_string(&w), "DISPLAY1");
        assert_eq!(wide_to_string(&[]), "");
    }

    #[test]
    fn window_ids_round_trip() {
        let id = WindowId(0x1234_5678);
        assert_eq!(window_id(hwnd(id)), id);
    }

    #[test]
    fn the_cursor_is_on_one_of_the_monitors() {
        // Needs an input desktop: a headless session reports no cursor or no monitor.
        let p = WindowsPlatform::new();
        let (Some((x, y)), Ok(monitors)) = (p.cursor_position(), p.monitors()) else {
            return;
        };
        if monitors.is_empty() {
            return;
        }
        assert!(
            monitors.iter().any(|m| m.descriptor.rect.contains(x, y)),
            "cursor ({x}, {y}) is on no monitor"
        );
    }

    #[test]
    fn enumerated_monitors_have_real_geometry() {
        // A headless runner may report none; any monitor it does report must be sane.
        for m in WindowsPlatform::new()
            .monitors()
            .expect("enumeration works")
        {
            assert!(m.descriptor.rect.w > 0 && m.descriptor.rect.h > 0);
            assert!(m.work.w > 0 && m.work.h > 0);
            assert!(m.scale >= 1.0);
        }
    }
}
