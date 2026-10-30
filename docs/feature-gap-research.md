# Feature-gap research: what users want that window managers lack

Researched and accessed on **2026-10-30**. Purpose: help the maintainers decide which features would make Krova genuinely distinctive, without making a claim that is not true.

How to read this file:

- Counts are what I saw on the date above. Reaction counts are GitHub `reactions.total_count` (all reaction types) unless I say "+1" (thumbs-up only).
- "No source found" means I looked at the pages listed and found nothing. It does not mean the tool lacks the feature. "Not documented" means the vendor pages I read are silent.
- "Free / paid" is the price model as stated on the cited page on the access date.
- Effort is my estimate for a small team: **S** = days, **M** = 1-3 weeks, **L** = a month or more. It is based on Win32/macOS API knowledge and on reading `crates/krova-platform/src/lib.rs` and `docs/PORTING-MAP.md`. It is not measured.
- Sources worked unevenly. Part D lists what I could not reach and where my facts come from a page-summarising fetch tool rather than raw text.

## Headlines

1. The most-requested things are not "more snap presets". They are about **windows staying in their zones**: returning after a dock/undock or monitor reconnect, resizing together, moving by keyboard, opening where they belong.
2. The single biggest FancyZones request, **maximize/fullscreen inside a zone** (410 reactions, open since 2019), is also the hardest and cannot be fully solved from outside the app (fullscreen video is app-owned).
3. **User-drawn zones on macOS already exist** (MacsyZones: free, GPL-3.0; BentoBox: $9; BetterSnapTool: $1.99; MaxTo for Mac launched 2026-10-18). `docs/compare-research.md` does not list these. Any "only" claim built on "drawn zones on a Mac" is not available. See B.3.
4. **Windows 11 now restores window positions when a monitor is re-docked** (verified in Microsoft's page). The Windows-side "restore after dock" demand is therefore about *zones*, not raw positions. On macOS the gap is real and is filled only by paid tools (Moom, Rectangle Pro) as far as I found.
5. Recommendation (Part C): build one shared "snap registry" (which window is in which zone, in memory only), then ship **keyboard zone control, reconnect memory, linked dividers, app-to-zone rules, and portable layout files + CLI**.

---

## A. Ranked candidate features

### A.0 How the ranking was made

Ordered by strength of *unmet* demand: breadth first (how many independent sources), then the size of the biggest single signal, then how poorly the free tools serve it. It is **not** ordered by feasibility. Part C picks what to build. Features that are already widely served are ranked low even if people mention them (they are parity, not personality).

### A.1 Summary table

Offered today: F = free tool, P = paid tool, none = "no source found". Detail and citations are in A.2.

| # | Feature | Strongest evidence (seen 2026-10-30) | Offered today | Effort | Main technical risk (Win / mac) |
|---|---|---|---|---|---|
| 1 | Windows return to their zones when a monitor (re)connects, on dock/undock, after wake | PowerToys #4: 313 reactions; Moom Classic: 10 of 250 reviews (5 praise, 5 report it failing); Magnet 3 reviews; Rectangle 2 threads | Win 11 native (positions, F); Moom P; Rectangle Pro P; DisplayFusion Pro P; free Mac: none | M | Win: debounce vs Windows' own relocation / mac: OS reshuffles windows asynchronously, no stable window identity beyond a session |
| 2 | Keyboard zone control: hotkey to zone N, next/prev zone, focus adjacent zone, send to other monitor keeping the zone | PowerToys #492: 262; #2303: 95; #6973: 37; Magnet 8 reviews on cross-display shortcuts | FancyZones F (Win+Arrow between zones; no hotkey-per-zone); BentoBox P; MaxTo P; MacsyZones F | S-M | Win: Win-key combos are reserved, needs a keyboard hook or other chords / mac: global hotkeys fine, conflicts with system shortcuts |
| 3 | Maximize / fullscreen *inside* a zone ("zone as virtual monitor") | PowerToys #279: **410** (333 +1), 189 comments, open since 2019-09-06 | none found | L, partial | Win: needs per-window state hooks, fullscreen video is app-owned / mac: green button means Space fullscreen |
| 4 | Layouts that adapt to window count / auto-fill empty zones (auto-tiling, still user-drawn) | PowerToys #2694: 384; #4507: 15; #5226: 10 | Dynamic tilers F (no drawn zones); hybrid: none found | L | Both: window-created hooks, "which windows are tileable", apps that fight resizing |
| 5 | Linked dividers: resize one snapped window, neighbours follow | PowerToys #254: 103 (+#4795: 21, #11792: 38); Loop #264: 11; HN comment | Win Snap F (own layouts); AquaSnap F-personal; Swish P; MacsyZones F (indicated, unconfirmed); FancyZones none (open request) | M | Both: live resize events, min-size conflicts, jitter |
| 6 | Virtual-desktop / Space aware zones and arrangements | PowerToys Workspaces #34578: 118; #10222: 38; #759: 20; Rectangle discussion 19 upvotes + closed #215 (23) | FancyZones F (per desktop, devdocs); BentoBox P; MacsyZones F | Win M / mac L | Win: desktop-change detection is undocumented COM / mac: no public Space API |
| 7 | Per-app rules ("app X opens in zone N") | PowerToys #11833: 29 (+closed #4102); Workspaces/HN mentions | FancyZones F (last known zone only); MaxTo 3.1 P; DisplayFusion Pro P | M | Win: catch window creation without a visible jump / mac: AXObserver per running app |
| 8 | Raise a whole snapped group together (Snap-group-like) | PowerToys #16018: 75 | Win 11 Snap groups F (own layouts); FancyZones none (open) | M | Win: foreground-lock rules / mac: cross-app raise changes focus |
| 9 | Drag-from-anywhere (Alt-drag) that also snaps to zones | PowerToys #47268: 57 (opened 2026-04-28); #47431: 12 | PowerToys Grab And Move F (no zones); AltSnap F; BetterSnapTool P | M | Both: Krova must run its own move loop |
| 10 | Pin a window on top | AeroSpace #2: 107 and #4: 58; Magnet 2 reviews; Loop #1121: 2 | PowerToys Always On Top F; DisplayFusion Free F; mac: none found | Win S / mac L | mac: no public API to change another app's window level (my API knowledge, not verified here) |
| 11 | Snap to other windows' edges (magnet) | PowerToys #8: 80 | AquaSnap F-personal; DisplayFusion (tier not shown) | M-L | Both: correct during an OS-driven drag |
| 12 | Scripting: CLI / URL scheme / Shortcuts | PowerToys #12: 50; one BetterSnapTool review | FancyZones CLI F; Rectangle URL scheme F; SizeUp AppleScript P | S-M | mac: URL scheme and CLI to a running instance; Win: single-instance IPC |
| 13 | Type exact zone values / ruler in the editor | PowerToys #463: 77; #1564: 13; MacsyZones #20: 2 | FancyZones: no numeric entry documented | S | None notable |
| 14 | Portable layout files / settings sync | Loop #945: 5; one review each at BetterSnapTool, Moom, Magnet | FancyZones JSON by hand F; Rectangle Pro iCloud P; Magnet iCloud P; cross-OS: none found | S | Schema versioning |
| 15 | Ultrawide / portrait / big-monitor layouts | Magnet 8 reviews; Divvy 2; AeroSpace #60: 65; HN | Drawn zones do this by design (FancyZones F, MacsyZones F, BentoBox P) | S (templates) | None |
| 16 | Span several zones with one window while dragging | PowerToys #6973: 37; MacsyZones #6 and #22 | FancyZones F; BentoBox P | S-M | Overlay hit-testing |
| 17 | Gaps / padding between zones | PowerToys #2151: 36 (1-px gap) | FancyZones F; macOS "margins" toggle F; Divvy P | S | DWM invisible-border compensation |
| 18 | Restore pre-snap size when dragged out (as a toggle) | BetterSnapTool review 2026-08-02; Magnet review 2026-10-13 | FancyZones F; Rectangle F; SizeUp P; BetterSnapTool P | S | None |
| 19 | Swap two windows by dropping one on the other | PowerToys #290: 15 | none found | M | Needs zone occupancy tracking |
| 20 | Zone stacks / tabs | PowerToys #15921: 16 | FancyZones cycles windows in a zone F; Rectangle cycles stack F; AquaSnap TidyTabs | L | Needs tab-strip overlay windows |

### A.2 Evidence by feature

Citations: PowerToys issues are `https://github.com/microsoft/PowerToys/issues/<n>`; Rectangle discussions are listed at `https://github.com/rxhanson/Rectangle/discussions?discussions_q=is%3Aopen+sort%3Atop`. App Store reviews come from `https://itunes.apple.com/us/rss/customerreviews/page=<1-5>/id=<id>/sortby=mostrecent/json` with ids Magnet 441258766, BetterSnapTool 417375580, Moom Classic 419330170, Divvy 413857545. Review quotes give date and star rating.

#### 1. Windows return to their zones after dock/undock, reconnect or wake

- **Plain words:** unplug a monitor and plug it back: the windows that were in zones on it go back into those zones, without you re-snapping them.
- **PowerToys #4** ("Full window manager including specific layouts for docking and undocking laptops", open, 313 reactions: 293 +1; 27 comments; created 2019-05-07). Comment by `myronu` (38 reactions): "I cannot stand how when I undock my laptop all of my windows jump to the laptop monitor. When I dock again they don't return to their assigned monitor." Comment by `dtalaski` (9): "When docking the computer, windows snap to their previously defined zone." These comments date from 2019-2020.
- **PowerToys Workspaces #34688** (3 reactions): "Allow users to add multiple monitor setups for a workspace, and have Workspaces auto-detect which setup is in use during launch."
- **Moom Classic reviews** (Mac App Store, 250 reviews fetched): 10 describe docked/undocked, monitor-change or wake restoration; I counted 5 praising it and 5 reporting it failing. Examples: 1-star 2026-03-06 "I purchased this app specifically to remember app/window location when switching from working on just my laptop to on a laptop + external monitor", reporting it does not; 5-star 2023-07-19 "I was tired of having to rearrange my three monitor workspace when my Macbook Pro M1 would wake from sleep"; 3-star 2023-03-29 "Apple makes it essentially impossible to fully recognize window identities".
- **Magnet reviews** (3 of 250): 1-star 2026-07-11 "every time I wake up my Mac after having set my windows the way I want them, the windows are a jumbled mess in the middle of the screen"; 4-star 2025-03-25 wants to "save layouts" for an office big screen and a home setup; 5-star 2025-02-16 wants saved workspaces. **BetterSnapTool**: 2 five-star reviews praise saved window positions (2024-11-15, 2024-01-06).
- **Rectangle:** discussion "Restore windows position on external monitor" (5 upvotes, 3 comments, 2022-10-13); discussion "Save and restore window layouts" (10 upvotes, 11 comments, 2021-12-16); closed issue #127 "Remember window position per display" (9 +1).
- **Hacker News:** Ask HN "macOS app to keep windows in defined spaces when (un)plugging screens?" (https://news.ycombinator.com/item?id=30425934, 2022-02-22, 2 points, 3 comments); comment 2026-02-13 by `jeroenhd` on macOS: "Connecting monitors will do anything from keeping all windows in the same position to restoring previous positions to launching them across screens, sometimes completely outside visible screen space, seemingly arbitrarily." (https://news.ycombinator.com/item?id=47000009)
- **Offered today:**
  - Windows 11 native: "When you re-dock your computer to your monitor, Windows puts everything back exactly where you had it before", with a setting "Remember window locations based on monitor connection" (https://support.microsoft.com/en-us/windows/how-to-use-multiple-monitors-in-windows-329c6962-5a4d-b481-7baa-bec9671f728a, raw text checked). This restores positions, not zones. I found this wording only in the page's Windows 11 material, not for Windows 10.
  - FancyZones: "Keep windows in their zones when the screen resolution changes" (resolution only). Reconnect behaviour: not documented, not tested by me.
  - Paid: Moom, Rectangle Pro ("Activate when displays are connected or disconnected"), DisplayFusion Pro ("Save or Restore all window locations" is Pro-only on its compare page).
  - Free on macOS: no source found.
- **Helps:** laptop users with a dock; anyone whose Mac scrambles windows after sleep.
- **Effort M.** Keep an in-memory map `WindowId -> (monitor id, zone id)` for windows Krova snapped; on display-change events, wait for the OS to settle, then re-snap. Windows: `WM_DISPLAYCHANGE`. macOS: screen-parameters notification, plus a debounce because macOS moves windows itself. Persisting across reboots would need app+title matching, which conflicts with "window titles are never logged or stored" (`docs/SPEC.md` item 10), so keep it in memory only. A comparable open-source tool shows the race is real: MacsyZones #114 (2026-09-29) "Layouts per monitor detected incorrectly if it takes longer for another display to come online".

#### 2. Keyboard zone control

- **Plain words:** a shortcut sends the focused window to zone 3 (or the next/previous zone, or the neighbouring zone to the left); another sends it to the other monitor in the same relative zone; another moves focus between zones.
- **PowerToys #492** (open, 262 reactions: 183 +1, 62 heart; 65 comments; updated 2026-06-05): "I want to hit a user definable shortcut and the focused window should resize to the zone which is mapped to this shortcut." **#2303** "Win+Ctrl+Arrow to switch focus to adjacent zones" (95 reactions). **#6973** "Allow to change keyboard shortcut for stretching a window across zones" (37).
- **Magnet reviews:** at least 8 reviews are about shortcuts that move windows across displays. Seven fall between 2024-10-09 and 2024-11-05, around the 3.0.2 update that broke and then restored it. Quotes: 1-star 2024-10-25 "Literally the only thing I needed Magnet for was moving windows between several external monitors"; 4-star 2026-09-15 "doesn't have the ability to toggle a window between monitors". Also 4-star 2026-01-21: "I would rather have one shortcut that moves my window to the left and toggle through 1/2, 1/3, etc options by pressing it repeatedly".
- **Rectangle:** discussion "Move window to an specific Display" (4 upvotes, 18 comments); closed issue #199 (12 reactions). **Loop** #260 "Generic keyboard shortcut to trigger Loop" (8). **MacsyZones** #17 "Snap based on arrow keys" (3 +1), #93 (pressing a zone's number during a Shift-drag).
- **Offered today:** FancyZones has Win+Arrow between zones (opt-in "Override Windows Snap"), Win+Ctrl+Alt+[number] to apply a *layout*, Alt+X rotation across monitors, and Win+PgUp/PgDn to cycle windows in a zone (learn.microsoft.com/windows/powertoys/fancyzones). It documents no hotkey-per-zone, and #492 is still open. MaxTo (paid): "Snap any window into one with a keyboard shortcut." BentoBox ($9): "You can also assign a shortcut to any zone". MacsyZones (free): Quick Snapper picks a zone by number key. Rectangle, Magnet, Divvy etc. use presets, not user zones.
- **Helps:** keyboard-first users, accessibility, ultrawide users.
- **Effort S-M.** "Which zone is this window in" is pure geometry (max overlap), no tracking needed. Windows: `RegisterHotKey` for Ctrl/Alt chords; overriding Win+Arrow needs a keyboard hook (FancyZones does this). macOS: Carbon hotkeys need no extra permission. The cost is the settings UI and chord conflicts, not the OS plumbing.

#### 3. Maximize / fullscreen inside a zone

- **Plain words:** clicking maximize (or pressing F11 in a video) fills the zone instead of the whole monitor.
- **PowerToys #279** (open, **410 reactions: 333 +1, 55 heart**; 189 comments; created 2019-09-06; updated 2026-05-12). Top comment (30 reactions): "display a Netflix or YouTube video full screen in one of the zones". A maintainer wrote: "the idea is to be able to maximize/go fullscreen as if the zone is a virtual monitor. Not sure this is doable." A contributor (11 reactions) noted it "does involve hooking into the window WndProc. It's somewhat hacky". Workarounds named in the thread: a virtual-display driver, NVIDIA Surround, and switching to MaxTo (a 2020 user claim, 12 reactions; MaxTo's page today does not mention maximize-to-region).
- **Offered today:** no source found for any tool that does it.
- **Helps:** ultrawide and big-monitor users who use zones as virtual screens.
- **Effort L, and only partly solvable.** The video case is the app's own fullscreen and cannot be redirected. What may be possible is re-fitting a window to its zone when it becomes maximized (Windows: location-change event plus `GetWindowPlacement`; visible flash likely). On macOS the green button enters native fullscreen in its own Space. Do not promise this.

#### 4. Layouts that adapt to window count (auto-tiling, still user-drawn)

- **Plain words:** open a window and it fills the next empty zone; or pick a layout per window count (1 window = full, 2 = halves, 3 = main + two stacked).
- **PowerToys #2694** (open, 384 reactions: 246 +1, 86 heart; 48 comments; 2020-05-05): "add the ability to tile windows automatically". **#4507** "Snap all open Windows to zones" (15). **#5226** "New Windows Fill Empty Zones" (10). **#41135** "Fanzy Zones with Dynamic Tiling" (3). **Loop** #243 "Automatic tiling feature or Quick tile all" (3).
- **Hacker News**, FancyZones thread (https://news.ycombinator.com/item?id=20895031, 507 points, 163 comments, 2019): `nickjj`: "This one requires pre-setting a bunch of layouts up front, unlike i3 where you can just seamlessly open and move windows that auto-split as you go". The same thread also has users who prefer fixed layouts, e.g. `jplayer01`.
- **Offered today:** dynamic tilers (AeroSpace, yabai, Amethyst, Rift on macOS; FancyWM, GlazeWM, komorebi named in issue comments on Windows). None of those use user-drawn zones as far as I saw. A 2025-02-11 comment on #2694 says FancyWM "looks like its been abandoned, or is at least on hold" (a user's claim, not checked).
- **Helps:** i3/Hyprland refugees. This is a different philosophy from "snap where I drop it".
- **Effort L.** Needs window-created/destroyed events, rules for which windows are tileable, and handling of apps that resize themselves back (an HN commenter, `rickbutton`, describes workspacer and WinSCP moving the window back and forth in an endless loop). A small version ("fill the next empty zone", "snap all windows to zones") is M.

#### 5. Linked dividers

- **Plain words:** after snapping two windows side by side, dragging the edge between them resizes both.
- **PowerToys #254** (open, 103 reactions: 76 +1; 39 comments; updated 2026-10-29): "whenever I resize one of them, the other ones resizes accordingly". Related: **#4795** (21) "Combined window resizing like in Snap?"; **#11792** (38) "FancyZones editor to allow custom zones for Win11 snapping". **Loop #264** "Resizing adjacent windows" (11, open since 2024-03-23).
- **Hacker News**, "Resizing windows on macOS Tahoe - the saga continues" (https://news.ycombinator.com/item?id=46997008, 876 points, 522 comments, 2026-02-12). Comment by `neodymiumphish` (https://news.ycombinator.com/item?id=46997385): "I want two things: - Predefined zones à la FancyZones - Tied edges ... so that I can grab the edge between two apps and have them both resize together".
- **Offered today:** Windows Snap for its own layouts ("Resize a window on one side, and the adjoining windows will resize to fit", Microsoft Snap page, raw text checked); AquaSnap (free for personal use per its page: "resizing one of them while pressing [Ctrl] will also resize the other one"); Swish ($16; "Drag the divider to resize multiple windows at once"); MacsyZones (free) is indicated but unconfirmed: its changelog mentions "Snap Resizing" and "Snap Resize buttons", its tracker has "Stray zone grabber handles", and an HN commenter says it "appears to resize multiple adjoining windows" without having used it (item 46998195). Absent: FancyZones and Loop (open requests). Rectangle, Magnet, BentoBox: no source found.
- **Helps:** anyone with two or three side-by-side windows; ultrawide users.
- **Effort M.** Needs a window-event layer (Windows `SetWinEventHook` move/size events; macOS `AXObserver` resize notifications) and the snap registry. Risks: apps with minimum sizes, slow repaint causing jitter, mixed DPI.

#### 6. Virtual-desktop / Space aware zones and arrangements

- **PowerToys Workspaces #34578** (open, **118 reactions: 94 +1**; 23 comments; 2024-09-04): "Only the active virtual desktop is included in workspaces." **#10222** "Different desktop content per virtual desktop" (38). **FancyZones #759** "Windows should remember their last virtual desktop location after reboot" (20).
- **Rectangle:** the top open discussion is "Support for Spaces (aka Desktops)" (19 upvotes, 7 comments, 2020-09-03); closed #215 (23 +1) and #85 "move window to next/previous desktop" (22 +1).
- **Magnet reviews:** 1-star 2024-10-19 "It only works with the First Virtual Desktop"; 1-star 2026-10-13 "It doesn't work with Spaces in the latest MacOS". Magnet's own FAQ says it cannot arrange windows across Spaces (compare-research [M3]).
- **Offered today:** FancyZones stores applied layouts per monitor *and* virtual desktop (PowerToys devdocs: "Manages applied layouts for different monitors and virtual desktops"). BentoBox ($9): "Each display and Space can keep its own layouts". MacsyZones (free): "remembers your preferred layout for each screen and workspace combination". SizeUp ($12.99): "Move a window from one Space to another".
- **Effort: Windows M, macOS L.** `docs/PORTING-MAP.md` already limits Krova to the current desktop. Per-desktop layouts need desktop-change detection (Windows: public `IVirtualDesktopManager` only answers "is this window on the current desktop"; change notifications need undocumented interfaces that shift between builds). macOS has no public Space identifier; known tools use private calls (my API knowledge, not verified here).

#### 7. Per-app rules

- **PowerToys #11833** "FancyZones: Force windows to open in zones" (29 reactions, 12 comments, 2021-06-22): "Fancy Zones should at least have an option to behave more like a true tiling window manager." It cites an earlier request #4102 that was closed.
- **Moom review** 5-star 2024-03-02 values being able to "save and recall default window sizes" per app. **HN** (`phkahler`, 2019): "the WM remembers the last placement of each app (and child window, and second or 3rd instance), and put them there when relaunched".
- **Offered today:** FancyZones "Move newly created windows to the last known zone" (free) plus an excluded-apps list; MaxTo 3.1 remembers a window's position per application (paid, page says automatic / on demand / forgotten); DisplayFusion Pro triggers ("Wait for Event: Window Created", "Move Window to Selected Monitor"). Free macOS: no source found.
- **Effort M.** Windows: window-show event, filter to real top-level app windows; PowerToys' own Workspaces FAQ admits the visible jump: "PowerToys cannot tell an app to launch to a specific position... launch an app first, and then give an instruction to move and resize it". macOS: observer per running process for window-created, cleaned up on quit. Rules keyed by executable / bundle id, never titles, fit the privacy rules.

#### 8. Raise a whole snapped group together

- **PowerToys #16018** (open, 75 reactions, 23 comments, 2022-02-03): "One can easily bring the entire set of windows to the front. This doesn't apply for some odd reason to windows snapped in FancyZones."
- **Offered today:** Windows 11 Snap groups (Microsoft Snap page); FancyZones: not offered (open request). macOS: no source found.
- **Effort M.** Z-order raise without stealing focus is possible on Windows with `SetWindowPos`; cross-app raising on macOS goes through app activation and changes focus.

#### 9. Drag-from-anywhere that also snaps to zones

- **PowerToys #47268** (opened 2026-04-28, 57 reactions in six months): "Clicking ALT and moving the window works but when I click Shift to trigger fancy zones, that doesn't work." **#47431** (12). A Show HN comment (2024-12-29, Resizer2) reports the same: "Win+LM+RMB doesnt snap windows into Fancyzone zones."
- **Offered today:** PowerToys Grab And Move (Alt or Win + drag anywhere in the window, no zones; learn.microsoft.com/windows/powertoys/grab-and-move); AltSnap (free, GPL-3.0, 5,353 stars); BetterSnapTool (paid; its page says windows can be moved or resized by holding a key); Easy Move+Resize (named in an HN comment, not checked).
- **Effort M.** Krova currently watches a drag the OS performs. Drag-from-anywhere means swallowing the click and running Krova's own move loop. macOS accessibility moves are slower per call, so expect lag.

#### 10. Pin on top

- **AeroSpace** (a tiling manager; its audience is power users) #2 "sticky floating windows" (107) and #4 "floating windows always on top" (58). **Magnet** 4-star 2026-08-21 "I would really like an option to keep a selected window on top"; 5-star 2026-04-17 asks the same. **Loop** #1121 (2). **Rectangle** discussion "Keep windows on top 'sticky'" (4).
- **Offered today:** PowerToys Always On Top (free, Windows); DisplayFusion Free "Function to force a window to be 'always on top'"; AquaSnap (shake). Mac: no source found. Rectangle Pro "pin one app to a side" is a different feature.
- **Effort: Windows S, macOS L** (see table). Already well served on Windows; a macOS version would be notable but costly.

#### 11-20. Shorter notes

- **11 Window-edge magnet (PowerToys #8, 80 +1, 25 comments, 2019-05-07).** AquaSnap: windows "behave like magnets". DisplayFusion lists "Window Snapping" ("per-app ignore options"); free vs Pro tier not shown on the pages I read. Needs live correction during a drag.
- **12 Scripting (PowerToys #12 "Scripting API for Window Manager", 50, 0 comments).** FancyZones documents a CLI (`PowerToys.FancyZones.CLI.exe`: set-layout, set-hotkey, get-monitors...). Rectangle documents `rectangle://execute-action?name=...`. BetterSnapTool 4-star 2026-03-06: "No ability to use Apple Shortcuts to set a Window to a specific location". Hammerspoon was named by 7 commenters in an HN macOS-apps thread (https://news.ycombinator.com/item?id=40627395).
- **13 Numeric entry (PowerToys #463 "allow manual coordinates (absolute and relative)", 77 +1; #1564 ruler, 13).** FancyZones docs describe nudging canvas zones by keys, not typing values. MacsyZones #20 "allow typing the pixel values" (2). Also helps accessibility.
- **14 Portable layouts.** Loop #945 "import/export all of Loop's settings" (5). BetterSnapTool 4-star 2024-03-04: "save the configurations and move them to any of my machines". Moom 5-star 2024-04-18 wants iCloud sync; Magnet 3-star 2025-12-17 struggles with iCloud sync. FancyZones: custom layouts are a JSON file that "can be... exported to share layouts across devices", but "editing these files is not recommended". No source found for a layout file that works on both Windows and macOS.
- **15 Ultrawide / portrait.** Magnet: at least 8 reviews mention ultrawide or large monitors, including wishes for sixths ("Nothing smaller than quarters", 2-star 2026-05-27), "Center Two Thirds" (4-star 2024-10-01) and "middle half of my 5K monitor" (2024-10-14). Divvy 3-star 2018-04-12 "max grid settings are 10". HN 2025-09-10 (item 45199663): "FancyZones is a must-have if you use an ultrawide monitor!". AeroSpace #60 "Better support for ultrawide monitors" (65). Drawn zones answer this directly; the work is good templates (centre column, 3/5 + 2/5, portrait stacks).
- **16 Multi-zone span.** FancyZones documents Ctrl-while-dragging, hovering a shared edge and Win+Ctrl+Alt+Arrows. BentoBox: "Span a window across adjacent zones". MacsyZones #6 (3 +1) and #22 are open requests. Parity feature.
- **17 Gaps.** FancyZones "Show space around zones"; macOS Settings "Tiled windows have margins"; Divvy margins praised in a 2020 review. #2151 "1-pixel gap between adjacent windows" (36). A BetterSnapTool review (1-star 2025-04-22) complains of unwanted gaps, so make it a setting.
- **18 Restore size on drag-out.** BetterSnapTool 5-star 2026-08-02: "thanks to the ability to disable 'Restore old window size if window is dragged away.' The native macOS solution doesn't allow for this preference". Magnet 1-star 2026-10-13 likes keeping the snapped size. FancyZones offers "Restore the original size of windows when unsnapping" as a setting. So: offer a toggle.
- **19 Swap windows (PowerToys #290, 15).** No source found for a tool that does it. Becomes easy once the registry knows each zone's occupant.
- **20 Zone tabs (PowerToys #15921, 16).** Rectangle has "Cycle stacked windows" (README); AquaSnap mentions TidyTabs tabbed groups; FancyZones cycles with Win+PgUp/PgDn. `docs/SPEC.md` lists stacks/tabs as a v0.1 non-goal.

---

## B. Verified gaps

Format: **Offered by** (with source) / **Absent per source** / **Not documented or not checked**. "Absent per source" means an open request or user report says so; it is the strongest negative I can give.

### B.1 Per top candidate

**1. Windows return to their zones on reconnect**
- Offered by: Windows 11 (positions only; Microsoft support page); Moom (P; reviews); Rectangle Pro (P; vendor page "Activate when displays are connected or disconnected"); DisplayFusion Pro (P; compare page "Save or Restore all window locations").
- Absent per source: FancyZones for dock/undock zones (#4 open, but its comments are from 2019-2020; Windows 11 has since added the native setting).
- Not documented / not checked: FancyZones reconnect behaviour in practice; MacsyZones and BentoBox (MacsyZones #114 shows reconnect bugs, so it does something); Magnet (reviews ask for it); Rectangle free.
- Claim status: "windows return to their *zones*" is not shown to be offered by any free tool in what I read, but I did not test FancyZones or MacsyZones. **Test before "only".**

**2. Keyboard hotkey to a specific zone**
- Offered by: BentoBox (P), MaxTo (P), MacsyZones (F, Quick Snapper number keys).
- Absent per source: FancyZones (#492 open, 262 reactions). Rectangle/Magnet/Divvy are preset-based, not zone-based.
- Claim status: not unique. Do not use "only".

**3. Maximize inside a zone**
- Offered by: no source found.
- Absent per source: FancyZones (#279 open). MaxTo's page does not mention it.
- Claim status: no one appears to offer it, **and it is mostly infeasible**; see Part C.

**4. Auto-fill / adaptive layouts using drawn zones**
- Offered by: no source found for the hybrid. Pure dynamic tilers exist (free).
- Claim status: a "drawn zones that auto-fill" claim may hold, but it needs a market check at the time of use.

**5. Linked dividers over drawn zones**
- Offered by: Windows Snap (its own layouts), AquaSnap (free for personal use), Swish (P). Indicated but unconfirmed: MacsyZones (F; see A.2 feature 5).
- Absent per source: FancyZones (#254 open), Loop (#264 open).
- Not documented: Rectangle, Magnet, BentoBox, MaxTo.
- Claim status: on **Windows**, "free and open-source drawn zones with linked dividers" is plausible (FancyZones lacks it). On macOS it may not be unique (MacsyZones, unconfirmed). Any "only" needs the checks in C.2.

**6. Virtual desktop / Space layouts**
- Offered by: FancyZones (devdocs), BentoBox, MacsyZones, SizeUp (move between Spaces).
- Claim status: not unique.

**7. App-to-zone rules**
- Offered by: FancyZones (last known zone only), MaxTo 3.1, DisplayFusion Pro.
- Claim status: not unique on Windows. Free macOS: no source found.

**8. Group raise**
- Offered by: Windows 11 Snap groups. Absent per source: FancyZones (#16018 open).

**9. Drag-anywhere + zones**
- Offered by: no source found for the combination. PowerToys has both pieces separately (Grab And Move and FancyZones) and users ask to join them (#47268 open).

**14. Layout file that works on both OSes**
- Offered by: no source found. FancyZones' file is Windows-only; other tools' files are per-OS apps.
- Claim status: true by construction for Krova (normalized 0..1 coordinates, `docs/SPEC.md` item 1). Verify with a real import on both OSes before saying it.

### B.2 What a truthful "only" can and cannot say

- Cannot say: "only free drawn-zone manager", "only one with Shift-to-show-zones", "only one on Mac with custom zones".
- Possible, subject to checks: "free and MIT-licensed, with the same drawn zones on Windows and macOS" (as of a stated date). Cross-OS tools I found are both paid: MaxTo (Windows + Mac; Mac launched 2026-10-18) and Divvy (Windows + Mac; grid rather than drawn zones; last Mac App Store update 2019-08-23). The free open-source ones I found cover one OS each (FancyZones: Windows; MacsyZones: macOS). I found no free open-source tool covering both, which is not proof there is none.
- Any "only" must carry a date and the list of tools checked, and be re-run before each release.

### B.3 Corrections needed in `docs/compare-research.md`

(I did not edit it.)

1. Its closing bullet says one tool covering both OSes with the same drawn-zones workflow is where Krova "looks distinct", and that user-drawn zones on macOS are not offered by free Rectangle, Magnet or macOS tiling. Those three are right, but the section is silent on tools that **do** offer drawn zones on macOS:
   - **MacsyZones**: free, "open source" per its site, GPL-3.0 per GitHub API (920 stars, last push 2026-09-15). Site: "Hold the Snap Key (default: Shift) while dragging a window", "Create layouts with your own design", per-screen and per-workspace layouts, keyboard Quick Snapper. A paid "Pro" ($7) has the same features. https://macsyzones.com and https://github.com/rohanrhu/MacsyZones
   - **BentoBox**: $9 once, 14-day trial, macOS 14+; "Hold Shift or right-click while dragging to show your zones". https://bentoboxapp.com/
   - **BetterSnapTool**: $1.99 on the Mac App Store; "create your own, custom snap areas anywhere on your display". https://folivora.ai/bettersnaptool
   - **MaxTo**: Windows and **Mac** (Mac launch post dated 2026-10-18); "Carve any monitor into custom zones. Snap any window into one with a keyboard shortcut."; EUR 19/year or EUR 79 lifetime. https://maxto.net/ and https://maxto.net/mac
   - **Loop**: free, GPL-3.0, 11.9k stars; radial menu with custom frames; not drawn zones. https://github.com/MrKai77/Loop
2. Its "what the site must not hide" list names FancyZones' keyboard snapping, layout hotkeys, spanning, mouse-wheel switching, CLI and exclusion list. Add what the same Microsoft doc page also lists: a **setting to restore the original size on unsnap**, **space around zones**, **last known zone for new windows**, **rotating windows across monitors** (Alt+X) and **cycling windows within a zone** (Win+PgUp/PgDn). Also note MaxTo does not document a hold-a-key overlay on its pages (its Windows page says windows "snap into zones while you drag"); the earlier "hold-a-key" distinction was not checked against it.
3. The note that Windows' remember-position behaviour was "not verified" can now be closed: it is in Microsoft's multi-monitor page (quoted in A.2, feature 1).

---

## C. Recommendation

### C.1 Build this first: a snap registry and a window-event layer

Almost every high-value item (1, 2, 5, 7, 8, 19) needs the same two things that v0.1 does not have: (a) an in-memory map `WindowId -> (monitor, zone)` for windows Krova snapped, and (b) OS window events (Windows `SetWinEventHook`; macOS `AXObserver` / `NSWorkspace` notifications) instead of only the mouse hook. Neither stores titles or touches disk. Build them once, then each feature is S-M instead of M-L.

### C.2 The five features

| Pick | Feature | Why | Win / mac |
|---|---|---|---|
| P1 | Keyboard zone control (hotkey per zone, next/prev, send to other monitor keeping the zone) | Rank 2; already planned for v0.2; 262/95/37 reactions plus 8 Magnet reviews. Table stakes more than personality | S-M on both |
| P2 | Reconnect memory: windows go back to their zones | Rank 1; broadest evidence; macOS has only paid answers; fits "private" because it is in-memory | M on both; macOS timing is the risk |
| P3 | Linked dividers on drawn zones | 103 reactions open for seven years on the Windows incumbent; users ask for it on Mac too | M on both; Windows is the clearer gap |
| P4 | App-to-zone rules (names only) | 29 reactions plus several mentions; paid elsewhere | M on both |
| P5 | Portable layout files + CLI / URL scheme | Cheap; cross-OS portability is true by construction; scripting asked for (#12: 50) | S-M |

Together these make one story: **"drawn zones that remember their windows."**

**Honest wording, only if the feature ships and passes its check:**

- **Overall line (website):** "Free, open-source (MIT) zone manager. Draw your zones once; the same zones work on Windows and macOS." Check before using "only" anywhere near it: re-search GitHub, Homebrew casks, winget, the Mac App Store and the Microsoft Store for free + open-source + both-OS zone managers; re-check MaxTo, Divvy and MacsyZones; date the claim.
- **P1:** "Snap by mouse or by keyboard: give any zone its own shortcut." No "only". Check: none needed beyond truth of own feature.
- **P2:** "When a monitor comes back, its windows go back into their zones. Krova keeps this in memory only and never stores window titles." Do not claim "restores window positions" (Windows 11 does that natively). "Only" check: (a) dock/undock test with FancyZones on Windows 11 with and without its "Keep windows in their zones" option; (b) read or trial MacsyZones, BentoBox, MaxTo, Moom, Rectangle Pro for the same behaviour; (c) test Krova with a real Mac sleep/wake and a real dock.
- **P3:** "Drag the edge between two snapped windows and both resize. Free and MIT-licensed." Do not write "only" without: (a) re-reading the latest FancyZones release notes and doc page, (b) checking AquaSnap's licence and WindowGrid/GridMove, (c) a MaxTo and BentoBox trial. On macOS, check MacsyZones' resize handles first (they may already do this).
- **P4:** "Choose which zone an app opens in. Rules use app names, never window titles." No "only" (FancyZones has last-known-zone; MaxTo and DisplayFusion have rules).
- **P5:** "Layouts are plain JSON files in proportions, so the same file works on Windows and macOS." Check: import one file on both OSes, from a clean install, before publishing.

### C.3 Quick wins (parity, not personality)

Ship these because users expect them and they cost days: typed numeric zone values (#463: 77), ultrawide and portrait templates (centre column, 3/5 + 2/5, thirds, sixths), multi-zone span while dragging, a gaps setting, and a "restore size when dragged out" toggle. Treat accessibility of the editor (keyboard-only, screen reader) as a baseline; I found no demand signal either way, but FancyZones documents a keyboard-operable editor, so it is a fair expectation.

### C.4 Considered and rejected or deferred

| Idea | Verdict and reason |
|---|---|
| Maximize / fullscreen inside a zone (#279, 410) | Defer. Highest count, but L and mostly unsolvable (video fullscreen is app-owned; maintainers called it "hacky"). Revisit as "maximize button fills my zone" only after the event layer exists. |
| Full auto-tiling (#2694, 384) | Defer. Different product (tilers serve it, free); high cost. A small "fill next empty zone / snap all" (M) is a possible v0.3. |
| Workspaces that launch apps | Reject for now. Already free on Windows (PowerToys Workspaces; its own #34578 asks for virtual-desktop support, 118 reactions), paid on Mac (Rectangle Pro, Moom). Needs launching, CLI args and scope creep. Two Magnet/Moom reviews ask for it (2025-02-16, 2021-04-14). |
| Virtual desktop / Space layouts | Defer. Good evidence (118, 38, 19 upvotes) but Windows M and macOS L (private APIs); already in BentoBox/MacsyZones/FancyZones. Windows-first if ever. |
| Pin on top | Defer. Free on Windows (PowerToys, DisplayFusion Free); impossible to do cleanly on macOS. |
| Group raise (#16018, 75) | Defer. Valid, Windows 11 has Snap groups; do after the registry exists. |
| Drag-anywhere with zones (#47268, 57) | Defer. Needs its own move loop; PowerToys may close this gap itself. |
| Window-edge magnet (#8, 80) | Reject. Needs live correction during an OS drag; AquaSnap offers it. |
| Gestures (Swish) | Reject. Mac-only, paid niche, and an HN thread (item 46997008) shows users split on it. |
| Hot corners / edge actions | Reject. No demand signal found beyond Magnet's drag-to-edge areas. |
| Undo last snap | Reject. No source found asking for it. |
| Snap preview with real window contents | Reject. No source found asking; FancyZones has a "make dragged window transparent" option already. |
| Share a zone in a video call (PowerToys #2774, 91; #43464, 39) | Reject. It is a screen-capture feature, not window management. A maintainer pointed it back to #279. |
| Zone tabs / stacks (#15921, 16) | Defer. L, a stated v0.1 non-goal, FancyZones already cycles windows in a zone. |
| Cloud sync of settings | Reject. Conflicts with offline/no-network; file export (P5) covers the need. |

Two supporting observations from reviews that fit Krova's principles but are not features:

- **Reliability is the main churn driver on macOS.** Of the 40 one-star Magnet reviews I read, roughly 30 (my read, not a precise count) are about the app not working after an update, crashing, interfering with other apps, or unanswered support, not about missing features. A public issue tracker and quick fixes are an advantage. FancyZones' own docs list about 20 apps that cannot be zoned, and the top FancyZones issues include WSLg/WSA windows (#10942, 31) and Remote Desktop windows (#9040, 22).
- **Relative layouts and no phone-home are valued.** Moom 4-star 2023-10-12: "Unlike Better Snap Tool, this layouts on a grid instead of fixed pixels, which is great when you switch between a laptop screen and an external monitor... One negative is that it phones home." A BetterSnapTool review (1-star 2023-10-12) complains it seems "to do layouts in pixels and not relatively". Krova's normalized coordinates and no-network rule match both. Nags are a small recurring complaint (Magnet 1-star 2026-06-11 "nags for reviews"; MacsyZones #84 'Frequent "Buy Premium" pop-up').

---

## D. Method and limits

### D.1 What worked

- **GitHub REST search** (unauthenticated; search limit 10 per minute, core 60 per hour, I stayed inside it). All seen 2026-10-30:
  - PowerToys, label `Product-FancyZones`, open issues, sorted by reactions: `total_count` 969; I read the top 40.
  - PowerToys, label `Product-Workspaces`, open: 150; read the top 20.
  - Rectangle: 54 open issues (reaction counts are tiny, the top open issue has 9), 765 issues in all (top 20 read); the Rectangle discussions list (open, sorted by top; 25 shown).
  - Loop: 49 open (top 25 read); MacsyZones: 50 open (top 30 read); AeroSpace: 141 open (top 15 read); AltSnap: 122 open (top 12 read, mostly donation and small requests).
  - Comments on PowerToys #279, #2694, #4 (comments sorted by their own reactions).
  - I did **not** query the "Idea-Enhancement" label separately; sorting FancyZones issues by reactions already surfaces those.
- **Mac App Store reviews** via the iTunes RSS, pages 1-5 (up to 250 per app): Magnet 250 (2024-08-26 to 2026-10-19), BetterSnapTool 250 (2023-05-23 to 2026-10-27), Moom Classic 250 (2012-06-08 to 2026-09-11), Divvy 213 (2011-01-21 to 2026-10-17). Star split (1/2/3/4/5): Magnet 40/9/21/20/160; BetterSnapTool 22/4/10/10/204; Moom 23/6/12/12/197; Divvy 18/9/14/15/157. I read them with keyword filters plus hand reading of matches. Counts such as "8 of 250 Magnet reviews" are hand counts of reviews I read, not regex output; a regex screen I also ran was too noisy to quote.
- **Hacker News** via Algolia: story and comment search; full comment trees for item 20895031 (FancyZones, 507 points / 163 comments), 46997008 (macOS Tahoe resizing, 876 / 522), 40627395 (macOS apps, 183 / 254). Tool-name counts below are distinct commenters per regex.
  - Tahoe thread (2026-02-12): Rectangle 18, AeroSpace 8, AltSnap/AltDrag 6, Raycast 5, PowerToys 5, BentoBox 3, Swish 3, FancyZones 3, Moom 2, Hammerspoon 2, yabai 2, MacsyZones 1, Lasso 1, Amethyst 1, Magnet 0, BetterSnapTool 0.
  - Macs-apps thread (2024-06-09): Rectangle 26, Raycast 17, Hammerspoon 7, Magnet 6, yabai 4, AeroSpace 3, BetterSnapTool 2.
- **Vendor and Microsoft pages**: Microsoft Learn (FancyZones, Workspaces, Grab And Move, Always On Top), Microsoft Support (Snap, multi-monitor), vendor pages for Rectangle Pro, BetterSnapTool, Moom, Swish, SizeUp, Divvy, DisplayFusion (features and compare), AquaSnap, MaxTo (and its Mac page), BentoBox, MacsyZones, Loop.

### D.2 What was unavailable or incomplete

- **WebSearch**: failed with an API error, as in the earlier research. No search-engine results were used.
- **Reddit**: refused by the fetch tool ("unable to fetch from www.reddit.com"); one attempt, then dropped. No Reddit evidence is in this file.
- **Hookshot**: hookshot.app is now a notice pointing to Rectangle Pro; no separate product page.
- **Rectangle Pro price**, **DisplayFusion pricing**, **AquaSnap pricing/Pro tier**: not on the pages I could read.
- **Mac App Store rating counts**: the lookup API returned 0 for all apps, so I cannot say how many ratings each has.
- **Not covered**: GridMove, WindowGrid, Lasso, Raycast's window management, Amethyst/yabai issue trackers, Product Hunt, Stack Overflow, Homebrew/winget searches for new entrants.
- **Swish** appears to have a Black Friday price banner on the fetched page; I quote its $16 list price only.
- **Page-summary caveat:** the fetch tool summarises pages with a small model. The Microsoft Learn pages for FancyZones, Workspaces and Grab And Move came back as raw page text. I also checked raw text for: Microsoft's multi-monitor and Snap support pages, the Swish, BentoBox, Rectangle discussions and MacsyZones README text, and PowerToys devdocs. Product-page facts for BetterSnapTool, Moom, Rectangle Pro, DisplayFusion, AquaSnap, MaxTo, SizeUp, Divvy, Loop, MacsyZones' site and the AltSnap / MacsyZones repository metadata came through the summariser and **must be re-read on the raw page before anything is published**.

### D.3 Bias

- GitHub reactions skew to Windows power users who file issues, and to **old** issues, which have had years to accumulate votes (#279 dates from 2019). Newer signals to weigh more per unit time: #34578 (118 since 2024-09) and #47268 (57 since 2026-04). Reaction totals include hearts; I give the +1 count where it differs.
- App Store reviews: in these samples 5-star reviews are the majority (Magnet 160 of 250), so they are not uniformly negative, but 1-3-star reviews are dominated by breakage and support, which hides feature wishes in the 4-5-star "one request" reviews. The RSS "most recent" window covers about 26 months for Magnet and 14 years for Moom, so the samples are not comparable in time.
- Hacker News skews to developers and tiling-window-manager fans. The macOS Tahoe thread is about window *resizing* in general and mentions window managers only incidentally.
- Many wishes in review text come from users of one tool asking for features that exist in a competitor (e.g. Magnet users asking for Moom's saved layouts). That is real unmet demand within that tool's market, but not proof that free tools lack the feature.
- Rectangle's open-issue counts are low (54 open), probably because requests are moved to discussions or closed, so reaction counts there are not comparable to PowerToys.
- Effort and risk ratings are mine and not benchmarked.

### D.4 Housekeeping

- This is the only file written to the repo. Scratch data and helper scripts are under `%TEMP%\kr` and `%TEMP%\krscripts` (outside the repo).
- Reproduce the headline queries (seen 2026-10-30):
  - `https://api.github.com/search/issues?q=repo:microsoft/PowerToys+label:Product-FancyZones+type:issue+state:open&sort=reactions&order=desc&per_page=40`
  - `https://api.github.com/search/issues?q=repo:microsoft/PowerToys+label:Product-Workspaces+type:issue+state:open&sort=reactions&order=desc&per_page=20`
  - `https://hn.algolia.com/api/v1/items/46997008`
  - `https://itunes.apple.com/us/rss/customerreviews/page=1/id=441258766/sortby=mostrecent/json` (and pages 2-5; ids in A.2)
