# Features branch report

Branch `worktree-agent-aac27475ab9e2e2de` (see `git branch --show-current` in the worktree). Nothing is pushed, tagged or released. The interface to the UI is `docs/features-contract.md`.

Everything here was built and run on one Windows 11 machine (10.0.26300, one 1536x864 display at 125%, Windows-GNU toolchain). **macOS code was not touched** (new `Platform` methods have `Unsupported`/`false` defaults, so `mac.rs` and `NullPlatform` need no edits). Nothing was compiled for macOS.

## Status per work package

| Package | State | What exists |
|---|---|---|
| T0 window destroy-on-close | **done, measured** | `"create": false` in `tauri.conf.json`; `ui_window.rs` creates the window on demand and destroys it on close; `RunEvent::ExitRequested` is vetoed only when it carries no exit code, so the tray's Quit (`app.exit(0)`) still exits. |
| T1 `registry` (core) | **done, unit-tested** | `SnapRegistry` (bounded, 1024), `plan_reconnect`, zone selection helpers, `plan_keyboard_move`. |
| T2 engine, reconnect memory | **done, unit-tested, exercised end to end with a faked second monitor** | snaps (zone and keyboard) are recorded; a drag of a registered window removes its entry; display-change watcher plus a press-time check; 1.5 s debounce; restore through `place_window`. |
| T3 keyboard zone control | **done; registration measured, delivery measured for Ctrl+Alt (see below)** | `RegisterHotKey` thread, live re-registration on settings change, status in `AppState`. |
| T4 linked dividers | **done, unit-tested; real-mouse test results below** | `SetWinEventHook` (MOVESIZESTART/END), `krova_core::linked` planner, applied when the mouse button is released. No live following. |
| T5 layout files | **done, unit-tested** | `layout_io` plus `export_layout` / `import_layout` commands. |
| T6 zone gap | **done, unit-tested** | `Layout::zone_pixels_gapped`, used by the snap session, keyboard moves, reconnect, linked dividers and the overlay (`OverlayScene::gapped`). |
| T7 docs | **done** | this file, `docs/features-contract.md`. `SECURITY-REVIEW.md` is untouched. |

## Decisions the owner should know about

1. **Default shortcut modifier is `ctrl_alt_shift`, not `win_alt`.** Measured with a plain Python `RegisterHotKey` probe (no Krova code) on this machine: **Win+Alt: all 12 refused (error 1409)**; **Win+Ctrl: 11 of 12 refused** (only Win+Ctrl+Up is accepted); **Ctrl+Alt: 0 refused; Ctrl+Alt+Shift: 0 refused.** Windows itself keeps Win+Alt+digit (taskbar jump lists) and the Win+Alt/Win+Ctrl arrows. So the brief's default would have shown a dead feature on the owner's own machine. `win_alt` and `win_ctrl` are still selectable; a refused chord is reported in `AppState.shortcuts.failed`. One-line revert: `#[default]` on `ShortcutModifier::WinAlt` and `Settings::default()` in `crates/krova-core/src/config.rs`, then fix the three tests that name the default, and the contract.
2. **Linked dividers move a whole divider line, like the layout editor does.** In a 2x2 grid, dragging the vertical seam moves all four windows' edges; windows whose edge is merely near the line, or not connected to the dragged stretch of it, stay.
3. **Neighbours follow when the mouse is released, not live.** Live following needs `EVENT_OBJECT_LOCATIONCHANGE` with throttling and was not attempted.
4. **A resize that cannot be linked removes the registry entry** (the user placed the window elsewhere); so does a move, a maximize, or a drag. Reconnect then leaves that window alone.
5. **The registry stores the rect the window really got** (it can be bigger than the zone if the window has a minimum size). Reconnect treats a window within 2 px of either the zone rect or the stored rect as already in place, so it is never pushed twice.
6. **Closing the window destroys it.** `DESTROY_ON_CLOSE` in `src-tauri/src/ui_window.rs` flips it back to hide-on-close. `navigate` events for a window that is still loading are parked and delivered 250 ms after the page load event; the UI may also call the new `take_pending_view` command. A second launch accepts `--settings` to open the Settings view (any other argument opens the editor).
7. **Test profile override** in `src-tauri/Cargo.toml` (`[profile.test.package.krova-app] opt-level = 2`). Without it the `src-tauri` unit-test executable could not start on Windows-GNU (`STATUS_ENTRYPOINT_NOT_FOUND`): in debug builds rustc shares generic code between crates, a test that merely dropped a `Vec<String>` resolved to the `tauri` crate's copy, which pulled the whole webview stack (and its comctl32 v6 import `TaskDialogIndirect`) into a test executable that has no manifest. Found with a linker map, not guessed. The baseline 17 tests passed on a fresh clone of `HEAD` before my changes (checked), so this was introduced by my new code and is contained.
8. **App manifest** `src-tauri/windows/app.manifest` (per-monitor DPI v2, `asInvoker`, Common Controls v6) replaces tauri's default via `build.rs`. tao also requests per-monitor v2 at run time. The linker warning `.rsrc merge failure: multiple non-default manifests` still appears, as before.

## What was verified at runtime, and what was not

### T0: memory (isolated test build, identifier `dev.krova.test`, own target dir; the owner's installed instance, pid 17084, was untouched throughout)

Method: a separate debug binary built with `TAURI_CONFIG='{"identifier":"dev.krova.test"}'` (verified: the string `dev.krova.test` is in the binary, `dev.krova.desktop` is not), run, the editor opened through the single-instance second launch, closed with `WM_CLOSE` to its window, reopened, etc. Memory read with `GetProcessMemoryInfo` (private bytes and working set) for the app and all its descendants. `docs/measurements/measure-idle.ps1` was extended (`-ExePath`, `-Identifier`, `-Samples`, `-Label`) but **could not be run here: this environment refused to run PowerShell, so the numbers below came from an equivalent Python probe** and the extended script is untested by me.

| State | Processes | Private MB (all) | Working set MB (all, double-counts shared pages) |
|---|---|---|---|
| A: just started, window never opened | 1 | 3.0 | 18 |
| B: editor open | 7 (app + 6 `msedgewebview2.exe`) | 199.9 | 403 |
| C: window closed with the X | **1** | **6.3** | **36.5** |
| D: reopened with `--settings` | 7 | 199.5 | 389 |
| E: closed again | 1 | 6.3 | 36.7 |
| F: reopened again | 7 | 203.5 | 393 |

After close the app stays alive in the tray and the six WebView2 processes are gone; reopening works repeatedly. Idle CPU with no window: 0.39% and 0.78% of one core in two 4 s samples (noise level; the tray and hooks are idle). The owner's earlier measurement (window open): 230 MB private, 473 MB working set across 7 processes. A screenshot of the recreated window on `--settings` showed the Settings view, so the parked `navigate` event was delivered. Not tested: the tray menu items themselves (a script cannot click the tray), Quit from the tray.

### T3: hotkeys

See the table in the section "Hotkey measurements" below (filled from the last desktop run).

### Things only unit-tested

Reconnect planning against real monitor changes (this machine has one monitor; the end-to-end test fakes a second one), linked dividers with windows that refuse a size, `import_layout`/`export_layout` through the real Tauri IPC (the commands are tested as plain functions), the macOS build of anything.

## Known limits and risks

- Windows 11 restores window positions itself on re-dock; Krova then sees the window already near its zone and does nothing, or puts it into the zone if it is not.
- Reconnect memory and linked dividers apply to windows Krova snapped in this session. Nothing is persisted (by design).
- A window that refuses a size (Notepad cannot be narrower than about 400 px) ends up larger than its zone; the registry remembers the real rect.
- The press-time monitor check and the display watcher both feed the same debounce; a dock that flickers for more than 1.5 s without pause could delay the restore.
- Elevated windows are skipped as before; the shortcuts never act on Krova's own windows, the desktop, the taskbar or minimized windows.
- Real-mouse and real-keyboard tests take over the input for a few seconds; they are `#[ignore]` for that reason.
- `win_events.rs` message-loop threads are stopped with an event handle that the owner keeps open until the thread is joined (no thread-id messaging). If a thread fails to start within 2 s the event handle is leaked on purpose.

## New unsafe blocks and attack surface

- `crates/krova-platform/src/win_events.rs` (all with SAFETY comments): `CreateEventW/SetEvent/CloseHandle` for the stop event; `MsgWaitForMultipleObjectsEx/PeekMessageW/TranslateMessage/DispatchMessageW` for the loop; `RegisterHotKey/UnregisterHotKey` (global shortcut registration, thread bound); `SetWinEventHook/UnhookWinEvent` (out-of-context, `WINEVENT_SKIPOWNPROCESS`, only `EVENT_SYSTEM_MOVESIZESTART/END`; the callback reads only the window handle and enqueues); a hidden top-level window (`KrovaDisplayWatch`) that receives `WM_DISPLAYCHANGE`, `WM_SETTINGCHANGE` (work area) and resume-from-sleep broadcasts. Two `unsafe impl Send/Sync` for the event handle wrapper.
- `crates/krova-platform/src/win.rs`: `IsWindow`, `IsIconic`, `IsZoomed`, `GetForegroundWindow`, `GetWindowThreadProcessId` / `GetCurrentProcessId` (to skip Krova's own windows), all read-only; no window title is read.
- New Tauri commands: `export_layout` (validates, returns text), `import_layout` (64 KiB cap, strict envelope, validated, unique id), `take_pending_view` (returns one of two fixed strings). The second-launch argument `--settings` can only select between two fixed views.
- New in-memory state: registry of window handle values, zone ids and rects. No titles, no process names, nothing written to disk except `config.json` fields from the contract and layout files the user exports.
- Test-only: `src-tauri` gets a Windows dev-dependency on the `windows` crate (same version as `krova-platform`; the lockfile gains one edge).

## Try each feature by hand (about two minutes each)

Build an isolated copy first so the installed Krova is not disturbed: `TAURI_CONFIG='{"identifier":"dev.krova.test"}' CARGO_TARGET_DIR=<somewhere> cargo build --manifest-path src-tauri/Cargo.toml --features tauri/custom-protocol` (UI built first with `npm run build --prefix ui`). Start `krova-app.exe`; start it again to open the editor, or with `--settings` for Settings.

1. **Memory fix.** Open the window, then close it with the X: in Task Manager the `msedgewebview2.exe` children disappear and Krova stays in the tray. Tray, Edit layouts reopens it.
2. **Keyboard.** Focus any ordinary window (not elevated). Press Ctrl+Alt+Shift+2: it goes to zone 2 of its monitor. Ctrl+Alt+Shift+Right / Left: next / previous zone, wrapping. Ctrl+Alt+Shift+Up: next monitor, same zone number.
3. **Reconnect.** Snap a window to a zone on the external monitor (Shift-drag), unplug the monitor, plug it back, wait two seconds: the window returns to its zone. Drag the window yourself first and it is left alone.
4. **Linked dividers.** Snap two windows side by side (halves). Drag the edge between them with the mouse and release: the other window's edge follows.
5. **Layout files.** (UI pending.) With the commands: `export_layout` returns JSON; `import_layout` stores it on a monitor.
6. **Zone gap.** Set `zone_gap_px` in `config.json` (for example 10) and restart: the overlay and the snapped windows leave a 10 px gap (UI control pending).

## Manual test checklist for the owner

- [ ] Close the window with the X, check the tray icon stays and Task Manager shows no WebView2 children; reopen from the tray.
- [ ] Tray Quit still exits the process (not tested by a script).
- [ ] Ctrl+Alt+Shift+1..9, Left, Right, Up on a two-monitor setup; check the layouts differ per monitor.
- [ ] Turn "Enabled" off in the tray: the shortcuts stop working (they are unregistered).
- [ ] Dock / undock with windows snapped; sleep and wake.
- [ ] Linked dividers on a window that has a large minimum size (neighbour stops at its minimum).
- [ ] Mixed-DPI setup: the per-monitor v2 manifest and the zone maths on a 100% and a 150% monitor.
- [ ] Antivirus / SmartScreen behaviour of a build that now registers hotkeys and a WinEvent hook.
