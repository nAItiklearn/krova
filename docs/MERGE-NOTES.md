# Merge notes (integrate)

`integrate` = `next` + `wave3` + `wave4` + security fixes F-26, F-28, F-29 +
`notch` + `corners` + `engine` (wave 1, wave 2: shortcut scheme, rebinding,
warm `panel`/`switcher` popups, pause, `hooks.rs`; appearance; popup pages
plumbing). Each branch's lines sit in `// --- <branch> ---` fences; glue between
branches is in `// --- integration ---` fences.

## How the conflicts were resolved

Rule throughout: keep both sides, fences intact, one `import type` per module in TS.

- `krova-core/config.rs`: `Settings` has wave3, wave4, `notch`, `hot_corners`,
  `app_rules`, `shortcuts`, `appearance`; `MonitorProfile` has `reserved`,
  `auto_arrange`, `preset_seeded`. Wave 1 core files (`arrange`, `stack`, `swap`,
  `presets`, `app_rules`) are byte-identical to engine's.
- `krova-platform`: every module kept; `win.rs` `mouse_proc` runs the hot-corner
  check first (atomics only), then the drag feed, then wave4's pointer feed.
- `src-tauri`: capabilities are engine's `default.json` (main) and `popups.json`
  (panel, switcher) plus a new `picker.json`. The notch's standalone panel window
  is gone: it calls `ui_window::show_panel`. `PanelAnchor` is engine's type
  (corner as a string; same JSON). Tray: engine's Pause and Quick panel items use
  wave4's `t()` (`tray.pause`, `tray.panel`, English in every table for now).
  `SnapRecord.owner` (F-20) is filled in by wave3 restore and the hover picker.
- UI: engine's `NavigateTarget` (adds `cheat_sheet`, `panel`); the notch's
  `PanelAnchor` (typed `NotchCorner`) and `onPanelAnchor`/`panelAnchor`/`hidePanel`;
  `PopupLabel` gains `picker`.
- Docs: SECURITY-REVIEW section 8 (engine) then 9 (integrate); PROGRESS and
  features-contract keep every branch's sections; features-contract has a new
  "Integration of the branches" section that describes the glue below.

## Integration (what the fenced glue does)

- Pause = the engine's pause. Notch Pause and a `pause` hot corner send
  `Msg::SetPaused`. While paused: focus follows mouse and the hover picker stop
  (`wave4::paused`, watcher stopped on `PauseChanged`), hot corners keep only
  `pause` corners, the notch shows "Paused".
- Share-safe: `wave3::apply_settings` tells the notch (`notch::set_share_safe`)
  and the strips (`Shared::set_hide_app_names`); `build_state` drops
  `front_app.exe`; the notch, ripple and strip windows join the F-29
  capture-exclusion registry.
- `focus` / `calm_desk`: engine `Action(..)` hooks (registered in `wave3::start`)
  and hot corners (`wave3::register_corner_actions`) run wave3's toggles.
- Hover picker window `picker` (`popups.rs`): shown near the cursor on
  `hover-picker`, hidden on `hover-picker-end` / after `place_hovered`.

## Test-binary trap (src-tauri, windows-gnu)

Code reachable from unit tests must not hold tauri types (`AppHandle`, `Arc<dyn Fn>`
built from one, `Option<String>` drop glue from tauri). It pulls the webview stack
into the test exe, which has no manifest and dies with `STATUS_ENTRYPOINT_NOT_FOUND`
(see `[profile.test.package.krova-app]`). The notch hook is a plain boxed callback
for that reason; hooks are registered only from `start` functions tests do not call.

## Still open (for the next merges and a person with a PC)

1. ~~**UI pages.**~~ Done on `next` (final join): `ui-merge` brought `panel.html`,
   `switcher.html` and `picker.html` as Vite inputs; the picker page calls
   `place_hovered` and hides its own window.
2. ~~**Share-safe in the popups.**~~ Done on `next`: the panel, switcher and picker
   windows join the capture-exclusion registry when built (`popups::track_capture`
   -> `krova_platform::desk::track_capture_window`); `front_app.exe` is null under
   share-safe, so the panel shows no program name.
3. **`show_zones`** (hot corner) is only an event: no "show the zones" overlay
   call outside a drag exists yet.
4. **Translations.** `tray.pause` and `tray.panel` are English in every table.
5. **By hand:** notch click opens the engine panel next to it (both edges);
   notch Pause dims the tray icon and stops focus follows mouse; a `pause` corner
   resumes; share-safe on: notch shows "N zones", tab strips show dots, OBS
   capture shows no notch, ripple or strips; hover picker window appears next to
   the cursor without stealing focus.
