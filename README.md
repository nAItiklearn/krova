# Krova

Krova is a free, offline window zone manager for Windows 10/11 and macOS 13 or later, released under the MIT license.

You define zones on each monitor. While you drag a window, hold Shift (the default; you can change the key) and Krova shows the zones of the monitor under the cursor. Drop the window on a zone and it snaps to it.

- **Zones per monitor.** Each monitor has its own layouts. Zones are stored as proportions of the screen, so a layout survives a change of resolution or scaling.
- **Layouts come back.** Krova recognises a monitor when you reconnect it and restores the layouts you made for it.
- **Edge snapping (optional).** Drag a window to a screen edge or corner to snap it to a half or a quarter. It does not trigger on the border between two monitors.
- **Tray app.** It lives in the system tray (the menu bar on macOS) with Enable/Disable, Edit layouts, Settings, Launch at login and Quit.
- **No accounts, no telemetry, no network.** See [Privacy](#privacy).

The full behaviour is in [docs/SPEC.md](docs/SPEC.md). That is the design; the [Status](#status) section says what has actually been built and run.

## Status

**Version 0.1, in development. Nothing has been released, and there are no installers to download yet.** This section is deliberately blunt. [docs/PROGRESS.md](docs/PROGRESS.md) is the detailed ledger of what has been compiled and run versus only written; read it before relying on anything here.

- **Windows layer:** compiled and unit-tested on Windows (monitors, window placement, drag detection, autostart, the native overlay). **The app itself has never been launched**, so the tray, the mouse hook, the overlay and the UI-to-Rust commands have not been tried at runtime. It has not been dogfooded.
- **macOS layer:** written without a Mac and **never compiled**. The first real build will be the macOS CI job. Treat every macOS claim as unverified.
- **Linux:** not supported. The core crates build and test there (the platform crate falls back to a stub), but there is no Linux app.
- **UI:** the editor and settings were exercised in a browser against a mock backend. The real Tauri backend has not run.
- **Not done yet:** code signing and macOS notarization (an unsigned macOS build may be blocked by Gatekeeper), measurements of drag latency and idle CPU/RAM on real hardware, an independent security review, and the end-to-end browser tests.
- **Known limits:**
  - On Windows, windows of programs running as administrator are skipped, because Windows does not let a normal process move them.
  - On macOS, Krova needs the Accessibility permission (System Settings, Privacy & Security, Accessibility) before it can move other apps' windows. The mouse watcher may also need Input Monitoring on recent macOS versions; this is unverified because the macOS layer has never run.
  - Some apps refuse to be resized below a minimum size. The code is designed to keep such a window inside the work area instead of forcing it; this has not been tried on real apps.

## Repository layout

| Path | What it is |
| --- | --- |
| `crates/krova-core` | Pure Rust, no OS code: zones, layouts, monitor matching, edge-snap rules, placement math, the drag state machine, layout editing and config persistence. `#![forbid(unsafe_code)]`. |
| `crates/krova-platform` | The OS layer behind a `Platform` trait: Windows and macOS implementations behind `cfg`, plus a stub for other systems. |
| `src-tauri` | The Tauri 2 app: tray, commands, the drag engine. It is its own Cargo workspace, so the root `cargo` commands do not build it. |
| `ui` | Svelte 5 + Vite + TypeScript: zone editor and settings. Also runs in a browser in demo mode. See [ui/README.md](ui/README.md). |
| `website` | The static Astro site (not deployed). See [website/README.md](website/README.md). |
| `docs` | [SPEC](docs/SPEC.md), [ADR-001 (stack)](docs/ADR-001-stack.md), [PORTING-MAP](docs/PORTING-MAP.md), [PROGRESS](docs/PROGRESS.md) and [DECISIONS](docs/DECISIONS.md). |
| `assets/icon` | The icon source (SVG). The generated app icons are checked in under `src-tauri/icons`. |
| `.github/workflows` | CI (`ci.yml`) and the release workflow (`release.yml`). |

## Try the UI in a browser (demo mode)

This needs only Node. It shows the editor and settings against fake monitors; nothing is moved and nothing is saved to a real config.

```
npm install --prefix ui
npm run demo --prefix ui
```

Then open <http://localhost:1420> (the demo uses that port; if another dev server holds it, run `npm run demo --prefix ui -- --port 5173`).

## Build and run the app on Windows

You need a Rust toolchain, Node, and the Microsoft Edge WebView2 runtime (Tauri's webview; it ships with Windows 11 and is present on most up-to-date Windows 10 systems). Tauri officially supports the MSVC toolchain, and that is what CI uses. The Windows-GNU toolchain has been used for development here: it works as long as a current MinGW-w64 (for example WinLibs) comes first on `PATH`, ahead of any old `C:\MinGW`.

The Tauri config has **no** `beforeDevCommand` / `beforeBuildCommand` hooks on purpose, so the UI is always built or served by you, from the repo root:

```
npm install --prefix ui
npm run build --prefix ui
```

Then either bundle with the Tauri CLI:

```
npx -y @tauri-apps/cli@2 build
```

or compile only the Rust app:

```
cargo build --manifest-path src-tauri/Cargo.toml
```

`cargo build` on its own produced `krova-app.exe` on the Windows-GNU toolchain here. Treat it as a development build: Tauri normally treats a build made without its CLI as a development build that loads the UI dev-server URL (<http://localhost:1420>) instead of the bundled files, so keep `npm run dev --prefix ui` running when you use it. This has not been confirmed here because the app has never been launched. For a self-contained executable use the CLI build. The CLI build has not been run on this machine either, and the installer step (`--bundles nsis`, as the release workflow uses) has not been run anywhere.

For live development, run the UI dev server in one terminal and the app in another (the app needs the dev server because there is no `beforeDevCommand`):

```
npm run dev --prefix ui
npx -y @tauri-apps/cli@2 dev
```

The icons are checked in under `src-tauri/icons`, so a fresh clone needs no icon step. macOS builds have not been attempted; see [CONTRIBUTING.md](CONTRIBUTING.md) for the setup.

## Verify

The commands CI runs, from the repo root. The core crates:

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The app (a separate workspace, so it needs `--manifest-path`; build the UI first because Tauri embeds `ui/dist`):

```
npm run build --prefix ui
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

The UI:

```
npm run check --prefix ui
npm test --prefix ui
```

As of 2026-10-30 these were run on Windows and passed for the core crates, the app (Windows-GNU) and the UI; the macOS and Linux runs and the CI workflow itself have not happened yet. See [docs/PROGRESS.md](docs/PROGRESS.md) for exact counts.

## Privacy

In plain words:

- Krova makes no network calls. There are no accounts, no telemetry, no crash reports and no update checks.
- Window titles are never read, logged or stored. To move a window Krova looks at the window being dragged, its position and size, and the cursor position, in memory and only at that moment.
- Settings and layouts live in one local file, `config.json`, in your user account's application-data folder. It holds your settings and, per monitor, an identifier, a label, your layouts and the active layout. Delete the file to erase it.
- On macOS Krova asks for the Accessibility permission, and possibly Input Monitoring, which the OS requires before any app can move other apps' windows or watch the mouse. Windows needs no permission.

These are design rules written into the spec and the code's comments, and the app has not been run yet; an independent security review has not been done. The longer version, written for end users, is the website's privacy page: [website/src/pages/privacy.astro](website/src/pages/privacy.astro).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT. See [LICENSE](LICENSE).
