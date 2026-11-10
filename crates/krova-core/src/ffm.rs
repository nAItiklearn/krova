//! Focus follows mouse and the hover zone picker: the pure decisions.
//!
//! The platform layer watches the cursor. When it stops moving, one one-shot
//! wait runs per enabled feature (see [`rest_waits`]); when a wait ends without
//! the cursor moving again, the platform describes what is under the cursor in an
//! [`Observation`] and [`decide`] says whether to activate that window. Nothing
//! runs while the cursor is still, so the feature costs no CPU when idle.

/// Shortest and longest delay before the window under a resting cursor is
/// activated, in milliseconds.
pub const MIN_DELAY_MS: i32 = 150;
pub const MAX_DELAY_MS: i32 = 800;
pub const DEFAULT_DELAY_MS: i32 = 350;
/// How long the cursor rests on a title bar, with the modifier held, before the
/// hover zone picker opens.
pub const HOVER_DELAY_MS: u32 = 250;

/// The delay setting forced into `MIN_DELAY_MS..=MAX_DELAY_MS`.
pub fn clamp_delay(ms: i32) -> i32 {
    ms.clamp(MIN_DELAY_MS, MAX_DELAY_MS)
}

/// What a finished wait asks the platform to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestAction {
    /// Look for a title bar under the cursor (hover zone picker).
    Hover,
    /// Activate the window under the cursor (focus follows mouse).
    Focus,
}

/// The waits after the cursor stops, as `(ms after it stopped, action)`, earliest
/// first. Empty when neither feature is on: the platform then ignores movement.
pub fn rest_waits(focus_delay_ms: Option<i32>, hover: bool) -> Vec<(u32, RestAction)> {
    let mut out = Vec::with_capacity(2);
    if hover {
        out.push((HOVER_DELAY_MS, RestAction::Hover));
    }
    if let Some(ms) = focus_delay_ms {
        // Clamped to 150..=800 first, so the cast is exact.
        out.push((clamp_delay(ms) as u32, RestAction::Focus));
    }
    out.sort_by_key(|&(at, action)| (at, action == RestAction::Focus));
    out
}

/// Everything the platform found out about the cursor's resting place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Observation {
    /// There is a top-level window under the cursor.
    pub has_window: bool,
    /// A mouse button is held right now.
    pub button_down: bool,
    /// A button was pressed or released while the cursor was coming to rest
    /// (the click already decided the focus).
    pub clicked_during_rest: bool,
    /// An ordinary, visible, captioned application window (not the desktop, the
    /// taskbar, a tool window or one that refuses activation).
    pub arrangeable: bool,
    /// Belongs to Krova itself.
    pub own_window: bool,
    /// A menu or popup is open, or a move/size loop is running.
    pub menu_open: bool,
    /// The window runs elevated (Krova cannot and must not activate it).
    pub elevated: bool,
    /// A full-screen app (game, video, presentation) is in front.
    pub fullscreen_in_front: bool,
    /// The window already has the focus.
    pub already_foreground: bool,
    /// The OS refused to activate this same window last time; trying again would
    /// only flash its taskbar button again.
    pub refused_before: bool,
}

/// Why the window under a resting cursor was left alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skip {
    NoWindow,
    ButtonDown,
    Clicked,
    NotArrangeable,
    OwnWindow,
    MenuOpen,
    Elevated,
    Fullscreen,
    AlreadyFocused,
    RefusedBefore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Focus,
    Skip(Skip),
}

/// Activate the window under the cursor, or say why not. The order matters only
/// for which reason is reported; any single reason is enough to skip.
pub fn decide(o: &Observation) -> Decision {
    let skip = if !o.has_window {
        Some(Skip::NoWindow)
    } else if o.button_down {
        Some(Skip::ButtonDown)
    } else if o.clicked_during_rest {
        Some(Skip::Clicked)
    } else if o.fullscreen_in_front {
        Some(Skip::Fullscreen)
    } else if o.menu_open {
        Some(Skip::MenuOpen)
    } else if o.own_window {
        Some(Skip::OwnWindow)
    } else if !o.arrangeable {
        Some(Skip::NotArrangeable)
    } else if o.elevated {
        Some(Skip::Elevated)
    } else if o.already_foreground {
        Some(Skip::AlreadyFocused)
    } else if o.refused_before {
        Some(Skip::RefusedBefore)
    } else {
        None
    };
    skip.map_or(Decision::Focus, Decision::Skip)
}

/// Whether the hover zone picker should open for the window under the cursor:
/// the feature is on, the activation modifier is held, no button is down, the
/// cursor is on the window's title bar and Krova may move the window.
pub fn hover_wanted(
    enabled: bool,
    modifier_down: bool,
    button_down: bool,
    on_caption: bool,
    movable: bool,
) -> bool {
    enabled && modifier_down && !button_down && on_caption && movable
}

#[cfg(test)]
mod tests {
    use super::*;

    fn focusable() -> Observation {
        Observation {
            has_window: true,
            arrangeable: true,
            ..Observation::default()
        }
    }

    #[test]
    fn delay_is_clamped() {
        assert_eq!(clamp_delay(0), 150);
        assert_eq!(clamp_delay(-5), 150);
        assert_eq!(clamp_delay(350), 350);
        assert_eq!(clamp_delay(5000), 800);
    }

    #[test]
    fn waits_are_one_shot_per_enabled_feature_and_ordered() {
        assert!(rest_waits(None, false).is_empty());
        assert_eq!(rest_waits(Some(350), false), [(350, RestAction::Focus)]);
        assert_eq!(rest_waits(None, true), [(250, RestAction::Hover)]);
        assert_eq!(
            rest_waits(Some(800), true),
            [(250, RestAction::Hover), (800, RestAction::Focus)]
        );
        // A short focus delay comes first; an out-of-range one is clamped.
        assert_eq!(
            rest_waits(Some(10), true),
            [(150, RestAction::Focus), (250, RestAction::Hover)]
        );
        // Equal times: the picker is looked at before the focus moves.
        assert_eq!(
            rest_waits(Some(250), true),
            [(250, RestAction::Hover), (250, RestAction::Focus)]
        );
    }

    #[test]
    fn an_ordinary_window_is_focused() {
        assert_eq!(decide(&focusable()), Decision::Focus);
    }

    #[test]
    fn every_exclusion_skips() {
        type Change = fn(&mut Observation);
        let cases: [(Change, Skip); 10] = [
            (|o| o.has_window = false, Skip::NoWindow),
            (|o| o.button_down = true, Skip::ButtonDown),
            (|o| o.clicked_during_rest = true, Skip::Clicked),
            (|o| o.arrangeable = false, Skip::NotArrangeable),
            (|o| o.own_window = true, Skip::OwnWindow),
            (|o| o.menu_open = true, Skip::MenuOpen),
            (|o| o.elevated = true, Skip::Elevated),
            (|o| o.fullscreen_in_front = true, Skip::Fullscreen),
            (|o| o.already_foreground = true, Skip::AlreadyFocused),
            (|o| o.refused_before = true, Skip::RefusedBefore),
        ];
        for (change, why) in cases {
            let mut o = focusable();
            change(&mut o);
            assert_eq!(decide(&o), Decision::Skip(why), "{o:?}");
        }
    }

    #[test]
    fn a_held_button_wins_over_everything_else() {
        let o = Observation {
            button_down: true,
            elevated: true,
            fullscreen_in_front: true,
            ..focusable()
        };
        assert_eq!(decide(&o), Decision::Skip(Skip::ButtonDown));
    }

    #[test]
    fn the_picker_needs_every_condition() {
        assert!(hover_wanted(true, true, false, true, true));
        assert!(!hover_wanted(false, true, false, true, true));
        assert!(!hover_wanted(true, false, false, true, true));
        assert!(!hover_wanted(true, true, true, true, true));
        assert!(!hover_wanted(true, true, false, false, true));
        assert!(!hover_wanted(true, true, false, true, false));
    }
}
