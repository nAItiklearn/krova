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

## Keyboard map (display text; superseded by Wave 2 below)

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

## Wave 1: auto-arrange, stacks, swap, app rules, presets (engine branch)

Krova arranges out of the box: auto-arrange is **on for every monitor by default**, and on first run a monitor that still has only the built-in layouts (with `halves` active) gets the preset that fits its shape (main + side for three windows: landscape, ultrawide 2.1:1 or wider, or portrait), added in front and made active. This happens once per monitor profile (`preset_seeded`).

### Config additions

| Where | Field | Type | Default | Notes |
|---|---|---|---|---|
| `MonitorProfile` | `auto_arrange` | `ArrangeSettings` | `{enabled: true, policy: "next_free", fill_gaps: false, stack_when_full: true}` | Per monitor. A missing key loads as on. Change with `set_auto_arrange`. |
| `MonitorProfile` | `preset_seeded` | bool | `false` | Internal: the first-run preset was offered. The UI never shows or sends it (send back what you got). |
| `Settings` | `app_rules` | `AppRule[]` | `[]` | `{exe, action}`; `action` is `{"kind":"never_touch"}`, `{"kind":"float"}`, `{"kind":"preferred_zone","value":N}` (1-based zone in reading order) or `{"kind":"start_on_monitor","value":"monitor-2"}`. By executable **file name** (`chrome.exe`), never a title or path; case-insensitive, one rule per program, at most 256. Sanitised on load and on save (invalid rules are dropped). Prefer `set_app_rule` / `remove_app_rule` for readable errors. |

TS: `ArrangeSettings`, `ArrangePolicy`, `AppRule`, `RuleAction`.

### `AppState` additions

- `capabilities.auto_arrange`: auto-arrange, stacks and tab strips work on this OS (Windows: true; macOS: false). Hide the auto-arrange switches when false.
- `front_app`: `FrontApp | null` = `{exe: string | null, arranged: bool, reason: FrontAppReason | null}`. Updated when the focus moves; read with the state (not pushed on every focus change). `reason` is `"dialog"`, `"tool"`, `"popup"`, `"elevated"`, `"never_touch"`, `"float"`, `"auto_arrange_off"`, `"paused"` or `"other"`, for the "Not arranged" chip.

### Commands

| Command | Arguments | Returns | Notes |
|---|---|---|---|
| `list_presets` | `monitorId: string \| null` | `PresetInfo[]` | Every distinct preset (grid / columns / rows / main + side, 1-5 windows, landscape / ultrawide / portrait, main 50/60/66.7 %). `PresetInfo` = `{id, name, family, count, variant, main_size, recommended, layout}`; `recommended` marks the presets for that monitor's shape. `layout` is an ordinary layout: save it with `save_layout`. |
| `set_auto_arrange` | `monitorId`, `settings: ArrangeSettings` | `AppState` | Persists and emits `state-changed`. Switching off forgets the stacks (windows stay where they are). |
| `set_app_rule` | `rule: AppRule` | `AppState` | Adds, or replaces the rule of the same program in place. Errors are sentences (`"C:\x.exe" is a path, not a program file name`). |
| `remove_app_rule` | `exe: string` | `AppState` | Case-insensitive. Error if there is no such rule. |
| `arrange_monitor` | `monitorId` | `null` | "Tile now": arrange every ordinary window already open on that monitor (at most 64). Asynchronous; the windows move a moment later. |

### Behaviour (what the user sees)

- A new ordinary window (not a dialog, tool window, owned pop-up, splash, notification, full-screen, cloaked or elevated window) springs into the next free zone of the active layout about 350 ms after it appears. An app rule can send it to zone N, to another monitor, or keep it floating.
- With every zone taken it joins a **stack**: the zone of the preferred zone, the focused window or the last zone. A native **tab strip** sits on the zone's top edge: rounded translucent tabs, a 1 px hairline, the program's icon and short name (from the executable, never the title), the visible tab underlined in the accent colour, light or dark with the OS (or `theme`). Click a tab or turn the wheel over the strip to switch; gaps between tabs let clicks through. Share-safe (wave 3) hides names and icons.
- Closing or minimising a window takes it out of its zone; the next tab of a stack shows. `fill_gaps` moves later windows up.
- Dragging a tracked window (with the snap modifier) onto an occupied zone **swaps** the two windows. Moving a tracked window away by hand makes it float.
- Changing the active layout re-tiles: windows keep their zone id; windows whose zone is gone are arranged again.
- Keyboard swap and stack cycling are engine functions bound in wave 2 (`Ctrl+Alt+Shift+Arrow`, `Ctrl+Alt+[` / `]`).

### Window classification (for other branches)

`krova_platform::classify(&WindowFacts) -> WindowKind` is OS-independent and unit-tested; `Platform::probe_window(id) -> Option<WindowProbe {kind, exe, exe_path, owner}>` reads the facts on Windows (styles, owner, class family, DWM cloaking, elevation) and the program path. `WindowKind` gained `cloaked` and `elevated`. Only `normal` windows are ever arranged. No title is read.

## Wave 2: shortcut scheme, rebinding, popups, pause (engine branch)

**This replaces the "Keyboard map" above.** Base chord `Ctrl+Alt`; every action can be rebound or switched off one by one.

| Action id | Default | Does |
|---|---|---|
| `move_left` / `move_right` / `move_up` / `move_down` | `Ctrl+Alt+Arrow` | Focused window to the neighbouring zone in that direction; past the layout's edge, to the nearest zone on the next monitor that way. A window in no zone goes to the far zone in that direction. |
| `swap_left` ... `swap_down` | `Ctrl+Alt+Shift+Arrow` | Swap with the window in the neighbouring zone (auto-arranged windows); an untracked window just moves. |
| `zone_1` ... `zone_9` | `Ctrl+Alt+1..9` | Focused window to zone N (reading order). |
| `layout_1` ... `layout_9` | `Ctrl+Alt+Shift+1..9` | Apply layout N (the monitor's list order) on the monitor under the cursor; auto-arranged windows re-tile. |
| `maximize_in_zone` | `Ctrl+Alt+Enter` | A window in a zone fills its monitor's work area; again: back into its zone. |
| `stack_prev` / `stack_next` | `Ctrl+Alt+[` / `]` | Previous / next tab of the focused window's stack. |
| `switcher` | `Ctrl+Alt+Space` | Layout switcher on the monitor under the cursor (also needs `layout_switcher_shortcut`). |
| `pause` | `Ctrl+Alt+T` | Pause / resume auto-arrange; windows stay where they are. |
| `cheat_sheet` | `Ctrl+Alt+/` | Show every shortcut (the `switcher` window with `navigate` = `cheat_sheet`). |
| `focus` | `Ctrl+Alt+F` | Reported to the engine hooks only (`Action("focus")`); wave 3 implements focus. |

**Known collisions.** `Ctrl+Alt+Left` / `Right` are "navigate back / forward" in Visual Studio and JetBrains IDEs (a global hot key wins while Krova runs), and older Intel graphics drivers bind `Ctrl+Alt+Arrow` to screen rotation; Windows then refuses the chord and its row shows `in_use` with suggestions. `Ctrl+Alt+Space` is a common quick-entry shortcut.

### Settings

| Field | Type | Default | Notes |
|---|---|---|---|
| `shortcuts` | `ShortcutMap` = `{ [actionId]: string \| null }` | `{}` | Only the user's changes: a chord text (`"Ctrl+Shift+F"`; modifiers written in the order Win, Ctrl, Alt, Shift, parsed in any order and case) or `null` (off). Missing ids keep their default. Loaded leniently: unknown ids and unreadable chords are dropped, a duplicate chord switches the later action off. |
| `shortcut_modifier` | (deprecated) | | Migrated once: a config without `shortcuts` whose modifier was `win_alt` / `win_ctrl` gets overrides for the actions that existed then (move left / right / up, zones, switcher). `ctrl_alt_shift` (the old default) and `ctrl_alt` start from the new scheme. |

`keyboard_shortcuts` still switches every shortcut except the switcher; `layout_switcher_shortcut` the switcher. `AppState.shortcuts` / `layout_switcher` keep working as group summaries.

### `AppState` additions

- `shortcut_actions: ShortcutActionState[]`: one row per action in sheet order: `{id, label (1-4 words), group: "Windows" | "Layouts" | "Krova", chord: string | null, default_chord, status: "active" | "off" | "in_use" | "error" | "unsupported", message: string | null}`. Render the shortcut sheet and the cheat sheet straight from it.
- `paused: bool`. While paused the tray item "Pause arranging" is checked, the tray icon is drawn at 40 % opacity and its tooltip says "Krova (paused)".

### Commands

| Command | Arguments | Returns | Notes |
|---|---|---|---|
| `probe_shortcut` | `action: string`, `chord: string` | `ProbeResult` | `{status: "free" \| "current" \| "conflict" \| "in_use" \| "invalid" \| "unknown", chord, conflict_with, message, suggestions}`. Krova's own map first (`conflict` names the action), then a live `RegisterHotKey` probe. `suggestions`: up to 3 free chords with the same key. |
| `set_shortcut` | `action`, `chord: string \| null` | `AppState` | Rebind or switch off; re-registers at once. A chord another action has is refused ("Ctrl+Alt+Left is already Move left"). |
| `reset_shortcuts` | none | `AppState` | Every action back to its default. |
| `set_paused` | `paused: bool` | `null` | Like `Ctrl+Alt+T`; the new state comes with `state-changed`. |
| `hide_popup` | `label: "panel" \| "switcher"` | `null` | Call on Esc. |
| `popup_open_ms` | `label` | `number \| null` | How long the last open took (request to shown); see `docs/measurements/popup-open-latency.md`. |

### Windows `panel` and `switcher`

Two more windows in `tauri.conf.json` (`create: false`, undecorated, transparent, always on top, no taskbar entry; panel 360x520, switcher 820x560), both in the capability file. Created on first use, then **kept warm**: closing, Esc (`hide_popup`) or a click outside (focus lost) only hides them; the next open is a show plus a `navigate` event. The UI tells them apart by `getCurrentWindow().label`. `navigate` payloads: `"panel"` in `panel`; `"switcher"` or `"cheat_sheet"` in `switcher`. The switcher opens centred (upper third) on the monitor `switcher_monitor` names; the panel near the tray (bottom right of the primary work area), or next to the notch (see "Popup windows" below). A left click on the tray icon opens the panel; the tray menu has "Quick panel" and "Pause arranging".

TS: `ShortcutMap`, `ShortcutActionState`, `ShortcutGroup`, `ShortcutStatusCode`, `ProbeResult`, `PopupLabel`, `PanelAnchor`; `NavigateTarget` gained `cheat_sheet` and `panel`.

### Popup windows: pages, backdrop, capabilities, `open_main`, `show_panel`

- `panel` loads `panel.html`, `switcher` loads `switcher.html` (the pages are built by the UI branches `ui-panel` and `ui-switcher`; on this branch they are not in `ui/dist` yet). Both: undecorated, transparent, no shadow, not resizable, always on top, no taskbar entry, created hidden on first use and kept warm.
- The **switcher** window is sized and placed to cover the whole target monitor (`switcher_monitor`, physical pixels) on every open, then shown, focused and sent `navigate` (`"switcher"` or `"cheat_sheet"`). The page hides itself (apply, Esc, backdrop click, blur) with `hide_popup` or the window API.
- Both windows get the system's blurred backdrop once their page loaded: acrylic (`DWMSBT_TRANSIENTWINDOW`), else mica, through `DwmSetWindowAttribute(DWMWA_SYSTEMBACKDROP_TYPE)` (`krova_platform::apply_backdrop`), with the dark or light frame colours of the theme. Windows 11 22H2+; elsewhere nothing happens and the page's own background shows. No extra crate.
- Capabilities: `capabilities/popups.json` for `panel` and `switcher`: `core:default`, `core:event:allow-emit-to`, `core:window:allow-set-size`, `-set-position`, `-show`, `-hide`, `-set-focus`. App commands (`get_state`, `switcher_monitor`, `set_active_layout`, `delete_layout`, `insert_layout`, `move_layout`, ...) have no per-command ACL yet (F-08), so they are callable from these windows like from `main`. `capabilities/default.json` is `main` only again.
- Command `open_main` (`view: string`): `"switcher"` opens the switcher window on the monitor under the cursor, `"cheat_sheet"` the cheat sheet; anything else creates or focuses the editor window and emits `navigate` with that view or settings section (`"settings"`, `"editor"`, `"corners"`, ...; a section name is lower-case letters, digits, `_` and `-`, at most 32 long; at most 64 distinct names per run). Errors are sentences. TS: `Backend.openMain`.
- `panel-anchor` payload (changed from `{rect}`): `PanelAnchor` = `{x, y, w, h, corner: "top_center" | "top_left" | "top_right" | "bottom_left" | "bottom_right", monitor_id}` in physical pixels. **The notch calls `ui_window::show_panel(&app, Some(anchor))`**: the anchor is stored in `Shared.panel_anchor`, the panel opens on that monitor next to it (below a top notch, above a bottom one, centred or aligned to the corner, kept inside the work area) and the anchor is emitted to the panel unchanged. `show_panel(&app, None)` (and the tray) opens it near the tray.

## Appearance (engine branch)

| Field | Type | Default | Notes |
|---|---|---|---|
| `appearance.accent` | `#rrggbb` | `#ff5b1f` | Validated like `highlight_color` (bad values fall back to the default). Used by every native overlay: the drag zones, the stack tab strips' underline, and the notch. |
| `appearance.corner_radius_px` | int | `10` | `0..=16` logical px; zone tiles and tabs are rounded with it (scaled to each monitor's DPI). |
| `appearance.border_width_px` | int | `2` | `1..=6` logical px; idle zones use it, the highlighted zone twice it. |
| `highlight_opacity` | float | `0.35` | Unchanged: still the fill opacity. |
| `highlight_color` | `#rrggbb` | **`#ff5b1f`** (was `#3b82f6`) | Old name of the accent. Kept so older UIs and files load; the overlays read `appearance.accent`. A config written before `appearance` existed whose `highlight_color` is not the old blue default has that colour migrated into `appearance.accent`. |

TS: `Appearance`.

## Engine hooks (for the wave 3 / wave 4 / notch / corners branches)

`src-tauri/src/hooks.rs`. Register once at start-up: `shared.hooks.register(Arc::new(|event| { ... }))`. A hook runs on the engine thread right after the event and must return at once (send to a channel, flip a flag); never call back into the engine synchronously. No event carries a title.

| `EngineEvent` | When |
|---|---|
| `FocusChanged { window }` | The foreground window changed. |
| `WindowMoved { window, rect }` | Krova placed a window (snap, shortcut, auto-arrange, restore, fill screen). |
| `MonitorsChanged` | The monitor set settled after a change. |
| `SettingsChanged` | Settings or layouts changed (UI, tray, a layout applied by shortcut). |
| `PauseChanged(bool)` | Auto-arrange paused / resumed. |
| `Action(&'static str)` | A feature the engine does not implement was asked for (`"focus"` today). |

Calls the other way (on `Shared` or in `engine`):

- `engine::request(&shared, ShortcutAction::...)`: run any shortcut action as if pressed (switcher, cheat sheet, pause, focus, ...).
- `shared.is_paused()`; `engine::notify(&shared, Msg::SetPaused(b))` changes it.
- `shared.active_layouts()`: `[{profile_id, layout_id, layout_name}]` for every connected monitor (the notch shows the name).
- `shared.set_hide_app_names(bool)` / `hide_app_names()`: share-safe; the stack tab strips stop drawing names and icons from their next refresh.
- `*lock(&shared.panel_anchor) = Some(rect)`, then the `"panel"` view through `shared.open_view`: the panel opens anchored there and receives `panel-anchor`.
- `shared.state_changed`: call it to push `state-changed` to the UI and refresh the tray after changing something.
- `krova_platform::classify` / `Platform::probe_window`: the window classifier (see Wave 1).

## Not part of the contract

Everything about the registry, the engine messages and the platform traits is internal and may change.
