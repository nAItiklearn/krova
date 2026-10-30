# Security policy

> **Owner: confirm before publishing.** This file makes three assumptions that only you can check:
> 1. The repository is `https://github.com/nAItiklearn/krova`. That URL comes from `REPO_URL` in `website/src/site.ts`, where it is itself marked as an assumption (the owner name is taken from the Windy remote; the repository name is a guess).
> 2. GitHub **private vulnerability reporting** is switched on for that repository (Settings, Code security, Private vulnerability reporting).
> 3. The response targets below are ones you can keep.
>
> No e-mail address is published here on purpose. Delete this note once the points are confirmed.

## Supported versions

Krova is pre-1.0. Only the **latest 0.x release** receives security fixes; there are no backports to older 0.x versions. Fixes ship in the next 0.x release.

## Reporting a vulnerability

Please report privately. Do not open a public issue or pull request with exploit details.

1. Use GitHub private vulnerability reporting: on the repository page go to **Security**, then **Report a vulnerability**
   (`https://github.com/nAItiklearn/krova/security/advisories/new`).
2. If that page is not available (the feature is off, or the repository is not public yet), open a public issue titled "Security contact request" that contains **no technical details**, and a maintainer will arrange a private channel.

Helpful to include: Krova version or commit, OS and version, what you did, what you expected and what happened, and the impact. If you attach a `config.json`, remove the monitor identifiers first (`hardware_id`, `device_path`).

Krova is maintained on a best-effort basis. Proposed targets (owner to confirm): acknowledgement within 7 days, a status update within 30 days, and coordinated public disclosure after a fix ships or after 90 days, whichever comes first. Good-faith research that avoids harming other people's systems or data is welcome.

## What is in scope

- The application: `crates/krova-core`, `crates/krova-platform`, `src-tauri` (commands, capabilities, CSP, tray, single-instance) and `ui/`.
- Anything that breaks a privacy guarantee below: network traffic started by Krova, window titles read, logged or stored, or data in `config.json` beyond what is documented.
- A crafted `config.json` or IPC payload that causes code execution, a crash, or loss of user data.
- Privilege problems: making Krova move windows of elevated processes, or any escalation through the autostart entry, installer or single-instance mechanism.
- The release process: workflows in `.github/workflows`, the published installers, and tampering or supply-chain issues in how they are built.
- The website, where a page misstates what Krova does with data (the privacy page).

## What is out of scope

- Attacks that already need code execution as the same user. Such a process can edit `config.json`, the autostart entry and the app-data folder without Krova's help.
- Bugs in the operating system, in the WebView2 or WebKit runtimes, or in a third-party dependency with no Krova-specific impact. Please report those upstream; a note to us is welcome so we can bump the version.
- Advisories that only affect development tooling (test runner, site generator) and do not reach a shipped artifact. File those as ordinary issues.
- Windows UIPI limits, such as being unable to snap windows of programs running as administrator. This is documented behaviour.
- Warnings about unsigned installers. Code signing and macOS notarisation are known gaps.
- Denial of service by someone who can already write the config file or end the process, social engineering, and physical access.

## Privacy guarantees and how to check them

Krova promises:

1. **No accounts, no telemetry, no network calls** made by Krova itself.
2. **Window titles are never read, logged or stored.**
3. **Settings and layouts live only in a local `config.json`** (plus the small side effects listed under "What else is on disk").

Known limits of these statements:

- The Windows installer downloads Microsoft's WebView2 runtime if it is not already installed, and the WebView2 (Windows) and WebKit (macOS) runtimes are separate operating-system components with their own update behaviour.
- The operating system may apply its own crash reporting.
- The macOS code had not been compiled or run at the time of the review in `docs/SECURITY-REVIEW.md`, so macOS behaviour is unverified.
- To move windows, Krova watches mouse input system-wide while it runs (a global mouse hook on Windows, a listen-only event tap on macOS), polls the state of the activation key and Escape during a drag, and on Windows checks whether the window's program is elevated. None of that is stored or sent.

### How to check

- **Network.** Block the Krova process in your firewall (a source build is named `krova-app.exe`; an installed build may use another name) and use it normally; behaviour should not change. To watch, use Resource Monitor (Network tab) or `netstat -ano` on Windows, or `lsof -i -a -p <pid>` or Little Snitch on macOS. Attribute traffic by process: `msedgewebview2.exe` belongs to Microsoft's runtime, not to Krova.
- **Source.** From the repository root, these should print nothing:

  ```
  rg -n "reqwest|hyper|TcpStream|UdpSocket|fetch\(|XMLHttpRequest|WebSocket|sendBeacon|updater" crates src-tauri/src ui/src
  rg -n "GetWindowText|WM_GETTEXT|kAXTitle|kCGWindowName|kCGWindowOwnerName" crates src-tauri/src
  ```

  And this should report "nothing to print" (no HTTP client in the Windows build; use `aarch64-apple-darwin` for macOS):

  ```
  cargo tree --locked --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc -e normal -i reqwest
  ```

  Permissions are minimal: `src-tauri/capabilities/default.json` grants only event listening to the main window, and `src-tauri/Cargo.toml` lists no filesystem, shell, HTTP or updater plugin. The content security policy is in `src-tauri/tauri.conf.json`.
- **What `config.json` holds.** It is plain JSON you can read. Location: `%APPDATA%\dev.krova.desktop\config.json` on Windows, `~/Library/Application Support/dev.krova.desktop/config.json` on macOS. Fields are defined in `crates/krova-core/src/config.rs`: settings, and per monitor an id, label, hardware id, device path, last size and position, layouts and the active layout. There is no field for windows, programs or titles.

### What else is on disk

Besides `config.json` (and a transient `config.json.tmp` or a `config.json.bak` after a recovery): the embedded browser's profile (Windows: under `%LOCALAPPDATA%\dev.krova.desktop`), and, if you enable "Launch at login", a registry value `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\Krova` on Windows or a login item / `~/Library/LaunchAgents/dev.krova.desktop.plist` on macOS. On macOS the single-instance guard also uses `/tmp/dev_krova_desktop_si.sock`.

## Security review

An AI-assisted source review dated 2026-10-30 is in [`docs/SECURITY-REVIEW.md`](docs/SECURITY-REVIEW.md): scope, what was and was not verified, findings with severity, and residual risks. It is not an independent audit; no such audit has been done yet.
