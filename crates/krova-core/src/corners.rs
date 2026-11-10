//! Hot corners: push the cursor into an outer screen corner and hold it there
//! for a moment to run an action.
//!
//! Pure logic only. The platform layer feeds cursor samples and timer ticks to a
//! [`CornerMachine`] and does what it answers ([`Output`]): arm one one-shot
//! timer, cancel it, or fire. Everything here is in physical (virtual-screen)
//! pixels; the distances of the rules are in logical pixels and are scaled by the
//! DPI of the monitor that owns the corner, so mixed-DPI setups feel the same on
//! every screen.
//!
//! Rules:
//! - Only *outer* corners count: a monitor corner whose outside neighbours are on
//!   no other monitor (the same "touching another screen is internal" rule as
//!   [`crate::edge`]). The cursor stops there, so it can be "thrown" into it.
//! - A corner is hit when the cursor is within [`HIT_LOGICAL_PX`] of it, armed
//!   after `dwell_ms` there without leaving and with no mouse button held, fires
//!   once, and re-arms only after the cursor went more than
//!   [`REARM_LOGICAL_PX`] away.
//! - After any fire no corner fires for [`COOLDOWN_MS`].

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::config::{normalize_hex_color, Settings};
use crate::geometry::PxRect;

/// How close (logical px, per axis) the cursor must be to the corner point.
pub const HIT_LOGICAL_PX: f64 = 3.0;
/// How far (logical px, per axis) the cursor must go before a fired corner re-arms.
pub const REARM_LOGICAL_PX: f64 = 24.0;
/// Quiet time after a fire, for every corner.
pub const COOLDOWN_MS: u64 = 600;
/// Shortest and longest dwell the settings accept.
pub const MIN_DWELL_MS: u32 = 150;
pub const MAX_DWELL_MS: u32 = 1000;
pub const DEFAULT_DWELL_MS: u32 = 300;
/// Accent used before (or without) `Settings.appearance.accent`.
pub const DEFAULT_ACCENT: &str = "#ff5b1f";
/// Side of the ripple feedback, in logical px.
pub const RIPPLE_LOGICAL_PX: f64 = 48.0;
/// Length of the ripple animation.
pub const RIPPLE_MS: u32 = 220;

// ---------------------------------------------------------------- settings

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Corner {
    pub const ALL: [Corner; 4] = [
        Corner::TopLeft,
        Corner::TopRight,
        Corner::BottomLeft,
        Corner::BottomRight,
    ];

    /// The serde name (`"top_left"`, ...), also used in the `hot-corner` event.
    pub fn as_str(self) -> &'static str {
        match self {
            Corner::TopLeft => "top_left",
            Corner::TopRight => "top_right",
            Corner::BottomLeft => "bottom_left",
            Corner::BottomRight => "bottom_right",
        }
    }

    fn is_left(self) -> bool {
        matches!(self, Corner::TopLeft | Corner::BottomLeft)
    }

    fn is_top(self) -> bool {
        matches!(self, Corner::TopLeft | Corner::TopRight)
    }
}

/// What a hot corner does. The names are the UI's (`HotCornerAction` in TS).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HotCornerAction {
    Switcher,
    CheatSheet,
    Focus,
    CalmDesk,
    Pause,
    ShowZones,
    Panel,
}

impl HotCornerAction {
    pub const ALL: [HotCornerAction; 7] = [
        HotCornerAction::Switcher,
        HotCornerAction::CheatSheet,
        HotCornerAction::Focus,
        HotCornerAction::CalmDesk,
        HotCornerAction::Pause,
        HotCornerAction::ShowZones,
        HotCornerAction::Panel,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            HotCornerAction::Switcher => "switcher",
            HotCornerAction::CheatSheet => "cheat_sheet",
            HotCornerAction::Focus => "focus",
            HotCornerAction::CalmDesk => "calm_desk",
            HotCornerAction::Pause => "pause",
            HotCornerAction::ShowZones => "show_zones",
            HotCornerAction::Panel => "panel",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.as_str() == s)
    }
}

/// `Settings.hot_corners`. Deserialising never fails: an unknown action is "no
/// action", a missing or mistyped field takes its default, so a hand-edited or
/// newer config cannot break loading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HotCornerSettings {
    pub top_left: Option<HotCornerAction>,
    pub top_right: Option<HotCornerAction>,
    pub bottom_left: Option<HotCornerAction>,
    pub bottom_right: Option<HotCornerAction>,
    /// How long the cursor must rest in the corner, `150..=1000` ms.
    pub dwell_ms: u32,
    /// Do nothing while a full-screen app (game, video, slides) is in front.
    pub ignore_fullscreen: bool,
}

impl Default for HotCornerSettings {
    fn default() -> Self {
        Self {
            top_left: None,
            top_right: Some(HotCornerAction::Switcher),
            bottom_left: None,
            bottom_right: Some(HotCornerAction::CalmDesk),
            dwell_ms: DEFAULT_DWELL_MS,
            ignore_fullscreen: true,
        }
    }
}

impl HotCornerSettings {
    pub fn action(&self, corner: Corner) -> Option<HotCornerAction> {
        match corner {
            Corner::TopLeft => self.top_left,
            Corner::TopRight => self.top_right,
            Corner::BottomLeft => self.bottom_left,
            Corner::BottomRight => self.bottom_right,
        }
    }

    /// Is any corner given an action?
    pub fn any(&self) -> bool {
        Corner::ALL.iter().any(|c| self.action(*c).is_some())
    }

    /// Clamp the dwell into range.
    pub fn sanitized(mut self) -> Self {
        self.dwell_ms = self.dwell_ms.clamp(MIN_DWELL_MS, MAX_DWELL_MS);
        self
    }

    fn from_value(v: &Value) -> Self {
        let d = Self::default();
        let action = |key: &str, default: Option<HotCornerAction>| match v.get(key) {
            None => default,
            Some(Value::String(s)) => HotCornerAction::parse(s),
            Some(_) => None,
        };
        let dwell = v
            .get("dwell_ms")
            .and_then(Value::as_f64)
            .filter(|n| n.is_finite())
            .map_or(d.dwell_ms, |n| {
                n.round()
                    .clamp(f64::from(MIN_DWELL_MS), f64::from(MAX_DWELL_MS)) as u32
            });
        Self {
            top_left: action("top_left", d.top_left),
            top_right: action("top_right", d.top_right),
            bottom_left: action("bottom_left", d.bottom_left),
            bottom_right: action("bottom_right", d.bottom_right),
            dwell_ms: dwell,
            ignore_fullscreen: v
                .get("ignore_fullscreen")
                .and_then(Value::as_bool)
                .unwrap_or(d.ignore_fullscreen),
        }
    }
}

impl<'de> Deserialize<'de> for HotCornerSettings {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v = Value::deserialize(deserializer)?;
        Ok(Self::from_value(&v))
    }
}

/// The accent colour as `(r, g, b)`: `Settings.appearance.accent` when that block
/// exists and holds a valid `#rrggbb`, otherwise [`DEFAULT_ACCENT`]. Read through
/// JSON so it works before and after the appearance settings land.
pub fn accent_rgb(settings: &Settings) -> (u8, u8, u8) {
    let value = serde_json::to_value(settings).unwrap_or(Value::Null);
    let hex = value
        .get("appearance")
        .and_then(|a| a.get("accent"))
        .and_then(Value::as_str)
        .and_then(normalize_hex_color)
        .unwrap_or_else(|| DEFAULT_ACCENT.to_owned());
    hex_rgb(&hex).unwrap_or((0xff, 0x5b, 0x1f))
}

fn hex_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let h = normalize_hex_color(hex)?;
    let n = u32::from_str_radix(&h[1..], 16).ok()?;
    Some(((n >> 16) as u8, (n >> 8) as u8, n as u8))
}

// ------------------------------------------------------------------ spots

/// One outer corner of one monitor that has an action.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CornerSpot {
    pub corner: Corner,
    pub action: HotCornerAction,
    /// Index of the monitor in the list given to [`outer_spots`].
    pub monitor: usize,
    /// Full rect of that monitor.
    pub monitor_rect: PxRect,
    /// The corner pixel itself (the last pixel inside the monitor).
    pub x: i32,
    pub y: i32,
    /// Device pixels per logical pixel of that monitor.
    pub scale: f64,
}

impl CornerSpot {
    fn px(&self, logical: f64) -> i32 {
        let s = if self.scale.is_finite() && self.scale > 0.0 {
            self.scale
        } else {
            1.0
        };
        (logical * s).round().max(1.0) as i32
    }

    /// Per-axis (Chebyshev) distance from the corner pixel.
    pub fn distance(&self, (x, y): (i32, i32)) -> i32 {
        (x.saturating_sub(self.x).saturating_abs()).max(y.saturating_sub(self.y).saturating_abs())
    }

    /// Is the cursor within the hit tolerance of this corner (and on its monitor)?
    pub fn hit(&self, pos: (i32, i32)) -> bool {
        self.monitor_rect.contains(pos.0, pos.1) && self.distance(pos) <= self.px(HIT_LOGICAL_PX)
    }

    /// Has the cursor gone far enough to re-arm this corner?
    pub fn left_far(&self, pos: (i32, i32)) -> bool {
        self.distance(pos) > self.px(REARM_LOGICAL_PX)
    }

    /// Half-size of the square around the corner where the platform should send
    /// samples: one pixel beyond the re-arm distance, so the first sample outside
    /// it is always far enough away to re-arm.
    pub fn wake_band(&self) -> i32 {
        self.px(REARM_LOGICAL_PX) + 1
    }

    /// Top-left of the `size`-pixel square ripple anchored in this corner.
    pub fn ripple_origin(&self, size: i32) -> (i32, i32) {
        let x = if self.corner.is_left() {
            self.x
        } else {
            self.x - size + 1
        };
        let y = if self.corner.is_top() {
            self.y
        } else {
            self.y - size + 1
        };
        (x, y)
    }

    /// Side of the ripple on this monitor, in device pixels.
    pub fn ripple_size(&self) -> i32 {
        self.px(RIPPLE_LOGICAL_PX)
    }
}

/// The corner pixel of `m` and the three pixels just outside it.
fn corner_geometry(m: PxRect, c: Corner) -> ((i32, i32), [(i32, i32); 3]) {
    let (x, ox) = if c.is_left() {
        (m.x, m.x - 1)
    } else {
        (m.right() - 1, m.right())
    };
    let (y, oy) = if c.is_top() {
        (m.y, m.y - 1)
    } else {
        (m.bottom() - 1, m.bottom())
    };
    ((x, y), [(ox, y), (x, oy), (ox, oy)])
}

/// Is corner `c` of monitor `index` an outer corner, i.e. none of the pixels just
/// outside it belong to another monitor?
pub fn is_outer(monitors: &[(PxRect, f64)], index: usize, c: Corner) -> bool {
    let Some(&(m, _)) = monitors.get(index) else {
        return false;
    };
    if m.w <= 0 || m.h <= 0 {
        return false;
    }
    let (_, outside) = corner_geometry(m, c);
    !monitors
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != index)
        .any(|(_, (o, _))| outside.iter().any(|&(x, y)| o.contains(x, y)))
}

/// Every outer corner of `monitors` (rect, scale) that `settings` gives an action.
pub fn outer_spots(monitors: &[(PxRect, f64)], settings: &HotCornerSettings) -> Vec<CornerSpot> {
    let mut spots = Vec::new();
    for (i, &(rect, scale)) in monitors.iter().enumerate() {
        for c in Corner::ALL {
            let Some(action) = settings.action(c) else {
                continue;
            };
            if !is_outer(monitors, i, c) {
                continue;
            }
            let ((x, y), _) = corner_geometry(rect, c);
            spots.push(CornerSpot {
                corner: c,
                action,
                monitor: i,
                monitor_rect: rect,
                x,
                y,
                scale,
            });
        }
    }
    spots
}

/// `pos` if it is on a monitor, otherwise the nearest point of the nearest
/// monitor (where the OS pins the cursor). Unchanged with no monitors.
pub fn clamp_to_monitors((x, y): (i32, i32), monitors: &[(PxRect, f64)]) -> (i32, i32) {
    let mut best: Option<((i32, i32), i64)> = None;
    for (m, _) in monitors {
        if m.w <= 0 || m.h <= 0 {
            continue;
        }
        if m.contains(x, y) {
            return (x, y);
        }
        let cx = x.clamp(m.x, m.right() - 1);
        let cy = y.clamp(m.y, m.bottom() - 1);
        let d = (i64::from(cx) - i64::from(x)).pow(2) + (i64::from(cy) - i64::from(y)).pow(2);
        if best.map_or(true, |(_, bd)| d < bd) {
            best = Some(((cx, cy), d));
        }
    }
    best.map_or((x, y), |(p, _)| p)
}

/// A window covering the whole monitor counts as full screen.
pub fn covers_monitor(window: PxRect, monitor: PxRect) -> bool {
    monitor.w > 0
        && monitor.h > 0
        && window.x <= monitor.x
        && window.y <= monitor.y
        && window.right() >= monitor.right()
        && window.bottom() >= monitor.bottom()
}

// ----------------------------------------------------------------- machine

/// What the platform must do after feeding the machine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Output {
    Nothing,
    /// (Re)arm the single one-shot timer to call [`CornerMachine::timer`] at
    /// this time (ms on the caller's clock). Replaces any pending timer.
    ArmTimer {
        at_ms: u64,
    },
    /// No timer is needed any more.
    CancelTimer,
    /// Run the corner's action now.
    Fire(CornerSpot),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    /// In spot `spot` since `since` (reset while a button is held).
    Dwelling {
        spot: usize,
        since: u64,
    },
    /// Fired spot `spot`; waits for the cursor to go far away.
    Fired {
        spot: usize,
    },
}

/// The hot-corner state machine. Feed it every cursor sample near a corner
/// ([`pointer`](Self::pointer)) and the timer it asked for
/// ([`timer`](Self::timer)); both return what to do.
#[derive(Debug, Clone)]
pub struct CornerMachine {
    spots: Vec<CornerSpot>,
    dwell_ms: u64,
    state: State,
    cooldown_until: u64,
}

impl CornerMachine {
    pub fn new(spots: Vec<CornerSpot>, dwell_ms: u32) -> Self {
        Self {
            spots,
            dwell_ms: u64::from(dwell_ms.clamp(MIN_DWELL_MS, MAX_DWELL_MS)),
            state: State::Idle,
            cooldown_until: 0,
        }
    }

    pub fn spots(&self) -> &[CornerSpot] {
        &self.spots
    }

    /// New monitors or settings. Forgets any dwell (the caller cancels its timer)
    /// but keeps the cooldown.
    pub fn reconfigure(&mut self, spots: Vec<CornerSpot>, dwell_ms: u32) {
        self.spots = spots;
        self.dwell_ms = u64::from(dwell_ms.clamp(MIN_DWELL_MS, MAX_DWELL_MS));
        self.state = State::Idle;
    }

    fn due(&self, since: u64) -> u64 {
        since.saturating_add(self.dwell_ms).max(self.cooldown_until)
    }

    fn enter(&mut self, pos: (i32, i32), now: u64) -> Output {
        match self.spots.iter().position(|s| s.hit(pos)) {
            Some(spot) => {
                self.state = State::Dwelling { spot, since: now };
                Output::ArmTimer {
                    at_ms: self.due(now),
                }
            }
            None => {
                self.state = State::Idle;
                Output::Nothing
            }
        }
    }

    /// A cursor sample. `buttons_down`: any mouse button is held.
    pub fn pointer(&mut self, pos: (i32, i32), buttons_down: bool, now: u64) -> Output {
        match self.state {
            State::Idle => self.enter(pos, now),
            State::Dwelling { spot, .. } => {
                if !self.spots[spot].hit(pos) {
                    match self.enter(pos, now) {
                        Output::Nothing => Output::CancelTimer,
                        other => other,
                    }
                } else if buttons_down {
                    // A held button restarts the dwell: never fire mid-drag.
                    self.state = State::Dwelling { spot, since: now };
                    Output::ArmTimer {
                        at_ms: self.due(now),
                    }
                } else {
                    Output::Nothing
                }
            }
            State::Fired { spot } => {
                if self.spots[spot].left_far(pos) {
                    self.enter(pos, now)
                } else {
                    Output::Nothing
                }
            }
        }
    }

    /// The timer asked for has expired; `pos` is where the cursor is now.
    pub fn timer(&mut self, pos: (i32, i32), buttons_down: bool, now: u64) -> Output {
        let State::Dwelling { spot, since } = self.state else {
            return Output::Nothing;
        };
        let s = self.spots[spot];
        if !s.hit(pos) {
            return match self.enter(pos, now) {
                Output::Nothing => Output::CancelTimer,
                other => other,
            };
        }
        if buttons_down {
            self.state = State::Dwelling { spot, since: now };
            return Output::ArmTimer {
                at_ms: self.due(now),
            };
        }
        let due = self.due(since);
        if now < due {
            return Output::ArmTimer { at_ms: due };
        }
        self.state = State::Fired { spot };
        self.cooldown_until = now.saturating_add(COOLDOWN_MS);
        Output::Fire(s)
    }
}

// ------------------------------------------------------------------ ripple

/// One frame of the ripple: how far it has grown (0..~1.05) and how opaque it
/// is (0..=1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RippleFrame {
    pub radius: f64,
    pub alpha: f64,
}

/// Under-damped spring from 0 to 1 (damping ratio 0.85), settled by `RIPPLE_MS`.
fn spring(t: f64) -> f64 {
    const ZETA: f64 = 0.85;
    // Natural frequency chosen so the response is within ~1% at t = 1.
    const OMEGA: f64 = 6.5;
    if t >= 1.0 {
        return 1.0;
    }
    if t <= 0.0 {
        return 0.0;
    }
    let wd = OMEGA * (1.0 - ZETA * ZETA).sqrt();
    let decay = (-ZETA * OMEGA * t).exp();
    1.0 - decay * ((wd * t).cos() + ZETA * OMEGA / wd * (wd * t).sin())
}

/// The ripple `t_ms` after the corner fired; `None` once it is over.
pub fn ripple_frame(t_ms: u32) -> Option<RippleFrame> {
    if t_ms >= RIPPLE_MS {
        return None;
    }
    let t = f64::from(t_ms) / f64::from(RIPPLE_MS);
    let radius = spring(t);
    // Quick fade in, then out along the spring's second half.
    let alpha = (t / 0.15).min(1.0) * (1.0 - spring(((t - 0.35) / 0.65).max(0.0))).clamp(0.0, 1.0);
    Some(RippleFrame { radius, alpha })
}

/// Paint the ripple into `pixels` (`size * size` premultiplied ARGB, top-down
/// rows): a quarter disc centred on the corner pixel, anti-aliased, `frame.radius`
/// of the square's side, in `rgb` at `frame.alpha` (0.9 at most, so it stays a
/// hint rather than a block of colour).
pub fn paint_ripple(
    pixels: &mut [u32],
    size: i32,
    corner: Corner,
    rgb: (u8, u8, u8),
    frame: RippleFrame,
) {
    let Ok(n) = usize::try_from(size) else {
        return;
    };
    if n == 0 || pixels.len() != n * n {
        return;
    }
    let radius = frame.radius.max(0.0) * f64::from(size);
    let alpha = frame.alpha.clamp(0.0, 1.0) * 0.9;
    let last = f64::from(size - 1);
    for row in 0..n {
        for col in 0..n {
            // Distance of the pixel centre from the corner pixel's centre.
            let (c, r) = (col as f64, row as f64);
            let dx = if corner.is_left() { c } else { last - c };
            let dy = if corner.is_top() { r } else { last - r };
            let d = (dx * dx + dy * dy).sqrt();
            let coverage = (radius - d + 0.5).clamp(0.0, 1.0);
            // Softer towards the rim, like light spreading from the corner.
            let falloff = if radius > 0.0 {
                1.0 - 0.45 * (d / radius).min(1.0)
            } else {
                0.0
            };
            let a = alpha * coverage * falloff;
            let a8 = (a * 255.0).round() as u32;
            let pm = |v: u8| ((f64::from(v) * a).round() as u32).min(a8);
            pixels[row * n + col] = (a8 << 24) | (pm(rgb.0) << 16) | (pm(rgb.1) << 8) | pm(rgb.2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const M: PxRect = PxRect::new(0, 0, 1920, 1080);

    fn all(action: HotCornerAction) -> HotCornerSettings {
        HotCornerSettings {
            top_left: Some(action),
            top_right: Some(action),
            bottom_left: Some(action),
            bottom_right: Some(action),
            ..HotCornerSettings::default()
        }
    }

    fn corners_of(spots: &[CornerSpot], monitor: usize) -> Vec<Corner> {
        spots
            .iter()
            .filter(|s| s.monitor == monitor)
            .map(|s| s.corner)
            .collect()
    }

    fn machine(monitors: &[(PxRect, f64)]) -> CornerMachine {
        CornerMachine::new(outer_spots(monitors, &all(HotCornerAction::Switcher)), 300)
    }

    // ---- settings

    #[test]
    fn defaults_match_the_ui() {
        let d = HotCornerSettings::default();
        assert_eq!(d.top_left, None);
        assert_eq!(d.top_right, Some(HotCornerAction::Switcher));
        assert_eq!(d.bottom_left, None);
        assert_eq!(d.bottom_right, Some(HotCornerAction::CalmDesk));
        assert_eq!(d.dwell_ms, 300);
        assert!(d.ignore_fullscreen);
        let json = serde_json::to_value(&d).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "top_left": null, "top_right": "switcher", "bottom_left": null,
                "bottom_right": "calm_desk", "dwell_ms": 300, "ignore_fullscreen": true
            })
        );
    }

    #[test]
    fn every_action_round_trips_by_name() {
        for a in HotCornerAction::ALL {
            let json = serde_json::to_string(&a).unwrap();
            assert_eq!(json, format!("\"{}\"", a.as_str()));
            assert_eq!(HotCornerAction::parse(a.as_str()), Some(a));
            assert_eq!(serde_json::from_str::<HotCornerAction>(&json).unwrap(), a);
        }
        for c in Corner::ALL {
            assert_eq!(
                serde_json::to_string(&c).unwrap(),
                format!("\"{}\"", c.as_str())
            );
        }
    }

    #[test]
    fn bad_values_never_fail_loading() {
        let s: HotCornerSettings = serde_json::from_str(
            r#"{"top_left":"warp_drive","top_right":7,"bottom_left":"panel","dwell_ms":-4,"ignore_fullscreen":"yes"}"#,
        )
        .unwrap();
        assert_eq!(s.top_left, None);
        assert_eq!(s.top_right, None);
        assert_eq!(s.bottom_left, Some(HotCornerAction::Panel));
        // Missing keeps the default.
        assert_eq!(s.bottom_right, Some(HotCornerAction::CalmDesk));
        assert_eq!(s.dwell_ms, MIN_DWELL_MS);
        assert!(s.ignore_fullscreen);

        let s: HotCornerSettings = serde_json::from_str(r#"{"dwell_ms": 99999}"#).unwrap();
        assert_eq!(s.dwell_ms, MAX_DWELL_MS);
        let s: HotCornerSettings = serde_json::from_str("null").unwrap();
        assert_eq!(s, HotCornerSettings::default());
        let s: HotCornerSettings = serde_json::from_str(r#"{"top_right": null}"#).unwrap();
        assert_eq!(s.top_right, None);
    }

    #[test]
    fn sanitizing_clamps_the_dwell() {
        let mut s = HotCornerSettings {
            dwell_ms: 5,
            ..Default::default()
        };
        assert_eq!(s.clone().sanitized().dwell_ms, 150);
        s.dwell_ms = 5000;
        assert_eq!(s.clone().sanitized().dwell_ms, 1000);
        s.dwell_ms = 420;
        assert_eq!(s.sanitized().dwell_ms, 420);
    }

    #[test]
    fn settings_carry_hot_corners_and_old_configs_load() {
        let old: Settings = serde_json::from_str(r#"{"enabled": true}"#).unwrap();
        assert_eq!(old.hot_corners, HotCornerSettings::default());
        let mut s = Settings::default();
        s.hot_corners.dwell_ms = 9;
        assert_eq!(s.sanitized().hot_corners.dwell_ms, MIN_DWELL_MS);
        let json = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(json["hot_corners"]["top_right"], "switcher");
    }

    #[test]
    fn the_accent_falls_back_to_krova_orange() {
        assert_eq!(accent_rgb(&Settings::default()), (0xff, 0x5b, 0x1f));
        assert_eq!(hex_rgb("#3B82F6"), Some((0x3b, 0x82, 0xf6)));
        assert_eq!(hex_rgb("nope"), None);
    }

    // ---- outer corners

    #[test]
    fn a_single_monitor_has_four_outer_corners() {
        let spots = outer_spots(&[(M, 1.0)], &all(HotCornerAction::Focus));
        assert_eq!(spots.len(), 4);
        let at = |c| spots.iter().find(|s| s.corner == c).map(|s| (s.x, s.y));
        assert_eq!(at(Corner::TopLeft), Some((0, 0)));
        assert_eq!(at(Corner::TopRight), Some((1919, 0)));
        assert_eq!(at(Corner::BottomLeft), Some((0, 1079)));
        assert_eq!(at(Corner::BottomRight), Some((1919, 1079)));
    }

    #[test]
    fn corners_without_an_action_are_skipped() {
        let spots = outer_spots(&[(M, 1.0)], &HotCornerSettings::default());
        assert_eq!(
            corners_of(&spots, 0),
            [Corner::TopRight, Corner::BottomRight]
        );
        let none = HotCornerSettings {
            top_right: None,
            bottom_right: None,
            ..Default::default()
        };
        assert!(!none.any());
        assert!(outer_spots(&[(M, 1.0)], &none).is_empty());
    }

    #[test]
    fn side_by_side_monitors_lose_the_shared_corners() {
        let monitors = [(M, 1.0), (PxRect::new(1920, 0, 1920, 1080), 1.0)];
        let spots = outer_spots(&monitors, &all(HotCornerAction::Focus));
        assert_eq!(corners_of(&spots, 0), [Corner::TopLeft, Corner::BottomLeft]);
        assert_eq!(
            corners_of(&spots, 1),
            [Corner::TopRight, Corner::BottomRight]
        );
    }

    #[test]
    fn stacked_monitors_lose_the_shared_corners() {
        let monitors = [(M, 1.0), (PxRect::new(0, -1080, 1920, 1080), 1.0)];
        let spots = outer_spots(&monitors, &all(HotCornerAction::Focus));
        assert_eq!(
            corners_of(&spots, 0),
            [Corner::BottomLeft, Corner::BottomRight]
        );
        assert_eq!(corners_of(&spots, 1), [Corner::TopLeft, Corner::TopRight]);
    }

    #[test]
    fn l_shaped_arrangements_keep_the_corners_the_cursor_stops_at() {
        // A 1440p screen with a 1080p screen to its right, bottom-aligned, and a
        // third one below the first: an L.
        let a = PxRect::new(0, 0, 2560, 1440);
        let b = PxRect::new(2560, 360, 1920, 1080);
        let c = PxRect::new(0, 1440, 2560, 1440);
        let monitors = [(a, 1.0), (b, 1.0), (c, 1.0)];
        let spots = outer_spots(&monitors, &all(HotCornerAction::Focus));
        // a: top-right stays (b starts 360 px lower); bottom corners touch c, and
        // bottom-right also touches b.
        assert_eq!(corners_of(&spots, 0), [Corner::TopLeft, Corner::TopRight]);
        // b: its left corners touch a; the right ones are outer.
        assert_eq!(
            corners_of(&spots, 1),
            [Corner::TopRight, Corner::BottomRight]
        );
        // c: top corners touch a; bottom ones are outer. Its top-right corner pixel's
        // diagonal neighbour (2560, 1439) is on b, so it is internal too.
        assert_eq!(
            corners_of(&spots, 2),
            [Corner::BottomLeft, Corner::BottomRight]
        );
    }

    #[test]
    fn a_diagonal_touch_is_enough_to_make_a_corner_internal() {
        // b only touches a at a's bottom-right pixel, diagonally.
        let b = PxRect::new(1920, 1080, 1920, 1080);
        assert!(!is_outer(&[(M, 1.0), (b, 1.0)], 0, Corner::BottomRight));
        assert!(!is_outer(&[(M, 1.0), (b, 1.0)], 1, Corner::TopLeft));
        assert!(is_outer(&[(M, 1.0), (b, 1.0)], 0, Corner::TopRight));
        assert!(!is_outer(&[(M, 1.0)], 3, Corner::TopRight));
        assert!(!is_outer(
            &[(PxRect::new(0, 0, 0, 0), 1.0)],
            0,
            Corner::TopLeft
        ));
    }

    // ---- hit test and mixed DPI

    #[test]
    fn the_hit_zone_is_three_logical_pixels() {
        let spots = outer_spots(&[(M, 1.0)], &all(HotCornerAction::Focus));
        let tr = spots.iter().find(|s| s.corner == Corner::TopRight).unwrap();
        assert!(tr.hit((1919, 0)));
        assert!(tr.hit((1916, 3)));
        assert!(!tr.hit((1915, 0)));
        assert!(!tr.hit((1919, 4)));
        // Off the monitor never counts.
        assert!(!tr.hit((1920, 0)));
    }

    #[test]
    fn distances_scale_with_each_monitors_dpi() {
        // A 4K screen at 200% next to a 1080p screen at 100%.
        let hi = PxRect::new(0, 0, 3840, 2160);
        let lo = PxRect::new(3840, 0, 1920, 1080);
        let spots = outer_spots(&[(hi, 2.0), (lo, 1.0)], &all(HotCornerAction::Focus));
        let hi_tl = spots
            .iter()
            .find(|s| s.monitor == 0 && s.corner == Corner::TopLeft)
            .unwrap();
        let lo_tr = spots
            .iter()
            .find(|s| s.monitor == 1 && s.corner == Corner::TopRight)
            .unwrap();
        assert!(hi_tl.hit((6, 6)) && !hi_tl.hit((7, 0)));
        assert!(lo_tr.hit((5756, 3)) && !lo_tr.hit((5755, 0)));
        assert!(!hi_tl.left_far((48, 0)) && hi_tl.left_far((49, 0)));
        assert!(!lo_tr.left_far((5759 - 24, 0)) && lo_tr.left_far((5759 - 25, 0)));
        assert_eq!(hi_tl.wake_band(), 49);
        assert_eq!(lo_tr.wake_band(), 25);
        // hi's bottom-right is outer: lo is only 1080 px tall.
        assert!(spots
            .iter()
            .any(|s| s.monitor == 0 && s.corner == Corner::BottomRight));
        assert!(!spots
            .iter()
            .any(|s| s.monitor == 0 && s.corner == Corner::TopRight));
        // A broken scale behaves like 100%.
        let mut odd = *lo_tr;
        odd.scale = f64::NAN;
        assert!(odd.hit((5756, 3)));
    }

    #[test]
    fn fractional_scales_round() {
        let spots = outer_spots(&[(M, 1.5)], &all(HotCornerAction::Focus));
        let tl = spots[0];
        assert!(tl.hit((4, 5)) && !tl.hit((6, 0)));
        assert_eq!(tl.wake_band(), 37);
        assert_eq!(tl.ripple_size(), 72);
    }

    // ---- state machine

    #[test]
    fn it_fires_once_after_the_dwell() {
        let mut m = machine(&[(M, 1.0)]);
        assert_eq!(m.pointer((900, 500), false, 0), Output::Nothing);
        assert_eq!(
            m.pointer((1919, 0), false, 1000),
            Output::ArmTimer { at_ms: 1300 }
        );
        assert_eq!(m.pointer((1918, 1), false, 1100), Output::Nothing);
        // Early tick (timer slack): re-arm for the rest.
        assert_eq!(
            m.timer((1918, 1), false, 1290),
            Output::ArmTimer { at_ms: 1300 }
        );
        let Output::Fire(s) = m.timer((1918, 1), false, 1300) else {
            panic!("should fire");
        };
        assert_eq!(
            (s.corner, s.action, s.monitor),
            (Corner::TopRight, HotCornerAction::Switcher, 0)
        );
        // Staying, or a stale timer, does nothing more.
        assert_eq!(m.pointer((1919, 0), false, 5000), Output::Nothing);
        assert_eq!(m.timer((1919, 0), false, 5000), Output::Nothing);
    }

    #[test]
    fn leaving_before_the_dwell_cancels() {
        let mut m = machine(&[(M, 1.0)]);
        m.pointer((0, 0), false, 0);
        assert_eq!(m.pointer((10, 10), false, 100), Output::CancelTimer);
        // A late timer finds nothing to do.
        assert_eq!(m.timer((10, 10), false, 300), Output::Nothing);
        // Coming back starts a fresh dwell.
        assert_eq!(
            m.pointer((1, 1), false, 400),
            Output::ArmTimer { at_ms: 700 }
        );
    }

    #[test]
    fn a_timer_finding_the_cursor_gone_cancels() {
        let mut m = machine(&[(M, 1.0)]);
        m.pointer((0, 0), false, 0);
        assert_eq!(m.timer((500, 500), false, 300), Output::CancelTimer);
        assert_eq!(m.timer((500, 500), false, 600), Output::Nothing);
    }

    #[test]
    fn it_rearms_only_after_going_far() {
        let mut m = machine(&[(M, 1.0)]);
        m.pointer((0, 0), false, 0);
        assert!(matches!(m.timer((0, 0), false, 300), Output::Fire(_)));
        // Wobbling out of the hit zone but within 24 px: still spent.
        assert_eq!(m.pointer((20, 20), false, 2000), Output::Nothing);
        assert_eq!(m.pointer((0, 0), false, 2100), Output::Nothing);
        assert_eq!(m.pointer((24, 0), false, 2200), Output::Nothing);
        // 25 px away re-arms; coming back dwells again.
        assert_eq!(m.pointer((25, 0), false, 2300), Output::Nothing);
        assert_eq!(
            m.pointer((0, 0), false, 2400),
            Output::ArmTimer { at_ms: 2700 }
        );
        assert!(matches!(m.timer((0, 0), false, 2700), Output::Fire(_)));
    }

    #[test]
    fn a_held_button_never_fires() {
        let mut m = machine(&[(M, 1.0)]);
        // Dragging a window into the corner.
        assert_eq!(m.pointer((0, 0), true, 0), Output::ArmTimer { at_ms: 300 });
        assert_eq!(m.timer((0, 0), true, 300), Output::ArmTimer { at_ms: 600 });
        assert_eq!(
            m.pointer((1, 0), true, 350),
            Output::ArmTimer { at_ms: 650 }
        );
        // Released at 400 (no sample): the full dwell counts from the last held
        // sample or tick.
        assert_eq!(m.timer((1, 0), false, 650), Output::Fire(m.spots()[0]));
    }

    #[test]
    fn a_button_pressed_mid_dwell_restarts_it() {
        let mut m = machine(&[(M, 1.0)]);
        m.pointer((0, 0), false, 0);
        assert_eq!(m.timer((0, 0), true, 300), Output::ArmTimer { at_ms: 600 });
        assert!(matches!(m.timer((0, 0), false, 600), Output::Fire(_)));
    }

    #[test]
    fn the_cooldown_covers_every_corner() {
        let monitors = [(M, 1.0)];
        let mut m = machine(&monitors);
        m.pointer((0, 0), false, 0);
        assert!(matches!(m.timer((0, 0), false, 300), Output::Fire(_)));
        // Straight to the opposite corner: its dwell would end at 650, but the
        // cooldown runs until 900.
        assert_eq!(
            m.pointer((1919, 1079), false, 350),
            Output::ArmTimer { at_ms: 900 }
        );
        assert_eq!(
            m.timer((1919, 1079), false, 650),
            Output::ArmTimer { at_ms: 900 }
        );
        let Output::Fire(s) = m.timer((1919, 1079), false, 900) else {
            panic!("fires after the cooldown");
        };
        assert_eq!(s.corner, Corner::BottomRight);
    }

    #[test]
    fn moving_from_one_corner_to_another_switches_the_dwell() {
        // Two tiny monitors would be silly; use a narrow one where two corners
        // are close: 4 px wide.
        let narrow = PxRect::new(0, 0, 4, 1080);
        let mut m = CornerMachine::new(
            outer_spots(&[(narrow, 1.0)], &all(HotCornerAction::Pause)),
            300,
        );
        // (0, 0) hits top-left first.
        assert_eq!(m.pointer((0, 0), false, 0), Output::ArmTimer { at_ms: 300 });
        // Down to the bottom-left: a new dwell, not a cancel.
        assert_eq!(
            m.pointer((0, 1079), false, 100),
            Output::ArmTimer { at_ms: 400 }
        );
        let Output::Fire(s) = m.timer((0, 1079), false, 400) else {
            panic!();
        };
        assert_eq!(s.corner, Corner::BottomLeft);
    }

    #[test]
    fn internal_corners_never_fire() {
        let monitors = [(M, 1.0), (PxRect::new(1920, 0, 1920, 1080), 1.0)];
        let mut m = machine(&monitors);
        // The shared top corners.
        assert_eq!(m.pointer((1919, 0), false, 0), Output::Nothing);
        assert_eq!(m.pointer((1920, 0), false, 10), Output::Nothing);
        assert_eq!(m.timer((1920, 0), false, 400), Output::Nothing);
        // The far one works.
        assert_eq!(
            m.pointer((3839, 0), false, 20),
            Output::ArmTimer { at_ms: 320 }
        );
        let Output::Fire(s) = m.timer((3839, 0), false, 320) else {
            panic!();
        };
        assert_eq!((s.monitor, s.corner), (1, Corner::TopRight));
    }

    #[test]
    fn reconfiguring_forgets_the_dwell_but_keeps_the_cooldown() {
        let mut m = machine(&[(M, 1.0)]);
        m.pointer((0, 0), false, 0);
        assert!(matches!(m.timer((0, 0), false, 300), Output::Fire(_)));
        m.reconfigure(outer_spots(&[(M, 1.0)], &all(HotCornerAction::Panel)), 150);
        // Fired state is gone, so the same corner dwells again, after the cooldown.
        assert_eq!(
            m.pointer((0, 0), false, 400),
            Output::ArmTimer { at_ms: 900 }
        );
        m.reconfigure(Vec::new(), 150);
        assert_eq!(m.timer((0, 0), false, 900), Output::Nothing);
        assert_eq!(m.pointer((0, 0), false, 1000), Output::Nothing);
    }

    #[test]
    fn the_dwell_is_clamped() {
        let m = CornerMachine::new(Vec::new(), 10);
        assert_eq!(m.dwell_ms, 150);
        let m = CornerMachine::new(Vec::new(), 60_000);
        assert_eq!(m.dwell_ms, 1000);
    }

    // ---- full screen and ripple

    #[test]
    fn points_past_the_desktop_are_pinned_to_the_nearest_monitor() {
        let monitors = [(M, 1.0), (PxRect::new(1920, 0, 1920, 1080), 2.0)];
        assert_eq!(clamp_to_monitors((-5, -3), &monitors), (0, 0));
        assert_eq!(clamp_to_monitors((3845, 1090), &monitors), (3839, 1079));
        assert_eq!(clamp_to_monitors((1919, -9), &monitors), (1919, 0));
        assert_eq!(clamp_to_monitors((700, 300), &monitors), (700, 300));
        assert_eq!(clamp_to_monitors((7, 7), &[]), (7, 7));
    }

    #[test]
    fn full_screen_means_covering_the_whole_monitor() {
        assert!(covers_monitor(M, M));
        assert!(covers_monitor(PxRect::new(-8, -8, 1936, 1096), M));
        assert!(!covers_monitor(PxRect::new(0, 0, 1920, 1040), M));
        assert!(!covers_monitor(M, PxRect::new(0, 0, 0, 0)));
    }

    #[test]
    fn the_ripple_grows_fades_and_ends() {
        let first = ripple_frame(0).unwrap();
        assert_eq!(first.alpha, 0.0);
        assert_eq!(first.radius, 0.0);
        let mid = ripple_frame(60).unwrap();
        assert!(mid.alpha > 0.9 && mid.radius > 0.4);
        let late = ripple_frame(215).unwrap();
        assert!(late.alpha < 0.1, "{late:?}");
        assert!(late.radius > 0.95);
        assert_eq!(ripple_frame(RIPPLE_MS), None);
        for t in 0..RIPPLE_MS {
            let f = ripple_frame(t).unwrap();
            assert!((0.0..=1.0).contains(&f.alpha));
            assert!((0.0..1.1).contains(&f.radius), "t={t} {f:?}");
        }
    }

    #[test]
    fn the_ripple_is_a_quarter_disc_in_the_right_corner() {
        let size = 48;
        let mut px = vec![0u32; 48 * 48];
        let frame = RippleFrame {
            radius: 1.0,
            alpha: 1.0,
        };
        paint_ripple(&mut px, size, Corner::BottomRight, (255, 91, 31), frame);
        let at = |x: usize, y: usize| px[y * 48 + x];
        // Opaque-ish at the corner, empty at the opposite corner.
        assert!(at(47, 47) >> 24 > 200);
        assert_eq!(at(0, 0), 0);
        // Premultiplied: no channel above alpha.
        for p in &px {
            let a = p >> 24;
            assert!((p >> 16 & 0xff) <= a && (p >> 8 & 0xff) <= a && (p & 0xff) <= a);
        }
        // A wrong buffer size is ignored rather than written out of bounds.
        let mut small = vec![7u32; 3];
        paint_ripple(&mut small, size, Corner::TopLeft, (1, 2, 3), frame);
        assert_eq!(small, [7, 7, 7]);
        let mut tl = vec![0u32; 48 * 48];
        paint_ripple(&mut tl, size, Corner::TopLeft, (255, 91, 31), frame);
        assert!(tl[0] >> 24 > 200 && tl[47 * 48 + 47] == 0);
    }

    #[test]
    fn the_ripple_sits_inside_the_monitor() {
        let spots = outer_spots(&[(M, 1.0)], &all(HotCornerAction::Focus));
        for s in spots {
            let (x, y) = s.ripple_origin(48);
            let r = PxRect::new(x, y, 48, 48);
            assert!(M.contains(r.x, r.y) && M.contains(r.right() - 1, r.bottom() - 1));
            assert!(r.contains(s.x, s.y));
        }
    }
}
