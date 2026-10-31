//! Zone selection for the keyboard shortcuts: which zone a window should go to.
//!
//! "Zone order" is reading order: top to bottom by the zone's `y` rounded to 1%,
//! then left to right. Zone number `N` in a shortcut (`<mods>+N`) is the N-th zone
//! in that order, not the zone's id, so the numbers follow what the user sees.
//!
//! Everything here is pure; callers pass in the layouts and rectangles.

use crate::geometry::PxRect;
use crate::snap::MonitorView;
use crate::zone::Layout;

/// What a shortcut asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    /// `<mods>+Left`: the previous zone (wraps around).
    PrevZone,
    /// `<mods>+Right`: the next zone (wraps around).
    NextZone,
    /// `<mods>+1` ... `<mods>+9`: zone number N (1-based, reading order).
    Zone(u8),
    /// `<mods>+Up`: the next monitor, keeping the zone number.
    NextMonitor,
}

/// A zone on a monitor (index into the monitor list the caller passed in).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyboardTarget {
    pub monitor: usize,
    pub zone: u32,
}

fn percent(v: f64) -> i64 {
    if v.is_finite() {
        (v * 100.0).round() as i64
    } else {
        i64::MAX
    }
}

/// Zone ids in reading order: top to bottom (`y` rounded to 1%), then left to
/// right; ties fall back to the zone id so the order is total and stable.
pub fn reading_order(layout: &Layout) -> Vec<u32> {
    let mut zones: Vec<_> = layout.zones.iter().collect();
    zones.sort_by(|a, b| {
        percent(a.rect.y)
            .cmp(&percent(b.rect.y))
            .then(a.rect.x.total_cmp(&b.rect.x))
            .then(a.id.cmp(&b.id))
    });
    zones.into_iter().map(|z| z.id).collect()
}

/// Position of `zone_id` in reading order (0-based).
pub fn zone_index(layout: &Layout, zone_id: u32) -> Option<usize> {
    reading_order(layout).iter().position(|&id| id == zone_id)
}

/// Zone id of zone number `n` (1-based). `None` for 0, for numbers beyond the
/// layout, and for an empty layout.
pub fn zone_by_number(layout: &Layout, n: usize) -> Option<u32> {
    let index = n.checked_sub(1)?;
    reading_order(layout).get(index).copied()
}

/// Neighbouring zone in reading order, wrapping around at both ends. With no
/// current zone, `forward` gives the first zone and backward the last.
fn step(layout: &Layout, current: Option<u32>, forward: bool) -> Option<u32> {
    let order = reading_order(layout);
    let len = order.len();
    if len == 0 {
        return None;
    }
    let at = current.and_then(|id| order.iter().position(|&z| z == id));
    let next = match (at, forward) {
        (Some(i), true) => (i + 1) % len,
        (Some(i), false) => (i + len - 1) % len,
        (None, true) => 0,
        (None, false) => len - 1,
    };
    Some(order[next])
}

/// The zone of `layout` (laid out on `work`) that `rect` overlaps most; ties go to
/// the earlier zone in reading order. `None` if it overlaps no zone.
pub fn zone_for_rect(layout: &Layout, work: PxRect, rect: PxRect) -> Option<u32> {
    let overlap = |a: PxRect, b: PxRect| -> i64 {
        let w = i64::from(a.right().min(b.right())) - i64::from(a.x.max(b.x));
        let h = i64::from(a.bottom().min(b.bottom())) - i64::from(a.y.max(b.y));
        if w > 0 && h > 0 {
            w * h
        } else {
            0
        }
    };
    let mut best: Option<(i64, u32)> = None;
    for id in reading_order(layout) {
        let Some(zone) = layout.zone_pixels(id, work) else {
            continue;
        };
        let area = overlap(zone, rect);
        // Strictly greater: on a tie the earlier zone in reading order stays.
        let better = match best {
            Some((a, _)) => area > a,
            None => area > 0,
        };
        if better {
            best = Some((area, id));
        }
    }
    best.map(|(_, id)| id)
}

/// Monitor indices ordered left to right (then top to bottom), by their rect.
pub fn monitor_order(rects: &[PxRect]) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..rects.len()).collect();
    idx.sort_by(|&a, &b| {
        rects[a]
            .x
            .cmp(&rects[b].x)
            .then(rects[a].y.cmp(&rects[b].y))
            .then(a.cmp(&b))
    });
    idx
}

/// The monitor a window is on: the one containing its centre, or, if the centre is
/// on no monitor (a window dragged half off-screen), the nearest one.
pub fn monitor_for_rect(monitors: &[PxRect], window: PxRect) -> Option<usize> {
    let cx = i64::from(window.x) + i64::from(window.w) / 2;
    let cy = i64::from(window.y) + i64::from(window.h) / 2;
    let distance2 = |m: &PxRect| -> i64 {
        let dx = (i64::from(m.x) - cx).max(cx - i64::from(m.right())).max(0);
        let dy = (i64::from(m.y) - cy).max(cy - i64::from(m.bottom())).max(0);
        dx * dx + dy * dy
    };
    monitors
        .iter()
        .enumerate()
        .min_by(|(ia, a), (ib, b)| distance2(a).cmp(&distance2(b)).then(ia.cmp(ib)))
        .map(|(i, _)| i)
}

/// Zone of `to` that has the same reading-order position as `zone_id` has in
/// `from`. If `to` has fewer zones the last one is used; with no current zone the
/// first one. `None` only when `to` has no zones.
fn same_position(from: &Layout, zone_id: Option<u32>, to: &Layout) -> Option<u32> {
    let order = reading_order(to);
    let last = order.len().checked_sub(1)?;
    let index = zone_id.and_then(|z| zone_index(from, z)).unwrap_or(0);
    Some(order[index.min(last)])
}

/// Where should `action` send a window that is on `monitors[from]`, currently in
/// `current_zone` of that monitor's layout (`None` if it is in no zone)?
///
/// Returns `None` when there is nothing sensible to do: no such monitor, an empty
/// or missing layout, a zone number past the end, or `NextMonitor` with no other
/// monitor that has a layout.
pub fn plan_keyboard_move(
    action: KeyAction,
    monitors: &[MonitorView<'_>],
    from: usize,
    current_zone: Option<u32>,
) -> Option<KeyboardTarget> {
    let here = monitors.get(from)?;
    let layout = here.layout.filter(|l| !l.zones.is_empty());
    match action {
        KeyAction::PrevZone | KeyAction::NextZone => {
            let layout = layout?;
            let forward = action == KeyAction::NextZone;
            Some(KeyboardTarget {
                monitor: from,
                zone: step(layout, current_zone, forward)?,
            })
        }
        KeyAction::Zone(n) => Some(KeyboardTarget {
            monitor: from,
            zone: zone_by_number(layout?, usize::from(n))?,
        }),
        KeyAction::NextMonitor => {
            let rects: Vec<PxRect> = monitors.iter().map(|m| m.rect).collect();
            let order = monitor_order(&rects);
            let position = order.iter().position(|&m| m == from)?;
            // Next monitor to the right, wrapping; skip any that has no layout.
            (1..order.len())
                .map(|offset| order[(position + offset) % order.len()])
                .find_map(|to| {
                    let target = monitors[to].layout.filter(|l| !l.zones.is_empty())?;
                    let zone = match layout {
                        Some(source) => same_position(source, current_zone, target)?,
                        // The source has no layout: take the first zone.
                        None => reading_order(target).first().copied()?,
                    };
                    Some(KeyboardTarget { monitor: to, zone })
                })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Rect;
    use crate::zone::templates;

    fn halves() -> Layout {
        templates().remove(0)
    }
    fn thirds() -> Layout {
        templates().remove(1)
    }
    fn grid() -> Layout {
        templates().remove(2)
    }

    fn view(x: i32, layout: Option<&Layout>) -> MonitorView<'_> {
        MonitorView {
            rect: PxRect::new(x, 0, 1920, 1080),
            work: PxRect::new(x, 0, 1920, 1040),
            layout,
        }
    }

    #[test]
    fn reading_order_is_rows_then_columns() {
        // Ids are deliberately scrambled relative to the visual order.
        let mut l = grid();
        l.zones[0].id = 40;
        l.zones[1].id = 30;
        l.zones[2].id = 20;
        l.zones[3].id = 10;
        // Visual order: top-left(40), top-right(30), bottom-left(20), bottom-right(10).
        assert_eq!(reading_order(&l), vec![40, 30, 20, 10]);
        assert_eq!(zone_index(&l, 20), Some(2));
        assert_eq!(zone_index(&l, 99), None);
    }

    #[test]
    fn rows_are_compared_at_one_percent_resolution() {
        // y differs by 0.2%: the same row, so x decides.
        let l = Layout::new(
            "r",
            "r",
            &[
                Rect::new(0.5, 0.002, 0.5, 0.5),
                Rect::new(0.0, 0.0, 0.5, 0.5),
                Rect::new(0.0, 0.5, 1.0, 0.5),
            ],
        );
        assert_eq!(reading_order(&l), vec![2, 1, 3]);
        // y differs by a full 2%: a different row, so the upper one comes first.
        let l = Layout::new(
            "r",
            "r",
            &[
                Rect::new(0.0, 0.02, 0.5, 0.5),
                Rect::new(0.5, 0.0, 0.5, 0.5),
            ],
        );
        assert_eq!(reading_order(&l), vec![2, 1]);
    }

    #[test]
    fn identical_positions_fall_back_to_the_id() {
        let l = Layout::new(
            "o",
            "o",
            &[Rect::new(0.0, 0.0, 1.0, 1.0), Rect::new(0.0, 0.0, 0.5, 0.5)],
        );
        assert_eq!(reading_order(&l), vec![1, 2]);
    }

    #[test]
    fn zone_numbers_are_one_based_and_guarded() {
        let l = thirds();
        assert_eq!(zone_by_number(&l, 0), None);
        assert_eq!(zone_by_number(&l, 1), Some(1));
        assert_eq!(zone_by_number(&l, 3), Some(3));
        assert_eq!(zone_by_number(&l, 4), None);
        assert_eq!(zone_by_number(&l, usize::MAX), None);
        let empty = Layout {
            id: "e".into(),
            name: "e".into(),
            zones: vec![],
        };
        assert_eq!(zone_by_number(&empty, 1), None);
        assert!(reading_order(&empty).is_empty());
    }

    #[test]
    fn stepping_wraps_in_both_directions() {
        let l = thirds();
        assert_eq!(step(&l, Some(1), true), Some(2));
        assert_eq!(step(&l, Some(3), true), Some(1));
        assert_eq!(step(&l, Some(1), false), Some(3));
        assert_eq!(step(&l, Some(2), false), Some(1));
        // Not in any zone: forward starts at the first zone, backward at the last.
        assert_eq!(step(&l, None, true), Some(1));
        assert_eq!(step(&l, None, false), Some(3));
        // A zone id that is not in the layout counts as "not in any zone".
        assert_eq!(step(&l, Some(77), true), Some(1));
        let one = Layout::new("o", "o", &[Rect::new(0.0, 0.0, 1.0, 1.0)]);
        assert_eq!(step(&one, Some(1), true), Some(1));
        assert_eq!(step(&one, Some(1), false), Some(1));
    }

    #[test]
    fn stepping_an_empty_layout_is_none() {
        let empty = Layout {
            id: "e".into(),
            name: "e".into(),
            zones: vec![],
        };
        assert_eq!(step(&empty, None, true), None);
        assert_eq!(step(&empty, Some(1), false), None);
    }

    #[test]
    fn the_zone_with_the_largest_overlap_wins() {
        let l = halves();
        let work = PxRect::new(0, 0, 1920, 1040);
        assert_eq!(
            zone_for_rect(&l, work, PxRect::new(100, 100, 800, 600)),
            Some(1)
        );
        assert_eq!(
            zone_for_rect(&l, work, PxRect::new(1000, 100, 800, 600)),
            Some(2)
        );
        // Straddling the seam: 300 px on the left, 500 px on the right.
        assert_eq!(
            zone_for_rect(&l, work, PxRect::new(660, 0, 800, 600)),
            Some(2)
        );
        // Entirely outside the work area.
        assert_eq!(
            zone_for_rect(&l, work, PxRect::new(5000, 0, 100, 100)),
            None
        );
        assert_eq!(zone_for_rect(&l, work, PxRect::new(0, 0, 0, 0)), None);
    }

    #[test]
    fn an_overlap_tie_goes_to_the_earlier_zone() {
        let l = halves();
        let work = PxRect::new(0, 0, 1920, 1040);
        assert_eq!(
            zone_for_rect(&l, work, PxRect::new(760, 0, 400, 400)),
            Some(1)
        );
    }

    #[test]
    fn monitors_are_ordered_left_to_right() {
        let rects = [
            PxRect::new(1920, 0, 1920, 1080),
            PxRect::new(-1920, 0, 1920, 1080),
            PxRect::new(0, 0, 1920, 1080),
        ];
        assert_eq!(monitor_order(&rects), vec![1, 2, 0]);
        assert!(monitor_order(&[]).is_empty());
        // Same x: top before bottom.
        let stacked = [
            PxRect::new(0, 1080, 1920, 1080),
            PxRect::new(0, 0, 1920, 1080),
        ];
        assert_eq!(monitor_order(&stacked), vec![1, 0]);
    }

    #[test]
    fn the_monitor_of_a_window_is_the_one_under_its_centre() {
        let m = [
            PxRect::new(0, 0, 1920, 1080),
            PxRect::new(1920, 0, 1920, 1080),
        ];
        assert_eq!(
            monitor_for_rect(&m, PxRect::new(100, 100, 400, 300)),
            Some(0)
        );
        assert_eq!(
            monitor_for_rect(&m, PxRect::new(2000, 100, 400, 300)),
            Some(1)
        );
        // Centre at x = 1900 + 200 = 2100 -> second monitor even though it starts on the first.
        assert_eq!(
            monitor_for_rect(&m, PxRect::new(1900, 100, 400, 300)),
            Some(1)
        );
        // Far off to the right of everything: nearest is the second monitor.
        assert_eq!(
            monitor_for_rect(&m, PxRect::new(9000, 0, 100, 100)),
            Some(1)
        );
        assert_eq!(monitor_for_rect(&[], PxRect::new(0, 0, 1, 1)), None);
    }

    #[test]
    fn left_and_right_move_within_the_monitor() {
        let t = thirds();
        let v = [view(0, Some(&t))];
        let go = |a, z| plan_keyboard_move(a, &v, 0, z);
        assert_eq!(
            go(KeyAction::NextZone, Some(1)),
            Some(KeyboardTarget {
                monitor: 0,
                zone: 2
            })
        );
        assert_eq!(
            go(KeyAction::PrevZone, Some(1)),
            Some(KeyboardTarget {
                monitor: 0,
                zone: 3
            })
        );
        assert_eq!(
            go(KeyAction::NextZone, None),
            Some(KeyboardTarget {
                monitor: 0,
                zone: 1
            })
        );
    }

    #[test]
    fn digits_pick_a_zone_by_number_and_ignore_out_of_range() {
        let g = grid();
        let v = [view(0, Some(&g))];
        let go = |n| plan_keyboard_move(KeyAction::Zone(n), &v, 0, None);
        assert_eq!(
            go(1),
            Some(KeyboardTarget {
                monitor: 0,
                zone: 1
            })
        );
        assert_eq!(
            go(4),
            Some(KeyboardTarget {
                monitor: 0,
                zone: 4
            })
        );
        assert_eq!(go(5), None);
        assert_eq!(go(9), None);
        assert_eq!(go(0), None);
    }

    #[test]
    fn up_sends_the_window_to_the_next_monitor_keeping_its_zone_number() {
        let h = halves();
        let t = thirds();
        let v = [view(0, Some(&h)), view(1920, Some(&t))];
        let up = |from, z| plan_keyboard_move(KeyAction::NextMonitor, &v, from, z);
        // Zone 2 of the halves -> zone 2 of the thirds.
        assert_eq!(
            up(0, Some(2)),
            Some(KeyboardTarget {
                monitor: 1,
                zone: 2
            })
        );
        // From the right monitor it wraps back to the left one.
        assert_eq!(
            up(1, Some(3)),
            Some(KeyboardTarget {
                monitor: 0,
                zone: 2
            })
        );
        // The target has fewer zones: the last one is used.
        assert_eq!(up(1, Some(3)).unwrap().zone, 2);
        // Not in a zone yet: the first zone.
        assert_eq!(
            up(0, None),
            Some(KeyboardTarget {
                monitor: 1,
                zone: 1
            })
        );
    }

    #[test]
    fn up_follows_screen_order_not_enumeration_order() {
        let h = halves();
        // Enumerated right-to-left: index 0 is the right-hand monitor.
        let v = [
            view(1920, Some(&h)),
            view(0, Some(&h)),
            view(3840, Some(&h)),
        ];
        let up = |from| plan_keyboard_move(KeyAction::NextMonitor, &v, from, Some(1));
        assert_eq!(up(1).unwrap().monitor, 0); // left -> middle
        assert_eq!(up(0).unwrap().monitor, 2); // middle -> right
        assert_eq!(up(2).unwrap().monitor, 1); // right -> wraps to left
    }

    #[test]
    fn up_skips_monitors_without_a_layout_and_needs_a_second_monitor() {
        let h = halves();
        let one = [view(0, Some(&h))];
        assert_eq!(
            plan_keyboard_move(KeyAction::NextMonitor, &one, 0, Some(1)),
            None
        );
        let v = [view(0, Some(&h)), view(1920, None), view(3840, Some(&h))];
        assert_eq!(
            plan_keyboard_move(KeyAction::NextMonitor, &v, 0, Some(2)),
            Some(KeyboardTarget {
                monitor: 2,
                zone: 2
            })
        );
        let only_empty = [view(0, Some(&h)), view(1920, None)];
        assert_eq!(
            plan_keyboard_move(KeyAction::NextMonitor, &only_empty, 0, Some(1)),
            None
        );
    }

    #[test]
    fn guards_against_missing_monitors_and_layouts() {
        let h = halves();
        let v = [view(0, Some(&h))];
        assert_eq!(plan_keyboard_move(KeyAction::NextZone, &v, 7, None), None);
        assert_eq!(plan_keyboard_move(KeyAction::NextZone, &[], 0, None), None);
        let none = [view(0, None)];
        assert_eq!(
            plan_keyboard_move(KeyAction::NextZone, &none, 0, None),
            None
        );
        assert_eq!(plan_keyboard_move(KeyAction::Zone(1), &none, 0, None), None);
        let empty = Layout {
            id: "e".into(),
            name: "e".into(),
            zones: vec![],
        };
        let e = [view(0, Some(&empty))];
        assert_eq!(plan_keyboard_move(KeyAction::PrevZone, &e, 0, None), None);
        assert_eq!(plan_keyboard_move(KeyAction::Zone(1), &e, 0, None), None);
    }

    #[test]
    fn up_from_a_monitor_without_a_layout_lands_in_the_first_zone() {
        let h = halves();
        let v = [view(0, None), view(1920, Some(&h))];
        assert_eq!(
            plan_keyboard_move(KeyAction::NextMonitor, &v, 0, None),
            Some(KeyboardTarget {
                monitor: 1,
                zone: 1
            })
        );
    }
}
