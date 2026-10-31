# Progress

## Verification status — read this first

State on 2026-10-31. `main` is on the **public** repository `github.com/nAItiklearn/krova`. There is no tag and no published release (`RELEASE_PUBLISHED = false` in `website/src/site.ts`). The owner downloaded and ran the Windows installer built by `release.yml` (started by hand with `workflow_dispatch`), so the Windows build and bundle jobs work on GitHub.

What has actually been run, by whom, and where:

| Area | State |
|---|---|
| `krova-core`, `krova-platform` | `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` clean on Windows (GNU toolchain, WinLibs first on PATH) **and** for both macOS targets (`aarch64-apple-darwin`, `x86_64-apple-darwin`, checked from Windows; clippy type-checks without linking). Tests: **core 176 passed**, **platform 72 passed**, plus ignored tests that need a real desktop (the overlay window, real Notepad windows, real hot keys). The Rust agent ran those by hand; see `docs/features-branch-report.md`. |
| `krova-platform::mac*` (everything macOS-only) | Compiles and passes `clippy -D warnings` on `macos-latest` in CI and for both macOS targets locally. **Never run on a Mac**: no permission prompt, event tap, Accessibility call or overlay window has executed. Hand-declared FFI, no third-party crates. `mac_math` (not cfg-gated) is compiled and tested on every OS. |
| `src-tauri` | fmt and `clippy --all-targets -D warnings` clean on Windows and for both macOS targets (the macOS check needs a stand-in C compiler for one build script; recipe in the README). **43 tests pass** (+3 ignored desktop tests). The owner ran the Windows build on 2026-10-30: Shift-drag snapping, the tray and the editor work ("working nicely"). That is a manual smoke test; drag latency, real monitor unplug and replug, linked dividers with a real mouse and mixed-DPI setups are not tested. Separate workspace, so the root commands do not cover it: use `--manifest-path src-tauri/Cargo.toml`. |
| `ui/` | **166 vitest tests**, `svelte-check` (0 errors, 0 warnings), `vite build`, the e2e type check, and **240 Playwright tests** against the demo mock backend (Edge, Windows only; the last full run was by the UI agent on the branch that is now on `main`). Earlier e2e runs found three real accessibility bugs, since fixed and kept as regression tests. The UI also runs inside the real Tauri app on Windows. |
| `website/` | Static Astro site, 15 pages (home, download, performance, changelog, compare, guides, support, privacy, license) plus the real editor as a live demo at `/demo/`; light, dark and system theme. `npm run build` and the link check pass, and the build job is green on GitHub. Not tested in Firefox or Safari or with a screen reader. The deploy job fails until the owner switches on Pages (see below). Download buttons stay hidden until `REPO_PUBLIC` and `RELEASE_PUBLISHED` are flipped in `website/src/site.ts`; `REPO_PUBLIC` is already true. |
| `.github/workflows/*` | `ci.yml` runs on Windows, macOS and Ubuntu. At commit `746c4b0` (2026-10-31) every CI job was green: core on Windows, macOS and Ubuntu, the UI job, and the app on Windows and macOS. (A dead-code lint that only appears in a macOS test build failed the macOS app job at `e4297aa`; `5b17cf3` fixed it.) `release.yml` ran once by hand and produced the Windows installer. `pages.yml` builds the site; its deploy job fails until Pages is enabled. The e2e suite is **not** part of CI. |

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

## Owner actions (only the repository owner can do these)

- [ ] Settings → Pages → Build and deployment → Source: **GitHub Actions**. Until then the `website` workflow's deploy job fails (its build job is fine).
- [ ] Settings → Code security → enable **private vulnerability reporting** (`SECURITY.md` points people to it).
- [ ] Add a protection rule for `v*` tags.
- [ ] Delete the local branch `backup-pre-rewrite` (never pushed; it holds the history from before the rewrite, with the old email address).
- [ ] Hand-test on a real Windows PC: the keyboard shortcuts with a real keyboard, unplugging and replugging a monitor, linked dividers with a real mouse, a mixed-DPI setup, and that closing the editor window drops the memory use.
- [ ] Review the privacy page once more against a running build (WebView2 profile folder, the Run key, hot keys): the reviewer verified it from source only.
- [ ] When ready to release: push a `v*` tag by hand, check the draft, then set `RELEASE_PUBLISHED = true` in `site.ts`. The asset names in `site.ts` (`Krova-windows-x64-setup.exe`, `Krova-macos-universal.dmg`) must match `release.yml`.

## Not done

- [ ] Measurements: drag latency; the official `measure-idle.ps1` run for the new tray-only state (the agent's own probe measured about 6 MB private with the window closed, about 200 MB with the editor open; those are not on the website until the official script has produced them)
- [ ] A hands-on test on a Mac
- [ ] Code signing and notarisation
- [ ] Add the e2e suite to CI (after `npm ci` in `ui/`: `npm run e2e`; Edge is preinstalled on `windows-latest`)
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
