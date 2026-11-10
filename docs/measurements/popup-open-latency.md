# Popup open latency (panel and switcher windows)

**Status: method in place, numbers not measured yet.** The measurement needs the real app on a desktop; the engine branch was built and tested headless (no Tauri test harness creates real WebView2 windows).

What is measured: the time from the request (tray click, `Ctrl+Alt+Space`, `Ctrl+Alt+/`) to the popup window being shown, kept per window by `popups::PopupState` and readable with the `popup_open_ms` command (in the devtools console of any Krova window: `await window.__TAURI_INTERNALS__.invoke("popup_open_ms", { label: "switcher" })`).

How to measure:

1. `npm run build --prefix ui`, then run the app (`npx -y @tauri-apps/cli@2 dev` or the release build).
2. Press `Ctrl+Alt+Space` once (cold: the switcher window is created), read `popup_open_ms` for `switcher`.
3. Press Esc, then `Ctrl+Alt+Space` ten more times with Esc in between (warm), reading the value each time.
4. Repeat with a left click on the tray icon for `panel`.
5. Record the cold value and the median of the warm values below, with the machine (CPU, Windows build) and whether WebView2 was already running for the editor.

Expectation: cold is dominated by WebView2 start-up (hundreds of milliseconds); warm is a show and a focus and should be well under 50 ms. A warm window keeps its WebView2 renderer alive while Krova runs: record the memory with `measure-idle.ps1` after the first open.

| Date | Machine | Window | Cold (ms) | Warm median (ms) |
|---|---|---|---|---|
| | | | | |
