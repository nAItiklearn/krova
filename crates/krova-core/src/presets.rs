//! Preset layout families: ordinary [`Layout`]s generated from a few choices.
//!
//! A [`PresetSpec`] names a family (grid, columns, rows, main + side), a window count
//! from 1 to 5, a screen shape ([`PresetVariant`]) and, for main + side, how much of
//! the screen the main zone takes ([`MainSize`]). [`PresetSpec::layout`] turns it into
//! a plain layout: the user can edit and save it like any drawn layout, and nothing
//! remembers that it came from a preset.
//!
//! Zones come out in reading order and are numbered 1 to N in that order, so zone N
//! is also what `<mods>+N` and the auto-arrange planner call zone N. The zones tile
//! the work area exactly: no gaps, no overlaps, and no side below 10 % of the screen.
//!
//! # Shapes
//!
//! Grid and columns/rows fill the screen with equal cells. A grid lays its cells out
//! in rows; the last row may have fewer, wider cells.
//!
//! | windows | landscape | ultrawide | portrait |
//! |---|---|---|---|
//! | 1 | 1 | 1 | 1 |
//! | 2 | 2 | 2 | 1 / 1 |
//! | 3 | 2 / 1 | 3 | 1 / 2 |
//! | 4 | 2 / 2 | 2 / 2 | 2 / 2 |
//! | 5 | 3 / 2 | 3 / 2 | 1 / 2 / 2 |
//!
//! Main + side:
//!
//! * landscape: the main zone on the left, the others stacked on the right;
//! * ultrawide (aspect 2.1 or wider): two main zones side by side and a narrow column
//!   on the right for the rest. The column is half as wide as the landscape side
//!   column would be, so the main size 50 / 60 / 66.7 % gives columns of 25 / 20 /
//!   16.7 % of the width. With two windows there is one main zone and the column;
//! * portrait: the main zone on top, the others in a small grid below it.
//!
//! With one window every family is the whole screen.

use serde::{Deserialize, Serialize};

use crate::geometry::Rect;
use crate::zone::Layout;

/// Fewest windows a preset is made for.
pub const MIN_COUNT: u8 = 1;
/// Most windows a preset is made for.
pub const MAX_COUNT: u8 = 5;
/// A screen at least this wide for its height (width / height) is "ultrawide".
pub const ULTRAWIDE_ASPECT: f64 = 2.1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresetFamily {
    #[default]
    Grid,
    Columns,
    Rows,
    MainSide,
}

impl PresetFamily {
    pub const ALL: [PresetFamily; 4] = [
        PresetFamily::Grid,
        PresetFamily::Columns,
        PresetFamily::Rows,
        PresetFamily::MainSide,
    ];
}

/// The shape of the screen a preset is made for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresetVariant {
    #[default]
    Landscape,
    Ultrawide,
    Portrait,
}

impl PresetVariant {
    pub const ALL: [PresetVariant; 3] = [
        PresetVariant::Landscape,
        PresetVariant::Ultrawide,
        PresetVariant::Portrait,
    ];

    /// The variant that suits a work area of `width` x `height` pixels: portrait when
    /// it is taller than wide, ultrawide from [`ULTRAWIDE_ASPECT`] up, else landscape.
    /// A work area with no size counts as landscape.
    pub fn for_size(width: i32, height: i32) -> Self {
        if width <= 0 || height <= 0 {
            return PresetVariant::Landscape;
        }
        if height > width {
            return PresetVariant::Portrait;
        }
        // width / height >= 2.1  <=>  width * 10 >= height * 21 (exact, no floats).
        if i64::from(width) * 10 >= i64::from(height) * 21 {
            PresetVariant::Ultrawide
        } else {
            PresetVariant::Landscape
        }
    }
}

/// How much of the screen the main zone of a main + side preset takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MainSize {
    /// 50 %.
    Half,
    /// 60 %.
    #[default]
    Sixty,
    /// 66.7 %.
    TwoThirds,
}

impl MainSize {
    pub const ALL: [MainSize; 3] = [MainSize::Half, MainSize::Sixty, MainSize::TwoThirds];

    /// The share of the screen, 0 to 1.
    pub fn fraction(self) -> f64 {
        match self {
            MainSize::Half => 0.5,
            MainSize::Sixty => 0.6,
            MainSize::TwoThirds => 2.0 / 3.0,
        }
    }

    fn label(self) -> &'static str {
        match self {
            MainSize::Half => "50%",
            MainSize::Sixty => "60%",
            MainSize::TwoThirds => "66.7%",
        }
    }

    fn id_part(self) -> &'static str {
        match self {
            MainSize::Half => "50",
            MainSize::Sixty => "60",
            MainSize::TwoThirds => "67",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PresetError {
    #[error("a preset is made for 1 to 5 windows, not {0}")]
    CountOutOfRange(u8),
}

/// What to generate. Parts that do not change the result for the chosen family (the
/// variant of a row layout, the main size of a grid) are accepted and ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct PresetSpec {
    pub family: PresetFamily,
    /// Number of windows, 1 to 5.
    pub count: u8,
    pub variant: PresetVariant,
    pub main_size: MainSize,
}

impl Default for PresetSpec {
    fn default() -> Self {
        Self {
            family: PresetFamily::Grid,
            count: 2,
            variant: PresetVariant::Landscape,
            main_size: MainSize::Sixty,
        }
    }
}

/// An axis-aligned box given by its edges, so neighbours share exactly the same
/// edge value and nothing accumulates rounding error.
#[derive(Clone, Copy)]
struct Edges {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

impl Edges {
    const FULL: Edges = Edges {
        x0: 0.0,
        y0: 0.0,
        x1: 1.0,
        y1: 1.0,
    };

    fn rect(self) -> Rect {
        Rect::new(self.x0, self.y0, self.x1 - self.x0, self.y1 - self.y0)
    }
}

/// Edge `index` of `count` equal divisions of `lo..hi`; the last edge is `hi` exactly.
fn cut(lo: f64, hi: f64, index: usize, count: usize) -> f64 {
    if index >= count {
        hi
    } else {
        lo + (hi - lo) * index as f64 / count as f64
    }
}

/// Fill `area` with rows of equal height; row `r` has `rows[r]` equal cells.
fn rows_of(area: Edges, rows: &[usize]) -> Vec<Rect> {
    let mut out = Vec::new();
    for (r, &cells) in rows.iter().enumerate() {
        let (y0, y1) = (
            cut(area.y0, area.y1, r, rows.len()),
            cut(area.y0, area.y1, r + 1, rows.len()),
        );
        for c in 0..cells {
            out.push(
                Edges {
                    x0: cut(area.x0, area.x1, c, cells),
                    y0,
                    x1: cut(area.x0, area.x1, c + 1, cells),
                    y1,
                }
                .rect(),
            );
        }
    }
    out
}

/// `count` cells in one column, top to bottom.
fn column_of(area: Edges, count: usize) -> Vec<Rect> {
    rows_of(area, &vec![1; count])
}

/// Cells per row of the grid family.
fn grid_rows(count: usize, variant: PresetVariant) -> &'static [usize] {
    match (variant, count) {
        (_, 1) => &[1],
        (PresetVariant::Portrait, 2) => &[1, 1],
        (PresetVariant::Portrait, 3) => &[1, 2],
        (PresetVariant::Portrait, 5) => &[1, 2, 2],
        (_, 2) => &[2],
        (PresetVariant::Ultrawide, 3) => &[3],
        (_, 3) => &[2, 1],
        (_, 4) => &[2, 2],
        (_, 5) => &[3, 2],
        _ => &[],
    }
}

fn main_side(count: usize, variant: PresetVariant, main: f64) -> Vec<Rect> {
    if count == 1 {
        return vec![Edges::FULL.rect()];
    }
    match variant {
        PresetVariant::Landscape => {
            let mut out = vec![Edges {
                x1: main,
                ..Edges::FULL
            }
            .rect()];
            out.extend(column_of(
                Edges {
                    x0: main,
                    ..Edges::FULL
                },
                count - 1,
            ));
            out
        }
        PresetVariant::Ultrawide => {
            // The narrow column is half of what a landscape side column would be.
            let edge = 1.0 - (1.0 - main) / 2.0;
            let column = Edges {
                x0: edge,
                ..Edges::FULL
            };
            if count == 2 {
                let mut out = vec![Edges {
                    x1: edge,
                    ..Edges::FULL
                }
                .rect()];
                out.extend(column_of(column, 1));
                return out;
            }
            let middle = edge / 2.0;
            let mut out = vec![
                Edges {
                    x1: middle,
                    ..Edges::FULL
                }
                .rect(),
                Edges {
                    x0: middle,
                    x1: edge,
                    ..Edges::FULL
                }
                .rect(),
            ];
            out.extend(column_of(column, count - 2));
            out
        }
        PresetVariant::Portrait => {
            let mut out = vec![Edges {
                y1: main,
                ..Edges::FULL
            }
            .rect()];
            out.extend(rows_of(
                Edges {
                    y0: main,
                    ..Edges::FULL
                },
                grid_rows(count - 1, PresetVariant::Landscape),
            ));
            out
        }
    }
}

impl PresetSpec {
    pub fn new(
        family: PresetFamily,
        count: u8,
        variant: PresetVariant,
        main_size: MainSize,
    ) -> Self {
        Self {
            family,
            count,
            variant,
            main_size,
        }
    }

    /// The zone rectangles in reading order.
    pub fn rects(&self) -> Result<Vec<Rect>, PresetError> {
        if !(MIN_COUNT..=MAX_COUNT).contains(&self.count) {
            return Err(PresetError::CountOutOfRange(self.count));
        }
        let n = usize::from(self.count);
        Ok(match self.family {
            PresetFamily::Grid => rows_of(Edges::FULL, grid_rows(n, self.variant)),
            PresetFamily::Columns => rows_of(Edges::FULL, &[n]),
            PresetFamily::Rows => column_of(Edges::FULL, n),
            PresetFamily::MainSide => main_side(n, self.variant, self.main_size.fraction()),
        })
    }

    /// The same preset with every choice that changes nothing reset to its first
    /// value, so two specs that give the same zones are equal. Used for ids and to
    /// list each distinct preset once.
    pub fn canonical(&self) -> PresetSpec {
        let Ok(target) = self.rects() else {
            return *self;
        };
        let mut spec = *self;
        spec.variant = PresetVariant::ALL
            .into_iter()
            .find(|&variant| PresetSpec { variant, ..spec }.rects().as_ref() == Ok(&target))
            .unwrap_or(self.variant);
        spec.main_size = MainSize::ALL
            .into_iter()
            .find(|&main_size| PresetSpec { main_size, ..spec }.rects().as_ref() == Ok(&target))
            .unwrap_or(self.main_size);
        spec
    }

    /// Stable id, e.g. `grid-3`, `columns-4`, `main-side-3-ultrawide-60`. Two specs
    /// with the same zones have the same id. At most 64 characters.
    pub fn id(&self) -> String {
        let c = self.canonical();
        let mut id = match c.family {
            PresetFamily::Grid => "grid",
            PresetFamily::Columns => "columns",
            PresetFamily::Rows => "rows",
            PresetFamily::MainSide => "main-side",
        }
        .to_owned();
        id.push_str(&format!("-{}", c.count));
        match c.variant {
            PresetVariant::Landscape => {}
            PresetVariant::Ultrawide => id.push_str("-ultrawide"),
            PresetVariant::Portrait => id.push_str("-portrait"),
        }
        if c.family == PresetFamily::MainSide && c.count > 1 {
            id.push('-');
            id.push_str(c.main_size.id_part());
        }
        id
    }

    /// Short display name, e.g. `Grid 3` or `Main + side 3 (portrait, 60%)`.
    pub fn name(&self) -> String {
        let c = self.canonical();
        let base = match c.family {
            PresetFamily::Grid => "Grid",
            PresetFamily::Columns => "Columns",
            PresetFamily::Rows => "Rows",
            PresetFamily::MainSide => "Main + side",
        };
        let mut notes: Vec<&str> = Vec::new();
        match c.variant {
            PresetVariant::Landscape => {}
            PresetVariant::Ultrawide => notes.push("ultrawide"),
            PresetVariant::Portrait => notes.push("portrait"),
        }
        if c.family == PresetFamily::MainSide && c.count > 1 {
            notes.push(c.main_size.label());
        }
        if notes.is_empty() {
            format!("{base} {}", c.count)
        } else {
            format!("{base} {} ({})", c.count, notes.join(", "))
        }
    }

    /// The preset as an ordinary layout: zone ids 1..=N in reading order.
    pub fn layout(&self) -> Result<Layout, PresetError> {
        let rects = self.rects()?;
        Ok(Layout::new(&self.id(), &self.name(), &rects))
    }
}

/// Every distinct preset once, in a fixed order (family, then window count, then
/// screen shape, then main size). Specs that would give the same zones are listed
/// only once.
pub fn all() -> Vec<PresetSpec> {
    let mut out: Vec<PresetSpec> = Vec::new();
    for family in PresetFamily::ALL {
        for count in MIN_COUNT..=MAX_COUNT {
            for variant in PresetVariant::ALL {
                for main_size in MainSize::ALL {
                    let spec = PresetSpec::new(family, count, variant, main_size).canonical();
                    if !out.contains(&spec) {
                        out.push(spec);
                    }
                }
            }
        }
    }
    out
}

/// The five layouts of one family for 1 to 5 windows, for a picker that animates
/// between counts.
pub fn family_layouts(
    family: PresetFamily,
    variant: PresetVariant,
    main_size: MainSize,
) -> Vec<Layout> {
    (MIN_COUNT..=MAX_COUNT)
        .filter_map(|count| {
            PresetSpec::new(family, count, variant, main_size)
                .layout()
                .ok()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::reading_order;
    use crate::zone::MAX_NAME_LEN;

    fn every_spec() -> Vec<PresetSpec> {
        let mut v = Vec::new();
        for family in PresetFamily::ALL {
            for count in MIN_COUNT..=MAX_COUNT {
                for variant in PresetVariant::ALL {
                    for main_size in MainSize::ALL {
                        v.push(PresetSpec::new(family, count, variant, main_size));
                    }
                }
            }
        }
        v
    }

    fn spec(family: PresetFamily, count: u8, variant: PresetVariant, size: MainSize) -> Layout {
        PresetSpec::new(family, count, variant, size)
            .layout()
            .unwrap()
    }

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn rect_is(r: &Rect, x: f64, y: f64, w: f64, h: f64) -> bool {
        near(r.x, x) && near(r.y, y) && near(r.w, w) && near(r.h, h)
    }

    fn overlap_area(a: &Rect, b: &Rect) -> f64 {
        let w = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
        let h = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
        if w > 0.0 && h > 0.0 {
            w * h
        } else {
            0.0
        }
    }

    #[test]
    fn every_combination_is_a_valid_layout() {
        assert_eq!(every_spec().len(), 4 * 5 * 3 * 3);
        for s in every_spec() {
            let l = s.layout().unwrap_or_else(|e| panic!("{s:?}: {e}"));
            l.validate().unwrap_or_else(|e| panic!("{s:?}: {e}"));
            assert_eq!(l.zones.len(), usize::from(s.count), "{s:?}");
        }
    }

    #[test]
    fn the_zones_tile_the_screen_exactly() {
        for s in every_spec() {
            let l = s.layout().unwrap();
            let total: f64 = l.zones.iter().map(|z| z.rect.area()).sum();
            assert!(near(total, 1.0), "{s:?}: areas add up to {total}");
            for (i, a) in l.zones.iter().enumerate() {
                for b in &l.zones[i + 1..] {
                    assert!(
                        overlap_area(&a.rect, &b.rect) < 1e-9,
                        "{s:?}: zones {} and {} overlap",
                        a.id,
                        b.id
                    );
                }
                let r = a.rect;
                assert!(r.x >= 0.0 && r.y >= 0.0, "{s:?}");
                assert!(
                    r.x + r.w <= 1.0 + 1e-12 && r.y + r.h <= 1.0 + 1e-12,
                    "{s:?}"
                );
            }
        }
    }

    #[test]
    fn no_zone_is_tiny() {
        for s in every_spec() {
            for z in &s.layout().unwrap().zones {
                assert!(z.rect.w >= 0.1 && z.rect.h >= 0.1, "{s:?}: {:?}", z.rect);
            }
        }
    }

    #[test]
    fn small_work_areas_still_get_whole_pixel_zones_that_tile() {
        use crate::geometry::PxRect;
        for (w, h) in [(20, 20), (37, 23), (100, 60), (23, 400), (641, 479)] {
            let work = PxRect::new(-7, 13, w, h);
            for s in every_spec() {
                let l = s.layout().unwrap();
                let mut covered = 0i64;
                for z in &l.zones {
                    let px = l.zone_pixels(z.id, work).unwrap();
                    assert!(px.w >= 1 && px.h >= 1, "{s:?} on {w}x{h}: {px:?}");
                    assert!(px.x >= work.x && px.right() <= work.right());
                    assert!(px.y >= work.y && px.bottom() <= work.bottom());
                    covered += i64::from(px.w) * i64::from(px.h);
                }
                assert_eq!(covered, i64::from(w) * i64::from(h), "{s:?} on {w}x{h}");
            }
        }
    }

    #[test]
    fn zone_numbers_follow_reading_order() {
        for s in every_spec() {
            let l = s.layout().unwrap();
            let ids: Vec<u32> = (1..=u32::from(s.count)).collect();
            assert_eq!(reading_order(&l), ids, "{s:?}");
        }
    }

    #[test]
    fn counts_outside_one_to_five_are_refused() {
        for count in [0u8, 6, 64, 255] {
            let s = PresetSpec {
                count,
                ..PresetSpec::default()
            };
            assert_eq!(s.rects(), Err(PresetError::CountOutOfRange(count)));
            assert_eq!(s.layout(), Err(PresetError::CountOutOfRange(count)));
        }
        assert!(PresetError::CountOutOfRange(9).to_string().contains('9'));
        // Describing a bad spec must not panic.
        let bad = PresetSpec {
            count: 0,
            ..PresetSpec::default()
        };
        assert_eq!(bad.canonical(), bad);
        let _ = (bad.id(), bad.name());
    }

    #[test]
    fn one_window_is_the_whole_screen_in_every_family() {
        for family in PresetFamily::ALL {
            for variant in PresetVariant::ALL {
                for size in MainSize::ALL {
                    let l = spec(family, 1, variant, size);
                    assert_eq!(l.zones.len(), 1);
                    assert!(rect_is(&l.zones[0].rect, 0.0, 0.0, 1.0, 1.0));
                }
            }
        }
    }

    #[test]
    fn columns_and_rows_are_equal_slices() {
        for n in 1..=5u8 {
            let c = spec(
                PresetFamily::Columns,
                n,
                PresetVariant::Landscape,
                MainSize::Sixty,
            );
            let r = spec(
                PresetFamily::Rows,
                n,
                PresetVariant::Landscape,
                MainSize::Sixty,
            );
            let k = f64::from(n);
            for (i, (cz, rz)) in c.zones.iter().zip(&r.zones).enumerate() {
                let i = i as f64;
                assert!(rect_is(&cz.rect, i / k, 0.0, 1.0 / k, 1.0));
                assert!(rect_is(&rz.rect, 0.0, i / k, 1.0, 1.0 / k));
            }
        }
    }

    #[test]
    fn columns_and_rows_ignore_variant_and_main_size() {
        for family in [PresetFamily::Columns, PresetFamily::Rows] {
            let base = spec(family, 4, PresetVariant::Landscape, MainSize::Half);
            for variant in PresetVariant::ALL {
                for size in MainSize::ALL {
                    let l = spec(family, 4, variant, size);
                    assert_eq!(l.zones, base.zones);
                    assert_eq!(l.id, base.id);
                    assert_eq!(l.name, base.name);
                }
            }
        }
    }

    #[test]
    fn the_landscape_grid_shapes() {
        let g = |n| {
            spec(
                PresetFamily::Grid,
                n,
                PresetVariant::Landscape,
                MainSize::Sixty,
            )
        };
        let l2 = g(2);
        assert!(rect_is(&l2.zones[1].rect, 0.5, 0.0, 0.5, 1.0));
        let l3 = g(3);
        assert!(rect_is(&l3.zones[0].rect, 0.0, 0.0, 0.5, 0.5));
        assert!(rect_is(&l3.zones[1].rect, 0.5, 0.0, 0.5, 0.5));
        assert!(
            rect_is(&l3.zones[2].rect, 0.0, 0.5, 1.0, 0.5),
            "the last row is one wide cell"
        );
        let l4 = g(4);
        assert!(rect_is(&l4.zones[3].rect, 0.5, 0.5, 0.5, 0.5));
        let l5 = g(5);
        let third = 1.0 / 3.0;
        assert!(rect_is(&l5.zones[2].rect, 2.0 * third, 0.0, third, 0.5));
        assert!(rect_is(&l5.zones[3].rect, 0.0, 0.5, 0.5, 0.5));
        assert!(rect_is(&l5.zones[4].rect, 0.5, 0.5, 0.5, 0.5));
    }

    #[test]
    fn the_ultrawide_grid_uses_three_columns_for_three_windows() {
        let l = spec(
            PresetFamily::Grid,
            3,
            PresetVariant::Ultrawide,
            MainSize::Sixty,
        );
        let third = 1.0 / 3.0;
        for (i, z) in l.zones.iter().enumerate() {
            assert!(rect_is(&z.rect, i as f64 * third, 0.0, third, 1.0));
        }
        assert_ne!(
            l.zones,
            spec(
                PresetFamily::Grid,
                3,
                PresetVariant::Landscape,
                MainSize::Sixty
            )
            .zones
        );
    }

    #[test]
    fn the_portrait_grid_stacks_rows() {
        let g = |n| {
            spec(
                PresetFamily::Grid,
                n,
                PresetVariant::Portrait,
                MainSize::Sixty,
            )
        };
        let l2 = g(2);
        assert!(rect_is(&l2.zones[0].rect, 0.0, 0.0, 1.0, 0.5));
        assert!(rect_is(&l2.zones[1].rect, 0.0, 0.5, 1.0, 0.5));
        let l3 = g(3);
        assert!(
            rect_is(&l3.zones[0].rect, 0.0, 0.0, 1.0, 0.5),
            "the wide cell is on top"
        );
        assert!(rect_is(&l3.zones[2].rect, 0.5, 0.5, 0.5, 0.5));
        let l5 = g(5);
        assert_eq!(l5.zones.len(), 5);
        assert!(rect_is(&l5.zones[0].rect, 0.0, 0.0, 1.0, 1.0 / 3.0));
    }

    #[test]
    fn landscape_main_side_puts_the_main_zone_on_the_left() {
        for (size, m) in [
            (MainSize::Half, 0.5),
            (MainSize::Sixty, 0.6),
            (MainSize::TwoThirds, 2.0 / 3.0),
        ] {
            let l = spec(PresetFamily::MainSide, 3, PresetVariant::Landscape, size);
            assert!(rect_is(&l.zones[0].rect, 0.0, 0.0, m, 1.0), "{size:?}");
            assert!(rect_is(&l.zones[1].rect, m, 0.0, 1.0 - m, 0.5), "{size:?}");
            assert!(rect_is(&l.zones[2].rect, m, 0.5, 1.0 - m, 0.5), "{size:?}");
        }
        let two = spec(
            PresetFamily::MainSide,
            2,
            PresetVariant::Landscape,
            MainSize::Sixty,
        );
        assert!(rect_is(&two.zones[1].rect, 0.6, 0.0, 0.4, 1.0));
        let five = spec(
            PresetFamily::MainSide,
            5,
            PresetVariant::Landscape,
            MainSize::Sixty,
        );
        assert!(rect_is(&five.zones[4].rect, 0.6, 0.75, 0.4, 0.25));
    }

    #[test]
    fn ultrawide_main_side_is_two_main_zones_and_a_narrow_column() {
        let l = spec(
            PresetFamily::MainSide,
            4,
            PresetVariant::Ultrawide,
            MainSize::Sixty,
        );
        // Column: (1 - 0.6) / 2 = 0.2 wide; the main zones share the other 0.8.
        assert!(rect_is(&l.zones[0].rect, 0.0, 0.0, 0.4, 1.0));
        assert!(rect_is(&l.zones[1].rect, 0.4, 0.0, 0.4, 1.0));
        assert!(rect_is(&l.zones[2].rect, 0.8, 0.0, 0.2, 0.5));
        assert!(rect_is(&l.zones[3].rect, 0.8, 0.5, 0.2, 0.5));
        let three = spec(
            PresetFamily::MainSide,
            3,
            PresetVariant::Ultrawide,
            MainSize::Half,
        );
        assert!(rect_is(&three.zones[2].rect, 0.75, 0.0, 0.25, 1.0));
        for size in MainSize::ALL {
            let l = spec(PresetFamily::MainSide, 5, PresetVariant::Ultrawide, size);
            let column = l.zones[2].rect.w;
            assert!(
                column < l.zones[0].rect.w,
                "the column is the narrow one ({size:?})"
            );
        }
    }

    #[test]
    fn ultrawide_main_side_with_two_windows_is_one_main_zone_and_the_column() {
        let l = spec(
            PresetFamily::MainSide,
            2,
            PresetVariant::Ultrawide,
            MainSize::Sixty,
        );
        assert!(rect_is(&l.zones[0].rect, 0.0, 0.0, 0.8, 1.0));
        assert!(rect_is(&l.zones[1].rect, 0.8, 0.0, 0.2, 1.0));
    }

    #[test]
    fn portrait_main_side_puts_the_main_zone_on_top() {
        let l = spec(
            PresetFamily::MainSide,
            3,
            PresetVariant::Portrait,
            MainSize::Sixty,
        );
        assert!(rect_is(&l.zones[0].rect, 0.0, 0.0, 1.0, 0.6));
        assert!(rect_is(&l.zones[1].rect, 0.0, 0.6, 0.5, 0.4));
        assert!(rect_is(&l.zones[2].rect, 0.5, 0.6, 0.5, 0.4));
        let five = spec(
            PresetFamily::MainSide,
            5,
            PresetVariant::Portrait,
            MainSize::Half,
        );
        assert!(
            rect_is(&five.zones[1].rect, 0.0, 0.5, 0.5, 0.25),
            "four below form a 2 x 2 grid"
        );
        assert!(rect_is(&five.zones[4].rect, 0.5, 0.75, 0.5, 0.25));
        let two = spec(
            PresetFamily::MainSide,
            2,
            PresetVariant::Portrait,
            MainSize::TwoThirds,
        );
        assert!(rect_is(&two.zones[1].rect, 0.0, 2.0 / 3.0, 1.0, 1.0 / 3.0));
    }

    #[test]
    fn main_side_with_three_windows_below_the_main_zone_uses_a_wide_last_cell() {
        let l = spec(
            PresetFamily::MainSide,
            4,
            PresetVariant::Portrait,
            MainSize::Sixty,
        );
        assert!(rect_is(&l.zones[1].rect, 0.0, 0.6, 0.5, 0.2));
        assert!(rect_is(&l.zones[2].rect, 0.5, 0.6, 0.5, 0.2));
        assert!(rect_is(&l.zones[3].rect, 0.0, 0.8, 1.0, 0.2));
    }

    #[test]
    fn the_main_size_only_matters_for_main_side_with_two_or_more_windows() {
        let a = spec(
            PresetFamily::Grid,
            3,
            PresetVariant::Landscape,
            MainSize::Half,
        );
        let b = spec(
            PresetFamily::Grid,
            3,
            PresetVariant::Landscape,
            MainSize::TwoThirds,
        );
        assert_eq!(a.zones, b.zones);
        let a = spec(
            PresetFamily::MainSide,
            2,
            PresetVariant::Landscape,
            MainSize::Half,
        );
        let b = spec(
            PresetFamily::MainSide,
            2,
            PresetVariant::Landscape,
            MainSize::TwoThirds,
        );
        assert_ne!(a.zones, b.zones);
        assert_ne!(a.id, b.id);
    }

    #[test]
    fn variants_are_detected_from_the_work_area() {
        use PresetVariant::*;
        assert_eq!(PresetVariant::for_size(1920, 1040), Landscape);
        assert_eq!(PresetVariant::for_size(1920, 1200), Landscape);
        assert_eq!(PresetVariant::for_size(2560, 1400), Landscape);
        assert_eq!(PresetVariant::for_size(3440, 1400), Ultrawide);
        assert_eq!(PresetVariant::for_size(5120, 1400), Ultrawide);
        assert_eq!(PresetVariant::for_size(1080, 1920), Portrait);
        assert_eq!(PresetVariant::for_size(1000, 1001), Portrait);
        assert_eq!(PresetVariant::for_size(1000, 1000), Landscape, "square");
        // The ultrawide threshold is inclusive.
        assert_eq!(PresetVariant::for_size(2100, 1000), Ultrawide);
        assert_eq!(PresetVariant::for_size(2099, 1000), Landscape);
        // Nonsense sizes fall back to landscape instead of dividing by zero.
        assert_eq!(PresetVariant::for_size(0, 0), Landscape);
        assert_eq!(PresetVariant::for_size(-5, 100), Landscape);
        assert_eq!(PresetVariant::for_size(100, 0), Landscape);
        assert_eq!(PresetVariant::for_size(i32::MAX, 1), Ultrawide);
        assert_eq!(PresetVariant::for_size(1, i32::MAX), Portrait);
    }

    #[test]
    fn an_id_always_stands_for_one_shape_and_fits_the_limits() {
        let mut by_id: std::collections::HashMap<String, Vec<Rect>> = Default::default();
        for s in every_spec() {
            let l = s.layout().unwrap();
            assert!(l.id.chars().count() <= MAX_NAME_LEN, "{}", l.id);
            assert!(l.name.chars().count() <= MAX_NAME_LEN, "{}", l.name);
            assert!(l
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'));
            let rects: Vec<Rect> = l.zones.iter().map(|z| z.rect).collect();
            // Same id means same zones, always.
            let seen = by_id.entry(l.id.clone()).or_insert_with(|| rects.clone());
            assert_eq!(*seen, rects, "id {} stands for two shapes", l.id);
        }
    }

    #[test]
    fn some_ids_and_names_spelled_out() {
        let s = |f, n, v, m| PresetSpec::new(f, n, v, m);
        let g3 = s(
            PresetFamily::Grid,
            3,
            PresetVariant::Landscape,
            MainSize::Half,
        );
        assert_eq!(g3.id(), "grid-3");
        assert_eq!(g3.name(), "Grid 3");
        let ms = s(
            PresetFamily::MainSide,
            3,
            PresetVariant::Ultrawide,
            MainSize::TwoThirds,
        );
        assert_eq!(ms.id(), "main-side-3-ultrawide-67");
        assert_eq!(ms.name(), "Main + side 3 (ultrawide, 66.7%)");
        let p = s(
            PresetFamily::MainSide,
            2,
            PresetVariant::Portrait,
            MainSize::Sixty,
        );
        assert_eq!(p.id(), "main-side-2-portrait-60");
        assert_eq!(p.name(), "Main + side 2 (portrait, 60%)");
        let l = s(
            PresetFamily::MainSide,
            4,
            PresetVariant::Landscape,
            MainSize::Half,
        );
        assert_eq!(l.id(), "main-side-4-50");
        let c = s(
            PresetFamily::Columns,
            5,
            PresetVariant::Portrait,
            MainSize::Half,
        );
        assert_eq!(c.id(), "columns-5");
        assert_eq!(
            s(
                PresetFamily::Rows,
                2,
                PresetVariant::Landscape,
                MainSize::Half
            )
            .name(),
            "Rows 2"
        );
        let one = s(
            PresetFamily::MainSide,
            1,
            PresetVariant::Portrait,
            MainSize::TwoThirds,
        );
        assert_eq!(one.id(), "main-side-1");
        assert_eq!(one.name(), "Main + side 1");
    }

    #[test]
    fn specs_that_give_the_same_zones_are_one_preset() {
        let a = PresetSpec::new(
            PresetFamily::Grid,
            4,
            PresetVariant::Portrait,
            MainSize::Half,
        );
        let b = PresetSpec::new(
            PresetFamily::Grid,
            4,
            PresetVariant::Landscape,
            MainSize::TwoThirds,
        );
        assert_eq!(a.canonical(), b.canonical());
        assert_eq!(a.id(), b.id());
        let u = PresetSpec::new(
            PresetFamily::Grid,
            2,
            PresetVariant::Ultrawide,
            MainSize::Sixty,
        );
        assert_eq!(u.canonical().variant, PresetVariant::Landscape);
        // A real difference is kept.
        let p = PresetSpec::new(
            PresetFamily::Grid,
            2,
            PresetVariant::Portrait,
            MainSize::Sixty,
        );
        assert_eq!(p.canonical().variant, PresetVariant::Portrait);
        // Canonicalising twice changes nothing.
        for s in every_spec() {
            assert_eq!(s.canonical().canonical(), s.canonical());
            assert_eq!(s.canonical().rects(), s.rects());
        }
    }

    #[test]
    fn all_lists_every_distinct_preset_once_in_a_stable_order() {
        let list = all();
        assert_eq!(list, all());
        let mut ids: Vec<String> = list.iter().map(PresetSpec::id).collect();
        let total = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), total, "no duplicates");
        // Grid 9 + columns 5 + rows 5 + main + side 37.
        assert_eq!(total, 56);
        assert_eq!(list[0].family, PresetFamily::Grid);
        assert_eq!(list[0].count, 1);
        assert_eq!(list.last().unwrap().family, PresetFamily::MainSide);
        for s in &list {
            s.layout().unwrap().validate().unwrap();
        }
        // Every shape any spec can make is in the list.
        let known: Vec<Vec<Rect>> = list.iter().map(|s| s.rects().unwrap()).collect();
        for s in every_spec() {
            assert!(known.contains(&s.rects().unwrap()), "{s:?}");
        }
    }

    #[test]
    fn a_family_comes_as_five_layouts_for_a_picker() {
        for family in PresetFamily::ALL {
            let layouts = family_layouts(family, PresetVariant::Landscape, MainSize::Sixty);
            assert_eq!(layouts.len(), 5);
            for (i, l) in layouts.iter().enumerate() {
                assert_eq!(l.zones.len(), i + 1);
            }
        }
    }

    #[test]
    fn specs_travel_as_json_with_defaults_for_missing_parts() {
        let s = PresetSpec::new(
            PresetFamily::MainSide,
            3,
            PresetVariant::Ultrawide,
            MainSize::TwoThirds,
        );
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(
            json,
            r#"{"family":"main_side","count":3,"variant":"ultrawide","main_size":"two_thirds"}"#
        );
        assert_eq!(serde_json::from_str::<PresetSpec>(&json).unwrap(), s);
        let d: PresetSpec = serde_json::from_str("{}").unwrap();
        assert_eq!(d, PresetSpec::default());
        let partial: PresetSpec = serde_json::from_str(r#"{"count":4}"#).unwrap();
        assert_eq!(partial.count, 4);
        assert_eq!(partial.family, PresetFamily::Grid);
        assert!(serde_json::from_str::<PresetSpec>(r#"{"family":"spiral"}"#).is_err());
        assert!(serde_json::from_str::<PresetSpec>(r#"{"count":-1}"#).is_err());
    }

    #[test]
    fn presets_work_with_the_zone_helpers() {
        use crate::geometry::PxRect;
        let work = PxRect::new(0, 0, 3440, 1400);
        let variant = PresetVariant::for_size(work.w, work.h);
        let l = spec(PresetFamily::MainSide, 4, variant, MainSize::Sixty);
        assert_eq!(variant, PresetVariant::Ultrawide);
        for z in &l.zones {
            let px = l.zone_pixels(z.id, work).unwrap();
            assert!(px.w >= 200 && px.h >= 500, "{px:?}");
        }
        // Every pixel belongs to exactly one zone.
        assert_eq!(l.hit_test_px(work, 10, 10), Some(1));
        assert_eq!(l.hit_test_px(work, 3439, 1399), Some(4));
    }
}
