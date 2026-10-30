# Krova — feature specification

Free, MIT, offline window zone manager for Windows 10/11 and macOS 13+.

## 1. Core behavior
1. **Zones.** A *layout* is a list of zones; each zone is a rectangle in *normalized* coordinates (0..1 of the monitor work area) so layouts survive resolution/DPI changes.
2. **Editor.** Per monitor, full-screen overlay editor: split a zone horizontally/vertically at a cursor position, drag dividers, delete/merge zones, apply templates (halves, thirds, grid 2x2, priority-grid, columns), name the layout.
3. **Snap by drag.** While the user drags a window and holds the activation modifier (default Shift on Windows, Shift on macOS; configurable, Ctrl/Alt/Shift/Cmd), zones of the monitor under the cursor are shown. The zone under the cursor is highlighted. Releasing the mouse button snaps the window to that zone's pixel rect. Releasing the modifier mid-drag hides zones and cancels snapping.
4. **Overlap rule.** If zones touch, the smaller-area zone wins at a point on a shared edge; hit-testing is half-open `[x, x+w)`.
5. **Per-monitor layouts.** Each monitor stores its own layouts and active layout id, keyed by stable hardware id (EDID manufacturer+product+serial where available; fallback device path; fallback resolution+position). Reconnecting a monitor restores its layouts. Matching is scored (see krova-core `monitor`).
6. **Edge snapping (optional).** Dragging a window so the cursor reaches a screen edge/corner snaps to half/quarter. Never triggers on a border shared by two monitors (the edge is considered internal if another monitor's rect abuts it at that cursor position within tolerance).
7. **Move behavior.** Setting: animate (≈120 ms ease-out) or instant.
8. **Robustness.** Clamp to windows' min/max size; skip minimized, fullscreen, and elevated windows (documented); mixed-DPI conversion per monitor; work area excludes taskbar/menu bar/dock; Windows invisible-border compensation; macOS Spaces / Windows virtual desktops: only the current desktop's windows are touched.
9. **App shell.** Tray / menu-bar icon with: Enable/Disable, Edit layouts, Settings, Launch at login, Quit. No window in the taskbar by default. Dark/light follows the OS.
10. **Privacy.** No accounts, telemetry, or network calls. Window titles are never logged or stored. Settings and layouts stay in a local JSON file.

## 2. Persistence
`config.json` in the OS app-data directory. Fields: `schema_version`, `settings`, `monitors[]` (id, label, layouts[], active_layout). Atomic write (temp file + rename), migration hook per version, corrupt-file quarantine (`.bak`) and default fallback.

## 3. Settings
Activation modifier · highlight color/opacity · animate vs instant · edge snapping on/off + edge size · launch at login · enabled · theme (system/light/dark).

## 4. Learned from open-source references (behavioral)
- Normalized zones + per-monitor persistence (FancyZones) avoid DPI drift.
- Modifier-gated overlays avoid accidental snapping (FancyZones/Rectangle).
- Snap should not fight the OS native snap: Windows Snap Assist is not disabled; Krova only acts when its modifier is down.

## 5. Non-goals (v0.1)
Window stacks/tabs, hover-activation, quick actions above windows, global keyboard-shortcut layouts (planned v0.2), Linux.
