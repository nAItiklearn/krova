//! Windows: classifying windows, finding their program, raising and focusing them.
//!
//! The facts are read with cheap, non-blocking calls (window styles, the owner, the
//! class name, DWM cloaking, the process token) and handed to the OS-independent
//! [`classify`](crate::classify::classify). The program is found from the process
//! image path. **No window title is ever read.**

use std::ffi::c_void;

use krova_core::app_rules::exe_file_name;
use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, BOOL, HWND, LPARAM, RECT, TRUE};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindow, GetWindowLongW, GetWindowRect, GetWindowThreadProcessId, IsIconic,
    IsWindow, IsWindowVisible, SetForegroundWindow, SetWindowPos, ShowWindow, GWL_EXSTYLE,
    GWL_STYLE, GW_OWNER, HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE,
    SW_RESTORE, WS_CAPTION, WS_CHILD, WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_THICKFRAME,
};
use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

use crate::classify::{classify, ClassFamily, WindowFacts};
use crate::win::{
    class_name, hwnd, is_cloaked, is_own_process_window, owner_of, target_is_protected, window_id,
};
use crate::{PlatformError, WindowId, WindowProbe};

/// Classes of the shell's own windows: the desktop, the taskbars, the start menu
/// host and the system tray overflow. Never arranged.
const SHELL_CLASSES: &[&str] = &[
    "Progman",
    "WorkerW",
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
    "NotifyIconOverflowWindow",
    "Windows.UI.Core.CoreWindow",
    "XamlExplorerHostIslandWindow",
    "TopLevelWindowForOverflowXamlIsland",
];

fn class_family(class: &str) -> ClassFamily {
    if class == "#32770" {
        ClassFamily::Dialog
    } else if SHELL_CLASSES.contains(&class) {
        ClassFamily::Shell
    } else {
        ClassFamily::Other
    }
}

/// Does the window cover its monitor exactly (a borderless full-screen window)?
fn covers_monitor(h: HWND) -> bool {
    let mut r = RECT::default();
    // SAFETY: plain out-pointer calls on a window handle; they fail cleanly when the
    // window is gone.
    if unsafe { GetWindowRect(h, &mut r) }.is_err() {
        return false;
    }
    let monitor = unsafe { MonitorFromWindow(h, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        return false;
    }
    let m = info.rcMonitor;
    r.left <= m.left && r.top <= m.top && r.right >= m.right && r.bottom >= m.bottom
}

/// Read the facts [`classify`] needs about a window.
pub fn window_facts(h: HWND) -> WindowFacts {
    // SAFETY: style and visibility reads on any handle are harmless; they return 0
    // for a window that is gone.
    let style = unsafe { GetWindowLongW(h, GWL_STYLE) } as u32;
    let ex = unsafe { GetWindowLongW(h, GWL_EXSTYLE) } as u32;
    let visible = unsafe { IsWindowVisible(h) }.as_bool();
    let owner = unsafe { GetWindow(h, GW_OWNER) }
        .ok()
        .filter(|o| !o.0.is_null());
    let caption = style & WS_CAPTION.0 == WS_CAPTION.0;
    let class = class_family(&class_name(h));
    let own_process = is_own_process_window(h);
    let mut facts = WindowFacts {
        child: style & WS_CHILD.0 != 0,
        visible,
        cloaked: is_cloaked(h),
        caption,
        sizing_border: style & WS_THICKFRAME.0 != 0,
        min_max_buttons: style & (WS_MINIMIZEBOX.0 | WS_MAXIMIZEBOX.0) != 0,
        tool_window: ex & WS_EX_TOOLWINDOW.0 != 0,
        app_window: ex & WS_EX_APPWINDOW.0 != 0,
        no_activate: ex & WS_EX_NOACTIVATE.0 != 0,
        owned: owner.is_some(),
        class,
        covers_monitor: !caption && covers_monitor(h),
        elevated: false,
        own_process,
    };
    // Opening the process token is the costly part: only for windows that would
    // otherwise be arranged.
    if classify(&facts) == krova_core::app_rules::WindowKind::Normal {
        facts.elevated = target_is_protected(h);
    }
    facts
}

/// Full path of the program that owns the window, or `None` if it cannot be read
/// (a protected process). In memory only.
pub fn exe_path(h: HWND) -> Option<String> {
    let mut pid = 0u32;
    // SAFETY: out-pointer for the process id.
    unsafe { GetWindowThreadProcessId(h, Some(&mut pid)) };
    if pid == 0 {
        return None;
    }
    // SAFETY: limited query rights; the handle is closed below.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let mut buf = vec![0u16; 1024];
    let mut len = buf.len() as u32;
    // SAFETY: `buf` holds `len` UTF-16 units; the call writes at most that many.
    let read = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
    };
    let _ = unsafe { CloseHandle(process) };
    read.ok()?;
    let len = usize::try_from(len).ok()?.min(buf.len());
    Some(String::from_utf16_lossy(&buf[..len]))
}

/// Classify the window and find its program. `None` when the window is gone.
pub fn probe(window: WindowId) -> Option<WindowProbe> {
    let h = hwnd(window);
    // SAFETY: IsWindow is harmless on any handle.
    if !unsafe { IsWindow(h) }.as_bool() {
        return None;
    }
    let kind = classify(&window_facts(h));
    let exe_path = exe_path(h);
    let exe = exe_path
        .as_deref()
        .map(exe_file_name)
        .filter(|e| !e.is_empty())
        .map(str::to_owned);
    Some(WindowProbe {
        kind,
        exe,
        exe_path,
        owner: owner_of(h),
    })
}

/// Bring the window above its siblings without taking the focus.
pub fn raise(window: WindowId) -> Result<(), PlatformError> {
    let h = hwnd(window);
    // SAFETY: z-order change only; no move, no size, no activation.
    unsafe {
        if !IsWindow(h).as_bool() {
            return Err(PlatformError::WindowGone);
        }
        SetWindowPos(
            h,
            HWND_TOP,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER,
        )
        .map_err(|e| PlatformError::Os(e.to_string()))
    }
}

/// Give the window the focus (restoring it if minimized) and raise it.
pub fn activate(window: WindowId) -> Result<(), PlatformError> {
    let h = hwnd(window);
    // SAFETY: plain window-manager calls on a handle that is checked first.
    unsafe {
        if !IsWindow(h).as_bool() {
            return Err(PlatformError::WindowGone);
        }
        if IsIconic(h).as_bool() {
            let _ = ShowWindow(h, SW_RESTORE);
        }
        if !SetForegroundWindow(h).as_bool() {
            // Windows refuses the focus to a background process; at least raise it.
            return raise(window);
        }
    }
    Ok(())
}

unsafe extern "system" fn collect_window(h: HWND, data: LPARAM) -> BOOL {
    // SAFETY: `data` is the `Vec` handed in by `top_level`, alive for the call.
    let out = unsafe { &mut *(data.0 as *mut Vec<WindowId>) };
    if unsafe { IsWindowVisible(h) }.as_bool() && out.len() < 4096 {
        out.push(window_id(h));
    }
    TRUE
}

/// Visible top-level windows, front to back.
pub fn top_level() -> Vec<WindowId> {
    let mut out: Vec<WindowId> = Vec::new();
    // SAFETY: the callback only pushes into `out`, which outlives the call.
    let _ = unsafe {
        EnumWindows(
            Some(collect_window),
            LPARAM((&mut out as *mut Vec<WindowId>).cast::<c_void>() as isize),
        )
    };
    out
}

/// Give a window (by its raw handle) the system's blurred "acrylic" backdrop, with
/// "mica" as the fallback, and the dark or light title colours. Windows 11 22H2 or
/// later; returns `false` where the system has neither, so the page keeps its own
/// opaque background.
pub fn apply_backdrop(raw: isize, dark: bool) -> bool {
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMSBT_MAINWINDOW, DWMSBT_TRANSIENTWINDOW,
        DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_IMMERSIVE_DARK_MODE,
    };
    let h = HWND(raw as *mut c_void);
    // SAFETY: plain attribute writes on a window of this process; each value lives
    // on the stack for the duration of its call.
    unsafe {
        if !IsWindow(h).as_bool() {
            return false;
        }
        let dark_mode = i32::from(dark);
        let _ = DwmSetWindowAttribute(
            h,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            (&dark_mode as *const i32).cast(),
            std::mem::size_of::<i32>() as u32,
        );
        for kind in [DWMSBT_TRANSIENTWINDOW, DWMSBT_MAINWINDOW] {
            let value = kind.0;
            if DwmSetWindowAttribute(
                h,
                DWMWA_SYSTEMBACKDROP_TYPE,
                (&value as *const i32).cast(),
                std::mem::size_of::<i32>() as u32,
            )
            .is_ok()
            {
                return true;
            }
        }
    }
    false
}
/// Whether Windows is set to dark mode for apps.
pub fn prefers_dark() -> bool {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize")
        .and_then(|k| k.get_value::<u32, _>("AppsUseLightTheme"))
        .map(|light| light == 0)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_families() {
        assert_eq!(class_family("#32770"), ClassFamily::Dialog);
        assert_eq!(class_family("Shell_TrayWnd"), ClassFamily::Shell);
        assert_eq!(class_family("Chrome_WidgetWin_1"), ClassFamily::Other);
    }

    #[test]
    fn a_gone_window_has_no_probe() {
        assert_eq!(probe(WindowId(0)), None);
        assert!(raise(WindowId(0)).is_err());
    }

    #[test]
    fn the_shell_windows_are_listed_and_classified_without_reading_titles() {
        let windows = top_level();
        assert!(!windows.is_empty(), "an interactive session has windows");
        for w in windows.iter().take(50) {
            // Any window may vanish between the two calls; that is fine.
            if let Some(p) = probe(*w) {
                if let Some(exe) = &p.exe {
                    assert!(!exe.contains('\\'), "a file name, not a path: {exe}");
                }
            }
        }
    }

    #[test]
    #[ignore = "opens Notepad; needs an interactive desktop session"]
    fn notepad_is_a_normal_window_of_notepad_exe() {
        use std::process::Command;
        use std::time::{Duration, Instant};
        let mut child = Command::new("notepad.exe").spawn().expect("notepad starts");
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut found = None;
        while Instant::now() < deadline && found.is_none() {
            std::thread::sleep(Duration::from_millis(200));
            found = top_level().into_iter().find(|w| {
                probe(*w).is_some_and(|p| {
                    p.exe
                        .as_deref()
                        .is_some_and(|e| e.eq_ignore_ascii_case("notepad.exe"))
                })
            });
        }
        let w = found.expect("a notepad window");
        let p = probe(w).unwrap();
        assert_eq!(p.kind, krova_core::app_rules::WindowKind::Normal);
        raise(w).unwrap();
        let _ = child.kill();
        let _ = child.wait();
    }
}
