//! Windows backend for the drag-highlight overlay.
//!
//! STATUS: written without a compiler available (the shell tool was down),
//! against `windows` 0.58. The first real build is CI on windows-latest; treat
//! this file as unverified until that run is green.
//!
//! A layered, click-through, topmost tool window lives on its own thread with its
//! own message loop. The scene is rasterised by `overlay::render` and handed to
//! `UpdateLayeredWindow` (per-pixel alpha). Nothing is repainted unless the scene
//! actually changed, and bursts of updates collapse into one repaint.

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
    COLORREF, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject,
    BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, DIB_RGB_COLORS,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, PeekMessageW,
    PostThreadMessageW, RegisterClassExW, SetWindowPos, ShowWindow, TranslateMessage,
    UpdateLayeredWindow, HMENU, HWND_TOPMOST, MSG, PM_NOREMOVE, PM_REMOVE, SWP_NOACTIVATE,
    SWP_SHOWWINDOW, SW_HIDE, ULW_ALPHA, WM_APP, WM_CLOSE, WM_QUIT, WNDCLASSEXW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

use crate::overlay::{render, Overlay, OverlayScene};
use crate::PlatformError;

/// Posted to the overlay thread whenever the shared scene changed.
const WM_UPDATE: u32 = WM_APP + 1;

type SharedScene = Arc<Mutex<Option<OverlayScene>>>;

pub struct WinOverlay {
    scene: SharedScene,
    thread_id: u32,
    join: Option<JoinHandle<()>>,
}

impl WinOverlay {
    /// Create the (initially hidden) overlay window on its own thread.
    pub fn start() -> Result<Self, PlatformError> {
        let scene: SharedScene = Arc::new(Mutex::new(None));
        let (ready_tx, ready_rx) = mpsc::channel::<Result<u32, String>>();
        let shared = Arc::clone(&scene);
        let join = thread::Builder::new()
            .name("krova-overlay".into())
            .spawn(move || run(shared, ready_tx))
            .map_err(|e| PlatformError::Os(e.to_string()))?;
        match ready_rx.recv_timeout(Duration::from_secs(2)) {
            Ok(Ok(thread_id)) => Ok(Self {
                scene,
                thread_id,
                join: Some(join),
            }),
            Ok(Err(msg)) => {
                let _ = join.join();
                Err(PlatformError::Os(format!(
                    "could not create overlay: {msg}"
                )))
            }
            Err(_) => Err(PlatformError::Os("overlay thread did not start".into())),
        }
    }
}

impl Overlay for WinOverlay {
    fn set_scene(&self, scene: Option<OverlayScene>) {
        if let Ok(mut slot) = self.scene.lock() {
            *slot = scene;
        }
        let _ = unsafe { PostThreadMessageW(self.thread_id, WM_UPDATE, WPARAM(0), LPARAM(0)) };
    }
}

impl Drop for WinOverlay {
    fn drop(&mut self) {
        let _ = unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // Ignore a WM_CLOSE sent by another program: it would destroy the overlay.
    if msg == WM_CLOSE {
        return LRESULT(0);
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn create_window() -> windows::core::Result<HWND> {
    unsafe {
        let hinst = HINSTANCE(GetModuleHandleW(PCWSTR::null())?.0);
        let class: Vec<u16> = "KrovaOverlay\0".encode_utf16().collect();
        // SAFETY: WNDCLASSEXW is plain data; all-zero is a valid starting value.
        let mut wc: WNDCLASSEXW = std::mem::zeroed();
        wc.cbSize = size_of::<WNDCLASSEXW>() as u32;
        wc.lpfnWndProc = Some(wnd_proc);
        wc.hInstance = hinst;
        wc.lpszClassName = PCWSTR(class.as_ptr());
        // Registering twice fails harmlessly; the class from the first call is reused.
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

fn run(scene: SharedScene, ready: mpsc::Sender<Result<u32, String>>) {
    let hwnd = match create_window() {
        Ok(h) => {
            crate::win_capture::register_overlay(h);
            h
        }
        Err(e) => {
            let _ = ready.send(Err(e.to_string()));
            return;
        }
    };
    // Touching the queue creates it, so messages posted right away are not lost.
    let mut msg = MSG::default();
    let _ = unsafe { PeekMessageW(&mut msg, HWND::default(), 0, 0, PM_NOREMOVE) };
    let _ = ready.send(Ok(unsafe { GetCurrentThreadId() }));

    let mut shown: Option<OverlayScene> = None;
    while unsafe { GetMessageW(&mut msg, HWND::default(), 0, 0) }.0 > 0 {
        if msg.message == WM_UPDATE {
            // Collapse a burst of updates into a single repaint.
            while unsafe {
                PeekMessageW(&mut msg, HWND::default(), WM_UPDATE, WM_UPDATE, PM_REMOVE)
            }
            .as_bool()
            {}
            let wanted = scene.lock().ok().and_then(|s| s.clone());
            if wanted != shown {
                apply(hwnd, wanted.as_ref());
                shown = wanted;
            }
            continue;
        }
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    crate::win_capture::unregister_overlay(hwnd);
    let _ = unsafe { DestroyWindow(hwnd) };
}

fn apply(hwnd: HWND, scene: Option<&OverlayScene>) {
    let pixels = scene.map(render).unwrap_or_default();
    let Some(scene) = scene.filter(|_| !pixels.is_empty()) else {
        let _ = unsafe { ShowWindow(hwnd, SW_HIDE) };
        return;
    };
    let m = scene.monitor;
    // Position and show without activating, then hand over the bitmap.
    let _ = unsafe {
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            m.x,
            m.y,
            m.w,
            m.h,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        )
    };
    paint(hwnd, m, &pixels);
}

/// Push premultiplied ARGB `pixels` (`monitor.w * monitor.h`) to the layered window.
fn paint(hwnd: HWND, monitor: PxRect, pixels: &[u32]) {
    // The DIB below is sized from the monitor rect and filled from `pixels`; refuse to
    // touch it unless the two agree exactly.
    let expected = usize::try_from(monitor.w)
        .unwrap_or(0)
        .saturating_mul(usize::try_from(monitor.h).unwrap_or(0));
    if expected == 0 || pixels.len() != expected {
        return;
    }
    unsafe {
        let screen = GetDC(HWND::default());
        let mem = CreateCompatibleDC(screen);

        // SAFETY: BITMAPINFO is plain data; all-zero is a valid starting value.
        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = monitor.w;
        bmi.bmiHeader.biHeight = -monitor.h; // negative height = top-down rows
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32; // biCompression 0 is BI_RGB

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
                // SAFETY: the DIB section holds monitor.w * monitor.h 32-bit pixels,
                // which is exactly pixels.len() (checked by `render`).
                std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits.cast::<u32>(), pixels.len());
                let old = SelectObject(mem, bitmap);
                let dst = POINT {
                    x: monitor.x,
                    y: monitor.y,
                };
                let size = SIZE {
                    cx: monitor.w,
                    cy: monitor.h,
                };
                let src = POINT { x: 0, y: 0 };
                let blend = BLENDFUNCTION {
                    BlendOp: 0, // AC_SRC_OVER
                    BlendFlags: 0,
                    SourceConstantAlpha: 255,
                    AlphaFormat: 1, // AC_SRC_ALPHA: pixels are premultiplied ARGB
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overlay::OverlayStyle;

    #[test]
    #[ignore = "creates a real window; needs an interactive desktop session"]
    fn overlay_starts_updates_and_stops() {
        let overlay = WinOverlay::start().expect("overlay window");
        overlay.set_scene(Some(OverlayScene {
            monitor: PxRect::new(0, 0, 200, 100),
            zones: vec![(1, PxRect::new(0, 0, 100, 100))],
            highlight: Some(1),
            preview: None,
            style: OverlayStyle::from_settings("#3b82f6", 0.35),
            gapped: false,
        }));
        overlay.set_scene(None);
        drop(overlay);
    }
}
