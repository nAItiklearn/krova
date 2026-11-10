# Krova roadmap: parity with Shoji, then past it

Written 2026-11-10. Krova stays clean-room (`DECISIONS.md`): features are matched by *what the user can do*, never by copying Shoji's names, copy, visuals or behaviour details. Where Shoji only names a feature without describing it (Focus, No peek, Clean Space, Tab View), Krova defines its own version below.

## The polish bar (every feature, however small)

A feature is not done until all of these hold. Reviewers check this list, not just the tests.

1. **It shows itself.** Every state change has a visible, calm response: a zone lights, a card moves, a toast confirms with an Undo where it destroys anything. No silent success, no silent failure.
2. **Motion is physical.** Springs (damping ratio ~0.8-0.95), never linear tweens; one shared spring helper in `ui/src/lib/motion.ts` so everything feels like one app. Interruptible: a new gesture picks up the current velocity. `prefers-reduced-motion` jumps to the end state.
3. **One visual system.** Tokens only (`ui/src/app.css`); radius, type and shadow scales from the tokens; light and dark both designed, not inverted. Native overlays (drawn in Rust) use the same colours and radii as the UI.
4. **Short words.** Labels of 1-3 words, helper text of one sentence. Settings say what they do, not how.
5. **Keyboard and screen reader complete.** Everything reachable without a mouse, visible focus, roles and live announcements. WCAG AA contrast in both themes.
6. **Shown in the product.** Each feature gets a small demo in the "What Krova can do" settings page and a real screenshot on the website.
7. **Measured.** No idle CPU when nothing happens; memory noted in `docs/measurements/` if it adds a window or thread.
8. **Tested.** Pure logic in `krova-core` with unit tests; UI with vitest and a Playwright e2e in demo mode; platform code with an ignored real-desktop test and a line in the manual test list.

## Status of v0.1 features

Zones editor, modifier-drag snap, edge snap, per-monitor layouts with stable ids, keyboard zone control, window memory on reconnect, linked dividers, zone gap, layout import/export, tray, autostart, themes. All automated checks green; **real-hardware tests not yet run** (see `PROGRESS.md`). New work does not make those tests less urgent. There is no app-rules code yet, although `PROGRESS.md` mentions it.

## Waves

Each wave is built in worktrees by parallel agents with disjoint file ownership, then merged and re-tested before the next wave starts.

### Wave 0 (in progress)
- Desktop UI restyle (clean, Mac-quality, Krova's own look).
- Website redesign.
- Layout switcher: backend now; UI after the owner approves `docs/prototypes/layout-switcher/`.
- Security hardening F-20..F-23 (second pass of `SECURITY-REVIEW.md`).

### Wave 1: arrange without dragging
| Feature | Krova's version | Shows itself by |
|---|---|---|
| **Auto-arrange** (parity: tiling) | Off by default. When on for a monitor, a new window goes to the next free zone of the active layout; closing one lets the rest fill in (optional). Works with *any* drawn layout, not only presets. | The window springs into its zone with the zone briefly lit; a pill says "Auto-arrange on Display 1" when toggled. |
| **Preset families** | Grid, Columns, Rows, Main + side for 1-5 windows, ultrawide and portrait variants, main size 50/60/66 %. They are ordinary layouts the user can edit. | Template picker with live thumbnails that animate between counts. |
| **Stacks** | When every zone is taken, a new window stacks in the focused zone. A slim tab strip on the zone's top edge (Krova's tab view) lists the stack; scroll or `mods+[` / `]` cycles. | Tabs slide in with a spring; the hidden window peeks for 150 ms on cycle. |
| **Swap** | Drop a window on an occupied zone with the modifier held: the two swap. `mods+Shift+arrow` swaps with the neighbour. | Both windows travel at once; the target zone shows a two-way arrow while hovering. |
| **App rules** | Per app (by executable name, never title): never touch / always float / preferred zone / start on monitor. Dialogs, tool windows and owned popups are never arranged. | Rules list with app icon and an inline segmented control; a "Not arranged" chip in the quick panel when the front app is excluded. |

### Wave 2: keyboard and control
| Feature | Krova's version | Shows itself by |
|---|---|---|
| **Rebind any shortcut** | Per-action recorder, turn off, reset. Conflict check against Krova's own map and a live `RegisterHotKey` probe; suggests free chords. | Recorder pill pulses while listening; conflicts in red with one-click suggestion chips. |
| **Shortcut per layout** | `mods+F1..F9` or user keys apply layout N on the monitor under the cursor. | The switcher deck flashes open for 600 ms on the applied card, then closes. |
| **Cheat sheet** | `mods+/` shows every shortcut with the user's own keys; any key closes. | Sheet rises with a spring, keys drawn as keycaps. |
| **Pause** | `mods+P` and a tray item; windows stay where they are. | Tray icon dims; a pill says "Krova paused". |
| **Quick panel** | Clicking the tray icon opens a small panel (not the editor): active layout per monitor, auto-arrange, pause, switcher, editor. Its own lightweight window, so the 199 MB editor is not needed day to day. | The panel is the on-screen entry point for the switcher. |

### Wave 3: workspaces and focus
| Feature | Krova's version | Shows itself by |
|---|---|---|
| **Workspaces** (parity: Setups) | Save the apps on screen with their zones and monitors. Restore launches missing apps, places all of them, minimises the rest (never quits). One shortcut each. Stored locally, executable paths only. | Workspace cards in the switcher deck (second tab: Layouts / Workspaces); on restore each window lands in turn. |
| **Focus** | `mods+F` dims everything except the active window's zone (20-80 %), following focus. | Dim fades with a spring; the zone edge glows faintly. |
| **Calm desk** (Krova's answer to Clean Space) | Minimise every window not in the active layout's zones; toggle back to restore them. | Windows shrink toward the taskbar in sequence. |
| **Share-safe** (Krova's answer to No peek) | While on, Krova's overlays, previews and switcher show only zone shapes, never app names or window contents; for screen sharing. | A small shield in the quick panel. |
| **Window highlight** | None / active border / all borders / soft glow; colour and width. Native click-through overlay, no input hooks. | The border eases between windows when focus changes. |

### Wave 4: fit and finish
- Reserved edges for third-party docks and bars, per monitor.
- Focus follows mouse (opt-in, with a delay).
- Hover actions: a small zone picker on a window's title bar while the modifier is held.
- Languages: i18n layer, English first, then Spanish, German, Japanese, Simplified Chinese (reviewable JSON).
- App icon variants (original Krova designs).
- First-run tour (5 steps, skippable) and the "What Krova can do" settings page with looping demos.
- Updates: no network in the app (privacy promise). "Check for updates" opens the GitHub releases page in the browser. Signing needs the owner's certificate (F-01).

### Beyond Shoji (Krova only)
Freeform drawn zones, linked dividers, edge snap, layout files, zone memory on reconnect, virtual-desktop-aware layouts (`feature-gap-research.md` #6), maximize inside a zone (#3, partial), a CLI for layouts, free and open source, Windows first.

## Open decisions for the owner
- Auto-arrange default: off (recommended; Krova's identity is "you draw, it obeys") or on.
- Workspace restore launching apps: always, or ask each time.
