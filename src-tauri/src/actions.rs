//! Pure decisions behind the engine's window features (keyboard moves, window
//! memory, linked dividers), kept free of the OS so they can be unit-tested.

use krova_core::linked::EDGE_TOLERANCE_PX;
use krova_core::registry::{
    monitor_for_rect, plan_keyboard_move, zone_for_rect, KeyAction, SnapRecord,
};
use krova_core::{MonitorView, PxRect};

/// How far (pixels) a window may be from where Krova put it and still count as
/// "still there" when picking the current zone for a keyboard move.
const STILL_THERE_PX: i32 = 8;

/// A window's place in a layout: which zone of which layout of which monitor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    pub profile_id: String,
    pub layout_id: String,
    pub zone_id: u32,
}

impl Slot {
    /// The registry record for a window that now occupies `rect` in this slot.
    pub fn record(self, rect: PxRect) -> SnapRecord {
        SnapRecord {
            monitor_profile_id: self.profile_id,
            layout_id: self.layout_id,
            zone_id: self.zone_id,
            rect,
        }
    }
}

/// Where a keyboard shortcut sends the focused window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyMove {
    /// Visible frame the window should fill.
    pub target: PxRect,
    /// Work area of the destination monitor.
    pub work: PxRect,
    pub slot: Slot,
}

fn edges_within(a: PxRect, b: PxRect, tolerance: i32) -> bool {
    let t = i64::from(tolerance);
    [
        (a.x, b.x),
        (a.y, b.y),
        (a.right(), b.right()),
        (a.bottom(), b.bottom()),
    ]
    .iter()
    .all(|&(p, q)| (i64::from(p) - i64::from(q)).abs() <= t)
}

/// Resolve a shortcut for a window currently occupying `window`.
///
/// `views` and `profile_ids` are the connected monitors in the same order. The
/// window's current zone is what the registry remembers if the window is still
/// where Krova put it, otherwise the zone it overlaps most. Returns `None` when
/// there is nothing to do (no layout, zone number too high, only one monitor).
pub fn plan_key_move(
    action: KeyAction,
    views: &[MonitorView<'_>],
    profile_ids: &[String],
    window: PxRect,
    record: Option<&SnapRecord>,
    gap: i32,
) -> Option<KeyMove> {
    let rects: Vec<PxRect> = views.iter().map(|v| v.rect).collect();
    let from = monitor_for_rect(&rects, window)?;
    let here = views.get(from)?;

    let remembered = record
        .filter(|r| {
            profile_ids.get(from).map(String::as_str) == Some(r.monitor_profile_id.as_str())
                && here.layout.is_some_and(|l| l.id == r.layout_id)
                && edges_within(window, r.rect, STILL_THERE_PX)
        })
        .map(|r| r.zone_id);
    let current = remembered.or_else(|| {
        here.layout
            .and_then(|l| zone_for_rect(l, here.work, window))
    });

    let target = plan_keyboard_move(action, views, from, current)?;
    let view = views.get(target.monitor)?;
    let layout = view.layout?;
    Some(KeyMove {
        target: layout.zone_pixels_gapped(target.zone, view.work, gap)?,
        work: view.work,
        slot: Slot {
            profile_id: profile_ids.get(target.monitor)?.clone(),
            layout_id: layout.id.clone(),
            zone_id: target.zone,
        },
    })
}

/// How a window differs from where Krova left it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// Same place and size (within a few pixels).
    Same,
    /// Same size, different place: the user moved it somewhere else.
    Moved,
    /// The size changed: the user resized it.
    Resized,
}

pub fn classify_change(recorded: PxRect, now: PxRect) -> Change {
    if edges_within(recorded, now, EDGE_TOLERANCE_PX) {
        return Change::Same;
    }
    let tol = i64::from(EDGE_TOLERANCE_PX);
    let size_changed = (i64::from(now.w) - i64::from(recorded.w)).abs() > tol
        || (i64::from(now.h) - i64::from(recorded.h)).abs() > tol;
    if size_changed {
        Change::Resized
    } else {
        Change::Moved
    }
}

/// Whether two rects are the same within the edge tolerance.
pub fn same_place(a: PxRect, b: PxRect) -> bool {
    edges_within(a, b, EDGE_TOLERANCE_PX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use krova_core::registry::KeyAction;
    use krova_core::{templates, Layout};

    fn view(x: i32, layout: Option<&Layout>) -> MonitorView<'_> {
        MonitorView {
            rect: PxRect::new(x, 0, 1920, 1080),
            work: PxRect::new(x, 0, 1920, 1040),
            layout,
        }
    }

    fn ids() -> Vec<String> {
        vec!["monitor-1".to_owned(), "monitor-2".to_owned()]
    }

    #[test]
    fn next_zone_uses_the_geometry_when_nothing_is_remembered() {
        let t = templates();
        let views = [view(0, Some(&t[1]))]; // thirds
        let in_first = PxRect::new(20, 20, 500, 400);
        let m = plan_key_move(KeyAction::NextZone, &views, &ids(), in_first, None, 0).unwrap();
        assert_eq!(m.target, PxRect::new(640, 0, 640, 1040));
        assert_eq!(
            m.slot,
            Slot {
                profile_id: "monitor-1".into(),
                layout_id: "thirds".into(),
                zone_id: 2
            }
        );
        assert_eq!(m.work, views[0].work);
    }

    #[test]
    fn the_remembered_zone_wins_while_the_window_is_still_there() {
        let t = templates();
        let views = [view(0, Some(&t[1]))]; // thirds
                                            // Krova put the window into the right-hand zone (3) but it refused to shrink
                                            // and ended up 1500 px wide: by overlap alone it ties between zones 2 and 3
                                            // and geometry picks the earlier one.
        let wide = PxRect::new(420, 0, 1500, 1040);
        assert_eq!(zone_for_rect(&t[1], views[0].work, wide), Some(2));
        let record = SnapRecord {
            monitor_profile_id: "monitor-1".into(),
            layout_id: "thirds".into(),
            zone_id: 3,
            rect: wide,
        };
        // With the record the window is in zone 3, so "next" wraps around to 1 ...
        let m = plan_key_move(KeyAction::NextZone, &views, &ids(), wide, Some(&record), 0).unwrap();
        assert_eq!(m.slot.zone_id, 1);
        // ... without it the geometry says zone 2, so "next" is zone 3.
        let m = plan_key_move(KeyAction::NextZone, &views, &ids(), wide, None, 0).unwrap();
        assert_eq!(m.slot.zone_id, 3);
    }

    #[test]
    fn a_stale_record_is_ignored() {
        let t = templates();
        let views = [view(0, Some(&t[0]))];
        let record = SnapRecord {
            monitor_profile_id: "monitor-1".into(),
            layout_id: "halves".into(),
            zone_id: 1,
            rect: PxRect::new(0, 0, 960, 1040),
        };
        // The window now sits in the right half: the record is 960 px off.
        let moved = PxRect::new(980, 20, 800, 600);
        let m =
            plan_key_move(KeyAction::NextZone, &views, &ids(), moved, Some(&record), 0).unwrap();
        // Geometry says zone 2, so next wraps to zone 1.
        assert_eq!(m.slot.zone_id, 1);
        // A record for another layout is stale too.
        let other = SnapRecord {
            layout_id: "grid".into(),
            rect: moved,
            ..record
        };
        let m = plan_key_move(KeyAction::NextZone, &views, &ids(), moved, Some(&other), 0).unwrap();
        assert_eq!(m.slot.zone_id, 1);
    }

    #[test]
    fn digits_pick_a_zone_and_out_of_range_does_nothing() {
        let t = templates();
        let views = [view(0, Some(&t[2]))]; // 2x2 grid
        let w = PxRect::new(100, 100, 400, 300);
        let m = plan_key_move(KeyAction::Zone(4), &views, &ids(), w, None, 0).unwrap();
        assert_eq!(m.target, PxRect::new(960, 520, 960, 520));
        assert_eq!(m.slot.zone_id, 4);
        assert_eq!(
            plan_key_move(KeyAction::Zone(5), &views, &ids(), w, None, 0),
            None
        );
        assert_eq!(
            plan_key_move(KeyAction::Zone(9), &views, &ids(), w, None, 0),
            None
        );
    }

    #[test]
    fn up_moves_to_the_other_monitor_keeping_the_zone_number() {
        let t = templates();
        let views = [view(0, Some(&t[0])), view(1920, Some(&t[1]))];
        let in_right_half = PxRect::new(1000, 100, 500, 400);
        let m = plan_key_move(
            KeyAction::NextMonitor,
            &views,
            &ids(),
            in_right_half,
            None,
            0,
        )
        .unwrap();
        assert_eq!(m.slot.profile_id, "monitor-2");
        assert_eq!(m.slot.layout_id, "thirds");
        assert_eq!(m.slot.zone_id, 2);
        assert_eq!(m.target, PxRect::new(1920 + 640, 0, 640, 1040));
        // One monitor only: nothing to do.
        assert_eq!(
            plan_key_move(
                KeyAction::NextMonitor,
                &views[..1],
                &ids(),
                in_right_half,
                None,
                0
            ),
            None
        );
    }

    #[test]
    fn the_gap_setting_shapes_the_target() {
        let t = templates();
        let views = [view(0, Some(&t[0]))];
        let w = PxRect::new(100, 100, 400, 300);
        let m = plan_key_move(KeyAction::Zone(1), &views, &ids(), w, None, 10).unwrap();
        assert_eq!(m.target, PxRect::new(10, 10, 945, 1020));
    }

    #[test]
    fn no_layout_or_no_monitors_means_no_move() {
        let w = PxRect::new(100, 100, 400, 300);
        let views = [view(0, None)];
        assert_eq!(
            plan_key_move(KeyAction::NextZone, &views, &ids(), w, None, 0),
            None
        );
        assert_eq!(
            plan_key_move(KeyAction::NextZone, &[], &ids(), w, None, 0),
            None
        );
    }

    #[test]
    fn a_missing_profile_id_is_not_a_panic() {
        let t = templates();
        let views = [view(0, Some(&t[0]))];
        let w = PxRect::new(100, 100, 400, 300);
        assert_eq!(
            plan_key_move(KeyAction::Zone(1), &views, &[], w, None, 0),
            None
        );
    }

    #[test]
    fn changes_are_classified_against_the_recorded_rect() {
        let r = PxRect::new(100, 100, 800, 600);
        assert_eq!(classify_change(r, r), Change::Same);
        assert_eq!(
            classify_change(r, PxRect::new(102, 99, 801, 601)),
            Change::Same
        );
        assert_eq!(
            classify_change(r, PxRect::new(300, 200, 800, 600)),
            Change::Moved
        );
        assert_eq!(
            classify_change(r, PxRect::new(100, 100, 950, 600)),
            Change::Resized
        );
        assert_eq!(
            classify_change(r, PxRect::new(40, 100, 860, 600)),
            Change::Resized
        );
        // Moved and resized at once counts as resized.
        assert_eq!(
            classify_change(r, PxRect::new(300, 200, 400, 300)),
            Change::Resized
        );
        assert!(same_place(r, PxRect::new(103, 100, 800, 600)));
        assert!(!same_place(r, PxRect::new(110, 100, 800, 600)));
    }
}
