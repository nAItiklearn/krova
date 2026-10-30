# Progress

## Verification status — read this first

Local commits exist (initial import, progress, README/CONTRIBUTING/CI, security hardening, UI accessibility fixes). **Nothing is pushed**: there is no remote, no tag and no release.

What has actually been run, by whom, on Windows (GNU toolchain, WinLibs first on PATH):

| Area | State |
|---|---|
| `krova-core`, `krova-platform` | `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo test`: **core 80 passed, platform 60 passed + 1 ignored** (the `win_overlay` smoke test needs an interactive desktop). |
| `krova-platform::mac*` (everything macOS-only) | **Never compiled** (no Mac; not built on Windows). Hand-declared FFI, no third-party crates. `mac_math` (not cfg-gated) is compiled and tested on every OS. First real build: the `macos-latest` CI job. |
| `src-tauri` | fmt, `clippy --all-targets -D warnings` clean, **17 tests pass**, `cargo build` produces `krova-app.exe`. **The app has never been launched**, so the tray, mouse hook, overlay and UI commands are untested at runtime. Separate workspace, so the root commands do not cover it: use `--manifest-path src-tauri/Cargo.toml`. |
| `ui/` | 41 vitest tests, `svelte-check`, `vite build`, and **159 Playwright tests** against the demo mock backend (Edge, Windows only). The tests found three real accessibility bugs (armed "Confirm delete" unreadable on hover, zone accessible names not containing their visible text, dark-theme selected-zone contrast 3.37:1); all three are fixed (contrast re-computed: 6.46, 6.54 and 7.63:1) and kept as regression tests, plus a real `<h1>`. All 159 e2e tests, 41 unit tests and `svelte-check` (196 files, 0 errors) pass on an independent re-run. The real Tauri backend has never run. |
| `website/` | Built; Lighthouse 100/100/100/100 in Chromium/Edge. Not tested in Firefox/Safari. |
| `.github/workflows/*` | **Never run on GitHub** (no repository yet). `ci.yml` and `release.yml` were syntax-checked and their shell steps exercised against fake files only. |

Build order for the app (the Tauri config has no `before*Command` hooks, so nothing depends on a hook's working directory):

```
npm ci --prefix ui
npm run build --prefix ui              # tauri bundles ui/dist
cargo test --manifest-path src-tauri/Cargo.toml
npx -y @tauri-apps/cli@2 dev           # needs `npm run dev --prefix ui` running in another terminal
```

Toolchain: `C:\MinGW\bin` (old, 32-bit) can come before WinLibs on PATH and breaks Rust's `dlltool` (`Invalid bfd target`). Put `...\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin` first.

## Done (written)

- [x] DECISIONS.md, SPEC.md, PORTING-MAP.md, ADR-001-stack.md, `.claude/agents`
- [x] `krova-core`: geometry, zones, monitor matching, edge snap, config persistence, layout editing, placement math, drag state machine, profile ops, input limits
- [x] `krova-platform`: `Platform` trait, `NullPlatform`, monitor-id parsing, input coalescing, overlay renderer; Windows layer with native overlay; macOS layer (unverified)
- [x] `src-tauri`: single-instance tray app, nine commands, drag engine, CSP, minimal capabilities, icons
- [x] `ui/`: editor, settings, demo mode, TS mirror of the edit rules, unit + e2e tests
- [x] `website/`: home, privacy (corrected by the security review), license, 404
- [x] README, CONTRIBUTING, SECURITY.md, `docs/SECURITY-REVIEW.md`, Dependabot config
- [x] CI (`ci.yml`) and a release workflow (`release.yml`) that publishes **only** on a `v*` tag push. No tag has been pushed.

## Security review (docs/SECURITY-REVIEW.md): status of findings

No Critical or High findings in Krova's own code; privacy promises hold for Krova's code (network, titles, telemetry), with the privacy page corrected where it overstated.

**Fixed (with tests):** F-04 (zone/name/size limits, config size cap), F-05 (overlay pixel cap), F-06 (settings sanitised on load), F-07(d) (`schema_version` truncation), F-08 (CSP `base-uri`/`form-action`/`object-src`, `freezePrototype`, listen/unlisten-only events), F-09 (id overflow, validated `edit_*` input), F-12 (Dependabot), F-14 (privacy page), F-16 (saturating rect math, DIB length check).

**Open, deliberately deferred:**
- F-01 (Medium): installers are unsigned and not notarised.
- F-02 (Medium): actions pinned by tag not SHA; `npx @tauri-apps/cli@2` and `rust-toolchain@stable` float; any `v*` tag publishes a non-draft release with no approval. Fix before the first real tag (pin SHAs, add the CLI to a lockfile, draft releases, protect tags).
- F-03 (Medium): the macOS FFI layer has never been compiled or run. Do not ship the macOS artifact until it has been built on `macos-latest` and exercised on Apple Silicon and Intel.
- F-07(a-c): no in-app indication when the config cannot be saved (newer schema/corrupt); a second recovery overwrites the first `.bak`.
- F-08 (rest): the nine commands have no per-command app-manifest permissions.
- F-10, F-11: hook-thread edge cases (orphan thread, stale thread ids, no health check if Windows drops a slow hook); placement can block on an unresponsive window.
- F-13: the single-instance plugin uses a fixed `/tmp` socket path on macOS.
- F-15, F-17: injected input is not filtered; the config temp file name is fixed.
- `npm audit`: `ui/` has 6 advisories in dev tooling only (vitest/tinypool; 0 with `--omit=dev`); `website/` has 5 in Astro 5.18.2, a static site that uses none of the affected features (Astro 7 is a breaking upgrade needing Node 22.23.4+). Re-audit before publishing the site.

## Before the first push

- [ ] Create the GitHub repository. `REPO_URL` in `website/src/site.ts` is `https://github.com/nAItiklearn/krova`: the owner comes from Windy's remote and the repo name is an **assumption**.
- [ ] Decide what email goes in the commit history. All four commits are authored with the owner's real name and personal Gmail address (review F-19). If that should not be public, switch to the GitHub noreply address and rewrite the (local-only) history before pushing.
- [ ] Confirm the three assumptions at the top of `SECURITY.md`, enable GitHub private vulnerability reporting, then delete that note.
- [ ] Review the privacy page once more against a running build (WebView2 profile folder, the Run key / LaunchAgent, Input Monitoring on macOS): the reviewer verified it from source only.
- [ ] Keep `RELEASE_PUBLISHED = false` in `site.ts` until a real release exists. The asset names in `site.ts` (`Krova-windows-x64-setup.exe`, `Krova-macos-universal.dmg`) must match `release.yml`.

## Not done

- [ ] Launch the app for the first time and dogfood it (tray, drag, overlay, snapping, settings, autostart)
- [ ] Measurements (drag latency, idle CPU/RAM) on real hardware
- [ ] A first macOS build (CI) and a hands-on test on a Mac
- [ ] Code signing and notarisation
- [ ] Add the e2e suite to CI (after `npm ci` in `ui/`: `npm run e2e`; Edge is preinstalled on `windows-latest`)

## Known gaps / decisions to revisit

- **Windows-GNU linking:** `src-tauri` originally listed `crate-type = ["staticlib", "cdylib", "rlib"]` (Tauri's mobile template). On GNU the cdylib DLL failed to link with `export ordinal too large: 99913` (a DLL holds at most 65535 exports). Krova is desktop-only, so those crate types were removed; `src-tauri` now links without lld and **no `.cargo/config.toml` is needed**. If lld is ever wanted: `winget install MartinStorsjo.LLVM-MinGW.UCRT`, then `[target.x86_64-pc-windows-gnu] rustflags = ["-C", "link-arg=-fuse-ld=lld"]` (not before lld is installed: it would break every GNU-target build).
- **Linker warning, unresolved:** every `src-tauri` link on GNU prints `.rsrc merge failure: multiple non-default manifests`. The build succeeds, but whether the embedded manifest (Common Controls v6, DPI awareness) is the right one is unknown. Check on first launch: blurry UI on a scaled display, odd dialogs, or a missing-entry-point error. CI builds with MSVC.
- Windows monitor `hardware_id` is the PnP id (e.g. `DELA0B7`) without the EDID serial; identical models are told apart by device path, then position. macOS stores a serial where reported.
- Elevated target windows are skipped at press time; no "run as administrator" mode (non-goal).
- `win.rs` and `mac_input.rs` delay `Release` by 30 ms after a real drag so the OS finishes its own move loop; tune on hardware.
- macOS: blanket `#![allow(dead_code, unused_unsafe, unused_imports, ...)]` in the mac files only keeps a first CI run from failing on warnings; tighten once green. The overlay is the lowest-confidence macOS item; if it misbehaves, make `create_overlay` return `Err(Unsupported)`.
- macOS: `start_input` fails until Accessibility is granted. `get_state` retries it once permission is `Granted`; a restart may still be needed. The Accessibility prompt helper (`prompt_for_accessibility`) is not yet wired to any command.
