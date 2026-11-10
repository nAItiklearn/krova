//! Swapping windows between zones, and finding the zone next to a zone.
//!
//! * [`neighbour`] answers "which zone is to the left / right / above / below this
//!   one" from the zone rectangles alone, so it works for any drawn layout.
//! * [`swap_windows`], [`swap_with_neighbour`] and [`drop_on_zone`] turn a swap
//!   gesture into a [`Plan`] the engine can apply, like the auto-arrange planner.
//!
//! # Neighbour rules
//!
//! A zone is a candidate in a direction when it reaches strictly further in that
//! direction than the starting zone, with both its centre and its far edge. (So the
//! main zone of a main + side layout has nothing "below" it, even though the lower
//! side zone's centre is lower.) Candidates are ranked by
//!
//! 1. overlap on the perpendicular axis: zones that share some of the starting
//!    zone's height (for left / right) or width (for up / down) come first, and the
//!    larger the shared part the better;
//! 2. distance along the axis: the gap between the facing edges, zero when they
//!    touch or overlap;
//! 3. how far the centres are apart on the perpendicular axis;
//! 4. reading order ([`crate::registry::reading_order`]), so a perfect tie always
//!    resolves to the zone the keyboard shortcuts would number first.
//!
//! Candidates that share no perpendicular span are only used when nothing
//! overlaps; they are ranked by axis distance, then perpendicular distance, then
//! reading order. There is no wrap-around: at the edge of the layout the answer is
//! `None`.

use serde::{Deserialize, Serialize};

use crate::arrange::Plan;
use crate::geometry::Rect;
use crate::registry::reading_order;
use crate::stack::{StackChange, Stacks, WindowId};
use crate::zone::Layout;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

impl Direction {
    pub const ALL: [Direction; 4] = [
        Direction::Left,
        Direction::Right,
        Direction::Up,
        Direction::Down,
    ];

    pub fn opposite(self) -> Direction {
        match self {
            Direction::Left => Direction::Right,
            Direction::Right => Direction::Left,
            Direction::Up => Direction::Down,
            Direction::Down => Direction::Up,
        }
    }
}

/// Tolerance for "strictly beyond" and "overlaps".
const EPS: f64 = 1e-9;
/// Ranking values are compared after rounding to this many steps per unit, so
/// float noise (1/3 versus 0.333333) never decides a ranking.
const STEPS: f64 = 1e6;

fn steps(v: f64) -> i64 {
    // Values are bounded by a few units; the saturating cast guards the rest.
    (v * STEPS).round() as i64
}

/// An interval on one axis.
#[derive(Clone, Copy)]
struct Span {
    lo: f64,
    hi: f64,
}

impl Span {
    fn center(self) -> f64 {
        (self.lo + self.hi) / 2.0
    }

    /// Mirror around zero, so "up" and "left" can be treated as "forward".
    fn flipped(self) -> Span {
        Span {
            lo: -self.hi,
            hi: -self.lo,
        }
    }

    /// Length of the shared part; negative (minus the gap) when they do not touch.
    fn overlap(self, other: Span) -> f64 {
        self.hi.min(other.hi) - self.lo.max(other.lo)
    }
}

fn finite(r: &Rect) -> bool {
    [r.x, r.y, r.w, r.h].iter().all(|v| v.is_finite())
}

/// The rectangle as (span along the direction of travel, span across it), with the
/// direction of travel made to point towards larger values.
fn oriented(r: &Rect, dir: Direction) -> (Span, Span) {
    let x = Span {
        lo: r.x,
        hi: r.x + r.w,
    };
    let y = Span {
        lo: r.y,
        hi: r.y + r.h,
    };
    match dir {
        Direction::Right => (x, y),
        Direction::Left => (x.flipped(), y),
        Direction::Down => (y, x),
        Direction::Up => (y.flipped(), x),
    }
}

/// The zone next to `from` in direction `dir`, or `None` at the edge of the layout
/// (also for an unknown zone, an empty layout and a layout of one zone).
pub fn neighbour(layout: &Layout, from: u32, dir: Direction) -> Option<u32> {
    let start = layout.zones.iter().find(|z| z.id == from)?;
    if !finite(&start.rect) {
        return None;
    }
    let (from_along, from_across) = oriented(&start.rect, dir);
    let order = reading_order(layout);

    layout
        .zones
        .iter()
        .filter(|z| z.id != from && finite(&z.rect))
        .filter_map(|z| {
            let (along, across) = oriented(&z.rect, dir);
            // Beyond the start in this direction: a further centre and a further far
            // edge. The far edge keeps a full-height main zone from having a zone
            // "below" it just because that zone's centre is lower.
            if along.center() <= from_along.center() + EPS || along.hi <= from_along.hi + EPS {
                return None;
            }
            let gap = (along.lo - from_along.hi).max(0.0);
            let shared = from_across.overlap(across);
            let off = (across.center() - from_across.center()).abs();
            let rank = order
                .iter()
                .position(|&id| id == z.id)
                .unwrap_or(usize::MAX);
            let key = if shared > EPS {
                (0u8, -steps(shared), steps(gap), steps(off), rank)
            } else {
                (1u8, steps(gap), steps(-shared), steps(off), rank)
            };
            Some((key, z.id))
        })
        .min_by_key(|&(key, _)| key)
        .map(|(_, id)| id)
}

/// Make two arranged windows trade places. Empty plan if either is unknown or they
/// are the same window. See [`Stacks::swap`] for what "trade places" means with
/// stacks.
pub fn swap_windows(layout: &Layout, stacks: &Stacks, a: WindowId, b: WindowId) -> Plan {
    if a == b || !stacks.contains(a) || !stacks.contains(b) {
        return Plan::default();
    }
    Plan::from_changes(layout, stacks, vec![StackChange::Swap { a, b }]).unwrap_or_default()
}

/// `<mods>+Shift+arrow`: swap `window` with the visible window of the neighbouring
/// zone in `dir`, or move it there when that zone is empty. Empty plan if the window
/// is not arranged or there is no neighbour.
pub fn swap_with_neighbour(
    layout: &Layout,
    stacks: &Stacks,
    window: WindowId,
    dir: Direction,
) -> Plan {
    let Some(zone) = stacks.zone_of(window) else {
        return Plan::default();
    };
    match neighbour(layout, zone, dir) {
        Some(target) => swap_into(layout, stacks, window, target),
        None => Plan::default(),
    }
}

/// Drop a window on a zone with the swap modifier held: swap with the zone's visible
/// window, or move there when the zone is empty. Empty plan if the window is not
/// arranged (an outside window being dropped is an ordinary snap, not a swap), the
/// zone is not in the layout, or the window is already in that zone.
pub fn drop_on_zone(layout: &Layout, stacks: &Stacks, window: WindowId, target: u32) -> Plan {
    if !layout.zones.iter().any(|z| z.id == target) || stacks.zone_of(window).is_none() {
        return Plan::default();
    }
    swap_into(layout, stacks, window, target)
}

fn swap_into(layout: &Layout, stacks: &Stacks, window: WindowId, target: u32) -> Plan {
    if stacks.zone_of(window) == Some(target) {
        return Plan::default();
    }
    let change = match stacks.visible(target) {
        Some(other) => StackChange::Swap {
            a: window,
            b: other,
        },
        None => StackChange::Place {
            window,
            zone: target,
        },
    };
    Plan::from_changes(layout, stacks, vec![change]).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arrange::Placement;
    use crate::zone::{templates, Zone};

    fn cols(n: usize) -> Layout {
        let w = 1.0 / n as f64;
        let rects: Vec<Rect> = (0..n)
            .map(|i| Rect::new(i as f64 * w, 0.0, w, 1.0))
            .collect();
        Layout::new("cols", "Columns", &rects)
    }

    fn rows(n: usize) -> Layout {
        let h = 1.0 / n as f64;
        let rects: Vec<Rect> = (0..n)
            .map(|i| Rect::new(0.0, i as f64 * h, 1.0, h))
            .collect();
        Layout::new("rows", "Rows", &rects)
    }

    fn grid() -> Layout {
        templates().remove(2)
    }

    /// Main zone on the left (60 %), two zones stacked on the right.
    fn main_side() -> Layout {
        Layout::new(
            "ms",
            "ms",
            &[
                Rect::new(0.0, 0.0, 0.6, 1.0),
                Rect::new(0.6, 0.0, 0.4, 0.5),
                Rect::new(0.6, 0.5, 0.4, 0.5),
            ],
        )
    }

    fn st(items: &[(WindowId, u32)]) -> Stacks {
        Stacks::from_assignments(items.iter().copied())
    }

    fn placed(p: &Plan) -> Vec<(WindowId, u32)> {
        p.placements.iter().map(|x| (x.window, x.zone)).collect()
    }

    #[test]
    fn directions_have_opposites() {
        for d in Direction::ALL {
            assert_eq!(d.opposite().opposite(), d);
            assert_ne!(d.opposite(), d);
        }
        assert_eq!(Direction::ALL.len(), 4);
    }

    #[test]
    fn direction_serialises_as_a_lowercase_word() {
        assert_eq!(serde_json::to_string(&Direction::Left).unwrap(), "\"left\"");
        let d: Direction = serde_json::from_str("\"down\"").unwrap();
        assert_eq!(d, Direction::Down);
    }

    #[test]
    fn columns_have_left_and_right_neighbours_only() {
        let l = cols(3);
        assert_eq!(neighbour(&l, 1, Direction::Right), Some(2));
        assert_eq!(neighbour(&l, 2, Direction::Right), Some(3));
        assert_eq!(neighbour(&l, 3, Direction::Right), None, "no wrap-around");
        assert_eq!(neighbour(&l, 3, Direction::Left), Some(2));
        assert_eq!(neighbour(&l, 1, Direction::Left), None);
        for z in 1..=3 {
            assert_eq!(neighbour(&l, z, Direction::Up), None);
            assert_eq!(neighbour(&l, z, Direction::Down), None);
        }
    }

    #[test]
    fn rows_have_up_and_down_neighbours_only() {
        let l = rows(3);
        assert_eq!(neighbour(&l, 1, Direction::Down), Some(2));
        assert_eq!(neighbour(&l, 2, Direction::Down), Some(3));
        assert_eq!(neighbour(&l, 3, Direction::Down), None);
        assert_eq!(neighbour(&l, 2, Direction::Up), Some(1));
        assert_eq!(neighbour(&l, 1, Direction::Up), None);
        assert_eq!(neighbour(&l, 2, Direction::Left), None);
        assert_eq!(neighbour(&l, 2, Direction::Right), None);
    }

    #[test]
    fn a_grid_connects_each_zone_to_the_adjacent_ones() {
        let l = grid(); // 1 2 / 3 4
        let expect = [
            (1, Direction::Right, Some(2)),
            (1, Direction::Down, Some(3)),
            (1, Direction::Left, None),
            (1, Direction::Up, None),
            (2, Direction::Left, Some(1)),
            (2, Direction::Down, Some(4)),
            (3, Direction::Up, Some(1)),
            (3, Direction::Right, Some(4)),
            (4, Direction::Up, Some(2)),
            (4, Direction::Left, Some(3)),
            (4, Direction::Right, None),
            (4, Direction::Down, None),
        ];
        for (from, dir, to) in expect {
            assert_eq!(neighbour(&l, from, dir), to, "{from} {dir:?}");
        }
    }

    #[test]
    fn going_there_and_back_returns_to_the_start_in_regular_layouts() {
        for l in [cols(4), rows(4), grid()] {
            for z in &l.zones {
                for dir in Direction::ALL {
                    if let Some(n) = neighbour(&l, z.id, dir) {
                        assert_eq!(neighbour(&l, n, dir.opposite()), Some(z.id));
                    }
                }
            }
        }
    }

    #[test]
    fn main_side_prefers_the_zone_that_shares_the_most_height() {
        let l = main_side();
        // From the main zone both side zones share half the height: the first in
        // reading order (top) wins the tie.
        assert_eq!(neighbour(&l, 1, Direction::Right), Some(2));
        assert_eq!(neighbour(&l, 2, Direction::Left), Some(1));
        assert_eq!(neighbour(&l, 3, Direction::Left), Some(1));
        assert_eq!(neighbour(&l, 2, Direction::Down), Some(3));
        assert_eq!(neighbour(&l, 3, Direction::Up), Some(2));
    }

    #[test]
    fn a_full_height_zone_has_nothing_above_or_below_it() {
        let l = main_side();
        assert_eq!(neighbour(&l, 1, Direction::Down), None);
        assert_eq!(neighbour(&l, 1, Direction::Up), None);
        assert_eq!(neighbour(&l, 1, Direction::Left), None);
        assert_eq!(neighbour(&l, 2, Direction::Up), None);
        assert_eq!(neighbour(&l, 3, Direction::Down), None);
        assert_eq!(neighbour(&l, 2, Direction::Right), None);
        assert_eq!(neighbour(&l, 3, Direction::Right), None);
    }

    #[test]
    fn more_shared_height_beats_a_shorter_distance() {
        // From the left zone going right: the tall zone shares the full height but
        // sits 0.2 away; the small zone touches but shares only a sliver.
        let l = Layout::new(
            "o",
            "o",
            &[
                Rect::new(0.0, 0.0, 0.3, 1.0),
                Rect::new(0.3, 0.0, 0.1, 0.2),
                Rect::new(0.5, 0.0, 0.5, 1.0),
            ],
        );
        assert_eq!(neighbour(&l, 1, Direction::Right), Some(3));
    }

    #[test]
    fn equal_overlap_goes_to_the_nearer_zone() {
        let l = Layout::new(
            "o",
            "o",
            &[
                Rect::new(0.0, 0.0, 0.2, 1.0),
                Rect::new(0.6, 0.0, 0.2, 1.0),
                Rect::new(0.2, 0.0, 0.2, 1.0),
            ],
        );
        assert_eq!(neighbour(&l, 1, Direction::Right), Some(3));
        assert_eq!(neighbour(&l, 3, Direction::Right), Some(2));
    }

    #[test]
    fn a_perfect_tie_goes_to_the_first_zone_in_reading_order() {
        // Two zones mirror each other around the start; ids are deliberately reversed.
        let l = Layout {
            id: "t".into(),
            name: "t".into(),
            zones: vec![
                Zone {
                    id: 9,
                    rect: Rect::new(0.0, 0.0, 0.5, 1.0),
                },
                Zone {
                    id: 8,
                    rect: Rect::new(0.5, 0.5, 0.5, 0.5),
                },
                Zone {
                    id: 7,
                    rect: Rect::new(0.5, 0.0, 0.5, 0.5),
                },
            ],
        };
        // Reading order: 9 (0,0), 7 (0.5,0), 8 (0.5,0.5): zone 7 is first.
        assert_eq!(neighbour(&l, 9, Direction::Right), Some(7));
    }

    #[test]
    fn zones_with_no_shared_span_are_used_only_when_nothing_overlaps() {
        // Top-left start; the only zone to its right is in the bottom row.
        let l = Layout::new(
            "d",
            "d",
            &[Rect::new(0.0, 0.0, 0.5, 0.4), Rect::new(0.5, 0.6, 0.5, 0.4)],
        );
        assert_eq!(neighbour(&l, 1, Direction::Right), Some(2));
        assert_eq!(neighbour(&l, 2, Direction::Left), Some(1));
        // With an overlapping zone available it wins even though it is farther away.
        let l = Layout::new(
            "d",
            "d",
            &[
                Rect::new(0.0, 0.0, 0.3, 0.4),
                Rect::new(0.3, 0.6, 0.2, 0.4),
                Rect::new(0.7, 0.0, 0.3, 0.4),
            ],
        );
        assert_eq!(neighbour(&l, 1, Direction::Right), Some(3));
    }

    #[test]
    fn among_zones_with_no_shared_span_the_closer_one_wins() {
        let l = Layout::new(
            "d",
            "d",
            &[
                Rect::new(0.0, 0.0, 0.2, 0.3),
                Rect::new(0.7, 0.7, 0.3, 0.3),
                Rect::new(0.3, 0.7, 0.2, 0.3),
            ],
        );
        assert_eq!(neighbour(&l, 1, Direction::Right), Some(3));
    }

    #[test]
    fn nested_and_concentric_zones_have_sensible_neighbours() {
        let l = Layout::new(
            "n",
            "n",
            &[
                Rect::new(0.0, 0.0, 1.0, 1.0),
                Rect::new(0.25, 0.25, 0.5, 0.5),
            ],
        );
        // Same centre: neither is "beyond" the other.
        for dir in Direction::ALL {
            assert_eq!(neighbour(&l, 1, dir), None);
            assert_eq!(neighbour(&l, 2, dir), None);
        }
        // A zone nested near the right edge: the big zone has nothing beyond it, the
        // small one reaches back to the big zone on its left.
        let l = Layout::new(
            "n",
            "n",
            &[Rect::new(0.0, 0.0, 1.0, 1.0), Rect::new(0.6, 0.2, 0.3, 0.6)],
        );
        assert_eq!(neighbour(&l, 1, Direction::Right), None);
        assert_eq!(neighbour(&l, 2, Direction::Left), Some(1));
    }

    #[test]
    fn partly_overlapping_zones_are_neighbours_in_the_direction_they_extend() {
        let l = Layout::new(
            "p",
            "p",
            &[Rect::new(0.0, 0.0, 0.6, 1.0), Rect::new(0.4, 0.0, 0.6, 1.0)],
        );
        assert_eq!(neighbour(&l, 1, Direction::Right), Some(2));
        assert_eq!(neighbour(&l, 2, Direction::Left), Some(1));
        assert_eq!(neighbour(&l, 1, Direction::Left), None);
        assert_eq!(neighbour(&l, 1, Direction::Up), None);
    }

    #[test]
    fn identical_rectangles_are_not_neighbours() {
        let l = Layout::new(
            "i",
            "i",
            &[Rect::new(0.0, 0.0, 0.5, 0.5), Rect::new(0.0, 0.0, 0.5, 0.5)],
        );
        for dir in Direction::ALL {
            assert_eq!(neighbour(&l, 1, dir), None);
        }
    }

    #[test]
    fn degenerate_layouts_have_no_neighbours() {
        let one = cols(1);
        for dir in Direction::ALL {
            assert_eq!(neighbour(&one, 1, dir), None);
        }
        let empty = Layout {
            id: "e".into(),
            name: "e".into(),
            zones: vec![],
        };
        assert_eq!(neighbour(&empty, 1, Direction::Right), None);
        assert_eq!(
            neighbour(&cols(3), 99, Direction::Right),
            None,
            "unknown zone"
        );
    }

    #[test]
    fn non_finite_zones_are_never_neighbours_and_never_panic() {
        let l = Layout {
            id: "n".into(),
            name: "n".into(),
            zones: vec![
                Zone {
                    id: 1,
                    rect: Rect::new(0.0, 0.0, 0.5, 1.0),
                },
                Zone {
                    id: 2,
                    rect: Rect::new(f64::NAN, 0.0, 0.2, 1.0),
                },
                Zone {
                    id: 3,
                    rect: Rect::new(0.5, 0.0, f64::INFINITY, 1.0),
                },
                Zone {
                    id: 4,
                    rect: Rect::new(0.5, 0.0, 0.5, 1.0),
                },
            ],
        };
        assert_eq!(neighbour(&l, 1, Direction::Right), Some(4));
        for dir in Direction::ALL {
            assert_eq!(neighbour(&l, 2, dir), None, "a NaN start has no neighbours");
        }
    }

    #[test]
    fn float_noise_does_not_decide_a_tie() {
        // The lower zone is a hair taller; the tie still goes to the first zone in
        // reading order instead of whichever side the rounding error fell on.
        let l = Layout::new(
            "f",
            "f",
            &[
                Rect::new(0.0, 0.0, 0.5, 1.0),
                Rect::new(0.5, 0.0, 0.5, 0.5),
                Rect::new(0.5, 0.5, 0.5, 0.5 + 1e-12),
            ],
        );
        assert_eq!(neighbour(&l, 1, Direction::Right), Some(2));
        // And the same with the noise on the upper zone.
        let l = Layout::new(
            "f",
            "f",
            &[
                Rect::new(0.0, 0.0, 0.5, 1.0),
                Rect::new(0.5, 0.0, 0.5, 0.5 - 1e-12),
                Rect::new(0.5, 0.5 - 1e-12, 0.5, 0.5),
            ],
        );
        assert_eq!(neighbour(&l, 1, Direction::Right), Some(2));
    }

    #[test]
    fn a_zone_centred_on_the_start_beats_one_off_to_the_side() {
        // All three zones touch the start and share a third of its height; the middle
        // one is centred on it, so it wins over the first in reading order.
        let third = 1.0 / 3.0;
        let l = Layout::new(
            "c",
            "c",
            &[
                Rect::new(0.0, 0.0, 0.5, 1.0),
                Rect::new(0.5, 0.0, 0.5, third),
                Rect::new(0.5, third, 0.5, third),
                Rect::new(0.5, 2.0 * third, 0.5, third),
            ],
        );
        assert_eq!(neighbour(&l, 1, Direction::Right), Some(3));
    }

    #[test]
    fn tiny_zones_are_found_too() {
        let l = Layout::new(
            "t",
            "t",
            &[
                Rect::new(0.0, 0.0, 0.01, 0.01),
                Rect::new(0.01, 0.0, 0.01, 0.01),
            ],
        );
        assert_eq!(neighbour(&l, 1, Direction::Right), Some(2));
        assert_eq!(neighbour(&l, 2, Direction::Left), Some(1));
        assert_eq!(neighbour(&l, 1, Direction::Down), None);
    }

    // ----- swapping -----

    #[test]
    fn two_windows_in_different_zones_trade_zones() {
        let l = cols(3);
        let s = st(&[(1, 1), (2, 3)]);
        let p = swap_windows(&l, &s, 1, 2);
        assert_eq!(placed(&p), vec![(2, 1), (1, 3)]);
        assert_eq!(p.stack_changes, vec![StackChange::Swap { a: 1, b: 2 }]);
        assert!(p.raise.is_empty(), "single windows need no raising");
        let mut after = s;
        p.apply(&mut after).unwrap();
        assert_eq!(after.zone_of(1), Some(3));
        assert_eq!(after.zone_of(2), Some(1));
    }

    #[test]
    fn swapping_inside_one_stack_moves_nobody_but_reorders_the_tabs() {
        let l = cols(2);
        let s = st(&[(1, 1), (2, 1), (3, 2)]);
        let p = swap_windows(&l, &s, 1, 2);
        assert!(p.placements.is_empty());
        assert!(p.raise.is_empty());
        let mut after = s;
        p.apply(&mut after).unwrap();
        assert_eq!(after.members(1), &[2, 1]);
        assert_eq!(
            after.visible(1),
            Some(2),
            "the visible window stays visible"
        );
    }

    #[test]
    fn swapping_unknown_or_identical_windows_does_nothing() {
        let l = cols(2);
        let s = st(&[(1, 1), (2, 2)]);
        assert!(swap_windows(&l, &s, 1, 1).is_empty());
        assert!(swap_windows(&l, &s, 1, 9).is_empty());
        assert!(swap_windows(&l, &s, 9, 1).is_empty());
        assert!(swap_windows(&l, &Stacks::new(), 1, 2).is_empty());
    }

    #[test]
    fn swapping_with_a_stack_shows_the_arriving_window_on_top() {
        let l = cols(2);
        // Zone 1 holds a stack with window 2 on top; zone 2 holds window 3.
        let s = st(&[(1, 1), (2, 1), (3, 2)]);
        let p = swap_windows(&l, &s, 3, 2);
        assert_eq!(
            placed(&p),
            vec![(3, 1), (2, 2)].into_iter().collect::<Vec<_>>()
        );
        assert_eq!(p.raise, vec![3], "window 3 now sits over window 1");
        let mut after = s;
        p.apply(&mut after).unwrap();
        assert_eq!(after.visible(1), Some(3));
        assert_eq!(after.members(1), &[1, 3]);
    }

    #[test]
    fn swap_with_neighbour_trades_with_the_visible_window_there() {
        let l = cols(3);
        let s = st(&[(1, 1), (2, 2), (3, 2), (4, 3)]); // zone 2 is a stack, 3 on top
        let p = swap_with_neighbour(&l, &s, 1, Direction::Right);
        assert_eq!(placed(&p), vec![(3, 1), (1, 2)]);
        assert_eq!(p.raise, vec![1], "window 1 lands on top of window 2");
    }

    #[test]
    fn swap_with_neighbour_moves_into_an_empty_zone() {
        let l = cols(3);
        let s = st(&[(1, 1)]);
        let p = swap_with_neighbour(&l, &s, 1, Direction::Right);
        assert_eq!(placed(&p), vec![(1, 2)]);
        assert_eq!(
            p.stack_changes,
            vec![StackChange::Place { window: 1, zone: 2 }]
        );
        let mut after = s;
        p.apply(&mut after).unwrap();
        assert!(after.members(1).is_empty(), "the old zone is free again");
    }

    #[test]
    fn leaving_a_stack_for_an_empty_neighbour_reveals_the_tab_underneath() {
        let l = cols(2);
        let s = st(&[(1, 1), (2, 1)]);
        let p = swap_with_neighbour(&l, &s, 2, Direction::Right);
        assert_eq!(placed(&p), vec![(2, 2)]);
        assert_eq!(p.raise, vec![1]);
    }

    #[test]
    fn swap_with_neighbour_does_nothing_at_the_edge_or_for_loose_windows() {
        let l = cols(2);
        let s = st(&[(1, 1), (2, 2)]);
        assert!(swap_with_neighbour(&l, &s, 1, Direction::Left).is_empty());
        assert!(swap_with_neighbour(&l, &s, 2, Direction::Right).is_empty());
        assert!(swap_with_neighbour(&l, &s, 1, Direction::Up).is_empty());
        assert!(swap_with_neighbour(&l, &s, 99, Direction::Right).is_empty());
    }

    #[test]
    fn four_swaps_in_a_row_walk_a_window_round_a_grid() {
        let l = grid();
        let mut s = st(&[(1, 1), (2, 2), (3, 3), (4, 4)]);
        for dir in [
            Direction::Right,
            Direction::Down,
            Direction::Left,
            Direction::Up,
        ] {
            let p = swap_with_neighbour(&l, &s, 1, dir);
            assert!(!p.is_empty());
            p.apply(&mut s).unwrap();
            assert_eq!(s.check_invariants(), Ok(()));
        }
        assert_eq!(s.zone_of(1), Some(1), "back where it started");
        assert_eq!(s.assignments().len(), 4);
    }

    #[test]
    fn dropping_on_an_occupied_zone_swaps() {
        let l = cols(3);
        let s = st(&[(1, 1), (2, 3)]);
        let p = drop_on_zone(&l, &s, 1, 3);
        assert_eq!(placed(&p), vec![(2, 1), (1, 3)]);
    }

    #[test]
    fn dropping_on_an_empty_zone_moves() {
        let l = cols(3);
        let s = st(&[(1, 1), (2, 3)]);
        let p = drop_on_zone(&l, &s, 1, 2);
        assert_eq!(placed(&p), vec![(1, 2)]);
        assert_eq!(p.placements, vec![Placement { window: 1, zone: 2 }]);
    }

    #[test]
    fn dropping_somewhere_pointless_does_nothing() {
        let l = cols(3);
        let s = st(&[(1, 1), (2, 2)]);
        assert!(drop_on_zone(&l, &s, 1, 1).is_empty(), "same zone");
        assert!(drop_on_zone(&l, &s, 1, 99).is_empty(), "unknown zone");
        assert!(
            drop_on_zone(&l, &s, 99, 2).is_empty(),
            "loose window: an ordinary snap"
        );
    }

    #[test]
    fn swaps_are_deterministic() {
        let l = main_side();
        let s = st(&[(1, 1), (2, 2), (3, 3)]);
        let a = swap_with_neighbour(&l, &s, 1, Direction::Right);
        let b = swap_with_neighbour(&l, &s, 1, Direction::Right);
        assert_eq!(a, b);
        assert_eq!(placed(&a), vec![(2, 1), (1, 2)]);
    }
}
