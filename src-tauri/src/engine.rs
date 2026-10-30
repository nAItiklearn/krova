//! The drag engine: turns OS drag events into overlay updates and window moves.
//!
//! `plan` is pure (snap-session actions in, effects out) and unit-tested; `run`
//! is the thin loop that feeds it and executes the effects.

use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread;

use krova_core::{
    Action, DragEvent, MonitorView, MoveMode, PxRect, Session, Settings, SnapSettings,
};
use krova_platform::overlay::{Overlay, OverlayScene, OverlayStyle};
use krova_platform::{InputEvent, WindowId};

use crate::model::{lock, Shared};

/// What the overlay currently shows, accumulated from snap-session actions.
#[derive(Debug, Default)]
pub struct SceneState {
    zones_monitor: Option<usize>,
    zones: Vec<(u32, PxRect)>,
    highlight: Option<u32>,
    preview: Option<(usize, PxRect)>,
}

impl SceneState {
    fn scene(&self, monitors: &[MonitorView<'_>], settings: &Settings) -> Option<OverlayScene> {
        let index = self.zones_monitor.or(self.preview.map(|(m, _)| m))?;
        let monitor = monitors.get(index)?.rect;
        Some(OverlayScene {
            monitor,
            zones: self.zones.clone(),
            highlight: self.highlight,
            preview: self.preview.filter(|(m, _)| *m == index).map(|(_, r)| r),
            style: OverlayStyle::from_settings(
                &settings.highlight_color,
                settings.highlight_opacity,
            ),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Show this scene on the overlay (`None` hides it).
    Scene(Option<OverlayScene>),
    Place {
        window: WindowId,
        target: PxRect,
        work: PxRect,
        animate: bool,
    },
}

/// Translate snap-session `actions` into effects. Scene changes within one batch
/// are merged into a single update, flushed before any window move.
pub fn plan(
    state: &mut SceneState,
    actions: &[Action],
    window: Option<WindowId>,
    monitors: &[MonitorView<'_>],
    settings: &Settings,
) -> Vec<Effect> {
    let mut out = Vec::new();
    let mut dirty = false;
    for action in actions {
        match *action {
            Action::ShowZones { monitor } => {
                let Some(m) = monitors.get(monitor) else {
                    continue;
                };
                state.zones = m
                    .layout
                    .map(|l| {
                        l.zones
                            .iter()
                            .filter_map(|z| l.zone_pixels(z.id, m.work).map(|r| (z.id, r)))
                            .collect()
                    })
                    .unwrap_or_default();
                state.zones_monitor = Some(monitor);
                state.highlight = None;
                dirty = true;
            }
            Action::Highlight { zone, .. } => {
                state.highlight = zone;
                dirty = true;
            }
            Action::HideZones => {
                state.zones_monitor = None;
                state.zones.clear();
                state.highlight = None;
                dirty = true;
            }
            Action::EdgePreview { monitor, rect } => {
                state.preview = rect.map(|r| (monitor, r));
                dirty = true;
            }
            Action::Snap { monitor, rect, .. } => {
                if dirty {
                    out.push(Effect::Scene(state.scene(monitors, settings)));
                    dirty = false;
                }
                if let (Some(window), Some(m)) = (window, monitors.get(monitor)) {
                    out.push(Effect::Place {
                        window,
                        target: rect,
                        work: m.work,
                        animate: settings.move_mode == MoveMode::Animate,
                    });
                }
            }
        }
    }
    if dirty {
        out.push(Effect::Scene(state.scene(monitors, settings)));
    }
    out
}

/// Start the OS input monitor and the engine thread. Without input monitoring
/// (unsupported OS, missing permission) the app still runs for editing layouts.
pub fn start(shared: &Arc<Shared>) {
    let (tx, rx) = mpsc::channel();
    let modifier = lock(&shared.model).config.settings.modifier;
    match shared.platform.start_input(modifier, tx) {
        Ok(handle) => *lock(&shared.input) = Some(handle),
        Err(e) => {
            eprintln!("krova: drag input is unavailable: {e}");
            return;
        }
    }
    let overlay = match shared.platform.create_overlay() {
        Ok(o) => Some(o),
        Err(e) => {
            eprintln!("krova: overlay is unavailable: {e}");
            None
        }
    };
    let shared = Arc::clone(shared);
    let spawned = thread::Builder::new()
        .name("krova-engine".into())
        .spawn(move || run(&rx, &shared, overlay.as_deref()));
    if let Err(e) = spawned {
        eprintln!("krova: could not start the engine thread: {e}");
    }
}

fn run(rx: &Receiver<InputEvent>, shared: &Shared, overlay: Option<&dyn Overlay>) {
    let mut session = Session::new(SnapSettings::default());
    let mut scene = SceneState::default();
    while let Ok(ev) = rx.recv() {
        // A fresh press is a cheap moment to notice monitors that were plugged in or out.
        if matches!(ev.event, DragEvent::Press { .. }) {
            if let Ok(infos) = shared.platform.monitors() {
                if let Err(e) = lock(&shared.model).refresh_monitors(infos) {
                    eprintln!("krova: could not save the config: {e}");
                }
            }
        }
        let effects = {
            let model = lock(&shared.model);
            let settings = &model.config.settings;
            session.set_settings(SnapSettings {
                enabled: settings.enabled,
                edge_snapping: settings.edge_snapping,
                edge_px: settings.edge_size_px,
            });
            let views = model.views();
            let actions = session.handle(ev.event, &views);
            plan(&mut scene, &actions, ev.window, &views, settings)
        };
        // The model lock is released: moving a window can take ~120 ms when animated.
        for effect in effects {
            match effect {
                Effect::Scene(s) => {
                    if let Some(o) = overlay {
                        o.set_scene(s);
                    }
                }
                Effect::Place {
                    window,
                    target,
                    work,
                    animate,
                } => {
                    if let Err(e) = shared.platform.place_window(window, target, work, animate) {
                        eprintln!("krova: could not move the window: {e}");
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use krova_core::{templates, Layout, SnapTarget};

    fn views(layout: &Layout) -> [MonitorView<'_>; 1] {
        [MonitorView {
            rect: PxRect::new(0, 0, 1920, 1080),
            work: PxRect::new(0, 0, 1920, 1040),
            layout: Some(layout),
        }]
    }

    #[test]
    fn showing_and_highlighting_make_one_scene_update() {
        let layouts = templates();
        let v = views(&layouts[0]);
        let mut st = SceneState::default();
        let fx = plan(
            &mut st,
            &[
                Action::ShowZones { monitor: 0 },
                Action::Highlight {
                    monitor: 0,
                    zone: Some(2),
                },
            ],
            None,
            &v,
            &Settings::default(),
        );
        assert_eq!(fx.len(), 1);
        let Effect::Scene(Some(scene)) = &fx[0] else {
            panic!("expected a scene, got {:?}", fx[0]);
        };
        assert_eq!(scene.monitor, v[0].rect);
        assert_eq!(
            scene.zones,
            vec![
                (1, PxRect::new(0, 0, 960, 1040)),
                (2, PxRect::new(960, 0, 960, 1040))
            ]
        );
        assert_eq!(scene.highlight, Some(2));
        assert_eq!(scene.preview, None);
    }

    #[test]
    fn hiding_clears_the_scene() {
        let layouts = templates();
        let v = views(&layouts[0]);
        let mut st = SceneState::default();
        let s = Settings::default();
        plan(&mut st, &[Action::ShowZones { monitor: 0 }], None, &v, &s);
        let fx = plan(&mut st, &[Action::HideZones], None, &v, &s);
        assert_eq!(fx, vec![Effect::Scene(None)]);
    }

    #[test]
    fn edge_preview_shows_and_clears() {
        let layouts = templates();
        let v = views(&layouts[0]);
        let mut st = SceneState::default();
        let s = Settings::default();
        let r = PxRect::new(0, 0, 960, 1040);
        let fx = plan(
            &mut st,
            &[Action::EdgePreview {
                monitor: 0,
                rect: Some(r),
            }],
            None,
            &v,
            &s,
        );
        let Effect::Scene(Some(scene)) = &fx[0] else {
            panic!("expected a scene");
        };
        assert!(scene.zones.is_empty());
        assert_eq!(scene.preview, Some(r));
        let fx = plan(
            &mut st,
            &[Action::EdgePreview {
                monitor: 0,
                rect: None,
            }],
            None,
            &v,
            &s,
        );
        assert_eq!(fx, vec![Effect::Scene(None)]);
    }

    #[test]
    fn snap_hides_the_overlay_before_moving_the_window() {
        let layouts = templates();
        let v = views(&layouts[0]);
        let mut st = SceneState::default();
        let s = Settings::default();
        plan(&mut st, &[Action::ShowZones { monitor: 0 }], None, &v, &s);
        let target = PxRect::new(0, 0, 960, 1040);
        let fx = plan(
            &mut st,
            &[
                Action::HideZones,
                Action::Snap {
                    monitor: 0,
                    rect: target,
                    target: SnapTarget::Zone(1),
                },
            ],
            Some(WindowId(7)),
            &v,
            &s,
        );
        assert_eq!(
            fx,
            vec![
                Effect::Scene(None),
                Effect::Place {
                    window: WindowId(7),
                    target,
                    work: v[0].work,
                    animate: true
                }
            ]
        );
    }

    #[test]
    fn instant_mode_turns_animation_off() {
        let layouts = templates();
        let v = views(&layouts[0]);
        let mut st = SceneState::default();
        let s = Settings {
            move_mode: MoveMode::Instant,
            ..Settings::default()
        };
        let fx = plan(
            &mut st,
            &[Action::Snap {
                monitor: 0,
                rect: PxRect::new(0, 0, 960, 1040),
                target: SnapTarget::Edge,
            }],
            Some(WindowId(1)),
            &v,
            &s,
        );
        assert!(matches!(
            fx.as_slice(),
            [Effect::Place { animate: false, .. }]
        ));
    }

    #[test]
    fn a_snap_without_a_window_moves_nothing() {
        let layouts = templates();
        let v = views(&layouts[0]);
        let mut st = SceneState::default();
        let fx = plan(
            &mut st,
            &[Action::Snap {
                monitor: 0,
                rect: PxRect::new(0, 0, 960, 1040),
                target: SnapTarget::Zone(1),
            }],
            None,
            &v,
            &Settings::default(),
        );
        assert!(fx.is_empty());
    }

    #[test]
    fn actions_for_a_vanished_monitor_are_ignored() {
        let layouts = templates();
        let v = views(&layouts[0]);
        let mut st = SceneState::default();
        let fx = plan(
            &mut st,
            &[Action::ShowZones { monitor: 5 }],
            None,
            &v,
            &Settings::default(),
        );
        assert!(fx.is_empty());
    }
}
