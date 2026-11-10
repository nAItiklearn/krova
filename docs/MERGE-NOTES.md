# Merge notes: `engine` into `integrate`

`integrate` = `next` + `wave3` + `wave4` + security fixes F-26, F-28, F-29.
`engine` (at `5db758e`) = an older base (`eb82eb7`, F-20..F-24) + its own wave 1
commit (`c5734cc`) + wave 2 (`a7e0e2c`: Ctrl+Alt shortcut scheme, rebinding,
warm popups, pause, `hooks.rs`) + appearance (`946b59c`, `5db758e`).

A dry run (`git merge-tree --write-tree integrate engine`, before F-26 was
committed) gives content conflicts in exactly these 12 files: `krova-core`
`config.rs`; `krova-platform` `Cargo.toml`, `lib.rs`; `src-tauri` `commands.rs`,
`lib.rs`, `model.rs`, `tray.rs`; `ui` `backend.ts`, `bindings/index.ts`,
`mock/mockBackend.ts`; `docs/PROGRESS.md`, `docs/features-contract.md`.
`docs/SECURITY-REVIEW.md` will conflict as well now (both append a section).
Everything else, including `win.rs` and `win_overlay.rs`, auto-merges.

General rule, as for wave3/wave4: keep both sides, keep the `// --- wave3 ---` /
`// --- wave4 ---` fences, put engine's lines outside the fences.

## Rust

| File | Collision | Resolve |
|---|---|---|
| `krova-core/src/config.rs` `Settings` | wave3 (`focus`, `calm_desk`, `share_safe`, `highlight`) + wave4 (`focus_follows_mouse`, `focus_follows_mouse_delay_ms`, `hover_picker`, `language`) vs engine (`app_rules`, `shortcuts`, `appearance`) | Keep all; no name clashes. Same for `Default` and for `sanitized()` (wave3 sanitizes `focus`, `highlight`; engine `app_rules`, `shortcuts`, `appearance`; check that wave4's `focus_follows_mouse_delay_ms` / `language` clamps, which sit elsewhere in `sanitized`, survive). |
| `config.rs` `MonitorProfile` + its `new` | wave4 `reserved: ReservedEdges` vs engine `auto_arrange: ArrangeSettings`, `preset_seeded: bool` | Keep all three fields, all `#[serde(default)]`. |
| `config.rs` (semantic) | wave3 `focus::appearance()` and `focus::share_safe()` read `appearance.accent` / `appearance.corner_radius_px` through `serde_json::Value` because the field did not exist on their base | Works unchanged after the merge (engine names match: `accent`, `corner_radius_px`). Optional follow-up: read `settings.appearance` directly; note engine's radius is `i32` (0..=16), wave3 casts to `u32` and caps it. `on_settings_changed` already compares `appearance(s)`, so a new accent reaches the focus-dim/highlight overlays. |
| `krova-platform/Cargo.toml` | Both add `Win32_UI_Shell`; integrate also has `Win32_System_Registry`, `Win32_Globalization`, `Win32_Storage_FileSystem`, `Win32_System_Com` | One `Win32_UI_Shell` line; keep every integrate feature. |
| `krova-platform/src/lib.rs` mods | wave3 (`desk`, `win_desk`, `win_capture`, `win_visuals`) + wave4 (`pointer`, `win_ffm`, `win_hover`, `win_shell`) vs engine (`win_strips`, `win_windows`; also `classify`, `strips` above) | Keep all. |
| `krova-platform/src/win.rs` | wave4 made ten helpers `pub(crate)` and added a pointer call in `mouse_proc` (fenced); engine made the same helpers `pub(crate)` and added `owner_of` and trait methods | Auto-merges (identical visibility edits). Check after the merge: `mouse_proc` must still call the wave4 pointer feed exactly once and engine's hook body must not return early before it. |
| `win_overlay.rs` | F-29 registers the overlay in `run` / unregisters before `DestroyWindow`; engine (F-24) ignores `WM_CLOSE` in `wnd_proc` | Auto-merges; keep both. |
| `src-tauri/src/lib.rs` | wave4 `mod i18n;` vs engine `mod hooks;` (plus engine's `popups`, `shortcut_cmds` modules and commands) | Keep both mods. Command list: wave3 (6) and wave4 (3) blocks plus engine's `probe_shortcut`, `set_shortcut`, `reset_shortcuts`, `set_paused`, `hide_popup`, `popup_open_ms`, `take_pending_view`, layout/preset/app-rule commands: no duplicate names. Startup: keep `engine::start(&shared)` first, then `wave3::start(&shared)` and `wave4::start(app.handle(), &shared)` (each owns its own thread: `krova-visuals`, `krova-pointer`; engine its engine/hook threads; nothing races). |
| `src-tauri/src/model.rs` `AppState` | wave3 `wave3: Wave3State` + wave4 `#[serde(flatten)] wave4: Wave4State` (keys `wave4_capabilities`, `pointer`, `language`, `languages`) vs engine `front_app`, `paused`, `shortcut_actions` and new types `FrontApp`, `ActiveLayout`, `ShortcutActionState`, `MonitorCtx` | Keep all; the conflict block on the engine side also carries the new structs, so close `AppState` after the merged fields and keep the structs after it. No flattened key collides. Same in the `Default`/initial state block. |
| `src-tauri/src/commands.rs` `build_state` | wave3 `fill_state`, wave4 `state.wave4 = state_extras(...)` vs engine `front_app`, `paused`, `shortcut_actions` | Keep all. `change_settings`: keep `wave3::on_settings_changed` and `wave4::settings_changed` next to engine's `hooks.emit(SettingsChanged)`. |
| `src-tauri/src/tray.rs` | wave4 builds labels with `t("tray.…")` and registers them with `wave4::manage_tray_labels` (relabel on language change); engine adds `pause` (check) and `panel` items with English text and a bigger `TrayItems { enabled, paused, autostart, tray, normal, dimmed }` | Use `t()` for engine's items too: add `tray.pause` ("Pause arranging") and `tray.panel` ("Quick panel") to every table in `src-tauri/i18n` (missing keys fall back to English, so a stub is fine), add them to the `manage_tray_labels` list (`Check(paused.clone())`, `Plain(panel)`), and keep engine's `TrayItems` shape (clone `paused` before moving it). Keep wave4's "Check for updates" item and handler. |

## UI

| File | Collision | Resolve |
|---|---|---|
| `bindings/index.ts` `Settings` | wave3/wave4 fields vs engine `app_rules`, `shortcuts`, `appearance` | Keep all. |
| `bindings/index.ts` `MonitorProfile` | wave4 `reserved?` vs engine `auto_arrange`, `preset_seeded` | Keep all. |
| `bindings/index.ts` `AppState` | `wave3`, wave4 flattened keys vs `front_app`, `paused`, `shortcut_actions` | Keep all. |
| `bindings/index.ts` type section after `NavigateTarget` | wave3 + wave4 type blocks vs engine's shortcut/arrange/preset types | Keep both blocks; checked: no exported type name is defined on both sides. `EntryResult.path?` (F-26) is integrate-only. |
| `backend.ts` | the merged type import list, and the `Backend` interface / Tauri implementation blocks | One `import type { … }` with the union of names (the wave3/wave4 merge already hit a duplicate-import error here). Keep wave3, wave4 and engine methods. |
| `mock/mockBackend.ts` | imports (`wave3Mock`, `wave4`) vs engine's (`presets`, `shortcuts`); defaults; `reserved` vs `auto_arrange`/`preset_seeded` in profiles; sanitize; snapshot (`wave3`, `...wave4State()` vs engine fields) | Keep all, one line each. |

## Docs

- `PROGRESS.md`, `features-contract.md`: keep both sections (engine's wave 2 / appearance sections and the wave3/wave4 ones).
- `SECURITY-REVIEW.md`: engine's section 8 (F-20..F-24) then integrate's section 9 (F-26, F-28, F-29). Integrate's base lacks section 8, so take engine's version of sections 1-8 and append 9.

## Semantic points to check after the merge

1. **Engine hooks (`hooks.rs`) are unused by wave3/wave4.** Wave3 watches focus with its own WinEvent hook in `win_visuals`; wave4 has its own cursor watcher. That works, but engine's shortcut actions that it does not implement are emitted as `EngineEvent::Action("focus" | "calm_desk" | …)` and nobody listens: register a hook in `wave3::start` mapping `"focus"` to `toggle_focus` and `"calm_desk"` to the calm-desk toggle (on a worker thread or via a channel; hooks run on the engine thread and must not block or call back into the engine). Otherwise the shortcuts for those actions do nothing.
2. **Pause.** Engine's `paused` stops auto-arrange; decide whether wave4 focus-follows-mouse and the hover picker should also stand still while paused (`wave4::settings_changed` could read `shared.is_paused()`; listen to `EngineEvent::PauseChanged`).
3. **Share-safe vs engine's popups.** Engine's quick panel, switcher and cheat sheet are Tauri windows and `AppState.front_app.exe` is an app name. With `share_safe` on, the UI must not show `front_app.exe` (or show it only as "an app"), and F-29's `WDA_EXCLUDEFROMCAPTURE` does not cover Tauri windows; if wanted, apply it to the `panel`/`switcher` windows' HWNDs from `popups.rs` when share-safe changes.
4. **Capabilities.** Engine's `capabilities/default.json` lists windows `main`, `panel`, `switcher`; integrate's lists only `main` and neither side adds new permissions, so take engine's file. wave4's hover picker is drawn by the UI; if it gets its own window label, add it there (once).
5. **Shortcut modifier vs hover modifier.** wave4 uses `settings.modifier` (the drag modifier) for the hover picker; engine replaced `shortcut_modifier` with the `shortcuts` map but keeps `modifier`, so nothing changes. Make sure engine's migration does not rewrite `modifier`.
6. **Appearance accent in wave3 visuals.** `VisualsConfig::from_settings` takes the accent through `focus::appearance`; after the merge, a test that sets `settings.appearance.accent` and expects the highlight colour (when `highlight.color` is null) would pin it.
7. **Wave 1 twice.** `next` has the wave 1 core logic as `20015cf` (krova-core only); engine has its own full wave 1 commit `c5734cc` (core, app, UI incl. `mock/presets.ts`). The shared core files merged cleanly in the dry run, but run the full test suites afterwards: two versions of the same planner/preset code would show up as duplicate tests or diverging results there.
