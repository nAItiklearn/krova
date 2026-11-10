# Wave 1 core API: what exists, what to wire

Pure logic in `krova-core`, no OS calls, nothing added to `Settings`, `Config` or the registry yet. This note is the hand-over for the wiring step. Behaviour and rules are in the module docs; this lists the surface.

| Module | Purpose |
|---|---|
| `stack` | `Stacks`: which window is in which zone, tab order, which tab is visible. `StackChange` is one edit as plain data. |
| `arrange` | `plan(&ArrangeContext, ArrangeEvent) -> Plan`. `ArrangeSettings`, `ArrangePolicy`. |
| `swap` | `neighbour(layout, zone, Direction)`, `swap_windows`, `swap_with_neighbour`, `drop_on_zone`; all return a `Plan`. |
| `presets` | `PresetSpec` -> ordinary `Layout`; `all()`, `family_layouts()`, `PresetVariant::for_size(w, h)`. |
| `app_rules` | `AppRules` (rules by executable name), `WindowKind`, `is_arrangeable`, `AppRules::decide`. |

Identities: a window is a `u64` (`stack::WindowId`; the engine's `WindowKey(u64)` and the platform's `WindowId(u64)` both carry one). Zones are **stable zone ids** everywhere in state and plans. The only zone *numbers* (1-based, reading order, as `<mods>+N`) are the ones a user types: `RuleAction::PreferredZone(n)` and `ArrangeEvent::WindowOpened::preferred_zone`. Reading order is `registry::reading_order`.

## Proposed settings (all `#[serde(default)]`, `schema_version` stays 1)

**Per monitor profile** (`MonitorProfile.auto_arrange: ArrangeSettings`). The roadmap makes auto-arrange a per-monitor switch, and the policy suits one screen (main-first for an ultrawide, next-free for a laptop):

| Field | Type | Default | Meaning |
|---|---|---|---|
| `auto_arrange.enabled` | bool | `false` | The quick panel switch. A disabled planner plans nothing. |
| `auto_arrange.policy` | `"next_free"` \| `"main_first"` | `"next_free"` | Where a new window goes. |
| `auto_arrange.fill_gaps` | bool | `false` | Closing a window lets later windows move up. Off by default: nothing moves unasked. |
| `auto_arrange.stack_when_full` | bool | `true` | With every zone taken, a new window joins a stack; off leaves it alone. |

**User-wide** (`Settings.app_rules: AppRules`, serialised as a plain array):

```json
[ { "exe": "spotify.exe", "action": { "kind": "preferred_zone", "value": 3 } },
  { "exe": "OBS.exe",     "action": { "kind": "never_touch" } },
  { "exe": "calc.exe",    "action": { "kind": "float" } },
  { "exe": "slack.exe",   "action": { "kind": "start_on_monitor", "value": "monitor-2" } } ]
```

Call `AppRules::sanitized()` from `Settings::sanitized()` (limits: 256 rules, names up to 128 chars, no paths, no control / bidi / zero-width characters, one rule per program ignoring case). The UI saves through `AppRules::set` / `remove` so it gets precise errors (`RuleError` has readable messages).

Not settings: presets (the picker produces a layout, saved with the existing `save_layout`), stack and swap shortcuts (`<mods>+[` / `]`, `<mods>+Shift+arrow` belong to the Wave 2 rebinding map), swap by drop (a tracked window dropped on an occupied zone while snapping).

Runtime state, not persisted: one `Stacks` per monitor.

## Engine call points

| When | Call | Then |
|---|---|---|
| A top-level window appears | platform classifies a `WindowKind` and finds the exe file name (`app_rules::exe_file_name` for a path); `rules.decide(kind, exe)` | `Decision::Skip(reason)`: leave it (show "Not arranged" for the front app). `Decision::Arrange { preferred_zone, start_monitor }`: pick the monitor (`start_monitor` first), then `arrange::plan(ctx, WindowOpened { id, preferred_zone })` |
| A tracked window is destroyed | `plan(ctx, WindowClosed { id })` | |
| Foreground changes to a tracked window | `plan(ctx, Focused { id })`, then remember it as `ctx.focused` | |
| Drag-drop of a tracked window on a zone with an occupied target | `swap::drop_on_zone(layout, stacks, id, zone)` | Empty plan for an untracked window: that is an ordinary snap. |
| `<mods>+Shift+arrow` | `swap::swap_with_neighbour(layout, stacks, id, dir)` | |
| Stack cycle key / wheel on a tab strip | `stacks.cycle(zone, CycleDirection)` returns the window to raise | |
| Hover target while swapping | `swap::neighbour` / `stacks.visible(zone)` | for the two-way arrow |
| Active layout changes or is edited | `stacks.retain_zones(\|z\| layout.zones.iter().any(\|zn\| zn.id == z))` | returns the windows that lost their zone; feed them back as `WindowOpened` when auto-arrange is on |
| Auto-arrange switched on | `Stacks::from_assignments(registry snapshot)` | Existing windows are not moved, only new ones are arranged. |
| Monitor disconnected | drop that monitor's `Stacks` | |
| Template picker | `presets::all()`, `family_layouts(family, variant, size)`, `PresetVariant::for_size(work.w, work.h)` | `PresetSpec::layout()` gives the editable layout |

Applying a `Plan`:

1. For each `plan.placements` entry: `layout.zone_pixels_gapped(zone, work, gap)` and the existing animated move. All moves run at once (swaps travel together).
2. `plan.apply(&mut stacks)` (all or nothing). The edits in `plan.stack_changes` are plain data, so they can also be logged or replayed.
3. For each `plan.raise` window: bring it to the front of its zone (it is the tab now showing). Hidden tabs are not minimised; they sit behind at the same rectangle.
4. Light the zone of the event window (the lit zone and the "Auto-arrange on Display 1" pill are UI).

`plan()` never fails and never panics: invalid input (an unknown window, a layout with no zones, a stack at its limit) gives an empty plan.

## Rules the planner follows (summary)

* Opening: a free preferred zone; else the first free zone (`next_free`) or the first zone with the windows in the way shifted along, up to the first free zone (`main_first`); else, with every zone taken, the stack of the preferred zone, the focused window's zone, or the last zone, in that order.
* Closing: the next tab shows if the visible one left. With `fill_gaps`, later windows move up in order, each only as far as needed; a stack gives up its oldest hidden window instead of moving whole.
* Focus: focusing a hidden tab makes it the visible one.
* Swap: windows trade seats; into an empty neighbour a window simply moves.
* Limits: 32 windows per stack, 1024 tracked, 256 rules.

## Open questions

* Should an app rule's preferred zone, when occupied, stack on that zone instead of falling back to the next free zone? Current: fall back, and stack there only when everything is full.
* `main_first` with every zone taken stacks like `next_free` (no shifting). Alternative: stack onto the main zone.
* `fill_gaps` default (`false`) and the global default of auto-arrange (`false`) are the roadmap's open decision.
