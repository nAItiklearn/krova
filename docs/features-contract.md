# Features contract: Rust backend and UI

This is the interface between the Rust side (this branch) and the Svelte UI (built in parallel from this document). Serde names are stable: change them only together with this file.

Everything is additive. A v0.1 `config.json` loads unchanged: every new setting has a serde default, and `schema_version` stays `1`.

## Settings (in `config.settings`, sent to and from the UI)

| Field | Type | Default | Notes |
|---|---|---|---|
| `keyboard_shortcuts` | bool | `true` | Global shortcuts that move the focused window between zones. |
| `shortcut_modifier` | string enum | `"win_alt"` | One of `"ctrl_alt"`, `"win_alt"`, `"ctrl_alt_shift"`, `"win_ctrl"`. Ctrl+Alt+Left/Right collide with IDE back/forward, hence the default. |
| `restore_on_reconnect` | bool | `true` | When a monitor comes back, windows that Krova had snapped there return to their zones. |
| `linked_dividers` | bool | `true` | Resizing a snapped window by an edge moves the shared edge of its snapped neighbours too (applied when the mouse is released). |
| `zone_gap_px` | int | `0` | Space between zones and around the work area, in pixels. Clamped to `0..=32` on load and on save. It applies to zone snaps, the keyboard shortcuts, the reconnect restore and the drag overlay. Edge snaps (half / quarter) ignore it. |

Settings are sanitised in Rust (`Settings::sanitized`), both when the config is loaded and when the UI saves them, so the UI never has to trust its own clamping.

## `AppState` additions

```jsonc
{
  "shortcuts":    { "active": true, "failed": ["Win+Alt+3 is already used by another app"] },
  "capabilities": { "shortcuts": true, "window_memory": true }
}
```

- `shortcuts.active`: the shortcuts are registered with the OS (at least one of the twelve was accepted).
- `shortcuts.failed`: one human-readable sentence per refused shortcut. Empty when all twelve were accepted or when the feature is off. Show it as is.
- `capabilities.shortcuts`: this OS build can register global shortcuts (true on Windows, false elsewhere in this branch).
- `capabilities.window_memory`: this OS build can read window positions, which reconnect memory and linked dividers need (true on Windows, false elsewhere). The UI should hide or disable the corresponding settings when `false`.
- The state is pushed (`state-changed`) after every settings change, so a shortcut refusal shows up as soon as the user picks a modifier.

## Commands

All errors are plain strings.

| Command | Arguments | Returns | Notes |
|---|---|---|---|
| `export_layout` | `layout: Layout` | `string` | Pretty JSON envelope `{"krova_layout":1,"layout":{...}}`. The layout is validated first (zone count, text length, rectangles, duplicate ids). The UI chooses where to save the text. |
| `import_layout` | `monitorId: string`, `json: string` | `AppState` | At most 64 KiB, envelope version 1 only, same limits as `save_layout`. If the layout id already exists on that monitor it gets a numeric suffix (`halves` becomes `halves-2`). Goes through `Config::save_layout`, so the 64-layouts-per-monitor cap applies. Persists and emits `state-changed`. The imported layout is **not** activated. |
| `take_pending_view` | none | `string \| null` | Optional pull path for the window lifecycle (below). |

Existing commands are unchanged.

## Keyboard map (display text)

With the chosen modifier (`<mods>`, for example `Win+Alt`):

- `<mods>+Left` / `<mods>+Right`: move the focused window to the previous / next zone (wraps around).
- `<mods>+1` ... `<mods>+9`: move it to zone N.
- `<mods>+Up`: send it to the next monitor, keeping its zone number (if the other layout has fewer zones, the last one).

"Zone order" is reading order: top to bottom by the zone's `y` rounded to 1%, then left to right. Zone N is the N-th zone in that order, not the zone's id.

The shortcuts act on the window that has the focus. They do nothing for the desktop, the taskbar, minimized windows, windows of programs running as administrator, and Krova's own windows. A maximized window is restored and moved.

## Layout files

```json
{ "krova_layout": 1, "layout": { "id": "mine", "name": "Mine", "zones": [ { "id": 1, "rect": { "x": 0, "y": 0, "w": 0.3, "h": 1 } } ] } }
```

Coordinates are proportions of the work area, so a file works on any monitor and on both operating systems. Import is strict: exactly the two top-level keys, version 1, at most 64 KiB, at most 64 zones, ids and names at most 64 characters without control characters, every side at least 0.01.

## Window lifecycle (not a UI change, but visible to the UI)

The window is created on demand and **destroyed** when closed, so the embedded browser exits while Krova sits in the tray. The UI should not rely on module state surviving a close.

- `navigate` events (`"editor"` or `"settings"`) emitted while the window is being created are parked and delivered about 250 ms after the page finished loading, when the page has had time to register its listener. A UI that wants to be exact can call `take_pending_view` once, after it has registered its `navigate` listener: it returns the parked view, or `null` if none is waiting (the push is then a no-op because the view was already taken).
- The `main` window entry in `tauri.conf.json` has `"create": false`; the capability file still names the `main` label.

## Not part of the contract

Everything about the registry, the engine messages and the platform traits is internal and may change.
