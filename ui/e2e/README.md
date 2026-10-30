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
| `keyboard.spec.ts` | Tab order, visible focus ring at every stop, a complete edit with the keyboard only, live region |
| `emulation.spec.ts` | `prefers-reduced-motion`, `prefers-color-scheme` light and dark, `forced-colors` |
| `a11y.spec.ts` | axe-core (WCAG 2.0 to 2.2, A and AA) over 10 states x light and dark; best-practice findings are only recorded as annotations |
| `privacy.spec.ts` | the page only talks to the local dev server, no external references, CSP, and a self-test of the guard |

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
