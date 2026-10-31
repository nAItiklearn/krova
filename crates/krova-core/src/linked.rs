//! Linked dividers: when a snapped window is resized by one of its edges, the
//! neighbouring snapped windows that share that edge follow, so the seam between
//! them stays closed.
//!
//! Pure planning only. The caller passes the resized window's remembered rect and
//! its rect now, plus the rects of the other windows snapped on the same monitor
//! and layout (all as *visible frames*); it gets back the rect the resized window
//! should end up with and the new rect of every window whose edge has to move.
//!
//! The seam is treated as one divider line, like the layout editor does: every
//! window whose edge lies on that line and is connected to the dragged edge
//! (overlapping or touching along the line) moves with it. In a 2x2 grid, dragging
//! the vertical seam moves it for all four windows; windows whose edges are merely
//! close to the line but not on it, or that are not connected to the dragged
//! stretch of it, are left alone.

use std::collections::BTreeMap;

use crate::geometry::PxRect;
use crate::registry::WindowKey;

/// How far (pixels) an edge may be from a seam and still lie on it. Also the
/// smallest edge movement that counts as a resize at all.
pub const EDGE_TOLERANCE_PX: i32 = 4;

/// Smallest width or height a window is shrunk to by a linked resize.
pub const MIN_LINKED_SIDE_PX: i32 = 120;

/// Result of a linked resize.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedPlan {
    /// The rect the resized window should end up with. Differs from what the user
    /// dragged it to only when a neighbour could not shrink any further.
    pub resized: PxRect,
    /// New rects for the windows that follow, ordered by window key. Windows whose
    /// rect would not change are not listed.
    pub follows: Vec<(WindowKey, PxRect)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Axis {
    /// Seam is a vertical line; windows sit side by side (across = x).
    X,
    /// Seam is a horizontal line; windows are stacked (across = y).
    Y,
}

/// `(a0, a1, b0, b1)`: `a` runs across the seam, `b` along it.
type Ab = (i32, i32, i32, i32);

fn split(axis: Axis, r: PxRect) -> Ab {
    match axis {
        Axis::X => (r.x, r.right(), r.y, r.bottom()),
        Axis::Y => (r.y, r.bottom(), r.x, r.right()),
    }
}

fn join(axis: Axis, (a0, a1, b0, b1): Ab) -> PxRect {
    match axis {
        Axis::X => PxRect::new(a0, b0, a1.saturating_sub(a0), b1.saturating_sub(b0)),
        Axis::Y => PxRect::new(b0, a0, b1.saturating_sub(b0), a1.saturating_sub(a0)),
    }
}

fn near(p: i32, q: i32) -> bool {
    (i64::from(p) - i64::from(q)).abs() <= i64::from(EDGE_TOLERANCE_PX)
}

/// One dragged edge of the resized window.
#[derive(Clone, Copy)]
struct Seam {
    axis: Axis,
    /// The dragged edge is the far (right/bottom) edge of the dragged window, i.e.
    /// the dragged window is on the low side of the seam.
    dragged_low: bool,
    old_edge: i32,
    new_edge: i32,
}

/// Move one seam: the dragged window's edge and every connected edge on the line.
fn move_seam(
    rects: &mut BTreeMap<WindowKey, PxRect>,
    dragged: WindowKey,
    seam: Seam,
    gap: i32,
    min_side: i32,
) {
    let Seam {
        axis,
        dragged_low,
        old_edge,
        new_edge,
    } = seam;
    // Where the low side's far edge sits before and after (the high side sits
    // `gap` further on).
    let (old_low, want_low) = if dragged_low {
        (old_edge, new_edge)
    } else {
        (old_edge.saturating_sub(gap), new_edge.saturating_sub(gap))
    };

    // Windows with an edge on the seam, and which side they are on.
    let mut candidates: Vec<(WindowKey, bool)> = Vec::new();
    for (&key, &rect) in rects.iter() {
        if key == dragged {
            continue;
        }
        let (a0, a1, _, _) = split(axis, rect);
        if near(a1, old_low) {
            candidates.push((key, true));
        } else if near(a0, old_low.saturating_add(gap)) {
            candidates.push((key, false));
        }
    }

    // Grow the connected set outwards from the dragged window.
    let slack = EDGE_TOLERANCE_PX.saturating_add(gap);
    let touches =
        |x: &Ab, y: &Ab| x.2 <= y.3.saturating_add(slack) && y.2 <= x.3.saturating_add(slack);
    let mut members: Vec<(WindowKey, bool)> = vec![(dragged, dragged_low)];
    loop {
        let before = members.len();
        candidates.retain(|&(key, low)| {
            let ab = split(axis, rects[&key]);
            let connected = members
                .iter()
                .any(|(m, _)| touches(&ab, &split(axis, rects[m])));
            if connected {
                members.push((key, low));
            }
            !connected
        });
        if members.len() == before {
            break;
        }
    }

    let new_low = if members.len() == 1 {
        // Nobody to follow: the window goes where the user dragged it.
        want_low
    } else {
        // Nobody may be squeezed below the minimum (or below what it already is).
        let mut lo = i32::MIN;
        let mut hi = i32::MAX;
        for &(key, low) in &members {
            let (a0, a1, _, _) = split(axis, rects[&key]);
            let limit = min_side.min(a1.saturating_sub(a0)).max(1);
            if low {
                lo = lo.max(a0.saturating_add(limit));
            } else {
                hi = hi.min(a1.saturating_sub(gap).saturating_sub(limit));
            }
        }
        if lo > hi {
            return;
        }
        want_low.clamp(lo, hi)
    };

    for (key, low) in members {
        let Some(rect) = rects.get_mut(&key) else {
            continue;
        };
        let mut ab = split(axis, *rect);
        if low {
            ab.1 = new_low;
        } else {
            ab.0 = new_low.saturating_add(gap);
        }
        *rect = join(axis, ab);
    }
}

/// Plan the neighbours' reaction to `dragged` being resized from `old` to `new`.
///
/// - `peers`: the other windows snapped on the same monitor and layout, with their
///   current visible frames (the resized window itself is ignored if listed);
/// - `gap`: the zone gap in pixels (the distance between adjacent windows);
/// - `min_side`: the smallest width or height a window may be squeezed to.
///
/// Returns `None` when the change is not a resize (a pure move, or movement within
/// [`EDGE_TOLERANCE_PX`]).
pub fn plan_linked_resize(
    dragged: WindowKey,
    old: PxRect,
    new: PxRect,
    peers: &[(WindowKey, PxRect)],
    gap: i32,
    min_side: i32,
) -> Option<LinkedPlan> {
    let delta = |a: i32, b: i32| i64::from(a) - i64::from(b);
    let tol = i64::from(EDGE_TOLERANCE_PX);
    let resized_w = delta(new.w, old.w).abs() > tol;
    let resized_h = delta(new.h, old.h).abs() > tol;
    if !resized_w && !resized_h {
        return None;
    }
    let gap = gap.max(0);

    let mut rects: BTreeMap<WindowKey, PxRect> = peers
        .iter()
        .filter(|(k, r)| *k != dragged && r.w > 0 && r.h > 0)
        .map(|(k, r)| (*k, *r))
        .collect();
    let originals = rects.clone();
    rects.insert(dragged, old);

    let edges = [
        (Axis::X, false, old.x, new.x),
        (Axis::X, true, old.right(), new.right()),
        (Axis::Y, false, old.y, new.y),
        (Axis::Y, true, old.bottom(), new.bottom()),
    ];
    for (axis, dragged_low, old_edge, new_edge) in edges {
        if delta(new_edge, old_edge).abs() > tol {
            let seam = Seam {
                axis,
                dragged_low,
                old_edge,
                new_edge,
            };
            move_seam(&mut rects, dragged, seam, gap, min_side);
        }
    }

    let resized = rects.remove(&dragged).unwrap_or(new);
    let follows = rects
        .into_iter()
        .filter(|(k, r)| originals.get(k) != Some(r))
        .collect();
    Some(LinkedPlan { resized, follows })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: i32 = MIN_LINKED_SIDE_PX;

    fn k(n: u64) -> WindowKey {
        WindowKey(n)
    }

    fn plan(
        dragged: u64,
        old: PxRect,
        new: PxRect,
        peers: &[(u64, PxRect)],
        gap: i32,
    ) -> Option<LinkedPlan> {
        let peers: Vec<_> = peers.iter().map(|(n, r)| (k(*n), *r)).collect();
        plan_linked_resize(k(dragged), old, new, &peers, gap, MIN)
    }

    fn follow(p: &LinkedPlan, n: u64) -> Option<PxRect> {
        p.follows
            .iter()
            .find(|(key, _)| *key == k(n))
            .map(|(_, r)| *r)
    }

    // Two halves of a 1920x1040 work area.
    const LEFT: PxRect = PxRect::new(0, 0, 960, 1040);
    const RIGHT: PxRect = PxRect::new(960, 0, 960, 1040);

    #[test]
    fn two_halves_dragging_the_left_windows_right_edge() {
        let new = PxRect::new(0, 0, 1100, 1040);
        let p = plan(1, LEFT, new, &[(2, RIGHT)], 0).unwrap();
        assert_eq!(p.resized, new);
        assert_eq!(p.follows, vec![(k(2), PxRect::new(1100, 0, 820, 1040))]);
    }

    #[test]
    fn two_halves_dragging_the_right_windows_left_edge() {
        let new = PxRect::new(800, 0, 1120, 1040);
        let p = plan(2, RIGHT, new, &[(1, LEFT)], 0).unwrap();
        assert_eq!(p.resized, new);
        assert_eq!(p.follows, vec![(k(1), PxRect::new(0, 0, 800, 1040))]);
    }

    #[test]
    fn shrinking_works_too_and_the_neighbour_grows() {
        let new = PxRect::new(0, 0, 700, 1040);
        let p = plan(1, LEFT, new, &[(2, RIGHT)], 0).unwrap();
        assert_eq!(follow(&p, 2), Some(PxRect::new(700, 0, 1220, 1040)));
    }

    #[test]
    fn three_columns_dragging_a_middle_edge_moves_only_its_neighbour() {
        let a = PxRect::new(0, 0, 640, 1040);
        let b = PxRect::new(640, 0, 640, 1040);
        let c = PxRect::new(1280, 0, 640, 1040);
        // Right edge of the middle window: only the right column follows.
        let new_b = PxRect::new(640, 0, 760, 1040);
        let p = plan(2, b, new_b, &[(1, a), (3, c)], 0).unwrap();
        assert_eq!(p.follows, vec![(k(3), PxRect::new(1400, 0, 520, 1040))]);
        // Left edge of the middle window: only the left column follows.
        let new_b = PxRect::new(500, 0, 780, 1040);
        let p = plan(2, b, new_b, &[(1, a), (3, c)], 0).unwrap();
        assert_eq!(p.follows, vec![(k(1), PxRect::new(0, 0, 500, 1040))]);
    }

    const TL: PxRect = PxRect::new(0, 0, 960, 520);
    const TR: PxRect = PxRect::new(960, 0, 960, 520);
    const BL: PxRect = PxRect::new(0, 520, 960, 520);
    const BR: PxRect = PxRect::new(960, 520, 960, 520);

    #[test]
    fn a_2x2_grid_moves_the_whole_vertical_line() {
        let new = PxRect::new(0, 0, 1100, 520);
        let p = plan(1, TL, new, &[(2, TR), (3, BL), (4, BR)], 0).unwrap();
        assert_eq!(p.resized, new);
        assert_eq!(follow(&p, 2), Some(PxRect::new(1100, 0, 820, 520)));
        assert_eq!(follow(&p, 3), Some(PxRect::new(0, 520, 1100, 520)));
        assert_eq!(follow(&p, 4), Some(PxRect::new(1100, 520, 820, 520)));
    }

    #[test]
    fn a_2x2_grid_moves_the_whole_horizontal_line() {
        let new = PxRect::new(960, 0, 960, 400);
        let p = plan(2, TR, new, &[(1, TL), (3, BL), (4, BR)], 0).unwrap();
        assert_eq!(p.resized, new);
        assert_eq!(follow(&p, 1), Some(PxRect::new(0, 0, 960, 400)));
        assert_eq!(follow(&p, 3), Some(PxRect::new(0, 400, 960, 640)));
        assert_eq!(follow(&p, 4), Some(PxRect::new(960, 400, 960, 640)));
    }

    #[test]
    fn dragging_the_corner_of_a_grid_moves_both_lines() {
        let new = PxRect::new(0, 0, 1000, 600);
        let p = plan(1, TL, new, &[(2, TR), (3, BL), (4, BR)], 0).unwrap();
        assert_eq!(p.resized, new);
        assert_eq!(follow(&p, 2), Some(PxRect::new(1000, 0, 920, 600)));
        assert_eq!(follow(&p, 3), Some(PxRect::new(0, 600, 1000, 440)));
        assert_eq!(follow(&p, 4), Some(PxRect::new(1000, 600, 920, 440)));
    }

    #[test]
    fn a_t_junction_moves_every_window_on_the_line() {
        // A left window over the full height, two windows stacked on the right.
        let a = PxRect::new(0, 0, 960, 1040);
        let b = PxRect::new(960, 0, 960, 520);
        let c = PxRect::new(960, 520, 960, 520);
        // Drag the top-right window's left edge: the left window and the
        // bottom-right window (same line) move too.
        let new_b = PxRect::new(800, 0, 1120, 520);
        let p = plan(2, b, new_b, &[(1, a), (3, c)], 0).unwrap();
        assert_eq!(p.resized, new_b);
        assert_eq!(follow(&p, 1), Some(PxRect::new(0, 0, 800, 1040)));
        assert_eq!(follow(&p, 3), Some(PxRect::new(800, 520, 1120, 520)));
    }

    #[test]
    fn windows_near_but_not_on_the_seam_are_left_alone() {
        let third = PxRect::new(1000, 0, 920, 1040); // left edge 40 px off the seam
        let p = plan(
            1,
            LEFT,
            PxRect::new(0, 0, 1100, 1040),
            &[(2, RIGHT), (3, third)],
            0,
        )
        .unwrap();
        assert_eq!(follow(&p, 2), Some(PxRect::new(1100, 0, 820, 1040)));
        assert_eq!(follow(&p, 3), None);
    }

    #[test]
    fn an_edge_within_the_tolerance_counts_as_on_the_seam() {
        let slightly_off = PxRect::new(963, 0, 957, 1040); // 3 px off
        let p = plan(
            1,
            LEFT,
            PxRect::new(0, 0, 1100, 1040),
            &[(2, slightly_off)],
            0,
        )
        .unwrap();
        assert_eq!(follow(&p, 2), Some(PxRect::new(1100, 0, 820, 1040)));
        let too_far = PxRect::new(966, 0, 954, 1040); // 6 px off
        let p = plan(1, LEFT, PxRect::new(0, 0, 1100, 1040), &[(2, too_far)], 0).unwrap();
        assert!(p.follows.is_empty());
    }

    #[test]
    fn windows_far_along_the_line_and_not_connected_stay_put() {
        // Two rows of halves with an empty band between them.
        let top_l = PxRect::new(0, 0, 960, 300);
        let top_r = PxRect::new(960, 0, 960, 300);
        let low_l = PxRect::new(0, 700, 960, 340);
        let low_r = PxRect::new(960, 700, 960, 340);
        let p = plan(
            1,
            top_l,
            PxRect::new(0, 0, 1100, 300),
            &[(2, top_r), (3, low_l), (4, low_r)],
            0,
        )
        .unwrap();
        assert_eq!(p.follows, vec![(k(2), PxRect::new(1100, 0, 820, 300))]);
    }

    #[test]
    fn an_edge_with_no_neighbour_just_resizes() {
        // Dragging the left edge of the left window (the screen edge).
        let new = PxRect::new(40, 0, 920, 1040);
        let p = plan(1, LEFT, new, &[(2, RIGHT)], 0).unwrap();
        assert_eq!(p.resized, new);
        assert!(p.follows.is_empty());
        // Alone on the screen.
        let p = plan(1, LEFT, PxRect::new(0, 0, 1500, 1040), &[], 0).unwrap();
        assert_eq!(p.resized, PxRect::new(0, 0, 1500, 1040));
        assert!(p.follows.is_empty());
    }

    #[test]
    fn a_neighbour_is_never_squeezed_below_the_minimum() {
        // Drag so far that the right window would be 40 px wide.
        let new = PxRect::new(0, 0, 1880, 1040);
        let p = plan(1, LEFT, new, &[(2, RIGHT)], 0).unwrap();
        assert_eq!(follow(&p, 2), Some(PxRect::new(1920 - MIN, 0, MIN, 1040)));
        assert_eq!(
            p.resized,
            PxRect::new(0, 0, 1920 - MIN, 1040),
            "the dragged window is held back too"
        );
    }

    #[test]
    fn the_dragged_window_keeps_its_minimum_too() {
        // Shrink the left window to 30 px: it stops at the minimum.
        let new = PxRect::new(0, 0, 30, 1040);
        let p = plan(1, LEFT, new, &[(2, RIGHT)], 0).unwrap();
        assert_eq!(p.resized, PxRect::new(0, 0, MIN, 1040));
        assert_eq!(follow(&p, 2), Some(PxRect::new(MIN, 0, 1920 - MIN, 1040)));
    }

    #[test]
    fn a_window_already_below_the_minimum_does_not_block_the_others() {
        let tiny = PxRect::new(960, 0, 60, 1040);
        let rest = PxRect::new(1020, 0, 900, 1040);
        // Drag the tiny window's left edge a little to the left: it grows.
        let new = PxRect::new(900, 0, 120, 1040);
        let p = plan(2, tiny, new, &[(1, LEFT), (3, rest)], 0).unwrap();
        assert_eq!(p.resized, new);
        assert_eq!(follow(&p, 1), Some(PxRect::new(0, 0, 900, 1040)));
    }

    #[test]
    fn gaps_are_kept_between_the_windows() {
        // Halves with a 10 px gap: 10 px margin around, 10 px seam.
        let l = PxRect::new(10, 10, 945, 1020);
        let r = PxRect::new(965, 10, 945, 1020);
        let new = PxRect::new(10, 10, 1000, 1020);
        let p = plan(1, l, new, &[(2, r)], 10).unwrap();
        assert_eq!(p.resized, new);
        assert_eq!(follow(&p, 2), Some(PxRect::new(1020, 10, 890, 1020)));
        // From the other side.
        let new = PxRect::new(900, 10, 1010, 1020);
        let p = plan(2, r, new, &[(1, l)], 10).unwrap();
        assert_eq!(follow(&p, 1), Some(PxRect::new(10, 10, 880, 1020)));
    }

    #[test]
    fn with_gaps_the_stacked_windows_of_a_column_stay_connected() {
        // Left column of two stacked windows (10 px apart), one window on the right.
        let tl = PxRect::new(10, 10, 945, 500);
        let bl = PxRect::new(10, 520, 945, 510);
        let r = PxRect::new(965, 10, 945, 1020);
        let new = PxRect::new(10, 10, 1005, 500);
        let p = plan(1, tl, new, &[(2, bl), (3, r)], 10).unwrap();
        assert_eq!(follow(&p, 2), Some(PxRect::new(10, 520, 1005, 510)));
        assert_eq!(follow(&p, 3), Some(PxRect::new(1025, 10, 885, 1020)));
    }

    #[test]
    fn a_pure_move_or_jitter_is_not_a_resize() {
        let moved = PxRect::new(200, 100, 960, 1040);
        assert_eq!(plan(1, LEFT, moved, &[(2, RIGHT)], 0), None);
        let jitter = PxRect::new(1, 1, 962, 1038);
        assert_eq!(plan(1, LEFT, jitter, &[(2, RIGHT)], 0), None);
        assert_eq!(plan(1, LEFT, LEFT, &[(2, RIGHT)], 0), None);
    }

    #[test]
    fn the_dragged_window_listed_among_the_peers_is_ignored() {
        let new = PxRect::new(0, 0, 1100, 1040);
        let p = plan(1, LEFT, new, &[(1, LEFT), (2, RIGHT)], 0).unwrap();
        assert_eq!(p.follows.len(), 1);
        assert_eq!(p.follows[0].0, k(2));
    }

    #[test]
    fn degenerate_peers_are_ignored() {
        let zero = PxRect::new(960, 0, 0, 0);
        let p = plan(1, LEFT, PxRect::new(0, 0, 1100, 1040), &[(2, zero)], 0).unwrap();
        assert!(p.follows.is_empty());
    }

    #[test]
    fn the_result_does_not_depend_on_the_order_of_the_peers() {
        let new = PxRect::new(0, 0, 1100, 520);
        let a = plan(1, TL, new, &[(2, TR), (3, BL), (4, BR)], 0).unwrap();
        let b = plan(1, TL, new, &[(4, BR), (3, BL), (2, TR)], 0).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn absurd_rects_do_not_overflow() {
        let huge = PxRect::new(i32::MAX - 5, i32::MAX - 5, 1000, 1000);
        let p = plan(
            1,
            LEFT,
            PxRect::new(0, 0, 1100, 1040),
            &[(2, huge), (3, RIGHT)],
            0,
        );
        assert!(p.is_some());
        let p = plan(
            1,
            PxRect::new(i32::MIN, 0, 10, 10),
            PxRect::new(i32::MIN, 0, 2000, 10),
            &[(2, huge)],
            i32::MAX,
        );
        assert!(p.is_some());
    }
}
