//! Layout editing operations used by the zone editor: split, merge-on-delete
//! and divider dragging. All coordinates are normalized (0..=1).

use crate::geometry::Rect;
use crate::zone::{Layout, Zone, MAX_ZONES};

/// Smallest zone side the editor allows, as a fraction of the work area.
pub const MIN_ZONE: f64 = 0.05;

const EPS: f64 = 1e-6;

fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() < EPS
}

/// Direction of a divider line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// A vertical divider: zones end up side by side (positions are x values).
    Vertical,
    /// A horizontal divider: zones end up stacked (positions are y values).
    Horizontal,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EditError {
    #[error("zone {0} not found")]
    NoSuchZone(u32),
    #[error("the change would leave a zone smaller than the minimum size")]
    TooSmall,
    #[error("zone {0} has no neighbour it can merge into")]
    NoMergeTarget(u32),
    #[error("a layout needs at least one zone")]
    LastZone,
    #[error("there is no divider at that position")]
    NoDivider,
    #[error("the layout cannot take another zone")]
    TooManyZones,
}

/// Union of two rects that share a full edge, if they do.
fn merged(a: &Rect, b: &Rect) -> Option<Rect> {
    if approx(a.y, b.y) && approx(a.h, b.h) && (approx(a.x + a.w, b.x) || approx(b.x + b.w, a.x)) {
        return Some(Rect::new(a.x.min(b.x), a.y, a.w + b.w, a.h));
    }
    if approx(a.x, b.x) && approx(a.w, b.w) && (approx(a.y + a.h, b.y) || approx(b.y + b.h, a.y)) {
        return Some(Rect::new(a.x, a.y.min(b.y), a.w, a.h + b.h));
    }
    None
}

impl Layout {
    /// Next free zone id, or `None` if ids have run out.
    fn next_zone_id(&self) -> Option<u32> {
        self.zones
            .iter()
            .map(|z| z.id)
            .max()
            .unwrap_or(0)
            .checked_add(1)
    }

    /// Split a zone at normalized position `at` along `axis`. The original
    /// zone keeps the left/top part; the new zone (returned id) takes the rest.
    pub fn split_zone(&mut self, zone_id: u32, axis: Axis, at: f64) -> Result<u32, EditError> {
        let idx = self
            .zones
            .iter()
            .position(|z| z.id == zone_id)
            .ok_or(EditError::NoSuchZone(zone_id))?;
        let r = self.zones[idx].rect;
        let (first, second) = match axis {
            Axis::Vertical => {
                if at - r.x < MIN_ZONE || r.x + r.w - at < MIN_ZONE {
                    return Err(EditError::TooSmall);
                }
                (
                    Rect::new(r.x, r.y, at - r.x, r.h),
                    Rect::new(at, r.y, r.x + r.w - at, r.h),
                )
            }
            Axis::Horizontal => {
                if at - r.y < MIN_ZONE || r.y + r.h - at < MIN_ZONE {
                    return Err(EditError::TooSmall);
                }
                (
                    Rect::new(r.x, r.y, r.w, at - r.y),
                    Rect::new(r.x, at, r.w, r.y + r.h - at),
                )
            }
        };
        if self.zones.len() >= MAX_ZONES {
            return Err(EditError::TooManyZones);
        }
        let new_id = self.next_zone_id().ok_or(EditError::TooManyZones)?;
        self.zones[idx].rect = first;
        self.zones.push(Zone {
            id: new_id,
            rect: second,
        });
        Ok(new_id)
    }

    /// Delete a zone, giving its space to a neighbour that shares a full edge.
    /// Returns the id of the zone that grew.
    pub fn remove_zone(&mut self, zone_id: u32) -> Result<u32, EditError> {
        let idx = self
            .zones
            .iter()
            .position(|z| z.id == zone_id)
            .ok_or(EditError::NoSuchZone(zone_id))?;
        if self.zones.len() == 1 {
            return Err(EditError::LastZone);
        }
        let removed = self.zones[idx].rect;
        let (target, union) = self
            .zones
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != idx)
            .find_map(|(i, z)| merged(&removed, &z.rect).map(|u| (i, u)))
            .ok_or(EditError::NoMergeTarget(zone_id))?;
        self.zones[target].rect = union;
        let grown = self.zones[target].id;
        self.zones.remove(idx);
        Ok(grown)
    }

    /// Move every zone edge lying on the divider at `from` to `to`. Edges on
    /// the work-area border are not dividers and are rejected.
    pub fn move_divider(&mut self, axis: Axis, from: f64, to: f64) -> Result<(), EditError> {
        let mut next = self.zones.clone();
        let (mut before, mut after) = (0usize, 0usize);
        for z in &mut next {
            let (pos, len) = match axis {
                Axis::Vertical => (&mut z.rect.x, &mut z.rect.w),
                Axis::Horizontal => (&mut z.rect.y, &mut z.rect.h),
            };
            if approx(*pos + *len, from) {
                *len = to - *pos;
                before += 1;
            } else if approx(*pos, from) {
                let end = *pos + *len;
                *pos = to;
                *len = end - to;
                after += 1;
            }
        }
        if before == 0 || after == 0 {
            return Err(EditError::NoDivider);
        }
        let too_small = |z: &Zone| {
            let side = match axis {
                Axis::Vertical => z.rect.w,
                Axis::Horizontal => z.rect.h,
            };
            side < MIN_ZONE - EPS || !z.rect.is_valid()
        };
        if next.iter().any(too_small) {
            return Err(EditError::TooSmall);
        }
        self.zones = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zone::templates;

    fn total_area(l: &Layout) -> f64 {
        l.zones.iter().map(|z| z.rect.area()).sum()
    }

    #[test]
    fn split_keeps_tiling_and_hits_both_halves() {
        let mut l = Layout::new("one", "One", &[Rect::new(0.0, 0.0, 1.0, 1.0)]);
        let new = l.split_zone(1, Axis::Vertical, 0.4).unwrap();
        assert_eq!(new, 2);
        l.validate().unwrap();
        assert!(approx(total_area(&l), 1.0));
        assert_eq!(l.hit_test(0.39, 0.5), Some(1));
        assert_eq!(l.hit_test(0.4, 0.5), Some(2));

        let new = l.split_zone(2, Axis::Horizontal, 0.5).unwrap();
        assert_eq!(new, 3);
        l.validate().unwrap();
        assert!(approx(total_area(&l), 1.0));
        assert_eq!(l.hit_test(0.7, 0.2), Some(2));
        assert_eq!(l.hit_test(0.7, 0.8), Some(3));
    }

    #[test]
    fn split_rejects_slivers_and_unknown_zones() {
        let mut l = templates().remove(0);
        assert_eq!(
            l.split_zone(1, Axis::Vertical, 0.01),
            Err(EditError::TooSmall)
        );
        assert_eq!(
            l.split_zone(1, Axis::Vertical, 0.49),
            Err(EditError::TooSmall)
        );
        assert_eq!(
            l.split_zone(9, Axis::Vertical, 0.2),
            Err(EditError::NoSuchZone(9))
        );
        assert_eq!(l.zones.len(), 2, "failed edits must not change the layout");
    }

    #[test]
    fn remove_gives_space_to_neighbour() {
        let mut l = templates().remove(2); // 2x2 grid
        let grown = l.remove_zone(1).unwrap();
        assert_eq!(grown, 2);
        l.validate().unwrap();
        assert_eq!(l.zones.len(), 3);
        assert!(approx(total_area(&l), 1.0));
        assert_eq!(l.hit_test(0.1, 0.1), Some(2));
    }

    #[test]
    fn remove_without_full_edge_neighbour_is_refused() {
        let mut l = Layout::new(
            "t",
            "t",
            &[
                Rect::new(0.0, 0.0, 0.5, 1.0),
                Rect::new(0.5, 0.0, 0.5, 0.5),
                Rect::new(0.5, 0.5, 0.5, 0.5),
            ],
        );
        assert_eq!(l.remove_zone(1), Err(EditError::NoMergeTarget(1)));
        assert_eq!(l.zones.len(), 3);
        assert_eq!(l.remove_zone(2), Ok(3));
    }

    #[test]
    fn last_zone_cannot_be_removed() {
        let mut l = Layout::new("one", "One", &[Rect::new(0.0, 0.0, 1.0, 1.0)]);
        assert_eq!(l.remove_zone(1), Err(EditError::LastZone));
    }

    #[test]
    fn divider_moves_both_sides() {
        let mut l = templates().remove(0);
        l.move_divider(Axis::Vertical, 0.5, 0.3).unwrap();
        l.validate().unwrap();
        assert!(approx(l.zones[0].rect.w, 0.3));
        assert!(approx(l.zones[1].rect.x, 0.3));
        assert!(approx(l.zones[1].rect.w, 0.7));
    }

    #[test]
    fn divider_moves_every_aligned_edge_in_a_grid() {
        let mut l = templates().remove(2);
        l.move_divider(Axis::Vertical, 0.5, 0.6).unwrap();
        l.validate().unwrap();
        assert!(approx(l.zones[0].rect.w, 0.6) && approx(l.zones[2].rect.w, 0.6));
        assert!(approx(l.zones[1].rect.x, 0.6) && approx(l.zones[3].rect.x, 0.6));
        assert!(approx(total_area(&l), 1.0));
    }

    #[test]
    fn divider_rejects_border_and_slivers() {
        let mut l = templates().remove(0);
        assert_eq!(
            l.move_divider(Axis::Vertical, 0.0, 0.2),
            Err(EditError::NoDivider)
        );
        assert_eq!(
            l.move_divider(Axis::Vertical, 1.0, 0.8),
            Err(EditError::NoDivider)
        );
        assert_eq!(
            l.move_divider(Axis::Vertical, 0.3, 0.2),
            Err(EditError::NoDivider)
        );
        assert_eq!(
            l.move_divider(Axis::Vertical, 0.5, 0.98),
            Err(EditError::TooSmall)
        );
        assert_eq!(
            l.move_divider(Axis::Vertical, 0.5, 0.01),
            Err(EditError::TooSmall)
        );
        assert_eq!(
            l,
            templates().remove(0),
            "failed edits must not change the layout"
        );
    }

    #[test]
    fn splitting_stops_at_the_zone_limit() {
        // A full layout: the left half is one wide zone, the right half is 63 thin columns.
        let thin = 0.5 / (MAX_ZONES - 1) as f64;
        let mut rects = vec![Rect::new(0.0, 0.0, 0.5, 1.0)];
        rects.extend((0..MAX_ZONES - 1).map(|i| Rect::new(0.5 + i as f64 * thin, 0.0, thin, 1.0)));
        let mut l = Layout::new("full", "Full", &rects);
        assert_eq!(l.zones.len(), MAX_ZONES);
        let before = l.clone();
        // Splitting the wide zone is geometrically fine, but there is no room left.
        assert_eq!(
            l.split_zone(1, Axis::Vertical, 0.25),
            Err(EditError::TooManyZones)
        );
        assert_eq!(l, before, "a refused split must not change the layout");
        // One zone fewer and the same split works.
        l.zones.pop();
        assert!(l.split_zone(1, Axis::Vertical, 0.25).is_ok());
        assert_eq!(l.zones.len(), MAX_ZONES);
    }

    #[test]
    fn exhausted_zone_ids_do_not_overflow() {
        let mut l = Layout::new("one", "One", &[Rect::new(0.0, 0.0, 1.0, 1.0)]);
        l.zones[0].id = u32::MAX;
        let before = l.clone();
        assert_eq!(
            l.split_zone(u32::MAX, Axis::Vertical, 0.5),
            Err(EditError::TooManyZones)
        );
        assert_eq!(l, before);
    }
}
