# Features contract: Rust backend and UI

This is the interface between the Rust side (this branch) and the Svelte UI (built in parallel from this document). Serde names are stable: change them only together with this file.

Everything is additive. A v0.1 `config.json` loads unchanged: every new setting has a serde default, and `schema_version` stays `1`.

## Settings (in `config.settings`, sent to and from the UI)

| Field | Type | Default | Notes |
|---|---|---|---|
| `keyboard_shortcuts` | bool | `true` | Global shortcuts that move the focused window between zones. |
| `shortcut_modifier` | string enum | `"ctrl_alt_shift"` | One of `"ctrl_alt"`, `"win_alt"`, `"ctrl_alt_shift"`, `"win_ctrl"`. **Changed from the brief's `"win_alt"`**: measured with `RegisterHotKey` on Windows 11 10.0.26300, Windows refuses all twelve Win+Alt chords (error 1409) and eleven of twelve Win+Ctrl chords, while Ctrl+Alt and Ctrl+Alt+Shift are accepted for all twelve. Ctrl+Alt+Left/Right collide with IDE back/forward, so Ctrl+Alt+Shift is the default. Offer `win_alt` / `win_ctrl` in the UI only with the hint that Windows may keep them: `shortcuts.failed` says which. |
| `restore_on_reconnect` | bool | `true` | When a monitor comes back, windows that Krova had snapped there return to their zones. |
| `linked_dividers` | bool | `true` | Resizing a snapped window by an edge moves the shared edge of its snapped neighbours too (applied when the mouse is released). |
| `zone_gap_px` | int | `0` | Space between zones and around the work area, in pixels. Clamped to `0..=32` on load and on save. It applies to zone snaps, the keyboard shortcuts, the reconnect restore and the drag overlay. Edge snaps (half / quarter) ignore it. |
| `layout_switcher_shortcut` | bool | `true` | Global `<shortcut_modifier>+Space` opens the layout switcher on the monitor under the cursor (see "Layout switcher" below). Independent of `keyboard_shortcuts`: either can be on without the other. Uses the same `shortcut_modifier`. |

Settings are sanitised in Rust (`Settings::sanitized`), both when the config is loaded and when the UI saves them, so the UI never has to trust its own clamping.

## `AppState` additions

```jsonc
{
  "shortcuts":      { "active": true, "failed": ["Win+Alt+3 is already used by another app"] },
  "capabilities":   { "shortcuts": true, "window_memory": true },
  "layout_switcher": { "active": true, "failed": [] }
}
```

- `shortcuts.active`: the shortcuts are registered with the OS (at least one of the twelve was accepted).
- `shortcuts.failed`: one human-readable sentence per refused shortcut. Empty when all twelve were accepted or when the feature is off. Show it as is.
- `layout_switcher`: the same two fields for the layout switcher's own shortcut, `<mods>+Space`. `active` is true when the OS accepted it; `failed` holds at most one sentence ("Ctrl+Alt+Shift+Space is already used by another app"). Both are empty/false while `layout_switcher_shortcut` is off or Krova is disabled. It is separate from `shortcuts` on purpose: switching `keyboard_shortcuts` off leaves `shortcuts` inactive even when the switcher's chord is registered.
- `capabilities.shortcuts`: this OS build can register global shortcuts (true on Windows, false elsewhere in this branch).
- `capabilities.window_memory`: this OS build can read window positions, which reconnect memory and linked dividers need (true on Windows, false elsewhere). The UI should hide or disable the corresponding settings when `false`.
- The state is pushed (`state-changed`) after every settings change, so a shortcut refusal shows up as soon as the user picks a modifier.

## Commands

All errors are plain strings.

| Command | Arguments | Returns | Notes |
|---|---|---|---|
| `export_layout` | `layout: Layout` | `string` | Pretty JSON envelope `{"krova_layout":1,"layout":{...}}`. The layout is validated first (zone count, text length, rectangles, duplicate ids). The UI chooses where to save the text. |
| `import_layout` | `monitorId: string`, `json: string` | `AppState` | At most 64 KiB, envelope version 1 only, same limits as `save_layout`. If the layout id already exists on that monitor it gets a numeric suffix (`halves` becomes `halves-2`). Goes through `Config::save_layout`, so the 64-layouts-per-monitor cap applies. Persists and emits `state-changed`. The imported layout is **not** activated. |
| `take_pending_view` | none | `string \| null` | Optional pull path for the window lifecycle (below). May now also return `"switcher"`. |
| `move_layout` | `monitorId: string`, `layoutId: string`, `toIndex: number` | `AppState` | Reorder: the layout ends up at index `toIndex` of the monitor's list, the others shift by one. `0 <= toIndex < layouts.length`, otherwise `"position N is out of range (the monitor has M layouts)"` (the caller's picture of the list is stale: refresh). Moving a layout to where it already is succeeds. The active layout is tracked by id and stays active. Unknown monitor or layout is an error. Persists and emits `state-changed`. |
| `insert_layout` | `monitorId: string`, `layout: Layout`, `index: number`, `activate: bool` | `AppState` | The undo of `delete_layout`: keep the deleted `Layout` (and the index it had, and whether it was the active one), and put it back in one step. `0 <= index <= layouts.length` (`length` appends). Validated like `save_layout` and subject to the 64-layouts cap; unlike `save_layout` it **never replaces**: a layout id the monitor already has is `"layout X already exists"`. With `activate` it also becomes the monitor's active layout. Any error changes nothing. Persists and emits `state-changed`. |
| `switcher_monitor` | none | `string \| null` | `MonitorProfile.id` of the monitor the layout switcher is for (below), or `null`. |

Existing commands are unchanged. (`save_layout` + `move_layout` + `set_active_layout` could also undo a delete, but as three persists and three `state-changed` pushes, with the layout visibly at the end of the list in between; `insert_layout` does it atomically.)

## Keyboard map (display text)

With the chosen modifier (`<mods>`, by default `Ctrl+Alt+Shift`):

- `<mods>+Left` / `<mods>+Right`: move the focused window to the previous / next zone (wraps around).
- `<mods>+1` ... `<mods>+9`: move it to zone N.
- `<mods>+Up`: send it to the next monitor, keeping its zone number (if the other layout has fewer zones, the last one).
- `<mods>+Space`: open the layout switcher (its own setting, `layout_switcher_shortcut`).

"Zone order" is reading order: top to bottom by the zone's `y` rounded to 1%, then left to right. Zone N is the N-th zone in that order, not the zone's id.

The shortcuts act on the window that has the focus. They do nothing for the desktop, the taskbar, minimized windows, windows of programs running as administrator, and Krova's own windows. A maximized window is restored and moved.

## Layout files

```json
{ "krova_layout": 1, "layout": { "id": "mine", "name": "Mine", "zones": [ { "id": 1, "rect": { "x": 0, "y": 0, "w": 0.3, "h": 1 } } ] } }
```

Coordinates are proportions of the work area, so a file works on any monitor and on both operating systems. Import is strict: exactly the two top-level keys, version 1, at most 64 KiB, at most 64 zones, ids and names at most 64 characters without control characters, every side at least 0.01.

## Window lifecycle (not a UI change, but visible to the UI)

The window is created on demand and **destroyed** when closed, so the embedded browser exits while Krova sits in the tray. The UI should not rely on module state surviving a close.

- `navigate` events (`"editor"`, `"settings"`, or `"switcher"`; TS type `NavigateTarget`) emitted while the window is being created are parked and delivered about 250 ms after the page finished loading, when the page has had time to register its listener. A UI that wants to be exact can call `take_pending_view` once, after it has registered its `navigate` listener: it returns the parked view, or `null` if none is waiting (the push is then a no-op because the view was already taken).
- The `main` window entry in `tauri.conf.json` has `"create": false`; the capability file still names the `main` label.

## Layout switcher (backend done, view not built)

A quick overlay listing the saved layouts of one monitor as stacked cards: tap or Enter activates, swipe or Delete deletes (with undo), drag or Alt+Up/Down reorders. The interaction prototype (`docs/prototypes/layout-switcher/index.html`, a standalone page outside the app build) is not committed yet and waits for the owner's approval; the Svelte view is built only after that.

**Opening.** `<mods>+Space` (setting `layout_switcher_shortcut`, status `AppState.layout_switcher`). On a press the engine refreshes the monitor list, reads the cursor position, and picks the monitor whose rect contains it; if the position cannot be read or is on no monitor it falls back to the primary monitor, then the first connected one; with no monitor connected nothing opens. It remembers that monitor's profile id and brings the UI window forward with `navigate` = `"switcher"`. It does nothing while Krova or `layout_switcher_shortcut` is off, whatever `keyboard_shortcuts` says.

**What the UI does on `navigate("switcher")`** (the event, or `take_pending_view` for a window that was still being created):

1. `switcher_monitor` gives the `MonitorProfile.id`. If that monitor was unplugged since, it returns the primary one instead, so a non-null answer is always connected. Every press stores a fresh answer and sends `navigate` again, so a view that is already showing may treat the second press as "close" or "move to the new monitor".
2. The cards are `state.config.monitors[id].layouts` in order, `active_layout` marks the active one, and the entry of `state.monitors` with the same `profile_id` has the `label` and `rect` for the header ("Display 1 · 2560 x 1440").
3. Actions are existing commands, always with that monitor id: activate = `set_active_layout`; delete = `delete_layout` (a monitor keeps at least one layout, so the last card cannot be deleted; deleting the active layout activates the first remaining one); undo = `insert_layout` with the deleted `Layout`, its old index and whether it was active; reorder = `move_layout` with the card's final index (one call when the drag or Alt+Up/Down ends is enough).
4. A UI that does not know `"switcher"` must ignore the navigate (the current one does: `app.navigate` only accepts `editor` and `settings`).

**Shortcut availability, measured** with `RegisterHotKey` on Windows 11 10.0.26300 (ignored tests `space_chords_probe` and `global_hotkeys_register_and_deliver_for_every_modifier`): `Ctrl+Alt+Shift+Space` (the default) and `Win+Alt+Space` were accepted and a real key press of each was delivered as the switcher request; `Win+Ctrl+Space` was refused (in use). `Ctrl+Alt+Space` was accepted in one run and refused in a later run on the same PC, because something else running there took it (it is a common quick-entry shortcut), which is what `layout_switcher.failed` is for. The switcher's chord is registered on the same thread as the other twelve, so it costs no extra thread.

**Window.** Today `navigate("switcher")` goes to the `main` window like the other views. Recommended for the real view: a separate small window (label `switcher`, transparent, undecorated, always on top, no taskbar entry, created on demand and destroyed on close like `main`), so opening it never replaces an editor that has unsaved edits, closing it never takes the editor with it, and its size and position come from the target monitor. The contract above (events, commands, state) does not change with that; only `ui_window.rs`, `tauri.conf.json` and the capability file do.

## Wave 3: workspaces, focus dim, calm desk, share-safe, window highlight (branch `wave3`)

All additions; a config without them loads unchanged and `schema_version` stays `1`.

**Settings** (`config.settings`, sanitised in Rust like the rest):

| Field | Type | Default | Notes |
|---|---|---|---|
| `focus` | `{ enabled: bool, opacity: number }` | `{ false, 0.45 }` | Focus dim. `opacity` clamped to `0.2..=0.8`. Toggle with `toggle_focus_dim` (the engine binds `Ctrl+Alt+F` to `wave3::toggle_focus` later). |
| `calm_desk` | `{ all_monitors: bool }` | `{ true }` | `false`: only the monitor under the cursor. |
| `share_safe` | bool | `false` | While on, Krova's overlays, previews and switcher show only zone shapes, never app names, icons or window contents. Rust code asks `wave3::share_safe(&Shared)` (native overlay code: `krova_platform::desk::share_safe()`); the UI reads `AppState.wave3.share_safe`. On Windows every native overlay window (snap overlay, focus dim, highlight rings) is also excluded from screen capture (`SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)`, `WDA_NONE` again when off; F-29). That is defence in depth: any overlay that draws app names or icons must still check `share_safe()`. |
| `highlight` | `{ mode: "none" \| "active" \| "all" \| "glow", color: "#rrggbb" \| null, width_px: 1..6 }` | `{ "none", null, 2 }` | `color: null` = the appearance accent (`#ff5b1f` until `settings.appearance.accent` exists; read through one function, `krova_core::focus::appearance`). On Windows 11 (build 22000+) `active` and `all` with `width_px <= 2` use the border colour Windows draws itself (`DWMWA_BORDER_COLOR`, about 1 px); wider borders, older builds and `glow` use a click-through overlay. |

**Config:** `config.workspaces: Workspace[]` (at most 32; invalid ones are dropped on load).

```jsonc
{ "id": "ws-1", "name": "Coding",
  "layouts": [ { "monitor_id": "monitor-1", "layout_id": "halves" } ],  // active layout per monitor at capture
  "entries": [ { "exe": "C:\\Program Files\\Code\\Code.exe", "monitor_id": "monitor-1", "zone_id": 1 } ] }
```

Names follow the layout rules: 1-64 characters after trimming spaces, no control characters, no zero-width characters (U+200B-200D, U+2060, U+FEFF), no direction controls (U+202A-202E, U+2066-2069), no U+2028/2029. At most 64 entries. An entry is only an executable path, a monitor profile id and a zone id: never a window title, never command-line arguments.

**`AppState.wave3`** (TS `Wave3State`): `capabilities { workspaces, calm_desk, focus_dim, highlight, dwm_borders }` (all `false` on macOS for now), `calm_desk { active, count }`, `focus_active` (dim on and its overlays running), `share_safe`, `visuals_error` (why the overlays could not start, or `null`), `restoring` (id of the workspace being restored, or `null`).

**Commands** (errors are plain strings; each persists and emits `state-changed`):

| Command | Arguments | Returns | Notes |
|---|---|---|---|
| `toggle_focus_dim` | none | `AppState` | Flips `settings.focus.enabled`. |
| `toggle_calm_desk` | none | `AppState` | On: minimises (never closes) every visible arrangeable window that is neither snapped by Krova into the active layout nor sitting in one of its zones (12 px tolerance, plain or gapped rects). Off: restores exactly the windows calm desk minimised that are still minimised. Memory only, never persisted. |
| `capture_workspace` | `name: string`, `id: string \| null` | `AppState` | Visible arrangeable windows, topmost first; each is mapped to its zone through the snap registry, else by the largest overlap with a zone of its monitor's active layout. Windows in no zone are skipped. With `id`, replaces that workspace. |
| `rename_workspace` | `id`, `name` | `AppState` | |
| `delete_workspace` | `id` | `AppState` | |
| `restore_workspace` | `id`, `launch: bool` | `RestoreReport` | Runs off the UI thread; one restore at a time. Activates the recorded layouts (if they still exist); reuses one open window per entry with the same executable (paths compared without case; minimised windows count and are restored). With `launch`, starts a missing program with `ShellExecuteExW` (no arguments, from its own folder) only if the captured path is a local `X:\...` path ending in `.exe`, without `..`, without an alternate data stream (no `:` after the drive), with no folder or file name ending in `.` or a space, not on a network share, not under a blocked folder (`%TEMP%`, `%TMP%`, `%LOCALAPPDATA%\Temp`, `%SystemRoot%\Temp`, any folder called `Temp` or `Tmp`, Downloads, Desktop, `%APPDATA%` Roaming; compared without case, also as final paths) and the file still exists. Before starting it, Krova opens the file without write or delete sharing, resolves its final path (`GetFinalPathNameByHandleW`), runs the same check again on that path, requires a fixed drive (`GetDriveTypeW`, no removable or network drives) and starts exactly that final path while still holding the file (F-26); then waits up to 10 s, event-driven (WinEvent hook), for each launched program's first arrangeable window. Places the windows one after another with the normal (animated) move and records them in the snap registry; minimises every other arrangeable window. Pushes `state-changed` at the end. |

`RestoreReport = { workspace_id, minimized: number, entries: [{ program, monitor_id, zone_id, outcome, path? }] }`; `program` is the file name only (`Code.exe`); `path` is present only for `refused` entries and is the full path that was refused (the final path when the file could be opened, else the stored one); `outcome` is one of `placed`, `launched`, `timed_out`, `not_running`, `refused`, `no_zone`, `failed`.

**Focus dim** draws one layered, click-through, no-activate, topmost black window per monitor at the configured opacity, with a rounded hole (radius `settings.appearance.corner_radius_px`, default 10 px, DPI-scaled) where the focused window is. It fades, and the hole moves between windows, with the shared spring (damping 0.88, settled after 220 ms); the 16 ms frame timer exists only while a spring moves, so it costs nothing while idle. It follows focus and the focused window's moves, and fades out when the focus is on the desktop, the taskbar or a minimised window.

**Highlight overlay** (when Windows does not draw it): a click-through ring following the focused window, eased between windows on focus change, shaped for Windows 11's rounded corners (8 px at 100 %); `all` puts one ring just above each window (at most 48; unfocused ones fainter); `glow` is a soft per-pixel-alpha halo. Hidden for full-screen, minimised and cloaked windows. Switching off resets every border colour Krova set. The overlay thread runs only while focus dim or a highlight is on, and is per-monitor DPI aware.

**Known limits:** packaged (Microsoft Store) apps report a path under `WindowsApps` that cannot be started directly, so restore reuses them when they are open and otherwise reports `refused` or `timed_out`. Programs running as administrator are not listed (Krova cannot move them). The Svelte views (workspace cards in the switcher deck, quick-panel toggles) are not built.
## Notch (Windows; `capabilities.notch`)

A small ink pill at a screen edge, the main way to control Krova without opening the app. Native window (layered, topmost, never activated, per-pixel alpha), no WebView while collapsed, no idle CPU (a frame timer runs only while it animates).

**Setting** `notch` (object in `config.settings`; TS `NotchSettings`). Every field has a default and a bad value falls back to its default on its own, so a hand-edited file never makes the config unreadable.

| Field | Type | Default | Notes |
|---|---|---|---|
| `enabled` | bool | `true` | The menu's "Hide notch" sets it to `false`; Settings turns it back on. |
| `corner` | string enum | `"top_center"` | `"top_center"`, `"top_left"`, `"top_right"`, `"bottom_left"`, `"bottom_right"` (TS `NotchCorner`). Dragging the notch saves the position it snaps to. |
| `monitor` | string enum | `"primary"` | `"primary"` or `"cursor"` (TS `NotchMonitor`). `cursor` = the monitor under the cursor, re-checked when the foreground window or the displays change (never polled). |
| `hide_in_fullscreen` | bool | `true` | Hidden while a fullscreen app (game, video, presentation) is in front of its monitor. |

**Look.** Collapsed 46 x 13 logical px, fully rounded, ink `#141413` with a hairline rim and a dot: the accent (`appearance.accent`, default `#ff5b1f`) while Krova runs, grey `#8a877f` while paused. Hover for 150 ms grows it with a spring (damping 0.85, about 240 ms) to 220 x 28 with corner radius `appearance.corner_radius_px` (default 10) and the monitor's active layout name in off-white `#f2f1ee` (Segoe UI Variable, grayscale anti-aliasing); "Paused · name" while paused. With share-safe on (`settings.share_safe` once it exists) the name becomes "3 zones" and the menu lists "Layout 1..N". Sizes follow the monitor's DPI. It sits `4` logical px in from the monitor edge, but never under a reserved edge: a bottom position sits just above a bottom taskbar, a top one below a top taskbar.

**Mouse.**
- Left click: opens the quick panel next to the notch, or closes it if it is open (below). 
- Right click: a native menu with Pause/Resume Krova, Layouts (activates one on the notch's monitor), Layout switcher (opens the switcher for that monitor, like `<mods>+Space`), Cheat sheet (`navigate` = `"cheatsheet"` to the main window), Open Krova (the editor), Hide notch.
- Drag (more than 4 px): the collapsed pill follows the cursor; on release it springs to the nearest of the five positions on its monitor and saves `notch.corner` (`state-changed` follows).

**Quick panel.** A left click places the `panel` window (frameless, transparent, always on top, no taskbar entry, 360 x 440 logical px) below a top notch or above a bottom one, aligned with it and inside the work area, shows and focuses it, and sends it two events:

1. `panel-anchor` with `PanelAnchor` `{ "x", "y", "w", "h", "corner", "monitor_id" }`: the notch's rect in **physical** screen px, its position, and the `MonitorProfile.id` of its monitor.
2. `navigate` with `"panel"`.

A page that loaded late can pull the last anchor with the command `panel_anchor` (returns `PanelAnchor | null`). Esc should call `hide_panel`; losing focus hides it too. The window is created on the first click and then kept hidden ("warm"), never destroyed, so later clicks are instant. If a `panel` window already exists (the engine's), it is reused as is.

| Command | Arguments | Returns | Notes |
|---|---|---|---|
| `panel_anchor` | none | `PanelAnchor \| null` | The last anchor sent. |
| `hide_panel` | none | nothing | Hides the panel window. |

`AppState.capabilities.notch` is `true` on Windows and `false` elsewhere (and absent from older backends). The UI should hide the notch settings when it is not `true`.

**Demo mode.** `onPanelAnchor` sends a fake anchor (the notch on the primary demo monitor at `settings.notch.corner`) right after subscribing; `demoControls.clickNotch(corner?)` sends another, and `demoControls.onNotchNavigate` gets `"panel"` with it.

## Not part of the contract

Everything about the registry, the engine messages and the platform traits is internal and may change.

## Wave 4 (branch `wave4`): reserved edges, focus follows mouse, hover picker, updates, languages

Additive like everything above: every new field has a serde default, `schema_version` stays `1`, a v0.1 config loads unchanged.

### Settings additions

| Field | Type | Default | Notes |
|---|---|---|---|
| `focus_follows_mouse` | bool | `false` | Activate the window under the cursor once it rests. Opt-in. |
| `focus_follows_mouse_delay_ms` | int | `350` | Rest time before the activation, clamped to `150..=800`. |
| `hover_picker` | bool | `false` | With the activation `modifier` held on a window's title bar, the backend emits `hover-picker`. |
| `language` | string enum | `"system"` | `"system"`, `"en"`, `"es"`, `"de"`, `"ja"`, `"zh-Hans"`. Anything else is sanitised to `"system"`. Only the native strings (tray, notifications, overlay labels) follow it in this branch. |

### `MonitorProfile.reserved`

`{ "top": 0, "right": 0, "bottom": 0, "left": 0 }`, for third-party docks and bars that do not shrink the OS work area themselves. **Units: logical pixels (96 dpi)**, multiplied by the monitor's `scale` when applied. Each side is clamped to `0..=400` (the file loader accepts any number and clamps it). The result is subtracted from the OS work area in one place (`AppModel::work_of`), so zones, zone and edge snapping, keyboard moves, the reconnect restore, the overlay and `MonitorSnapshot.work` all use the reduced area. If the two sides of one axis would leave less than 200 physical pixels of it, that axis's reserves are ignored (the other axis still applies). Missing in old configs (treat as all zero).

### `AppState` additions (top level)

```jsonc
{
  "wave4_capabilities": { "focus_follows_mouse": true, "hover_picker": true, "open_releases_page": true },
  "pointer": { "active": false, "focus_refused": 0 },
  "language": "en",
  "languages": [ { "id": "system", "name": "System" }, { "id": "en", "name": "English" }, { "id": "es", "name": "Español" },
                 { "id": "de", "name": "Deutsch" }, { "id": "ja", "name": "日本語" }, { "id": "zh-Hans", "name": "简体中文" } ]
}
```

- `wave4_capabilities`: true on Windows, all false on macOS for now. Hide the settings of features that are false.
- `pointer.active`: the resting-cursor watcher runs (a pointer feature is on, the capability exists, Krova is enabled). `pointer.focus_refused`: how often Windows refused an activation since Krova started; a non-zero value is worth one sentence ("Windows blocked some focus changes").
- `language`: the language actually in use (never `"system"`); `languages`: the picker's choices in order.

### Commands

| Command | Arguments | Returns | Notes |
|---|---|---|---|
| `set_reserved_edges` | `monitorId: string`, `reserved: {top,right,bottom,left}` | `AppState` | Sanitised (`0..=400` each). Unknown monitor: `"monitor X not found"`. Persists and emits `state-changed`. |
| `place_hovered` | `zoneId: number` | `null` | Moves the window under the open hover picker into that zone of its monitor's active layout (gap applied, animated per `move_mode`) and remembers it like a drag snap. Errors: `"no window is under the picker"`, `"zone N is not in the active layout"`, `"monitor X is not connected"`, `"that window can no longer be moved"`. Emits `hover-picker-end` on success. |
| `open_releases_page` | none | `null` | Opens `https://github.com/nAItiklearn/krova/releases` (fixed at compile time) in the default browser via `ShellExecuteW`. Krova itself makes no network request. The tray has the same entry, "Check for updates…". |

### Events

- `hover-picker`: `{ monitor_id, window_rect: PxRect, cursor: [x, y], layout_id, zones: [{ id, rect: PxRect }] }`. Pixels are virtual-screen physical pixels; `zones` are exactly where `place_hovered` would put the window. Never carries a title.
- `hover-picker-end`: payload `null`. The cursor came to rest somewhere else (not on Krova's own windows), the modifier was released before the next rest, or a zone was chosen. Close the picker.

### Behaviour and limits

- **Zero idle CPU.** Both pointer features reuse the existing `WH_MOUSE_LL` drag hook. The hook stores the cursor position and pokes a capacity-1 channel; a worker thread sleeps until then and runs one one-shot wait per enabled feature (hover 250 ms, focus 150-800 ms) after each movement. Nothing runs while the cursor is still or while both features are off (the watcher is stopped). Both pause with Krova's master switch (the hook is the drag hook).
- **Focus follows mouse never activates** while a mouse button is held or was clicked during the rest, Krova's own windows, the desktop or taskbar, tool and no-activate windows, disabled windows (an open modal dialog), windows of elevated programs, while a menu or move/size loop is open, or while a full-screen app is in front (`SHQueryUserNotificationState` busy/D3D/presentation, or a foreground window covering its whole monitor). It does not raise windows by any other means.
- **Only documented focus APIs.** Activation is `SetForegroundWindow`. No `AttachThreadInput`, no injected input, no change to the system "activate on hover" setting. Windows' foreground-lock rules may refuse; Windows then flashes the window's taskbar button, Krova counts the refusal (`pointer.focus_refused`) and does not retry that window until the cursor rests on another one. How often Windows refuses on a real desktop is not measured yet (manual test below).
- **Hover picker** checks the title bar with `WM_NCHITTEST` via `SendMessageTimeout` (20 ms, abort if hung), so a hung program cannot stall it. Only movable windows (not elevated, not Krova's own, captioned) qualify.
- **Languages**: tables in `src-tauri/i18n/<id>.json` (flat `"key": "text"`, `_`-prefixed keys are notes). English is the source; `es`, `de`, `ja`, `zh-Hans` are English copies marked `"_status": "TODO"`, to be translated by people. Per-key fallback to English, then to the key. `"system"` resolves from `GetUserDefaultUILanguage` (zh-CN and zh-SG get `zh-Hans`; Traditional Chinese and unknown languages get English). The tray relabels itself when the language changes.
