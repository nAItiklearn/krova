//! Auto-arrange planner: where does a new window go, and what moves when one closes.
//!
//! The planner is a pure function. It gets the active layout, the current
//! occupancy ([`Stacks`]: window to zone, plus stack membership), which arranged
//! window had the focus, the settings and one event, and returns a [`Plan`]: the
//! windows to move, the edits to apply to the occupancy and the windows to raise.
//! It never touches the OS and never mutates its input, so the engine can call it,
//! show the result (the lit zone, the toast) and then [`Plan::apply`] it.
//!
//! Zones are named by their stable id. "Reading order" is the zone order of the
//! keyboard shortcuts ([`crate::registry::reading_order`]): top to bottom by `y`
//! rounded to 1 %, then left to right. A zone is *free* when nobody is in it.
//!
//! # Opening a window
//!
//! 1. A preferred zone (an app rule) wins if it is free. It is a 1-based zone
//!    number, the same number `<mods>+N` uses; numbers past the layout are ignored.
//! 2. Otherwise the first free zone by policy:
//!    [`ArrangePolicy::NextFree`] takes the first free zone in reading order;
//!    [`ArrangePolicy::MainFirst`] gives the new window the first zone and shifts
//!    the windows that were in the way one zone along, up to the first free zone.
//!    Windows beyond that point do not move.
//! 3. With no free zone left the window joins a stack (unless
//!    [`ArrangeSettings::stack_when_full`] is off): that of its preferred zone, else
//!    that of the focused window's zone, else the last zone in reading order.
//!
//! # Closing a window
//!
//! The window leaves its stack; if it was the visible one, the next tab shows. With
//! [`ArrangeSettings::fill_gaps`] a zone that emptied is refilled from the zones
//! after it, keeping their order and moving each window as few zones as it takes. A
//! zone holding a stack gives up its oldest hidden window rather than its whole
//! stack, so no zone is left empty while another is stacked.
//!
//! # Focus
//!
//! Focusing a hidden member of a stack makes it the visible one.

use serde::{Deserialize, Serialize};

use crate::registry::reading_order;
pub use crate::stack::WindowId;
use crate::stack::{StackChange, StackError, Stacks, MAX_STACK_DEPTH};
use crate::zone::Layout;

/// How a new window picks its zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArrangePolicy {
    /// The first free zone in reading order.
    #[default]
    NextFree,
    /// The new window takes the first zone; the windows in the way shift along.
    MainFirst,
}

/// Auto-arrange settings. Proposed to live per monitor profile (the roadmap makes
/// the switch per monitor); not part of `Settings` yet. Every field has a default so
/// an older config, or a missing key, loads as "off".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ArrangeSettings {
    /// Off by default. A disabled planner plans nothing.
    pub enabled: bool,
    pub policy: ArrangePolicy,
    /// Closing a window lets the windows after it move up.
    pub fill_gaps: bool,
    /// With every zone taken, a new window joins a stack. Off: it is left alone.
    pub stack_when_full: bool,
}

impl Default for ArrangeSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            policy: ArrangePolicy::NextFree,
            fill_gaps: false,
            stack_when_full: true,
        }
    }
}

/// What happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrangeEvent {
    /// A window the engine decided may be arranged has appeared. `preferred_zone` is
    /// a 1-based zone number in reading order (an app rule's `PreferredZone`).
    WindowOpened {
        id: WindowId,
        preferred_zone: Option<u32>,
    },
    /// A tracked window went away. Unknown windows are ignored.
    WindowClosed { id: WindowId },
    /// A window became the foreground window.
    Focused { id: WindowId },
}

/// Everything the planner reads.
#[derive(Debug, Clone, Copy)]
pub struct ArrangeContext<'a> {
    /// The active layout of the monitor.
    pub layout: &'a Layout,
    /// The occupancy before the event.
    pub stacks: &'a Stacks,
    /// The arranged window that had the focus before the event, if any.
    pub focused: Option<WindowId>,
    pub settings: ArrangeSettings,
}

/// A window that has to go to a zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub window: WindowId,
    pub zone: u32,
}

/// The outcome of one event.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Plan {
    /// Windows whose zone changes (a new window included), ordered by the reading
    /// position of the zone they go to, then by tab, then by window id.
    pub placements: Vec<Placement>,
    /// The edits that turn the old occupancy into the new one, in order.
    pub stack_changes: Vec<StackChange>,
    /// Windows that must be brought to the front of a zone shared with other windows:
    /// a window that joined a stack, or the tab revealed when the one on top left. A
    /// window that ends up alone in its zone, with nothing to reveal, is not listed.
    /// Ordered by zone id.
    pub raise: Vec<WindowId>,
}

impl Plan {
    /// Nothing to do.
    pub fn is_empty(&self) -> bool {
        self.placements.is_empty() && self.stack_changes.is_empty() && self.raise.is_empty()
    }

    /// Replay the edits on `stacks`. All or nothing: on an error `stacks` is left as
    /// it was.
    pub fn apply(&self, stacks: &mut Stacks) -> Result<(), StackError> {
        let mut next = stacks.clone();
        for change in &self.stack_changes {
            next.apply(change)?;
        }
        *stacks = next;
        Ok(())
    }

    /// Derive the placements and the raise list from `changes` applied to `before`.
    /// `None` if the edits are not valid for `before`. Used by the planners in this
    /// crate; public so the engine can build a plan from its own edits (a drag, a
    /// manual stack cycle) and get the same shape back.
    pub fn from_changes(
        layout: &Layout,
        before: &Stacks,
        changes: Vec<StackChange>,
    ) -> Option<Plan> {
        if changes.is_empty() {
            return Some(Plan::default());
        }
        // (zone, visible window, windows in the zone) for every occupied zone.
        let snapshot = |s: &Stacks| -> Vec<(u32, WindowId, usize)> {
            s.occupied_zones()
                .filter_map(|zone| s.visible(zone).map(|w| (zone, w, s.depth(zone))))
                .collect()
        };
        let mut after = before.clone();
        // Windows that took the top of a zone shared with others because of an edit.
        // A stack that moves as a whole keeps its z-order, so `MoveStack` surfaces
        // nothing; a window that lands alone in a zone needs no raising either.
        let mut surfaced: Vec<WindowId> = Vec::new();
        for change in &changes {
            let was = snapshot(&after);
            after.apply(change).ok()?;
            if matches!(change, StackChange::MoveStack { .. }) {
                continue;
            }
            for (zone, window, depth_after) in snapshot(&after) {
                let previous = was.iter().find(|entry| entry.0 == zone);
                if previous.map(|entry| entry.1) == Some(window) {
                    continue;
                }
                let depth_before = previous.map_or(0, |entry| entry.2);
                if depth_before.max(depth_after) >= 2 {
                    surfaced.push(window);
                }
            }
        }

        let order = reading_order(layout);
        let rank = |zone: u32| order.iter().position(|&z| z == zone).unwrap_or(usize::MAX);

        let mut moved: Vec<(usize, usize, WindowId, u32)> = after
            .assignments()
            .into_iter()
            .filter(|&(window, zone)| before.zone_of(window) != Some(zone))
            .map(|(window, zone)| {
                (
                    rank(zone),
                    after.tab_index(window).unwrap_or(0),
                    window,
                    zone,
                )
            })
            .collect();
        moved.sort_unstable();
        let placements = moved
            .into_iter()
            .map(|(_, _, window, zone)| Placement { window, zone })
            .collect();

        let mut raise: Vec<(u32, WindowId)> = surfaced
            .into_iter()
            .filter_map(|window| after.zone_of(window).map(|zone| (zone, window)))
            .filter(|&(zone, window)| after.visible(zone) == Some(window))
            .collect();
        raise.sort_unstable();
        raise.dedup();
        let raise = raise.into_iter().map(|(_, window)| window).collect();

        Some(Plan {
            placements,
            stack_changes: changes,
            raise,
        })
    }
}

/// Plan the response to one event.
pub fn plan(ctx: &ArrangeContext<'_>, event: ArrangeEvent) -> Plan {
    if !ctx.settings.enabled {
        return Plan::default();
    }
    let order = reading_order(ctx.layout);
    let changes = match event {
        ArrangeEvent::WindowOpened { id, preferred_zone } => {
            on_opened(ctx, &order, id, preferred_zone)
        }
        ArrangeEvent::WindowClosed { id } => on_closed(ctx, &order, id),
        ArrangeEvent::Focused { id } => on_focused(ctx, id),
    };
    // Any failure inside the planner means "do nothing": a half plan is worse.
    changes
        .and_then(|c| Plan::from_changes(ctx.layout, ctx.stacks, c))
        .unwrap_or_default()
}

/// A working copy of the occupancy that records the edits made to it.
struct Sim {
    work: Stacks,
    changes: Vec<StackChange>,
}

impl Sim {
    fn new(base: &Stacks) -> Self {
        Self {
            work: base.clone(),
            changes: Vec::new(),
        }
    }

    fn apply(&mut self, change: StackChange) -> Option<()> {
        self.work.apply(&change).ok()?;
        self.changes.push(change);
        Some(())
    }
}

fn on_opened(
    ctx: &ArrangeContext<'_>,
    order: &[u32],
    id: WindowId,
    preferred: Option<u32>,
) -> Option<Vec<StackChange>> {
    let stacks = ctx.stacks;
    // Nothing to place into, or a window we already know: nothing to do.
    if order.is_empty() || stacks.contains(id) {
        return Some(Vec::new());
    }
    let free = |zone: u32| stacks.depth(zone) == 0;
    let preferred_zone = preferred
        .and_then(|n| usize::try_from(n).ok())
        .and_then(|n| n.checked_sub(1))
        .and_then(|i| order.get(i))
        .copied();

    let mut sim = Sim::new(stacks);
    if let Some(zone) = preferred_zone.filter(|&z| free(z)) {
        sim.apply(StackChange::Place { window: id, zone })?;
        return Some(sim.changes);
    }

    match order.iter().position(|&z| free(z)) {
        Some(first_free) => {
            let target = match ctx.settings.policy {
                ArrangePolicy::NextFree => order[first_free],
                ArrangePolicy::MainFirst => {
                    // Everything before the first free zone is occupied: slide each
                    // stack one zone along, last one first, to open the first zone.
                    for p in (0..first_free).rev() {
                        sim.apply(StackChange::MoveStack {
                            from: order[p],
                            to: order[p + 1],
                        })?;
                    }
                    order[0]
                }
            };
            sim.apply(StackChange::Place {
                window: id,
                zone: target,
            })?;
        }
        None => {
            if !ctx.settings.stack_when_full {
                return Some(Vec::new());
            }
            let zone = stack_target(ctx, order, preferred_zone)?;
            sim.apply(StackChange::Place { window: id, zone })?;
        }
    }
    Some(sim.changes)
}

/// The zone whose stack a new window joins when every zone is taken: the preferred
/// zone, else the focused window's zone, else the last zone; if that stack is full,
/// the first other zone with room.
fn stack_target(
    ctx: &ArrangeContext<'_>,
    order: &[u32],
    preferred_zone: Option<u32>,
) -> Option<u32> {
    let focused_zone = ctx
        .focused
        .and_then(|window| ctx.stacks.zone_of(window))
        .filter(|zone| order.contains(zone));
    let first = preferred_zone
        .or(focused_zone)
        .or_else(|| order.last().copied())?;
    std::iter::once(first)
        .chain(order.iter().copied().filter(|&z| z != first))
        .find(|&zone| ctx.stacks.depth(zone) < MAX_STACK_DEPTH)
}

fn on_closed(ctx: &ArrangeContext<'_>, order: &[u32], id: WindowId) -> Option<Vec<StackChange>> {
    let Some(zone) = ctx.stacks.zone_of(id) else {
        return Some(Vec::new());
    };
    let mut sim = Sim::new(ctx.stacks);
    sim.apply(StackChange::Remove { window: id })?;
    if ctx.settings.fill_gaps && sim.work.depth(zone) == 0 {
        if let Some(gap) = order.iter().position(|&z| z == zone) {
            fill_gaps(&mut sim, order, gap)?;
        }
    }
    Some(sim.changes)
}

/// Refill the empty zones from position `gap` on with windows from later zones, in
/// order. A single window moves up whole; a stack gives up its first hidden window.
fn fill_gaps(sim: &mut Sim, order: &[u32], gap: usize) -> Option<()> {
    for (p, &donor) in order.iter().enumerate().skip(gap + 1) {
        loop {
            let depth = sim.work.depth(donor);
            if depth == 0 {
                break;
            }
            let Some(slot) = order[gap..p]
                .iter()
                .copied()
                .find(|&z| sim.work.depth(z) == 0)
            else {
                break;
            };
            let window = if depth == 1 {
                sim.work.visible(donor)?
            } else {
                sim.work.hidden(donor).next()?
            };
            sim.apply(StackChange::Place { window, zone: slot })?;
        }
    }
    Some(())
}

fn on_focused(ctx: &ArrangeContext<'_>, id: WindowId) -> Option<Vec<StackChange>> {
    if ctx.stacks.contains(id) && !ctx.stacks.is_visible(id) {
        Some(vec![StackChange::Raise { window: id }])
    } else {
        Some(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Rect;
    use crate::zone::Zone;

    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0 >> 33
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    /// `n` equal columns; zone ids 1..=n left to right.
    fn cols(n: usize) -> Layout {
        let w = 1.0 / n as f64;
        let rects: Vec<Rect> = (0..n)
            .map(|i| Rect::new(i as f64 * w, 0.0, w, 1.0))
            .collect();
        Layout::new("cols", "Columns", &rects)
    }

    /// 2x2 grid whose ids run against reading order: 40, 30, 20, 10.
    fn scrambled_grid() -> Layout {
        let mut l = crate::zone::templates().remove(2);
        for (zone, id) in l.zones.iter_mut().zip([40, 30, 20, 10]) {
            zone.id = id;
        }
        l
    }

    fn settings() -> ArrangeSettings {
        ArrangeSettings {
            enabled: true,
            ..ArrangeSettings::default()
        }
    }

    fn main_first() -> ArrangeSettings {
        ArrangeSettings {
            policy: ArrangePolicy::MainFirst,
            ..settings()
        }
    }

    fn filling() -> ArrangeSettings {
        ArrangeSettings {
            fill_gaps: true,
            ..settings()
        }
    }

    fn open(id: WindowId) -> ArrangeEvent {
        ArrangeEvent::WindowOpened {
            id,
            preferred_zone: None,
        }
    }

    fn open_in(id: WindowId, n: u32) -> ArrangeEvent {
        ArrangeEvent::WindowOpened {
            id,
            preferred_zone: Some(n),
        }
    }

    fn close(id: WindowId) -> ArrangeEvent {
        ArrangeEvent::WindowClosed { id }
    }

    fn run(
        layout: &Layout,
        stacks: &Stacks,
        focused: Option<WindowId>,
        settings: ArrangeSettings,
        event: ArrangeEvent,
    ) -> Plan {
        plan(
            &ArrangeContext {
                layout,
                stacks,
                focused,
                settings,
            },
            event,
        )
    }

    /// Plan, apply to `stacks`, check the invariants and return the plan.
    fn step(
        layout: &Layout,
        stacks: &mut Stacks,
        focused: Option<WindowId>,
        settings: ArrangeSettings,
        event: ArrangeEvent,
    ) -> Plan {
        let p = run(layout, stacks, focused, settings, event);
        p.apply(stacks).expect("a planned change applies");
        assert_eq!(stacks.check_invariants(), Ok(()));
        p
    }

    fn st(items: &[(WindowId, u32)]) -> Stacks {
        Stacks::from_assignments(items.iter().copied())
    }

    fn placed(p: &Plan) -> Vec<(WindowId, u32)> {
        p.placements.iter().map(|x| (x.window, x.zone)).collect()
    }

    // ----- settings -----

    #[test]
    fn auto_arrange_is_off_by_default() {
        let d = ArrangeSettings::default();
        assert!(!d.enabled);
        assert_eq!(d.policy, ArrangePolicy::NextFree);
        assert!(!d.fill_gaps);
        assert!(d.stack_when_full);
    }

    #[test]
    fn settings_load_from_partial_or_empty_json() {
        let d: ArrangeSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(d, ArrangeSettings::default());
        let s: ArrangeSettings =
            serde_json::from_str(r#"{"enabled":true,"policy":"main_first"}"#).unwrap();
        assert!(s.enabled && s.stack_when_full && !s.fill_gaps);
        assert_eq!(s.policy, ArrangePolicy::MainFirst);
        let back: ArrangeSettings =
            serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
        assert!(serde_json::from_str::<ArrangeSettings>(r#"{"policy":"sideways"}"#).is_err());
    }

    #[test]
    fn a_disabled_planner_plans_nothing() {
        let l = cols(3);
        let s = Stacks::new();
        let off = ArrangeSettings::default();
        assert!(run(&l, &s, None, off, open(1)).is_empty());
        let s = st(&[(1, 1)]);
        assert!(run(&l, &s, None, off, close(1)).is_empty());
        assert!(run(
            &l,
            &st(&[(1, 1), (2, 1)]),
            None,
            off,
            ArrangeEvent::Focused { id: 1 }
        )
        .is_empty());
    }

    // ----- degenerate layouts -----

    #[test]
    fn a_layout_without_zones_never_places_anything() {
        let empty = Layout {
            id: "e".into(),
            name: "e".into(),
            zones: vec![],
        };
        let s = Stacks::new();
        assert!(run(&empty, &s, None, settings(), open(1)).is_empty());
        assert!(run(&empty, &s, None, settings(), open_in(1, 1)).is_empty());
        assert!(run(&empty, &s, None, main_first(), open(1)).is_empty());
        assert!(run(&empty, &s, None, settings(), close(1)).is_empty());
    }

    #[test]
    fn with_one_zone_the_first_window_fills_it_and_the_rest_stack() {
        let l = cols(1);
        let mut s = Stacks::new();
        let p = step(&l, &mut s, None, settings(), open(1));
        assert_eq!(placed(&p), vec![(1, 1)]);
        assert!(p.raise.is_empty(), "a lone window needs no raising");
        let p = step(&l, &mut s, Some(1), settings(), open(2));
        assert_eq!(placed(&p), vec![(2, 1)]);
        assert_eq!(p.raise, vec![2]);
        assert_eq!(s.members(1), &[1, 2]);
        assert_eq!(s.visible(1), Some(2));
        let p = step(&l, &mut s, Some(2), settings(), close(2));
        assert!(p.placements.is_empty());
        assert_eq!(p.raise, vec![1], "the tab underneath shows again");
        let p = step(&l, &mut s, None, filling(), close(1));
        assert!(p.placements.is_empty() && s.is_empty());
    }

    #[test]
    fn main_first_with_one_zone_behaves_like_next_free() {
        let l = cols(1);
        let mut s = Stacks::new();
        step(&l, &mut s, None, main_first(), open(1));
        let p = step(&l, &mut s, Some(1), main_first(), open(2));
        assert_eq!(placed(&p), vec![(2, 1)]);
        assert_eq!(s.members(1), &[1, 2]);
    }

    #[test]
    fn a_layout_with_non_finite_zones_still_plans_deterministically() {
        let l = Layout {
            id: "n".into(),
            name: "n".into(),
            zones: vec![
                Zone {
                    id: 1,
                    rect: Rect::new(f64::NAN, 0.0, 0.5, 1.0),
                },
                Zone {
                    id: 2,
                    rect: Rect::new(0.0, f64::INFINITY, 0.5, 1.0),
                },
                Zone {
                    id: 3,
                    rect: Rect::new(0.5, 0.0, 0.5, 1.0),
                },
            ],
        };
        let mut s = Stacks::new();
        let mut seen = Vec::new();
        for w in 1..=5 {
            let p = step(&l, &mut s, None, settings(), open(w));
            seen.push(placed(&p));
        }
        let mut s2 = Stacks::new();
        for (i, w) in (1..=5).enumerate() {
            let p = step(&l, &mut s2, None, settings(), open(w));
            assert_eq!(placed(&p), seen[i]);
        }
        assert_eq!(s.len(), 5);
    }

    // ----- opening: next free -----

    #[test]
    fn windows_fill_zones_in_reading_order_not_id_order() {
        let l = scrambled_grid();
        let mut s = Stacks::new();
        let zones: Vec<u32> = (1..=4)
            .map(|w| step(&l, &mut s, None, settings(), open(w)).placements[0].zone)
            .collect();
        assert_eq!(zones, vec![40, 30, 20, 10]);
    }

    #[test]
    fn the_first_window_goes_to_the_first_zone() {
        let l = cols(3);
        let p = run(&l, &Stacks::new(), None, settings(), open(7));
        assert_eq!(
            p,
            Plan {
                placements: vec![Placement { window: 7, zone: 1 }],
                stack_changes: vec![StackChange::Place { window: 7, zone: 1 }],
                raise: vec![],
            }
        );
    }

    #[test]
    fn a_new_window_takes_the_first_free_zone_even_after_a_hole() {
        let l = cols(4);
        // Zones 1 and 3 are taken; 2 is the first free one.
        let s = st(&[(1, 1), (2, 3)]);
        let p = run(&l, &s, None, settings(), open(9));
        assert_eq!(placed(&p), vec![(9, 2)]);
        assert_eq!(p.stack_changes.len(), 1, "nothing else moves");
    }

    #[test]
    fn opening_a_window_that_is_already_arranged_does_nothing() {
        let l = cols(3);
        let s = st(&[(1, 2)]);
        assert!(run(&l, &s, None, settings(), open(1)).is_empty());
        assert!(run(&l, &s, None, main_first(), open_in(1, 3)).is_empty());
    }

    // ----- opening: preferred zone -----

    #[test]
    fn a_free_preferred_zone_wins_over_the_policy() {
        let l = cols(4);
        let s = st(&[(1, 1)]);
        let p = run(&l, &s, None, settings(), open_in(2, 3));
        assert_eq!(placed(&p), vec![(2, 3)]);
        let p = run(&l, &s, None, main_first(), open_in(2, 3));
        assert_eq!(
            placed(&p),
            vec![(2, 3)],
            "no shifting for an explicit preference"
        );
        assert_eq!(p.stack_changes.len(), 1);
    }

    #[test]
    fn the_preferred_zone_is_a_reading_order_number_not_an_id() {
        let l = scrambled_grid();
        let p = run(&l, &Stacks::new(), None, settings(), open_in(1, 2));
        assert_eq!(
            placed(&p),
            vec![(1, 30)],
            "number 2 is the second zone in reading order"
        );
        let p = run(&l, &Stacks::new(), None, settings(), open_in(1, 4));
        assert_eq!(placed(&p), vec![(1, 10)]);
    }

    #[test]
    fn an_occupied_preferred_zone_falls_back_to_a_free_zone() {
        let l = cols(3);
        let s = st(&[(1, 2)]);
        let p = run(&l, &s, None, settings(), open_in(2, 2));
        assert_eq!(placed(&p), vec![(2, 1)]);
    }

    #[test]
    fn with_everything_taken_the_preferred_zone_gets_the_stack() {
        let l = cols(3);
        let s = st(&[(1, 1), (2, 2), (3, 3)]);
        let p = run(&l, &s, Some(1), settings(), open_in(4, 3));
        assert_eq!(placed(&p), vec![(4, 3)]);
        assert_eq!(p.raise, vec![4]);
    }

    #[test]
    fn a_preferred_zone_number_outside_the_layout_is_ignored() {
        let l = cols(3);
        let s = Stacks::new();
        for n in [0, 4, 99, u32::MAX] {
            let p = run(&l, &s, None, settings(), open_in(1, n));
            assert_eq!(placed(&p), vec![(1, 1)], "preference {n}");
        }
    }

    // ----- opening: main first -----

    #[test]
    fn main_first_gives_the_new_window_the_first_zone_and_shifts_the_rest() {
        let l = cols(3);
        let mut s = Stacks::new();
        let p = step(&l, &mut s, None, main_first(), open(1));
        assert_eq!(placed(&p), vec![(1, 1)]);
        let p = step(&l, &mut s, None, main_first(), open(2));
        assert_eq!(placed(&p), vec![(2, 1), (1, 2)]);
        let p = step(&l, &mut s, None, main_first(), open(3));
        assert_eq!(placed(&p), vec![(3, 1), (2, 2), (1, 3)]);
        assert_eq!(
            p.stack_changes,
            vec![
                StackChange::MoveStack { from: 2, to: 3 },
                StackChange::MoveStack { from: 1, to: 2 },
                StackChange::Place { window: 3, zone: 1 },
            ]
        );
        assert_eq!(s.assignments(), vec![(3, 1), (2, 2), (1, 3)]);
    }

    #[test]
    fn main_first_does_not_move_windows_beyond_the_first_free_zone() {
        let l = cols(4);
        // Zone 1 and 3 are taken; the first free zone is 2.
        let s = st(&[(1, 1), (2, 3)]);
        let p = run(&l, &s, None, main_first(), open(9));
        assert_eq!(placed(&p), vec![(9, 1), (1, 2)]);
        let mut after = s.clone();
        p.apply(&mut after).unwrap();
        assert_eq!(after.zone_of(2), Some(3), "window 2 stays put");
    }

    #[test]
    fn main_first_with_a_free_main_zone_moves_nobody() {
        let l = cols(3);
        let s = st(&[(1, 2), (2, 3)]);
        let p = run(&l, &s, None, main_first(), open(9));
        assert_eq!(placed(&p), vec![(9, 1)]);
        assert_eq!(p.stack_changes.len(), 1);
    }

    #[test]
    fn main_first_moves_a_stack_as_one_unit() {
        let l = cols(3);
        let s = st(&[(1, 1), (2, 2), (3, 2)]);
        let p = run(&l, &s, None, main_first(), open(9));
        assert_eq!(placed(&p), vec![(9, 1), (1, 2), (2, 3), (3, 3)]);
        let mut after = s;
        p.apply(&mut after).unwrap();
        assert_eq!(
            after.visible(3),
            Some(3),
            "the visible tab travels with its stack"
        );
        assert!(p.raise.is_empty(), "no window changed visibility");
    }

    #[test]
    fn main_first_with_everything_taken_stacks_like_next_free() {
        let l = cols(2);
        let s = st(&[(1, 1), (2, 2)]);
        let p = run(&l, &s, Some(1), main_first(), open(3));
        assert_eq!(placed(&p), vec![(3, 1)]);
        assert_eq!(
            p.stack_changes,
            vec![StackChange::Place { window: 3, zone: 1 }]
        );
    }

    // ----- opening: everything taken -----

    #[test]
    fn a_new_window_joins_the_stack_of_the_focused_zone() {
        let l = cols(3);
        let s = st(&[(1, 1), (2, 2), (3, 3)]);
        let p = run(&l, &s, Some(2), settings(), open(4));
        assert_eq!(placed(&p), vec![(4, 2)]);
        assert_eq!(p.raise, vec![4]);
        assert_eq!(
            p.stack_changes,
            vec![StackChange::Place { window: 4, zone: 2 }]
        );
    }

    #[test]
    fn without_a_usable_focus_the_last_zone_in_reading_order_gets_the_stack() {
        let l = scrambled_grid(); // last in reading order is id 10
        let s = st(&[(1, 40), (2, 30), (3, 20), (4, 10)]);
        for focused in [None, Some(99)] {
            let p = run(&l, &s, focused, settings(), open(5));
            assert_eq!(placed(&p), vec![(5, 10)], "focused {focused:?}");
        }
    }

    #[test]
    fn a_focus_in_a_zone_outside_the_layout_is_not_used() {
        let l = cols(2);
        let s = st(&[(1, 1), (2, 2), (3, 77)]);
        let p = run(&l, &s, Some(3), settings(), open(4));
        assert_eq!(placed(&p), vec![(4, 2)], "falls back to the last zone");
    }

    #[test]
    fn stacking_can_be_turned_off() {
        let l = cols(2);
        let s = st(&[(1, 1), (2, 2)]);
        let off = ArrangeSettings {
            stack_when_full: false,
            ..settings()
        };
        assert!(run(&l, &s, Some(1), off, open(3)).is_empty());
        // A free zone is still used.
        let one = st(&[(1, 1)]);
        assert_eq!(placed(&run(&l, &one, None, off, open(3))), vec![(3, 2)]);
    }

    #[test]
    fn a_full_stack_hands_the_new_window_to_the_next_zone_with_room() {
        let l = cols(2);
        let mut items: Vec<(WindowId, u32)> = (0..MAX_STACK_DEPTH as u64).map(|w| (w, 1)).collect();
        items.push((500, 2));
        let s = st(&items);
        let p = run(&l, &s, Some(0), settings(), open(900));
        assert_eq!(
            placed(&p),
            vec![(900, 2)],
            "zone 1 is full, zone 2 has room"
        );
    }

    #[test]
    fn when_no_stack_has_room_the_window_is_left_alone() {
        let l = cols(1);
        let items: Vec<(WindowId, u32)> = (0..MAX_STACK_DEPTH as u64).map(|w| (w, 1)).collect();
        let s = st(&items);
        assert!(run(&l, &s, Some(0), settings(), open(900)).is_empty());
    }

    #[test]
    fn at_the_global_window_limit_a_new_window_is_left_alone() {
        let l = cols(64);
        let items: Vec<(WindowId, u32)> = (0..crate::stack::MAX_TRACKED_WINDOWS as u64)
            .map(|w| (w, (w / 16) as u32 + 1))
            .collect();
        let s = st(&items);
        assert_eq!(s.len(), crate::stack::MAX_TRACKED_WINDOWS);
        assert!(run(&l, &s, None, settings(), open(100_000)).is_empty());
    }

    #[test]
    fn a_layout_with_duplicate_zone_ids_never_breaks_the_occupancy() {
        // Layout::validate refuses this, but the planner must survive it anyway.
        let l = Layout {
            id: "d".into(),
            name: "d".into(),
            zones: vec![
                Zone {
                    id: 1,
                    rect: Rect::new(0.0, 0.0, 0.5, 1.0),
                },
                Zone {
                    id: 1,
                    rect: Rect::new(0.5, 0.0, 0.25, 1.0),
                },
                Zone {
                    id: 2,
                    rect: Rect::new(0.75, 0.0, 0.25, 1.0),
                },
            ],
        };
        for settings in [settings(), main_first(), filling()] {
            let mut s = Stacks::new();
            for w in 0..6 {
                step(&l, &mut s, Some(0), settings, open(w));
            }
            for w in 0..6 {
                step(&l, &mut s, Some(0), settings, close(w));
            }
            assert!(s.is_empty());
        }
    }

    #[test]
    fn a_layout_at_the_zone_limit_fills_in_order_and_then_stacks() {
        let l = cols(64);
        let mut s = Stacks::new();
        for w in 0..64u64 {
            let p = step(&l, &mut s, None, settings(), open(w));
            assert_eq!(placed(&p), vec![(w, w as u32 + 1)]);
        }
        let p = step(&l, &mut s, Some(10), settings(), open(100));
        assert_eq!(placed(&p), vec![(100, 11)], "stacks where the focus is");
        assert_eq!(s.len(), 65);
    }

    #[test]
    fn windows_in_zones_outside_the_layout_do_not_count_as_occupying_it() {
        let l = cols(2);
        let s = st(&[(1, 77)]);
        let p = run(&l, &s, None, settings(), open(2));
        assert_eq!(placed(&p), vec![(2, 1)]);
    }

    // ----- closing -----

    #[test]
    fn closing_an_unknown_window_does_nothing() {
        let l = cols(3);
        let s = st(&[(1, 1)]);
        assert!(run(&l, &s, None, filling(), close(99)).is_empty());
        assert!(run(&l, &Stacks::new(), None, filling(), close(1)).is_empty());
    }

    #[test]
    fn closing_leaves_a_hole_unless_gaps_are_filled() {
        let l = cols(4);
        let s = st(&[(1, 1), (2, 2), (3, 3), (4, 4)]);
        let p = run(&l, &s, None, settings(), close(2));
        assert_eq!(p.stack_changes, vec![StackChange::Remove { window: 2 }]);
        assert!(p.placements.is_empty() && p.raise.is_empty());
    }

    #[test]
    fn filling_gaps_moves_only_the_windows_after_the_gap() {
        let l = cols(4);
        let s = st(&[(1, 1), (2, 2), (3, 3), (4, 4)]);
        let p = run(&l, &s, None, filling(), close(2));
        assert_eq!(placed(&p), vec![(3, 2), (4, 3)]);
        let mut after = s;
        p.apply(&mut after).unwrap();
        assert_eq!(after.assignments(), vec![(1, 1), (3, 2), (4, 3)]);
    }

    #[test]
    fn closing_the_first_window_shifts_everything_up() {
        let l = cols(3);
        let s = st(&[(1, 1), (2, 2), (3, 3)]);
        let p = run(&l, &s, None, filling(), close(1));
        assert_eq!(placed(&p), vec![(2, 1), (3, 2)]);
    }

    #[test]
    fn closing_the_last_window_moves_nobody() {
        let l = cols(3);
        let s = st(&[(1, 1), (2, 2), (3, 3)]);
        let p = run(&l, &s, None, filling(), close(3));
        assert!(p.placements.is_empty());
        assert_eq!(p.stack_changes.len(), 1);
    }

    #[test]
    fn filling_skips_over_older_holes_and_leaves_windows_before_the_gap_alone() {
        let l = cols(5);
        // Zone 2 is an old hole; the window in zone 3 closes; zones 4 and 5 follow.
        let s = st(&[(1, 1), (3, 3), (4, 4), (5, 5)]);
        let p = run(&l, &s, None, filling(), close(3));
        assert_eq!(placed(&p), vec![(4, 3), (5, 4)]);
        let mut after = s;
        p.apply(&mut after).unwrap();
        assert_eq!(after.assignments(), vec![(1, 1), (4, 3), (5, 4)]);
        // The older hole at zone 2 is not touched: nothing before the gap moves.
    }

    #[test]
    fn a_window_that_would_not_move_further_up_stays() {
        let l = cols(4);
        // Gap at 2 after closing; window 4 in zone 4, zone 3 empty already.
        let s = st(&[(1, 1), (2, 2), (4, 4)]);
        let p = run(&l, &s, None, filling(), close(2));
        assert_eq!(placed(&p), vec![(4, 2)], "one move, straight into the gap");
    }

    #[test]
    fn closing_a_stacked_window_never_leaves_a_hole() {
        let l = cols(2);
        let s = st(&[(1, 1), (2, 2), (3, 2)]);
        // Window 3 is visible in zone 2. Closing it shows window 2; nothing moves.
        let p = run(&l, &s, None, filling(), close(3));
        assert!(p.placements.is_empty());
        assert_eq!(p.raise, vec![2]);
        // Closing the hidden one raises nothing.
        let p = run(&l, &s, None, filling(), close(2));
        assert!(p.placements.is_empty() && p.raise.is_empty());
    }

    #[test]
    fn filling_a_gap_unstacks_the_oldest_hidden_window_instead_of_moving_the_stack() {
        let l = cols(2);
        let s = st(&[(1, 1), (2, 2), (3, 2), (4, 2)]); // zone 2: tabs 2,3,4; 4 on top
        let p = run(&l, &s, None, filling(), close(1));
        assert_eq!(placed(&p), vec![(2, 1)]);
        assert!(
            p.raise.is_empty(),
            "the visible window of zone 2 did not change"
        );
        let mut after = s;
        p.apply(&mut after).unwrap();
        assert_eq!(after.members(1), &[2]);
        assert_eq!(after.members(2), &[3, 4]);
        assert_eq!(after.visible(2), Some(4));
    }

    #[test]
    fn filling_several_gaps_from_one_stack_empties_it_into_the_gaps_in_order() {
        let l = cols(3);
        // Zones 1 and 2 are empty after the close; zone 3 holds a stack of three.
        let s = st(&[(1, 1), (7, 3), (8, 3), (9, 3)]);
        let p = run(&l, &s, None, filling(), close(1));
        assert_eq!(placed(&p), vec![(7, 1), (8, 2)]);
        let mut after = s;
        p.apply(&mut after).unwrap();
        assert_eq!(after.assignments(), vec![(7, 1), (8, 2), (9, 3)]);
    }

    #[test]
    fn filling_works_with_main_first_too() {
        let l = cols(3);
        let s = st(&[(3, 1), (2, 2), (1, 3)]);
        let p = run(
            &l,
            &s,
            None,
            ArrangeSettings {
                fill_gaps: true,
                ..main_first()
            },
            close(3),
        );
        assert_eq!(placed(&p), vec![(2, 1), (1, 2)]);
    }

    #[test]
    fn closing_a_window_in_a_zone_outside_the_layout_just_forgets_it() {
        let l = cols(3);
        let s = st(&[(1, 1), (2, 77), (3, 2)]);
        let p = run(&l, &s, None, filling(), close(2));
        assert!(p.placements.is_empty());
        assert_eq!(p.stack_changes, vec![StackChange::Remove { window: 2 }]);
    }

    // ----- focus -----

    #[test]
    fn focusing_a_hidden_tab_makes_it_the_visible_one() {
        let l = cols(2);
        let s = st(&[(1, 1), (2, 1), (3, 2)]);
        assert_eq!(s.visible(1), Some(2));
        let p = run(&l, &s, None, settings(), ArrangeEvent::Focused { id: 1 });
        assert_eq!(p.stack_changes, vec![StackChange::Raise { window: 1 }]);
        assert!(p.placements.is_empty());
        let mut after = s;
        p.apply(&mut after).unwrap();
        assert_eq!(after.visible(1), Some(1));
    }

    #[test]
    fn focusing_a_visible_or_unknown_window_does_nothing() {
        let l = cols(2);
        let s = st(&[(1, 1), (2, 1), (3, 2)]);
        assert!(run(&l, &s, None, settings(), ArrangeEvent::Focused { id: 2 }).is_empty());
        assert!(run(&l, &s, None, settings(), ArrangeEvent::Focused { id: 3 }).is_empty());
        assert!(run(&l, &s, None, settings(), ArrangeEvent::Focused { id: 99 }).is_empty());
    }

    // ----- plan shape -----

    #[test]
    fn placements_are_ordered_by_where_they_go() {
        let l = scrambled_grid();
        let s = st(&[(1, 40), (2, 30), (3, 20), (4, 10)]);
        let changes = vec![
            StackChange::Remove { window: 1 },
            StackChange::Remove { window: 4 },
            StackChange::Place {
                window: 4,
                zone: 40,
            },
            StackChange::Place {
                window: 1,
                zone: 10,
            },
        ];
        let p = Plan::from_changes(&l, &s, changes).unwrap();
        assert_eq!(
            placed(&p),
            vec![(4, 40), (1, 10)],
            "by reading position, not window id"
        );
    }

    #[test]
    fn from_changes_rejects_edits_that_do_not_fit() {
        let l = cols(2);
        let s = st(&[(1, 1)]);
        assert!(Plan::from_changes(&l, &s, vec![StackChange::Remove { window: 5 }]).is_none());
        assert_eq!(Plan::from_changes(&l, &s, vec![]), Some(Plan::default()));
        assert!(Plan::default().is_empty());
    }

    #[test]
    fn apply_is_all_or_nothing() {
        let bad = Plan {
            placements: vec![],
            stack_changes: vec![
                StackChange::Place { window: 9, zone: 1 },
                StackChange::Remove { window: 55 },
            ],
            raise: vec![],
        };
        let mut s = st(&[(1, 1)]);
        let before = s.clone();
        assert!(bad.apply(&mut s).is_err());
        assert_eq!(s, before);
    }

    #[test]
    fn the_same_input_always_gives_the_same_plan() {
        let l = cols(4);
        let s = st(&[(1, 1), (2, 3), (3, 3)]);
        for settings in [settings(), main_first(), filling()] {
            for event in [
                open(9),
                open_in(9, 3),
                close(2),
                close(1),
                ArrangeEvent::Focused { id: 2 },
            ] {
                let a = run(&l, &s, Some(3), settings, event);
                let b = run(&l, &s, Some(3), settings, event);
                assert_eq!(a, b);
            }
        }
    }

    #[test]
    fn the_order_zones_were_filled_in_does_not_change_the_plan() {
        let l = cols(4);
        let a = st(&[(1, 1), (2, 2), (3, 4)]);
        let b = st(&[(3, 4), (1, 1), (2, 2)]);
        assert_eq!(a, b);
        for event in [open(9), close(2)] {
            assert_eq!(
                run(&l, &a, None, filling(), event),
                run(&l, &b, None, filling(), event)
            );
        }
    }

    #[test]
    fn planning_does_not_touch_its_input() {
        let l = cols(3);
        let s = st(&[(1, 1), (2, 2)]);
        let before = s.clone();
        let _ = run(&l, &s, None, main_first(), open(3));
        let _ = run(&l, &s, None, filling(), close(1));
        assert_eq!(s, before);
    }

    #[test]
    fn plan_apply_reproduces_the_planned_occupancy() {
        let l = cols(3);
        let mut s = st(&[(1, 1), (2, 2)]);
        let p = run(&l, &s, None, main_first(), open(3));
        p.apply(&mut s).unwrap();
        for pl in &p.placements {
            assert_eq!(s.zone_of(pl.window), Some(pl.zone));
        }
        assert_eq!(s.assignments(), vec![(3, 1), (1, 2), (2, 3)]);
    }

    // ----- stress -----

    fn stress(seed: u64, settings: ArrangeSettings, with_preferences: bool) {
        let mut rng = Lcg(seed * 7919 + 13);
        let zone_count = 1 + rng.below(5) as usize;
        let layout = cols(zone_count);
        let order = reading_order(&layout);
        let mut s = Stacks::new();
        let mut focused: Option<WindowId> = None;
        for _ in 0..200 {
            let w = rng.below(14);
            let event = match rng.below(5) {
                0 | 1 => ArrangeEvent::WindowOpened {
                    id: w,
                    preferred_zone: (with_preferences && rng.below(3) == 0)
                        .then(|| rng.below(7) as u32),
                },
                2 | 3 => ArrangeEvent::WindowClosed { id: w },
                _ => ArrangeEvent::Focused { id: w },
            };
            let before = s.clone();
            let p = run(&layout, &before, focused, settings, event);

            // The recorded edits replay to the same value as `apply`.
            let mut replay = before.clone();
            for c in &p.stack_changes {
                replay.apply(c).expect("planned edits are valid");
            }
            let mut applied = before.clone();
            p.apply(&mut applied).unwrap();
            assert_eq!(replay, applied);
            assert_eq!(applied.check_invariants(), Ok(()));

            // Placements are exactly the windows whose zone changed, all inside the layout.
            let mut expected: Vec<WindowId> = applied
                .assignments()
                .into_iter()
                .filter(|&(win, zone)| before.zone_of(win) != Some(zone))
                .map(|(win, _)| win)
                .collect();
            let mut got: Vec<WindowId> = p.placements.iter().map(|x| x.window).collect();
            expected.sort_unstable();
            got.sort_unstable();
            assert_eq!(expected, got, "seed {seed}");
            for pl in &p.placements {
                assert!(order.contains(&pl.zone));
                assert_eq!(applied.zone_of(pl.window), Some(pl.zone));
            }
            // Raised windows end up on top of their zone, listed once, by zone id.
            for &r in &p.raise {
                assert!(applied.is_visible(r));
            }
            let raised_zones: Vec<u32> = p
                .raise
                .iter()
                .map(|&r| applied.zone_of(r).unwrap())
                .collect();
            assert!(raised_zones.windows(2).all(|w| w[0] < w[1]), "seed {seed}");

            match event {
                ArrangeEvent::WindowOpened { id, preferred_zone } => {
                    if before.contains(id) {
                        assert!(p.is_empty());
                    } else if let Some(zone) = before_first_free(&before, &order) {
                        // A free zone exists: the window is placed, in a free zone
                        // under the next-free policy.
                        assert!(applied.contains(id), "seed {seed}");
                        if settings.policy == ArrangePolicy::NextFree && preferred_zone.is_none() {
                            assert_eq!(applied.zone_of(id), Some(zone));
                            assert_eq!(p.placements.len(), 1);
                        }
                        if settings.policy == ArrangePolicy::MainFirst && preferred_zone.is_none() {
                            assert_eq!(applied.zone_of(id), Some(order[0]));
                        }
                    }
                }
                ArrangeEvent::WindowClosed { id } => assert!(!applied.contains(id)),
                ArrangeEvent::Focused { id } => {
                    if before.contains(id) {
                        assert!(applied.is_visible(id));
                    }
                }
            }
            if settings.fill_gaps && !with_preferences {
                // The occupied zones are always the first ones in reading order.
                let occupied: Vec<bool> = order.iter().map(|&z| applied.depth(z) > 0).collect();
                let first_empty = occupied.iter().position(|&o| !o).unwrap_or(occupied.len());
                assert!(
                    occupied[first_empty..].iter().all(|&o| !o),
                    "seed {seed}: {occupied:?}"
                );
            }
            s = applied;
            focused = match event {
                ArrangeEvent::Focused { id } if s.contains(id) => Some(id),
                _ => focused.filter(|&f| s.contains(f)),
            };
        }
    }

    fn before_first_free(stacks: &Stacks, order: &[u32]) -> Option<u32> {
        order.iter().copied().find(|&z| stacks.depth(z) == 0)
    }

    #[test]
    fn random_events_keep_every_invariant_for_every_setting() {
        for seed in 0..16u64 {
            for policy in [ArrangePolicy::NextFree, ArrangePolicy::MainFirst] {
                for fill_gaps in [false, true] {
                    for stack_when_full in [false, true] {
                        let st = ArrangeSettings {
                            enabled: true,
                            policy,
                            fill_gaps,
                            stack_when_full,
                        };
                        stress(seed, st, false);
                        stress(seed, st, true);
                    }
                }
            }
        }
    }
}
