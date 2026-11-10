//! Stacks: several windows sharing one zone, one of them visible.
//!
//! A [`Stacks`] value is the whole "who is in which zone" state of one monitor.
//! Every arranged window is in exactly one zone; a zone with a single window is a
//! stack of one. Within a zone the windows keep their tab order (the order they
//! joined in) and exactly one of them is the visible one.
//!
//! Invariants, enforced by every operation and checked by
//! [`Stacks::check_invariants`]:
//!
//! * a window is in at most one zone, and appears once in it;
//! * a stored stack is never empty;
//! * the visible window is always a member of its stack;
//! * a stack never holds more than [`MAX_STACK_DEPTH`] windows and the whole
//!   value never more than [`MAX_TRACKED_WINDOWS`].
//!
//! Zones are identified by their stable [`crate::zone::Zone::id`], never by their
//! reading-order position, so editing a layout does not scramble the stacks. The
//! type knows nothing about layouts; callers drop zones that no longer exist with
//! [`Stacks::retain_zones`].
//!
//! Everything here is pure and deterministic.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Opaque identity of a window, as handed over by the platform layer.
pub type WindowId = u64;

/// Most windows one zone may stack. A tab strip with more tabs than this is not
/// usable, and the bound keeps a runaway caller from growing a stack without end.
pub const MAX_STACK_DEPTH: usize = 32;
/// Most windows one [`Stacks`] value tracks. Matches the size of the snap registry.
pub const MAX_TRACKED_WINDOWS: usize = 1024;

/// Which way [`Stacks::cycle`] steps through the tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CycleDirection {
    Next,
    Prev,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum StackError {
    #[error("window {0} is not in any zone")]
    NotTracked(WindowId),
    #[error("a window cannot be swapped with itself")]
    SameWindow,
    #[error("zone {0} has no windows")]
    ZoneEmpty(u32),
    #[error("zone {0} already has windows")]
    ZoneOccupied(u32),
    #[error("zone {0} already holds the most windows a stack may have")]
    StackFull(u32),
    #[error("too many windows are tracked")]
    TooManyWindows,
}

/// One edit of a [`Stacks`] value, as plain data. A plan is a list of these, so the
/// engine can apply it, log it or replay it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackChange {
    /// `window` is now in `zone`, on top. Works for a new window and for one that
    /// is moving out of another zone. Placing a window in the zone it is already in
    /// only brings it to the front.
    Place { window: WindowId, zone: u32 },
    /// `window` leaves its zone. The next tab (or the previous one, if it was the
    /// last) becomes visible when the removed window was.
    Remove { window: WindowId },
    /// `window` becomes the visible one of its stack.
    Raise { window: WindowId },
    /// The whole stack of zone `from` moves to the empty zone `to`.
    MoveStack { from: u32, to: u32 },
    /// The two windows trade places; see [`Stacks::swap`].
    Swap { a: WindowId, b: WindowId },
}

/// What [`Stacks::remove`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Removed {
    /// The zone the window was in.
    pub zone: u32,
    /// The zone has no windows left.
    pub emptied: bool,
    /// The window that became visible because the removed one was. `None` when the
    /// removed window was hidden, or when the zone is empty now.
    pub new_visible: Option<WindowId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ZoneStack {
    /// Tab order: the order the windows joined in. Never empty.
    tabs: Vec<WindowId>,
    /// Index into `tabs` of the visible window. Always `< tabs.len()`.
    front: usize,
}

impl ZoneStack {
    fn visible(&self) -> WindowId {
        self.tabs[self.front]
    }
}

/// Which window is in which zone, plus which one is on top in each zone.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Stacks {
    /// Ordered by zone id, so iteration (and therefore every plan) is deterministic.
    zones: BTreeMap<u32, ZoneStack>,
}

impl Stacks {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build from `(window, zone)` pairs. Windows that share a zone become a stack in
    /// the order given, the last one on top. A window listed twice ends up where it
    /// was listed last. Entries beyond the limits are dropped.
    pub fn from_assignments<I>(items: I) -> Self
    where
        I: IntoIterator<Item = (WindowId, u32)>,
    {
        let mut stacks = Self::new();
        for (window, zone) in items {
            // A refused entry (over a limit) is simply not tracked.
            let _ = stacks.push(zone, window);
        }
        stacks
    }

    /// Number of windows tracked.
    pub fn len(&self) -> usize {
        self.zones.values().map(|s| s.tabs.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.zones.is_empty()
    }

    /// Zones that have at least one window, by ascending zone id.
    pub fn occupied_zones(&self) -> impl Iterator<Item = u32> + '_ {
        self.zones.keys().copied()
    }

    /// The zone `window` is in.
    pub fn zone_of(&self, window: WindowId) -> Option<u32> {
        self.zones
            .iter()
            .find(|(_, s)| s.tabs.contains(&window))
            .map(|(zone, _)| *zone)
    }

    pub fn contains(&self, window: WindowId) -> bool {
        self.zone_of(window).is_some()
    }

    /// Windows of `zone` in tab order. Empty for a zone nobody is in.
    pub fn members(&self, zone: u32) -> &[WindowId] {
        self.zones.get(&zone).map_or(&[], |s| s.tabs.as_slice())
    }

    /// Number of windows in `zone`.
    pub fn depth(&self, zone: u32) -> usize {
        self.members(zone).len()
    }

    /// Tab position of `window` within its zone (0 is the first tab).
    pub fn tab_index(&self, window: WindowId) -> Option<usize> {
        self.zones
            .values()
            .find_map(|s| s.tabs.iter().position(|&w| w == window))
    }

    /// The window currently shown in `zone`.
    pub fn visible(&self, zone: u32) -> Option<WindowId> {
        self.zones.get(&zone).map(ZoneStack::visible)
    }

    /// Whether `window` is the visible one of its zone.
    pub fn is_visible(&self, window: WindowId) -> bool {
        self.zone_of(window)
            .and_then(|zone| self.visible(zone))
            .is_some_and(|v| v == window)
    }

    /// The windows of `zone` that are not on top, in tab order.
    pub fn hidden(&self, zone: u32) -> impl Iterator<Item = WindowId> + '_ {
        let visible = self.visible(zone);
        self.members(zone)
            .iter()
            .copied()
            .filter(move |&w| Some(w) != visible)
    }

    /// Every `(window, zone)` pair: zones by ascending id, windows in tab order.
    pub fn assignments(&self) -> Vec<(WindowId, u32)> {
        self.zones
            .iter()
            .flat_map(|(&zone, s)| s.tabs.iter().map(move |&w| (w, zone)))
            .collect()
    }

    /// Put `window` in `zone`, on top of that zone's stack. A window that is already
    /// somewhere else moves; one that is already in `zone` is just brought to the
    /// front. Nothing changes when an error is returned.
    pub fn push(&mut self, zone: u32, window: WindowId) -> Result<(), StackError> {
        match self.zone_of(window) {
            Some(current) if current == zone => {
                self.bring_to_front(window);
                return Ok(());
            }
            Some(_) => {}
            None => {
                if self.len() >= MAX_TRACKED_WINDOWS {
                    return Err(StackError::TooManyWindows);
                }
            }
        }
        if self.depth(zone) >= MAX_STACK_DEPTH {
            return Err(StackError::StackFull(zone));
        }
        self.remove(window);
        let stack = self.zones.entry(zone).or_insert_with(|| ZoneStack {
            tabs: Vec::new(),
            front: 0,
        });
        stack.tabs.push(window);
        stack.front = stack.tabs.len() - 1;
        Ok(())
    }

    /// Take `window` out of its zone. `None` if it was not tracked.
    pub fn remove(&mut self, window: WindowId) -> Option<Removed> {
        let zone = self.zone_of(window)?;
        let stack = self.zones.get_mut(&zone)?;
        let index = stack.tabs.iter().position(|&w| w == window)?;
        stack.tabs.remove(index);
        if stack.tabs.is_empty() {
            self.zones.remove(&zone);
            return Some(Removed {
                zone,
                emptied: true,
                new_visible: None,
            });
        }
        let was_visible = index == stack.front;
        if index < stack.front {
            // A tab before the visible one went away: the visible window slid left.
            stack.front -= 1;
        } else if was_visible && stack.front >= stack.tabs.len() {
            // The last tab was visible: fall back to the one before it.
            stack.front = stack.tabs.len() - 1;
        }
        Some(Removed {
            zone,
            emptied: false,
            new_visible: was_visible.then(|| stack.visible()),
        })
    }

    /// Show the next or previous tab of `zone` (wrapping) and return the window that
    /// is visible now. `None` when the zone has fewer than two windows: there is
    /// nothing to cycle.
    pub fn cycle(&mut self, zone: u32, direction: CycleDirection) -> Option<WindowId> {
        let stack = self.zones.get_mut(&zone)?;
        let n = stack.tabs.len();
        if n < 2 {
            return None;
        }
        stack.front = match direction {
            CycleDirection::Next => (stack.front + 1) % n,
            CycleDirection::Prev => (stack.front + n - 1) % n,
        };
        Some(stack.visible())
    }

    /// Make `window` the visible one of its stack. Returns whether anything changed
    /// (`false` for an untracked window and for one that is already visible).
    pub fn bring_to_front(&mut self, window: WindowId) -> bool {
        for stack in self.zones.values_mut() {
            if let Some(index) = stack.tabs.iter().position(|&w| w == window) {
                let changed = stack.front != index;
                stack.front = index;
                return changed;
            }
        }
        false
    }

    /// Move the whole stack of zone `from` to the empty zone `to`. Moving a zone onto
    /// itself is a no-op (as long as it has windows).
    pub fn move_stack(&mut self, from: u32, to: u32) -> Result<(), StackError> {
        if !self.zones.contains_key(&from) {
            return Err(StackError::ZoneEmpty(from));
        }
        if from == to {
            return Ok(());
        }
        if self.zones.contains_key(&to) {
            return Err(StackError::ZoneOccupied(to));
        }
        if let Some(stack) = self.zones.remove(&from) {
            self.zones.insert(to, stack);
        }
        Ok(())
    }

    /// Make two windows trade places.
    ///
    /// In different zones each window takes the other's seat: its tab position and
    /// whether that seat is the visible one. So swapping the two visible windows of
    /// two zones leaves both zones showing what the user expects, whatever the stack
    /// depths. In the same zone only the tab order changes and the visible window
    /// stays the visible window.
    pub fn swap(&mut self, a: WindowId, b: WindowId) -> Result<(), StackError> {
        if a == b {
            return Err(StackError::SameWindow);
        }
        let za = self.zone_of(a).ok_or(StackError::NotTracked(a))?;
        let zb = self.zone_of(b).ok_or(StackError::NotTracked(b))?;
        if za == zb {
            if let Some(stack) = self.zones.get_mut(&za) {
                let ia = stack.tabs.iter().position(|&w| w == a);
                let ib = stack.tabs.iter().position(|&w| w == b);
                if let (Some(ia), Some(ib)) = (ia, ib) {
                    stack.tabs.swap(ia, ib);
                    if stack.front == ia {
                        stack.front = ib;
                    } else if stack.front == ib {
                        stack.front = ia;
                    }
                }
            }
            return Ok(());
        }
        let ia = self.tab_index(a).ok_or(StackError::NotTracked(a))?;
        let ib = self.tab_index(b).ok_or(StackError::NotTracked(b))?;
        if let Some(stack) = self.zones.get_mut(&za) {
            stack.tabs[ia] = b;
        }
        if let Some(stack) = self.zones.get_mut(&zb) {
            stack.tabs[ib] = a;
        }
        Ok(())
    }

    /// Forget every zone `keep` rejects (a layout edit removed it) and return the
    /// windows that were in them, by ascending zone id and tab order.
    pub fn retain_zones(&mut self, mut keep: impl FnMut(u32) -> bool) -> Vec<WindowId> {
        let mut dropped = Vec::new();
        self.zones.retain(|&zone, stack| {
            if keep(zone) {
                true
            } else {
                dropped.extend_from_slice(&stack.tabs);
                false
            }
        });
        dropped
    }

    /// Apply one [`StackChange`]. Nothing changes when an error is returned.
    pub fn apply(&mut self, change: &StackChange) -> Result<(), StackError> {
        match *change {
            StackChange::Place { window, zone } => self.push(zone, window),
            StackChange::Remove { window } => self
                .remove(window)
                .map(|_| ())
                .ok_or(StackError::NotTracked(window)),
            StackChange::Raise { window } => {
                if self.contains(window) {
                    self.bring_to_front(window);
                    Ok(())
                } else {
                    Err(StackError::NotTracked(window))
                }
            }
            StackChange::MoveStack { from, to } => self.move_stack(from, to),
            StackChange::Swap { a, b } => self.swap(a, b),
        }
    }

    /// Verify the invariants listed in the module docs. Every operation keeps them,
    /// so this only fails if there is a bug; tests and debug builds call it.
    pub fn check_invariants(&self) -> Result<(), &'static str> {
        let mut seen = std::collections::HashSet::new();
        let mut total = 0usize;
        for stack in self.zones.values() {
            if stack.tabs.is_empty() {
                return Err("a stored stack is empty");
            }
            if stack.tabs.len() > MAX_STACK_DEPTH {
                return Err("a stack is deeper than the maximum");
            }
            if stack.front >= stack.tabs.len() {
                return Err("the visible window is not a member of its stack");
            }
            for &w in &stack.tabs {
                if !seen.insert(w) {
                    return Err("a window is in more than one place");
                }
            }
            total += stack.tabs.len();
        }
        if total > MAX_TRACKED_WINDOWS {
            return Err("more windows than the maximum are tracked");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tiny deterministic generator so the stress tests need no dependency.
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0 >> 33
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    fn stacks(items: &[(WindowId, u32)]) -> Stacks {
        Stacks::from_assignments(items.iter().copied())
    }

    #[test]
    fn a_new_value_is_empty() {
        let s = Stacks::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
        assert_eq!(s.zone_of(1), None);
        assert!(!s.contains(1));
        assert_eq!(s.visible(1), None);
        assert!(s.members(1).is_empty());
        assert_eq!(s.depth(1), 0);
        assert_eq!(s.tab_index(1), None);
        assert!(!s.is_visible(1));
        assert_eq!(s.hidden(1).count(), 0);
        assert!(s.assignments().is_empty());
        assert_eq!(s.occupied_zones().count(), 0);
        assert_eq!(s.check_invariants(), Ok(()));
    }

    #[test]
    fn the_first_window_is_visible_and_the_newest_is_on_top() {
        let mut s = Stacks::new();
        s.push(1, 10).unwrap();
        assert_eq!(s.visible(1), Some(10));
        assert!(s.is_visible(10));
        s.push(1, 11).unwrap();
        s.push(1, 12).unwrap();
        assert_eq!(s.members(1), &[10, 11, 12]);
        assert_eq!(s.visible(1), Some(12));
        assert_eq!(s.hidden(1).collect::<Vec<_>>(), vec![10, 11]);
        assert!(!s.is_visible(10));
        assert_eq!(s.tab_index(11), Some(1));
        assert_eq!(s.depth(1), 3);
        assert_eq!(s.len(), 3);
        assert_eq!(s.check_invariants(), Ok(()));
    }

    #[test]
    fn pushing_a_window_into_its_own_zone_only_raises_it() {
        let mut s = stacks(&[(1, 5), (2, 5), (3, 5)]);
        assert_eq!(s.visible(5), Some(3));
        s.push(5, 1).unwrap();
        assert_eq!(s.members(5), &[1, 2, 3], "no duplicate, order kept");
        assert_eq!(s.visible(5), Some(1));
        assert_eq!(s.check_invariants(), Ok(()));
    }

    #[test]
    fn pushing_moves_a_window_out_of_its_old_zone() {
        let mut s = stacks(&[(1, 1), (2, 2)]);
        s.push(2, 1).unwrap();
        assert_eq!(s.zone_of(1), Some(2));
        assert_eq!(s.members(2), &[2, 1]);
        assert_eq!(s.visible(2), Some(1));
        assert_eq!(s.depth(1), 0, "the emptied zone is gone");
        assert_eq!(s.occupied_zones().collect::<Vec<_>>(), vec![2]);
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn a_window_leaving_a_stack_keeps_the_visible_one_visible() {
        let mut s = stacks(&[(1, 1), (2, 1), (3, 1)]);
        s.bring_to_front(2);
        // Remove a tab before the visible one.
        let r = s.remove(1).unwrap();
        assert_eq!(
            r,
            Removed {
                zone: 1,
                emptied: false,
                new_visible: None
            }
        );
        assert_eq!(s.visible(1), Some(2));
        assert_eq!(s.members(1), &[2, 3]);
        // Remove a tab after the visible one.
        let r = s.remove(3).unwrap();
        assert_eq!(r.new_visible, None);
        assert_eq!(s.visible(1), Some(2));
        assert_eq!(s.check_invariants(), Ok(()));
    }

    #[test]
    fn removing_the_visible_window_shows_the_next_tab() {
        let mut s = stacks(&[(1, 1), (2, 1), (3, 1)]);
        s.bring_to_front(2);
        let r = s.remove(2).unwrap();
        assert_eq!(r.new_visible, Some(3));
        assert_eq!(s.visible(1), Some(3));
        assert_eq!(s.members(1), &[1, 3]);
    }

    #[test]
    fn removing_the_visible_last_tab_shows_the_previous_one() {
        let mut s = stacks(&[(1, 1), (2, 1), (3, 1)]);
        assert_eq!(s.visible(1), Some(3));
        let r = s.remove(3).unwrap();
        assert_eq!(r.new_visible, Some(2));
        assert_eq!(s.visible(1), Some(2));
        assert!(s.is_visible(2));
    }

    #[test]
    fn removing_the_only_window_empties_the_zone() {
        let mut s = stacks(&[(1, 4)]);
        let r = s.remove(1).unwrap();
        assert_eq!(
            r,
            Removed {
                zone: 4,
                emptied: true,
                new_visible: None
            }
        );
        assert!(s.is_empty());
        assert_eq!(s.visible(4), None);
        assert_eq!(s.remove(1), None, "second removal finds nothing");
        assert_eq!(s.remove(99), None);
    }

    #[test]
    fn cycling_wraps_in_both_directions() {
        let mut s = stacks(&[(1, 1), (2, 1), (3, 1)]);
        s.bring_to_front(1);
        assert_eq!(s.cycle(1, CycleDirection::Next), Some(2));
        assert_eq!(s.cycle(1, CycleDirection::Next), Some(3));
        assert_eq!(s.cycle(1, CycleDirection::Next), Some(1));
        assert_eq!(s.cycle(1, CycleDirection::Prev), Some(3));
        assert_eq!(s.cycle(1, CycleDirection::Prev), Some(2));
        assert_eq!(s.visible(1), Some(2));
        assert_eq!(s.members(1), &[1, 2, 3], "cycling never reorders");
    }

    #[test]
    fn cycling_needs_at_least_two_windows() {
        let mut s = stacks(&[(1, 1)]);
        assert_eq!(s.cycle(1, CycleDirection::Next), None);
        assert_eq!(s.cycle(1, CycleDirection::Prev), None);
        assert_eq!(s.cycle(9, CycleDirection::Next), None, "unknown zone");
        assert_eq!(s.visible(1), Some(1));
    }

    #[test]
    fn bring_to_front_reports_whether_anything_changed() {
        let mut s = stacks(&[(1, 1), (2, 1)]);
        assert!(s.bring_to_front(1));
        assert!(!s.bring_to_front(1), "already visible");
        assert!(!s.bring_to_front(77), "untracked");
        assert_eq!(s.visible(1), Some(1));
    }

    #[test]
    fn swapping_the_visible_windows_of_two_zones() {
        let mut s = stacks(&[(1, 1), (2, 2)]);
        s.swap(1, 2).unwrap();
        assert_eq!(s.zone_of(1), Some(2));
        assert_eq!(s.zone_of(2), Some(1));
        assert!(s.is_visible(1) && s.is_visible(2));
        assert_eq!(s.check_invariants(), Ok(()));
    }

    #[test]
    fn swapping_across_stacks_trades_seats() {
        // Zone 1: [a, b] with b on top; zone 2: [c, d] with c on top.
        let mut s = stacks(&[(10, 1), (11, 1), (20, 2), (21, 2)]);
        s.bring_to_front(20);
        assert_eq!(s.visible(1), Some(11));
        assert_eq!(s.visible(2), Some(20));
        s.swap(11, 20).unwrap();
        assert_eq!(s.members(1), &[10, 20]);
        assert_eq!(s.members(2), &[11, 21]);
        assert_eq!(
            s.visible(1),
            Some(20),
            "the seat that was on top stays on top"
        );
        assert_eq!(s.visible(2), Some(11));
        assert_eq!(s.check_invariants(), Ok(()));
    }

    #[test]
    fn swapping_in_the_same_zone_reorders_the_tabs_and_keeps_the_visible_window() {
        let mut s = stacks(&[(1, 1), (2, 1), (3, 1)]);
        s.bring_to_front(1);
        s.swap(1, 3).unwrap();
        assert_eq!(s.members(1), &[3, 2, 1]);
        assert_eq!(s.visible(1), Some(1), "window 1 is still the visible one");
        s.swap(2, 3).unwrap();
        assert_eq!(s.members(1), &[2, 3, 1]);
        assert_eq!(s.visible(1), Some(1));
    }

    #[test]
    fn swap_rejects_bad_input_and_changes_nothing() {
        let mut s = stacks(&[(1, 1), (2, 2)]);
        let before = s.clone();
        assert_eq!(s.swap(1, 1), Err(StackError::SameWindow));
        assert_eq!(s.swap(1, 9), Err(StackError::NotTracked(9)));
        assert_eq!(s.swap(9, 1), Err(StackError::NotTracked(9)));
        assert_eq!(s, before);
    }

    #[test]
    fn moving_a_whole_stack() {
        let mut s = stacks(&[(1, 1), (2, 1), (3, 2)]);
        s.bring_to_front(1);
        s.move_stack(1, 5).unwrap();
        assert_eq!(s.members(5), &[1, 2]);
        assert_eq!(s.visible(5), Some(1), "the visible window travels along");
        assert_eq!(s.depth(1), 0);
        assert_eq!(s.move_stack(5, 5), Ok(()));
        assert_eq!(s.move_stack(1, 6), Err(StackError::ZoneEmpty(1)));
        assert_eq!(s.move_stack(5, 2), Err(StackError::ZoneOccupied(2)));
        assert_eq!(s.move_stack(8, 8), Err(StackError::ZoneEmpty(8)));
        assert_eq!(s.check_invariants(), Ok(()));
    }

    #[test]
    fn a_stack_cannot_grow_past_the_maximum() {
        let mut s = Stacks::new();
        for w in 0..MAX_STACK_DEPTH as u64 {
            s.push(1, w).unwrap();
        }
        assert_eq!(s.depth(1), MAX_STACK_DEPTH);
        let before = s.clone();
        assert_eq!(
            s.push(1, 1000),
            Err(StackError::StackFull(1)),
            "a new window is refused"
        );
        assert_eq!(s, before);
    }

    #[test]
    fn a_refused_move_does_not_lose_the_window() {
        let mut s = Stacks::new();
        for w in 0..MAX_STACK_DEPTH as u64 {
            s.push(1, w).unwrap();
        }
        s.push(2, 500).unwrap();
        let before = s.clone();
        assert_eq!(s.push(1, 500), Err(StackError::StackFull(1)));
        assert_eq!(s, before, "window 500 is still in zone 2");
        assert_eq!(s.zone_of(500), Some(2));
    }

    #[test]
    fn tracking_stops_at_the_global_limit_but_moves_still_work() {
        let mut s = Stacks::new();
        for w in 0..MAX_TRACKED_WINDOWS as u64 {
            // 32 per zone, so 32 zones fill it.
            s.push((w / MAX_STACK_DEPTH as u64) as u32, w).unwrap();
        }
        assert_eq!(s.len(), MAX_TRACKED_WINDOWS);
        assert_eq!(s.push(60, 99_999), Err(StackError::TooManyWindows));
        assert_eq!(s.len(), MAX_TRACKED_WINDOWS);
        // An already tracked window can still move to a zone with room.
        s.push(60, 0).unwrap();
        assert_eq!(s.zone_of(0), Some(60));
        assert_eq!(s.check_invariants(), Ok(()));
    }

    #[test]
    fn from_assignments_groups_by_zone_and_last_one_wins() {
        let s = Stacks::from_assignments([(1, 3), (2, 1), (3, 3), (1, 1)]);
        assert_eq!(s.members(3), &[3]);
        assert_eq!(
            s.members(1),
            &[2, 1],
            "window 1 moved to where it was listed last"
        );
        assert_eq!(s.visible(1), Some(1));
        assert_eq!(s.check_invariants(), Ok(()));
        let empty = Stacks::from_assignments(std::iter::empty());
        assert!(empty.is_empty());
    }

    #[test]
    fn from_assignments_drops_entries_over_the_limits() {
        let s = Stacks::from_assignments((0..100u64).map(|w| (w, 1)));
        assert_eq!(s.depth(1), MAX_STACK_DEPTH);
        let s = Stacks::from_assignments((0..2000u64).map(|w| (w, (w % 64) as u32)));
        assert!(s.len() <= MAX_TRACKED_WINDOWS);
        assert_eq!(s.check_invariants(), Ok(()));
    }

    #[test]
    fn assignments_come_out_by_zone_then_tab_order() {
        let s = stacks(&[(7, 9), (1, 2), (5, 9), (3, 2)]);
        assert_eq!(s.assignments(), vec![(1, 2), (3, 2), (7, 9), (5, 9)]);
        assert_eq!(
            Stacks::from_assignments(s.assignments()).assignments(),
            s.assignments()
        );
    }

    #[test]
    fn retain_zones_drops_and_reports_the_windows_of_removed_zones() {
        let mut s = stacks(&[(1, 1), (2, 2), (3, 2), (4, 3)]);
        let dropped = s.retain_zones(|z| z != 2);
        assert_eq!(dropped, vec![2, 3]);
        assert_eq!(s.assignments(), vec![(1, 1), (4, 3)]);
        assert!(s.retain_zones(|_| true).is_empty());
        let all = s.retain_zones(|_| false);
        assert_eq!(all, vec![1, 4]);
        assert!(s.is_empty());
    }

    #[test]
    fn apply_covers_every_change() {
        let mut s = Stacks::new();
        s.apply(&StackChange::Place { window: 1, zone: 1 }).unwrap();
        s.apply(&StackChange::Place { window: 2, zone: 1 }).unwrap();
        s.apply(&StackChange::Raise { window: 1 }).unwrap();
        assert_eq!(s.visible(1), Some(1));
        s.apply(&StackChange::MoveStack { from: 1, to: 4 }).unwrap();
        s.apply(&StackChange::Place { window: 3, zone: 1 }).unwrap();
        s.apply(&StackChange::Swap { a: 3, b: 2 }).unwrap();
        assert_eq!(s.zone_of(3), Some(4));
        assert_eq!(s.zone_of(2), Some(1));
        s.apply(&StackChange::Remove { window: 2 }).unwrap();
        assert!(!s.contains(2));
        assert_eq!(
            s.apply(&StackChange::Remove { window: 2 }),
            Err(StackError::NotTracked(2))
        );
        assert_eq!(
            s.apply(&StackChange::Raise { window: 2 }),
            Err(StackError::NotTracked(2))
        );
        assert_eq!(
            s.apply(&StackChange::MoveStack { from: 9, to: 10 }),
            Err(StackError::ZoneEmpty(9))
        );
        assert_eq!(s.check_invariants(), Ok(()));
    }

    #[test]
    fn errors_have_readable_messages() {
        for e in [
            StackError::NotTracked(1),
            StackError::SameWindow,
            StackError::ZoneEmpty(1),
            StackError::ZoneOccupied(1),
            StackError::StackFull(1),
            StackError::TooManyWindows,
        ] {
            assert!(!e.to_string().is_empty());
        }
    }

    #[test]
    fn check_invariants_catches_a_corrupted_value() {
        let mut s = stacks(&[(1, 1)]);
        s.zones.get_mut(&1).unwrap().front = 4;
        assert!(s.check_invariants().is_err());
        let mut s = stacks(&[(1, 1), (2, 2)]);
        s.zones.get_mut(&2).unwrap().tabs[0] = 1;
        assert!(s.check_invariants().is_err());
        let mut s = stacks(&[(1, 1)]);
        s.zones.get_mut(&1).unwrap().tabs.clear();
        assert!(s.check_invariants().is_err());
    }

    #[test]
    fn random_operations_never_break_the_invariants() {
        for seed in 0..20u64 {
            let mut rng = Lcg(seed + 1);
            let mut s = Stacks::new();
            for _ in 0..400 {
                let w = rng.below(24);
                let z = rng.below(6) as u32;
                match rng.below(7) {
                    0 | 1 => {
                        let _ = s.push(z, w);
                    }
                    2 => {
                        s.remove(w);
                    }
                    3 => {
                        let dir = if rng.below(2) == 0 {
                            CycleDirection::Next
                        } else {
                            CycleDirection::Prev
                        };
                        s.cycle(z, dir);
                    }
                    4 => {
                        s.bring_to_front(w);
                    }
                    5 => {
                        let _ = s.swap(w, rng.below(24));
                    }
                    _ => {
                        let _ = s.move_stack(z, rng.below(6) as u32);
                    }
                }
                assert_eq!(s.check_invariants(), Ok(()), "seed {seed}");
                // Every zone that exists has a visible window that is one of its members.
                for zone in s.occupied_zones() {
                    let v = s.visible(zone).expect("occupied zone has a visible window");
                    assert!(s.members(zone).contains(&v));
                }
                // A window is in at most one place.
                let all = s.assignments();
                let mut ids: Vec<_> = all.iter().map(|(w, _)| *w).collect();
                ids.sort_unstable();
                ids.dedup();
                assert_eq!(ids.len(), all.len(), "seed {seed}");
            }
        }
    }

    #[test]
    fn the_same_operations_give_the_same_value() {
        let build = || {
            let mut s = Stacks::new();
            for (z, w) in [(3, 30), (1, 10), (3, 31), (2, 20), (1, 11)] {
                s.push(z, w).unwrap();
            }
            s.cycle(3, CycleDirection::Next);
            s.remove(20);
            s
        };
        assert_eq!(build(), build());
    }

    #[test]
    fn zone_insertion_order_does_not_matter() {
        let a = stacks(&[(1, 1), (2, 2), (3, 3)]);
        let b = stacks(&[(3, 3), (1, 1), (2, 2)]);
        assert_eq!(a, b);
        assert_eq!(a.assignments(), b.assignments());
    }
}
