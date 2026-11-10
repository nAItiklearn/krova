//! Windows side of the wave-3 seam ([`crate::desk::Desk`]): listing arrangeable
//! windows with their executable paths, minimising and restoring them, launching a
//! captured program and waiting for its first window.
//!
//! Window titles are never read. A program is launched with `ShellExecuteExW`, with
//! no arguments, from its own folder, and only after the caller checked the path.

use std::collections::HashSet;
use std::ffi::c_void;
use std::mem::size_of;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use krova_core::workspace::same_exe_path;
use krova_core::PxRect;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, BOOL, HMODULE, HWND, LPARAM, RECT, TRUE};
use windows::Win32::Graphics::Dwm::{
    DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS,
};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, GetCurrentThreadId, OpenProcess, QueryFullProcessImageNameW,
    PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::Shell::{
    ShellExecuteExW, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, EnumWindows, GetAncestor, GetClassNameW, GetMessageW, GetWindow,
    GetWindowLongW, GetWindowRect, GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible,
    PostThreadMessageW, ShowWindow, TranslateMessage, EVENT_OBJECT_SHOW, EVENT_OBJECT_UNCLOAKED,
    EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_MINIMIZEEND, GA_ROOT, GWL_EXSTYLE, GWL_STYLE, GW_OWNER,
    MSG, OBJID_WINDOW, SW_SHOWMINNOACTIVE, SW_SHOWNOACTIVATE, WINEVENT_OUTOFCONTEXT,
    WINEVENT_SKIPOWNPROCESS, WM_APP, WM_QUIT, WS_CAPTION, WS_EX_TOOLWINDOW,
};

use crate::desk::{Desk, DeskCapabilities, DeskWindow, LaunchFailure, Visuals};
use crate::{PlatformError, WindowId};
use krova_core::focus::{VisualsConfig, WIN11_BUILD};

/// Posted to the wait thread for every window event of interest.
const WM_SEEN: u32 = WM_APP + 21;

#[derive(Debug, Default, Clone, Copy)]
pub struct WinDesk;

impl WinDesk {
    pub fn new() -> Self {
        Self
    }
}

// ---------------------------------------------------------------- helpers

pub(crate) fn hwnd(id: WindowId) -> HWND {
    HWND(id.0 as usize as *mut c_void)
}

pub(crate) fn window_id(h: HWND) -> WindowId {
    WindowId(h.0 as usize as u64)
}

pub(crate) fn px(r: RECT) -> PxRect {
    PxRect::new(r.left, r.top, r.right - r.left, r.bottom - r.top)
}

/// The frame the user sees (DWM extended frame bounds), else the window rect.
pub(crate) fn frame(h: HWND) -> Option<PxRect> {
    let mut r = RECT::default();
    // SAFETY: `r` is a valid out-buffer of the size passed.
    let dwm = unsafe {
        DwmGetWindowAttribute(
            h,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&mut r as *mut RECT).cast::<c_void>(),
            size_of::<RECT>() as u32,
        )
    };
    if dwm.is_ok() {
        return Some(px(r));
    }
    // SAFETY: as above.
    unsafe { GetWindowRect(h, &mut r) }.ok()?;
    Some(px(r))
}

pub(crate) fn is_cloaked(h: HWND) -> bool {
    let mut cloaked = 0u32;
    // SAFETY: `cloaked` is a valid out-buffer of the size passed.
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

fn class_name(h: HWND) -> String {
    let mut buf = [0u16; 64];
    // SAFETY: the buffer outlives the call.
    let n = unsafe { GetClassNameW(h, &mut buf) };
    String::from_utf16_lossy(&buf[..usize::try_from(n).unwrap_or(0)])
}

fn pid_of(h: HWND) -> u32 {
    let mut pid = 0u32;
    // SAFETY: valid out-pointer; an invalid window yields 0.
    unsafe { GetWindowThreadProcessId(h, Some(&mut pid)) };
    pid
}

/// An ordinary application window: top-level, unowned, visible, on this virtual
/// desktop, with a caption, not a tool window and not the shell. Minimised ones
/// pass only with `allow_minimized`. Krova's own windows never pass.
pub(crate) fn arrangeable(h: HWND, allow_minimized: bool) -> bool {
    // SAFETY: plain query.
    ordinary_window(h, allow_minimized) && pid_of(h) != unsafe { GetCurrentProcessId() }
}

/// [`arrangeable`] without the "not Krova's own" rule (focus visuals decorate the
/// settings window too).
pub(crate) fn ordinary_window(h: HWND, allow_minimized: bool) -> bool {
    // SAFETY: plain queries on a window handle; all are safe on stale handles.
    unsafe {
        if !IsWindow(h).as_bool() || !IsWindowVisible(h).as_bool() {
            return false;
        }
        if !allow_minimized && IsIconic(h).as_bool() {
            return false;
        }
        if GetAncestor(h, GA_ROOT) != h {
            return false;
        }
        if GetWindow(h, GW_OWNER).is_ok_and(|o| !o.0.is_null()) {
            return false;
        }
        let style = GetWindowLongW(h, GWL_STYLE) as u32;
        let ex = GetWindowLongW(h, GWL_EXSTYLE) as u32;
        if (style & WS_CAPTION.0) != WS_CAPTION.0 || (ex & WS_EX_TOOLWINDOW.0) != 0 {
            return false;
        }
    }
    if is_cloaked(h) {
        return false;
    }
    if matches!(
        class_name(h).as_str(),
        "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
    ) {
        return false;
    }
    pid_of(h) != 0
}

/// Full path of the executable that owns the window. Never its title.
pub(crate) fn exe_path(h: HWND) -> Option<String> {
    let pid = pid_of(h);
    if pid == 0 {
        return None;
    }
    // SAFETY: limited query rights; the handle is closed below.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let mut buf = vec![0u16; 1024];
    let mut len = buf.len() as u32;
    // SAFETY: `buf` holds `len` UTF-16 units.
    let ok = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
    }
    .is_ok();
    // SAFETY: we opened it.
    let _ = unsafe { CloseHandle(process) };
    if !ok {
        return None;
    }
    let len = usize::try_from(len).unwrap_or(0).min(buf.len());
    Some(String::from_utf16_lossy(&buf[..len]))
}

unsafe extern "system" fn collect(h: HWND, data: LPARAM) -> BOOL {
    // SAFETY: `data` is the Vec that `top_level_windows` keeps alive for the walk.
    let list = unsafe { &mut *(data.0 as *mut Vec<HWND>) };
    if list.len() < 4096 {
        list.push(h);
    }
    TRUE
}

/// All top-level windows, topmost first.
pub(crate) fn top_level_windows() -> Vec<HWND> {
    let mut list: Vec<HWND> = Vec::new();
    // SAFETY: the callback only pushes into `list`, which outlives the call.
    let _ = unsafe { EnumWindows(Some(collect), LPARAM(&mut list as *mut Vec<HWND> as isize)) };
    list
}

/// The Windows build number, from the registry (not affected by compatibility shims).
pub fn os_build() -> u32 {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    winreg::RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
        .and_then(|k| k.get_value::<String, _>("CurrentBuildNumber"))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// ------------------------------------------------------------- wait thread

/// The wait thread's id; its WinEvent callback posts what it sees to itself.
fn post_seen(h: HWND) {
    // SAFETY: posting to our own thread's queue (we are on it: out-of-context hooks
    // call back on the installing thread).
    let _ = unsafe {
        PostThreadMessageW(
            GetCurrentThreadId(),
            WM_SEEN,
            windows::Win32::Foundation::WPARAM(h.0 as usize),
            LPARAM(0),
        )
    };
}

unsafe extern "system" fn seen_proc(
    _hook: HWINEVENTHOOK,
    _event: u32,
    h: HWND,
    id_object: i32,
    id_child: i32,
    _thread: u32,
    _time: u32,
) {
    if id_object == OBJID_WINDOW.0 && id_child == 0 && !h.0.is_null() {
        post_seen(h);
    }
}

/// Match a candidate window to the first open slot with its exe.
fn claim(
    h: HWND,
    exes: &[String],
    found: &mut [Option<WindowId>],
    taken: &mut HashSet<u64>,
) -> bool {
    let id = window_id(h);
    if taken.contains(&id.0) || !arrangeable(h, false) {
        return false;
    }
    let Some(exe) = exe_path(h) else {
        return false;
    };
    for (slot, want) in found.iter_mut().zip(exes) {
        if slot.is_none() && same_exe_path(&exe, want) {
            *slot = Some(id);
            taken.insert(id.0);
            return true;
        }
    }
    false
}

fn wait_thread(
    exes: Vec<String>,
    known: Vec<WindowId>,
    timeout: Duration,
    out: mpsc::Sender<Vec<Option<WindowId>>>,
    ready: mpsc::Sender<u32>,
) {
    // SAFETY: plain query.
    let me = unsafe { GetCurrentThreadId() };
    let mut msg = MSG::default();
    // Create the queue before the hooks so no posted event is lost.
    // SAFETY: peek on our own queue.
    let _ = unsafe {
        windows::Win32::UI::WindowsAndMessaging::PeekMessageW(
            &mut msg,
            HWND::default(),
            0,
            0,
            windows::Win32::UI::WindowsAndMessaging::PM_NOREMOVE,
        )
    };
    let flags = WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS;
    // SAFETY: out-of-context hooks with a plain callback, removed below on this thread.
    let hooks: Vec<HWINEVENTHOOK> = unsafe {
        vec![
            SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                HMODULE::default(),
                Some(seen_proc),
                0,
                0,
                flags,
            ),
            SetWinEventHook(
                EVENT_SYSTEM_MINIMIZEEND,
                EVENT_SYSTEM_MINIMIZEEND,
                HMODULE::default(),
                Some(seen_proc),
                0,
                0,
                flags,
            ),
            SetWinEventHook(
                EVENT_OBJECT_SHOW,
                EVENT_OBJECT_SHOW,
                HMODULE::default(),
                Some(seen_proc),
                0,
                0,
                flags,
            ),
            SetWinEventHook(
                EVENT_OBJECT_UNCLOAKED,
                EVENT_OBJECT_UNCLOAKED,
                HMODULE::default(),
                Some(seen_proc),
                0,
                0,
                flags,
            ),
        ]
    };
    let _ = ready.send(me);

    let mut found: Vec<Option<WindowId>> = vec![None; exes.len()];
    let mut taken: HashSet<u64> = known.iter().map(|w| w.0).collect();
    // Windows that appeared before the hooks were in place.
    for h in top_level_windows() {
        claim(h, &exes, &mut found, &mut taken);
    }

    let deadline = Instant::now() + timeout;
    // A one-shot timer wakes GetMessageW at the deadline (no polling in between).
    let ms = u32::try_from(timeout.as_millis())
        .unwrap_or(u32::MAX)
        .max(1);
    // SAFETY: a thread timer (no window); killed below.
    let timer =
        unsafe { windows::Win32::UI::WindowsAndMessaging::SetTimer(HWND::default(), 0, ms, None) };
    while found.iter().any(Option::is_none) && Instant::now() < deadline {
        // SAFETY: standard message loop on our own queue.
        if unsafe { GetMessageW(&mut msg, HWND::default(), 0, 0) }.0 <= 0 {
            break;
        }
        match msg.message {
            WM_SEEN => {
                let h = HWND(msg.wParam.0 as *mut c_void);
                claim(h, &exes, &mut found, &mut taken);
            }
            m if m == windows::Win32::UI::WindowsAndMessaging::WM_TIMER => {}
            _ => {
                // SAFETY: dispatching a message this thread received.
                unsafe {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }
    }
    // SAFETY: removing what we installed, on this thread.
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::KillTimer(HWND::default(), timer);
        for h in hooks {
            if !h.is_invalid() {
                let _ = UnhookWinEvent(h);
            }
        }
    }
    let _ = out.send(found);
}

// ------------------------------------------------------- safe launching (F-26)

/// `GetDriveTypeW` result for a fixed local disk.
const DRIVE_FIXED: u32 = 3;

/// An open file handle, closed on drop.
struct FileHandle(windows::Win32::Foundation::HANDLE);

impl Drop for FileHandle {
    fn drop(&mut self) {
        // SAFETY: a handle we opened and own.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

/// Open `path` for reading, sharing read only: while the handle is held nobody can
/// write, rename or delete the file (no `FILE_SHARE_WRITE` / `FILE_SHARE_DELETE`).
/// `directory` opens a folder instead (for resolving blocked folders).
fn open_locked(path: &str, directory: bool) -> Option<FileHandle> {
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_BACKUP_SEMANTICS, FILE_GENERIC_READ,
        FILE_SHARE_READ, OPEN_EXISTING,
    };
    let w = wide(path);
    let flags = if directory {
        FILE_FLAG_BACKUP_SEMANTICS
    } else {
        FILE_ATTRIBUTE_NORMAL
    };
    // SAFETY: `w` is NUL-terminated and outlives the call.
    unsafe {
        CreateFileW(
            PCWSTR(w.as_ptr()),
            FILE_GENERIC_READ.0,
            FILE_SHARE_READ,
            None,
            OPEN_EXISTING,
            flags,
            None,
        )
    }
    .ok()
    .map(FileHandle)
}

/// The final (link-resolved, normalised) path of an open file, as `C:\...`.
/// `Err` carries the raw path when it is not a plain drive path.
fn final_path(h: &FileHandle) -> Result<String, Option<String>> {
    use windows::Win32::Storage::FileSystem::{
        GetFinalPathNameByHandleW, FILE_NAME_NORMALIZED, GETFINALPATHNAMEBYHANDLE_FLAGS,
        VOLUME_NAME_DOS,
    };
    let mut buf = vec![0u16; 1024];
    // SAFETY: the buffer outlives the call; its length is passed with it.
    let n = unsafe {
        GetFinalPathNameByHandleW(
            h.0,
            &mut buf,
            GETFINALPATHNAMEBYHANDLE_FLAGS(FILE_NAME_NORMALIZED.0 | VOLUME_NAME_DOS.0),
        )
    } as usize;
    if n == 0 || n >= buf.len() {
        return Err(None);
    }
    let raw = String::from_utf16_lossy(&buf[..n]);
    krova_core::workspace::normalize_final_path(&raw).ok_or(Some(raw))
}

/// `GetDriveTypeW` of the drive of `path` (`C:\...`).
fn drive_type(path: &str) -> u32 {
    let Some(root) = path.get(..3) else {
        return 0;
    };
    let w = wide(root);
    // SAFETY: NUL-terminated buffer outliving the call.
    unsafe { windows::Win32::Storage::FileSystem::GetDriveTypeW(PCWSTR(w.as_ptr())) }
}

/// A known folder (Downloads, Desktop, Roaming AppData) of the current user.
fn known_folder(id: &windows::core::GUID) -> Option<String> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{SHGetKnownFolderPath, KF_FLAG_DEFAULT};
    // SAFETY: `id` is a valid folder id; the returned buffer is freed below.
    let p = unsafe { SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None) }.ok()?;
    // SAFETY: a NUL-terminated string from the shell.
    let s = unsafe { p.to_string() }.ok();
    // SAFETY: allocated by SHGetKnownFolderPath with the COM allocator.
    unsafe { CoTaskMemFree(Some(p.0 as *const c_void)) };
    s.filter(|s| !s.trim().is_empty())
}

/// Every blocked folder as given and, when it can be opened, as its final path
/// (a redirected or linked Desktop is caught either way).
fn with_final_paths(dirs: Vec<String>) -> Vec<String> {
    let mut out = Vec::with_capacity(dirs.len() * 2);
    for d in dirs {
        if let Some(Ok(f)) = open_locked(&d, true).map(|h| final_path(&h)) {
            if !f.eq_ignore_ascii_case(&d) {
                out.push(f);
            }
        }
        out.push(d);
    }
    out
}

/// Open `exe`, resolve its final path, check that path again and that it is on a
/// fixed drive. The handle is returned with the path and must be held until the
/// program has been started.
fn locked_launch_path(
    exe: &str,
    blocked: &[String],
) -> Result<(FileHandle, String), LaunchFailure> {
    let os = |m: &str| PlatformError::Os(m.to_owned());
    let handle = open_locked(exe, false).ok_or_else(|| {
        LaunchFailure::new(Some(exe.to_owned()), os("the program is no longer there"))
    })?;
    let path = final_path(&handle).map_err(|raw| {
        LaunchFailure::new(
            Some(raw.unwrap_or_else(|| exe.to_owned())),
            os("the program's real location is not a local drive"),
        )
    })?;
    if let Err(e) = krova_core::workspace::check_launch_path(&path, blocked) {
        return Err(LaunchFailure::new(Some(path), os(&e.to_string())));
    }
    if drive_type(&path) != DRIVE_FIXED {
        let e = krova_core::workspace::LaunchRefusal::NotFixedDrive;
        return Err(LaunchFailure::new(Some(path), os(&e.to_string())));
    }
    Ok((handle, path))
}

impl Desk for WinDesk {
    fn capabilities(&self) -> DeskCapabilities {
        DeskCapabilities {
            workspaces: true,
            calm_desk: true,
            focus_dim: true,
            highlight: true,
            dwm_borders: os_build() >= WIN11_BUILD,
        }
    }

    fn os_build(&self) -> u32 {
        os_build()
    }

    fn arrangeable_windows(&self, include_minimized: bool) -> Vec<DeskWindow> {
        top_level_windows()
            .into_iter()
            .filter(|&h| arrangeable(h, include_minimized))
            .map(|h| DeskWindow {
                id: window_id(h),
                exe: exe_path(h),
                rect: frame(h).unwrap_or(PxRect::new(0, 0, 0, 0)),
                // SAFETY: plain query.
                minimized: unsafe { IsIconic(h) }.as_bool(),
            })
            .collect()
    }

    fn minimize(&self, window: WindowId) -> Result<(), PlatformError> {
        let h = hwnd(window);
        // SAFETY: plain query / show call on a window handle.
        if !unsafe { IsWindow(h) }.as_bool() {
            return Err(PlatformError::WindowGone);
        }
        // Minimise without activating the next window: focus stays where it is.
        let _ = unsafe { ShowWindow(h, SW_SHOWMINNOACTIVE) };
        Ok(())
    }

    fn unminimize(&self, window: WindowId) -> Result<(), PlatformError> {
        let h = hwnd(window);
        // SAFETY: as above.
        if !unsafe { IsWindow(h) }.as_bool() {
            return Err(PlatformError::WindowGone);
        }
        let _ = unsafe { ShowWindow(h, SW_SHOWNOACTIVATE) };
        Ok(())
    }

    fn is_minimized(&self, window: WindowId) -> bool {
        // SAFETY: plain query.
        unsafe { IsIconic(hwnd(window)) }.as_bool()
    }

    fn temp_dirs(&self) -> Vec<String> {
        let mut dirs: Vec<String> = ["TEMP", "TMP"]
            .iter()
            .filter_map(|v| std::env::var(v).ok())
            .collect();
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            dirs.push(format!("{local}\\Temp"));
        }
        if let Ok(root) = std::env::var("SystemRoot") {
            dirs.push(format!("{root}\\Temp"));
        }
        dirs.push(std::env::temp_dir().display().to_string());
        dirs.retain(|d| !d.trim().is_empty());
        dirs
    }

    fn blocked_dirs(&self) -> Vec<String> {
        use windows::Win32::UI::Shell::{
            FOLDERID_Desktop, FOLDERID_Downloads, FOLDERID_RoamingAppData,
        };
        let mut dirs = self.temp_dirs();
        for id in [
            FOLDERID_Downloads,
            FOLDERID_Desktop,
            FOLDERID_RoamingAppData,
        ] {
            dirs.extend(known_folder(&id));
        }
        if let Ok(roaming) = std::env::var("APPDATA") {
            dirs.push(roaming);
        }
        dirs.retain(|d| !d.trim().is_empty());
        with_final_paths(dirs)
    }

    fn launch(&self, exe: &str, blocked_dirs: &[String]) -> Result<String, LaunchFailure> {
        // Held until ShellExecuteExW returns (SEE_MASK_NOASYNC: the process exists
        // by then), so the file launched is the file checked.
        let (_handle, exe) = locked_launch_path(exe, blocked_dirs)?;
        let exe = exe.as_str();
        let file = wide(exe);
        let dir = std::path::Path::new(exe)
            .parent()
            .map(|p| wide(&p.display().to_string()));
        let verb = wide("open");
        // SAFETY: SHELLEXECUTEINFOW is plain data; all-zero is a valid start.
        let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
        info.cbSize = size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_FLAG_NO_UI | SEE_MASK_NOASYNC;
        info.lpVerb = PCWSTR(verb.as_ptr());
        info.lpFile = PCWSTR(file.as_ptr());
        // Never any arguments.
        info.lpParameters = PCWSTR::null();
        info.lpDirectory = dir.as_ref().map_or(PCWSTR::null(), |d| PCWSTR(d.as_ptr()));
        info.nShow = SW_SHOWNOACTIVATE.0;
        // SAFETY: every pointer in `info` points at a buffer that outlives the call.
        unsafe { ShellExecuteExW(&mut info) }
            .map(|()| exe.to_owned())
            .map_err(|e| {
                LaunchFailure::new(
                    Some(exe.to_owned()),
                    PlatformError::Os(format!("could not start the program: {e}")),
                )
            })
    }

    fn wait_for_windows(
        &self,
        exes: &[String],
        known: &[WindowId],
        timeout: Duration,
    ) -> Vec<Option<WindowId>> {
        if exes.is_empty() {
            return Vec::new();
        }
        let (out_tx, out_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let (exes_owned, known_owned) = (exes.to_vec(), known.to_vec());
        let spawned = thread::Builder::new()
            .name("krova-launch-wait".into())
            .spawn(move || wait_thread(exes_owned, known_owned, timeout, out_tx, ready_tx));
        let Ok(join) = spawned else {
            return vec![None; exes.len()];
        };
        let thread_id = ready_rx.recv_timeout(Duration::from_secs(2)).ok();
        let result = out_rx
            .recv_timeout(timeout + Duration::from_secs(2))
            .unwrap_or_else(|_| vec![None; exes.len()]);
        if let Some(id) = thread_id {
            // SAFETY: the thread is still joinable, so its id is still ours.
            let _ = unsafe {
                PostThreadMessageW(
                    id,
                    WM_QUIT,
                    windows::Win32::Foundation::WPARAM(0),
                    LPARAM(0),
                )
            };
        }
        let _ = join.join();
        result
    }

    fn start_visuals(&self, config: VisualsConfig) -> Result<Box<dyn Visuals>, PlatformError> {
        Ok(Box::new(crate::win_visuals::WinVisuals::start(config)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_build_number_is_read() {
        // Windows 10 1507 is 10240; anything readable is above that.
        assert!(os_build() >= 10240, "{}", os_build());
    }

    #[test]
    fn launch_paths_are_resolved_locked_and_rechecked() {
        let d = WinDesk::new();
        let blocked = d.blocked_dirs();
        let lower: Vec<String> = blocked.iter().map(|b| b.to_lowercase()).collect();
        if let Ok(roaming) = std::env::var("APPDATA") {
            assert!(lower.contains(&roaming.to_lowercase()), "{blocked:?}");
        }
        assert!(
            lower.iter().any(|b| b.ends_with("downloads")),
            "{blocked:?}"
        );
        assert!(lower.iter().any(|b| b.ends_with("desktop")), "{blocked:?}");

        // Our own test binary lives in target\ (a fixed drive, not blocked).
        let me = std::env::current_exe().unwrap().display().to_string();
        let (h, path) = locked_launch_path(&me, &blocked).expect("own exe is launchable");
        assert!(!path.starts_with("\\\\"), "{path}");
        let name = std::path::Path::new(&me)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_lowercase();
        assert!(path.to_lowercase().ends_with(&name), "{path}");
        // While held, the file cannot be opened for writing.
        let w = wide(&me);
        use windows::Win32::Storage::FileSystem::{
            CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_WRITE, FILE_SHARE_READ,
            FILE_SHARE_WRITE, OPEN_EXISTING,
        };
        // SAFETY: NUL-terminated path outliving the call.
        let writer = unsafe {
            CreateFileW(
                PCWSTR(w.as_ptr()),
                FILE_GENERIC_WRITE.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
        };
        assert!(writer.is_err(), "the locked exe must not be writable");
        drop(h);

        // Blocking the folder it really lives in refuses it, with the final path.
        let dir = std::path::Path::new(&path)
            .parent()
            .unwrap()
            .display()
            .to_string();
        let err = locked_launch_path(&me, &[dir]).err().expect("refused");
        assert_eq!(err.path.as_deref(), Some(path.as_str()));
        // A missing file is refused with the stored path.
        let gone = "C:\\definitely\\not\\here.exe";
        let err = d.launch(gone, &blocked).expect_err("refused");
        assert_eq!(err.path.as_deref(), Some(gone));
    }

    #[test]
    fn temp_dirs_are_known() {
        let dirs = WinDesk::new().temp_dirs();
        assert!(!dirs.is_empty());
        let me = std::env::current_exe().unwrap().display().to_string();
        assert!(WinDesk::new().file_exists(&me));
        assert!(!WinDesk::new().file_exists("C:\\definitely\\not\\here.exe"));
    }

    #[test]
    fn listing_windows_never_lists_our_own_and_waiting_with_nothing_is_instant() {
        let d = WinDesk::new();
        // SAFETY: plain query.
        let me = unsafe { GetCurrentProcessId() };
        for w in d.arrangeable_windows(true) {
            assert_ne!(pid_of(hwnd(w.id)), me);
        }
        assert!(d
            .wait_for_windows(&[], &[], Duration::from_secs(5))
            .is_empty());
        let start = Instant::now();
        let got = d.wait_for_windows(
            &["C:\\no\\such\\program.exe".into()],
            &[],
            Duration::from_millis(300),
        );
        assert_eq!(got, vec![None]);
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[test]
    #[ignore = "starts Notepad on the real desktop; run by hand"]
    fn launching_notepad_finds_its_first_window() {
        let d = WinDesk::new();
        let exe = format!(
            "{}\\System32\\notepad.exe",
            std::env::var("SystemRoot").unwrap()
        );
        let known: Vec<WindowId> = d.arrangeable_windows(true).iter().map(|w| w.id).collect();
        d.launch(&exe, &d.blocked_dirs()).expect("notepad starts");
        // Windows 11's Notepad is a packaged app whose process path differs; accept
        // either the classic path or nothing on such systems.
        let got = d.wait_for_windows(&[exe], &known, Duration::from_secs(10));
        println!("found: {got:?}");
        // Windows 11's Notepad is a packaged app: the System32 stub starts a process
        // with another path, so the wait (by path) may find nothing. Then take the
        // new window whose program is called notepad.exe, to clean up.
        let w = got[0].or_else(|| {
            std::thread::sleep(Duration::from_secs(2));
            d.arrangeable_windows(false)
                .into_iter()
                .find(|w| {
                    !known.contains(&w.id)
                        && w.exe
                            .as_deref()
                            .is_some_and(|e| e.to_lowercase().ends_with("notepad.exe"))
                })
                .map(|w| w.id)
        });
        let w = w.expect("a Notepad window appeared");
        d.minimize(w).unwrap();
        assert!(d.is_minimized(w));
        d.unminimize(w).unwrap();
        assert!(!d.is_minimized(w));
        // SAFETY: asks the window we started to close.
        let _ = unsafe {
            windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                hwnd(w),
                windows::Win32::UI::WindowsAndMessaging::WM_CLOSE,
                windows::Win32::Foundation::WPARAM(0),
                LPARAM(0),
            )
        };
    }
}
