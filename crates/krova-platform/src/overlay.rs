//! Drag-highlight overlay: the OS-independent part.
//!
//! The overlay shows the zones of one monitor while a window is dragged. The
//! scene is described by [`OverlayScene`] and rasterised in software by
//! [`render`] into premultiplied ARGB pixels, so each OS backend only has to put
//! a ready-made bitmap on a transparent, click-through, topmost window. It is
//! repainted only when the scene changes (zone highlight, monitor switch).

use krova_core::PxRect;

/// Default highlight colour, used when the configured one does not parse.
const DEFAULT_COLOR: [u8; 3] = [0x3b, 0x82, 0xf6];

/// Space between neighbouring zones, so they read as separate tiles.
const GAP: i32 = 4;
const BORDER: i32 = 2;
const BORDER_ACTIVE: i32 = 4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlayStyle {
    pub color: [u8; 3],
    /// Fill opacity of the highlighted zone, `0.0..=1.0`.
    pub opacity: f32,
}

impl OverlayStyle {
    /// Build from the settings strings; an unparsable colour falls back to blue.
    pub fn from_settings(hex: &str, opacity: f32) -> Self {
        Self {
            color: parse_hex_color(hex).unwrap_or(DEFAULT_COLOR),
            opacity: if opacity.is_finite() {
                opacity.clamp(0.0, 1.0)
            } else {
                0.35
            },
        }
    }
}

/// Parse `#rrggbb` or `rrggbb`.
pub fn parse_hex_color(s: &str) -> Option<[u8; 3]> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 || !h.is_ascii() {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

/// Everything the overlay shows. All rects are in screen pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct OverlayScene {
    /// The monitor the overlay window covers.
    pub monitor: PxRect,
    pub zones: Vec<(u32, PxRect)>,
    pub highlight: Option<u32>,
    /// Edge-snap preview, drawn like a highlighted zone.
    pub preview: Option<PxRect>,
    pub style: OverlayStyle,
}

/// A backend that can display an [`OverlayScene`]; `None` hides the overlay.
pub trait Overlay: Send + Sync {
    fn set_scene(&self, scene: Option<OverlayScene>);
}

/// Premultiplied `0xAARRGGBB` for `color` at `alpha` (`0.0..=1.0`).
fn premul(color: [u8; 3], alpha: f32) -> u32 {
    let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u32;
    let c = |v: u8| (u32::from(v) * a + 127) / 255;
    (a << 24) | (c(color[0]) << 16) | (c(color[1]) << 8) | c(color[2])
}

struct Canvas {
    w: i32,
    h: i32,
    px: Vec<u32>,
}

impl Canvas {
    fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, value: u32) {
        let x0 = x.max(0);
        let y0 = y.max(0);
        let x1 = (x + w).min(self.w);
        let y1 = (y + h).min(self.h);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        for row in y0..y1 {
            let start = row as usize * self.w as usize + x0 as usize;
            self.px[start..start + (x1 - x0) as usize].fill(value);
        }
    }

    fn stroke(&mut self, r: PxRect, thickness: i32, value: u32) {
        let t = thickness.min(r.w / 2).min(r.h / 2).max(1);
        self.fill(r.x, r.y, r.w, t, value);
        self.fill(r.x, r.y + r.h - t, r.w, t, value);
        self.fill(r.x, r.y + t, t, r.h - 2 * t, value);
        self.fill(r.x + r.w - t, r.y + t, t, r.h - 2 * t, value);
    }

    fn tile(&mut self, r: PxRect, fill: u32, border: i32, border_value: u32) {
        self.fill(r.x, r.y, r.w, r.h, fill);
        self.stroke(r, border, border_value);
    }
}

/// Shrink `r` by the zone gap, never below a visible size.
fn inset(r: PxRect) -> PxRect {
    let d = GAP.min(r.w / 4).min(r.h / 4).max(0);
    PxRect::new(r.x + d, r.y + d, r.w - 2 * d, r.h - 2 * d)
}

/// Largest overlay bitmap that will be rendered (about 256 MB of pixels). Real monitors
/// are far below it (8K is 33 Mpx); it only stops a bogus monitor rect from asking for
/// gigabytes. Above it the overlay is simply not drawn.
pub const MAX_PIXELS: usize = 64 * 1024 * 1024;

/// Rasterise the scene into `monitor.w * monitor.h` premultiplied ARGB pixels,
/// row-major, top row first. Returns an empty buffer for an empty or implausibly
/// large monitor.
pub fn render(scene: &OverlayScene) -> Vec<u32> {
    let (w, h) = (scene.monitor.w, scene.monitor.h);
    if w <= 0 || h <= 0 {
        return Vec::new();
    }
    let count = (w as usize).saturating_mul(h as usize);
    if count > MAX_PIXELS {
        return Vec::new();
    }
    let mut canvas = Canvas {
        w,
        h,
        px: vec![0; count],
    };
    let local = |r: PxRect| {
        inset(PxRect::new(
            r.x.saturating_sub(scene.monitor.x),
            r.y.saturating_sub(scene.monitor.y),
            r.w,
            r.h,
        ))
    };
    let style = scene.style;

    let idle_fill = premul(style.color, style.opacity * 0.3);
    let idle_border = premul(style.color, 0.8);
    let active_fill = premul(style.color, style.opacity);
    let active_border = premul(style.color, 1.0);

    for (id, rect) in &scene.zones {
        if scene.highlight != Some(*id) {
            canvas.tile(local(*rect), idle_fill, BORDER, idle_border);
        }
    }
    // Highlight and preview last so they sit on top of their neighbours.
    for (id, rect) in &scene.zones {
        if scene.highlight == Some(*id) {
            canvas.tile(local(*rect), active_fill, BORDER_ACTIVE, active_border);
        }
    }
    if let Some(p) = scene.preview {
        canvas.tile(local(p), active_fill, BORDER_ACTIVE, active_border);
    }
    canvas.px
}

#[cfg(test)]
mod tests {
    use super::*;

    const STYLE: OverlayStyle = OverlayStyle {
        color: [255, 255, 255],
        opacity: 0.4,
    };

    fn scene(monitor: PxRect, zones: Vec<(u32, PxRect)>, highlight: Option<u32>) -> OverlayScene {
        OverlayScene {
            monitor,
            zones,
            highlight,
            preview: None,
            style: STYLE,
        }
    }

    fn at(buf: &[u32], w: i32, x: i32, y: i32) -> u32 {
        buf[(y * w + x) as usize]
    }

    fn alpha(p: u32) -> u32 {
        p >> 24
    }

    #[test]
    fn hex_colours() {
        assert_eq!(parse_hex_color("#3b82f6"), Some([0x3b, 0x82, 0xf6]));
        assert_eq!(parse_hex_color("FFAA00"), Some([255, 170, 0]));
        assert_eq!(parse_hex_color("  #000000 "), Some([0, 0, 0]));
        assert_eq!(parse_hex_color("#fff"), None);
        assert_eq!(parse_hex_color("#gggggg"), None);
        assert_eq!(parse_hex_color("#12345é"), None);
        assert_eq!(parse_hex_color(""), None);
    }

    #[test]
    fn style_falls_back_and_clamps() {
        let s = OverlayStyle::from_settings("nonsense", 7.0);
        assert_eq!(s.color, DEFAULT_COLOR);
        assert_eq!(s.opacity, 1.0);
        assert_eq!(
            OverlayStyle::from_settings("#000000", f32::NAN).opacity,
            0.35
        );
        assert_eq!(OverlayStyle::from_settings("#000000", -1.0).opacity, 0.0);
    }

    #[test]
    fn premultiplied_values() {
        assert_eq!(premul([255, 255, 255], 1.0), 0xFFFF_FFFF);
        assert_eq!(premul([255, 0, 0], 0.0), 0);
        let p = premul([200, 100, 0], 0.5);
        assert_eq!(alpha(p), 128);
        assert_eq!((p >> 16) & 0xFF, (200 * 128 + 127) / 255);
    }

    #[test]
    fn buffer_has_monitor_dimensions_and_starts_transparent() {
        let s = scene(PxRect::new(0, 0, 200, 100), vec![], None);
        let buf = render(&s);
        assert_eq!(buf.len(), 200 * 100);
        assert!(buf.iter().all(|&p| p == 0));
        assert!(render(&scene(PxRect::new(0, 0, 0, 100), vec![], None)).is_empty());
        assert!(render(&scene(PxRect::new(0, 0, 100, -5), vec![], None)).is_empty());
    }

    #[test]
    fn highlighted_zone_is_stronger_than_idle_zones() {
        let zones = vec![
            (1, PxRect::new(0, 0, 100, 100)),
            (2, PxRect::new(100, 0, 100, 100)),
        ];
        let buf = render(&scene(PxRect::new(0, 0, 200, 100), zones, Some(2)));
        let idle = alpha(at(&buf, 200, 50, 50));
        let active = alpha(at(&buf, 200, 150, 50));
        assert!(idle > 0, "idle zones are visible");
        assert!(active > idle, "highlight is stronger: {active} vs {idle}");
        assert_eq!(active, (0.4f32 * 255.0).round() as u32);
    }

    #[test]
    fn borders_are_opaque_and_gaps_stay_clear() {
        let zones = vec![(1, PxRect::new(0, 0, 100, 100))];
        let buf = render(&scene(PxRect::new(0, 0, 100, 100), zones, None));
        // GAP of 4 px: the outer ring is transparent, the border starts inside it.
        assert_eq!(at(&buf, 100, 1, 50), 0);
        assert_eq!(alpha(at(&buf, 100, 4, 50)), (0.8f32 * 255.0).round() as u32);
    }

    #[test]
    fn zones_are_drawn_relative_to_the_monitor_origin() {
        let monitor = PxRect::new(1920, 0, 200, 100);
        let zones = vec![(1, PxRect::new(1920, 0, 100, 100))];
        let buf = render(&scene(monitor, zones, Some(1)));
        assert!(alpha(at(&buf, 200, 50, 50)) > 0);
        assert_eq!(alpha(at(&buf, 200, 150, 50)), 0);
    }

    #[test]
    fn preview_rect_is_drawn_like_a_highlight() {
        let mut s = scene(PxRect::new(0, 0, 200, 100), vec![], None);
        s.preview = Some(PxRect::new(0, 0, 100, 100));
        let buf = render(&s);
        assert_eq!(
            alpha(at(&buf, 200, 50, 50)),
            (0.4f32 * 255.0).round() as u32
        );
        assert_eq!(at(&buf, 200, 150, 50), 0);
    }

    #[test]
    fn rects_outside_or_degenerate_never_panic() {
        let zones = vec![
            (1, PxRect::new(-500, -500, 300, 300)),
            (2, PxRect::new(150, 50, 400, 400)),
            (3, PxRect::new(10, 10, 0, 0)),
            (4, PxRect::new(10, 10, 3, 3)),
        ];
        let buf = render(&scene(PxRect::new(0, 0, 200, 100), zones, Some(2)));
        assert_eq!(buf.len(), 200 * 100);
    }

    #[test]
    fn an_implausibly_large_monitor_is_not_rendered() {
        // 100_000 x 100_000 would be 40 GB of pixels.
        let huge = scene(PxRect::new(0, 0, 100_000, 100_000), vec![], None);
        assert!(render(&huge).is_empty());
        // i32::MAX squared must not overflow the size computation either.
        let absurd = scene(PxRect::new(0, 0, i32::MAX, i32::MAX), vec![], None);
        assert!(render(&absurd).is_empty());
    }

    #[test]
    fn an_8k_monitor_is_still_rendered() {
        // 7680 x 4320 = 33 Mpx, under the cap.
        let s = scene(PxRect::new(0, 0, 7680, 4320), vec![], None);
        assert_eq!(render(&s).len(), 7680 * 4320);
    }
}
