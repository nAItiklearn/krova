# Krova UI

Svelte 5 + Vite + TypeScript front end for the Krova zone editor and settings,
shown in a single Tauri webview window. It also runs in a plain browser in
**demo mode** with a mock backend, for design review and Playwright.

No router library (Editor / Settings are switched by a small nav), no remote
fonts, no CDN, no network calls of any kind. System font stack only.

## Commands

```
npm install
npm run dev        # vite dev server on http://localhost:1420 (strict port; Tauri's devUrl)
npm run build      # production build into ui/dist (relative base './')
npm run demo       # vite --mode demo: browser-only UI with the mock backend
npm run build:demo # static build that always uses the mock backend
npm run preview    # serve ui/dist on port 1420
npm run check      # svelte-check (types + a11y lint)
npm test           # vitest unit tests
```

`npm run demo` serves on port 1420 too (`strictPort`); use
`npm run demo -- --port 5173` if Tauri's dev server is already running.

## Backend seam

`src/lib/backend.ts` is the only module that talks to Rust. It defines the
`Backend` interface and exports `api`, chosen once at startup:

| Condition | `api` |
| --- | --- |
| `vite --mode demo` (`import.meta.env.MODE === 'demo'`) | `mockBackend` (always) |
| `window.__TAURI_INTERNALS__` exists | `tauriBackend` |
| otherwise (plain browser) | `mockBackend` |

`tauriBackend` uses `invoke` from `@tauri-apps/api/core` and `listen` from
`@tauri-apps/api/event`. Tauri v2 maps camelCase JS argument names to the Rust
command's snake_case parameters.

All contract types live in `src/lib/bindings/index.ts` (hand-written mirror of
the serde shapes; the Rust build will replace it with ts-rs output). Nothing
else defines contract types.

| Backend method | Tauri command | Arguments (JS) | Result |
| --- | --- | --- | --- |
| `getState()` | `get_state` | | `AppState` |
| `saveSettings(settings)` | `save_settings` | `{ settings }` | `AppState` |
| `saveLayout(monitorId, layout)` | `save_layout` | `{ monitorId, layout }` | `AppState` (upsert by `layout.id`) |
| `deleteLayout(monitorId, layoutId)` | `delete_layout` | `{ monitorId, layoutId }` | `AppState` |
| `setActiveLayout(monitorId, layoutId)` | `set_active_layout` | `{ monitorId, layoutId }` | `AppState` |
| `listTemplates()` | `list_templates` | | `Layout[]` |
| `editSplitZone(layout, zoneId, axis, at)` | `edit_split_zone` | `{ layout, zoneId, axis, at }` | `Layout` |
| `editRemoveZone(layout, zoneId)` | `edit_remove_zone` | `{ layout, zoneId }` | `Layout` |
| `editMoveDivider(layout, axis, from, to)` | `edit_move_divider` | `{ layout, axis, from, to }` | `Layout` |
| `onNavigate(cb)` | event `navigate` | payload `"editor" \| "settings"` | |
| `onStateChanged(cb)` | event `state-changed` | payload `AppState` | |

Every command rejects with a string. The editor shows that string, verbatim,
in the `role="status"` line under the canvas and keeps the previous layout.

## How the editor works

* The canvas shows the selected monitor's layout, scaled to the monitor's
  **work area** aspect ratio. Zones are normalized 0..1 rects.
* Structural edits are backend commands (`edit_*`); the UI never decides
  whether an edit is legal. (It only draws previews and clamps a drag to the
  0.05 minimum so a drag does not end on a guaranteed error.)
* Edits go into an unsaved **draft** per monitor+layout. Save calls
  `save_layout`. Drafts survive switching monitors, layouts and views. Undo
  (Ctrl/Cmd+Z) and Revert work on the draft.
* Apply template replaces the zones of the layout being edited with the zones
  from `list_templates`. New layout calls `save_layout` right away.
  "Set as active" saves unsaved edits of that layout first.
* Delete layout needs a second press (no blocking `confirm()` dialog).

### Keyboard

| Where | Keys |
| --- | --- |
| Anywhere in the editor | `V` split vertically tool, `H` split horizontally tool, `Esc` back to Select, `Ctrl/Cmd+S` save, `Ctrl/Cmd+Z` undo, `Delete` delete selected zone |
| Zone (Select tool) | `Enter`/`Space` select, `Delete` delete |
| Zone (split tool) | Arrow keys move the guide (Shift = 5%), `Enter` splits |
| Divider | Arrow keys move it 1% (Shift = 5%) |
| Monitor tabs | Left/Right/Home/End |

## Demo mode

The mock (`src/lib/mock/`) keeps state in memory and persists config edits to
`localStorage["krova-demo"]`. It fakes two monitors (Display 1 (primary)
2560x1440 with a 2560x1400 work area, Display 2 1920x1080 at x=2560) with the
four templates each, and reimplements `edit_split_zone` / `edit_remove_zone` /
`edit_move_divider` in `src/lib/mock/edit.ts`, a mirror of
`crates/krova-core/src/edit.rs` (same error strings, `EPS = 1e-6`,
`MIN_ZONE = 0.05`, same id assignment, failed edits leave the layout unchanged).
A badge reads "Demo mode - changes are not saved to a real config".

URL parameters (demo/browser only):

* `?permission=denied` shows the macOS Accessibility banner (`granted`,
  `not_required` also accepted).
* `?reset` forgets persisted demo edits at startup.
* `?nopersist` keeps everything in memory only.

Hooks on `window.__krovaDemo` (demo only): `navigate('editor'|'settings')`,
`setPermission('denied'|'granted'|'not_required')`, `toggleEnabled()`,
`reset()`. They emit the same events the real backend would.

### Test ids for e2e

`monitor-tab`, `canvas`, `zone-<id>`, `divider` (`data-axis`, `data-pos`),
`split-preview`, `tool-select`, `tool-split-vertical`, `tool-split-horizontal`,
`delete-zone`, `undo`, `revert`, `save`, `status` (`data-kind` is
`info|success|error`), `layout-name`, `set-active`, `delete-layout`,
`new-layout`, `layout-item`, `template-<id>`, `demo-badge`,
`permission-banner`, `settings`, `save-indicator`, `setting-<key>` (switches,
sliders, text) and `setting-<key>-<value>` for radio options, `nav-editor`,
`nav-settings`, `version`, `privacy`.

## Layout of the source

```
src/main.ts                 entry
src/App.svelte              shell: nav, banner, views, footer, theme
src/app.css                 design tokens (light/dark), shared controls
src/components/             Editor, LayoutCanvas, Toolbar, MonitorTabs, LayoutPanel,
                            StatusLine, SettingsView, PermissionBanner, DemoBadge, Icon, LayoutThumb
src/lib/backend.ts          Backend interface, tauriBackend, api selection
src/lib/bindings/index.ts   contract types (only place types are defined)
src/lib/mock/               mockBackend, edit.ts (edit.rs mirror), templates.ts, tests
src/lib/*.svelte.ts         app / editor / settings stores (Svelte 5 runes)
src/lib/geometry.ts         hit-testing, divider discovery, previews (UI only)
```

## Security notes

`index.html` carries the CSP `default-src 'self'; style-src 'self'
'unsafe-inline'; img-src 'self' data:; connect-src 'self' ipc:
http://ipc.localhost`. The code uses no inline scripts or inline event handler
attributes (Svelte attaches listeners from the bundle) and the app code has no
`fetch`, `XMLHttpRequest`, `WebSocket` or external URL (the production bundle
is also built without Vite's `fetch`-based modulepreload polyfill; only the
Vite dev server's own HMR socket exists, and only under `npm run dev`).
`style-src 'unsafe-inline'` is only needed for the computed zone geometry in
`style` attributes.
