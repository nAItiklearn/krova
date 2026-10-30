//! OS-independent helpers for the input worker threads.

/// Left-button pointer events as delivered by an OS hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawPointer {
    Down(i32, i32),
    Move(i32, i32),
    Up,
}

/// Collapse every run of consecutive moves into its last one. A mouse hook can
/// deliver hundreds of moves per second; only the newest position matters.
pub fn coalesce_moves(events: &mut Vec<RawPointer>) {
    let mut out: Vec<RawPointer> = Vec::with_capacity(events.len());
    for ev in events.drain(..) {
        let prev_is_move = matches!(out.last(), Some(RawPointer::Move(..)));
        if prev_is_move && matches!(ev, RawPointer::Move(..)) {
            out.pop();
        }
        out.push(ev);
    }
    *events = out;
}

/// Reports a polled boolean (modifier key, Escape) only when it changes.
#[derive(Debug, Default)]
pub struct ChangeDetector {
    last: bool,
}

impl ChangeDetector {
    /// `Some(new_value)` if `now` differs from the last seen value.
    pub fn update(&mut self, now: bool) -> Option<bool> {
        if now == self.last {
            None
        } else {
            self.last = now;
            Some(now)
        }
    }

    /// Adopt `value` as the baseline without reporting a change.
    pub fn reset(&mut self, value: bool) {
        self.last = value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use RawPointer::{Down, Move, Up};

    #[test]
    fn runs_of_moves_collapse_to_the_last() {
        let mut v = vec![Move(1, 1), Move(2, 2), Move(3, 3)];
        coalesce_moves(&mut v);
        assert_eq!(v, vec![Move(3, 3)]);
    }

    #[test]
    fn button_events_keep_their_order_and_split_runs() {
        let mut v = vec![
            Down(0, 0),
            Move(1, 1),
            Move(2, 2),
            Up,
            Move(8, 8),
            Move(9, 9),
        ];
        coalesce_moves(&mut v);
        assert_eq!(v, vec![Down(0, 0), Move(2, 2), Up, Move(9, 9)]);
    }

    #[test]
    fn empty_and_single_event_batches_are_unchanged() {
        let mut v: Vec<RawPointer> = vec![];
        coalesce_moves(&mut v);
        assert!(v.is_empty());
        let mut v = vec![Up];
        coalesce_moves(&mut v);
        assert_eq!(v, vec![Up]);
    }

    #[test]
    fn change_detector_reports_edges_only() {
        let mut d = ChangeDetector::default();
        assert_eq!(d.update(false), None);
        assert_eq!(d.update(true), Some(true));
        assert_eq!(d.update(true), None);
        assert_eq!(d.update(false), Some(false));
    }

    #[test]
    fn reset_adopts_the_baseline_silently() {
        let mut d = ChangeDetector::default();
        d.reset(true);
        assert_eq!(d.update(true), None);
        assert_eq!(d.update(false), Some(false));
    }
}
