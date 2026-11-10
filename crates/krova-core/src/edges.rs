//! Reserved edges: strips along the sides of a monitor that Krova leaves alone,
//! for third-party docks and bars that do not register themselves with the OS
//! (an app bar that does register already shrinks the OS work area).
//!
//! Units: each side is in **logical pixels** (96 dpi pixels). It is multiplied by
//! the monitor's scale (device pixels per logical pixel) when it is applied, so
//! "40" means the same physical size on a 100 % and a 200 % monitor. The result is
//! subtracted from the OS work area of that monitor, and that reduced work area is
//! the one zones, snapping (zone and edge), keyboard moves and the reconnect
//! restore all use.
//!
//! Limits: every side is clamped to `0..=MAX_RESERVED_PX`. If the two sides of an
//! axis would leave less than [`MIN_USABLE_PX`] physical pixels of that axis, the
//! reserves of that axis are ignored (the other axis still applies), so a
//! nonsensical reserve can never make the screen unusable.

use serde::{Deserialize, Serialize};

use crate::geometry::PxRect;

/// Largest reserve on one side, in logical pixels.
pub const MAX_RESERVED_PX: u32 = 400;
/// Smallest width or height (physical pixels) a work area keeps after reserving.
pub const MIN_USABLE_PX: i32 = 200;

/// Logical pixels reserved on each side of one monitor.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "RawEdges")]
pub struct ReservedEdges {
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub left: u32,
}

/// What the config file may hold: any number (negative, fractional, huge) is
/// accepted and clamped, so a hand-edited value never makes the file unreadable.
#[derive(Deserialize, Default)]
#[serde(default)]
struct RawEdges {
    top: f64,
    right: f64,
    bottom: f64,
    left: f64,
}

fn clamp_side(v: f64) -> u32 {
    if v.is_finite() {
        // Clamped to 0..=400 first, so the cast is exact.
        v.round().clamp(0.0, f64::from(MAX_RESERVED_PX)) as u32
    } else {
        0
    }
}

impl From<RawEdges> for ReservedEdges {
    fn from(r: RawEdges) -> Self {
        Self {
            top: clamp_side(r.top),
            right: clamp_side(r.right),
            bottom: clamp_side(r.bottom),
            left: clamp_side(r.left),
        }
    }
}

/// A usable scale factor: finite and within what real monitors report.
fn sane_scale(scale: f64) -> f64 {
    if scale.is_finite() && scale > 0.0 {
        scale.clamp(0.5, 8.0)
    } else {
        1.0
    }
}

fn to_physical(logical: u32, scale: f64) -> i32 {
    // At most 400 * 8 = 3200: always fits.
    (f64::from(logical) * scale).round() as i32
}

impl ReservedEdges {
    pub const fn new(top: u32, right: u32, bottom: u32, left: u32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    pub fn is_zero(&self) -> bool {
        *self == Self::default()
    }

    /// Every side clamped to `0..=MAX_RESERVED_PX`.
    pub fn sanitized(self) -> Self {
        let c = |v: u32| v.min(MAX_RESERVED_PX);
        Self::new(c(self.top), c(self.right), c(self.bottom), c(self.left))
    }

    /// The work area that is left once these edges are reserved, on a monitor
    /// with `scale` device pixels per logical pixel. See the module docs.
    pub fn apply(&self, work: PxRect, scale: f64) -> PxRect {
        let e = self.sanitized();
        if e.is_zero() || work.w <= 0 || work.h <= 0 {
            return work;
        }
        let scale = sane_scale(scale);
        let (mut x, mut y, mut w, mut h) = (work.x, work.y, work.w, work.h);
        let (left, right) = (to_physical(e.left, scale), to_physical(e.right, scale));
        if work.w - left - right >= MIN_USABLE_PX {
            x = work.x.saturating_add(left);
            w = work.w - left - right;
        }
        let (top, bottom) = (to_physical(e.top, scale), to_physical(e.bottom, scale));
        if work.h - top - bottom >= MIN_USABLE_PX {
            y = work.y.saturating_add(top);
            h = work.h - top - bottom;
        }
        PxRect::new(x, y, w, h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORK: PxRect = PxRect::new(0, 0, 1920, 1040);

    #[test]
    fn nothing_reserved_keeps_the_work_area() {
        assert_eq!(ReservedEdges::default().apply(WORK, 1.0), WORK);
        assert_eq!(ReservedEdges::default().apply(WORK, 2.0), WORK);
    }

    #[test]
    fn each_side_is_cut_off_at_100_percent() {
        let e = ReservedEdges::new(10, 20, 30, 40);
        assert_eq!(e.apply(WORK, 1.0), PxRect::new(40, 10, 1860, 1000));
    }

    #[test]
    fn logical_pixels_are_scaled_by_the_monitor_scale() {
        let e = ReservedEdges::new(0, 0, 0, 48);
        let work = PxRect::new(-3840, 0, 3840, 2100);
        assert_eq!(
            e.apply(work, 2.0),
            PxRect::new(-3840 + 96, 0, 3840 - 96, 2100)
        );
        // 150 %: 48 * 1.5 = 72.
        assert_eq!(e.apply(WORK, 1.5).x, 72);
        // 125 %, rounded: 10 * 1.25 = 12.5 -> 13.
        assert_eq!(ReservedEdges::new(10, 0, 0, 0).apply(WORK, 1.25).y, 13);
    }

    #[test]
    fn absurd_scales_fall_back_or_are_clamped() {
        let e = ReservedEdges::new(0, 0, 0, 10);
        assert_eq!(e.apply(WORK, f64::NAN).x, 10);
        assert_eq!(e.apply(WORK, -2.0).x, 10);
        assert_eq!(e.apply(WORK, 1000.0).x, 80);
    }

    #[test]
    fn sides_are_clamped_to_400() {
        let e = ReservedEdges::new(9999, 401, 400, 0).sanitized();
        assert_eq!(e, ReservedEdges::new(400, 400, 400, 0));
        // Unsanitized input is clamped by `apply` too.
        let tall = PxRect::new(0, 0, 3000, 3000);
        assert_eq!(ReservedEdges::new(9999, 0, 0, 0).apply(tall, 1.0).y, 400);
    }

    #[test]
    fn reserves_larger_than_the_screen_are_ignored_per_axis() {
        let small = PxRect::new(0, 0, 800, 600);
        // 400 + 400 at 100 % leaves 0 px horizontally: ignored; vertical applies.
        let e = ReservedEdges::new(50, 400, 0, 400);
        assert_eq!(e.apply(small, 1.0), PxRect::new(0, 50, 800, 550));
        // At 200 % even one 400 px side is wider than the monitor.
        let e = ReservedEdges::new(400, 0, 0, 0);
        assert_eq!(e.apply(small, 2.0), small);
        // Exactly the minimum left is still applied.
        let e = ReservedEdges::new(0, 300, 0, 300);
        assert_eq!(e.apply(small, 1.0).w, MIN_USABLE_PX);
    }

    #[test]
    fn degenerate_work_areas_are_returned_unchanged() {
        let e = ReservedEdges::new(10, 10, 10, 10);
        let empty = PxRect::new(5, 5, 0, 0);
        assert_eq!(e.apply(empty, 1.0), empty);
    }

    #[test]
    fn the_file_format_is_lenient_and_clamps() {
        let e: ReservedEdges =
            serde_json::from_str(r#"{"top": -5, "right": 12.6, "bottom": 1e9}"#).unwrap();
        assert_eq!(e, ReservedEdges::new(0, 13, 400, 0));
        let e: ReservedEdges = serde_json::from_str("{}").unwrap();
        assert!(e.is_zero());
        let json = serde_json::to_string(&ReservedEdges::new(1, 2, 3, 4)).unwrap();
        assert_eq!(json, r#"{"top":1,"right":2,"bottom":3,"left":4}"#);
        assert!(serde_json::from_str::<ReservedEdges>(r#"{"top":"x"}"#).is_err());
    }

    #[test]
    fn profiles_load_without_reserves_and_with_clamped_ones() {
        use crate::config::Config;
        use crate::monitor::MonitorDescriptor;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut cfg = Config::default();
        cfg.resolve_monitors(&[MonitorDescriptor {
            hardware_id: Some("A".into()),
            device_path: None,
            rect: WORK,
        }]);
        assert!(cfg.monitors[0].reserved.is_zero());
        let mut value = serde_json::to_value(&cfg).unwrap();
        // A v0.1 profile has no `reserved` key at all.
        value["monitors"][0]
            .as_object_mut()
            .unwrap()
            .remove("reserved");
        std::fs::write(&path, value.to_string()).unwrap();
        let (loaded, _) = Config::load(&path).unwrap();
        assert!(loaded.monitors[0].reserved.is_zero());
        value["monitors"][0]["reserved"] = serde_json::json!({"left": 5000, "top": 24});
        std::fs::write(&path, value.to_string()).unwrap();
        let (loaded, _) = Config::load(&path).unwrap();
        assert_eq!(
            loaded.monitors[0].reserved,
            ReservedEdges::new(24, 0, 0, 400)
        );
    }
}
