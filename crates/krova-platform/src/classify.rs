//! What kind of window is this? The OS-independent half of the window classifier.
//!
//! The platform backend reads a handful of plain facts about a window
//! ([`WindowFacts`]: style bits, owner, class family, cloaking, elevation) and
//! [`classify`] turns them into a [`WindowKind`]. Keeping the decision here means it
//! is unit-tested on every OS and the same for every feature that needs it
//! (auto-arrange, workspaces, the window highlight, focus dimming). Nothing in the
//! facts is a title or anything the user typed.

use krova_core::app_rules::WindowKind;

/// Which family a window class belongs to, as far as classification cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClassFamily {
    /// Any other class.
    #[default]
    Other,
    /// The standard dialog class (`#32770` on Windows).
    Dialog,
    /// The desktop, the taskbar and other parts of the shell.
    Shell,
}

/// Plain facts about a top-level window, read by the platform layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WindowFacts {
    /// A child window (not top level at all).
    pub child: bool,
    pub visible: bool,
    /// Hidden by the shell: another virtual desktop, or a store app still starting.
    pub cloaked: bool,
    /// Has a title bar (`WS_CAPTION`).
    pub caption: bool,
    /// Has a sizing border (`WS_THICKFRAME`).
    pub sizing_border: bool,
    /// Has a minimize or maximize button.
    pub min_max_buttons: bool,
    /// A tool window (`WS_EX_TOOLWINDOW`): palettes, floating toolbars.
    pub tool_window: bool,
    /// Forced onto the taskbar (`WS_EX_APPWINDOW`).
    pub app_window: bool,
    /// Never takes the focus (`WS_EX_NOACTIVATE`): toasts, on-screen displays.
    pub no_activate: bool,
    /// Owned by another window (menus, dialogs, pop-ups).
    pub owned: bool,
    pub class: ClassFamily,
    /// Covers its whole monitor exactly, without a title bar.
    pub covers_monitor: bool,
    /// Its process is elevated and Krova is not, so it cannot be moved.
    pub elevated: bool,
    /// One of Krova's own windows.
    pub own_process: bool,
}

/// Classify a window from its facts. Only [`WindowKind::Normal`] windows are ever
/// arranged; every other kind says why not.
pub fn classify(f: &WindowFacts) -> WindowKind {
    if f.class == ClassFamily::Shell {
        return WindowKind::Shell;
    }
    if f.child || f.own_process || !f.visible {
        return WindowKind::Unknown;
    }
    if f.cloaked {
        return WindowKind::Cloaked;
    }
    if f.tool_window && !f.app_window {
        return WindowKind::Tool;
    }
    if f.class == ClassFamily::Dialog {
        return WindowKind::Dialog;
    }
    if f.owned && !f.app_window {
        // An owned window with a title bar is a dialog (a find box, a properties
        // sheet); without one it is a menu, a drop-down or a pop-up.
        return if f.caption {
            WindowKind::Dialog
        } else {
            WindowKind::OwnedPopup
        };
    }
    if f.no_activate {
        return WindowKind::Notification;
    }
    if !f.caption {
        if f.covers_monitor {
            return WindowKind::Fullscreen;
        }
        // Apps that draw their own title bar keep a sizing border and the
        // minimize / maximize buttons; a borderless box without them is a splash
        // screen or a loading window.
        if !(f.sizing_border && f.min_max_buttons) {
            return WindowKind::Splash;
        }
    }
    if f.elevated {
        return WindowKind::Elevated;
    }
    WindowKind::Normal
}

#[cfg(test)]
mod tests {
    use super::*;

    fn normal() -> WindowFacts {
        WindowFacts {
            visible: true,
            caption: true,
            sizing_border: true,
            min_max_buttons: true,
            ..WindowFacts::default()
        }
    }

    #[test]
    fn an_ordinary_app_window_is_normal() {
        assert_eq!(classify(&normal()), WindowKind::Normal);
        // A custom title bar (no caption) that can still be sized and maximized.
        let custom = WindowFacts {
            caption: false,
            ..normal()
        };
        assert_eq!(classify(&custom), WindowKind::Normal);
        // An app window that is owned but forced onto the taskbar.
        let owned_app = WindowFacts {
            owned: true,
            app_window: true,
            ..normal()
        };
        assert_eq!(classify(&owned_app), WindowKind::Normal);
    }

    #[test]
    fn dialogs_tools_and_popups_are_told_apart() {
        let dialog_class = WindowFacts {
            class: ClassFamily::Dialog,
            ..normal()
        };
        assert_eq!(classify(&dialog_class), WindowKind::Dialog);
        let owned_captioned = WindowFacts {
            owned: true,
            ..normal()
        };
        assert_eq!(classify(&owned_captioned), WindowKind::Dialog);
        let owned_bare = WindowFacts {
            owned: true,
            caption: false,
            ..normal()
        };
        assert_eq!(classify(&owned_bare), WindowKind::OwnedPopup);
        let tool = WindowFacts {
            tool_window: true,
            ..normal()
        };
        assert_eq!(classify(&tool), WindowKind::Tool);
        let toast = WindowFacts {
            no_activate: true,
            ..normal()
        };
        assert_eq!(classify(&toast), WindowKind::Notification);
    }

    #[test]
    fn shell_cloaked_hidden_and_own_windows_are_never_normal() {
        let shell = WindowFacts {
            class: ClassFamily::Shell,
            ..normal()
        };
        assert_eq!(classify(&shell), WindowKind::Shell);
        let cloaked = WindowFacts {
            cloaked: true,
            ..normal()
        };
        assert_eq!(classify(&cloaked), WindowKind::Cloaked);
        for f in [
            WindowFacts {
                visible: false,
                ..normal()
            },
            WindowFacts {
                child: true,
                ..normal()
            },
            WindowFacts {
                own_process: true,
                ..normal()
            },
        ] {
            assert_eq!(classify(&f), WindowKind::Unknown, "{f:?}");
        }
    }

    #[test]
    fn borderless_windows_are_fullscreen_or_splash() {
        let game = WindowFacts {
            caption: false,
            sizing_border: false,
            min_max_buttons: false,
            covers_monitor: true,
            ..normal()
        };
        assert_eq!(classify(&game), WindowKind::Fullscreen);
        let splash = WindowFacts {
            covers_monitor: false,
            ..game
        };
        assert_eq!(classify(&splash), WindowKind::Splash);
    }

    #[test]
    fn an_elevated_window_is_reported_as_such() {
        let admin = WindowFacts {
            elevated: true,
            ..normal()
        };
        assert_eq!(classify(&admin), WindowKind::Elevated);
        // A dialog stays a dialog whoever runs it.
        let admin_dialog = WindowFacts {
            class: ClassFamily::Dialog,
            ..admin
        };
        assert_eq!(classify(&admin_dialog), WindowKind::Dialog);
    }
}
