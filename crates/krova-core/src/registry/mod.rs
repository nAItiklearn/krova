//! The snap registry: which window Krova put into which zone.
//!
//! It lives in memory only. A window is identified by an opaque [`WindowKey`]
//! (the platform layer's handle value); the registry never sees a title, a process
//! name or anything else about the window. Nothing here is ever written to disk.
//!
//! Three things build on it:
//! - [`reconnect`]: put windows back into their zones when a monitor returns;
//! - [`select`]: pick the target zone for the keyboard shortcuts;
//! - [`crate::linked`]: keep neighbouring snapped windows' shared edge closed.
//!
//! Rule for keeping it honest: an entry is removed as soon as the user starts
//! dragging that window or places it somewhere else, and when the window is gone.
//! The size is bounded ([`MAX_ENTRIES`]); when full, the oldest entry makes room.
//! An entry also expires after [`MAX_AGE`] (a window left alone for hours is no
//! longer "where Krova put it" in any way the user remembers), and it carries the
//! [`WindowOwner`] of the window, so a handle the OS reused for another program is
//! never moved by mistake.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use crate::geometry::PxRect;

pub mod reconnect;
pub mod select;

pub use reconnect::{
    plan_reconnect, ConnectedMonitor, Placement, ReconnectPlan, WindowSnapshot,
    IN_PLACE_TOLERANCE_PX, MAX_RESTORED_PER_RECONNECT,
};
pub use select::{
    monitor_for_rect, monitor_order, plan_keyboard_move, reading_order, zone_by_number,
    zone_for_rect, zone_index, KeyAction, KeyboardTarget,
};

/// Most windows the registry remembers. Far more than anyone snaps; it only exists
/// so that a runaway caller cannot grow the map without limit.
pub const MAX_ENTRIES: usize = 1024;

/// How long an entry is kept without being refreshed. Six hours is longer than any
/// work session that leaves a monitor unplugged, and short enough that a stale
/// handle does not linger for days.
pub const MAX_AGE: Duration = Duration::from_secs(6 * 60 * 60);

/// Opaque identity of a window, as handed over by the platform layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WindowKey(pub u64);

/// The program a window belongs to, as the OS knows it: the process id and the
/// moment the process was created (in the OS's own tick unit). A window handle can be
/// reused by the OS for a different window after the first one closed; the pair
/// `(pid, started)` tells the two apart, because a process id is reused only for a
/// process created later. No name, path or title is ever part of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowOwner {
    pub pid: u32,
    /// Process creation time (Windows: `FILETIME` ticks of 100 ns).
    pub started: u64,
}

/// Where a window was put.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapRecord {
    /// Profile id (`monitor-N`) of the monitor the zone is on.
    pub monitor_profile_id: String,
    pub layout_id: String,
    pub zone_id: u32,
    /// The visible frame the window had right after Krova placed it. This is the
    /// zone rectangle unless the window refused that size (a minimum or maximum
    /// size), in which case it is what the window really ended up with. Linked
    /// dividers update it when a neighbour's edge is moved.
    pub rect: PxRect,
    /// Whose window it was when Krova placed it; `None` where the platform cannot
    /// tell. A record with an owner is only acted on while the window still has
    /// exactly that owner.
    pub owner: Option<WindowOwner>,
}

#[derive(Debug, Clone)]
struct Entry {
    record: SnapRecord,
    /// Insertion order; the smallest value is the oldest entry.
    seq: u64,
    /// When the entry was inserted (a replaced entry counts as newly inserted).
    at: Instant,
}

#[derive(Debug, Default, Clone)]
pub struct SnapRegistry {
    // A BTreeMap keeps iteration order stable, so plans built from the registry
    // are deterministic (and testable).
    entries: BTreeMap<WindowKey, Entry>,
    next_seq: u64,
}

impl SnapRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Record (or replace) where `key` was put. A replaced entry counts as newly
    /// inserted. At the size cap the oldest other entry is evicted first. Returns
    /// the record that was replaced, if any.
    pub fn insert(&mut self, key: WindowKey, record: SnapRecord) -> Option<SnapRecord> {
        self.insert_at(key, record, Instant::now())
    }

    /// [`SnapRegistry::insert`] with the insertion time given (for tests and for
    /// callers that already hold a clock reading).
    pub fn insert_at(
        &mut self,
        key: WindowKey,
        record: SnapRecord,
        now: Instant,
    ) -> Option<SnapRecord> {
        let previous = self.entries.remove(&key).map(|e| e.record);
        if self.entries.len() >= MAX_ENTRIES {
            if let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, e)| e.seq)
                .map(|(k, _)| *k)
            {
                self.entries.remove(&oldest);
            }
        }
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        self.entries.insert(
            key,
            Entry {
                record,
                seq,
                at: now,
            },
        );
        previous
    }

    /// Forget the entries older than `max_age` at `now`; returns how many went.
    pub fn expire(&mut self, now: Instant, max_age: Duration) -> usize {
        let before = self.entries.len();
        self.entries
            .retain(|_, e| now.saturating_duration_since(e.at) <= max_age);
        before - self.entries.len()
    }

    pub fn remove(&mut self, key: WindowKey) -> Option<SnapRecord> {
        self.entries.remove(&key).map(|e| e.record)
    }

    pub fn get(&self, key: WindowKey) -> Option<&SnapRecord> {
        self.entries.get(&key).map(|e| &e.record)
    }

    pub fn contains(&self, key: WindowKey) -> bool {
        self.entries.contains_key(&key)
    }

    /// Update the remembered rect of a window that is still in its zone (for
    /// example after a linked divider moved its edge). `false` if it is unknown.
    pub fn set_rect(&mut self, key: WindowKey, rect: PxRect) -> bool {
        match self.entries.get_mut(&key) {
            Some(e) => {
                e.record.rect = rect;
                true
            }
            None => false,
        }
    }

    /// All entries, ordered by window key.
    pub fn iter(&self) -> impl Iterator<Item = (WindowKey, &SnapRecord)> {
        self.entries.iter().map(|(k, e)| (*k, &e.record))
    }

    /// Entries whose zone is on the monitor with this profile id.
    pub fn on_monitor<'a>(
        &'a self,
        profile_id: &'a str,
    ) -> impl Iterator<Item = (WindowKey, &'a SnapRecord)> + 'a {
        self.iter()
            .filter(move |(_, r)| r.monitor_profile_id == profile_id)
    }

    /// Entries whose zone belongs to this layout of this monitor.
    pub fn on_layout<'a>(
        &'a self,
        profile_id: &'a str,
        layout_id: &'a str,
    ) -> impl Iterator<Item = (WindowKey, &'a SnapRecord)> + 'a {
        self.on_monitor(profile_id)
            .filter(move |(_, r)| r.layout_id == layout_id)
    }

    /// Keep only the entries for which `keep` returns true.
    pub fn retain(&mut self, mut keep: impl FnMut(WindowKey, &SnapRecord) -> bool) {
        self.entries.retain(|k, e| keep(*k, &e.record));
    }

    /// Forget every window of one monitor profile; returns how many were removed.
    pub fn remove_monitor(&mut self, profile_id: &str) -> usize {
        let before = self.entries.len();
        self.entries
            .retain(|_, e| e.record.monitor_profile_id != profile_id);
        before - self.entries.len()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(monitor: &str, layout: &str, zone: u32) -> SnapRecord {
        SnapRecord {
            monitor_profile_id: monitor.to_owned(),
            layout_id: layout.to_owned(),
            zone_id: zone,
            rect: PxRect::new(0, 0, 100, 100),
            owner: None,
        }
    }

    #[test]
    fn insert_get_remove() {
        let mut r = SnapRegistry::new();
        assert!(r.is_empty());
        assert_eq!(r.insert(WindowKey(1), rec("monitor-1", "halves", 1)), None);
        assert_eq!(r.len(), 1);
        assert!(r.contains(WindowKey(1)));
        assert_eq!(r.get(WindowKey(1)).unwrap().zone_id, 1);
        assert_eq!(r.get(WindowKey(2)), None);
        assert_eq!(r.remove(WindowKey(1)).unwrap().layout_id, "halves");
        assert_eq!(r.remove(WindowKey(1)), None);
        assert!(r.is_empty());
    }

    #[test]
    fn inserting_again_replaces_and_returns_the_old_record() {
        let mut r = SnapRegistry::new();
        r.insert(WindowKey(1), rec("monitor-1", "halves", 1));
        let old = r.insert(WindowKey(1), rec("monitor-2", "grid", 4)).unwrap();
        assert_eq!(old.monitor_profile_id, "monitor-1");
        assert_eq!(r.len(), 1);
        assert_eq!(r.get(WindowKey(1)).unwrap().zone_id, 4);
    }

    #[test]
    fn queries_by_monitor_and_layout() {
        let mut r = SnapRegistry::new();
        r.insert(WindowKey(1), rec("monitor-1", "halves", 1));
        r.insert(WindowKey(2), rec("monitor-1", "grid", 2));
        r.insert(WindowKey(3), rec("monitor-2", "halves", 1));
        let on1: Vec<_> = r.on_monitor("monitor-1").map(|(k, _)| k.0).collect();
        assert_eq!(on1, vec![1, 2]);
        let l: Vec<_> = r
            .on_layout("monitor-1", "halves")
            .map(|(k, _)| k.0)
            .collect();
        assert_eq!(l, vec![1]);
        assert_eq!(r.on_monitor("monitor-9").count(), 0);
        assert_eq!(r.iter().count(), 3);
    }

    #[test]
    fn set_rect_updates_only_known_windows() {
        let mut r = SnapRegistry::new();
        r.insert(WindowKey(1), rec("monitor-1", "halves", 1));
        let moved = PxRect::new(5, 6, 7, 8);
        assert!(r.set_rect(WindowKey(1), moved));
        assert_eq!(r.get(WindowKey(1)).unwrap().rect, moved);
        assert!(!r.set_rect(WindowKey(9), moved));
    }

    #[test]
    fn retain_and_remove_monitor() {
        let mut r = SnapRegistry::new();
        for i in 0..6 {
            let monitor = if i % 2 == 0 { "monitor-1" } else { "monitor-2" };
            r.insert(WindowKey(i), rec(monitor, "halves", 1));
        }
        assert_eq!(r.remove_monitor("monitor-2"), 3);
        assert_eq!(r.len(), 3);
        r.retain(|k, _| k.0 != 0);
        assert_eq!(r.len(), 2);
        assert_eq!(r.remove_monitor("monitor-2"), 0);
        r.clear();
        assert!(r.is_empty());
    }

    #[test]
    fn the_registry_is_bounded_and_evicts_the_oldest() {
        let mut r = SnapRegistry::new();
        for i in 0..MAX_ENTRIES as u64 {
            r.insert(WindowKey(i), rec("monitor-1", "halves", 1));
        }
        assert_eq!(r.len(), MAX_ENTRIES);
        // Refreshing key 0 makes key 1 the oldest.
        r.insert(WindowKey(0), rec("monitor-1", "halves", 2));
        assert_eq!(r.len(), MAX_ENTRIES);
        r.insert(WindowKey(99_999), rec("monitor-1", "halves", 1));
        assert_eq!(r.len(), MAX_ENTRIES);
        assert!(!r.contains(WindowKey(1)), "the oldest entry made room");
        assert!(r.contains(WindowKey(0)));
        assert!(r.contains(WindowKey(99_999)));
    }

    #[test]
    fn heavy_churn_never_exceeds_the_cap() {
        let mut r = SnapRegistry::new();
        for i in 0..(MAX_ENTRIES as u64 * 3) {
            r.insert(WindowKey(i), rec("monitor-1", "halves", 1));
            assert!(r.len() <= MAX_ENTRIES);
        }
        assert_eq!(r.len(), MAX_ENTRIES);
        // The newest survive.
        assert!(r.contains(WindowKey(MAX_ENTRIES as u64 * 3 - 1)));
        assert!(!r.contains(WindowKey(0)));
    }

    #[test]
    fn entries_expire_after_six_hours_and_a_refresh_starts_the_clock_again() {
        let mut r = SnapRegistry::new();
        let t0 = Instant::now();
        r.insert_at(WindowKey(1), rec("monitor-1", "halves", 1), t0);
        r.insert_at(
            WindowKey(2),
            rec("monitor-1", "halves", 2),
            t0 + Duration::from_secs(3 * 3600),
        );
        // Five hours in: nothing is old enough yet.
        assert_eq!(r.expire(t0 + Duration::from_secs(5 * 3600), MAX_AGE), 0);
        // Exactly six hours is still fine; one second more and the first is gone.
        assert_eq!(r.expire(t0 + MAX_AGE, MAX_AGE), 0);
        assert_eq!(r.expire(t0 + MAX_AGE + Duration::from_secs(1), MAX_AGE), 1);
        assert!(!r.contains(WindowKey(1)));
        assert!(r.contains(WindowKey(2)));
        // Replacing an entry counts as new.
        r.insert_at(
            WindowKey(2),
            rec("monitor-1", "halves", 3),
            t0 + Duration::from_secs(8 * 3600),
        );
        assert_eq!(r.expire(t0 + Duration::from_secs(13 * 3600), MAX_AGE), 0);
        assert_eq!(r.expire(t0 + Duration::from_secs(15 * 3600), MAX_AGE), 1);
        assert!(r.is_empty());
    }

    #[test]
    fn a_clock_that_runs_backwards_never_expires_anything() {
        let mut r = SnapRegistry::new();
        let t0 = Instant::now() + Duration::from_secs(10);
        r.insert_at(WindowKey(1), rec("monitor-1", "halves", 1), t0);
        assert_eq!(r.expire(Instant::now(), Duration::ZERO), 0);
    }

    #[test]
    fn the_owner_is_part_of_the_record() {
        let mut r = SnapRegistry::new();
        let mut record = rec("monitor-1", "halves", 1);
        record.owner = Some(WindowOwner {
            pid: 4242,
            started: 1,
        });
        r.insert(WindowKey(1), record);
        assert_eq!(r.get(WindowKey(1)).unwrap().owner.unwrap().pid, 4242);
    }
}
