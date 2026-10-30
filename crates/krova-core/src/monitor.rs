use serde::{Deserialize, Serialize};

use crate::geometry::PxRect;

/// What the OS tells us about a connected monitor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorDescriptor {
    /// Stable hardware identity (EDID manufacturer + product + serial when known).
    pub hardware_id: Option<String>,
    /// OS device path / display UUID. Stable per port on most systems.
    pub device_path: Option<String>,
    /// Position and size in virtual-screen pixels.
    pub rect: PxRect,
}

const MIN_SCORE: u32 = 10;

/// How well `current` matches a previously seen monitor `stored`.
/// 0 means "not the same monitor".
pub fn score(current: &MonitorDescriptor, stored: &MonitorDescriptor) -> u32 {
    let mut s = 0;
    match (&current.hardware_id, &stored.hardware_id) {
        (Some(a), Some(b)) if a == b => s += 100,
        (Some(_), Some(_)) => return 0,
        _ => {}
    }
    if let (Some(a), Some(b)) = (&current.device_path, &stored.device_path) {
        if a == b {
            s += 50;
        }
    }
    if current.rect.w == stored.rect.w && current.rect.h == stored.rect.h {
        s += 5;
        if current.rect.x == stored.rect.x && current.rect.y == stored.rect.y {
            s += 5;
        }
    }
    if s >= MIN_SCORE {
        s
    } else {
        0
    }
}

/// Assign each current monitor to at most one stored monitor, best scores
/// first. Returns, per current monitor, the index into `stored`.
pub fn match_monitors(
    current: &[MonitorDescriptor],
    stored: &[MonitorDescriptor],
) -> Vec<Option<usize>> {
    let mut pairs: Vec<(u32, usize, usize)> = Vec::new();
    for (ci, c) in current.iter().enumerate() {
        for (si, s) in stored.iter().enumerate() {
            let sc = score(c, s);
            if sc > 0 {
                pairs.push((sc, ci, si));
            }
        }
    }
    pairs.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let mut result = vec![None; current.len()];
    let mut used = vec![false; stored.len()];
    for (_, ci, si) in pairs {
        if result[ci].is_none() && !used[si] {
            result[ci] = Some(si);
            used[si] = true;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(hw: Option<&str>, path: Option<&str>, x: i32) -> MonitorDescriptor {
        MonitorDescriptor {
            hardware_id: hw.map(str::to_owned),
            device_path: path.map(str::to_owned),
            rect: PxRect::new(x, 0, 1920, 1080),
        }
    }

    #[test]
    fn same_hardware_matches_even_when_moved_or_new_port() {
        let stored = [d(Some("DEL-A1-123"), Some("port1"), 0)];
        let cur = [d(Some("DEL-A1-123"), Some("port9"), 1920)];
        assert_eq!(match_monitors(&cur, &stored), vec![Some(0)]);
    }

    #[test]
    fn different_hardware_never_matches() {
        let stored = [d(Some("DEL-A1-123"), Some("port1"), 0)];
        let cur = [d(Some("LGD-B2-999"), Some("port1"), 0)];
        assert_eq!(match_monitors(&cur, &stored), vec![None]);
    }

    #[test]
    fn identical_models_are_disambiguated_by_path_then_position() {
        let stored = [
            d(Some("DEL-A1"), Some("left"), 0),
            d(Some("DEL-A1"), Some("right"), 1920),
        ];
        // Order of enumeration flipped after reconnect.
        let cur = [
            d(Some("DEL-A1"), Some("right"), 0),
            d(Some("DEL-A1"), Some("left"), 1920),
        ];
        assert_eq!(match_monitors(&cur, &stored), vec![Some(1), Some(0)]);
    }

    #[test]
    fn each_stored_monitor_is_used_once() {
        let stored = [d(Some("DEL-A1"), None, 0)];
        let cur = [d(Some("DEL-A1"), None, 0), d(Some("DEL-A1"), None, 1920)];
        let m = match_monitors(&cur, &stored);
        assert_eq!(m.iter().filter(|x| x.is_some()).count(), 1);
        assert_eq!(m[0], Some(0));
    }

    #[test]
    fn fallback_to_geometry_without_ids() {
        let stored = [d(None, None, 0)];
        assert_eq!(match_monitors(&[d(None, None, 0)], &stored), vec![Some(0)]);
        // Same size but different position and no ids: too weak, treated as new.
        assert_eq!(match_monitors(&[d(None, None, 1920)], &stored), vec![None]);
    }
}
