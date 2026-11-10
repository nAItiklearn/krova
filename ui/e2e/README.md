# Krova UI end-to-end tests

Playwright tests for the UI in **demo mode**: the real Svelte app in a browser
against the in-memory mock backend (`npm run demo`, `vite --mode demo`, port
1420). Nothing here needs Rust, Tauri or a network.

## Run

```
cd ui
npm install
npm run e2e                                  # all tests, headless
npm run e2e -- e2e/editor.spec.ts            # one file
npm run e2e -- -g "divider"                  # by title
npm run e2e -- --headed --workers=1          # watch it
npm run e2e -- --repeat-each=5               # hunt for flakiness
npx playwright show-trace e2e/.results/<test>/trace.zip   # after a failure
```

`playwright.config.ts` starts `npm run demo` itself and reuses a server that is
already listening on http://localhost:1420 (`reuseExistingServer`, off when
`CI` is set). If you run `npm run dev` for Tauri on the same port, that server
is reused too; the UI falls back to the mock in a plain browser either way.

**Beware of a stale server on 1420**: it is reused silently, so the tests would
run against whatever code that server serves (for example another checkout).
`PW_PORT=14420 npm run e2e` starts the demo server on another port (`vite --port`)
and runs against it; the specs read the origin from `baseURL`, nothing else
depends on 1420.

### Against a static demo build (busy machine, long runs)

```
PW_PREVIEW=1 PW_PORT=1491 npm run e2e
```

`PW_PREVIEW=1` makes the config build the demo once (`vite build --mode demo
--outDir e2e/.demo-dist`, git-ignored) and serve it with `vite preview` instead of
the dev server. The dev server compiles modules on demand and reloads when files in
the checkout change (other work, another agent), which shows up as timeouts and
flakiness on a loaded machine; the static build does neither. Combine it with a
free `PW_PORT` so no stale server is reused. The build runs on every start of the
web server, so changes to the app are picked up on the next run.

If the machine is already busy (CPU at 100 % from other work), also lower the
worker count, for example `PW_WORKERS=2`: with 4 workers a test can pass and still
time out while its context closes and its trace is written.

### Browser

No browser is downloaded. The default is the Microsoft Edge that ships with
Windows (`channel: 'msedge'`). Pick another one with `PW_CHANNEL`:

```
PW_CHANNEL=chrome npm run e2e        # installed Google Chrome
PW_CHANNEL=chromium npm run e2e      # Playwright's own Chromium:
                                     #   npx playwright install chromium   (about 150 MB)
```

On a Linux CI runner use `PW_CHANNEL=chromium` (after `npx playwright install
--with-deps chromium`) or install Chrome and use `PW_CHANNEL=chrome`.

Generated files (`e2e/.results`, `e2e/.report`) are git-ignored by
`e2e/.gitignore`.

## What is covered

| File | Flows |
| --- | --- |
| `app.spec.ts` | first load, demo badge, both monitor tabs, no console errors, monitor selection and canvas aspect ratio, nav, `navigate` / `setPermission` / `toggleEnabled` / `reset` hooks, `?permission=`, `?reset`, `?nopersist`, persistence across reload |
| `editor.spec.ts` | templates + Save/Undo/Revert, pointer split (vertical, horizontal), refused splits closer than 5% to an edge (backend text in the live region, layout unchanged), divider drag (commit on release, Escape cancels, clamping), arrow-key divider moves, delete zone + undo, last zone, no merge target, layouts (new, rename, set active, two-step delete) |
| `settings.spec.ts` | every setting, the Saved indicator, hex validation (`aria-invalid`), persistence, theme switching and the rendered colour scheme |
| `settings-shortcuts.spec.ts` | the shortcut sheet against the backend actions (`shortcut_actions`, `probe_shortcut`, `set_shortcut`, `reset_shortcuts`: defaults, recording, families, refusals with suggestions, Backspace, Reset all), the "Keyboard shortcuts" and "Windows" groups: switch, persistence, `?shortcuts=failed` / `?shortcuts=off` notes, `?capabilities=none` disabled states (and Tab skipping them), live changes with the `setShortcuts` / `setCapabilities` hooks, Tab order after Appearance |
| `layout-transfer.spec.ts` | Export (envelope text, follows edits / layouts / monitors, Copy with and without a clipboard, Close, Escape), Import (Display 1 to Display 2, id suffixes, keyboard-only, 14 refused inputs with their exact messages, the 64-layout cap, persistence) |
| `keyboard.spec.ts` | Tab order, visible focus ring at every stop, a complete edit with the keyboard only, live region |
| `emulation.spec.ts` | `prefers-reduced-motion`, `prefers-color-scheme` light and dark, `forced-colors` |
| `a11y.spec.ts` | axe-core (WCAG 2.0 to 2.2, A and AA) over 18 states x light and dark (including the export / import panels, the shortcut warning / inactive / unavailable states and the disabled controls); best-practice findings are only recorded as annotations |
| `privacy.spec.ts` | the page only talks to the local dev server, no external references, CSP, and a self-test of the guard |

## Test ids of the keyboard shortcut, window and export / import features

| `data-testid` | What |
| --- | --- |
| `settings-shortcuts`, `settings-windows` | the two Settings cards (`role="group"`, named by their `h3`) |
| `setting-keyboard_shortcuts` | switch "Use keyboard shortcuts" |
| `shortcut-<row>`, `shortcut-conflict-<row>`, `shortcuts-reset` | a sheet row's record button (`move`, `swap`, `zone_n`, `layout_n`, `maximize_zone`, `stack_next`, `stack_prev`, `switcher`, `pause`, `focus`, `cheat_sheet`), its refusal notice, Reset all |
| `setting-shortcut_modifier` | (only with a backend without `shortcut_actions`) `select` "Modifier keys" (`ctrl_alt_shift` is the default and comes first, then `ctrl_alt`, `win_alt`, `win_ctrl`) |
| `shortcuts-win-hint` | hint under the select, only for `win_alt` / `win_ctrl` ("Windows keeps many shortcuts with the Windows key for itself ...") |
| `shortcuts-keymap` | the `dl` of key combinations; rows `keymap-left`, `keymap-right`, `keymap-number`, `keymap-up` are the `dt`s ("Ctrl + Alt + Shift + Left", "... + 1 to 9") |
| `shortcuts-status` | polite live region around the next three |
| `shortcuts-failed` | warning with the list of shortcuts that could not be registered |
| `shortcuts-inactive` | note "Keyboard shortcuts are not active right now." |
| `shortcuts-unavailable` | note "Keyboard shortcuts are not available on this system yet." |
| `window-memory-unavailable` | note about window memory; the two window switches are disabled |
| `setting-restore_on_reconnect`, `setting-linked_dividers` | switches in the Windows card |
| `setting-zone_gap_px` | range 0..32 "Space between zones (px)" (output `output[for="set-gap"]`) |
| `layout-transfer` | the "Export and import" card |
| `export-layout`, `import-layout` | the openers (`aria-expanded`) |
| `export-panel`, `export-text`, `export-copy` (`data-copied`), `export-close` | export panel, read-only text area, Copy, Close |
| `import-panel`, `import-text` (`aria-invalid`), `import-submit`, `import-close` | import panel, text area, Import, Close |

Demo flags for these: `?shortcuts=failed|off`, `?capabilities=none`; hooks
`demo(page, 'setShortcuts', 'ok'|'failed'|'off')` and `demo(page, 'setCapabilities',
'all'|'none')`. Copy tests grant `clipboard-read` / `clipboard-write` or replace
`navigator.clipboard` with an init script (see `layout-transfer.spec.ts`).

## Known UI issues

There are none open. The three defects the audit found (zone buttons failing
WCAG 2.5.3 label-in-name, the hovered armed "Confirm delete" button, small text
on a selected zone in the dark theme) are fixed and pinned by the "fixed UI
issues (regression tests)" block in `a11y.spec.ts`; no axe rule is switched off.
If a new defect turns up, track it the same way: a focused test marked
`test.fail()` (it turns red once the UI is fixed, which is the cue to delete the
marker) and switch off only the one rule it trips in the general scans.

A zone button's accessible name is its visible text ("Zone 1, 1280 × 1400,
50% × 100%"); the spelled-out size and the keyboard help are its accessible
description.

## Conventions

* `fixtures.ts` exports `test` (use it instead of `@playwright/test`'s). Its
  automatic **guard** fixture fails *every* test that
  * requests anything outside the dev server's origin (`context.on('request')`
    plus web sockets; data:/blob:/about: are fine), or
  * logs a console error or throws an uncaught exception.
  Opt out only for a negative control: `test.use({ strictGuards: false })`.
* Tests open `/?nopersist` (everything in memory, nothing shared) via
  `openApp(page)`. Tests of persistence use `?reset` first and then a plain
  visit; each Playwright test has its own browser context, so `localStorage` is
  never shared.
* Selectors are `data-testid` (the list in `ui/README.md`) and ARIA roles/names.
  Two helpers read internals on purpose: `focusRingVisible()` looks at the
  `.face` of a zone and at `::before` of a divider, because that is where the
  app draws the focus ring.
* Pointer positions are fractions of the canvas box (`editorUi(page).pointAt`)
  and are picked away from the 0.5% rounding step so a pixel of rounding never
  changes the outcome. There is no test at *exactly* 5%.
* No fixed sleeps: assertions auto-wait. Timers (delete confirmation 5 s,
  Saved badge 2.5 s) are driven with `page.clock`.

## CI

These tests are **not** in the CI workflow. A job would need, after
`npm ci` in `ui/`:

```yaml
- run: npx playwright install --with-deps chromium   # or: use the runner's Chrome
- run: npm run e2e
  env:
    PW_CHANNEL: chromium        # 'chrome' if Google Chrome is installed on the runner
- uses: actions/upload-artifact@v4
  if: failure()
  with: { name: playwright-report, path: ui/e2e/.report }
```

With `CI` set the config retries once, uses 2 workers, starts its own dev
server (no reuse) and writes an HTML report to `ui/e2e/.report`.
