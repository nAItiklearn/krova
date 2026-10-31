//! Drag-to-snap state machine. The platform layer feeds it raw drag events
//! (press / move / modifier / release) and executes the returned actions, so the
//! behaviour of the whole snap flow is OS-independent and unit-tested.
//!
//! Rules (see docs/SPEC.md section 1):
//! - Zones only show while the activation modifier is held during a window drag.
//! - Releasing the modifier mid-drag hides the zones and cancels zone snapping.
//! - Releasing the mouse over a highlighted zone snaps to that zone.
//! - Edge snapping needs no modifier and never applies while zones are shown.
//! - A press that resizes the window, or never moves it, is not a drag.

use crate::edge::edge_snap;
use crate::geometry::PxRect;
use crate::zone::Layout;

/// Cursor travel before a press counts as a drag.
pub const DRAG_THRESHOLD_PX: i32 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapSettings {
    pub enabled: bool,
    pub edge_snapping: bool,
    pub edge_px: i32,
    /// Space between zones and around the work area, in pixels (zone snaps only).
    pub gap_px: i32,
}

impl Default for SnapSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            edge_snapping: false,
            edge_px: 8,
            gap_px: 0,
        }
    }
}

/// A connected monitor as seen by the session.
#[derive(Debug, Clone, Copy)]
pub struct MonitorView<'a> {
    /// Full monitor rect (used to find the monitor under the cursor and for edges).
    pub rect: PxRect,
    /// Work area (excludes taskbar / menu bar / dock); zones map onto this.
    pub work: PxRect,
    /// Active layout of this monitor, if it has one.
    pub layout: Option<&'a Layout>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragEvent {
    /// Left button went down on a movable top-level window.
    Press {
        cursor: (i32, i32),
        window: PxRect,
    },
    /// Pointer moved with the left button down. `window` is the dragged
    /// window's current rect.
    Move {
        cursor: (i32, i32),
        window: PxRect,
        modifier_down: bool,
    },
    /// The activation modifier changed without the pointer moving.
    Modifier {
        down: bool,
    },
    Release,
    /// Escape or any other reason to abandon the gesture.
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapTarget {
    Zone(u32),
    Edge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Show the zones of `monitor` (and none of any other monitor).
    ShowZones {
        monitor: usize,
    },
    /// Highlight `zone` (or nothing) on the shown monitor.
    Highlight {
        monitor: usize,
        zone: Option<u32>,
    },
    HideZones,
    /// Preview of an edge snap; `None` removes it.
    EdgePreview {
        monitor: usize,
        rect: Option<PxRect>,
    },
    /// Move the dragged window so its visible frame fills `rect`.
    Snap {
        monitor: usize,
        rect: PxRect,
        target: SnapTarget,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Overlay {
    monitor: usize,
    zone: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EdgeHit {
    monitor: usize,
    rect: PxRect,
}

#[derive(Debug, Clone, Copy)]
struct Dragging {
    cursor: (i32, i32),
    modifier_down: bool,
    overlay: Option<Overlay>,
    edge: Option<EdgeHit>,
}

#[derive(Debug, Clone, Copy)]
enum State {
    Idle,
    Pressed {
        cursor: (i32, i32),
        window: PxRect,
    },
    /// A press that turned out to be a resize; ignored until release.
    Ignored,
    Dragging(Dragging),
}

#[derive(Debug)]
pub struct Session {
    settings: SnapSettings,
    state: State,
}

fn dist2(a: (i32, i32), b: (i32, i32)) -> i64 {
    let dx = i64::from(a.0) - i64::from(b.0);
    let dy = i64::from(a.1) - i64::from(b.1);
    dx * dx + dy * dy
}

/// Recompute overlay and edge preview for the current cursor / modifier.
fn refresh(d: &mut Dragging, s: &SnapSettings, monitors: &[MonitorView<'_>]) -> Vec<Action> {
    let mut out = Vec::new();
    let (cx, cy) = d.cursor;
    let under = monitors.iter().position(|m| m.rect.contains(cx, cy));

    let wanted = if d.modifier_down {
        under.and_then(|i| monitors[i].layout.map(|l| (i, l)))
    } else {
        None
    };
    match wanted {
        Some((i, layout)) => {
            let zone = layout.hit_test_px(monitors[i].work, cx, cy);
            match d.overlay {
                Some(o) if o.monitor == i => {
                    if o.zone != zone {
                        out.push(Action::Highlight { monitor: i, zone });
                    }
                }
                _ => {
                    out.push(Action::ShowZones { monitor: i });
                    out.push(Action::Highlight { monitor: i, zone });
                }
            }
            d.overlay = Some(Overlay { monitor: i, zone });
        }
        None => {
            if d.overlay.take().is_some() {
                out.push(Action::HideZones);
            }
        }
    }

    let wanted_edge = if d.overlay.is_none() && s.edge_snapping {
        under.and_then(|i| {
            let others: Vec<PxRect> = monitors
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, m)| m.rect)
                .collect();
            edge_snap((cx, cy), monitors[i].rect, &others, s.edge_px).map(|e| EdgeHit {
                monitor: i,
                rect: monitors[i].work.denormalize(e.region),
            })
        })
    } else {
        None
    };
    if wanted_edge != d.edge {
        match wanted_edge {
            Some(e) => out.push(Action::EdgePreview {
                monitor: e.monitor,
                rect: Some(e.rect),
            }),
            None => {
                if let Some(old) = d.edge {
                    out.push(Action::EdgePreview {
                        monitor: old.monitor,
                        rect: None,
                    });
                }
            }
        }
        d.edge = wanted_edge;
    }
    out
}

/// End of gesture: tear down overlays and, when `commit`, emit the snap. `gap` is
/// the zone gap: the snapped rect is the same one the overlay showed.
fn finish(d: Dragging, monitors: &[MonitorView<'_>], commit: bool, gap: i32) -> Vec<Action> {
    let mut out = Vec::new();
    if d.overlay.is_some() {
        out.push(Action::HideZones);
    }
    if let Some(e) = d.edge {
        out.push(Action::EdgePreview {
            monitor: e.monitor,
            rect: None,
        });
    }
    if commit {
        let zone_action = d.overlay.and_then(|o| {
            let zone = o.zone?;
            let m = monitors.get(o.monitor)?;
            let rect = m.layout?.zone_pixels_gapped(zone, m.work, gap)?;
            Some(Action::Snap {
                monitor: o.monitor,
                rect,
                target: SnapTarget::Zone(zone),
            })
        });
        let edge_action = d.edge.map(|e| Action::Snap {
            monitor: e.monitor,
            rect: e.rect,
            target: SnapTarget::Edge,
        });
        out.extend(zone_action.or(edge_action));
    }
    out
}

impl Session {
    pub fn new(settings: SnapSettings) -> Self {
        Self {
            settings,
            state: State::Idle,
        }
    }

    pub fn set_settings(&mut self, settings: SnapSettings) {
        self.settings = settings;
    }

    /// True while a drag is in progress (the platform may use this to decide
    /// whether to keep polling the modifier key).
    pub fn is_dragging(&self) -> bool {
        matches!(self.state, State::Dragging(_))
    }

    pub fn handle(&mut self, ev: DragEvent, monitors: &[MonitorView<'_>]) -> Vec<Action> {
        let state = std::mem::replace(&mut self.state, State::Idle);

        let gap = self.settings.gap_px;
        if !self.settings.enabled {
            return match state {
                State::Dragging(d) => finish(d, monitors, false, gap),
                _ => Vec::new(),
            };
        }

        // A new press always supersedes whatever was going on (a lost release
        // must not leave an overlay on screen).
        if let DragEvent::Press { cursor, window } = ev {
            let out = match state {
                State::Dragging(d) => finish(d, monitors, false, gap),
                _ => Vec::new(),
            };
            self.state = State::Pressed { cursor, window };
            return out;
        }

        let (next, out) = match (state, ev) {
            (
                State::Pressed {
                    cursor: c0,
                    window: w0,
                },
                DragEvent::Move {
                    cursor,
                    window,
                    modifier_down,
                },
            ) => {
                let threshold = i64::from(DRAG_THRESHOLD_PX);
                if window.w != w0.w || window.h != w0.h {
                    (State::Ignored, Vec::new())
                } else if (window.x, window.y) == (w0.x, w0.y)
                    || dist2(cursor, c0) < threshold * threshold
                {
                    (
                        State::Pressed {
                            cursor: c0,
                            window: w0,
                        },
                        Vec::new(),
                    )
                } else {
                    let mut d = Dragging {
                        cursor,
                        modifier_down,
                        overlay: None,
                        edge: None,
                    };
                    let out = refresh(&mut d, &self.settings, monitors);
                    (State::Dragging(d), out)
                }
            }
            (
                State::Dragging(mut d),
                DragEvent::Move {
                    cursor,
                    modifier_down,
                    ..
                },
            ) => {
                d.cursor = cursor;
                d.modifier_down = modifier_down;
                let out = refresh(&mut d, &self.settings, monitors);
                (State::Dragging(d), out)
            }
            (State::Dragging(mut d), DragEvent::Modifier { down }) => {
                d.modifier_down = down;
                let out = refresh(&mut d, &self.settings, monitors);
                (State::Dragging(d), out)
            }
            (State::Dragging(d), DragEvent::Release) => {
                (State::Idle, finish(d, monitors, true, gap))
            }
            (State::Dragging(d), DragEvent::Cancel) => {
                (State::Idle, finish(d, monitors, false, gap))
            }
            (_, DragEvent::Release | DragEvent::Cancel) => (State::Idle, Vec::new()),
            (s, _) => (s, Vec::new()),
        };
        self.state = next;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Rect;
    use crate::zone::templates;

    fn mon(x: i32, layout: Option<&Layout>) -> MonitorView<'_> {
        MonitorView {
            rect: PxRect::new(x, 0, 1920, 1080),
            work: PxRect::new(x, 0, 1920, 1040),
            layout,
        }
    }

    fn win_at(cursor: (i32, i32)) -> PxRect {
        PxRect::new(cursor.0 - 100, cursor.1 - 10, 800, 600)
    }

    fn mv(
        s: &mut Session,
        m: &[MonitorView<'_>],
        cursor: (i32, i32),
        modifier_down: bool,
    ) -> Vec<Action> {
        s.handle(
            DragEvent::Move {
                cursor,
                window: win_at(cursor),
                modifier_down,
            },
            m,
        )
    }

    /// Press at (500, 10) and move far enough to start a real drag, no modifier.
    fn start_drag(s: &mut Session, m: &[MonitorView<'_>]) {
        let p = (500, 10);
        assert!(s
            .handle(
                DragEvent::Press {
                    cursor: p,
                    window: win_at(p)
                },
                m
            )
            .is_empty());
        assert!(mv(s, m, (540, 40), false).is_empty());
        assert!(s.is_dragging());
    }

    #[test]
    fn modifier_drag_highlights_then_snaps_on_release() {
        let layouts = templates();
        let m = [mon(0, Some(&layouts[0]))];
        let mut s = Session::new(SnapSettings::default());
        start_drag(&mut s, &m);

        assert_eq!(
            mv(&mut s, &m, (1500, 30), true),
            vec![
                Action::ShowZones { monitor: 0 },
                Action::Highlight {
                    monitor: 0,
                    zone: Some(2)
                }
            ]
        );
        assert!(
            mv(&mut s, &m, (1600, 30), true).is_empty(),
            "same zone: no churn"
        );
        assert_eq!(
            mv(&mut s, &m, (100, 30), true),
            vec![Action::Highlight {
                monitor: 0,
                zone: Some(1)
            }]
        );
        assert_eq!(
            s.handle(DragEvent::Release, &m),
            vec![
                Action::HideZones,
                Action::Snap {
                    monitor: 0,
                    rect: PxRect::new(0, 0, 960, 1040),
                    target: SnapTarget::Zone(1)
                }
            ]
        );
        assert!(!s.is_dragging());
    }

    #[test]
    fn zone_snaps_honour_the_gap_setting() {
        let layouts = templates();
        let m = [mon(0, Some(&layouts[0]))];
        let mut s = Session::new(SnapSettings {
            gap_px: 10,
            ..SnapSettings::default()
        });
        start_drag(&mut s, &m);
        mv(&mut s, &m, (100, 30), true);
        let out = s.handle(DragEvent::Release, &m);
        assert_eq!(
            out[1],
            Action::Snap {
                monitor: 0,
                rect: PxRect::new(10, 10, 945, 1020),
                target: SnapTarget::Zone(1)
            }
        );
        // The hit-test still uses the full zone, so the gap itself is not a dead zone.
        let mut s = Session::new(SnapSettings {
            gap_px: 32,
            ..SnapSettings::default()
        });
        start_drag(&mut s, &m);
        assert_eq!(
            mv(&mut s, &m, (3, 3), true).last(),
            Some(&Action::Highlight {
                monitor: 0,
                zone: Some(1)
            })
        );
    }

    #[test]
    fn releasing_the_modifier_mid_drag_cancels_snapping() {
        let layouts = templates();
        let m = [mon(0, Some(&layouts[0]))];
        let mut s = Session::new(SnapSettings::default());
        start_drag(&mut s, &m);
        mv(&mut s, &m, (1500, 30), true);
        assert_eq!(mv(&mut s, &m, (1500, 40), false), vec![Action::HideZones]);
        assert!(s.handle(DragEvent::Release, &m).is_empty());
    }

    #[test]
    fn modifier_key_event_without_motion_toggles_the_overlay() {
        let layouts = templates();
        let m = [mon(0, Some(&layouts[0]))];
        let mut s = Session::new(SnapSettings::default());
        start_drag(&mut s, &m);
        assert_eq!(
            s.handle(DragEvent::Modifier { down: true }, &m),
            vec![
                Action::ShowZones { monitor: 0 },
                Action::Highlight {
                    monitor: 0,
                    zone: Some(1)
                }
            ]
        );
        assert_eq!(
            s.handle(DragEvent::Modifier { down: false }, &m),
            vec![Action::HideZones]
        );
    }

    #[test]
    fn no_modifier_means_no_snap() {
        let layouts = templates();
        let m = [mon(0, Some(&layouts[0]))];
        let mut s = Session::new(SnapSettings::default());
        start_drag(&mut s, &m);
        assert!(mv(&mut s, &m, (1500, 30), false).is_empty());
        assert!(s.handle(DragEvent::Release, &m).is_empty());
    }

    #[test]
    fn release_over_no_zone_snaps_nothing() {
        let layouts = templates();
        let m = [mon(0, Some(&layouts[0]))];
        let mut s = Session::new(SnapSettings::default());
        start_drag(&mut s, &m);
        // Cursor in the taskbar strip below the work area: on the monitor, in no zone.
        assert_eq!(
            mv(&mut s, &m, (1500, 1060), true),
            vec![
                Action::ShowZones { monitor: 0 },
                Action::Highlight {
                    monitor: 0,
                    zone: None
                }
            ]
        );
        assert_eq!(s.handle(DragEvent::Release, &m), vec![Action::HideZones]);
    }

    #[test]
    fn a_click_that_never_moves_the_window_is_not_a_drag() {
        let layouts = templates();
        let m = [mon(0, Some(&layouts[0]))];
        let mut s = Session::new(SnapSettings::default());
        let w = win_at((500, 10));
        s.handle(
            DragEvent::Press {
                cursor: (500, 10),
                window: w,
            },
            &m,
        );
        // Cursor travels but the window stays put (e.g. text selection).
        let out = s.handle(
            DragEvent::Move {
                cursor: (900, 300),
                window: w,
                modifier_down: true,
            },
            &m,
        );
        assert!(out.is_empty());
        assert!(!s.is_dragging());
        assert!(s.handle(DragEvent::Release, &m).is_empty());
    }

    #[test]
    fn tiny_wobble_stays_below_the_drag_threshold() {
        let layouts = templates();
        let m = [mon(0, Some(&layouts[0]))];
        let mut s = Session::new(SnapSettings::default());
        s.handle(
            DragEvent::Press {
                cursor: (500, 10),
                window: win_at((500, 10)),
            },
            &m,
        );
        assert!(mv(&mut s, &m, (503, 12), true).is_empty());
        assert!(!s.is_dragging());
    }

    #[test]
    fn resizing_a_window_never_snaps() {
        let layouts = templates();
        let m = [mon(0, Some(&layouts[0]))];
        let mut s = Session::new(SnapSettings::default());
        let w = win_at((500, 10));
        s.handle(
            DragEvent::Press {
                cursor: (500, 10),
                window: w,
            },
            &m,
        );
        let grown = PxRect::new(w.x, w.y, w.w + 200, w.h);
        let out = s.handle(
            DragEvent::Move {
                cursor: (900, 300),
                window: grown,
                modifier_down: true,
            },
            &m,
        );
        assert!(out.is_empty());
        // Later moves of the same gesture are still ignored.
        assert!(mv(&mut s, &m, (1500, 30), true).is_empty());
        assert!(s.handle(DragEvent::Release, &m).is_empty());
    }

    #[test]
    fn overlay_follows_the_cursor_across_monitors() {
        let layouts = templates();
        let m = [mon(0, Some(&layouts[0])), mon(1920, Some(&layouts[1]))];
        let mut s = Session::new(SnapSettings::default());
        start_drag(&mut s, &m);
        mv(&mut s, &m, (1000, 30), true);
        assert_eq!(
            mv(&mut s, &m, (1920 + 1500, 30), true),
            vec![
                Action::ShowZones { monitor: 1 },
                Action::Highlight {
                    monitor: 1,
                    zone: Some(3)
                }
            ]
        );
        let out = s.handle(DragEvent::Release, &m);
        assert_eq!(out.len(), 2);
        // Thirds on a 1920 wide work area starting at x = 1920.
        assert_eq!(
            out[1],
            Action::Snap {
                monitor: 1,
                rect: PxRect::new(1920 + 1280, 0, 640, 1040),
                target: SnapTarget::Zone(3)
            }
        );
    }

    #[test]
    fn a_monitor_without_a_layout_shows_nothing() {
        let m = [mon(0, None)];
        let mut s = Session::new(SnapSettings::default());
        start_drag(&mut s, &m);
        assert!(mv(&mut s, &m, (1500, 30), true).is_empty());
        assert!(s.handle(DragEvent::Release, &m).is_empty());
    }

    #[test]
    fn disabled_session_does_nothing_and_cleans_up() {
        let layouts = templates();
        let m = [mon(0, Some(&layouts[0]))];
        let mut s = Session::new(SnapSettings::default());
        start_drag(&mut s, &m);
        mv(&mut s, &m, (1500, 30), true);
        s.set_settings(SnapSettings {
            enabled: false,
            ..SnapSettings::default()
        });
        assert_eq!(mv(&mut s, &m, (1500, 40), true), vec![Action::HideZones]);
        assert!(s.handle(DragEvent::Release, &m).is_empty());
        let p = (500, 10);
        assert!(s
            .handle(
                DragEvent::Press {
                    cursor: p,
                    window: win_at(p)
                },
                &m
            )
            .is_empty());
        assert!(!s.is_dragging());
    }

    #[test]
    fn cancel_hides_and_does_not_snap() {
        let layouts = templates();
        let m = [mon(0, Some(&layouts[0]))];
        let mut s = Session::new(SnapSettings::default());
        start_drag(&mut s, &m);
        mv(&mut s, &m, (1500, 30), true);
        assert_eq!(s.handle(DragEvent::Cancel, &m), vec![Action::HideZones]);
        assert!(mv(&mut s, &m, (100, 30), true).is_empty());
        assert!(s.handle(DragEvent::Release, &m).is_empty());
    }

    #[test]
    fn a_lost_release_does_not_leave_an_overlay_behind() {
        let layouts = templates();
        let m = [mon(0, Some(&layouts[0]))];
        let mut s = Session::new(SnapSettings::default());
        start_drag(&mut s, &m);
        mv(&mut s, &m, (1500, 30), true);
        let p = (300, 10);
        assert_eq!(
            s.handle(
                DragEvent::Press {
                    cursor: p,
                    window: win_at(p)
                },
                &m
            ),
            vec![Action::HideZones]
        );
    }

    #[test]
    fn edge_snapping_previews_and_snaps_without_the_modifier() {
        let layouts = templates();
        let m = [mon(0, Some(&layouts[0]))];
        let mut s = Session::new(SnapSettings {
            edge_snapping: true,
            ..SnapSettings::default()
        });
        start_drag(&mut s, &m);
        let left_half = PxRect::new(0, 0, 960, 1040);
        assert_eq!(
            mv(&mut s, &m, (2, 500), false),
            vec![Action::EdgePreview {
                monitor: 0,
                rect: Some(left_half)
            }]
        );
        assert!(mv(&mut s, &m, (3, 520), false).is_empty());
        assert_eq!(
            s.handle(DragEvent::Release, &m),
            vec![
                Action::EdgePreview {
                    monitor: 0,
                    rect: None
                },
                Action::Snap {
                    monitor: 0,
                    rect: left_half,
                    target: SnapTarget::Edge
                }
            ]
        );
    }

    #[test]
    fn edge_preview_goes_away_when_the_cursor_leaves_the_edge() {
        let layouts = templates();
        let m = [mon(0, Some(&layouts[0]))];
        let mut s = Session::new(SnapSettings {
            edge_snapping: true,
            ..SnapSettings::default()
        });
        start_drag(&mut s, &m);
        mv(&mut s, &m, (2, 500), false);
        assert_eq!(
            mv(&mut s, &m, (400, 500), false),
            vec![Action::EdgePreview {
                monitor: 0,
                rect: None
            }]
        );
        assert!(s.handle(DragEvent::Release, &m).is_empty());
    }

    #[test]
    fn edge_snapping_ignores_the_border_between_two_monitors() {
        let layouts = templates();
        let m = [mon(0, Some(&layouts[0])), mon(1920, Some(&layouts[0]))];
        let mut s = Session::new(SnapSettings {
            edge_snapping: true,
            ..SnapSettings::default()
        });
        start_drag(&mut s, &m);
        assert!(mv(&mut s, &m, (1918, 500), false).is_empty());
        assert!(s.handle(DragEvent::Release, &m).is_empty());
    }

    #[test]
    fn zones_take_priority_over_edge_snapping() {
        let layouts = templates();
        let m = [mon(0, Some(&layouts[0]))];
        let mut s = Session::new(SnapSettings {
            edge_snapping: true,
            ..SnapSettings::default()
        });
        start_drag(&mut s, &m);
        mv(&mut s, &m, (2, 500), false); // edge preview is up
        assert_eq!(
            mv(&mut s, &m, (2, 501), true),
            vec![
                Action::ShowZones { monitor: 0 },
                Action::Highlight {
                    monitor: 0,
                    zone: Some(1)
                },
                Action::EdgePreview {
                    monitor: 0,
                    rect: None
                }
            ]
        );
        let out = s.handle(DragEvent::Release, &m);
        assert!(matches!(
            out.last(),
            Some(Action::Snap {
                target: SnapTarget::Zone(1),
                ..
            })
        ));
    }

    #[test]
    fn corner_edge_snap_uses_the_work_area() {
        let layouts = templates();
        let m = [mon(0, Some(&layouts[0]))];
        let mut s = Session::new(SnapSettings {
            edge_snapping: true,
            ..SnapSettings::default()
        });
        start_drag(&mut s, &m);
        let expect = m[0].work.denormalize(Rect::new(0.5, 0.0, 0.5, 0.5));
        assert_eq!(
            mv(&mut s, &m, (1918, 2), false),
            vec![Action::EdgePreview {
                monitor: 0,
                rect: Some(expect)
            }]
        );
    }
}
