# ADR-001: Stack

Status: accepted · 2026-10-30

## Decision
- **Tauri 2** shell, Rust workspace.
- `crates/krova-core`: pure Rust (no OS deps): zones, layouts, monitor identity & matching, edge-snap rules, config persistence (JSON, `schema_version`, atomic write). Fully unit-tested.
- `crates/krova-platform`: `trait Platform` (monitors, window under cursor, move/resize, drag events, autostart, permissions) with `windows.rs` (`windows` crate) and `macos.rs` (`objc2`, `core-graphics`, accessibility-sys) behind `cfg`.
- `src-tauri`: app glue, tray, commands.
- `ui/`: Svelte 5 + Vite + TypeScript for editor and settings. Types generated from Rust with `ts-rs` into `ui/src/lib/bindings` (single typed contract).
- **Drag highlight overlay is native**, not a webview: a layered Win32 window drawn with GDI/UpdateLayeredWindow on a dedicated thread, repainted only on zone change (the cursor move itself needs no repaint of the highlight, so it is effectively display-rate); NSWindow with CALayer on macOS. A webview adds a compositor hop and process IPC per frame; native avoids that and starts instantly.
- The zone **editor** uses a webview (not latency critical).
- Security: strict CSP (`default-src 'self'`), Tauri capabilities limited to the app's own commands, no shell/fs/http plugins enabled.
- Demo mode: `npm run demo` serves the UI in a browser with a mock backend (fake monitors) for design review and Playwright.

## Toolchain constraint
Local box has no MSVC (installer error 8012) → GNU toolchain (`stable-x86_64-pc-windows-gnu` + WinLibs). If Tauri fails to link locally, core + UI continue locally and CI (windows-latest MSVC, macos-latest) builds the app. Installers come only from CI.

## Alternatives rejected
Electron (RAM), pure native twice (double the UI work), webview overlay (latency).
