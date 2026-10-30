use crate::geometry::{PxRect, Rect};

/// Result of edge snapping: the normalized region of the monitor work area
/// the dragged window should fill.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeSnap {
    pub region: Rect,
}

/// Decide whether a cursor at `cursor` should edge-snap on `monitor`.
///
/// A monitor side only counts if no other monitor touches it at the cursor's
/// position, so the border between two screens never triggers a snap.
/// Left/right edges give halves, the top edge maximizes, corners give quarters.
/// A lone bottom edge does nothing (it is where the taskbar/dock lives).
pub fn edge_snap(
    cursor: (i32, i32),
    monitor: PxRect,
    others: &[PxRect],
    edge: i32,
) -> Option<EdgeSnap> {
    let (cx, cy) = cursor;
    if edge <= 0 || !monitor.contains(cx, cy) {
        return None;
    }
    let internal = |px: i32, py: i32| others.iter().any(|o| o.contains(px, py));

    let left = cx - monitor.x < edge && !internal(monitor.x - 1, cy);
    let right = monitor.right() - 1 - cx < edge && !internal(monitor.right(), cy);
    let top = cy - monitor.y < edge && !internal(cx, monitor.y - 1);
    let bottom = monitor.bottom() - 1 - cy < edge && !internal(cx, monitor.bottom());

    let region = match (left, right, top, bottom) {
        (true, false, true, _) => Rect::new(0.0, 0.0, 0.5, 0.5),
        (false, true, true, _) => Rect::new(0.5, 0.0, 0.5, 0.5),
        (true, false, false, true) => Rect::new(0.0, 0.5, 0.5, 0.5),
        (false, true, false, true) => Rect::new(0.5, 0.5, 0.5, 0.5),
        (true, false, false, false) => Rect::new(0.0, 0.0, 0.5, 1.0),
        (false, true, false, false) => Rect::new(0.5, 0.0, 0.5, 1.0),
        (false, false, true, _) => Rect::new(0.0, 0.0, 1.0, 1.0),
        _ => return None,
    };
    Some(EdgeSnap { region })
}

#[cfg(test)]
mod tests {
    use super::*;

    const M: PxRect = PxRect::new(0, 0, 1920, 1080);

    #[test]
    fn single_monitor_edges() {
        assert_eq!(
            edge_snap((2, 500), M, &[], 8).unwrap().region,
            Rect::new(0.0, 0.0, 0.5, 1.0)
        );
        assert_eq!(
            edge_snap((1918, 500), M, &[], 8).unwrap().region,
            Rect::new(0.5, 0.0, 0.5, 1.0)
        );
        assert_eq!(
            edge_snap((900, 1), M, &[], 8).unwrap().region,
            Rect::new(0.0, 0.0, 1.0, 1.0)
        );
        assert_eq!(edge_snap((900, 1078), M, &[], 8), None);
        assert_eq!(edge_snap((900, 500), M, &[], 8), None);
    }

    #[test]
    fn corners_give_quarters() {
        assert_eq!(
            edge_snap((1, 1), M, &[], 8).unwrap().region,
            Rect::new(0.0, 0.0, 0.5, 0.5)
        );
        assert_eq!(
            edge_snap((1919, 1), M, &[], 8).unwrap().region,
            Rect::new(0.5, 0.0, 0.5, 0.5)
        );
        assert_eq!(
            edge_snap((1, 1079), M, &[], 8).unwrap().region,
            Rect::new(0.0, 0.5, 0.5, 0.5)
        );
        assert_eq!(
            edge_snap((1919, 1079), M, &[], 8).unwrap().region,
            Rect::new(0.5, 0.5, 0.5, 0.5)
        );
    }

    #[test]
    fn shared_border_between_screens_never_triggers() {
        let right_neighbour = [PxRect::new(1920, 0, 1920, 1080)];
        // Right edge of the left monitor is internal.
        assert_eq!(edge_snap((1919, 500), M, &right_neighbour, 8), None);
        // But its far-left edge still works.
        assert!(edge_snap((0, 500), M, &right_neighbour, 8).is_some());
        // And the neighbour's left edge is internal too.
        let n = right_neighbour[0];
        assert_eq!(edge_snap((1920, 500), n, &[M], 8), None);
    }

    #[test]
    fn top_edge_next_to_a_monitor_above_is_internal() {
        let above = [PxRect::new(0, -1080, 1920, 1080)];
        assert_eq!(edge_snap((900, 0), M, &above, 8), None);
    }

    #[test]
    fn staggered_monitors_only_suppress_the_touching_part() {
        // Neighbour only covers the top half of the shared border.
        let n = [PxRect::new(1920, 0, 1920, 540)];
        assert_eq!(edge_snap((1919, 100), M, &n, 8), None);
        assert!(edge_snap((1919, 800), M, &n, 8).is_some());
    }

    #[test]
    fn outside_or_disabled() {
        assert_eq!(edge_snap((-5, 5), M, &[], 8), None);
        assert_eq!(edge_snap((1, 1), M, &[], 0), None);
    }
}
