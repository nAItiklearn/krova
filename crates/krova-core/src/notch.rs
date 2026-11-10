//! The notch: a small always-on-top pill at a screen edge that is the main way
//! to control Krova without opening the app.
//!
//! Everything here is pure and OS-independent: the settings, where the pill sits
//! on a monitor (and how it avoids the taskbar), which of the five positions a
//! drag snaps to, where the quick panel opens next to it, the spring that animates
//! every change, the pointer state machine (hover, click, drag) and the pixel
//! rasteriser that draws it. The Windows window lives in
//! `krova_platform::win_notch`; the app wiring in `src-tauri/src/notch.rs`.

use serde::{Deserialize, Deserializer, Serialize};

use crate::geometry::PxRect;

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

/// Where on its monitor the notch sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum NotchCorner {
    #[default]
    TopCenter,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl NotchCorner {
    pub const ALL: [NotchCorner; 5] = [
        NotchCorner::TopCenter,
        NotchCorner::TopLeft,
        NotchCorner::TopRight,
        NotchCorner::BottomLeft,
        NotchCorner::BottomRight,
    ];

    pub fn is_top(self) -> bool {
        matches!(self, Self::TopCenter | Self::TopLeft | Self::TopRight)
    }

    /// The serde name, e.g. `"top_center"`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TopCenter => "top_center",
            Self::TopLeft => "top_left",
            Self::TopRight => "top_right",
            Self::BottomLeft => "bottom_left",
            Self::BottomRight => "bottom_right",
        }
    }
}

/// Which monitor shows the notch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotchMonitor {
    #[default]
    Primary,
    /// The monitor under the cursor, re-evaluated when the foreground window or
    /// the displays change (never by polling).
    Cursor,
}

/// `Settings.notch`. Every field has a default, and an unknown or mistyped value
/// falls back to its default instead of making the whole config unreadable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct NotchSettings {
    pub enabled: bool,
    pub corner: NotchCorner,
    pub monitor: NotchMonitor,
    pub hide_in_fullscreen: bool,
}

impl Default for NotchSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            corner: NotchCorner::TopCenter,
            monitor: NotchMonitor::Primary,
            hide_in_fullscreen: true,
        }
    }
}

impl<'de> Deserialize<'de> for NotchSettings {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(d)?;
        Ok(Self::from_value(&value))
    }
}

impl NotchSettings {
    /// Lenient reading: each field is taken when valid, otherwise defaulted.
    pub fn from_value(value: &serde_json::Value) -> Self {
        let mut s = Self::default();
        let field = |name: &str| value.get(name).cloned();
        if let Some(b) = field("enabled").and_then(|v| v.as_bool()) {
            s.enabled = b;
        }
        if let Some(c) = field("corner").and_then(|v| serde_json::from_value(v).ok()) {
            s.corner = c;
        }
        if let Some(m) = field("monitor").and_then(|v| serde_json::from_value(v).ok()) {
            s.monitor = m;
        }
        if let Some(b) = field("hide_in_fullscreen").and_then(|v| v.as_bool()) {
            s.hide_in_fullscreen = b;
        }
        s
    }
}

// ---------------------------------------------------------------------------
// Geometry (all sizes in logical pixels unless the name says `_px`)
// ---------------------------------------------------------------------------

/// Collapsed pill, logical px.
pub const COLLAPSED: (f64, f64) = (46.0, 13.0);
/// Expanded (hovered) pill, logical px.
pub const EXPANDED: (f64, f64) = (220.0, 28.0);
/// Distance from the screen (or taskbar) edge, logical px.
pub const INSET: f64 = 4.0;
/// A press becomes a drag once the cursor moved this far, logical px.
pub const DRAG_THRESHOLD: f64 = 4.0;
/// Hover dwell before expanding.
pub const HOVER_MS: u32 = 150;
/// Gap between the notch and the panel it opens, logical px.
pub const PANEL_GAP: f64 = 6.0;

/// Scale a logical size to whole device pixels (at least 1).
pub fn to_px(logical: f64, scale: f64) -> i32 {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    ((logical * scale).round() as i32).max(1)
}

/// The notch's rect (physical px) at `corner` of a monitor.
///
/// The pill hugs the monitor edge, but never sits under a reserved area: where
/// the taskbar (or another app bar) reserves an edge, the work area's edge is
/// used instead, so a bottom notch sits just above a bottom taskbar and a top
/// notch just below a top one. Elsewhere the work area and the screen agree.
pub fn notch_rect(
    corner: NotchCorner,
    screen: PxRect,
    work: PxRect,
    w: i32,
    h: i32,
    inset: i32,
) -> PxRect {
    // The work area is only trusted when it lies inside the screen.
    let work = if work.w > 0
        && work.h > 0
        && work.x >= screen.x
        && work.y >= screen.y
        && work.right() <= screen.right()
        && work.bottom() <= screen.bottom()
    {
        work
    } else {
        screen
    };
    let left = work.x + inset;
    let right = work.right() - inset - w;
    let center = work.x + (work.w - w) / 2;
    let top = work.y + inset;
    let bottom = work.bottom() - inset - h;
    let (x, y) = match corner {
        NotchCorner::TopCenter => (center, top),
        NotchCorner::TopLeft => (left, top),
        NotchCorner::TopRight => (right, top),
        NotchCorner::BottomLeft => (left, bottom),
        NotchCorner::BottomRight => (right, bottom),
    };
    PxRect::new(x, y, w, h)
}

/// The position a drag released at `point` snaps to: the one whose collapsed
/// rect has its centre nearest to the point. Ties go to the earlier position in
/// [`NotchCorner::ALL`].
pub fn nearest_corner(
    screen: PxRect,
    work: PxRect,
    w: i32,
    h: i32,
    inset: i32,
    point: (i32, i32),
) -> NotchCorner {
    let dist = |c: NotchCorner| {
        let r = notch_rect(c, screen, work, w, h, inset);
        let dx = f64::from(r.x) + f64::from(r.w) / 2.0 - f64::from(point.0);
        let dy = f64::from(r.y) + f64::from(r.h) / 2.0 - f64::from(point.1);
        dx * dx + dy * dy
    };
    let mut best = NotchCorner::ALL[0];
    for c in NotchCorner::ALL {
        if dist(c) < dist(best) {
            best = c;
        }
    }
    best
}

/// Where a `pw` x `ph` panel opens for a notch at `notch` (physical px): below a
/// top notch, above a bottom one, `gap` away, aligned with the notch's side
/// (centred for `top_center`) and kept inside the work area.
pub fn panel_rect(
    notch: PxRect,
    corner: NotchCorner,
    work: PxRect,
    pw: i32,
    ph: i32,
    gap: i32,
) -> PxRect {
    let pw = pw.min(work.w).max(1);
    let ph = ph.min(work.h).max(1);
    let x = match corner {
        NotchCorner::TopCenter => notch.x + (notch.w - pw) / 2,
        NotchCorner::TopLeft | NotchCorner::BottomLeft => notch.x,
        NotchCorner::TopRight | NotchCorner::BottomRight => notch.right() - pw,
    };
    let y = if corner.is_top() {
        notch.bottom() + gap
    } else {
        notch.y - gap - ph
    };
    let x = x.clamp(work.x, work.right() - pw);
    let y = y.clamp(work.y, work.bottom() - ph);
    PxRect::new(x, y, pw, ph)
}

/// The notch's monitor: the primary one, or the one under the cursor; falls back
/// to the primary, then the first. `monitors` holds `(rect, primary)`.
pub fn pick_monitor(
    choice: NotchMonitor,
    monitors: &[(PxRect, bool)],
    cursor: Option<(i32, i32)>,
) -> Option<usize> {
    let primary = || {
        monitors
            .iter()
            .position(|(_, p)| *p)
            .or(if monitors.is_empty() { None } else { Some(0) })
    };
    match (choice, cursor) {
        (NotchMonitor::Cursor, Some((x, y))) => monitors
            .iter()
            .position(|(r, _)| r.contains(x, y))
            .or_else(primary),
        _ => primary(),
    }
}

/// Does a window rect cover the whole monitor (a fullscreen app)?
pub fn covers_monitor(window: PxRect, monitor: PxRect) -> bool {
    window.x <= monitor.x
        && window.y <= monitor.y
        && window.right() >= monitor.right()
        && window.bottom() >= monitor.bottom()
}

// ---------------------------------------------------------------------------
// Spring
// ---------------------------------------------------------------------------

/// Damping ratio of every notch motion (slightly under-damped, no visible bounce).
pub const DAMPING: f64 = 0.85;
/// Time to settle, seconds.
pub const SETTLE: f64 = 0.24;

/// A damped spring on one value. Retargeting keeps the velocity, so a new gesture
/// picks up the current motion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spring {
    pub value: f64,
    pub velocity: f64,
    pub target: f64,
}

impl Spring {
    pub fn at(value: f64) -> Self {
        Self {
            value,
            velocity: 0.0,
            target: value,
        }
    }

    /// Natural frequency for [`SETTLE`] at [`DAMPING`] (settles to about 2%).
    fn omega() -> f64 {
        4.0 / (DAMPING * SETTLE)
    }

    /// Advance by `dt` seconds (semi-implicit Euler in 1 ms sub-steps; stable).
    pub fn step(&mut self, dt: f64) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        let w = Self::omega();
        let mut left = dt.min(0.1);
        while left > 0.0 {
            let h = left.min(0.001);
            let accel = -2.0 * DAMPING * w * self.velocity - w * w * (self.value - self.target);
            self.velocity += accel * h;
            self.value += self.velocity * h;
            left -= h;
        }
        if self.at_rest() {
            self.value = self.target;
            self.velocity = 0.0;
        }
    }

    /// Within a quarter pixel and nearly still.
    pub fn at_rest(&self) -> bool {
        (self.value - self.target).abs() < 0.25 && self.velocity.abs() < 2.0
    }

    pub fn jump(&mut self, value: f64) {
        *self = Self::at(value);
    }
}

/// Four springs animating a rect (physical px).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RectSpring {
    pub x: Spring,
    pub y: Spring,
    pub w: Spring,
    pub h: Spring,
}

impl RectSpring {
    pub fn at(r: PxRect) -> Self {
        Self {
            x: Spring::at(f64::from(r.x)),
            y: Spring::at(f64::from(r.y)),
            w: Spring::at(f64::from(r.w)),
            h: Spring::at(f64::from(r.h)),
        }
    }

    pub fn set_target(&mut self, r: PxRect) {
        self.x.target = f64::from(r.x);
        self.y.target = f64::from(r.y);
        self.w.target = f64::from(r.w);
        self.h.target = f64::from(r.h);
    }

    pub fn jump(&mut self, r: PxRect) {
        *self = Self::at(r);
    }

    pub fn step(&mut self, dt: f64) {
        for s in [&mut self.x, &mut self.y, &mut self.w, &mut self.h] {
            s.step(dt);
        }
    }

    pub fn at_rest(&self) -> bool {
        self.x.at_rest() && self.y.at_rest() && self.w.at_rest() && self.h.at_rest()
    }

    /// The current rect, rounded to whole pixels (at least 1x1).
    pub fn rect(&self) -> PxRect {
        PxRect::new(
            self.x.value.round() as i32,
            self.y.value.round() as i32,
            (self.w.value.round() as i32).max(1),
            (self.h.value.round() as i32).max(1),
        )
    }

    pub fn target(&self) -> PxRect {
        PxRect::new(
            self.x.target.round() as i32,
            self.y.target.round() as i32,
            self.w.target.round() as i32,
            self.h.target.round() as i32,
        )
    }
}

// ---------------------------------------------------------------------------
// Pointer state machine
// ---------------------------------------------------------------------------

/// What the mouse did, in screen pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerInput {
    /// The cursor came over the notch.
    Enter,
    /// It rested there for [`HOVER_MS`].
    Dwell,
    /// It left the notch (ignored while a button is held).
    Leave,
    Down {
        x: i32,
        y: i32,
    },
    Move {
        x: i32,
        y: i32,
    },
    Up {
        x: i32,
        y: i32,
    },
}

/// What the window should do about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerOutput {
    Nothing,
    Expand,
    Collapse,
    /// A press and release without a drag.
    Click,
    /// The pill follows the cursor: its top-left is moved by `(dx, dy)` from
    /// where it was when the button went down.
    Drag {
        dx: i32,
        dy: i32,
    },
    /// The drag ended at `(x, y)`: snap to the nearest position.
    Drop {
        x: i32,
        y: i32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Away,
    /// Over the notch, waiting for the dwell.
    Over,
    Expanded,
    Pressed {
        x: i32,
        y: i32,
        expanded: bool,
    },
    Dragging {
        x: i32,
        y: i32,
    },
}

/// Hover, click and drag. `threshold` is [`DRAG_THRESHOLD`] in device pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pointer {
    pub phase: Phase,
    pub threshold: i32,
}

impl Pointer {
    pub fn new(threshold: i32) -> Self {
        Self {
            phase: Phase::Away,
            threshold: threshold.max(1),
        }
    }

    /// Should the pill be drawn at its expanded size?
    pub fn expanded(&self) -> bool {
        matches!(
            self.phase,
            Phase::Expanded | Phase::Pressed { expanded: true, .. }
        )
    }

    pub fn dragging(&self) -> bool {
        matches!(self.phase, Phase::Dragging { .. })
    }

    pub fn on(&mut self, input: PointerInput) -> PointerOutput {
        use PointerInput as I;
        use PointerOutput as O;
        match (self.phase, input) {
            (Phase::Away, I::Enter) => {
                self.phase = Phase::Over;
                O::Nothing
            }
            (Phase::Over, I::Dwell) => {
                self.phase = Phase::Expanded;
                O::Expand
            }
            (Phase::Over, I::Leave) => {
                self.phase = Phase::Away;
                O::Nothing
            }
            (Phase::Expanded, I::Leave) => {
                self.phase = Phase::Away;
                O::Collapse
            }
            (Phase::Away | Phase::Over | Phase::Expanded, I::Down { x, y }) => {
                let expanded = self.phase == Phase::Expanded;
                self.phase = Phase::Pressed { x, y, expanded };
                O::Nothing
            }
            (Phase::Pressed { x: px, y: py, .. }, I::Move { x, y }) => {
                let (dx, dy) = (x - px, y - py);
                if dx.abs() > self.threshold || dy.abs() > self.threshold {
                    self.phase = Phase::Dragging { x: px, y: py };
                    O::Drag { dx, dy }
                } else {
                    O::Nothing
                }
            }
            (Phase::Pressed { expanded, .. }, I::Up { .. }) => {
                self.phase = if expanded {
                    Phase::Expanded
                } else {
                    Phase::Over
                };
                O::Click
            }
            (Phase::Dragging { x: px, y: py }, I::Move { x, y }) => O::Drag {
                dx: x - px,
                dy: y - py,
            },
            (Phase::Dragging { .. }, I::Up { x, y }) => {
                // The pill is collapsed while it travels; it expands again on
                // the next hover.
                self.phase = Phase::Away;
                O::Drop { x, y }
            }
            // While a button is held, leaving the window does not end anything:
            // the window holds the mouse capture.
            (Phase::Pressed { .. } | Phase::Dragging { .. }, I::Leave) => O::Nothing,
            _ => O::Nothing,
        }
    }

    /// The capture was lost (another window took the mouse): forget the press.
    pub fn cancel(&mut self) -> PointerOutput {
        let was_expanded = self.expanded() || self.dragging();
        self.phase = Phase::Away;
        if was_expanded {
            PointerOutput::Collapse
        } else {
            PointerOutput::Nothing
        }
    }
}

// ---------------------------------------------------------------------------
// Rasteriser
// ---------------------------------------------------------------------------

/// The pill ink, `#141413`.
pub const INK: u32 = 0x14_14_13;
/// Text on the pill, `#f2f1ee`.
pub const TEXT: u32 = 0xf2_f1_ee;
/// Dot while Krova is paused, `#8a877f`.
pub const PAUSED_DOT: u32 = 0x8a_87_7f;
/// Default accent, `#ff5b1f`.
pub const DEFAULT_ACCENT: u32 = 0xff_5b_1f;
/// Default corner radius of the expanded pill, logical px.
pub const DEFAULT_RADIUS: f64 = 10.0;

/// `#rrggbb` to `0xrrggbb`.
pub fn parse_hex(s: &str) -> Option<u32> {
    let n = crate::config::normalize_hex_color(s)?;
    u32::from_str_radix(&n[1..], 16).ok()
}

/// Coverage (0..=1) of the pixel whose centre is `(px, py)` by a `w` x `h`
/// rounded rect at the origin with corner radius `r`, anti-aliased over one pixel.
pub fn rounded_rect_coverage(px: f64, py: f64, w: f64, h: f64, r: f64) -> f64 {
    let r = r.clamp(0.0, w.min(h) / 2.0);
    // Signed distance to the rounded rect (negative inside).
    let qx = (px - w / 2.0).abs() - (w / 2.0 - r);
    let qy = (py - h / 2.0).abs() - (h / 2.0 - r);
    let outside = qx.max(0.0).hypot(qy.max(0.0));
    let inside = qx.max(qy).min(0.0);
    let d = outside + inside - r;
    (0.5 - d).clamp(0.0, 1.0)
}

/// Coverage of a pixel centre by a disc.
pub fn disc_coverage(px: f64, py: f64, cx: f64, cy: f64, radius: f64) -> f64 {
    let d = (px - cx).hypot(py - cy) - radius;
    (0.5 - d).clamp(0.0, 1.0)
}

/// Everything needed to draw one frame of the notch.
#[derive(Debug, Clone, Copy)]
pub struct PillFrame<'a> {
    pub w: i32,
    pub h: i32,
    /// Corner radius, device px (clamped to half the height).
    pub radius: f64,
    /// `0xrrggbb`
    pub dot: u32,
    /// Dot centre and radius, device px.
    pub dot_at: (f64, f64, f64),
    /// Grayscale text coverage, `w * h` bytes, or `None`.
    pub text: Option<&'a [u8]>,
    /// 0..=1, how much of the text shows (it fades in as the pill grows).
    pub text_alpha: f64,
}

fn premultiply(rgb: u32, a: f64) -> u32 {
    let a = a.clamp(0.0, 1.0);
    let ch = |shift: u32| ((f64::from((rgb >> shift) & 0xff) * a).round() as u32).min(255);
    let alpha = (a * 255.0).round() as u32;
    (alpha << 24) | (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

fn mix(a: u32, b: u32, t: f64) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let ch = |shift: u32| {
        let x = f64::from((a >> shift) & 0xff);
        let y = f64::from((b >> shift) & 0xff);
        ((x + (y - x) * t).round() as u32).min(255)
    };
    (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

/// Rasterise a frame as premultiplied ARGB, top-down rows, `w * h` pixels. A
/// hairline lighter rim keeps the ink pill visible on dark wallpapers.
pub fn render_pill(f: &PillFrame<'_>) -> Vec<u32> {
    let (w, h) = (f.w.max(0), f.h.max(0));
    let len = usize::try_from(w).unwrap_or(0) * usize::try_from(h).unwrap_or(0);
    let mut out = vec![0u32; len];
    if len == 0 {
        return out;
    }
    let (fw, fh) = (f64::from(w), f64::from(h));
    let r = f.radius.min(fh / 2.0).max(0.0);
    let text = f.text.filter(|t| t.len() == len);
    let rim = mix(INK, 0xff_ff_ff, 0.14);
    for y in 0..h {
        for x in 0..w {
            let (px, py) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
            let cover = rounded_rect_coverage(px, py, fw, fh, r);
            if cover <= 0.0 {
                continue;
            }
            // Inner shape one pixel in: the ring between is the rim.
            let inner =
                rounded_rect_coverage(px - 1.0, py - 1.0, fw - 2.0, fh - 2.0, (r - 1.0).max(0.0));
            let mut rgb = mix(rim, INK, inner);
            let (cx, cy, cr) = f.dot_at;
            let dot = disc_coverage(px, py, cx, cy, cr);
            if dot > 0.0 {
                rgb = mix(rgb, f.dot, dot);
            }
            if let Some(t) = text {
                let i = usize::try_from(y * w + x).unwrap_or(0);
                let a = f64::from(t[i]) / 255.0 * f.text_alpha.clamp(0.0, 1.0);
                if a > 0.0 {
                    rgb = mix(rgb, TEXT, a);
                }
            }
            let i = usize::try_from(y * w + x).unwrap_or(0);
            out[i] = premultiply(rgb, cover);
        }
    }
    out
}

/// How far the pill is from collapsed (0) to expanded (1), by its width.
pub fn expansion(w: f64, collapsed_w: f64, expanded_w: f64) -> f64 {
    if expanded_w <= collapsed_w {
        return 0.0;
    }
    ((w - collapsed_w) / (expanded_w - collapsed_w)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: PxRect = PxRect {
        x: 0,
        y: 0,
        w: 1920,
        h: 1080,
    };
    const WORK: PxRect = PxRect {
        x: 0,
        y: 0,
        w: 1920,
        h: 1032,
    };

    #[test]
    fn settings_default_and_round_trip() {
        let s = NotchSettings::default();
        assert!(s.enabled && s.hide_in_fullscreen);
        assert_eq!(s.corner, NotchCorner::TopCenter);
        assert_eq!(s.monitor, NotchMonitor::Primary);
        let json = serde_json::to_value(s).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"enabled": true, "corner": "top_center", "monitor": "primary", "hide_in_fullscreen": true})
        );
        let back: NotchSettings = serde_json::from_value(json).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn bad_settings_fall_back_per_field() {
        let s: NotchSettings = serde_json::from_value(serde_json::json!({
            "enabled": false, "corner": "middle", "monitor": "cursor", "hide_in_fullscreen": 3
        }))
        .unwrap();
        assert!(!s.enabled);
        assert_eq!(s.corner, NotchCorner::TopCenter);
        assert_eq!(s.monitor, NotchMonitor::Cursor);
        assert!(s.hide_in_fullscreen);
        let s: NotchSettings = serde_json::from_str("42").unwrap();
        assert_eq!(s, NotchSettings::default());
        let s: NotchSettings = serde_json::from_str(r#"{"corner":"bottom_right"}"#).unwrap();
        assert_eq!(s.corner, NotchCorner::BottomRight);
        assert!(s.enabled);
    }

    #[test]
    fn old_and_broken_configs_still_load_with_a_notch() {
        let s: crate::Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(s.notch, NotchSettings::default());
        let s: crate::Settings = serde_json::from_str(r#"{"notch":"nonsense"}"#).unwrap();
        assert_eq!(s.notch, NotchSettings::default());
        let s: crate::Settings =
            serde_json::from_str(r#"{"notch":{"corner":"top_left","enabled":false}}"#).unwrap();
        assert_eq!(s.notch.corner, NotchCorner::TopLeft);
        assert!(!s.notch.enabled);
    }

    #[test]
    fn corners_have_their_serde_names() {
        for c in NotchCorner::ALL {
            assert_eq!(serde_json::to_value(c).unwrap(), c.as_str());
        }
    }

    #[test]
    fn positions_hug_the_edges_and_avoid_the_taskbar() {
        let r = |c| notch_rect(c, SCREEN, WORK, 46, 13, 4);
        assert_eq!(r(NotchCorner::TopCenter), PxRect::new(937, 4, 46, 13));
        assert_eq!(r(NotchCorner::TopLeft), PxRect::new(4, 4, 46, 13));
        assert_eq!(r(NotchCorner::TopRight), PxRect::new(1870, 4, 46, 13));
        // Just above the 48 px bottom taskbar.
        assert_eq!(r(NotchCorner::BottomLeft), PxRect::new(4, 1015, 46, 13));
        assert_eq!(r(NotchCorner::BottomRight), PxRect::new(1870, 1015, 46, 13));
    }

    #[test]
    fn a_top_taskbar_pushes_the_top_positions_down() {
        let work = PxRect::new(0, 40, 1920, 1040);
        let r = notch_rect(NotchCorner::TopCenter, SCREEN, work, 46, 13, 4);
        assert_eq!(r.y, 44);
        let r = notch_rect(NotchCorner::BottomRight, SCREEN, work, 46, 13, 4);
        assert_eq!(r.bottom(), 1076);
    }

    #[test]
    fn a_work_area_outside_the_screen_is_ignored() {
        let bad = PxRect::new(-50, 0, 10, 10);
        let r = notch_rect(NotchCorner::BottomLeft, SCREEN, bad, 46, 13, 4);
        assert_eq!(r, PxRect::new(4, 1063, 46, 13));
    }

    #[test]
    fn positions_work_on_a_second_monitor_with_negative_coordinates() {
        let screen = PxRect::new(-2560, -200, 2560, 1440);
        let r = notch_rect(NotchCorner::TopCenter, screen, screen, 92, 26, 8);
        assert_eq!(r, PxRect::new(-1326, -192, 92, 26));
    }

    #[test]
    fn drops_snap_to_the_nearest_position() {
        let n = |x, y| nearest_corner(SCREEN, WORK, 46, 13, 4, (x, y));
        assert_eq!(n(960, 300), NotchCorner::TopCenter);
        assert_eq!(n(100, 100), NotchCorner::TopLeft);
        assert_eq!(n(1800, 80), NotchCorner::TopRight);
        assert_eq!(n(30, 1000), NotchCorner::BottomLeft);
        assert_eq!(n(1900, 1079), NotchCorner::BottomRight);
        // Off-screen points still choose something sensible.
        assert_eq!(n(-500, 2000), NotchCorner::BottomLeft);
    }

    #[test]
    fn the_panel_opens_beside_the_notch_inside_the_work_area() {
        let top = notch_rect(NotchCorner::TopCenter, SCREEN, WORK, 220, 28, 4);
        let p = panel_rect(top, NotchCorner::TopCenter, WORK, 360, 420, 6);
        assert_eq!(p, PxRect::new(780, 38, 360, 420));

        let tr = notch_rect(NotchCorner::TopRight, SCREEN, WORK, 220, 28, 4);
        let p = panel_rect(tr, NotchCorner::TopRight, WORK, 360, 420, 6);
        assert_eq!(p.right(), tr.right());

        let bl = notch_rect(NotchCorner::BottomLeft, SCREEN, WORK, 220, 28, 4);
        let p = panel_rect(bl, NotchCorner::BottomLeft, WORK, 360, 420, 6);
        assert_eq!((p.x, p.bottom()), (bl.x, bl.y - 6));

        // Too big for the work area: clamped.
        let p = panel_rect(bl, NotchCorner::BottomLeft, WORK, 5000, 5000, 6);
        assert_eq!(p, WORK);
    }

    #[test]
    fn the_monitor_follows_the_choice_and_falls_back() {
        let a = (PxRect::new(0, 0, 100, 100), false);
        let b = (PxRect::new(100, 0, 100, 100), true);
        let mons = [a, b];
        assert_eq!(
            pick_monitor(NotchMonitor::Primary, &mons, Some((5, 5))),
            Some(1)
        );
        assert_eq!(
            pick_monitor(NotchMonitor::Cursor, &mons, Some((5, 5))),
            Some(0)
        );
        assert_eq!(
            pick_monitor(NotchMonitor::Cursor, &mons, Some((900, 5))),
            Some(1)
        );
        assert_eq!(pick_monitor(NotchMonitor::Cursor, &mons, None), Some(1));
        assert_eq!(pick_monitor(NotchMonitor::Primary, &[a], None), Some(0));
        assert_eq!(pick_monitor(NotchMonitor::Primary, &[], None), None);
    }

    #[test]
    fn fullscreen_is_a_window_covering_its_monitor() {
        assert!(covers_monitor(SCREEN, SCREEN));
        assert!(covers_monitor(PxRect::new(-8, -8, 1936, 1096), SCREEN));
        assert!(!covers_monitor(WORK, SCREEN));
    }

    #[test]
    fn the_spring_settles_in_about_a_quarter_second_without_overshooting_much() {
        let mut s = Spring::at(0.0);
        s.target = 100.0;
        let mut t = 0.0;
        let mut peak: f64 = 0.0;
        while !s.at_rest() && t < 2.0 {
            s.step(1.0 / 60.0);
            peak = peak.max(s.value);
            t += 1.0 / 60.0;
        }
        assert!(s.at_rest());
        assert_eq!(s.value, 100.0);
        assert!((0.15..0.45).contains(&t), "settled after {t}s");
        assert!(peak < 103.0, "overshoot to {peak}");
    }

    #[test]
    fn retargeting_keeps_the_velocity() {
        let mut s = Spring::at(0.0);
        s.target = 100.0;
        s.step(0.05);
        let v = s.velocity;
        assert!(v > 0.0);
        s.target = 0.0;
        assert_eq!(s.velocity, v);
        s.step(0.0);
        s.step(f64::NAN);
        assert_eq!(s.velocity, v);
    }

    #[test]
    fn the_rect_spring_reaches_its_target() {
        let mut r = RectSpring::at(PxRect::new(0, 0, 46, 13));
        r.set_target(PxRect::new(10, 20, 220, 28));
        assert!(!r.at_rest());
        for _ in 0..120 {
            r.step(1.0 / 60.0);
        }
        assert!(r.at_rest());
        assert_eq!(r.rect(), PxRect::new(10, 20, 220, 28));
        assert_eq!(r.target(), r.rect());
    }

    #[test]
    fn hover_needs_the_dwell_and_collapses_on_leave() {
        let mut p = Pointer::new(4);
        assert_eq!(p.on(PointerInput::Enter), PointerOutput::Nothing);
        assert!(!p.expanded());
        assert_eq!(p.on(PointerInput::Dwell), PointerOutput::Expand);
        assert!(p.expanded());
        assert_eq!(p.on(PointerInput::Leave), PointerOutput::Collapse);
        // A leave before the dwell never expands.
        p.on(PointerInput::Enter);
        assert_eq!(p.on(PointerInput::Leave), PointerOutput::Nothing);
        assert_eq!(p.on(PointerInput::Dwell), PointerOutput::Nothing);
    }

    #[test]
    fn a_short_press_is_a_click() {
        let mut p = Pointer::new(4);
        p.on(PointerInput::Enter);
        p.on(PointerInput::Dwell);
        p.on(PointerInput::Down { x: 10, y: 10 });
        assert!(p.expanded());
        assert_eq!(
            p.on(PointerInput::Move { x: 13, y: 7 }),
            PointerOutput::Nothing
        );
        assert_eq!(p.on(PointerInput::Up { x: 13, y: 7 }), PointerOutput::Click);
        assert!(p.expanded());
        // Collapsed and not yet dwelled is also a click.
        let mut p = Pointer::new(4);
        p.on(PointerInput::Enter);
        p.on(PointerInput::Down { x: 0, y: 0 });
        assert_eq!(p.on(PointerInput::Up { x: 0, y: 0 }), PointerOutput::Click);
        assert!(!p.expanded());
    }

    #[test]
    fn moving_past_the_threshold_drags_and_release_drops() {
        let mut p = Pointer::new(4);
        p.on(PointerInput::Enter);
        p.on(PointerInput::Down { x: 100, y: 100 });
        assert_eq!(
            p.on(PointerInput::Move { x: 105, y: 100 }),
            PointerOutput::Drag { dx: 5, dy: 0 }
        );
        assert!(p.dragging() && !p.expanded());
        assert_eq!(p.on(PointerInput::Leave), PointerOutput::Nothing);
        assert_eq!(
            p.on(PointerInput::Move { x: 300, y: 50 }),
            PointerOutput::Drag { dx: 200, dy: -50 }
        );
        assert_eq!(
            p.on(PointerInput::Up { x: 300, y: 50 }),
            PointerOutput::Drop { x: 300, y: 50 }
        );
        assert_eq!(p.phase, Phase::Away);
        // A cancelled drag collapses.
        p.on(PointerInput::Down { x: 0, y: 0 });
        p.on(PointerInput::Move { x: 50, y: 0 });
        assert_eq!(p.cancel(), PointerOutput::Collapse);
        assert_eq!(p.phase, Phase::Away);
    }

    #[test]
    fn coverage_is_full_inside_empty_outside_and_soft_at_the_edge() {
        let c = |x, y| rounded_rect_coverage(x, y, 46.0, 13.0, 6.5);
        assert_eq!(c(23.0, 6.5), 1.0);
        assert_eq!(c(-2.0, 6.5), 0.0);
        // The rounded corner cuts the bounding-box corner.
        assert_eq!(c(0.5, 0.5), 0.0);
        let edge = c(23.0, 0.5);
        assert!(edge > 0.0 && edge <= 1.0);
        assert_eq!(disc_coverage(5.0, 5.0, 5.0, 5.0, 2.0), 1.0);
        assert_eq!(disc_coverage(9.0, 5.0, 5.0, 5.0, 2.0), 0.0);
    }

    #[test]
    fn the_pill_is_ink_with_a_dot_and_transparent_corners() {
        let w = 46;
        let h = 13;
        let px = render_pill(&PillFrame {
            w,
            h,
            radius: 6.5,
            dot: DEFAULT_ACCENT,
            dot_at: (6.5, 6.5, 2.5),
            text: None,
            text_alpha: 0.0,
        });
        assert_eq!(px.len(), (w * h) as usize);
        let at = |x: i32, y: i32| px[(y * w + x) as usize];
        assert_eq!(at(0, 0), 0, "corner is transparent");
        assert_eq!(at(23, 6), 0xff_14_14_13, "middle is opaque ink");
        assert_eq!(at(6, 6), 0xff_ff_5b_1f, "dot centre is the accent");
        // Premultiplied: no channel exceeds alpha.
        for p in &px {
            let a = p >> 24;
            assert!((p >> 16 & 0xff) <= a && (p >> 8 & 0xff) <= a && (p & 0xff) <= a);
        }
    }

    #[test]
    fn text_coverage_is_blended_in_the_text_colour() {
        let (w, h) = (40, 20);
        let mut text = vec![0u8; (w * h) as usize];
        text[(10 * w + 20) as usize] = 255;
        let frame = PillFrame {
            w,
            h,
            radius: 10.0,
            dot: PAUSED_DOT,
            dot_at: (-10.0, -10.0, 0.0),
            text: Some(&text),
            text_alpha: 1.0,
        };
        let px = render_pill(&frame);
        assert_eq!(px[(10 * w + 20) as usize], 0xff_f2_f1_ee);
        let faded = render_pill(&PillFrame {
            text_alpha: 0.0,
            ..frame
        });
        assert_eq!(faded[(10 * w + 20) as usize], 0xff_14_14_13);
        // A mask of the wrong size is ignored rather than read out of bounds.
        let short = [255u8; 3];
        let px = render_pill(&PillFrame {
            text: Some(&short),
            ..frame
        });
        assert_eq!(px[(10 * w + 20) as usize], 0xff_14_14_13);
        assert!(render_pill(&PillFrame { w: 0, ..frame }).is_empty());
    }

    #[test]
    fn helpers() {
        assert_eq!(to_px(46.0, 1.5), 69);
        assert_eq!(to_px(13.0, f64::NAN), 13);
        assert_eq!(to_px(0.1, 1.0), 1);
        assert_eq!(parse_hex("#FF5B1F"), Some(DEFAULT_ACCENT));
        assert_eq!(parse_hex("nope"), None);
        assert_eq!(expansion(46.0, 46.0, 220.0), 0.0);
        assert_eq!(expansion(220.0, 46.0, 220.0), 1.0);
        assert_eq!(expansion(133.0, 46.0, 220.0), 0.5);
        assert_eq!(expansion(1.0, 5.0, 5.0), 0.0);
    }
}
