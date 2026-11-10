//! Stack tab strips: what a stacked zone shows, laid out and rendered in software.
//!
//! When several windows share one zone only one of them is visible. A small strip of
//! tabs, one per window, sits on the zone's top edge so the others can be reached.
//! This module is the OS-independent part: the scene description ([`StripScene`]),
//! the geometry ([`layout`]), the hit test ([`StripLayout::tab_at`]) and the
//! rasteriser ([`render`]), which produces premultiplied ARGB pixels for a layered
//! window. The OS backend only creates the windows, supplies glyphs ([`Glyphs`]:
//! text coverage and program icons) and forwards clicks as [`StripEvent`]s.
//!
//! What a tab shows: the program's icon and a short program name derived from its
//! executable file name (`Code`, `Chrome`). **Never the window title.** In
//! share-safe mode ([`StripScene::hide_names`]) neither the icon nor the name is
//! drawn, only a neutral dot, so a screen share reveals nothing about the apps.
//!
//! Look: rounded translucent neutral pills with a one pixel hairline, the active tab
//! marked by an underline in the accent colour. Light and dark palettes follow the
//! OS theme ([`StripTheme`]). Every length is in logical pixels and multiplied by
//! the monitor's scale, so the strip is the same physical size on every display.

use krova_core::PxRect;

use crate::WindowId;

/// Which palette to draw with (the OS's app theme, or the user's override).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StripTheme {
    Light,
    Dark,
}

/// One tab: one window of the stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StripTab {
    pub window: WindowId,
    /// Short program name, see [`short_name`]. Never a window title.
    pub label: String,
    /// Path of the program, used only to find its icon (memory only, not drawn).
    pub icon_path: Option<String>,
    /// The tab whose window is showing.
    pub active: bool,
}

/// Everything one strip shows.
#[derive(Debug, Clone, PartialEq)]
pub struct StripScene {
    /// Identifies the strip across updates (the engine derives it from the monitor
    /// and the zone), so the backend can keep one window per strip.
    pub key: u64,
    /// The zone this stack fills, in screen pixels. The strip sits on its top edge.
    pub zone: PxRect,
    pub tabs: Vec<StripTab>,
    pub theme: StripTheme,
    /// Accent colour of the active tab's underline.
    pub accent: [u8; 3],
    /// Corner radius in logical pixels (the appearance setting).
    pub radius_px: i32,
    /// Device pixels per logical pixel of the monitor the zone is on.
    pub scale: f64,
    /// Share-safe: draw no names and no icons.
    pub hide_names: bool,
}

/// Something the user did to a strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StripEvent {
    /// A tab was clicked: show this window.
    Select { window: WindowId },
    /// The wheel turned over a strip: step to the next or previous tab.
    Cycle { key: u64, forward: bool },
}

/// The OS side of the strips: shows, moves and removes the native windows so that
/// they match the given scenes. Scenes the host does not know yet are created, ones
/// it knows are updated (only when changed), ones missing from the list are removed.
pub trait StripHost: Send + Sync {
    fn set_strips(&self, strips: Vec<StripScene>);
}

/// A coverage mask of some text: one byte per pixel, 255 = fully covered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mask {
    pub w: i32,
    pub h: i32,
    pub alpha: Vec<u8>,
}

/// A square program icon as premultiplied `0xAARRGGBB` pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Icon {
    pub size: i32,
    pub pixels: Vec<u32>,
}

/// What the rasteriser needs from the OS: text and icons.
pub trait Glyphs {
    /// `text` drawn with a font `px` pixels tall, at most `max_w` pixels wide (cut
    /// with an ellipsis if longer). `None` if it cannot be drawn.
    fn text(&self, text: &str, px: i32, max_w: i32) -> Option<Mask>;
    /// The icon of the program at `path`, scaled to `px` by `px`.
    fn icon(&self, path: &str, px: i32) -> Option<Icon>;
}

// ---- metrics (logical pixels) ------------------------------------------------

/// Distance from the zone's top edge to the strip.
const TOP_MARGIN: f64 = 6.0;
/// Height of a tab.
const TAB_H: f64 = 26.0;
/// Space around and between the tabs inside the strip window.
const PAD: f64 = 2.0;
const GAP: f64 = 4.0;
/// Width of a tab with room for a name, its smallest and its largest.
const TAB_MAX_W: f64 = 148.0;
const TAB_MIN_W: f64 = 84.0;
/// Width of a tab that shows only its icon.
const TAB_ICON_W: f64 = 34.0;
/// Narrowest tab that still hosts a dot or an icon at all.
const TAB_FLOOR_W: f64 = 18.0;
/// Room left free on each side of the strip: the window's own caption buttons and icon.
const SIDE_MARGIN: f64 = 96.0;
const ICON: f64 = 16.0;
const FONT: f64 = 12.0;
const UNDERLINE: f64 = 2.0;
/// Largest strip window, so a bogus scale or zone cannot ask for a huge bitmap.
const MAX_STRIP_PX: i32 = 8192;

fn scaled(v: f64, scale: f64) -> i32 {
    let s = if scale.is_finite() && scale > 0.0 {
        scale.min(8.0)
    } else {
        1.0
    };
    (v * s).round().max(1.0) as i32
}

/// A short, readable program name from an executable file name: the stem, with a
/// capital first letter. `chrome.exe` is `Chrome`, `Code - Insiders.exe` is
/// `Code - Insiders`. At most 24 characters.
pub fn short_name(exe: &str) -> String {
    let file = exe.rsplit(['\\', '/']).next().unwrap_or(exe);
    let stem = match file.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && ext.len() <= 4 => stem,
        _ => file,
    };
    let mut chars = stem.chars();
    let mut out = String::new();
    if let Some(first) = chars.next() {
        out.extend(first.to_uppercase());
        out.extend(chars);
    }
    if out.chars().count() > 24 {
        out = out.chars().take(23).collect();
        out.push('\u{2026}');
    }
    out
}

/// One tab's place inside the strip window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabBox {
    pub window: WindowId,
    /// Offset and width within the strip window, in pixels.
    pub x: i32,
    pub w: i32,
    pub active: bool,
    /// Too narrow for a name: only the icon (or dot) is drawn.
    pub icon_only: bool,
}

/// Where the strip window goes and where its tabs are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StripLayout {
    /// The strip window, in screen pixels.
    pub rect: PxRect,
    pub tabs: Vec<TabBox>,
    /// Pixel size of one tab's height, inside the window.
    pub tab_h: i32,
    pub pad: i32,
}

impl StripLayout {
    /// The index of the tab under `x` (pixels from the strip window's left edge).
    pub fn tab_at(&self, x: i32, y: i32) -> Option<usize> {
        if y < self.pad || y >= self.pad + self.tab_h {
            return None;
        }
        self.tabs.iter().position(|t| x >= t.x && x < t.x + t.w)
    }
}

/// Place a strip. `None` for a stack of fewer than two tabs (nothing to switch) or a
/// zone too small to hold one.
pub fn layout(scene: &StripScene) -> Option<StripLayout> {
    let n = scene.tabs.len();
    if n < 2 {
        return None;
    }
    let scale = scene.scale;
    let tab_h = scaled(TAB_H, scale);
    let pad = scaled(PAD, scale);
    let gap = scaled(GAP, scale);
    let side = scaled(SIDE_MARGIN, scale);
    let available = scene.zone.w.saturating_sub(2 * side);
    let count = i32::try_from(n).ok()?;
    let fixed = 2 * pad + gap * (count - 1);
    let floor = scaled(TAB_FLOOR_W, scale);
    if available < fixed + floor * count {
        return None;
    }

    // Widest tabs that fit, from "name shown" down to "icon only".
    let per_tab = (available - fixed) / count;
    let max_w = scaled(TAB_MAX_W, scale);
    let min_w = scaled(TAB_MIN_W, scale);
    let icon_w = scaled(TAB_ICON_W, scale);
    let (tab_w, icon_only) = if per_tab >= min_w {
        (per_tab.min(max_w), false)
    } else {
        (per_tab.min(icon_w).max(floor), true)
    };

    let total_w = fixed + tab_w * count;
    let total_h = tab_h + 2 * pad;
    if total_w > MAX_STRIP_PX || total_h > MAX_STRIP_PX {
        return None;
    }
    let x = scene.zone.x + (scene.zone.w - total_w) / 2;
    let y = scene.zone.y + scaled(TOP_MARGIN, scale);
    let mut tabs = Vec::with_capacity(n);
    let mut cursor = pad;
    for t in &scene.tabs {
        tabs.push(TabBox {
            window: t.window,
            x: cursor,
            w: tab_w,
            active: t.active,
            icon_only,
        });
        cursor += tab_w + gap;
    }
    Some(StripLayout {
        rect: PxRect::new(x, y, total_w, total_h),
        tabs,
        tab_h,
        pad,
    })
}

// ---- colours -----------------------------------------------------------------

/// Colours of one theme, straight (not premultiplied) RGB plus alpha in 0..=1.
struct Palette {
    surface: ([u8; 3], f32),
    surface_active: ([u8; 3], f32),
    hairline: ([u8; 3], f32),
    text_active: [u8; 3],
    text: ([u8; 3], f32),
}

fn palette(theme: StripTheme) -> Palette {
    match theme {
        StripTheme::Dark => Palette {
            surface: ([30, 30, 33], 0.78),
            surface_active: ([46, 46, 51], 0.92),
            hairline: ([255, 255, 255], 0.16),
            text_active: [240, 240, 243],
            text: ([240, 240, 243], 0.72),
        },
        StripTheme::Light => Palette {
            surface: ([248, 248, 250], 0.80),
            surface_active: ([255, 255, 255], 0.95),
            hairline: ([0, 0, 0], 0.14),
            text_active: [24, 24, 27],
            text: ([24, 24, 27], 0.68),
        },
    }
}

/// The colour the strip draws a tab's body with (for tests and the host's sanity checks).
pub fn surface_color(theme: StripTheme, active: bool) -> ([u8; 3], f32) {
    let p = palette(theme);
    if active {
        p.surface_active
    } else {
        p.surface
    }
}

// ---- rasteriser --------------------------------------------------------------

/// The finished strip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StripBitmap {
    pub w: i32,
    pub h: i32,
    /// Premultiplied `0xAARRGGBB`, row-major, top row first.
    pub pixels: Vec<u32>,
    pub layout: StripLayout,
}

struct Canvas {
    w: i32,
    h: i32,
    px: Vec<u32>,
}

impl Canvas {
    /// Blend a straight colour at `alpha` (0..=1) over the pixel (source over, premultiplied).
    fn blend(&mut self, x: i32, y: i32, color: [u8; 3], alpha: f32) {
        if x < 0 || y < 0 || x >= self.w || y >= self.h || alpha <= 0.0 {
            return;
        }
        let a = alpha.clamp(0.0, 1.0);
        let i = (y * self.w + x) as usize;
        let dst = self.px[i];
        let da = ((dst >> 24) & 0xFF) as f32 / 255.0;
        let ch = |shift: u32, src: u8| -> u32 {
            let d = ((dst >> shift) & 0xFF) as f32 / 255.0;
            let s = f32::from(src) / 255.0 * a;
            ((s + d * (1.0 - a)) * 255.0).round().clamp(0.0, 255.0) as u32
        };
        let out_a = ((a + da * (1.0 - a)) * 255.0).round().clamp(0.0, 255.0) as u32;
        self.px[i] =
            (out_a << 24) | (ch(16, color[0]) << 16) | (ch(8, color[1]) << 8) | ch(0, color[2]);
    }

    /// Blend a premultiplied pixel scaled by `k` (0..=1).
    fn blend_premul(&mut self, x: i32, y: i32, src: u32, k: f32) {
        if x < 0 || y < 0 || x >= self.w || y >= self.h || k <= 0.0 {
            return;
        }
        let sa = ((src >> 24) & 0xFF) as f32 / 255.0 * k;
        if sa <= 0.0 {
            return;
        }
        let i = (y * self.w + x) as usize;
        let dst = self.px[i];
        let mix = |shift: u32| -> u32 {
            let s = ((src >> shift) & 0xFF) as f32 / 255.0 * k;
            let d = ((dst >> shift) & 0xFF) as f32 / 255.0;
            ((s + d * (1.0 - sa)) * 255.0).round().clamp(0.0, 255.0) as u32
        };
        let da = ((dst >> 24) & 0xFF) as f32 / 255.0;
        let out_a = ((sa + da * (1.0 - sa)) * 255.0).round().clamp(0.0, 255.0) as u32;
        self.px[i] = (out_a << 24) | (mix(16) << 16) | (mix(8) << 8) | mix(0);
    }
}

/// Signed distance (pixels, negative inside) from the pixel centre `(px, py)` to a
/// rounded rectangle at `(x, y, w, h)` with corner radius `r`.
fn sdf_round_rect(px: f32, py: f32, x: f32, y: f32, w: f32, h: f32, r: f32) -> f32 {
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;
    let qx = (px - cx).abs() - (w / 2.0 - r);
    let qy = (py - cy).abs() - (h / 2.0 - r);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - r
}

fn fill_round_rect(c: &mut Canvas, r: PxRect, radius: f32, color: [u8; 3], alpha: f32) {
    let (x, y, w, h) = (r.x as f32, r.y as f32, r.w as f32, r.h as f32);
    let rad = radius.min(w / 2.0).min(h / 2.0).max(0.0);
    for py in r.y.max(0)..(r.y + r.h).min(c.h) {
        for px in r.x.max(0)..(r.x + r.w).min(c.w) {
            let d = sdf_round_rect(px as f32 + 0.5, py as f32 + 0.5, x, y, w, h, rad);
            c.blend(px, py, color, alpha * (0.5 - d).clamp(0.0, 1.0));
        }
    }
}

/// A one pixel ring just inside the edge of the rounded rectangle.
fn stroke_round_rect(c: &mut Canvas, r: PxRect, radius: f32, color: [u8; 3], alpha: f32) {
    let (x, y, w, h) = (r.x as f32, r.y as f32, r.w as f32, r.h as f32);
    let rad = radius.min(w / 2.0).min(h / 2.0).max(0.0);
    for py in r.y.max(0)..(r.y + r.h).min(c.h) {
        for px in r.x.max(0)..(r.x + r.w).min(c.w) {
            let d = sdf_round_rect(px as f32 + 0.5, py as f32 + 0.5, x, y, w, h, rad);
            // Covered between d = -1 and d = 0, with soft edges on both sides.
            let outer = (0.5 - d).clamp(0.0, 1.0);
            let inner = (0.5 - (d + 1.0)).clamp(0.0, 1.0);
            c.blend(px, py, color, alpha * (outer - inner).max(0.0));
        }
    }
}

fn draw_mask(c: &mut Canvas, mask: &Mask, x: i32, y: i32, color: [u8; 3], alpha: f32) {
    if mask.w <= 0 || mask.h <= 0 || mask.alpha.len() != (mask.w * mask.h) as usize {
        return;
    }
    for row in 0..mask.h {
        for col in 0..mask.w {
            let cov = f32::from(mask.alpha[(row * mask.w + col) as usize]) / 255.0;
            if cov > 0.0 {
                c.blend(x + col, y + row, color, alpha * cov);
            }
        }
    }
}

fn draw_icon(c: &mut Canvas, icon: &Icon, x: i32, y: i32, k: f32) {
    if icon.size <= 0 || icon.pixels.len() != (icon.size * icon.size) as usize {
        return;
    }
    for row in 0..icon.size {
        for col in 0..icon.size {
            c.blend_premul(
                x + col,
                y + row,
                icon.pixels[(row * icon.size + col) as usize],
                k,
            );
        }
    }
}

fn draw_dot(c: &mut Canvas, cx: i32, cy: i32, r: f32, color: [u8; 3], alpha: f32) {
    let reach = r.ceil() as i32 + 1;
    for py in (cy - reach)..=(cy + reach) {
        for px in (cx - reach)..=(cx + reach) {
            let d =
                (((px - cx) as f32 + 0.5 - 0.5).powi(2) + ((py - cy) as f32).powi(2)).sqrt() - r;
            c.blend(px, py, color, alpha * (0.5 - d).clamp(0.0, 1.0));
        }
    }
}

/// Rasterise a strip. `None` when [`layout`] has nothing to place.
pub fn render(scene: &StripScene, glyphs: &dyn Glyphs) -> Option<StripBitmap> {
    let lay = layout(scene)?;
    let (w, h) = (lay.rect.w, lay.rect.h);
    let mut canvas = Canvas {
        w,
        h,
        px: vec![0; (w as usize) * (h as usize)],
    };
    let pal = palette(scene.theme);
    let scale = scene.scale;
    let radius = scaled(f64::from(scene.radius_px.clamp(0, 16)), scale) as f32;
    let radius = if scene.radius_px <= 0 { 0.0 } else { radius };
    let icon_px = scaled(ICON, scale);
    let font_px = scaled(FONT, scale);
    let inner_pad = scaled(8.0, scale);
    let icon_gap = scaled(6.0, scale);

    for (tab, boxed) in scene.tabs.iter().zip(&lay.tabs) {
        let body = PxRect::new(boxed.x, lay.pad, boxed.w, lay.tab_h);
        let (fill, fill_alpha) = if tab.active {
            pal.surface_active
        } else {
            pal.surface
        };
        fill_round_rect(&mut canvas, body, radius, fill, fill_alpha);
        stroke_round_rect(&mut canvas, body, radius, pal.hairline.0, pal.hairline.1);

        let (text_color, text_alpha) = if tab.active {
            (pal.text_active, 1.0)
        } else {
            pal.text
        };
        let mid_y = body.y + body.h / 2;

        if scene.hide_names {
            // A neutral dot: the number of tabs and the active one are visible, the
            // apps are not.
            draw_dot(
                &mut canvas,
                body.x + body.w / 2,
                mid_y,
                scaled(3.0, scale) as f32,
                text_color,
                text_alpha * 0.8,
            );
        } else {
            let icon = tab
                .icon_path
                .as_deref()
                .and_then(|p| glyphs.icon(p, icon_px));
            let mut text_x = body.x + inner_pad;
            if boxed.icon_only {
                // Centre the icon (or the first letter, if there is no icon).
                match &icon {
                    Some(i) => draw_icon(
                        &mut canvas,
                        i,
                        body.x + (body.w - i.size) / 2,
                        mid_y - i.size / 2,
                        1.0,
                    ),
                    None => {
                        let initial: String = tab.label.chars().take(1).collect();
                        if let Some(m) = glyphs.text(&initial, font_px, body.w) {
                            draw_mask(
                                &mut canvas,
                                &m,
                                body.x + (body.w - m.w) / 2,
                                mid_y - m.h / 2,
                                text_color,
                                text_alpha,
                            );
                        }
                    }
                }
            } else {
                if let Some(i) = &icon {
                    draw_icon(&mut canvas, i, text_x, mid_y - i.size / 2, 1.0);
                    text_x += i.size + icon_gap;
                }
                let room = body.x + body.w - inner_pad - text_x;
                if room > 0 {
                    if let Some(m) = glyphs.text(&tab.label, font_px, room) {
                        draw_mask(
                            &mut canvas,
                            &m,
                            text_x,
                            mid_y - m.h / 2,
                            text_color,
                            text_alpha,
                        );
                    }
                }
            }
        }

        if tab.active {
            // The accent underline, inset by the corner radius so it sits in the
            // flat part of the pill's bottom edge.
            let thickness = scaled(UNDERLINE, scale);
            let inset = (radius.ceil() as i32)
                .max(scaled(6.0, scale))
                .min(body.w / 3);
            let bar = PxRect::new(
                body.x + inset,
                body.y + body.h - thickness - 1,
                (body.w - 2 * inset).max(2),
                thickness,
            );
            for py in bar.y..bar.y + bar.h {
                for px in bar.x..bar.x + bar.w {
                    canvas.blend(px, py, scene.accent, 1.0);
                }
            }
        }
    }

    Some(StripBitmap {
        w,
        h,
        pixels: canvas.px,
        layout: lay,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Glyphs that draw solid blocks and record what was asked for.
    #[derive(Default)]
    struct Blocks {
        texts: std::cell::RefCell<Vec<String>>,
        icons: std::cell::RefCell<Vec<String>>,
    }

    impl Glyphs for Blocks {
        fn text(&self, text: &str, px: i32, max_w: i32) -> Option<Mask> {
            self.texts.borrow_mut().push(text.to_owned());
            let w = (px * text.chars().count() as i32 / 2).min(max_w).max(1);
            Some(Mask {
                w,
                h: px,
                alpha: vec![255; (w * px) as usize],
            })
        }

        fn icon(&self, path: &str, px: i32) -> Option<Icon> {
            self.icons.borrow_mut().push(path.to_owned());
            Some(Icon {
                size: px,
                pixels: vec![0xFF_80_40_20; (px * px) as usize],
            })
        }
    }

    fn tab(id: u64, label: &str, active: bool) -> StripTab {
        StripTab {
            window: WindowId(id),
            label: label.to_owned(),
            icon_path: Some(format!("C:\\apps\\{label}.exe")),
            active,
        }
    }

    fn scene(n: usize, zone_w: i32, scale: f64) -> StripScene {
        StripScene {
            key: 7,
            zone: PxRect::new(100, 50, zone_w, 900),
            tabs: (0..n)
                .map(|i| tab(i as u64 + 1, &format!("App{i}"), i == 0))
                .collect(),
            theme: StripTheme::Dark,
            accent: [0xff, 0x5b, 0x1f],
            radius_px: 8,
            scale,
            hide_names: false,
        }
    }

    #[test]
    fn short_names_come_from_the_executable_not_from_any_title() {
        assert_eq!(short_name("chrome.exe"), "Chrome");
        assert_eq!(short_name("Code - Insiders.exe"), "Code - Insiders");
        assert_eq!(
            short_name("C:\\Program Files\\App\\notepad++.exe"),
            "Notepad++"
        );
        assert_eq!(short_name("safari"), "Safari");
        assert_eq!(short_name(".hidden"), ".hidden");
        assert_eq!(short_name(""), "");
        assert_eq!(
            short_name("a-very-long-program-name-indeed.exe")
                .chars()
                .count(),
            24
        );
        assert!(short_name("a-very-long-program-name-indeed.exe").ends_with('\u{2026}'));
    }

    #[test]
    fn a_stack_of_fewer_than_two_has_no_strip() {
        assert_eq!(layout(&scene(0, 1200, 1.0)), None);
        assert_eq!(layout(&scene(1, 1200, 1.0)), None);
        assert!(layout(&scene(2, 1200, 1.0)).is_some());
    }

    #[test]
    fn the_strip_is_centred_on_the_zones_top_edge() {
        let s = scene(3, 1200, 1.0);
        let l = layout(&s).unwrap();
        let zone_mid = s.zone.x + s.zone.w / 2;
        let strip_mid = l.rect.x + l.rect.w / 2;
        assert!(
            (zone_mid - strip_mid).abs() <= 1,
            "{zone_mid} vs {strip_mid}"
        );
        assert_eq!(l.rect.y, s.zone.y + 6);
        assert_eq!(l.tab_h, 26);
        assert_eq!(l.rect.h, 26 + 2 * 2);
        // Wide zone: tabs are at their largest and show names.
        assert!(l.tabs.iter().all(|t| t.w == 148 && !t.icon_only));
        // Tabs do not overlap and keep the gap.
        for pair in l.tabs.windows(2) {
            assert_eq!(pair[1].x - (pair[0].x + pair[0].w), 4);
        }
        assert_eq!(l.rect.w, 2 * 2 + 3 * 148 + 2 * 4);
    }

    #[test]
    fn tabs_shrink_to_fit_then_show_only_icons() {
        // 5 tabs in a 800 px zone (704 px usable after the side margins).
        let l = layout(&scene(5, 800, 1.0)).unwrap();
        assert!(l.tabs.iter().all(|t| !t.icon_only), "{:?}", l.tabs[0]);
        assert!(l.tabs[0].w >= 84 && l.tabs[0].w < 148);
        assert!(l.rect.w <= 800 - 2 * 96);
        // 12 tabs in the same zone: too many for names.
        let l = layout(&scene(12, 800, 1.0)).unwrap();
        assert!(l.tabs.iter().all(|t| t.icon_only));
        assert!(l.tabs[0].w <= 34 && l.tabs[0].w >= 18);
        assert!(l.rect.w <= 800 - 2 * 96);
    }

    #[test]
    fn a_zone_too_small_for_the_tabs_has_no_strip() {
        assert_eq!(
            layout(&scene(2, 150, 1.0)),
            None,
            "less than the side margins"
        );
        assert_eq!(layout(&scene(32, 400, 1.0)), None);
    }

    #[test]
    fn everything_scales_with_the_monitor() {
        let one = layout(&scene(3, 1200, 1.0)).unwrap();
        let two = layout(&scene(3, 2400, 2.0)).unwrap();
        assert_eq!(two.tab_h, one.tab_h * 2);
        assert_eq!(two.rect.h, one.rect.h * 2);
        assert_eq!(two.tabs[0].w, one.tabs[0].w * 2);
        assert_eq!(two.rect.y - 50, (one.rect.y - 50) * 2);
        // Nonsense scales fall back to 1.
        for bad in [0.0, -2.0, f64::NAN, f64::INFINITY] {
            let l = layout(&scene(3, 1200, bad)).unwrap();
            assert_eq!(l.tab_h, 26, "scale {bad}");
        }
    }

    #[test]
    fn a_click_finds_its_tab() {
        let l = layout(&scene(3, 1200, 1.0)).unwrap();
        let t1 = l.tabs[1];
        assert_eq!(l.tab_at(t1.x + 10, 10), Some(1));
        assert_eq!(l.tab_at(t1.x + t1.w - 1, 10), Some(1));
        assert_eq!(l.tab_at(t1.x + t1.w, 10), None, "the gap between tabs");
        assert_eq!(l.tab_at(0, 10), None, "the padding at the left");
        assert_eq!(l.tab_at(t1.x + 10, 0), None, "above the tabs");
        assert_eq!(l.tab_at(t1.x + 10, l.pad + l.tab_h), None, "below the tabs");
        assert_eq!(l.tab_at(-5, 10), None);
    }

    fn alpha(b: &StripBitmap, x: i32, y: i32) -> u32 {
        b.pixels[(y * b.w + x) as usize] >> 24
    }

    #[test]
    fn the_gaps_between_tabs_stay_transparent_so_clicks_pass_through() {
        let s = scene(3, 1200, 1.0);
        let b = render(&s, &Blocks::default()).unwrap();
        let l = &b.layout;
        let gap_x = l.tabs[0].x + l.tabs[0].w + 2;
        let mid_y = l.pad + l.tab_h / 2;
        assert_eq!(alpha(&b, gap_x, mid_y), 0, "gap");
        assert_eq!(alpha(&b, 0, 0), 0, "corner of the window");
        let in_tab = l.tabs[1].x + l.tabs[1].w / 2;
        assert!(
            alpha(&b, in_tab, l.pad + 3) > 100,
            "tab body is solid enough"
        );
        // Pill corners are rounded: the very corner pixel is not covered.
        assert!(alpha(&b, l.tabs[1].x, l.pad) < 40, "rounded corner");
    }

    #[test]
    fn only_the_active_tab_has_the_accent_underline() {
        let s = scene(3, 1200, 1.0);
        let b = render(&s, &Blocks::default()).unwrap();
        let l = &b.layout;
        let y = l.pad + l.tab_h - 3;
        let accent = 0xFF_FF_5B_1F_u32;
        let px = |tab: usize| b.pixels[(y * b.w + l.tabs[tab].x + l.tabs[tab].w / 2) as usize];
        assert_eq!(px(0), accent, "the active tab");
        assert_ne!(px(1), accent);
        assert_ne!(px(2), accent);
    }

    #[test]
    fn names_and_icons_are_drawn_from_the_program_never_a_title() {
        let g = Blocks::default();
        render(&scene(3, 1200, 1.0), &g).unwrap();
        assert_eq!(*g.texts.borrow(), ["App0", "App1", "App2"]);
        assert_eq!(g.icons.borrow().len(), 3);
        assert!(g.icons.borrow()[0].ends_with("App0.exe"));
    }

    #[test]
    fn share_safe_draws_no_names_and_no_icons() {
        let g = Blocks::default();
        let mut s = scene(3, 1200, 1.0);
        s.hide_names = true;
        let b = render(&s, &g).unwrap();
        assert!(g.texts.borrow().is_empty(), "no text was even requested");
        assert!(g.icons.borrow().is_empty(), "no icon was even requested");
        // The tabs are still there, with a dot each.
        let l = &b.layout;
        let mid = (l.tabs[1].x + l.tabs[1].w / 2, l.pad + l.tab_h / 2 - 1);
        let centre = b.pixels[(mid.1 * b.w + mid.0) as usize];
        let body_only = b.pixels[((l.pad + 3) * b.w + mid.0) as usize];
        assert_ne!(centre, body_only, "a dot marks the middle of the tab");
    }

    #[test]
    fn narrow_tabs_draw_only_an_icon_or_an_initial() {
        let g = Blocks::default();
        render(&scene(12, 800, 1.0), &g).unwrap();
        assert!(g.texts.borrow().is_empty(), "icons stood in for the names");
        assert_eq!(g.icons.borrow().len(), 12);

        struct NoIcons(Blocks);
        impl Glyphs for NoIcons {
            fn text(&self, t: &str, px: i32, w: i32) -> Option<Mask> {
                self.0.text(t, px, w)
            }
            fn icon(&self, _: &str, _: i32) -> Option<Icon> {
                None
            }
        }
        let g = NoIcons(Blocks::default());
        render(&scene(12, 800, 1.0), &g).unwrap();
        assert_eq!(g.0.texts.borrow().len(), 12);
        assert!(g.0.texts.borrow().iter().all(|t| t == "A"));
    }

    #[test]
    fn light_and_dark_use_different_palettes() {
        let mut dark = scene(2, 1200, 1.0);
        dark.theme = StripTheme::Dark;
        let mut light = dark.clone();
        light.theme = StripTheme::Light;
        let d = render(&dark, &Blocks::default()).unwrap();
        let l = render(&light, &Blocks::default()).unwrap();
        assert_ne!(d.pixels, l.pixels);
        assert!(surface_color(StripTheme::Dark, true).0[0] < 128);
        assert!(surface_color(StripTheme::Light, true).0[0] > 200);
    }

    #[test]
    fn the_bitmap_is_premultiplied_and_sized_to_the_window() {
        let b = render(&scene(4, 1500, 1.5), &Blocks::default()).unwrap();
        assert_eq!(b.pixels.len(), (b.w * b.h) as usize);
        assert_eq!((b.w, b.h), (b.layout.rect.w, b.layout.rect.h));
        for p in &b.pixels {
            let a = p >> 24;
            for shift in [16, 8, 0] {
                assert!((p >> shift) & 0xFF <= a, "channel above alpha in {p:08x}");
            }
        }
    }

    #[test]
    fn zero_radius_gives_square_tabs() {
        let mut s = scene(2, 1200, 1.0);
        s.radius_px = 0;
        let b = render(&s, &Blocks::default()).unwrap();
        let l = &b.layout;
        assert!(
            alpha(&b, l.tabs[1].x + 1, l.pad + 1) > 0,
            "corner is filled"
        );
    }

    #[test]
    fn rendering_is_deterministic() {
        let s = scene(3, 1300, 1.25);
        let a = render(&s, &Blocks::default()).unwrap();
        let b = render(&s, &Blocks::default()).unwrap();
        assert_eq!(a, b);
    }
}
