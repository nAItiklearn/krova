//! Windows backend of the stack tab strips.
//!
//! One thread owns a hidden message-only window (the host) and one layered, topmost,
//! never-activating popup per strip. The strips are rendered by
//! [`strips::render`](crate::strips::render) into premultiplied pixels and handed to
//! `UpdateLayeredWindow`; pixels with zero alpha (the gaps between and around the
//! tabs) let clicks through to the window below, so only the tabs themselves take
//! the mouse. Text comes from GDI (Segoe UI, greyscale anti-aliasing) and icons from
//! the program file; both are drawn here, nothing else is read.
//!
//! Updates are posted to the host window (never to a thread id), coalesced, and a
//! strip is repainted only when its scene changed. Nothing runs while nothing
//! changes: the thread sleeps in `GetMessageW`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::mem::size_of;
use std::ptr::null_mut;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use krova_core::PxRect;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{
    COLORREF, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, CreateFontW, DeleteDC, DeleteObject, DrawTextW, GdiFlush,
    GetDC, ReleaseDC, SelectObject, SetBkMode, SetTextColor, ANTIALIASED_QUALITY, BITMAPINFO,
    BITMAPINFOHEADER, BLENDFUNCTION, CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DIB_RGB_COLORS,
    DT_CALCRECT, DT_END_ELLIPSIS, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, HBITMAP, HDC, HFONT,
    HGDIOBJ, OUT_DEFAULT_PRECIS, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::ExtractIconExW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyWindow, DispatchMessageW, DrawIconEx,
    GetMessageW, PostMessageW, PostQuitMessage, RegisterClassExW, SetWindowPos, TranslateMessage,
    UpdateLayeredWindow, DI_NORMAL, HICON, HMENU, HWND_MESSAGE, HWND_TOPMOST, MA_NOACTIVATE, MSG,
    SWP_NOACTIVATE, SWP_SHOWWINDOW, ULW_ALPHA, WM_APP, WM_CLOSE, WM_LBUTTONUP, WM_MOUSEACTIVATE,
    WM_MOUSEWHEEL, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_POPUP,
};

use crate::strips::{render, Glyphs, Icon, Mask, StripEvent, StripHost, StripLayout, StripScene};
use crate::{EventSink, PlatformError};

const WM_UPDATE: u32 = WM_APP + 1;
const WM_STOP: u32 = WM_APP + 2;
/// Most icons kept in memory (a few KiB each).
const ICON_CACHE: usize = 128;

type Scenes = Arc<Mutex<Vec<StripScene>>>;

/// The host handle as a plain number, so it can cross threads.
#[derive(Clone, Copy)]
struct HostHandle(usize);

impl HostHandle {
    fn hwnd(self) -> HWND {
        HWND(self.0 as *mut c_void)
    }
}

pub struct WinStrips {
    scenes: Scenes,
    host: HostHandle,
    join: Option<JoinHandle<()>>,
}

impl WinStrips {
    /// Start the strip thread. Clicks and wheel turns go to `events`.
    pub fn start(events: EventSink<StripEvent>) -> Result<Self, PlatformError> {
        let scenes: Scenes = Arc::new(Mutex::new(Vec::new()));
        let shared = Arc::clone(&scenes);
        let (ready_tx, ready_rx) = mpsc::channel::<Result<usize, String>>();
        let join = thread::Builder::new()
            .name("krova-strips".into())
            .spawn(move || run(&shared, events, &ready_tx))
            .map_err(|e| PlatformError::Os(e.to_string()))?;
        match ready_rx.recv_timeout(Duration::from_secs(2)) {
            Ok(Ok(host)) => Ok(Self {
                scenes,
                host: HostHandle(host),
                join: Some(join),
            }),
            Ok(Err(msg)) => {
                let _ = join.join();
                Err(PlatformError::Os(format!(
                    "could not create the tab strips: {msg}"
                )))
            }
            Err(_) => Err(PlatformError::Os(
                "the tab strip thread did not start".into(),
            )),
        }
    }
}

impl StripHost for WinStrips {
    fn set_strips(&self, strips: Vec<StripScene>) {
        if let Ok(mut slot) = self.scenes.lock() {
            *slot = strips;
        }
        // SAFETY: the host window lives until `drop` has joined its thread.
        let _ = unsafe { PostMessageW(self.host.hwnd(), WM_UPDATE, WPARAM(0), LPARAM(0)) };
    }
}

impl Drop for WinStrips {
    fn drop(&mut self) {
        // SAFETY: as above; the thread destroys its windows and ends its loop.
        let _ = unsafe { PostMessageW(self.host.hwnd(), WM_STOP, WPARAM(0), LPARAM(0)) };
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

// ------------------------------------------------------------------ thread state

struct Strip {
    hwnd: HWND,
    scene: StripScene,
    layout: StripLayout,
}

struct State {
    scenes: Scenes,
    sink: EventSink<StripEvent>,
    strips: HashMap<u64, Strip>,
    glyphs: GdiGlyphs,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|s| s.try_borrow_mut().ok()?.as_mut().map(f))
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn register_class(
    name: &[u16],
    proc_: windows::Win32::UI::WindowsAndMessaging::WNDPROC,
) -> windows::core::Result<HINSTANCE> {
    // SAFETY: plain class registration; the name buffer outlives the call. A second
    // registration fails harmlessly and the first class is reused.
    unsafe {
        let hinst = HINSTANCE(GetModuleHandleW(PCWSTR::null())?.0);
        let mut wc: WNDCLASSEXW = std::mem::zeroed();
        wc.cbSize = size_of::<WNDCLASSEXW>() as u32;
        wc.lpfnWndProc = proc_;
        wc.hInstance = hinst;
        wc.lpszClassName = PCWSTR(name.as_ptr());
        RegisterClassExW(&wc);
        Ok(hinst)
    }
}

fn run(scenes: &Scenes, sink: EventSink<StripEvent>, ready: &mpsc::Sender<Result<usize, String>>) {
    let host_class = wide("KrovaStripHost");
    let strip_class = wide("KrovaStrip");
    let created = register_class(&host_class, Some(host_proc))
        .and_then(|_| register_class(&strip_class, Some(strip_proc)))
        .and_then(|hinst| {
            // SAFETY: a message-only window (parent HWND_MESSAGE) for our own posts.
            unsafe {
                CreateWindowExW(
                    Default::default(),
                    PCWSTR(host_class.as_ptr()),
                    PCWSTR::null(),
                    Default::default(),
                    0,
                    0,
                    0,
                    0,
                    HWND_MESSAGE,
                    HMENU::default(),
                    hinst,
                    None,
                )
            }
        });
    let host = match created {
        Ok(h) => h,
        Err(e) => {
            let _ = ready.send(Err(e.to_string()));
            return;
        }
    };
    STATE.with(|s| {
        *s.borrow_mut() = Some(State {
            scenes: Arc::clone(scenes),
            sink,
            strips: HashMap::new(),
            glyphs: GdiGlyphs::new(),
        });
    });
    let _ = ready.send(Ok(host.0 as usize));

    let mut msg = MSG::default();
    // SAFETY: the standard message loop of this thread.
    while unsafe { GetMessageW(&mut msg, HWND::default(), 0, 0) }.0 > 0 {
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    // SAFETY: destroys the host window created above, on its own thread.
    let _ = unsafe { DestroyWindow(host) };
    STATE.with(|s| {
        if let Some(state) = s.borrow_mut().take() {
            drop(state.glyphs);
        }
    });
}

unsafe extern "system" fn host_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_UPDATE => {
            with_state(update);
            LRESULT(0)
        }
        WM_STOP => {
            with_state(|state| {
                for (_, strip) in state.strips.drain() {
                    destroy_strip(strip.hwnd);
                }
            });
            // SAFETY: ends this thread's message loop.
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        WM_CLOSE => LRESULT(0),
        // SAFETY: forwards the arguments unchanged.
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn low_i16(v: isize) -> i32 {
    i32::from((v & 0xFFFF) as u16 as i16)
}

fn high_i16(v: usize) -> i32 {
    i32::from(((v >> 16) & 0xFFFF) as u16 as i16)
}

unsafe extern "system" fn strip_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        // A click on a tab must not take the focus from the window being worked in.
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_CLOSE => LRESULT(0),
        WM_LBUTTONUP => {
            let (x, y) = (low_i16(lparam.0), low_i16(lparam.0 >> 16));
            let event = with_state(|state| {
                let strip = state.strips.values().find(|s| s.hwnd == hwnd)?;
                let tab = strip.layout.tab_at(x, y)?;
                Some((Arc::clone(&state.sink), strip.layout.tabs[tab].window))
            })
            .flatten();
            if let Some((sink, window)) = event {
                sink(StripEvent::Select { window });
            }
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            let delta = high_i16(wparam.0);
            let event = with_state(|state| {
                let key = state.strips.iter().find(|(_, s)| s.hwnd == hwnd)?.0;
                Some((Arc::clone(&state.sink), *key))
            })
            .flatten();
            if let (Some((sink, key)), true) = (event, delta != 0) {
                sink(StripEvent::Cycle {
                    key,
                    forward: delta < 0,
                });
            }
            LRESULT(0)
        }
        // SAFETY: forwards the arguments unchanged.
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// Make the windows match the wanted scenes.
fn update(state: &mut State) {
    let wanted = state.scenes.lock().map(|s| s.clone()).unwrap_or_default();
    let keys: Vec<u64> = wanted.iter().map(|s| s.key).collect();
    let gone: Vec<u64> = state
        .strips
        .keys()
        .filter(|k| !keys.contains(k))
        .copied()
        .collect();
    for key in gone {
        if let Some(strip) = state.strips.remove(&key) {
            destroy_strip(strip.hwnd);
        }
    }
    for scene in wanted {
        if state
            .strips
            .get(&scene.key)
            .is_some_and(|s| s.scene == scene)
        {
            continue;
        }
        let Some(bitmap) = render(&scene, &state.glyphs) else {
            if let Some(strip) = state.strips.remove(&scene.key) {
                destroy_strip(strip.hwnd);
            }
            continue;
        };
        let hwnd = match state.strips.get(&scene.key) {
            Some(s) => s.hwnd,
            None => match create_strip_window() {
                Ok(h) => h,
                Err(_) => continue,
            },
        };
        let rect = bitmap.layout.rect;
        // SAFETY: positions and shows our own window without activating it.
        let _ = unsafe {
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )
        };
        paint_layered(hwnd, rect, &bitmap.pixels);
        state.strips.insert(
            scene.key,
            Strip {
                hwnd,
                scene,
                layout: bitmap.layout,
            },
        );
    }
}

// --- integration ---
/// A strip window, excluded from screen capture while share-safe (F-29).
fn create_strip_window() -> windows::core::Result<HWND> {
    let hwnd = create_strip_window_raw()?;
    crate::win_capture::register_overlay(hwnd);
    Ok(hwnd)
}

fn destroy_strip(hwnd: HWND) {
    crate::win_capture::unregister_overlay(hwnd);
    // SAFETY: strip windows belong to the strips thread, which calls this.
    let _ = unsafe { DestroyWindow(hwnd) };
}
// --- end integration ---

fn create_strip_window_raw() -> windows::core::Result<HWND> {
    let class = wide("KrovaStrip");
    // SAFETY: creates a popup of the class registered at thread start.
    unsafe {
        let hinst = HINSTANCE(GetModuleHandleW(PCWSTR::null())?.0);
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

// ------------------------------------------------------------------ bitmaps

/// A top-down 32-bit DIB selected into its own memory DC.
struct Dib {
    dc: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
    bits: *mut u32,
    w: i32,
    h: i32,
}

impl Dib {
    fn new(w: i32, h: i32) -> Option<Self> {
        if w <= 0 || h <= 0 || w > 8192 || h > 8192 {
            return None;
        }
        // SAFETY: standard DIB creation; every object is released in `drop`.
        unsafe {
            let screen = GetDC(HWND::default());
            let dc = CreateCompatibleDC(screen);
            ReleaseDC(HWND::default(), screen);
            let mut bmi: BITMAPINFO = std::mem::zeroed();
            bmi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = w;
            bmi.bmiHeader.biHeight = -h;
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32;
            let mut bits: *mut c_void = null_mut();
            let Ok(bitmap) =
                CreateDIBSection(dc, &bmi, DIB_RGB_COLORS, &mut bits, HANDLE::default(), 0)
            else {
                let _ = DeleteDC(dc);
                return None;
            };
            if bits.is_null() {
                let _ = DeleteObject(bitmap);
                let _ = DeleteDC(dc);
                return None;
            }
            let old = SelectObject(dc, bitmap);
            Some(Self {
                dc,
                bitmap,
                old,
                bits: bits.cast(),
                w,
                h,
            })
        }
    }

    fn pixels(&mut self) -> &mut [u32] {
        // SAFETY: the section holds exactly w * h 32-bit pixels while `self` lives;
        // GDI has finished drawing into it once GdiFlush returns.
        unsafe {
            let _ = GdiFlush();
            std::slice::from_raw_parts_mut(self.bits, (self.w * self.h) as usize)
        }
    }
}

impl Drop for Dib {
    fn drop(&mut self) {
        // SAFETY: releases what `new` created, in reverse order.
        unsafe {
            SelectObject(self.dc, self.old);
            let _ = DeleteObject(self.bitmap);
            let _ = DeleteDC(self.dc);
        }
    }
}

/// Push premultiplied pixels to a layered window at `rect`.
fn paint_layered(hwnd: HWND, rect: PxRect, pixels: &[u32]) {
    let Some(mut dib) = Dib::new(rect.w, rect.h) else {
        return;
    };
    let dst = dib.pixels();
    if dst.len() != pixels.len() {
        return;
    }
    dst.copy_from_slice(pixels);
    // SAFETY: the DIB is selected into its DC for the duration of the call.
    unsafe {
        let screen = GetDC(HWND::default());
        let pos = POINT {
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
            Some(&pos),
            Some(&size),
            dib.dc,
            Some(&src),
            COLORREF(0),
            Some(&blend),
            ULW_ALPHA,
        );
        ReleaseDC(HWND::default(), screen);
    }
}

// ------------------------------------------------------------------ glyphs

struct GdiGlyphs {
    fonts: RefCell<HashMap<i32, HFONT>>,
    icons: RefCell<HashMap<(String, i32), Option<Icon>>>,
}

impl GdiGlyphs {
    fn new() -> Self {
        Self {
            fonts: RefCell::new(HashMap::new()),
            icons: RefCell::new(HashMap::new()),
        }
    }

    fn font(&self, px: i32) -> HFONT {
        *self.fonts.borrow_mut().entry(px).or_insert_with(|| {
            let face = wide("Segoe UI");
            // SAFETY: creates a logical font; deleted in `drop`.
            unsafe {
                CreateFontW(
                    -px,
                    0,
                    0,
                    0,
                    400,
                    0,
                    0,
                    0,
                    u32::from(DEFAULT_CHARSET.0),
                    u32::from(OUT_DEFAULT_PRECIS.0),
                    u32::from(CLIP_DEFAULT_PRECIS.0),
                    u32::from(ANTIALIASED_QUALITY.0),
                    0,
                    PCWSTR(face.as_ptr()),
                )
            }
        })
    }
}

impl Drop for GdiGlyphs {
    fn drop(&mut self) {
        for (_, font) in self.fonts.borrow_mut().drain() {
            // SAFETY: fonts created in `font`, no longer selected anywhere.
            let _ = unsafe { DeleteObject(font) };
        }
    }
}

impl Glyphs for GdiGlyphs {
    fn text(&self, text: &str, px: i32, max_w: i32) -> Option<Mask> {
        if text.is_empty() || px <= 0 || max_w <= 0 {
            return None;
        }
        let font = self.font(px);
        let mut buf: Vec<u16> = text.encode_utf16().collect();
        // Measure on a scratch DC.
        let mut probe = Dib::new(1, 1)?;
        let mut r = RECT {
            left: 0,
            top: 0,
            right: max_w,
            bottom: px * 2,
        };
        // SAFETY: font selected into our own DC; `buf` and `r` outlive the calls.
        unsafe {
            SelectObject(probe.dc, font);
            DrawTextW(
                probe.dc,
                &mut buf,
                &mut r,
                DT_CALCRECT | DT_SINGLELINE | DT_NOPREFIX,
            );
        }
        let _ = probe.pixels();
        let w = (r.right - r.left).clamp(1, max_w);
        let h = (r.bottom - r.top).max(1);
        let mut dib = Dib::new(w, h)?;
        let mut area = RECT {
            left: 0,
            top: 0,
            right: w,
            bottom: h,
        };
        // SAFETY: as above; white text on the zeroed (black) DIB.
        unsafe {
            SelectObject(dib.dc, font);
            SetBkMode(dib.dc, TRANSPARENT);
            SetTextColor(dib.dc, COLORREF(0x00FF_FFFF));
            DrawTextW(
                dib.dc,
                &mut buf,
                &mut area,
                DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS | DT_VCENTER,
            );
        }
        let alpha = dib
            .pixels()
            .iter()
            .map(|p| {
                let r = (p >> 16) & 0xFF;
                let g = (p >> 8) & 0xFF;
                let b = p & 0xFF;
                r.max(g).max(b) as u8
            })
            .collect();
        Some(Mask { w, h, alpha })
    }

    fn icon(&self, path: &str, px: i32) -> Option<Icon> {
        let key = (path.to_owned(), px);
        if let Some(cached) = self.icons.borrow().get(&key) {
            return cached.clone();
        }
        let icon = extract_icon(path, px);
        let mut cache = self.icons.borrow_mut();
        if cache.len() >= ICON_CACHE {
            cache.clear();
        }
        cache.insert(key, icon.clone());
        icon
    }
}

/// The program's icon at `px` pixels, premultiplied. Drawn once on black and once
/// on white so that icons without an alpha channel come out right too.
fn extract_icon(path: &str, px: i32) -> Option<Icon> {
    if px <= 0 || px > 256 {
        return None;
    }
    let file = wide(path);
    let mut large = HICON::default();
    let mut small = HICON::default();
    // SAFETY: asks for one icon of each size; both are destroyed below.
    let found = unsafe {
        ExtractIconExW(
            PCWSTR(file.as_ptr()),
            0,
            Some(&mut large),
            Some(&mut small),
            1,
        )
    };
    if found == 0 || found == u32::MAX {
        return None;
    }
    let chosen = if px <= 16 && !small.is_invalid() {
        small
    } else if !large.is_invalid() {
        large
    } else {
        small
    };
    let draw = |background: u32| -> Option<Vec<u32>> {
        let mut dib = Dib::new(px, px)?;
        dib.pixels().fill(background);
        // SAFETY: draws the icon into our own DIB.
        unsafe { DrawIconEx(dib.dc, 0, 0, chosen, px, px, 0, None, DI_NORMAL) }.ok()?;
        Some(dib.pixels().to_vec())
    };
    let on_black = draw(0xFF00_0000);
    let on_white = draw(0xFFFF_FFFF);
    // SAFETY: icons returned by ExtractIconExW, owned by us.
    unsafe {
        if !large.is_invalid() {
            let _ = DestroyIcon(large);
        }
        if !small.is_invalid() {
            let _ = DestroyIcon(small);
        }
    }
    let (black, white) = (on_black?, on_white?);
    let pixels = black
        .iter()
        .zip(&white)
        .map(|(&b, &w)| {
            // alpha = 255 - (white - black), per channel; take green, the most precise.
            let bg = (b >> 8) & 0xFF;
            let wg = (w >> 8) & 0xFF;
            let a = 255 - wg.saturating_sub(bg).min(255);
            let ch = |shift: u32| ((b >> shift) & 0xFF).min(a);
            (a << 24) | (ch(16) << 16) | (ch(8) << 8) | ch(0)
        })
        .collect();
    Some(Icon { size: px, pixels })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strips::{StripTab, StripTheme};
    use crate::WindowId;

    #[test]
    fn mouse_coordinates_are_signed() {
        assert_eq!(low_i16(0x0001_FFFF), -1);
        assert_eq!(low_i16(0x0000_0010), 16);
        assert_eq!(high_i16(0xFF88_0000), -120);
        assert_eq!(high_i16(0x0078_0000), 120);
    }

    #[test]
    fn gdi_text_gives_a_coverage_mask_within_the_width() {
        let g = GdiGlyphs::new();
        let m = g.text("Chrome", 12, 200).expect("text renders");
        assert!(m.w > 10 && m.w <= 200 && m.h >= 12);
        assert!(m.alpha.iter().any(|&a| a > 200), "some pixels are covered");
        let cut = g.text("A rather long program name", 12, 40).unwrap();
        assert!(cut.w <= 40);
        assert_eq!(g.text("", 12, 40), None);
    }

    #[test]
    fn an_icon_comes_out_premultiplied() {
        let g = GdiGlyphs::new();
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
        let icon = g
            .icon(&format!(r"{windir}\explorer.exe"), 16)
            .expect("explorer has an icon");
        assert_eq!(icon.pixels.len(), 16 * 16);
        for p in &icon.pixels {
            let a = p >> 24;
            assert!((p >> 16) & 0xFF <= a && (p >> 8) & 0xFF <= a && p & 0xFF <= a);
        }
        assert!(g.icon(r"C:\no\such\file.exe", 16).is_none());
    }

    #[test]
    #[ignore = "shows a real strip on screen; needs an interactive desktop session"]
    fn a_strip_appears_and_goes() {
        let (tx, _rx) = mpsc::channel();
        let host = WinStrips::start(Arc::new(move |e| {
            let _ = tx.send(e);
        }))
        .expect("strip host");
        let scene = StripScene {
            key: 1,
            zone: PxRect::new(100, 100, 1000, 600),
            tabs: vec![
                StripTab {
                    window: WindowId(1),
                    label: "Explorer".into(),
                    icon_path: Some(r"C:\Windows\explorer.exe".into()),
                    active: true,
                },
                StripTab {
                    window: WindowId(2),
                    label: "Notepad".into(),
                    icon_path: None,
                    active: false,
                },
            ],
            theme: StripTheme::Dark,
            accent: [0xff, 0x5b, 0x1f],
            radius_px: 10,
            scale: 1.0,
            hide_names: false,
        };
        host.set_strips(vec![scene]);
        std::thread::sleep(Duration::from_millis(800));
        host.set_strips(Vec::new());
        std::thread::sleep(Duration::from_millis(200));
    }
}
