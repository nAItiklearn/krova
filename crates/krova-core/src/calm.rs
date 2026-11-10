//! Calm desk (wave 3): minimise every window that is not in a zone of its
//! monitor's active layout; a second toggle restores exactly those still minimised.
//!
//! The memory of what was minimised lives in RAM only (the app keeps a
//! [`CalmDesk`]); nothing is written anywhere.

use std::collections::HashSet;

use crate::geometry::PxRect;
use crate::registry::select::monitor_for_rect;
use crate::zone::Layout;

/// How far (in pixels, per edge) a window may be off its zone and still count as
/// "in the zone": windows that refuse a size or have odd frames end up a little off.
pub const IN_ZONE_TOLERANCE_PX: i32 = 12;

/// A visible, arrangeable window right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalmWindow {
    pub id: u64,
    pub rect: PxRect,
    /// The snap registry says Krova put it into a zone of its monitor's active layout.
    pub snapped: bool,
}

/// A connected monitor with its active layout and the zone gap in effect.
#[derive(Debug, Clone, Copy)]
pub struct CalmMonitor<'a> {
    pub rect: PxRect,
    pub work: PxRect,
    pub layout: Option<&'a Layout>,
    pub gap: i32,
    /// Whether this monitor takes part (all monitors, or only the one under the cursor).
    pub included: bool,
}

fn near(a: PxRect, b: PxRect, tol: i32) -> bool {
    (a.x - b.x).abs() <= tol
        && (a.y - b.y).abs() <= tol
        && (a.right() - b.right()).abs() <= tol
        && (a.bottom() - b.bottom()).abs() <= tol
}

/// Whether a window sits in one of the zones of its monitor's layout (plain or
/// gapped rects, within [`IN_ZONE_TOLERANCE_PX`]).
pub fn in_a_zone(rect: PxRect, monitor: &CalmMonitor<'_>) -> bool {
    let Some(layout) = monitor.layout else {
        return false;
    };
    layout.zones.iter().any(|z| {
        let plain = layout.zone_pixels(z.id, monitor.work);
        let gapped = layout.zone_pixels_gapped(z.id, monitor.work, monitor.gap);
        [plain, gapped]
            .into_iter()
            .flatten()
            .any(|zr| near(rect, zr, IN_ZONE_TOLERANCE_PX))
    })
}

/// The windows calm desk minimises: on an included monitor, not snapped by Krova
/// and not sitting in a zone. Windows of monitors without a layout are left alone.
pub fn calm_targets(windows: &[CalmWindow], monitors: &[CalmMonitor<'_>]) -> Vec<u64> {
    let rects: Vec<PxRect> = monitors.iter().map(|m| m.rect).collect();
    windows
        .iter()
        .filter(|w| {
            let Some(i) = monitor_for_rect(&rects, w.rect) else {
                return false;
            };
            let m = &monitors[i];
            m.included && m.layout.is_some() && !w.snapped && !in_a_zone(w.rect, m)
        })
        .map(|w| w.id)
        .collect()
}

/// The in-memory toggle state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CalmDesk {
    /// Windows calm desk minimised, in the order it minimised them. Empty = off.
    minimized: Vec<u64>,
    active: bool,
}

impl CalmDesk {
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// How many windows are held minimised.
    pub fn count(&self) -> usize {
        self.minimized.len()
    }

    /// Remember the windows that were really minimised and switch on.
    pub fn engage(&mut self, minimized: Vec<u64>) {
        self.minimized = minimized;
        self.active = true;
    }

    /// Switch off. Returns the windows to restore: those calm desk minimised that
    /// are still minimised now (`still_minimized`), in reverse order so the first
    /// minimised comes back last (on top).
    pub fn release(&mut self, still_minimized: impl Fn(u64) -> bool) -> Vec<u64> {
        self.active = false;
        let mut seen: HashSet<u64> = HashSet::new();
        let mut out: Vec<u64> = std::mem::take(&mut self.minimized)
            .into_iter()
            .filter(|&id| seen.insert(id) && still_minimized(id))
            .collect();
        out.reverse();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Rect;

    fn halves() -> Layout {
        Layout::new(
            "halves",
            "Halves",
            &[Rect::new(0.0, 0.0, 0.5, 1.0), Rect::new(0.5, 0.0, 0.5, 1.0)],
        )
    }

    #[test]
    fn only_windows_outside_the_zones_are_targets() {
        let layout = halves();
        let m = PxRect::new(0, 0, 2000, 1000);
        let monitors = [CalmMonitor {
            rect: m,
            work: m,
            layout: Some(&layout),
            gap: 8,
            included: true,
        }];
        let w = |id, rect, snapped| CalmWindow { id, rect, snapped };
        let windows = [
            w(1, PxRect::new(0, 0, 1000, 1000), false), // left zone exactly
            w(2, PxRect::new(1008, 8, 984, 984), false), // right zone, gapped
            w(3, PxRect::new(1005, 3, 990, 995), false), // right zone, a few px off
            w(4, PxRect::new(300, 200, 800, 500), false), // floating
            w(5, PxRect::new(300, 200, 800, 500), true), // floating but snapped by Krova
        ];
        assert_eq!(calm_targets(&windows, &monitors), vec![4]);

        let excluded = [CalmMonitor {
            included: false,
            ..monitors[0]
        }];
        assert!(calm_targets(&windows, &excluded).is_empty());
        let no_layout = [CalmMonitor {
            layout: None,
            ..monitors[0]
        }];
        assert!(calm_targets(&windows, &no_layout).is_empty());
    }

    #[test]
    fn the_second_toggle_restores_only_what_is_still_minimised() {
        let mut calm = CalmDesk::default();
        assert!(!calm.is_active());
        calm.engage(vec![4, 7, 9]);
        assert!(calm.is_active());
        assert_eq!(calm.count(), 3);
        // The user restored 7 by hand meanwhile.
        let back = calm.release(|id| id != 7);
        assert_eq!(back, vec![9, 4]);
        assert!(!calm.is_active());
        assert_eq!(calm.count(), 0);
        assert!(calm.release(|_| true).is_empty());
    }
}
