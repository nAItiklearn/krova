//! Wave 3 visuals: focus dim, window highlight, calm desk and share-safe settings,
//! plus the pure maths their native overlays use (spring curve, cut-out and ring
//! geometry, which drawing backend to use).
//!
//! The platform layer only draws; every decision that can be tested lives here.

use serde::{Deserialize, Serialize};

use crate::config::{normalize_hex_color, Settings};
use crate::geometry::PxRect;

/// Accent used before (or without) `Settings.appearance.accent`.
pub const DEFAULT_ACCENT: &str = "#ff5b1f";
/// Corner radius used before (or without) `Settings.appearance.corner_radius_px`.
pub const DEFAULT_CORNER_RADIUS_PX: u32 = 10;
/// Largest corner radius taken from the appearance settings.
pub const MAX_CORNER_RADIUS_PX: u32 = 32;

pub const MIN_DIM_OPACITY: f32 = 0.2;
pub const MAX_DIM_OPACITY: f32 = 0.8;
pub const DEFAULT_DIM_OPACITY: f32 = 0.45;

pub const MIN_HIGHLIGHT_WIDTH: u32 = 1;
pub const MAX_HIGHLIGHT_WIDTH: u32 = 6;
pub const DEFAULT_HIGHLIGHT_WIDTH: u32 = 2;

/// First Windows 11 build: rounded window corners and `DWMWA_BORDER_COLOR`.
pub const WIN11_BUILD: u32 = 22000;
/// The DWM border is about one pixel wide; wider borders need the overlay.
pub const MAX_DWM_BORDER_WIDTH: u32 = 2;

/// How long a fade or a border move takes (the spring has settled by then).
pub const SPRING_MS: u32 = 220;
/// Damping ratio of the shared spring (the polish bar asks for 0.8-0.95).
pub const SPRING_DAMPING: f64 = 0.88;

// ---------------------------------------------------------------- settings

/// Focus dim: everything except the focused window is darkened.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FocusSettings {
    /// Off by default; toggled by a command (and later Ctrl+Alt+F).
    pub enabled: bool,
    /// How dark the rest of the screen gets, 0.2..=0.8.
    pub opacity: f32,
}

impl Default for FocusSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            opacity: DEFAULT_DIM_OPACITY,
        }
    }
}

impl FocusSettings {
    pub fn sanitized(mut self) -> Self {
        self.opacity = if self.opacity.is_finite() {
            self.opacity.clamp(MIN_DIM_OPACITY, MAX_DIM_OPACITY)
        } else {
            DEFAULT_DIM_OPACITY
        };
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HighlightMode {
    #[default]
    None,
    /// A border around the focused window.
    Active,
    /// A border around every arrangeable window.
    All,
    /// A soft glow around the focused window.
    Glow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HighlightSettings {
    pub mode: HighlightMode,
    /// `#rrggbb`, or `None` for the appearance accent.
    pub color: Option<String>,
    /// Border width in pixels at 100 % scale, 1..=6.
    pub width_px: u32,
}

impl Default for HighlightSettings {
    fn default() -> Self {
        Self {
            mode: HighlightMode::None,
            color: None,
            width_px: DEFAULT_HIGHLIGHT_WIDTH,
        }
    }
}

impl HighlightSettings {
    pub fn sanitized(mut self) -> Self {
        self.width_px = self
            .width_px
            .clamp(MIN_HIGHLIGHT_WIDTH, MAX_HIGHLIGHT_WIDTH);
        self.color = self.color.as_deref().and_then(normalize_hex_color);
        self
    }
}

/// Calm desk: minimise every window that is not in a zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CalmDeskSettings {
    /// Act on every monitor (`true`) or only on the monitor under the cursor.
    pub all_monitors: bool,
}

impl Default for CalmDeskSettings {
    fn default() -> Self {
        Self { all_monitors: true }
    }
}

// -------------------------------------------------------------- appearance

/// Accent colour and corner radius of the app's visual system. Read through JSON
/// so this works whether or not `Settings` has an `appearance` block yet; anything
/// missing or invalid falls back to [`DEFAULT_ACCENT`] / [`DEFAULT_CORNER_RADIUS_PX`].
pub fn appearance(settings: &Settings) -> (String, u32) {
    let value = serde_json::to_value(settings).unwrap_or(serde_json::Value::Null);
    let block = value.get("appearance");
    let accent = block
        .and_then(|a| a.get("accent"))
        .and_then(serde_json::Value::as_str)
        .and_then(normalize_hex_color)
        .unwrap_or_else(|| DEFAULT_ACCENT.to_owned());
    let radius = block
        .and_then(|a| a.get("corner_radius_px"))
        .and_then(serde_json::Value::as_u64)
        .map_or(DEFAULT_CORNER_RADIUS_PX, |r| {
            u32::try_from(r)
                .unwrap_or(MAX_CORNER_RADIUS_PX)
                .min(MAX_CORNER_RADIUS_PX)
        });
    (accent, radius)
}

/// Whether share-safe is on in these settings, read through JSON for the same
/// reason as [`appearance`]. Every overlay that could show an app name, an icon
/// or window contents asks this first and shows only zone shapes when it is true.
pub fn share_safe(settings: &Settings) -> bool {
    serde_json::to_value(settings)
        .ok()
        .and_then(|v| v.get("share_safe").and_then(serde_json::Value::as_bool))
        .unwrap_or(false)
}

/// `#rrggbb` as `(r, g, b)`.
pub fn hex_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let h = normalize_hex_color(hex)?;
    let n = u32::from_str_radix(&h[1..], 16).ok()?;
    Some(((n >> 16) as u8, (n >> 8) as u8, n as u8))
}

// ------------------------------------------------------------------ spring

/// Progress (0..=1, may overshoot slightly) of the shared spring `t_ms` after a
/// change started. Exactly 1 from [`SPRING_MS`] on, so an animation always ends
/// on its target and its timer can stop.
pub fn spring_progress(t_ms: u32) -> f64 {
    if t_ms >= SPRING_MS {
        return 1.0;
    }
    // Natural frequency chosen so the envelope has decayed to ~1 % at SPRING_MS.
    let zeta = SPRING_DAMPING;
    let t = f64::from(t_ms) / 1000.0;
    let settle = f64::from(SPRING_MS) / 1000.0;
    let omega = 4.6 / (zeta * settle);
    let wd = omega * (1.0 - zeta * zeta).sqrt();
    let envelope = (-zeta * omega * t).exp();
    1.0 - envelope * ((wd * t).cos() + (zeta * omega / wd) * (wd * t).sin())
}

/// A value moving from `from` to `to` along the spring.
pub fn spring_mix(from: f64, to: f64, t_ms: u32) -> f64 {
    from + (to - from) * spring_progress(t_ms)
}

/// A rect moving from `from` to `to` along the spring.
pub fn spring_rect(from: PxRect, to: PxRect, t_ms: u32) -> PxRect {
    let m = |a: i32, b: i32| spring_mix(f64::from(a), f64::from(b), t_ms).round() as i32;
    PxRect::new(
        m(from.x, to.x),
        m(from.y, to.y),
        m(from.w, to.w).max(0),
        m(from.h, to.h).max(0),
    )
}

/// Layered-window alpha (0..=255) for an opacity.
pub fn alpha_byte(opacity: f64) -> u8 {
    (opacity.clamp(0.0, 1.0) * 255.0).round() as u8
}

// ---------------------------------------------------------------- geometry

/// Intersection of two rects, `None` if they do not overlap.
pub fn intersect(a: PxRect, b: PxRect) -> Option<PxRect> {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let r = a.right().min(b.right());
    let btm = a.bottom().min(b.bottom());
    (r > x && btm > y).then(|| PxRect::new(x, y, r - x, btm - y))
}

/// The hole a monitor's dim overlay leaves for the focused window, relative to the
/// monitor's top-left corner. `None`: dim the whole monitor.
pub fn dim_hole(monitor: PxRect, focused: Option<PxRect>) -> Option<PxRect> {
    let hit = intersect(monitor, focused?)?;
    Some(PxRect::new(
        hit.x - monitor.x,
        hit.y - monitor.y,
        hit.w,
        hit.h,
    ))
}

/// Grow a rect by `by` pixels on every side.
pub fn outset(r: PxRect, by: i32) -> PxRect {
    PxRect::new(r.x - by, r.y - by, r.w + 2 * by, r.h + 2 * by)
}

/// Scale a 100 %-scale length to the monitor's DPI (at least 1 px).
pub fn scaled(px: u32, scale: f64) -> i32 {
    let s = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    ((f64::from(px) * s).round() as i32).max(1)
}

/// Corner radius of ordinary windows on this Windows build at this scale: Windows
/// 11 rounds top-level windows by 8 px; earlier builds draw square corners.
pub fn window_corner_radius(build: u32, scale: f64) -> i32 {
    if build >= WIN11_BUILD {
        scaled(8, scale)
    } else {
        0
    }
}

/// Is a window covering its whole monitor (borderless full screen, a game, a video)?
/// Highlights hide for those.
pub fn is_fullscreen(window: PxRect, monitor: PxRect) -> bool {
    window.x <= monitor.x
        && window.y <= monitor.y
        && window.right() >= monitor.right()
        && window.bottom() >= monitor.bottom()
}

/// How the highlight is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighlightBackend {
    Off,
    /// Windows 11 draws the border itself (`DWMWA_BORDER_COLOR`) on the focused window.
    DwmActive,
    /// The same on every arrangeable window.
    DwmAll,
    /// A click-through overlay ring following the focused window.
    OverlayActive,
    /// One overlay ring per arrangeable window.
    OverlayAll,
    /// A soft glow overlay around the focused window.
    Glow,
}

/// DWM borders are preferred on Windows 11 for thin borders; wider ones, older
/// builds and the glow use the overlay.
pub fn highlight_backend(h: &HighlightSettings, build: u32) -> HighlightBackend {
    let dwm = build >= WIN11_BUILD && h.width_px <= MAX_DWM_BORDER_WIDTH;
    match (h.mode, dwm) {
        (HighlightMode::None, _) => HighlightBackend::Off,
        (HighlightMode::Active, true) => HighlightBackend::DwmActive,
        (HighlightMode::Active, false) => HighlightBackend::OverlayActive,
        (HighlightMode::All, true) => HighlightBackend::DwmAll,
        (HighlightMode::All, false) => HighlightBackend::OverlayAll,
        (HighlightMode::Glow, _) => HighlightBackend::Glow,
    }
}

/// Alpha (0..=255) of a glow pixel `d` pixels outside the window edge, for a glow
/// `reach` pixels wide: strongest at the edge, smoothly to zero at `reach`.
pub fn glow_alpha(d: f64, reach: f64) -> u8 {
    if reach <= 0.0 || d >= reach {
        return 0;
    }
    let t = (1.0 - d.max(0.0) / reach).clamp(0.0, 1.0);
    // Smoothstep squared: soft falloff with no visible band.
    let s = t * t * (3.0 - 2.0 * t);
    alpha_byte(0.55 * s * s)
}

/// Distance from a point to the outside of a rounded rect `(w, h, radius)` whose
/// top-left is (0, 0): 0 inside, growing outward.
pub fn rounded_rect_distance(x: f64, y: f64, w: f64, h: f64, radius: f64) -> f64 {
    let r = radius.clamp(0.0, (w.min(h) / 2.0).max(0.0));
    let cx = x.clamp(r, (w - r).max(r));
    let cy = y.clamp(r, (h - r).max(r));
    let dx = x - cx;
    let dy = y - cy;
    let inside_core = (0.0..=w).contains(&x) && (0.0..=h).contains(&y);
    let d = (dx * dx + dy * dy).sqrt() - r;
    if inside_core && d <= 0.0 {
        0.0
    } else {
        d.max(0.0)
    }
}

/// Everything the native visuals thread needs, in plain values.
#[derive(Debug, Clone, PartialEq)]
pub struct VisualsConfig {
    pub dim: bool,
    pub dim_opacity: f32,
    pub highlight: HighlightSettings,
    /// `#rrggbb` already resolved (highlight colour or the accent).
    pub color: String,
    /// Corner radius of Krova's own shapes (the dim cut-out), at 100 % scale.
    pub corner_radius_px: u32,
}

impl VisualsConfig {
    /// Build from the settings; `focus` and `highlight` are the wave-3 blocks.
    pub fn from_settings(
        settings: &Settings,
        focus: &FocusSettings,
        highlight: &HighlightSettings,
    ) -> Self {
        let (accent, radius) = appearance(settings);
        let focus = focus.sanitized();
        let highlight = highlight.clone().sanitized();
        Self {
            dim: focus.enabled,
            dim_opacity: focus.opacity,
            color: highlight.color.clone().unwrap_or(accent),
            highlight,
            corner_radius_px: radius,
        }
    }

    /// Whether anything is drawn at all (if not, no thread needs to run).
    pub fn is_active(&self) -> bool {
        self.dim || self.highlight.mode != HighlightMode::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_are_clamped() {
        let f = FocusSettings {
            enabled: true,
            opacity: 2.0,
        }
        .sanitized();
        assert_eq!(f.opacity, MAX_DIM_OPACITY);
        let f = FocusSettings {
            enabled: true,
            opacity: f32::NAN,
        }
        .sanitized();
        assert_eq!(f.opacity, DEFAULT_DIM_OPACITY);
        assert_eq!(
            FocusSettings {
                enabled: false,
                opacity: 0.0
            }
            .sanitized()
            .opacity,
            MIN_DIM_OPACITY
        );
        let h = HighlightSettings {
            mode: HighlightMode::All,
            color: Some("ABCDEF".into()),
            width_px: 40,
        }
        .sanitized();
        assert_eq!(h.width_px, 6);
        assert_eq!(h.color.as_deref(), Some("#abcdef"));
        let h = HighlightSettings {
            color: Some("red".into()),
            width_px: 0,
            ..HighlightSettings::default()
        }
        .sanitized();
        assert_eq!((h.color, h.width_px), (None, 1));
    }

    #[test]
    fn defaults_are_off() {
        assert!(!FocusSettings::default().enabled);
        assert_eq!(FocusSettings::default().opacity, 0.45);
        assert_eq!(HighlightSettings::default().mode, HighlightMode::None);
        let json: HighlightSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(json, HighlightSettings::default());
        let mode: HighlightMode = serde_json::from_str("\"glow\"").unwrap();
        assert_eq!(mode, HighlightMode::Glow);
    }

    #[test]
    fn appearance_falls_back_without_the_block() {
        let s = Settings::default();
        assert_eq!(appearance(&s), (DEFAULT_ACCENT.to_owned(), 10));
        assert!(!share_safe(&s));
        assert_eq!(hex_rgb("#ff5b1f"), Some((0xff, 0x5b, 0x1f)));
        assert_eq!(hex_rgb("nope"), None);
    }

    #[test]
    fn the_spring_starts_at_zero_and_ends_exactly_on_target() {
        assert!(spring_progress(0).abs() < 1e-9);
        assert_eq!(spring_progress(SPRING_MS), 1.0);
        assert_eq!(spring_progress(10_000), 1.0);
        // Monotone enough and close to the target just before the end.
        assert!(spring_progress(SPRING_MS / 2) > 0.7);
        assert!((spring_progress(SPRING_MS - 1) - 1.0).abs() < 0.03);
        // Never overshoots by more than a few percent with this damping.
        assert!((0..SPRING_MS).all(|t| spring_progress(t) < 1.03));
        let r = spring_rect(
            PxRect::new(0, 0, 10, 10),
            PxRect::new(100, 50, 20, 30),
            SPRING_MS,
        );
        assert_eq!(r, PxRect::new(100, 50, 20, 30));
    }

    #[test]
    fn the_dim_hole_is_the_focused_part_of_the_monitor() {
        let m = PxRect::new(1920, 0, 1920, 1080);
        assert_eq!(dim_hole(m, None), None);
        assert_eq!(
            dim_hole(m, Some(PxRect::new(2000, 100, 400, 300))),
            Some(PxRect::new(80, 100, 400, 300))
        );
        // A window spanning two monitors is cut on both.
        assert_eq!(
            dim_hole(m, Some(PxRect::new(1800, 0, 400, 300))),
            Some(PxRect::new(0, 0, 280, 300))
        );
        assert_eq!(dim_hole(m, Some(PxRect::new(0, 0, 100, 100))), None);
    }

    #[test]
    fn backends_prefer_dwm_for_thin_borders_on_windows_11() {
        let h = |mode, width_px| HighlightSettings {
            mode,
            color: None,
            width_px,
        };
        use HighlightBackend::*;
        assert_eq!(highlight_backend(&h(HighlightMode::None, 2), 26100), Off);
        assert_eq!(
            highlight_backend(&h(HighlightMode::Active, 2), 26100),
            DwmActive
        );
        assert_eq!(
            highlight_backend(&h(HighlightMode::Active, 4), 26100),
            OverlayActive
        );
        assert_eq!(
            highlight_backend(&h(HighlightMode::Active, 1), 19045),
            OverlayActive
        );
        assert_eq!(highlight_backend(&h(HighlightMode::All, 1), 22000), DwmAll);
        assert_eq!(
            highlight_backend(&h(HighlightMode::All, 3), 22000),
            OverlayAll
        );
        assert_eq!(highlight_backend(&h(HighlightMode::Glow, 1), 26100), Glow);
    }

    #[test]
    fn geometry_helpers() {
        assert_eq!(
            outset(PxRect::new(10, 10, 100, 50), 3),
            PxRect::new(7, 7, 106, 56)
        );
        assert_eq!(scaled(2, 1.5), 3);
        assert_eq!(scaled(1, 0.0), 1);
        assert_eq!(window_corner_radius(22621, 1.25), 10);
        assert_eq!(window_corner_radius(19045, 2.0), 0);
        let m = PxRect::new(0, 0, 100, 100);
        assert!(is_fullscreen(PxRect::new(-8, -8, 116, 116), m));
        assert!(!is_fullscreen(PxRect::new(0, 0, 100, 90), m));
        assert_eq!(intersect(m, PxRect::new(100, 0, 5, 5)), None);
        assert_eq!(alpha_byte(0.45), 115);
    }

    #[test]
    fn glow_fades_from_the_edge() {
        assert!(glow_alpha(0.0, 12.0) > 100);
        assert!(glow_alpha(6.0, 12.0) < glow_alpha(2.0, 12.0));
        assert_eq!(glow_alpha(12.0, 12.0), 0);
        assert_eq!(glow_alpha(1.0, 0.0), 0);
        assert_eq!(rounded_rect_distance(5.0, 5.0, 10.0, 10.0, 2.0), 0.0);
        assert!((rounded_rect_distance(-3.0, 5.0, 10.0, 10.0, 2.0) - 3.0).abs() < 1e-9);
        // Outside a rounded corner the distance is measured to the arc.
        let d = rounded_rect_distance(-1.0, -1.0, 10.0, 10.0, 4.0);
        assert!((d - (50f64.sqrt() - 4.0)).abs() < 1e-9, "{d}");
    }

    #[test]
    fn visuals_config_resolves_the_colour() {
        let s = Settings::default();
        let mut h = HighlightSettings {
            mode: HighlightMode::Active,
            ..HighlightSettings::default()
        };
        let c = VisualsConfig::from_settings(&s, &FocusSettings::default(), &h);
        assert_eq!(c.color, DEFAULT_ACCENT);
        assert!(c.is_active() && !c.dim);
        h.color = Some("#112233".into());
        h.mode = HighlightMode::None;
        let c = VisualsConfig::from_settings(&s, &FocusSettings::default(), &h);
        assert_eq!(c.color, "#112233");
        assert!(!c.is_active());
    }
}
