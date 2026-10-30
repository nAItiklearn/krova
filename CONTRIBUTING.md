# Contributing to Krova

Thanks for helping. Krova is a free, offline window zone manager (MIT) for Windows 10/11 and macOS 13+. The [README](README.md) says what it is and how to build it; this file covers how to work on it.

## Read this first

[docs/PROGRESS.md](docs/PROGRESS.md) is the honest status ledger: what has been compiled and run, and what has only been written. Keep it honest.

- Say what you ran and what you did not. "Compiled and tests pass on Windows" and "written, not compiled" are different statements; do not blur them in a pull request, a commit message or a doc.
- If your change touches code you cannot build (for example the macOS layer without a Mac), say so in the pull request.
- Update `docs/PROGRESS.md` when your change moves something from "written" to "verified", or the other way round.

Design documents: [SPEC](docs/SPEC.md) (behaviour), [ADR-001](docs/ADR-001-stack.md) (stack), [PORTING-MAP](docs/PORTING-MAP.md) (Windows vs macOS), [DECISIONS](docs/DECISIONS.md).

## Development setup

You need Rust (stable) and Node. CI uses current stable Rust and Node 24. The core crates and the UI need nothing else; the Tauri app also needs a webview toolchain, below.

### Windows

- Install Rust with `rustup`. The MSVC toolchain (Visual Studio Build Tools with the C++ workload) is what Tauri officially supports and what CI uses.
- The Microsoft Edge WebView2 runtime must be present (it ships with Windows 11 and is on most up-to-date Windows 10 systems).
- The Windows-GNU toolchain (`stable-x86_64-pc-windows-gnu` with a current MinGW-w64 such as WinLibs) has been used for development when MSVC could not be installed. Put the WinLibs `mingw64\bin` first on `PATH`: an old `C:\MinGW\bin` earlier on `PATH` breaks Rust's `dlltool` (`Invalid bfd target`). On GNU, linking `src-tauri` prints a `.rsrc merge failure: multiple non-default manifests` warning; it is unresolved, so check dialogs and DPI scaling on the first launch. Details are in [docs/PROGRESS.md](docs/PROGRESS.md).
- Windows is the only OS this project has been compiled on so far.

### macOS (13 or later)

- Install the Xcode command line tools (`xcode-select --install`), Rust (`rustup`) and Node.
- The macOS layer (`crates/krova-platform/src/mac*.rs`) has **never been compiled**. It declares its system-framework FFI by hand and uses no third-party crates. Expect compile errors on first build, and please fix them with the smallest change that makes `cargo clippy --workspace --all-targets -- -D warnings` pass on a Mac. `mac_math.rs` is not `cfg`-gated, so its tests run on every OS.
- To try the app you must grant Accessibility (and possibly Input Monitoring) to the built binary in System Settings, Privacy & Security.

### Linux

Linux is not a supported target. `krova-core`, `krova-platform` (which falls back to `NullPlatform`) and the UI build and test on Linux, which is what the `core` CI job uses it for. The Tauri app is not built on Linux (it would need webkit2gtk) and CI has no Linux app job. Linux is fine for working on the core and the UI.

## The gate: run this before you push

From the repo root. CI runs exactly these.

Core crates:

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The app, which is its own Cargo workspace (the root workspace excludes `src-tauri`), so it needs `--manifest-path`. Tauri embeds `ui/dist` at compile time, so build the UI first:

```
npm install --prefix ui
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

CI ([.github/workflows/ci.yml](.github/workflows/ci.yml)) has three jobs:

| Job | Runs on | Steps |
| --- | --- | --- |
| `core` | Windows, macOS, Linux | `cargo fmt --all -- --check`, clippy with `-D warnings`, `cargo test --workspace` |
| `ui` | Linux | `npm ci`, `npm run check`, `npm test`, `npm run build` |
| `app` | Windows, macOS | build the UI, then `fmt --check`, clippy with `-D warnings` and `cargo test` on `src-tauri` |

At the time of writing CI has never run, because the GitHub repository does not exist yet (remove this sentence once it has). The workflow files were only checked for YAML syntax and their shell snippets run against fake files.

The website is not built by CI. If you change `website/`, run `npm install` and `npm run build` inside it (see [website/README.md](website/README.md)).

## Project conventions

### Crates

- **`krova-core` is pure and OS-independent.** It has `#![forbid(unsafe_code)]`. No OS APIs, no webview, no platform bindings; everything in it must be unit-testable on any OS. Its job is to decide what should happen: zones, layouts, monitor matching, edge snapping, placement math, the drag state machine, layout editing and config persistence.
- **OS code lives in `krova-platform`**, behind `cfg`. Everything the app needs from the OS goes through the `Platform` trait (`crates/krova-platform/src/lib.rs`). `win*.rs` is Windows, `mac*.rs` is macOS, and `NullPlatform` keeps other systems building. Keep `unsafe` small, local, and commented with the invariant it relies on. Put pure arithmetic in a file that is not `cfg`-gated (like `mac_math.rs`) so it is tested everywhere.
- **`src-tauri` is glue:** tray, commands, the drag engine (its `plan` function is pure and tested). Keep logic that does not need Tauri in `krova-core`.

### Types shared with the UI

- The contract types live in one place, `ui/src/lib/bindings/index.ts`. It is a hand-written mirror of the serde shapes in Rust. If you change a serialized type in `krova-core` or `src-tauri`, change it there in the same commit.
- `ui/src/lib/mock/edit.ts` is the TypeScript mirror of `crates/krova-core/src/edit.rs` (split, remove, move divider) used by the browser demo. **It must stay in step with `edit.rs`**: same error strings, same `EPS` and `MIN_ZONE`, same id assignment, and a failed edit leaves the layout unchanged. When you change one, change the other and their tests (`edit.test.ts` and the tests in `edit.rs`).
- The UI talks to Rust only through `ui/src/lib/backend.ts`. Command names and argument names must match `src-tauri/src/commands.rs` (Tauri v2 maps camelCase JavaScript arguments to snake_case Rust parameters). A new command also goes into `generate_handler!` in `src-tauri/src/lib.rs` and into the mock backend.

### Tauri configuration

- `tauri.conf.json` has **no** `beforeDevCommand` / `beforeBuildCommand`, on purpose: nothing depends on a hook's working directory. Build or serve the UI yourself (`npm run build --prefix ui`, or `npm run dev --prefix ui` for `tauri dev`).
- Icons are checked in under `src-tauri/icons` (see the README there for regenerating them from `assets/icon/krova.svg`).
- Keep the capabilities minimal (`src-tauri/capabilities/default.json`: events only) and the CSP strict. Do not enable the fs, shell or http plugins. A change that widens either needs a stated reason in the pull request.

### Privacy rules

These are hard rules, not preferences:

- **Window titles are never read, logged or stored.** Do not call the OS calls that return a window title (for example `GetWindowText` on Windows, or the title attribute of an Accessibility element or `kCGWindowName` on macOS), do not put anything about the windows a user moves into logs, error messages or `config.json`, and do not add a feature that needs the title. Monitor names (`localizedName` of a screen) are not window titles.
- No network access of any kind: no HTTP clients, update checks, crash reporters or analytics, and no remote fonts or scripts in the UI or the website.
- Settings and layouts stay in the local `config.json`.

If a feature seems to need one of these, open an issue to discuss it before writing code.

## Adding a platform feature

Say you want a new OS capability (for example, "is this window fullscreen?").

1. **Pure part first.** Anything that is a decision or arithmetic goes into `krova-core` (or `mac_math.rs` if it works on raw macOS data) with unit tests that run on every OS.
2. **Extend the trait.** Add the method to `Platform` in `crates/krova-platform/src/lib.rs` and give `NullPlatform` the clean `Unsupported` answer.
3. **Implement it per OS**, behind `cfg`: `win.rs` (and helpers) for Windows, `mac*.rs` for macOS. If you can only build one of them, still write both, and say in the pull request which one you compiled and ran. Record real differences in [docs/PORTING-MAP.md](docs/PORTING-MAP.md).
4. **Wire it into the app:** use it from `src-tauri` (`engine.rs` or a new command in `commands.rs`, registered in `lib.rs`). If the UI needs it, update `ui/src/lib/bindings/index.ts`, `ui/src/lib/backend.ts` and the mock backend.
5. **Respect the privacy rules above** and document any new limit (for example "skipped for elevated windows") in the spec and the README's known limits.
6. **Run the gate**, then update `docs/PROGRESS.md` to say honestly what was compiled and what was run.

## Commits and pull requests

- One logical change per commit. The subject line is imperative, at most about 72 characters, with an area prefix, for example `core: reject zones smaller than the minimum`, `platform/win: compensate invisible frame borders`, `ui: add undo to the editor`, `ci: pin the Node version`, `docs: ...`. Areas: `core`, `platform`, `platform/win`, `platform/mac`, `app`, `ui`, `website`, `ci`, `docs`.
- The body explains why the change is needed, not what the diff shows. Mention what you ran (the gate commands, on which OS) and anything you could not run.
- Do not commit build output (`target/`, `node_modules/`, `dist/`) or local files; the `.gitignore` covers them.
- A pull request should pass the gate on your machine and say what it did not test.

## Releases

Only maintainers publish. The workflow is [.github/workflows/release.yml](.github/workflows/release.yml) and it **has never been run**.

- **A manual run** (Actions, "release", Run workflow) builds the installers on Windows and macOS and uploads them as workflow artifacts. It publishes nothing.
- **A GitHub Release is created only when a tag matching `v*` is pushed.** The `publish` job then attaches the artifacts plus `SHA256SUMS.txt` to a release named after the tag. Nothing in the workflow deploys the website.
- Before tagging, bump the version everywhere it appears (`[workspace.package]` in the root `Cargo.toml`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `ui/package.json`, `website/package.json`, and `VERSION_LABEL` in `website/src/site.ts`; commit the updated `Cargo.lock` files). The workflow fails a tag build when the tag (for example `v0.1.0`) does not equal `v` plus the version in `src-tauri/tauri.conf.json`.
- Asset names are **version-free** so that `releases/latest/download/<name>` is stable: `Krova-windows-x64-setup.exe` (NSIS installer) and `Krova-macos-universal.dmg` (universal, Apple silicon and Intel). The download file names in `website/src/site.ts` (`DOWNLOADS`) must match these exactly. Change both together, and flip `RELEASE_PUBLISHED` in `site.ts` only after a real release with those assets exists.
- Do not mark a release as a pre-release: GitHub's `latest` links skip pre-releases, so the website's download links would not resolve.
- The builds are not code-signed or notarized yet. Expect Windows SmartScreen and macOS Gatekeeper warnings, and an unsigned macOS build may not be able to register for login or open at all.

## License

By contributing you agree that your contribution is licensed under the [MIT License](LICENSE).
