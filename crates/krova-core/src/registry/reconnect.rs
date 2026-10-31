//! Reconnect planning: when a monitor comes back, put the windows that were in its
//! zones back into those zones.
//!
//! The planner is pure. The caller supplies the registry, which monitors were
//! connected before and now, the config (for the layouts) and a snapshot of each
//! registered window; it gets back the placements to perform and the registry
//! entries to forget. It never touches the OS.
//!
//! Safety rules (each one is covered by a test):
//! - only windows whose monitor has just (re)appeared are considered;
//! - a window that no longer exists is forgotten, never placed;
//! - a minimized window is left alone and stays registered;
//! - a window whose layout or zone was deleted is forgotten;
//! - a window that is already in its zone is left alone, so running the plan
//!   twice does nothing the second time;
//! - windows the user dragged or placed elsewhere are not in the registry at all
//!   (the engine removes an entry the moment a drag of that window starts).

use std::collections::{HashMap, HashSet};

use crate::config::Config;
use crate::geometry::PxRect;

use super::{SnapRegistry, WindowKey};

/// How far (in pixels, per edge) a window may be from its zone rectangle and still
/// count as "already in place". Absorbs rounding and DPI differences in the
/// reported frame.
pub const IN_PLACE_TOLERANCE_PX: i32 = 2;

/// What the caller learned about one registered window, right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowSnapshot {
    pub exists: bool,
    pub minimized: bool,
    /// A maximized window was put there by the user, not by a zone: it is
    /// forgotten rather than restored.
    pub maximized: bool,
    /// Current visible frame, if it could be read.
    pub rect: Option<PxRect>,
    /// Profile id of the monitor the window is on now (`None` if on none).
    pub monitor_profile_id: Option<String>,
}

/// A connected monitor and its work area, keyed by profile id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectedMonitor {
    pub profile_id: String,
    pub work: PxRect,
}

/// Put `key` into `zone_id` of `layout_id`: its visible frame should fill `target`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placement {
    pub key: WindowKey,
    pub monitor_profile_id: String,
    pub layout_id: String,
    pub zone_id: u32,
    pub target: PxRect,
    /// Work area of the destination monitor (the window is kept inside it).
    pub work: PxRect,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReconnectPlan {
    /// Windows to move back into their zones, ordered by window key.
    pub placements: Vec<Placement>,
    /// Registry entries that can never be restored (window gone, zone gone).
    pub forget: Vec<WindowKey>,
}

fn near(a: PxRect, b: PxRect) -> bool {
    let t = i64::from(IN_PLACE_TOLERANCE_PX);
    [
        (a.x, b.x),
        (a.y, b.y),
        (a.right(), b.right()),
        (a.bottom(), b.bottom()),
    ]
    .iter()
    .all(|&(p, q)| (i64::from(p) - i64::from(q)).abs() <= t)
}

/// Work out which windows to put back.
///
/// - `previous`: profile ids of the monitors connected before the change;
/// - `current`: the monitors connected now, with their work areas;
/// - `windows`: a snapshot per registered window (a missing one counts as gone);
/// - `gap`: the zone gap setting, so targets match what a snap would produce.
pub fn plan_reconnect(
    registry: &SnapRegistry,
    previous: &[String],
    current: &[ConnectedMonitor],
    config: &Config,
    windows: &HashMap<WindowKey, WindowSnapshot>,
    gap: i32,
) -> ReconnectPlan {
    let before: HashSet<&str> = previous.iter().map(String::as_str).collect();
    let returned: HashMap<&str, &ConnectedMonitor> = current
        .iter()
        .filter(|m| !before.contains(m.profile_id.as_str()))
        .map(|m| (m.profile_id.as_str(), m))
        .collect();
    let mut plan = ReconnectPlan::default();
    if returned.is_empty() {
        return plan;
    }

    for (key, record) in registry.iter() {
        let Some(monitor) = returned.get(record.monitor_profile_id.as_str()) else {
            continue;
        };
        let snapshot = match windows.get(&key) {
            Some(s) if s.exists => s,
            _ => {
                plan.forget.push(key);
                continue;
            }
        };
        let target = config
            .monitors
            .iter()
            .find(|p| p.id == record.monitor_profile_id)
            .and_then(|p| p.layouts.iter().find(|l| l.id == record.layout_id))
            .and_then(|l| l.zone_pixels_gapped(record.zone_id, monitor.work, gap));
        let Some(target) = target else {
            plan.forget.push(key);
            continue;
        };
        if snapshot.maximized {
            plan.forget.push(key);
            continue;
        }
        if snapshot.minimized {
            continue;
        }
        let Some(now) = snapshot.rect else {
            continue;
        };
        let on_home =
            snapshot.monitor_profile_id.as_deref() == Some(record.monitor_profile_id.as_str());
        if on_home && (near(now, target) || near(now, record.rect)) {
            continue;
        }
        plan.placements.push(Placement {
            key,
            monitor_profile_id: record.monitor_profile_id.clone(),
            layout_id: record.layout_id.clone(),
            zone_id: record.zone_id,
            target,
            work: monitor.work,
        });
    }
    plan
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::MonitorDescriptor;
    use crate::registry::SnapRecord;

    const A: &str = "monitor-1";
    const B: &str = "monitor-2";

    fn desc(hw: &str, x: i32) -> MonitorDescriptor {
        MonitorDescriptor {
            hardware_id: Some(hw.to_owned()),
            device_path: None,
            rect: PxRect::new(x, 0, 1920, 1080),
        }
    }

    fn config() -> Config {
        let mut c = Config::default();
        c.resolve_monitors(&[desc("A", 0), desc("B", 1920)]);
        assert_eq!(c.monitors[0].id, A);
        assert_eq!(c.monitors[1].id, B);
        c
    }

    fn work(x: i32) -> PxRect {
        PxRect::new(x, 0, 1920, 1040)
    }

    fn both() -> Vec<ConnectedMonitor> {
        vec![
            ConnectedMonitor {
                profile_id: A.into(),
                work: work(0),
            },
            ConnectedMonitor {
                profile_id: B.into(),
                work: work(1920),
            },
        ]
    }

    fn only_a() -> Vec<String> {
        vec![A.to_owned()]
    }

    fn record(monitor: &str, layout: &str, zone: u32, rect: PxRect) -> SnapRecord {
        SnapRecord {
            monitor_profile_id: monitor.to_owned(),
            layout_id: layout.to_owned(),
            zone_id: zone,
            rect,
        }
    }

    /// A window the OS moved to monitor A while B was gone.
    fn displaced() -> WindowSnapshot {
        WindowSnapshot {
            exists: true,
            minimized: false,
            maximized: false,
            rect: Some(PxRect::new(300, 200, 800, 600)),
            monitor_profile_id: Some(A.to_owned()),
        }
    }

    fn snaps(items: &[(u64, WindowSnapshot)]) -> HashMap<WindowKey, WindowSnapshot> {
        items
            .iter()
            .map(|(k, s)| (WindowKey(*k), s.clone()))
            .collect()
    }

    /// Zone 2 (right half) of B's halves layout.
    fn b_right_half() -> PxRect {
        PxRect::new(1920 + 960, 0, 960, 1040)
    }

    fn registry_with_one() -> SnapRegistry {
        let mut r = SnapRegistry::new();
        r.insert(WindowKey(1), record(B, "halves", 2, b_right_half()));
        r
    }

    #[test]
    fn a_displaced_window_goes_back_to_its_zone_when_the_monitor_returns() {
        let plan = plan_reconnect(
            &registry_with_one(),
            &only_a(),
            &both(),
            &config(),
            &snaps(&[(1, displaced())]),
            0,
        );
        assert!(plan.forget.is_empty());
        assert_eq!(
            plan.placements,
            vec![Placement {
                key: WindowKey(1),
                monitor_profile_id: B.into(),
                layout_id: "halves".into(),
                zone_id: 2,
                target: b_right_half(),
                work: work(1920),
            }]
        );
    }

    #[test]
    fn nothing_happens_when_no_monitor_returned() {
        let reg = registry_with_one();
        let w = snaps(&[(1, displaced())]);
        // Same set before and after.
        let both_ids = vec![A.to_owned(), B.to_owned()];
        let plan = plan_reconnect(&reg, &both_ids, &both(), &config(), &w, 0);
        assert_eq!(plan, ReconnectPlan::default());
        // A monitor went away: nothing to restore either.
        let now = both();
        let plan = plan_reconnect(&reg, &both_ids, &now[..1], &config(), &w, 0);
        assert_eq!(plan, ReconnectPlan::default());
    }

    #[test]
    fn only_windows_of_the_returned_monitor_are_restored() {
        let mut reg = registry_with_one();
        reg.insert(
            WindowKey(2),
            record(A, "halves", 1, PxRect::new(0, 0, 960, 1040)),
        );
        let plan = plan_reconnect(
            &reg,
            &only_a(),
            &both(),
            &config(),
            &snaps(&[(1, displaced()), (2, displaced())]),
            0,
        );
        assert_eq!(plan.placements.len(), 1);
        assert_eq!(plan.placements[0].key, WindowKey(1));
    }

    #[test]
    fn a_window_that_no_longer_exists_is_forgotten_not_placed() {
        let gone = WindowSnapshot {
            exists: false,
            ..displaced()
        };
        let plan = plan_reconnect(
            &registry_with_one(),
            &only_a(),
            &both(),
            &config(),
            &snaps(&[(1, gone)]),
            0,
        );
        assert!(plan.placements.is_empty());
        assert_eq!(plan.forget, vec![WindowKey(1)]);
        // No snapshot at all counts as gone.
        let plan = plan_reconnect(
            &registry_with_one(),
            &only_a(),
            &both(),
            &config(),
            &HashMap::new(),
            0,
        );
        assert_eq!(plan.forget, vec![WindowKey(1)]);
    }

    #[test]
    fn a_minimized_window_is_skipped_but_stays_registered() {
        let min = WindowSnapshot {
            minimized: true,
            ..displaced()
        };
        let plan = plan_reconnect(
            &registry_with_one(),
            &only_a(),
            &both(),
            &config(),
            &snaps(&[(1, min)]),
            0,
        );
        assert_eq!(plan, ReconnectPlan::default());
    }

    #[test]
    fn a_maximized_window_is_forgotten_not_unmaximized() {
        let max = WindowSnapshot {
            maximized: true,
            ..displaced()
        };
        let plan = plan_reconnect(
            &registry_with_one(),
            &only_a(),
            &both(),
            &config(),
            &snaps(&[(1, max)]),
            0,
        );
        assert!(plan.placements.is_empty());
        assert_eq!(plan.forget, vec![WindowKey(1)]);
    }

    #[test]
    fn a_deleted_layout_or_zone_forgets_the_window() {
        let mut cfg = config();
        cfg.delete_layout(B, "halves").unwrap();
        let plan = plan_reconnect(
            &registry_with_one(),
            &only_a(),
            &both(),
            &cfg,
            &snaps(&[(1, displaced())]),
            0,
        );
        assert!(plan.placements.is_empty());
        assert_eq!(plan.forget, vec![WindowKey(1)]);

        // The layout still exists but the zone does not.
        let mut reg = SnapRegistry::new();
        reg.insert(WindowKey(1), record(B, "halves", 9, b_right_half()));
        let plan = plan_reconnect(
            &reg,
            &only_a(),
            &both(),
            &config(),
            &snaps(&[(1, displaced())]),
            0,
        );
        assert_eq!(plan.forget, vec![WindowKey(1)]);
    }

    #[test]
    fn a_profile_missing_from_the_config_forgets_the_window() {
        let mut reg = SnapRegistry::new();
        reg.insert(
            WindowKey(1),
            record("monitor-9", "halves", 1, b_right_half()),
        );
        let current = vec![ConnectedMonitor {
            profile_id: "monitor-9".into(),
            work: work(0),
        }];
        let plan = plan_reconnect(
            &reg,
            &[],
            &current,
            &config(),
            &snaps(&[(1, displaced())]),
            0,
        );
        assert_eq!(plan.forget, vec![WindowKey(1)]);
    }

    #[test]
    fn a_window_already_in_its_zone_is_left_alone() {
        let home = WindowSnapshot {
            exists: true,
            minimized: false,
            maximized: false,
            rect: Some(b_right_half()),
            monitor_profile_id: Some(B.to_owned()),
        };
        let plan = plan_reconnect(
            &registry_with_one(),
            &only_a(),
            &both(),
            &config(),
            &snaps(&[(1, home)]),
            0,
        );
        assert_eq!(plan, ReconnectPlan::default());
    }

    #[test]
    fn a_window_within_the_tolerance_counts_as_in_place() {
        let off_by_two = WindowSnapshot {
            exists: true,
            minimized: false,
            maximized: false,
            rect: Some(PxRect::new(
                b_right_half().x + 2,
                b_right_half().y,
                b_right_half().w - 2,
                b_right_half().h,
            )),
            monitor_profile_id: Some(B.to_owned()),
        };
        let plan = plan_reconnect(
            &registry_with_one(),
            &only_a(),
            &both(),
            &config(),
            &snaps(&[(1, off_by_two)]),
            0,
        );
        assert!(plan.placements.is_empty());
        let off_by_three = WindowSnapshot {
            rect: Some(PxRect::new(
                b_right_half().x + 3,
                b_right_half().y,
                b_right_half().w - 3,
                b_right_half().h,
            )),
            ..displaced()
        };
        let on_b = WindowSnapshot {
            monitor_profile_id: Some(B.to_owned()),
            ..off_by_three
        };
        let plan = plan_reconnect(
            &registry_with_one(),
            &only_a(),
            &both(),
            &config(),
            &snaps(&[(1, on_b)]),
            0,
        );
        assert_eq!(plan.placements.len(), 1, "3 px off is no longer in place");
    }

    #[test]
    fn a_window_that_refused_the_zone_size_is_not_pushed_again() {
        // It was told to fill the right half but its minimum width made it larger;
        // the registry holds what it really got.
        let achieved = PxRect::new(1920 + 700, 0, 1220, 1040);
        let mut reg = SnapRegistry::new();
        reg.insert(WindowKey(1), record(B, "halves", 2, achieved));
        let home = WindowSnapshot {
            exists: true,
            minimized: false,
            maximized: false,
            rect: Some(achieved),
            monitor_profile_id: Some(B.to_owned()),
        };
        let plan = plan_reconnect(&reg, &only_a(), &both(), &config(), &snaps(&[(1, home)]), 0);
        assert_eq!(plan, ReconnectPlan::default());
    }

    #[test]
    fn a_window_on_the_returned_monitor_but_out_of_its_zone_is_put_back() {
        let wandered = WindowSnapshot {
            exists: true,
            minimized: false,
            maximized: false,
            rect: Some(PxRect::new(1920 + 50, 50, 500, 400)),
            monitor_profile_id: Some(B.to_owned()),
        };
        let plan = plan_reconnect(
            &registry_with_one(),
            &only_a(),
            &both(),
            &config(),
            &snaps(&[(1, wandered)]),
            0,
        );
        assert_eq!(plan.placements.len(), 1);
    }

    #[test]
    fn a_window_whose_position_is_unknown_is_skipped() {
        let blind = WindowSnapshot {
            rect: None,
            ..displaced()
        };
        let plan = plan_reconnect(
            &registry_with_one(),
            &only_a(),
            &both(),
            &config(),
            &snaps(&[(1, blind)]),
            0,
        );
        assert_eq!(plan, ReconnectPlan::default());
    }

    #[test]
    fn running_the_plan_twice_does_nothing_the_second_time() {
        let mut reg = registry_with_one();
        reg.insert(
            WindowKey(2),
            record(B, "halves", 1, PxRect::new(1920, 0, 960, 1040)),
        );
        let cfg = config();
        let mut windows = snaps(&[(1, displaced()), (2, displaced())]);

        let first = plan_reconnect(&reg, &only_a(), &both(), &cfg, &windows, 0);
        assert_eq!(first.placements.len(), 2);

        // Execute it: windows land on their targets, the registry keeps what they got.
        for p in &first.placements {
            let s = windows.get_mut(&p.key).unwrap();
            s.rect = Some(p.target);
            s.monitor_profile_id = Some(p.monitor_profile_id.clone());
            reg.set_rect(p.key, p.target);
        }

        // Same "previous" set again (the caller has not advanced it yet): no-op.
        let second = plan_reconnect(&reg, &only_a(), &both(), &cfg, &windows, 0);
        assert_eq!(second, ReconnectPlan::default());
        // And once the caller advanced it, trivially nothing returned.
        let third = plan_reconnect(
            &reg,
            &[A.to_owned(), B.to_owned()],
            &both(),
            &cfg,
            &windows,
            0,
        );
        assert_eq!(third, ReconnectPlan::default());
    }

    #[test]
    fn the_target_uses_the_current_work_area_of_the_returned_monitor() {
        // B comes back at a different resolution and position.
        let current = vec![
            ConnectedMonitor {
                profile_id: A.into(),
                work: work(0),
            },
            ConnectedMonitor {
                profile_id: B.into(),
                work: PxRect::new(-2560, 0, 2560, 1400),
            },
        ];
        let plan = plan_reconnect(
            &registry_with_one(),
            &only_a(),
            &current,
            &config(),
            &snaps(&[(1, displaced())]),
            0,
        );
        assert_eq!(plan.placements[0].target, PxRect::new(-1280, 0, 1280, 1400));
        assert_eq!(plan.placements[0].work, PxRect::new(-2560, 0, 2560, 1400));
    }

    #[test]
    fn the_zone_gap_is_applied_to_the_target() {
        let plan = plan_reconnect(
            &registry_with_one(),
            &only_a(),
            &both(),
            &config(),
            &snaps(&[(1, displaced())]),
            10,
        );
        assert_eq!(
            plan.placements[0].target,
            PxRect::new(1920 + 965, 10, 945, 1020)
        );
    }

    #[test]
    fn placements_come_out_in_window_key_order() {
        let mut reg = SnapRegistry::new();
        for k in [30u64, 10, 20] {
            reg.insert(
                WindowKey(k),
                record(B, "halves", 1, PxRect::new(1920, 0, 960, 1040)),
            );
        }
        let w = snaps(&[(10, displaced()), (20, displaced()), (30, displaced())]);
        let plan = plan_reconnect(&reg, &only_a(), &both(), &config(), &w, 0);
        let keys: Vec<u64> = plan.placements.iter().map(|p| p.key.0).collect();
        assert_eq!(keys, vec![10, 20, 30]);
    }

    #[test]
    fn two_monitors_returning_together_are_both_restored() {
        let mut reg = SnapRegistry::new();
        reg.insert(
            WindowKey(1),
            record(A, "halves", 1, PxRect::new(0, 0, 960, 1040)),
        );
        reg.insert(WindowKey(2), record(B, "halves", 2, b_right_half()));
        let w = snaps(&[
            (
                1,
                WindowSnapshot {
                    monitor_profile_id: None,
                    ..displaced()
                },
            ),
            (2, displaced()),
        ]);
        let plan = plan_reconnect(&reg, &[], &both(), &config(), &w, 0);
        assert_eq!(plan.placements.len(), 2);
    }

    #[test]
    fn an_entry_removed_by_the_user_is_never_restored() {
        // The engine removes an entry when the user starts dragging the window.
        let mut reg = registry_with_one();
        reg.remove(WindowKey(1));
        let plan = plan_reconnect(
            &reg,
            &only_a(),
            &both(),
            &config(),
            &snaps(&[(1, displaced())]),
            0,
        );
        assert_eq!(plan, ReconnectPlan::default());
    }
}
