use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::geometry::{PxRect, Rect};

/// Most zones a layout may have. Bounds the work done per mouse move and per
/// overlay repaint, and keeps a corrupt or hostile config from exhausting memory.
pub const MAX_ZONES: usize = 64;
/// Longest layout id or name, in characters.
pub const MAX_NAME_LEN: usize = 64;
/// Smallest side a stored zone may have, as a fraction of the work area. The
/// editor enforces a larger minimum; this only rejects degenerate stored data.
pub const MIN_ZONE_SIDE: f64 = 0.01;

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
    #[error("layout has too many zones (at most 64)")]
    TooManyZones,
    #[error("layout id or name is too long (at most 64 characters)")]
    TextTooLong,
    #[error("the layout id or name has control, invisible or direction-changing characters")]
    HiddenCharacter,
}

/// Whether `s` holds a control, zero-width, line-separating or direction-changing
/// character (see `app_rules`; the rule is shared).
pub fn has_hidden_char(s: &str) -> bool {
    s.chars().any(crate::app_rules::is_hidden)
}

/// `s` without its hidden characters.
pub fn strip_hidden(s: &str) -> String {
    s.chars()
        .filter(|&c| !crate::app_rules::is_hidden(c))
        .collect()
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
        if self.zones.len() > MAX_ZONES {
            return Err(LayoutError::TooManyZones);
        }
        let too_long = |s: &str| s.chars().take(MAX_NAME_LEN + 1).count() > MAX_NAME_LEN;
        if too_long(&self.id) || too_long(&self.name) {
            return Err(LayoutError::TextTooLong);
        }
        // Invisible and direction-changing characters make two names look alike or
        // reorder what the user reads. Same rule as the app rules' program names.
        if has_hidden_char(&self.id) || has_hidden_char(&self.name) {
            return Err(LayoutError::HiddenCharacter);
        }
        let mut seen = HashSet::new();
        for z in &self.zones {
            if !seen.insert(z.id) {
                return Err(LayoutError::DuplicateId(z.id));
            }
            if !z.rect.is_valid() || z.rect.w < MIN_ZONE_SIDE || z.rect.h < MIN_ZONE_SIDE {
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

    /// Like [`Layout::zone_pixels`] but leaves `gap` pixels between neighbouring
    /// zones and between a zone and the edge of `work`. A gap of 0 (or less) gives
    /// exactly `zone_pixels`. Odd gaps are split so seams and margins are exactly
    /// `gap` wide; a zone never shrinks below 1 pixel.
    pub fn zone_pixels_gapped(&self, zone_id: u32, work: PxRect, gap: i32) -> Option<PxRect> {
        let gap = gap.clamp(0, crate::config::MAX_ZONE_GAP_PX);
        if gap == 0 {
            return self.zone_pixels(zone_id, work);
        }
        let lo = gap / 2;
        let hi = gap - lo;
        // Margin around the screen first, then each zone gives up half a gap on
        // every side (`hi` on the left/top, `lo` on the right/bottom).
        let inner = PxRect::new(
            work.x.saturating_add(lo),
            work.y.saturating_add(lo),
            work.w.saturating_sub(gap).max(1),
            work.h.saturating_sub(gap).max(1),
        );
        let r = self.zone_pixels(zone_id, inner)?;
        Some(PxRect::new(
            r.x.saturating_add(hi),
            r.y.saturating_add(hi),
            r.w.saturating_sub(gap).max(1),
            r.h.saturating_sub(gap).max(1),
        ))
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

    #[test]
    fn limits_in_the_messages_match_the_constants() {
        assert_eq!(MAX_ZONES, 64);
        assert_eq!(MAX_NAME_LEN, 64);
        assert!(LayoutError::TooManyZones.to_string().contains("64"));
        assert!(LayoutError::TextTooLong.to_string().contains("64"));
    }

    #[test]
    fn too_many_zones_are_refused() {
        let step = 1.0 / 64.0;
        let rects: Vec<Rect> = (0..=MAX_ZONES)
            .map(|i| Rect::new(i as f64 * step, 0.0, step, 1.0))
            .collect();
        assert_eq!(rects.len(), MAX_ZONES + 1);
        let too_many = Layout::new("many", "Many", &rects);
        assert_eq!(too_many.validate(), Err(LayoutError::TooManyZones));
        let exactly = Layout::new("max", "Max", &rects[..MAX_ZONES]);
        assert_eq!(exactly.validate(), Ok(()));
    }

    #[test]
    fn overlong_text_is_refused_but_long_unicode_within_the_limit_is_fine() {
        let mut l = templates()[0].clone();
        l.name = "x".repeat(MAX_NAME_LEN);
        assert_eq!(l.validate(), Ok(()));
        l.name = "x".repeat(MAX_NAME_LEN + 1);
        assert_eq!(l.validate(), Err(LayoutError::TextTooLong));
        l.name = "é".repeat(MAX_NAME_LEN);
        assert_eq!(l.validate(), Ok(()));
        l.name = "ok".to_owned();
        l.id = "y".repeat(MAX_NAME_LEN + 1);
        assert_eq!(l.validate(), Err(LayoutError::TextTooLong));
    }

    #[test]
    fn a_zero_gap_is_exactly_the_plain_zone_rect() {
        let l = &templates()[2];
        let work = PxRect::new(100, 50, 1001, 777);
        for z in &l.zones {
            assert_eq!(
                l.zone_pixels_gapped(z.id, work, 0),
                l.zone_pixels(z.id, work)
            );
            assert_eq!(
                l.zone_pixels_gapped(z.id, work, -4),
                l.zone_pixels(z.id, work)
            );
        }
        assert_eq!(l.zone_pixels_gapped(99, work, 8), None);
    }

    #[test]
    fn gaps_are_exact_between_zones_and_around_the_edge() {
        let l = &templates()[0]; // halves
        let work = PxRect::new(0, 0, 1920, 1040);
        let a = l.zone_pixels_gapped(1, work, 10).unwrap();
        let b = l.zone_pixels_gapped(2, work, 10).unwrap();
        assert_eq!(a, PxRect::new(10, 10, 945, 1020));
        assert_eq!(b, PxRect::new(965, 10, 945, 1020));
        assert_eq!(b.x - a.right(), 10, "seam between the zones");
        assert_eq!(a.x - work.x, 10, "left margin");
        assert_eq!(work.right() - b.right(), 10, "right margin");
        assert_eq!(a.y - work.y, 10, "top margin");
        assert_eq!(work.bottom() - a.bottom(), 10, "bottom margin");
    }

    #[test]
    fn odd_gaps_still_add_up_exactly() {
        let l = &templates()[2]; // 2x2 grid
        let work = PxRect::new(-1920, 20, 1920, 1000);
        for gap in [1, 3, 7, 31, 32] {
            let z: Vec<PxRect> = (1..=4)
                .map(|i| l.zone_pixels_gapped(i, work, gap).unwrap())
                .collect();
            assert_eq!(z[1].x - z[0].right(), gap, "horizontal seam, gap {gap}");
            assert_eq!(z[2].y - z[0].bottom(), gap, "vertical seam, gap {gap}");
            assert_eq!(z[0].x - work.x, gap, "left margin, gap {gap}");
            assert_eq!(z[0].y - work.y, gap, "top margin, gap {gap}");
            assert_eq!(work.right() - z[3].right(), gap, "right margin, gap {gap}");
            assert_eq!(
                work.bottom() - z[3].bottom(),
                gap,
                "bottom margin, gap {gap}"
            );
        }
    }

    #[test]
    fn a_huge_gap_is_clamped_and_never_collapses_a_zone() {
        let l = &templates()[0];
        let work = PxRect::new(0, 0, 40, 30);
        let z = l.zone_pixels_gapped(1, work, i32::MAX).unwrap();
        assert!(z.w >= 1 && z.h >= 1);
        let same = l.zone_pixels_gapped(1, work, crate::config::MAX_ZONE_GAP_PX);
        assert_eq!(Some(z), same, "gaps above the maximum count as the maximum");
    }

    #[test]
    fn degenerate_slivers_are_refused() {
        let mut l = templates()[0].clone();
        l.zones[0].rect = Rect::new(0.0, 0.0, 0.005, 1.0);
        assert_eq!(l.validate(), Err(LayoutError::InvalidRect(1)));
        l.zones[0].rect = Rect::new(0.0, 0.0, MIN_ZONE_SIDE, 1.0);
        assert_eq!(l.validate(), Ok(()));
    }

    #[test]
    fn invisible_and_direction_changing_characters_in_ids_and_names_are_refused() {
        // Right-to-left override, zero-width space, line separator, byte order mark,
        // an isolate, a soft hyphen and a plain control character.
        for bad in [
            "\u{202E}evil",
            "a\u{200B}b",
            "line\u{2028}break",
            "\u{FEFF}x",
            "a\u{2066}b",
            "co\u{00AD}op",
            "tab\there",
            "nul\u{0}",
        ] {
            let mut in_name = templates()[0].clone();
            in_name.name = bad.to_owned();
            assert_eq!(
                in_name.validate(),
                Err(LayoutError::HiddenCharacter),
                "{bad:?}"
            );
            let mut in_id = templates()[0].clone();
            in_id.id = bad.to_owned();
            assert_eq!(
                in_id.validate(),
                Err(LayoutError::HiddenCharacter),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn ordinary_names_in_any_script_are_fine() {
        for good in [
            "Halves",
            "My layout 2",
            "Zweispalter",
            "分屏",
            "Ёлка",
            "🙂 grid",
        ] {
            let mut l = templates()[0].clone();
            l.name = good.to_owned();
            assert_eq!(l.validate(), Ok(()), "{good}");
        }
        assert!(LayoutError::HiddenCharacter
            .to_string()
            .contains("invisible"));
    }

    #[test]
    fn hidden_characters_can_be_found_and_stripped() {
        assert!(has_hidden_char("a\u{200D}b"));
        assert!(!has_hidden_char("ab c"));
        assert_eq!(strip_hidden("a\u{200D}b\u{202E}c d"), "abc d");
    }
}
