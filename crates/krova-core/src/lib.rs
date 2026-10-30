//! Krova core: OS-independent zone geometry, layouts, monitor identity,
//! edge-snap rules and config persistence.
#![forbid(unsafe_code)]

pub mod config;
pub mod edge;
pub mod edit;
pub mod geometry;
pub mod monitor;
pub mod snap;
pub mod window;
pub mod zone;

pub use config::{
    Config, ConfigError, LoadStatus, Modifier, MonitorProfile, MoveMode, ProfileError, Settings,
    Theme,
};
pub use edge::{edge_snap, EdgeSnap};
pub use edit::{Axis, EditError};
pub use geometry::{PxRect, Rect};
pub use monitor::{match_monitors, MonitorDescriptor};
pub use snap::{Action, DragEvent, MonitorView, Session, SnapSettings, SnapTarget};
pub use window::{animate_step, clamp_to_limits, FrameInsets, SizeLimits};
pub use zone::{templates, Layout, LayoutError, Zone};
