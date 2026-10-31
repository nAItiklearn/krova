# UI contract notes: shortcuts, window memory, zone gap, export / import

What the Svelte UI (`ui/`) implements for the new backend contract, how the
in-browser demo mock behaves, which parts of the contract were ambiguous and how
they were resolved, and what could not be verified here.

The real Tauri path never runs in this environment (no Rust build, no WebView).
Everything below was exercised against the demo mock (`ui/src/lib/mock`) in
vitest and in Playwright (Edge). The invoke names and argument names were
written from the contract and re-read, not executed.

## Contract as implemented

`ui/src/lib/bindings/index.ts` (the one place for types):

* `Settings` gains `keyboard_shortcuts: boolean`, `shortcut_modifier:
  ShortcutModifier` (`"ctrl_alt" | "win_alt" | "ctrl_alt_shift" | "win_ctrl"`),
  `restore_on_reconnect: boolean`, `linked_dividers: boolean`, `zone_gap_px:
  number`.
* `AppState` gains `shortcuts: ShortcutStatus { active, failed: string[] }` and
  `capabilities: Capabilities { shortcuts, window_memory }`.

`ui/src/lib/backend.ts`:

| Method | Tauri command | JS arguments | Result |
| --- | --- | --- | --- |
| `exportLayout(layout)` | `export_layout` | `{ layout }` | `string` |
| `importLayout(monitorId, json)` | `import_layout` | `{ monitorId, json }` | `AppState` |

Both reject with a plain string; the UI shows it verbatim.

## What the UI does

### Settings view (`SettingsView.svelte`)

Two new cards after Appearance (after, not before: `keyboard.spec.ts` walks the
Settings Tab order up to Theme, and this keeps that order untouched).

* **Keyboard shortcuts** (`role="group"`): switch "Use keyboard shortcuts"
  (`setting-keyboard_shortcuts`); `select` "Modifier keys"
  (`setting-shortcut_modifier`: Win + Alt, Ctrl + Alt, Ctrl + Alt + Shift,
  Win + Ctrl; disabled while the switch is off, like Edge size under Edge
  snapping); a polite live region (`shortcuts-status`) that holds the conflict
  warning or the "not active" note; and the key map (`dl` with `kbd` caps).
  Key map: `<mods> + Left` previous zone, `<mods> + Right` next zone,
  `<mods> + 1 to 9` zone N, `<mods> + Up` next monitor keeping the zone number.
  `<mods>` follows the draft value of the select immediately (before the 300 ms
  auto-save).
* **Windows**: switch "Restore windows on reconnect"
  (`setting-restore_on_reconnect`, help text from the contract), switch "Linked
  dividers" (`setting-linked_dividers`), range "Space between zones (px)"
  (`setting-zone_gap_px`, 0..32, step 1, shows "N px").
* Failed shortcuts (`state.shortcuts.failed`, only when `capabilities.shortcuts`):
  warning "These shortcuts could not be registered:" with the messages as a
  list, plus a hint to pick other modifier keys. Shown with the warning colours
  of the permission banner (AA in both themes).
* `capabilities.shortcuts === false`: info line "Keyboard shortcuts are not
  available on this system yet.", switch and select disabled (stored values still
  shown), key map hidden (a list of keys that do nothing here would mislead).
* `capabilities.window_memory === false`: info line "Remembering snapped windows
  is not available on this system yet.", the two window switches disabled. The
  zone gap is **not** gated: it is geometry, not window memory.
* The existing 300 ms debounced auto-save and the "Saved" badge are unchanged:
  the new fields simply travel in `Settings`.

### Layout panel (`LayoutTransfer.svelte`, last card of the right-hand panel)

* **Export layout** opens an inline panel: read-only text area (the text of
  `export_layout` for the layout on screen, unsaved edits included, name
  trimmed with the same "Untitled layout" fallback Save uses), **Copy**, **Close**.
  The text re-exports while the panel is open when the layout on screen changes
  (edit, rename, other layout, other monitor). Copy tries
  `navigator.clipboard.writeText`, then `document.execCommand('copy')` on the
  selected text, and only then selects the text and says in the status line to
  press Ctrl+C (Cmd+C on a Mac).
* **Import layout** opens an inline panel: editable text area, **Import**,
  **Close**. The target is the selected monitor (the help text names it). On
  success the new layout is selected, the panel closes with the text cleared,
  focus returns to the opener and the status line says
  `Imported "<name>" as a new layout on <monitor>. It is selected now.` On error
  the backend string goes verbatim into the status line (`role="status"`,
  `aria-live="polite"`, `data-kind="error"`), the panel and the text stay, the
  text area gets `aria-invalid="true"` and `aria-describedby` pointing at the
  status line until the text is edited.
* Import never changes which layout is active.
* Escape closes the panel the focus is in and returns focus to its opener
  (`preventDefault`, so the editor's own Escape handler stands down).
* The card is last in DOM order, so it adds Tab stops after the templates and
  nothing in the existing Tab order moves.

## Demo mock behaviour

URL flags (demo/browser only; combine freely with `?nopersist` / `?reset`):

| Flag | Effect |
| --- | --- |
| (none) | `shortcuts: { active: true, failed: [] }`, `capabilities: { shortcuts: true, window_memory: true }` |
| `?shortcuts=failed` | `shortcuts: { active: true, failed: ["Win+Alt+3 is already used by another app"] }` |
| `?shortcuts=off` | `shortcuts: { active: false, failed: [] }` |
| `?capabilities=none` | both capabilities `false` (the macOS case today) |

Additionally the mock derives `shortcuts` from the rest of the state, which is
what a real backend would plausibly do: with `capabilities.shortcuts === false`
or `keyboard_shortcuts === false` the shortcuts are inactive and `failed` is
empty. Hooks on `window.__krovaDemo`: `setShortcuts('ok'|'failed'|'off')` and
`setCapabilities('all'|'none')` change this live and emit `state-changed`;
`reset()` goes back to the flags of the URL.

`saveSettings` sanitises like `Settings::sanitized`: booleans must be booleans
(otherwise the default), `shortcut_modifier` must be one of the four values
(otherwise rejected with `invalid shortcut_modifier "x"`, like the existing enum
errors), `zone_gap_px` is rounded and clamped to 0..32 (not finite: 0). Fields
missing from an older persisted `krova-demo` state get their defaults.

Layout rules (`mock/edit.ts` `validateLayout`, brought in line with `zone.rs`):
no zones; more than 64 zones; id or name longer than 64 characters (counted in
code points, like Rust `chars()`); then per zone a duplicate id or an invalid
rectangle (not finite, outside the unit square with 1e-9 slack, or a side below
0.01). `saveLayout` also refuses a new layout on a monitor that already holds 64
(`config.rs` already does). This makes the mock stricter than it was; nothing in
the existing tests depended on the looser behaviour.

`export_layout` (`mock/exchange.ts` `exportEnvelope`): validates, then returns
`JSON.stringify({ krova_layout: 1, layout }, null, 2)` with the keys in the
order id, name, zones / id, rect / x, y, w, h (two-space indent like
`serde_json::to_string_pretty`).

`import_layout` check order (`parseEnvelope`, then the monitor):

1. UTF-8 size over 65 536 bytes: `layout file is larger than 64 KiB`
   (bytes, not UTF-16 units; exactly 64 KiB is accepted)
2. nesting deeper than 128, or not JSON: `that is not valid JSON`
3. not an object, or no numeric `krova_layout`: `not a Krova layout file`
4. `krova_layout` is a number other than 1: `unsupported layout file version N`
5. `layout` not shaped like a layout (strings for id and name, a list of zones,
   integer u32 zone ids, four numbers per rect): `that file does not contain a
   valid layout` (my wording, see below). Unknown fields are ignored.
6. `Layout::validate` rules: `layout has no zones`, `layout has too many zones
   (at most 64)`, `layout id or name is too long (at most 64 characters)`,
   `duplicate zone id N`, `zone N has an invalid rectangle`
7. unknown monitor: `monitor X not found`
8. the monitor already holds 64 layouts: `a monitor can hold at most 64 layouts`
9. the id is made unique with `-2`, `-3`, ... (base shortened so the id stays
   within 64 characters), the layout is appended, config persisted like every
   other edit, the new `AppState` returned. A refused import changes nothing.

## Ambiguities and how they were resolved

1. **Shape errors.** The contract names strings for malformed JSON, a missing or
   unsupported envelope and the validation rules, but not for a well-formed
   envelope whose `layout` has the wrong types (a string where a list should be,
   a zone id of -1 or 1.5, ...). The mock says `that file does not contain a
   valid layout`. A serde error from Rust will read differently; the UI does not
   care, it shows whatever string arrives.
2. **`1e999`.** JavaScript parses it to `Infinity`, so the mock reports
   `zone N has an invalid rectangle`, as the task asked. `serde_json` refuses an
   out-of-range float while parsing, so the real backend most likely reports a
   JSON error instead. Same for zone ids that do not fit a u32: the mock treats
   them as a shape error.
3. **Nesting limit.** Mirrors serde's recursion limit (128) and reports it as
   invalid JSON. A real backend may phrase it differently.
4. **Unique-id suffix.** The contract says "a suffix" without a scheme. The UI
   does not depend on it: it finds the imported layout as the one id on the
   monitor that was not there before the call (the last one if several).
5. **Name collisions.** Only the id is made unique, as written; two layouts may
   share a name after an import.
6. **Export validates?** The contract does not say whether `export_layout`
   validates its argument. The mock does (same messages as `save_layout`); the UI
   works either way. Export sends the layout on screen, so a draft that backend
   edits already validated is exported with its unsaved edits.
7. **`shortcuts.active`.** Never defined beyond the flag. The UI shows "not active
   right now" only when the saved setting is on, the platform supports shortcuts
   and nothing is listed in `failed`; with entries in `failed` it shows the
   warning instead. It uses the *saved* setting for that note so the 300 ms
   before a toggle is saved does not flash it.
8. **Which toggles are "window memory".** `restore_on_reconnect` and
   `linked_dividers` are gated by `capabilities.window_memory`; `zone_gap_px` is
   not, because it only changes geometry.
9. **macOS wording.** `shortcutKeys()` uses Cmd / Control / Option only when the
   OS is a Mac **and** `capabilities.shortcuts` is true; today that never
   happens, so everything reads Win / Ctrl / Alt / Shift.
10. **`zone_gap_px` inert.** The control is a normal setting; nothing in the UI
    claims the gap is already applied. If the first backend build ignores the
    value, the slider still saves and reloads it.
11. **Character counting.** The 64-character limit counts code points (Rust
    `chars()`), not UTF-16 units; the layout name field keeps its existing
    `maxlength="60"`.
12. **Import target.** "The selected monitor" is the monitor tab that is open when
    Import is pressed (the help text names it). If the tab is changed while the
    panel is open, the next Import goes to the new tab.

## Existing code touched (behaviour that changed)

* `mock/edit.ts` `validateLayout`: stricter, mirrors `zone.rs` (see above).
* `mock/mockBackend.ts` `saveLayout`: refuses a 65th layout.
* `StatusLine.svelte` got `id="editor-status"` (the import text area points at it).
* `app.css` got shared `textarea`, `kbd` and `.notice` styles.
* `e2e/fixtures.ts`: the `Window.__krovaDemo` type lists the two new hooks.
* No existing test was edited to get green. `a11y.spec.ts` got new scenarios
  appended; the README tables were updated.

## Not verified here

* The real Tauri path: `invoke('export_layout', { layout })` and
  `invoke('import_layout', { monitorId, json })` follow the contract text but
  have never talked to the Rust commands. The exact error strings of Rust (shape
  errors, `1e999`, depth) are unknown.
* Clipboard behaviour inside WebView2 / WKWebView (Playwright runs Edge with the
  clipboard permissions granted, or with `navigator.clipboard` replaced).
* macOS rendering and the macOS wording (capabilities are false there today).
* Screen readers: the live regions are in place and asserted by attribute, but
  nothing was listened to.

## Verification (last run, on this branch)

From `ui/`:

| Command | Result |
| --- | --- |
| `npm test` | 159 vitest tests pass (41 before; new: `exchange.test.ts` 87, and `mockBackend.test.ts` grew from 13 to 44) |
| `npm run check` | svelte-check: 0 errors, 0 warnings |
| `npm run build` | OK |
| `npx tsc -p e2e/tsconfig.json --noEmit` | no errors |
| `PW_PORT=14420 npm run e2e` | 240 Playwright tests pass (159 before; new: `settings-shortcuts` 27, `layout-transfer` 38, 8 more axe states x 2 themes = 16) |

Port 1420 was held by a stale dev server (not started by this work) that served
the old UI, which `reuseExistingServer` picked up silently and made every new
test fail. It was left alone; `playwright.config.ts` now reads `PW_PORT`
(default 1420, passed to vite as `--port`) and `privacy.spec.ts` takes its
origin from the `baseURL` fixture instead of a literal. Behaviour with the
default port is unchanged.
