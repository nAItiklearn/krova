use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::geometry::{PxRect, Rect};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Zone {
    pub id: u32,
    pub rect: Rect,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub id: String,
    pub name: String,
    pub zones: Vec<Zone>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LayoutError {
    #[error("layout has no zones")]
    Empty,
    #[error("duplicate zone id {0}")]
    DuplicateId(u32),
    #[error("zone {0} has an invalid rectangle")]
    InvalidRect(u32),
}

impl Layout {
    pub fn new(id: &str, name: &str, rects: &[Rect]) -> Self {
        Self {
            id: id.to_owned(),
            name: name.to_owned(),
            zones: rects
                .iter()
                .enumerate()
                .map(|(i, r)| Zone {
                    id: i as u32 + 1,
                    rect: *r,
                })
                .collect(),
        }
    }

    pub fn validate(&self) -> Result<(), LayoutError> {
        if self.zones.is_empty() {
            return Err(LayoutError::Empty);
        }
        let mut seen = HashSet::new();
        for z in &self.zones {
            if !seen.insert(z.id) {
                return Err(LayoutError::DuplicateId(z.id));
            }
            if !z.rect.is_valid() {
                return Err(LayoutError::InvalidRect(z.id));
            }
        }
        Ok(())
    }

    /// Zone containing the normalized point. When zones overlap, the smaller
    /// area wins; remaining ties go to the lower id.
    pub fn hit_test(&self, nx: f64, ny: f64) -> Option<u32> {
        self.zones
            .iter()
            .filter(|z| z.rect.contains(nx, ny))
            .min_by(|a, b| {
                a.rect
                    .area()
                    .partial_cmp(&b.rect.area())
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(a.id.cmp(&b.id))
            })
            .map(|z| z.id)
    }

    /// Hit-test a pixel point against this layout on `work`.
    pub fn hit_test_px(&self, work: PxRect, px: i32, py: i32) -> Option<u32> {
        let (nx, ny) = work.normalize(px, py)?;
        self.hit_test(nx, ny)
    }

    pub fn zone_pixels(&self, zone_id: u32, work: PxRect) -> Option<PxRect> {
        self.zones
            .iter()
            .find(|z| z.id == zone_id)
            .map(|z| work.denormalize(z.rect))
    }
}

/// Built-in starter layouts.
pub fn templates() -> Vec<Layout> {
    let third = 1.0 / 3.0;
    vec![
        Layout::new(
            "halves",
            "Halves",
            &[Rect::new(0.0, 0.0, 0.5, 1.0), Rect::new(0.5, 0.0, 0.5, 1.0)],
        ),
        Layout::new(
            "thirds",
            "Thirds",
            &[
                Rect::new(0.0, 0.0, third, 1.0),
                Rect::new(third, 0.0, third, 1.0),
                Rect::new(2.0 * third, 0.0, third, 1.0),
            ],
        ),
        Layout::new(
            "grid",
            "Grid 2x2",
            &[
                Rect::new(0.0, 0.0, 0.5, 0.5),
                Rect::new(0.5, 0.0, 0.5, 0.5),
                Rect::new(0.0, 0.5, 0.5, 0.5),
                Rect::new(0.5, 0.5, 0.5, 0.5),
            ],
        ),
        Layout::new(
            "priority",
            "Priority",
            &[
                Rect::new(0.0, 0.0, 0.25, 1.0),
                Rect::new(0.25, 0.0, 0.5, 1.0),
                Rect::new(0.75, 0.0, 0.25, 1.0),
            ],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_are_valid() {
        for t in templates() {
            t.validate().unwrap_or_else(|e| panic!("{}: {e}", t.id));
        }
    }

    #[test]
    fn hit_test_shared_edge_goes_to_right_zone() {
        let l = &templates()[0];
        assert_eq!(l.hit_test(0.4999, 0.5), Some(1));
        assert_eq!(l.hit_test(0.5, 0.5), Some(2));
        assert_eq!(l.hit_test(1.0, 0.5), None);
    }

    #[test]
    fn overlap_prefers_smaller_zone() {
        let l = Layout::new(
            "o",
            "o",
            &[
                Rect::new(0.0, 0.0, 1.0, 1.0),
                Rect::new(0.25, 0.25, 0.5, 0.5),
            ],
        );
        assert_eq!(l.hit_test(0.5, 0.5), Some(2));
        assert_eq!(l.hit_test(0.1, 0.1), Some(1));
    }

    #[test]
    fn validate_rejects_bad_layouts() {
        let empty = Layout {
            id: "e".into(),
            name: "e".into(),
            zones: vec![],
        };
        assert_eq!(empty.validate(), Err(LayoutError::Empty));
        let mut dup = templates()[0].clone();
        dup.zones[1].id = 1;
        assert_eq!(dup.validate(), Err(LayoutError::DuplicateId(1)));
        let mut bad = templates()[0].clone();
        bad.zones[0].rect = Rect::new(0.9, 0.0, 0.5, 1.0);
        assert_eq!(bad.validate(), Err(LayoutError::InvalidRect(1)));
    }

    #[test]
    fn pixel_hit_and_rect_on_offset_monitor() {
        let work = PxRect::new(1920, 0, 2560, 1400);
        let l = &templates()[0];
        assert_eq!(l.hit_test_px(work, 1920 + 100, 100), Some(1));
        assert_eq!(l.hit_test_px(work, 1920 + 2000, 100), Some(2));
        assert_eq!(l.hit_test_px(work, 10, 10), None);
        assert_eq!(
            l.zone_pixels(2, work),
            Some(PxRect::new(1920 + 1280, 0, 1280, 1400))
        );
        assert_eq!(l.zone_pixels(99, work), None);
    }
}
