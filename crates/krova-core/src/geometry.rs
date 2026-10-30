use serde::{Deserialize, Serialize};

/// Rectangle in normalized (0..=1) monitor work-area coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    pub fn area(&self) -> f64 {
        self.w * self.h
    }

    /// Half-open containment: `[x, x+w) x [y, y+h)`.
    pub fn contains(&self, px: f64, py: f64) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }

    /// Finite, positive size, and inside the unit square (with float tolerance).
    pub fn is_valid(&self) -> bool {
        const EPS: f64 = 1e-9;
        [self.x, self.y, self.w, self.h]
            .iter()
            .all(|v| v.is_finite())
            && self.w > 0.0
            && self.h > 0.0
            && self.x >= -EPS
            && self.y >= -EPS
            && self.x + self.w <= 1.0 + EPS
            && self.y + self.h <= 1.0 + EPS
    }
}

/// Rectangle in physical pixels (virtual-screen coordinates).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PxRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl PxRect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    /// Right edge; saturates instead of overflowing on absurd rects from the OS.
    pub fn right(&self) -> i32 {
        self.x.saturating_add(self.w)
    }

    /// Bottom edge; saturates instead of overflowing on absurd rects from the OS.
    pub fn bottom(&self) -> i32 {
        self.y.saturating_add(self.h)
    }

    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && px < self.right() && py >= self.y && py < self.bottom()
    }

    /// Map a pixel point into normalized coordinates of this rect.
    pub fn normalize(&self, px: i32, py: i32) -> Option<(f64, f64)> {
        if self.w <= 0 || self.h <= 0 || !self.contains(px, py) {
            return None;
        }
        Some((
            f64::from(px - self.x) / f64::from(self.w),
            f64::from(py - self.y) / f64::from(self.h),
        ))
    }

    /// Map a normalized rect into this pixel rect. Edges are rounded
    /// individually so adjacent zones tile with no gaps or overlaps.
    pub fn denormalize(&self, r: Rect) -> PxRect {
        let edge = |origin: i32, size: i32, n: f64| origin + (n * f64::from(size)).round() as i32;
        let left = edge(self.x, self.w, r.x);
        let top = edge(self.y, self.h, r.y);
        let right = edge(self.x, self.w, r.x + r.w);
        let bottom = edge(self.y, self.h, r.y + r.h);
        PxRect::new(left, top, right - left, bottom - top)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_open_contains() {
        let r = Rect::new(0.0, 0.0, 0.5, 1.0);
        assert!(r.contains(0.0, 0.0));
        assert!(!r.contains(0.5, 0.5));
    }

    #[test]
    fn validity() {
        assert!(Rect::new(0.0, 0.0, 1.0, 1.0).is_valid());
        assert!(!Rect::new(0.5, 0.0, 0.6, 1.0).is_valid());
        assert!(!Rect::new(0.0, 0.0, 0.0, 1.0).is_valid());
        assert!(!Rect::new(f64::NAN, 0.0, 1.0, 1.0).is_valid());
    }

    #[test]
    fn denormalize_tiles_without_gaps() {
        let work = PxRect::new(100, 50, 1001, 777);
        let a = work.denormalize(Rect::new(0.0, 0.0, 1.0 / 3.0, 1.0));
        let b = work.denormalize(Rect::new(1.0 / 3.0, 0.0, 1.0 / 3.0, 1.0));
        let c = work.denormalize(Rect::new(2.0 / 3.0, 0.0, 1.0 / 3.0, 1.0));
        assert_eq!(a.right(), b.x);
        assert_eq!(b.right(), c.x);
        assert_eq!(c.right(), work.right());
        assert_eq!(a.x, work.x);
    }

    #[test]
    fn normalize_outside_is_none() {
        let work = PxRect::new(-1920, 0, 1920, 1080);
        assert_eq!(work.normalize(-1920, 0), Some((0.0, 0.0)));
        assert_eq!(work.normalize(0, 0), None);
        assert_eq!(work.normalize(-960, 540), Some((0.5, 0.5)));
    }

    #[test]
    fn absurd_rects_saturate_instead_of_overflowing() {
        let r = PxRect::new(i32::MAX - 10, i32::MAX - 10, 1000, 1000);
        assert_eq!(r.right(), i32::MAX);
        assert_eq!(r.bottom(), i32::MAX);
        assert!(!r.contains(0, 0));
        let r = PxRect::new(i32::MIN, i32::MIN, -1000, -1000);
        assert_eq!(r.right(), i32::MIN);
        assert!(!r.contains(i32::MIN, i32::MIN));
    }
}
