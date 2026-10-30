# Compare section: competitor research

Researched and accessed on 2026-10-30. Every cell is tagged:

- **[V]** = verified against the source listed in "Sources" (IDs in brackets).
- **[U]** = unverified. I could not confirm it, so do not state it on the site.

Krova v0.1 column = facts supplied by the Krova maintainers (plus the privacy line from `docs/SPEC.md`). It was not web-verified.

Method notes:

- WebSearch was unavailable (API error), and search-engine fallbacks (DuckDuckGo, Bing) returned nothing usable. All facts therefore come from fetching product, Microsoft, Apple and GitHub pages directly.
- Key claims were re-checked against raw page text or the GitHub and iTunes APIs, not only against fetch summaries.
- Absence claims ("no X documented") mean the docs I read do not describe X. They do not prove X is impossible.

## (a) Comparison table

| Criterion | Windows Snap (11 / 10) | PowerToys FancyZones | Rectangle (free) | Magnet | macOS tiling (15+) | Krova v0.1 (given) |
|---|---|---|---|---|---|---|
| 1. Platforms | [V] Windows 11 and 10 [W1]. Win 10 support ended 2025-10-14 [W1] | [V] Windows 11, or Win 10 2004 (build 19041)+; x64 or ARM64 [P4] | [V] macOS 14+; older builds for macOS 10.13-13 [R1] | [V] macOS 13.0+, Mac only [M1][M3] | [V] macOS Sequoia 15 or later [A3] | Windows 10/11, macOS 13+ (macOS build unverified) |
| 2. Price / licence model | [V] Feature of Windows, not a separate product [W1] | [V] MIT-licensed [P5]. Install docs list GitHub, Store, WinGet and package managers; no price is listed [P4] | [V] "Free and Open Source" [R4]. Rectangle Pro is a **separate paid app** (10-day trial, 3 devices per licence; **price not confirmed**, loaded dynamically) [R5] | [V] $4.99 one-time on the US Mac App Store; "Perpetual license, No subscription, No in-app purchases" [M1] | [V] Feature of macOS [A1] | Free, MIT |
| 3. Open source (licence) | [U] Not stated in sources | [V] Yes, MIT [P5] | [V] Yes, MIT per LICENSE file [R2] (GitHub's auto-detector shows "Other"; see section c) | [U] Not stated by the developer; source availability not checked | [U] Not stated in sources | Yes, MIT |
| 4. User-defined custom zones / layouts | [V] No. Predetermined layouts only; which ones appear depends on display size [W1] | [V] Yes. Grid editor and Canvas editor [P1] | [V] No zone editor. Fixed catalogue of sizes/positions (halves, thirds, quarters...), extras via "Extra" shortcuts and hidden prefs [R1][R3]. **Paid Rectangle Pro** advertises "Create snap targets anywhere on the screen" [R5] | [U] Lists "An infinite number of custom commands" and "Widely configurable layout properties" but does not define them [M1] | [V] No. Fixed set: halves, quarters, centre, fill, multi-window arrangements, full-screen tiles [A3] | Yes (draw zones per monitor) |
| 5. Hold-a-key zone overlay while dragging | [V] None documented. The snap layout box shows automatically while dragging; no modifier needed [W1] | [V] Yes. **Shift by default** ("Hold Shift key to activate zones while dragging"). Alternatives: a non-primary mouse button toggle, or untick both and zones appear as soon as a drag starts. No other modifier key is documented [P1] | [V] No zone overlay. Dragging to an edge shows a "footprint" preview of a preset area [R1]. Hidden terminal pref `snapModifiers` can restrict drag-snap to chosen modifier(s) (cmd/option/ctrl/shift/fn); it is not in the UI [R3] | [U] Not documented. Drag "trigger areas (optional/customizable)" [M1] | [V] Option-drag is documented as the "faster" method, with a settings toggle "Hold ⌥ key while dragging windows to tile". It highlights preset areas, not user zones. Default state of the toggle not stated; no other key offered [A1][A2] | Yes, modifier held while dragging (Shift default) |
| 6. Per-monitor layout assignment | [U] No per-monitor layout setting documented | [V] Yes. Select a monitor in the editor to make it the target of a layout; CLI `--monitor` [P1] | [U] Not documented for free Rectangle (position actions, no stored layouts) | [U] Docs say external monitors supported, up to six [M2]; per-monitor layouts not documented | [U] Not documented in the tiling articles [A1][A3] | Yes (per monitor) |
| 7. Layouts restored when a monitor is reconnected | [U] | [U] Docs do not state it. They describe default layouts applied when display configuration changes and "keep windows in their zones when the screen resolution changes" [P1] | [U] Free: not documented. Pro's vendor page lists "Activate when displays are connected or disconnected" [R5] | [U] | [U] | Yes |
| 8. Edge / corner drag snapping | [V] Yes. Left/right edge, corners (quarters) [W1]. Win 11 also shows layouts when dragging to the top [W1] | [U] Not documented as a FancyZones feature (zones only); Windows' own Snap is separate [P1] | [V] Yes. Edges, corners, thirds; can be disabled [R1] | [V] Yes. Corners = quarters, sides = halves, top = maximize, bottom = thirds [M2] | [V] Yes. Left/right edge, corners, menu bar (fill) [A1] | Optional edge snapping |
| 9. Keyboard-shortcut snapping | [V] Win+Arrow; Win+Z opens the layouts flyout [W1] | [V] Yes. Opt-in "Override Windows Snap" (Win+Arrows between zones), Win+Ctrl+Alt+[number] applies a custom layout, Win+Ctrl+Alt+Arrows spans zones [P1] | [V] Yes, configurable shortcuts [R1] | [V] Yes, "keyboard shortcuts (optional/customizable)" [M1] | [V] Yes, e.g. Fn-Control-Arrows, Fn-Control-F / C [A3] | **No** (planned v0.2) |
| 10. Admin rights / elevated windows | [U] Not documented either way | [V] Cannot snap elevated windows (e.g. Task Manager) unless PowerToys itself runs as administrator: "Restart as administrator" or the "Always run as administrator" setting [P1][P2]. A per-user installer exists [P4]; whether installing needs admin is not stated | [V] Needs the macOS Accessibility permission [R1]. Elevated windows not discussed (macOS) | [U] FAQ troubleshooting uses the Accessibility permission list [M3]; elevated windows not discussed | [U] Built in; not discussed | Skips elevated (admin) windows on Windows |
| 11. Telemetry / network calls | [U] Not Snap-specific; OS-level Windows diagnostics not examined | [V] Diagnostic data is "completely optional... off by default in v0.86 and beyond" [P6]. README says "The application logs basic diagnostic data (telemetry)" [P5]. Settings > General > "Diagnostics & feedback": enable/disable, view data, generate bug report [P3]. Categories: usage, stability, performance [P6]. Update checker runs while the app is running; "Download updates automatically" is on by default for new installs [P3][P4] | [V] Privacy policy (last updated 09Feb2020): "No user data is gathered by Rectangle." [R6]. Update checks via Sparkle: feed `https://rectangleapp.com/downloads/updates.xml`, interval 172800 s (2 days) [R7][R1] | [V] App Store label "Data Not Collected" (developer-declared, "not verified by Apple"); FAQ: "We do not collect any personal data." Optional iCloud sync [M1][M3] | [U] OS-level, not tiling-specific | No accounts, telemetry or network calls, per `docs/SPEC.md` (design spec; not audited here) |
| 12. Latest version / date confirmed | [U] Tied to Windows releases; no separate version | [V] PowerToys v0.102.3001, published 2026-10-27 [P7] | [V] v2.0.4, published 2026-10-20 [R8] | [V] 3.0.7, current-version date 2025-04-12, US App Store [M1] | [V] Tiling documented in Apple's guide for Sequoia 15, Tahoe 26 and macOS 27 "Golden Gate" [A1][A2] | v0.1 |

**Cell tally** (60 competitor cells = 12 criteria x 5 products; the Krova column is not counted):

| Product | Verified | Unverified |
|---|---|---|
| Windows Snap | 6 | 6 |
| FancyZones | 10 | 2 |
| Rectangle (free) | 10 | 2 |
| Magnet | 6 | 6 |
| macOS tiling | 7 | 5 |
| **Total** | **39** | **21** |

## Corrections added after later research (read before using the closing "differentiator" idea)

Added 2026-10-30 from `docs/feature-gap-research.md` (section B.3). The table above covers five widely used tools and is **not exhaustive**.

- **Drawn zones on macOS are not unique.** Free Rectangle, Magnet and macOS tiling indeed do not offer user-drawn zones (as tabulated), but other tools do: MacsyZones (free, GPL-3.0, Shift-drag, per-screen layouts; repository `rohanrhu/MacsyZones`, 920 stars, last push 2026-09-15, checked via the GitHub API), BentoBox (paid), BetterSnapTool (paid, custom snap areas) and MaxTo (Windows and Mac, paid). The website must not present drawn zones on macOS as a Krova-only feature.
- **The only combination still defensible** (and it needs a date and a re-check before every release): free, MIT-licensed, the same drawn-zone workflow on Windows and on macOS. Not a proof that no other such tool exists.
- **FancyZones does more than the table's cells say.** Its Microsoft doc page also lists: restore the original window size on unsnap, space around zones, last known zone for new windows, rotating windows across monitors (Alt+X) and cycling windows within a zone (Win+PgUp/PgDn).
- **Windows 11 remembers window positions across monitor connections natively** ("Remember window locations based on monitor connection", Microsoft multi-monitor page), so Krova's reconnect memory is about zones, not raw positions.

## (b) Sources

All "date accessed" = 2026-10-30. IDs match the table.

**Windows Snap (11 and 10)**

| ID | Claim | URL |
|---|---|---|
| W1 | Applies to Windows 11 and Windows 10. Drag to left/right edge or corner to snap. Layouts are "predetermined" and vary by display size. Win+Arrow keys; Win+Z opens the flyout; snap bar when dragging to the top; Snap Assist; Snap groups. Settings under System > Multitasking. Layouts, flyout, bar and groups appear only in the Windows 11 section. The Windows 10 section covers edge snapping, Win+Arrow and Snap Assist. "Support for Windows 10 has ended on October 14, 2025." | https://support.microsoft.com/en-us/windows/snap-your-windows-885a9b1e-a983-a3b1-16cd-c531795e6241 |

**PowerToys FancyZones**

| ID | Claim | URL |
|---|---|---|
| P1 | Docs page (metadata: ms.date 2026-10-14, updated 2026-10-21). "Hold Shift key to activate zones while dragging"; non-primary mouse button option; zones show immediately if both are cleared. Grid and Canvas custom layout editors. Select a monitor to target a layout. Win+Arrow via "Override Windows Snap". Win+Ctrl+Alt+[number] quick layout. Elevated apps need PowerToys in administrator mode. Settings table, including "Keep windows in their zones when the screen resolution changes". Also: mouse-wheel layout switching (off by default), zones spanning monitors, CLI. | https://learn.microsoft.com/en-us/windows/powertoys/fancyzones |
| P2 | Admin mode: "Restart as administrator" or "Always run as administrator". FancyZones needs it for snapping or moving elevated windows. Microsoft's own note: not recommended to always run as admin. | https://learn.microsoft.com/en-us/windows/powertoys/administrator |
| P3 | General settings. "Diagnostics & feedback" lets you enable or disable collection of diagnostic data, view it and generate a bug report package. "Download updates automatically... New installations have this enabled by default." The page does **not** state the default of the diagnostic-data toggle. | https://learn.microsoft.com/en-us/windows/powertoys/general |
| P4 | System requirements: "Windows 11 or Windows 10 version 2004 (20H1 / build 19041) or newer", 64-bit x64 or ARM64. Install via GitHub exe, Microsoft Store, WinGet, Chocolatey, Scoop. Per-user and machine-wide installers. "PowerToys uses an automatic update checker". | https://learn.microsoft.com/en-us/windows/powertoys/install |
| P5 | README: MIT licence; "The application logs basic diagnostic data (telemetry)"; points to the data and privacy doc. GitHub API reports licence `mit`. | https://github.com/microsoft/PowerToys and https://api.github.com/repos/microsoft/PowerToys |
| P6 | "PowerToys diagnostic data is completely optional for users and is off by default in v0.86 and beyond." Categories: usage, stability, performance. Lists events including UpdateCheck_Completed and UpdateDownload_Completed. States that personal-information handling is covered by the Microsoft privacy statement. Does not itself say how to switch it off. | https://raw.githubusercontent.com/microsoft/PowerToys/main/DATA_AND_PRIVACY.md (the Learn URL /windows/powertoys/data-and-privacy returned 404) |
| P7 | Latest non-prerelease: tag v0.102.3001, published 2026-10-27T10:10:02Z. | https://api.github.com/repos/microsoft/PowerToys/releases/latest (human page: https://github.com/microsoft/PowerToys/releases/tag/v0.102.3001) |
| P8 | Dev doc: "FancyZones can't move admin windows unless running as admin" (corroborates P1/P2). AppliedLayouts "Manages applied layouts for different monitors and virtual desktops" (code-structure note only; does not describe reconnect behaviour). | https://raw.githubusercontent.com/microsoft/PowerToys/main/doc/devdocs/modules/fancyzones.md |

**Rectangle (and Rectangle Pro)**

| ID | Claim | URL |
|---|---|---|
| R1 | README: "Rectangle supports macOS v14+"; v1.100 for macOS 10.15-13, v0.73 for 10.13/10.14. Snap-area table (edges, corners, thirds); keyboard shortcuts; "Rectangle's UI is intentionally simple" with additional sizes in the "Extra" section; uses Sparkle for updates; Accessibility permission troubleshooting; "Rectangle Pro app is entirely built on top of Rectangle". Also has Tile in Rows / Columns actions and a Cycle stacked windows action. | https://raw.githubusercontent.com/rxhanson/Rectangle/main/README.md (repo: https://github.com/rxhanson/Rectangle) |
| R2 | LICENSE file text is the MIT License ("Copyright (c) 2019-2026 Ryan Hanson; Based on the Spectacle app, Copyright (c) 2017 Eric Czarny"). | https://raw.githubusercontent.com/rxhanson/Rectangle/main/LICENSE |
| R3 | Hidden preference `snapModifiers` ("Only allow drag-to-snap when modifier keys are pressed"). Extra command `specified` for a custom size. Hidden-pref instructions for the optional ninths, eighths and sixths sizes. | https://raw.githubusercontent.com/rxhanson/Rectangle/main/TerminalCommands.md |
| R4 | Site: "Free and Open Source"; "Supports macOS 14+"; promotes Rectangle Pro. | https://rectangleapp.com |
| R5 | Rectangle Pro page: "Free for 10 days"; "One license can be active on 3 devices at a time"; "macOS 13.5+"; "Create snap targets anywhere on the screen"; "Make your own size & position"; "Activate when displays are connected or disconnected"; "Load your shortcuts from Rectangle". Price is loaded dynamically by Paddle and is **not** visible in the page source. | https://rectangleapp.com/pro |
| R6 | "Privacy Policy and Terms of Service. Last updated: 09Feb2020. ... No user data is gathered by Rectangle." The page covers the free app only. | https://rectangleapp.com/terms |
| R7 | `Info.plist`: SUFeedURL `https://rectangleapp.com/downloads/updates.xml`, SUScheduledCheckInterval 172800. | https://raw.githubusercontent.com/rxhanson/Rectangle/main/Rectangle/Info.plist |
| R8 | Latest release v2.0.4, published 2026-10-20T18:10:59Z. | https://api.github.com/repos/rxhanson/Rectangle/releases/latest |

**Magnet**

| ID | Claim | URL |
|---|---|---|
| M1 | US App Store listing: price $4.99; seller BOOTCODE A.S.; version 3.0.7, current-version date 2025-04-12; minimum macOS 13.0; feature list ("trigger areas", "keyboard shortcuts (optional/customizable)", "An infinite number of custom commands", "Synchronized via iCloud (optional)", "Perpetual license", "No subscription", "No in-app purchases", "No personal data collection"). Privacy label "Data Not Collected", with Apple's note that the info "has not been verified by Apple". | https://apps.apple.com/us/app/magnet/id441258766?mt=12 and https://itunes.apple.com/lookup?id=441258766&country=us |
| M2 | Site: drag to corners = quarters, side edges = halves, top = maximize, bottom = thirds; "Up to six external displays supported"; no price, no open-source statement. The site loads Google Analytics (website only). | https://magnet.crowdcafe.com/ |
| M3 | FAQ: "We do not collect any personal data."; "Magnet is compatible with macOS 13 Ventura or later."; Accessibility-permission troubleshooting; cannot arrange windows across different Spaces. | https://magnet.crowdcafe.com/faq.html |

**macOS built-in tiling**

| ID | Claim | URL |
|---|---|---|
| A1 | "In macOS Sequoia 15 or later" tiling methods: drag to the edge (left, right, corners, menu bar = fill); Option-drag "for faster window tiling"; green-button menu; Window menu; keyboard shortcuts. Each of the drag methods "can be turned on or off". Dragging through the top edge opens Mission Control unless that setting is off. The macOS 27 version of this article lists the same methods. | https://support.apple.com/guide/mac-help/tile-app-windows-mchlef287e5d/15.0/mac/15.0 (macOS 27 version: https://support.apple.com/guide/mac-help/tile-app-windows-mchlef287e5d/mac) |
| A2 | Settings (Desktop & Dock > Windows). Sequoia 15: "Drag windows to screen edges to tile", "Drag windows to menu bar to fill screen", "Hold ⌥ key while dragging windows to tile", "Tiled windows have margins". macOS 26 and 27 word the first one as "Drag windows to left or right edge of screen to tile". No defaults are stated. | https://support.apple.com/guide/mac-help/change-window-tiling-settings-on-mac-mchl118087b0/15.0/mac/15.0 (also .../26/mac/26 and .../27/mac/27) |
| A3 | Layout and shortcut table: Fill, Center, Left/Right/Top/Bottom halves, four corners, Return to Previous Size, paired and three-window arrangements, full-screen tiles. Shortcuts such as Fn-Control-Left Arrow and Fn-Control-F. "Window tiling is available in macOS Sequoia 15 or later." No thirds and no custom layouts appear. | https://support.apple.com/guide/mac-help/mac-window-tiling-icons-keyboard-shortcuts-mchl9674d0b0/15.0/mac/15.0 |

**Krova**

| ID | Claim | URL |
|---|---|---|
| K1 | Privacy line "No accounts, telemetry, or network calls..." is from the project's own spec (design intent, not audited here). | `D:\krova\docs\SPEC.md`, item 10 |

## (c) Do not claim

These statements are NOT verifiable, are disputed, or are false. Keep them off the website.

1. **"Krova is the only free / open-source zone manager."** False. FancyZones (MIT, Windows) and Rectangle (MIT, macOS) are both free and open source. Magnet, Rectangle and macOS tiling are not zone managers in the user-drawn sense; FancyZones is.
2. **Anything implying Shift-hold zone overlays or per-monitor custom zones are unique to Krova on Windows.** FancyZones documents Shift-to-show-zones (default), custom Grid/Canvas layouts and per-monitor layout targets.
3. **"FancyZones can't snap admin / elevated windows."** Wrong. It can if PowerToys is run as administrator. Safe wording: "needs PowerToys to run elevated."
4. **"FancyZones doesn't restore layouts when a monitor is reconnected"** (or the reverse). Unverified; the docs are silent. Only claim Krova's own behaviour.
5. **"PowerToys collects telemetry by default."** The data-and-privacy doc says diagnostic data is off by default from v0.86. The README still says the app "logs basic diagnostic data (telemetry)", and the two wordings sit awkwardly together. Do not claim "PowerToys sends no data" either: it has an update checker and an opt-in diagnostic setting. Quote the docs or stay silent.
6. **"FancyZones requires Shift."** Shift is the default only; a mouse-button option exists, or zones can show immediately.
7. **"FancyZones Win+Arrow snapping is on by default."** The docs call it opt-in ("Override Windows Snap"); the default state is not stated.
8. **"FancyZones has no edge snapping."** Not documented either way. Avoid.
9. **Rectangle:**
   - Do not say "Rectangle has no custom layouts" without limiting it to the **free** edition. Rectangle Pro (paid, separate app) advertises custom snap targets and monitor-connect actions.
   - Do not quote a Rectangle Pro price (not retrievable).
   - Do not call Rectangle Pro closed source or open source (unverified).
   - Do not say Rectangle has no modifier-key drag behaviour: the hidden `snapModifiers` preference exists.
   - GitHub's licence badge shows "Other" because the MIT text carries an extra "Based on Spectacle" line. The LICENSE file itself is MIT.
   - The privacy policy is dated 2020 and does not mention update checks, though Sparkle checks run every 2 days. Do not claim "Rectangle makes no network calls."
10. **Magnet:**
    - Do not claim it is closed source or open source, has/lacks custom layouts, has/lacks per-monitor layouts, or has/lacks a drag-modifier key. All unverified.
    - $4.99 is the US App Store price at access time; it varies by region and over time.
    - "Collects no data" is the developer's own declaration ("not verified by Apple"). Write "the developer states".
    - Last listed update is 2025-04-12 (over 18 months before access). Do not call it abandoned or outdated.
11. **macOS built-in tiling:**
    - Do not state whether Option is required or optional by default, or the default state of any tiling toggle. Apple documents both drag-to-edge and Option-drag, each with an on/off setting, and states no defaults.
    - Say "documented layouts are halves, quarters, centre, fill and multi-window arrangements". Do not say "macOS can't do thirds" or "can't do custom layouts"; absence from the docs is not proof.
    - The edge setting is worded differently in Sequoia 15 and in 26/27.
12. **Windows Snap:**
    - Do not call Windows Snap layouts "custom". They are predetermined and vary by display size.
    - Layouts, flyout, snap bar and Snap groups are documented only in the Windows 11 section. Do not credit them to Windows 10.
    - Do not repeat the Microsoft article's sentence that dragging to the top "maximizes vertically". It conflicts with the same article's snap-bar description.
    - Do not claim Windows lacks monitor-reconnect window restoration. I recall a Windows 11 multi-display setting about remembering window locations but could not verify it.
    - Do not claim Snap cannot move elevated windows. Not documented.
13. **Performance, RAM, CPU, size or startup comparisons.** Not researched for competitors. Do not claim Krova is lighter or faster than any of them.
14. **"Bloated" or "heavy" for PowerToys.** Not verified. The factual statement is that PowerToys is a multi-utility suite and FancyZones is one module that can be enabled or disabled (Settings CLI, P3).
15. **Popularity, ratings, "trusted by N users", or "more reliable than X".** Not researched, and Krova has no track record yet.
16. **"No admin rights needed to install PowerToys".** Per-user installers exist, but the docs do not say admin is unnecessary.

## What the site must not hide (competitor features Krova v0.1 lacks)

- **FancyZones is feature-complete next to Krova on Windows.**
  - Its docs list keyboard snapping, layout hotkeys, multi-zone spanning, mouse-wheel layout switching, a CLI and an exclusion list.
  - It can snap elevated windows when run as admin.
  - It comes from Microsoft, is MIT-licensed and has a long track record.
- **Keyboard snapping exists in all five alternatives.** Windows Snap, FancyZones, Rectangle, Magnet and macOS tiling all have it. Krova doesn't until v0.2.
- **Rectangle goes further than Krova in some areas.**
  - Stacked-window cycling, Tile in Rows/Columns, and URL-scheme actions (README [R1]).
  - Krova has no window stacks or auto-tiling.
- **macOS built-in tiling and Windows Snap are zero-install.** They are the baseline.
- **Where Krova looks distinct from these docs:**
  - One tool covers Windows and macOS with the same drawn-zones and hold-a-key workflow.
  - User-drawn zones on macOS are not offered by free Rectangle, Magnet or macOS tiling per their docs. Rectangle Pro advertises snap targets anywhere.
  - Present this as a difference, not as "the only one".
