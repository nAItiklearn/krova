# Porting map: macOS vs Windows

| Concern | Windows | macOS | Limits |
|---|---|---|---|
| Move/resize windows | `SetWindowPos` (SWP_NOZORDER\|NOACTIVATE), restore if maximized (`ShowWindow SW_RESTORE`) | `AXUIElementSetAttributeValue` kAXPositionAttribute / kAXSizeAttribute | Win: UIPI blocks elevated targets unless Krova is elevated. Mac: needs Accessibility permission; some apps (fixed-size) clamp |
| Visible frame | `DwmGetWindowAttribute(DWMWA_EXTENDED_FRAME_BOUNDS)` vs `GetWindowRect` → compensate invisible 7–8px borders | AX frame is the visible frame | |
| Drag detection | `SetWindowsHookEx(WH_MOUSE_LL)` + `GetAsyncKeyState` for modifier; drag = left down on a top-level window caption/body then movement > threshold, confirmed by `GetGUIThreadInfo` move-size state | `CGEventTap` (listen-only) or `NSEvent addGlobalMonitor` for leftMouseDragged + `NSEvent.modifierFlags` | Hook thread must be fast (<1 ms, Windows drops slow hooks); keep it a lock-free channel send |
| Monitors | `EnumDisplayMonitors`, `GetMonitorInfo` (work area), `GetDpiForMonitor`, `EnumDisplayDevices` + registry EDID for stable id | `NSScreen.screens`, `visibleFrame`, `CGDisplayVendorNumber/ModelNumber/SerialNumber`, `CGDisplayCreateUUIDFromDisplayID` | Serial may be absent → fallback chain |
| DPI | Per-Monitor-V2 manifest; coordinates in physical px | points; backing scale per screen | Mixed-DPI: always convert via monitor rect, never global scale |
| Overlay | Layered, transparent, click-through (`WS_EX_LAYERED\|TRANSPARENT\|TOOLWINDOW\|NOACTIVATE\|TOPMOST`) | Borderless transparent `NSWindow` at `.statusBar` level, `ignoresMouseEvents`, `collectionBehavior` canJoinAllSpaces | Webview overlay can lag; native GDI/D2D overlay preferred on Windows (ADR-001) |
| Fullscreen | `SHQueryUserNotificationState` / window covers monitor → skip | `AXFullScreen` attribute → skip | |
| Minimized | `IsIconic` → skip | `AXMinimized` → skip | |
| Virtual desktops/Spaces | `IVirtualDesktopManager::IsWindowOnCurrentVirtualDesktop` | Only on-screen windows via `CGWindowListCopyWindowInfo(onScreenOnly)` | |
| Autostart | `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` | `SMAppService.mainApp.register()` | Task Scheduler needed only for elevated autostart (not implemented) |
| Tray | Tauri tray | Tauri tray (NSStatusItem) | |
| Permissions | none (elevation limit) | Accessibility (`AXIsProcessTrustedWithOptions`) and, for CGEventTap, Input Monitoring | Must show onboarding with deep link to System Settings |
| Elevated windows | Skipped; documented. Optional "Run as administrator" setting is a non-goal | n/a | |
