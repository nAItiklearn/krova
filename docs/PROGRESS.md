# Progress

## Verification status — read this first

**Nothing in this repository has been committed yet** (`git init` ran; no commit exists).

What has actually been run, by whom:

| Area | State |
|---|---|
| `krova-core` and `krova-platform` on Windows | Owner's gate on 2026-10-30: `cargo fmt --all` clean; `cargo test --workspace` all passing (krova-core 66, krova-platform 58 passed + 1 ignored, which is the `win_overlay` test that needs an interactive desktop). Clippy found two `drain_collect` lints in `mac_math.rs` tests; fixed, and `cargo clippy --workspace --all-targets -- -D warnings` then finished clean on the agent's re-run. A final fmt + test re-run after that fix is still pending. This includes the profile ops, the overlay renderer, `win_overlay` (compiled; only its ignored smoke test is unrun) and the `create_overlay` trait method. |
| `krova-platform::mac*` (everything macOS-only) | **Never compiled** (no Mac; not built on Windows). Hand-declared FFI, no third-party crates. `mac_math` (not cfg-gated) is compiled and tested on every OS. First real build: the `macos-latest` CI job. |
| `src-tauri` (model, engine, commands, tray, lib) | **Never compiled.** Separate workspace, so `cargo test --workspace` at the root does not build it. Icons are now generated and committed, so it should build locally. |
| `ui/` (Svelte 5 demo + app) | Subagent ran it: 41 vitest tests, `svelte-check` clean, `vite build`, 71 headless-Chrome checks against the **mock** backend. The real Tauri backend has never run (invoke names and args were read-checked against the Rust commands). |
| `website/` (Astro) | Subagent built it and ran Lighthouse (100/100/100/100) in Chromium/Edge. Not tested in Firefox/Safari. |
| `.github/workflows/release.yml` | Adapted from Windy's working workflow; **never run**. |

Build order for the app (the Tauri config has no `before*Command` hooks, so nothing depends on the hook working directory):

```
npm install --prefix ui
npm run build --prefix ui              # tauri bundles ui/dist
cargo test --manifest-path src-tauri/Cargo.toml
npx -y @tauri-apps/cli@2 dev           # needs `npm run dev --prefix ui` running in another terminal
```

Toolchain note: `gcc` first on PATH is the old `C:\MinGW` one (`mingw32`). The WinLibs one is at `C:\Users\Lenovo\AppData\Local\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin` and must come first.

## Done (written)

- [x] DECISIONS.md, SPEC.md, PORTING-MAP.md, ADR-001-stack.md, `.claude/agents`
- [x] `krova-core`: geometry, zone, monitor matching, edge snap, config persistence, layout editing (`edit`), placement math (`window`), drag state machine (`snap`), profile ops
- [x] `krova-platform`: `Platform` trait, `NullPlatform`, monitor-id parsing, input coalescing, overlay renderer
- [x] Windows layer: monitors, placement with DWM frame compensation, `WH_MOUSE_LL` drag detection, elevated/other-desktop window skipping, autostart, native overlay window
- [x] macOS layer: permission helpers, NSScreen monitors, Accessibility placement, CGEventTap input, SMAppService/LaunchAgent autostart, NSWindow overlay (all unverified)
- [x] `src-tauri`: single-instance tray app, nine commands, drag engine (`plan` is pure and tested), CSP, minimal capabilities (events only; no fs/shell/http plugins), generated icons in `src-tauri/icons`
- [x] `ui/`: editor (split, drag dividers, delete/merge, templates, layouts), settings, demo mode (`npm run demo`), TS mirror of the edit rules with tests
- [x] `website/`: home, privacy, license, 404
- [x] CI (`ci.yml`): core (win/mac/linux), ui, app (win/mac). Release workflow (`release.yml`): builds installers on manual run; creates a GitHub Release **only** on a `v*` tag push. No tag has been pushed.

## Not done

- [ ] Final fmt + test re-run, then the first commit (stage explicit paths)
- [ ] Run the app for the first time (`src-tauri` has never compiled)
- [ ] Create the GitHub repository. `REPO_URL` in `website/src/site.ts` is set to `https://github.com/nAItiklearn/krova`: the owner comes from Windy's remote and the repo name is an **assumption**. Flip `RELEASE_PUBLISHED` only after a real release exists. The download names in `site.ts` (`Krova-windows-x64-setup.exe`, `Krova-macos-universal.dmg`) must match `release.yml`.
- [ ] Playwright tests (`ui/e2e/` does not exist)
- [ ] Security review (`docs/SECURITY-REVIEW.md`, `SECURITY.md`), README, CONTRIBUTING
- [ ] Measurements (drag latency, idle CPU/RAM) on real hardware
- [ ] Code signing / notarization (the unsigned macOS build may not be able to register for login or get past Gatekeeper)

## Follow-ups (deliberately deferred)

- **Astro advisories:** `npm audit` reports 5 advisories (1 critical) in Astro 5.18.2 and its dependencies; the suggested fix is Astro 7, a breaking change that needs Node 22.23.4 or later. The affected features (SSR, server islands, MDX, transitions, image optimisation, dev server) are unused by this static site, so this was not upgraded now. Revisit before publishing the site.

## Known gaps / decisions to revisit

- Windows monitor `hardware_id` is the PnP id (e.g. `DELA0B7`) without the EDID serial; identical models are told apart by device path, then position.
- Elevated target windows are skipped at press time; no "run as administrator" mode (non-goal).
- `win.rs` and `mac_input.rs` delay `Release` by 30 ms after a real drag so the OS finishes its own move loop; tune on hardware.
- macOS: blanket `#![allow(dead_code, unused_unsafe, unused_imports, ...)]` in the mac files exist only so a first CI run is not sunk by warnings under `-D warnings`. Tighten after the build is green. The overlay is the lowest-confidence macOS item; if it misbehaves, make `create_overlay` return `Err(Unsupported)`.
- macOS: `start_input` fails until Accessibility is granted. `get_state` retries it once permission is `Granted`; a restart may still be needed on some systems.
- Privacy page claims about storage location, "no update pings or crash reports", and what is read in memory were written from the spec and should be reviewed by someone who knows the implementation.
