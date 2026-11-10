//! The notch window's OS-independent interface. The geometry, the spring and the
//! drawing are in `krova_core::notch`; the Windows window is `win_notch`. Other
//! platforms report [`SUPPORTED`] = false and [`start_notch`] fails cleanly.

use krova_core::notch::{NotchCorner, NotchSettings};
use krova_core::PxRect;

use crate::{EventSink, PlatformError};

/// Whether this build can show the notch.
pub const SUPPORTED: bool = cfg!(windows);

/// One saved layout of a monitor, as the notch menu lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotchLayout {
    pub id: String,
    pub name: String,
    pub zones: usize,
}

/// A connected monitor, as the notch needs it (physical px).
#[derive(Debug, Clone, PartialEq)]
pub struct NotchMonitorInfo {
    /// `MonitorProfile.id`.
    pub id: String,
    pub rect: PxRect,
    pub work: PxRect,
    /// Device px per logical px from the monitor list; the window re-reads the
    /// monitor's DPI itself and only falls back to this.
    pub scale: f64,
    pub primary: bool,
    pub layouts: Vec<NotchLayout>,
    /// Id of the active layout.
    pub active: String,
}

impl NotchMonitorInfo {
    pub fn active_layout(&self) -> Option<&NotchLayout> {
        self.layouts.iter().find(|l| l.id == self.active)
    }
}

/// Everything the notch shows. Sent whole on every change.
#[derive(Debug, Clone, PartialEq)]
pub struct NotchModel {
    pub settings: NotchSettings,
    pub monitors: Vec<NotchMonitorInfo>,
    /// Krova is paused (`Settings.enabled` is false): grey dot, "Resume" in the menu.
    pub paused: bool,
    /// Accent colour `0xrrggbb` for the dot.
    pub accent: u32,
    /// Corner radius of the expanded pill, logical px.
    pub radius: f64,
}

impl Default for NotchModel {
    fn default() -> Self {
        Self {
            settings: NotchSettings::default(),
            monitors: Vec::new(),
            paused: false,
            accent: krova_core::notch::DEFAULT_ACCENT,
            radius: krova_core::notch::DEFAULT_RADIUS,
        }
    }
}

/// An entry of the notch's right-click menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotchMenu {
    /// Pause or resume Krova.
    TogglePause,
    /// Activate this layout (by id) on the notch's monitor.
    Layout(String),
    /// Open the layout switcher for the notch's monitor.
    Switcher,
    CheatSheet,
    /// Open the editor.
    OpenApp,
    /// Turn the notch off (`Settings.notch.enabled = false`).
    Hide,
}

/// Where the notch is, for anchoring the panel (physical px).
#[derive(Debug, Clone, PartialEq)]
pub struct NotchAnchor {
    pub rect: PxRect,
    pub corner: NotchCorner,
    pub monitor_id: String,
    /// The monitor's work area, so the panel can be kept inside it.
    pub work: PxRect,
    /// Device px per logical px on that monitor.
    pub scale: f64,
}

/// What the notch tells the app. The sink must only enqueue and return.
#[derive(Debug, Clone, PartialEq)]
pub enum NotchEvent {
    /// A left click: toggle the panel next to the notch.
    Clicked(NotchAnchor),
    Menu {
        monitor_id: String,
        item: NotchMenu,
    },
    /// A drag ended and the notch snapped here: save it.
    CornerChanged(NotchCorner),
    /// The displays, the work area or a DPI changed: refresh the monitors and
    /// send a new model.
    DisplayChanged,
}

/// A running notch. Dropping it destroys the window and stops its thread.
pub trait Notch: Send + Sync {
    /// Show the new state (cheap when nothing visible changed).
    fn update(&self, model: NotchModel);
    /// While on, layout names are replaced by zone counts.
    fn set_share_safe(&self, on: bool);
}

/// Create the notch window on its own thread. It shows once given a model with
/// `settings.enabled` and at least one monitor.
pub fn start_notch(sink: EventSink<NotchEvent>) -> Result<Box<dyn Notch>, PlatformError> {
    #[cfg(windows)]
    {
        crate::win_notch::WinNotch::start(sink).map(|n| Box::new(n) as Box<dyn Notch>)
    }
    #[cfg(not(windows))]
    {
        let _ = sink;
        Err(PlatformError::Unsupported)
    }
}

/// The text the expanded notch shows for a monitor.
pub fn label(monitor: &NotchMonitorInfo, paused: bool, share_safe: bool) -> String {
    let base = match monitor.active_layout() {
        Some(l) if share_safe => match l.zones {
            1 => "1 zone".to_owned(),
            n => format!("{n} zones"),
        },
        Some(l) => l.name.clone(),
        None => "No layout".to_owned(),
    };
    if paused {
        format!("Paused \u{b7} {base}")
    } else {
        base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor() -> NotchMonitorInfo {
        NotchMonitorInfo {
            id: "monitor-1".into(),
            rect: PxRect::new(0, 0, 100, 100),
            work: PxRect::new(0, 0, 100, 90),
            scale: 1.0,
            primary: true,
            layouts: vec![
                NotchLayout {
                    id: "halves".into(),
                    name: "Halves".into(),
                    zones: 2,
                },
                NotchLayout {
                    id: "one".into(),
                    name: "Focus".into(),
                    zones: 1,
                },
            ],
            active: "halves".into(),
        }
    }

    #[test]
    fn the_label_is_the_layout_name_unless_share_safe() {
        let mut m = monitor();
        assert_eq!(label(&m, false, false), "Halves");
        assert_eq!(label(&m, false, true), "2 zones");
        assert_eq!(label(&m, true, false), "Paused \u{b7} Halves");
        m.active = "one".into();
        assert_eq!(label(&m, false, true), "1 zone");
        m.active = "gone".into();
        assert_eq!(label(&m, false, false), "No layout");
    }

    #[test]
    fn start_fails_cleanly_where_unsupported() {
        if !SUPPORTED {
            assert!(matches!(
                start_notch(std::sync::Arc::new(|_| {})),
                Err(PlatformError::Unsupported)
            ));
        }
    }
}
