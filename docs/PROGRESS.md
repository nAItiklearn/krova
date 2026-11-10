# Progress

## Verification status — read this first

State on 2026-10-31. `main` is on the **public** repository `github.com/nAItiklearn/krova`. **v0.1.0 is published (2026-10-31), Windows only**: assets `Krova-windows-x64-setup.exe` and `SHA256SUMS.txt`, notes from `docs/release-notes/v0.1.0.md`. `RELEASE_PUBLISHED = true`, `MAC_RELEASED = false` in `website/src/site.ts`; the README says it is an early preview, not ready for everyday use. The owner downloaded and ran the Windows installer built by `release.yml` (started by hand with `workflow_dispatch`), so the Windows build and bundle jobs work on GitHub.

What has actually been run, by whom, and where:

| Area | State |
|---|---|
| `krova-core`, `krova-platform` | `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` clean on Windows (GNU toolchain, WinLibs first on PATH) **and** for both macOS targets (`aarch64-apple-darwin`, `x86_64-apple-darwin`, checked from Windows; clippy type-checks without linking). Tests: **core 176 passed**, **platform 72 passed**, plus ignored tests that need a real desktop (the overlay window, real Notepad windows, real hot keys). The Rust agent ran those by hand; see `docs/features-branch-report.md`. |
| `krova-platform::mac*` (everything macOS-only) | Compiles and passes `clippy -D warnings` on `macos-latest` in CI and for both macOS targets locally. **Never run on a Mac**: no permission prompt, event tap, Accessibility call or overlay window has executed. Hand-declared FFI, no third-party crates. `mac_math` (not cfg-gated) is compiled and tested on every OS. |
| `src-tauri` | fmt and `clippy --all-targets -D warnings` clean on Windows and for both macOS targets (the macOS check needs a stand-in C compiler for one build script; recipe in the README). **43 tests pass** (+3 ignored desktop tests). The owner ran the Windows build on 2026-10-30: Shift-drag snapping, the tray and the editor work ("working nicely"). That is a manual smoke test; drag latency, real monitor unplug and replug, linked dividers with a real mouse and mixed-DPI setups are not tested. Separate workspace, so the root commands do not cover it: use `--manifest-path src-tauri/Cargo.toml`. |
| `ui/` | **166 vitest tests**, `svelte-check` (0 errors, 0 warnings), `vite build`, the e2e type check, and **240 Playwright tests** against the demo mock backend (Edge, Windows only; the last full run was by the UI agent on the branch that is now on `main`). Earlier e2e runs found three real accessibility bugs, since fixed and kept as regression tests. The UI also runs inside the real Tauri app on Windows. |
| `website/` | Static Astro site, 15 pages (home, download, performance, changelog, compare, guides, support, privacy, license) plus the real editor as a live demo at `/demo/`; light, dark and system theme. `npm run build` and the link check pass, and the build job is green on GitHub. Not tested in Firefox or Safari or with a screen reader. Deployed by `pages.yml` to GitHub Pages. Download buttons stay hidden until `REPO_PUBLIC` and `RELEASE_PUBLISHED` are flipped in `website/src/site.ts`; `REPO_PUBLIC` is already true. |
| `.github/workflows/*` | `ci.yml` runs on Windows, macOS and Ubuntu. At commit `746c4b0` (2026-10-31) every CI job was green: core on Windows, macOS and Ubuntu, the UI job, and the app on Windows and macOS. (A dead-code lint that only appears in a macOS test build failed the macOS app job at `e4297aa`; `5b17cf3` fixed it.) `release.yml` ran once by hand and produced the Windows installer. `pages.yml` builds and deploys the site (run #5 at `36c87cf` succeeded, deploy included). Since `0e62116` the `ui-e2e` job runs the Playwright suite on `windows-latest` (Edge); the first run, at `0e62116`, was green along with every other job. |

Build order for the app (the Tauri config has no `before*Command` hooks, so nothing depends on a hook's working directory):

```
npm ci --prefix ui
npm run build --prefix ui              # tauri bundles ui/dist
cargo test --manifest-path src-tauri/Cargo.toml
npx -y @tauri-apps/cli@2 dev           # needs `npm run dev --prefix ui` running in another terminal
```

Toolchain: `C:\MinGW\bin` (old, 32-bit) can come before WinLibs on PATH and breaks Rust's `dlltool` (`Invalid bfd target`). Put `...\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin` first.

## Done

- [x] DECISIONS.md, SPEC.md, PORTING-MAP.md, ADR-001-stack.md, `.claude/agents`
- [x] `krova-core`: geometry, zones, monitor matching, edge snap, config persistence, layout editing, placement math, drag state machine, profile ops, input limits, snap registry (window memory), linked dividers, layout import and export, zone gap
- [x] `krova-platform`: `Platform` trait, `NullPlatform`, monitor-id parsing, input coalescing, overlay renderer; Windows layer with native overlay, global hot keys and window-event hook; macOS layer (compile-checked only)
- [x] `src-tauri`: single-instance tray app, drag engine, keyboard zone control, window memory on reconnect, lean window lifecycle (the editor window is destroyed on close), CSP, minimal capabilities, per-monitor DPI manifest, icons
- [x] `ui/`: editor, settings (shortcuts, window memory, zone gap, layout files), demo mode, TS mirror of the edit rules, unit and e2e tests
- [x] `website/`: guides, changelog, compare (including rows where Krova is weaker), performance, privacy, support, live demo
- [x] README, CONTRIBUTING, SECURITY.md, `docs/SECURITY-REVIEW.md`, Dependabot config
- [x] CI, a site workflow (`pages.yml`) and a release workflow (`release.yml`) that creates only a **draft** release and only on a `v*` tag push. No tag has been pushed.
- [x] Public repository, history rewritten to the GitHub noreply address before the first push

## Security review (docs/SECURITY-REVIEW.md): status of findings

No Critical or High findings in Krova's own code; privacy promises hold for Krova's code (network, titles, telemetry), with the privacy page corrected where it overstated. The review covered the code as it was on 2026-10-30.

**Fixed (with tests):** F-02 (actions pinned by commit SHA, Tauri CLI pinned to 2.12.3 and Rust to 1.99.1 in the workflows, releases are drafts), F-04 (zone/name/size limits, config size cap), F-05 (overlay pixel cap), F-06 (settings sanitised on load), F-07(d) (`schema_version` truncation), F-08 (CSP `base-uri`/`form-action`/`object-src`, `freezePrototype`, listen/unlisten-only events), F-09 (id overflow, validated `edit_*` input), F-12 (Dependabot), F-14 (privacy page), F-16 (saturating rect math, DIB length check).

**Not covered by the review yet (added afterwards):** global hot keys, the window-event hook behind linked dividers, layout import (an untrusted JSON file, limited to 64 KiB and validated), window memory, and the app-rules feature. Each was written with limits and validation, but none has had a reviewer's pass.

**Open, deliberately deferred:**
- F-01 (Medium): installers are unsigned and not notarised.
- F-02 (rest): a `v*` tag protection rule is the owner's to set (a tag only creates a draft, but protect it anyway).
- F-03 (Medium): the macOS layer has never run. It compiles and lints clean, which is not the same as working. Do not ship the macOS artifact until it has been exercised on Apple Silicon and Intel.
- F-07(a-c): no in-app indication when the config cannot be saved (newer schema/corrupt); a second recovery overwrites the first `.bak`.
- F-08 (rest): the Tauri commands have no per-command app-manifest permissions.
- F-10, F-11: hook-thread edge cases (orphan thread, stale thread ids, no health check if Windows drops a slow hook); placement can block on an unresponsive window.
- F-13: the single-instance plugin uses a fixed `/tmp` socket path on macOS.
- F-15, F-17: injected input is not filtered; the config temp file name is fixed.
- `npm audit`: `ui/` has advisories in dev tooling only (vitest/tinypool); `website/` has advisories in Astro 5.18.2, a static site that uses none of the affected features. Dependabot has opened major-version upgrade branches (Vite 8, Vitest 5, TypeScript 7, Astro 7, the Svelte Vite plugin 7). They are breaking changes and need a test run each before merging; none is merged.

## Manual tests for the notch branch

Run the app (`npm run tauri dev` or a build) on Windows. Watch Task Manager: with the notch idle, the Krova process stays at 0% CPU.

1. Start Krova: a small ink pill with an orange dot appears at the top centre of the primary monitor, 4 px below the edge, above every other window.
2. Hover it: nothing for about 150 ms, then it grows smoothly (no bounce) to show the active layout name. Move away: it shrinks back. A quick pass over it does not expand it.
3. Left click: the quick panel opens just below the notch, centred on it, and has the focus. Click elsewhere or press Esc: it hides. Click the notch again: it reopens instantly (no white flash). Click the notch while the panel is open: it closes.
4. Right click: the menu shows Pause Krova, Layouts (the active one checked), Layout switcher, Cheat sheet, Open Krova, Hide notch. Pick another layout: the notch label follows. Pause: the dot turns grey and the label reads "Paused · name"; the tray's Enabled checkmark clears. Resume restores both.
5. Layout switcher / Open Krova / Cheat sheet bring the main window forward with that view (Cheat sheet is ignored by a UI that does not know it yet).
6. Drag the notch: it shrinks to the pill and follows the cursor; drop near the bottom right: it springs into the corner just above the taskbar. Restart Krova: it comes back there. Try all five positions, with the taskbar at the bottom and at the top.
7. Hide notch: it disappears; setting `notch.enabled` back to true in Settings (or the config) brings it back without a restart.
8. Start a fullscreen video (F11 in a browser, or a game): the notch hides on that monitor and returns when the app leaves fullscreen or loses the focus. A normal maximized window does not hide it.
9. Mixed DPI: with monitors at 100% and 150%, set `notch.monitor` to `cursor`, move the cursor to the other monitor and click a window there: the notch moves and is drawn at that monitor's scale (crisp, the same physical size in logical terms). Change the scale in Display settings: it redraws at the new size.
10. Unplug or rearrange monitors: the notch moves to the (new) primary monitor.
11. Share-safe (once the setting exists): the label shows "N zones" and the menu lists "Layout 1..N" instead of names.
12. Privacy: no window titles appear anywhere (label, menu, logs).

## Owner actions (only the repository owner can do these)

- [x] Settings → Pages → Source: GitHub Actions (the deploy job succeeded on 2026-10-31).
- [ ] Settings → Code security → enable **private vulnerability reporting** (`SECURITY.md` points people to it).
- [ ] Add a protection rule for `v*` tags.
- [ ] Delete the local branch `backup-pre-rewrite` (never pushed; it holds the history from before the rewrite, with the old email address).
- [ ] Hand-test on a real Windows PC: the keyboard shortcuts with a real keyboard, unplugging and replugging a monitor, linked dividers with a real mouse, a mixed-DPI setup, and that closing the editor window drops the memory use.
- [ ] Review the privacy page once more against a running build (WebView2 profile folder, the Run key, hot keys): the reviewer verified it from source only.
- [ ] When ready to release: push a `v*` tag by hand, check the draft, then set `RELEASE_PUBLISHED = true` in `site.ts`. The asset names in `site.ts` (`Krova-windows-x64-setup.exe`, `Krova-macos-universal.dmg`) must match `release.yml`.

## Not done

- [ ] Measurements: drag latency, and resource use during a drag. (Done 2026-10-31 with the published v0.1.0 installer: official `measure-idle.ps1` runs for tray only, 1 process / 2.4 MB private, and editor open, 7 processes / 199 MB; `docs/measurements/idle-2026-10-31-*.txt`, charted in the README. Closing the editor was checked to end all WebView2 processes.)
- [ ] A hands-on test on a Mac
- [ ] Code signing and notarisation
- [ ] Review of the surfaces listed above under "Not covered by the review yet"

## Known gaps / decisions to revisit

- **Windows-GNU linking:** `src-tauri` originally listed `crate-type = ["staticlib", "cdylib", "rlib"]` (Tauri's mobile template). On GNU the cdylib DLL failed to link with `export ordinal too large: 99913` (a DLL holds at most 65535 exports). Krova is desktop-only, so those crate types were removed; `src-tauri` now links without lld and **no `.cargo/config.toml` is needed**. If lld is ever wanted: `winget install MartinStorsjo.LLVM-MinGW.UCRT`, then `[target.x86_64-pc-windows-gnu] rustflags = ["-C", "link-arg=-fuse-ld=lld"]` (not before lld is installed: it would break every GNU-target build).
- **Manifest and DPI:** the app embeds its own manifest (`src-tauri/windows/app.manifest`, Common Controls v6 and per-monitor DPI v2), which replaced the linker's "multiple non-default manifests" warning. It works on the owner's single-scale setup; a mixed-DPI setup is untested.
- Windows monitor `hardware_id` is the PnP id (e.g. `DELA0B7`) without the EDID serial; identical models are told apart by device path, then position. macOS stores a serial where reported.
- Elevated target windows are skipped at press time; no "run as administrator" mode (non-goal).
- `win.rs` and `mac_input.rs` delay `Release` by 30 ms after a real drag so the OS finishes its own move loop; tune on hardware.
- macOS: blanket `#![allow(dead_code, unused_unsafe, unused_imports, ...)]` in the mac files predates the first green macOS CI run; tighten it now that the lints pass. The overlay is the lowest-confidence macOS item; if it misbehaves, make `create_overlay` return `Err(Unsupported)`.
- macOS: `start_input` fails until Accessibility is granted. `get_state` retries it once permission is `Granted`; a restart may still be needed. The Accessibility prompt helper (`prompt_for_accessibility`) is not yet wired to any command.
- Window memory, keyboard shortcuts and linked dividers are Windows-only (`capabilities` is false for them on macOS).

## Manual tests for the wave3 branch

Automated: `krova-core` unit tests for `workspace`, `focus` and `calm`; `krova-platform` tests for `desk` and `win_desk`; `src-tauri` `wave3::tests` (fake desk and platform); `ui` `wave3Mock.test.ts`. Ignored desktop tests, run by hand: `cargo test -p krova-platform -- --ignored visuals_start launching_notepad --nocapture`. Both passed on Windows 11 build 26300 (agent run); the Notepad test showed that `System32\notepad.exe` starts the packaged Notepad, whose process path differs, so a wait by path cannot find it (the test then finds the window by file name).

- [ ] Workspaces: snap three apps into zones on two monitors, capture, move them around and open another app, restore: each lands in its zone in turn, the other app is minimised (not closed), every entry says `placed`.
- [ ] Restore with launching: close one captured classic desktop app (VS Code, for example) and restore with `launch`: it starts and lands in its zone within 10 s; with launching off it reports `not_running`. A program copied to `%TEMP%` and captured from there is never started (`refused`).
- [ ] Focus dim: `toggle_focus_dim`, then click between windows: the dim fades in, the rounded hole follows focus with a spring; drag the focused window: the hole follows; click the desktop: the dim fades out. Check a 100 % and a 150 % monitor and a window spanning both. Task Manager shows no CPU for Krova while nothing moves.
- [ ] Calm desk: with floating windows and windows in zones, toggle: only the floating ones minimise; restore one by hand, toggle again: only the others come back.
- [ ] Highlight `active` at 2 px on Windows 11: the focused window's own border takes the colour and the previous one returns to normal; at 5 px an overlay ring follows and eases between windows; `all`: every window gets a ring (or a Windows border); `glow`: a soft halo. All disappear for full-screen video, minimised windows and windows on another virtual desktop; switching the highlight off restores every border colour.
- [ ] Share-safe: switch it on; `AppState.wave3.share_safe` is true and no overlay shows an app name or icon (none does today; this matters once the switcher and previews exist).

## Manual tests for the wave4 branch

Backend only (no Svelte view yet). Automated: core 384, platform 83 (+4 ignored), app 74 (+3 ignored), UI vitest 185, `svelte-check` clean. The items below need a real Windows desktop; none has been run yet.

- [ ] **Reserved edges.** With a third-party bar that does not register as an app bar (or just pretend), `set_reserved_edges` on one monitor (for example `left: 48`): drag-snap, keyboard moves, edge snaps and the overlay all stop 48 logical px from the left edge, on a 100 % and a 150 % or 200 % monitor; the other monitor is unchanged. A reserve of 400 on both sides of a small monitor is ignored for that axis. Unplug and replug the monitor: the restore uses the reduced area.
- [ ] **Focus follows mouse, basic.** Turn it on (default 350 ms). Rest the cursor over an inactive Notepad: it becomes active after about the delay; moving across windows without stopping activates nothing. Try 150 and 800 ms.
- [ ] **Focus follows mouse, refusals.** Note whether Windows lets the activation through (`SetForegroundWindow` foreground-lock rules) or flashes the taskbar button instead, and whether `pointer.focus_refused` counts it. This decides whether the feature is usable as built (the ignored test `rest_at_the_cursor_reports_what_would_happen` in `krova-platform` prints the outcome at the cursor).
- [ ] **Focus follows mouse, exclusions.** Nothing is activated: with a mouse button held, right after a click, over the desktop and the taskbar, over Krova's own editor, over Task Manager or another elevated window, while a context menu or the Start menu is open, while a video or game is full screen, over a window that has a modal dialog open.
- [ ] **Hover picker.** Turn `hover_picker` on, hold Shift (the activation modifier) and rest on Notepad's title bar: one `hover-picker` event with the monitor's zones (watch it in the devtools console or a test page); `place_hovered` moves Notepad into the zone and a later reconnect restores it there. Resting elsewhere sends `hover-picker-end`. A hung window (for example a debugger-paused process) does not stall the cursor watcher. Shift-drag snapping still works with the picker on.
- [ ] **Idle CPU.** With both pointer features on and the cursor still, Krova's CPU stays at 0 % in Task Manager (and the `krova-pointer` thread does not wake); with both off the thread is gone.
- [ ] **Check for updates.** The tray item and `open_releases_page` open the GitHub releases page in the default browser; no request is made by Krova itself (watch with Resource Monitor).
- [ ] **Languages.** Set `language` to `de` (stub, still English text) and to `system` on a German Windows: the tray labels come from the right table and relabel without a restart; a missing key shows English.
- [ ] **macOS** (when a Mac is available): `wave4_capabilities` are all false and nothing above runs.

New surfaces not yet reviewed (add to the review list): the pointer watcher (hook callback, `SendMessageTimeout` to other windows, `SetForegroundWindow`), `ShellExecuteW` with a fixed URL (guarded by `is_safe_https_url`), and the string tables (compiled in, size and length limits).
## Manual tests for the corners branch

Automated: `krova_core::corners` unit tests (outer corners incl. L-shaped and mixed-DPI sets, dwell, re-arm, buttons, cooldown, ripple), `win_corners` table tests, `src-tauri` corners tests, `ui/src/lib/mock/cornersMock.test.ts`. Real desktop: `cargo test -p krova-platform a_real_cursor_in_a_corner_fires_once -- --ignored` (moves the mouse into the primary monitor's top-left corner with `SendInput`; passed 2026-11-10 on Windows 11 10.0.26300, single 125% monitor).

For a security reviewer (not yet reviewed): the only change inside `mouse_proc` is `win_corners::on_mouse_move`, which does atomic loads, bounded indexing (`i < COUNT.min(64)`), no lock, no allocation, and at most one `PostThreadMessageW`; it cannot panic or block. The full-screen check reads the foreground window's rect and class name, never its title. Nothing about corners is logged or written except the settings themselves.

By hand, on a real Windows PC:
- [ ] Default settings, one monitor: throw the cursor into the top-right corner and stop. After about 0.3 s an orange quarter-circle blooms and fades there and the layout switcher opens for that monitor. Staying in the corner does nothing more.
- [ ] Wiggle a little in the corner (under ~24 px): nothing. Move clearly away and back: it fires again (not sooner than 0.6 s after the last fire).
- [ ] Pass through the corner without stopping: nothing.
- [ ] Drag a window (title bar held) into the top-right corner and keep holding: nothing fires. Release there and keep resting: it fires one dwell later.
- [ ] Bottom-right with the default `calm_desk`: the ripple shows and the `hot-corner` event arrives (devtools: `api.onHotCorner`); calm desk itself runs once that branch is merged.
- [ ] Two monitors side by side: the two corners where the screens meet never fire; the far corners of both screens do. Repeat with a staggered (L-shaped) arrangement and with one screen at 100 % and the other at 150-200 %: the hit zone feels the same on both.
- [ ] Rearrange or unplug a monitor while Krova runs: the new outer corners work without a restart.
- [ ] A full-screen video or game in front: nothing fires (ripple included). Set `ignore_fullscreen` off: it fires.
- [ ] Switch Krova off in the tray: no corner fires. Back on: they work.
- [ ] Windows Settings, Accessibility, Visual effects, Animation effects off: the ripple is a still quarter-circle that disappears after a moment.
- [ ] Task Manager with the cursor resting in a set corner and elsewhere: no CPU use beyond the fire itself.
