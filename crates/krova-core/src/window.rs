//! Pure window-placement math: size-limit clamping, invisible-frame
//! compensation (Windows) and the move animation curve.

use crate::geometry::PxRect;

/// Duration of the animated move, per the spec.
pub const ANIMATION_MS: u32 = 120;

/// Size limits reported by a window; `None` means unconstrained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SizeLimits {
    pub min_w: Option<i32>,
    pub min_h: Option<i32>,
    pub max_w: Option<i32>,
    pub max_h: Option<i32>,
}

fn clamp_axis(target_pos: i32, target_len: i32, min: Option<i32>, max: Option<i32>) -> (i32, i32) {
    let mut len = target_len;
    if let Some(m) = min {
        len = len.max(m);
    }
    if let Some(m) = max {
        len = len.min(m);
    }
    // A window that ended up smaller than its zone is centred in it; one that
    // ended up larger grows from the zone's top/left edge.
    let pos = if len < target_len {
        target_pos + (target_len - len) / 2
    } else {
        target_pos
    };
    (pos, len)
}

/// Fit `target` to the window's size limits and keep it inside `bounds`.
/// All rects must be in the same space (outer window rects).
pub fn clamp_to_limits(target: PxRect, limits: SizeLimits, bounds: PxRect) -> PxRect {
    let (mut x, w) = clamp_axis(target.x, target.w, limits.min_w, limits.max_w);
    let (mut y, h) = clamp_axis(target.y, target.h, limits.min_h, limits.max_h);
    if x + w > bounds.right() {
        x = bounds.right() - w;
    }
    if y + h > bounds.bottom() {
        y = bounds.bottom() - h;
    }
    x = x.max(bounds.x);
    y = y.max(bounds.y);
    PxRect::new(x, y, w, h)
}

/// Difference between a window's outer rect and its visible frame. Windows 10/11
/// windows carry transparent resize borders that must not eat into a zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FrameInsets {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl FrameInsets {
    /// `outer` is `GetWindowRect`, `visible` is DWM's extended frame bounds.
    pub fn between(outer: PxRect, visible: PxRect) -> Self {
        Self {
            left: visible.x - outer.x,
            top: visible.y - outer.y,
            right: outer.right() - visible.right(),
            bottom: outer.bottom() - visible.bottom(),
        }
    }

    /// The outer rect that makes the *visible* frame land exactly on `visible`.
    pub fn outer_for_visible(self, visible: PxRect) -> PxRect {
        PxRect::new(
            visible.x - self.left,
            visible.y - self.top,
            visible.w + self.left + self.right,
            visible.h + self.top + self.bottom,
        )
    }
}

/// Cubic ease-out; `t` is clamped to `0..=1`.
pub fn ease_out(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Rect `elapsed_ms` into an animation from `from` to `to`. Lands exactly on
/// `to` once `ANIMATION_MS` has elapsed.
pub fn animate_step(from: PxRect, to: PxRect, elapsed_ms: u32) -> PxRect {
    if elapsed_ms >= ANIMATION_MS {
        return to;
    }
    let k = ease_out(f64::from(elapsed_ms) / f64::from(ANIMATION_MS));
    let lerp = |a: i32, b: i32| a + (f64::from(b - a) * k).round() as i32;
    PxRect::new(
        lerp(from.x, to.x),
        lerp(from.y, to.y),
        lerp(from.w, to.w),
        lerp(from.h, to.h),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORK: PxRect = PxRect::new(0, 0, 1920, 1040);

    #[test]
    fn unconstrained_window_is_untouched() {
        let t = PxRect::new(0, 0, 960, 1040);
        assert_eq!(clamp_to_limits(t, SizeLimits::default(), WORK), t);
    }

    #[test]
    fn min_size_grows_from_top_left_and_stays_on_screen() {
        let lim = SizeLimits {
            min_w: Some(1200),
            ..Default::default()
        };
        // Right-hand half cannot be narrower than 1200: it grows, then is
        // pushed left so it stays inside the work area.
        let r = clamp_to_limits(PxRect::new(960, 0, 960, 1040), lim, WORK);
        assert_eq!(r, PxRect::new(720, 0, 1200, 1040));
        // Left-hand half just grows.
        let r = clamp_to_limits(PxRect::new(0, 0, 960, 1040), lim, WORK);
        assert_eq!(r, PxRect::new(0, 0, 1200, 1040));
    }

    #[test]
    fn max_size_is_centred_in_the_zone() {
        let lim = SizeLimits {
            max_w: Some(500),
            max_h: Some(400),
            ..Default::default()
        };
        let r = clamp_to_limits(PxRect::new(960, 0, 960, 1040), lim, WORK);
        assert_eq!(r, PxRect::new(960 + 230, 320, 500, 400));
    }

    #[test]
    fn oversized_window_is_pinned_to_the_work_area_origin() {
        let lim = SizeLimits {
            min_w: Some(3000),
            ..Default::default()
        };
        let r = clamp_to_limits(PxRect::new(960, 0, 960, 1040), lim, WORK);
        assert_eq!((r.x, r.w), (0, 3000));
    }

    #[test]
    fn invisible_borders_are_compensated() {
        let outer = PxRect::new(93, 100, 1000, 507);
        let visible = PxRect::new(100, 100, 986, 500);
        let insets = FrameInsets::between(outer, visible);
        assert_eq!(
            insets,
            FrameInsets {
                left: 7,
                top: 0,
                right: 7,
                bottom: 7
            }
        );
        let target = PxRect::new(0, 0, 960, 1040);
        let o = insets.outer_for_visible(target);
        assert_eq!(o, PxRect::new(-7, 0, 974, 1047));
        // Round trip: the visible frame of `o` is exactly `target`.
        assert_eq!(
            PxRect::new(
                o.x + insets.left,
                o.y + insets.top,
                o.w - insets.left - insets.right,
                o.h - insets.top - insets.bottom
            ),
            target
        );
    }

    #[test]
    fn easing_is_monotonic_and_front_loaded() {
        assert_eq!(ease_out(0.0), 0.0);
        assert_eq!(ease_out(1.0), 1.0);
        assert_eq!(ease_out(-3.0), 0.0);
        assert_eq!(ease_out(9.0), 1.0);
        assert!(ease_out(0.5) > 0.5);
        let mut prev = 0.0;
        for i in 1..=20 {
            let v = ease_out(f64::from(i) / 20.0);
            assert!(v >= prev);
            prev = v;
        }
    }

    #[test]
    fn animation_starts_at_from_and_ends_exactly_on_target() {
        let from = PxRect::new(100, 100, 800, 600);
        let to = PxRect::new(0, 0, 960, 1040);
        assert_eq!(animate_step(from, to, 0), from);
        assert_eq!(animate_step(from, to, ANIMATION_MS), to);
        assert_eq!(animate_step(from, to, ANIMATION_MS + 500), to);
        let mid = animate_step(from, to, ANIMATION_MS / 2);
        assert!(mid.x < from.x && mid.x > to.x);
        assert!(mid.w > from.w && mid.w < to.w);
    }
}
