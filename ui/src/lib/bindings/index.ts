// Hand-written mirror of the serde JSON shapes emitted by the Rust side
// (crates/krova-core: geometry.rs, zone.rs, config.rs, plus the app glue's
// AppState / MonitorSnapshot). The Rust build will later replace this file with
// ts-rs output. THIS FILE IS THE ONLY PLACE WHERE CONTRACT TYPES ARE DEFINED;
// keep names and field names identical to the Rust side (snake_case fields).

/** Rectangle in normalized (0..1) monitor work-area coordinates. */
export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Rectangle in physical pixels (integers, virtual-screen coordinates). */
export interface PxRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Zone {
  id: number;
  rect: Rect;
}

export interface Layout {
  id: string;
  name: string;
  zones: Zone[];
}

/** `Super` is the Windows key on Windows and Command on macOS. */
export type Modifier = 'shift' | 'ctrl' | 'alt' | 'super';
export type MoveMode = 'animate' | 'instant';
export type Theme = 'system' | 'light' | 'dark';

/**
 * Keys held together with Left/Right/Up/1..9 for the window-move shortcuts.
 * `win` is the Windows key (Command on macOS, once shortcuts exist there).
 */
export type ShortcutModifier = 'ctrl_alt' | 'win_alt' | 'ctrl_alt_shift' | 'win_ctrl';

// --- corners ---
/** What a hot corner does. Rust: `krova_core::corners::HotCornerAction`. */
export type HotCornerAction =
  | 'switcher'
  | 'cheat_sheet'
  | 'focus'
  | 'calm_desk'
  | 'pause'
  | 'show_zones'
  | 'panel';
export type HotCorner = 'top_left' | 'top_right' | 'bottom_left' | 'bottom_right';

/**
 * `Settings.hot_corners`. Only outer corners count (a corner touching another
 * monitor never fires). Defaults: top_right `switcher`, bottom_right `calm_desk`,
 * others null, dwell 300, ignore_fullscreen true.
 */
export interface HotCornerSettings {
  top_left: HotCornerAction | null;
  top_right: HotCornerAction | null;
  bottom_left: HotCornerAction | null;
  bottom_right: HotCornerAction | null;
  /** Integer ms the cursor must rest in the corner, clamped to 150..1000. */
  dwell_ms: number;
  /** Do nothing while a full-screen app is in front. */
  ignore_fullscreen: boolean;
}

/** Payload of the backend event `hot-corner` (sent after a corner fired). */
export interface HotCornerEvent {
  corner: HotCorner;
  action: HotCornerAction;
  /** `MonitorProfile.id` of the monitor whose corner it was. */
  monitor_id: string | null;
}

/** Returned by `hot_corners_status`. */
export interface HotCornerStatus {
  /** This OS build can watch the screen corners (false on macOS for now). */
  supported: boolean;
  /** Corners are watched now: supported, Krova on, at least one corner set. */
  active: boolean;
}

export interface Settings {
  enabled: boolean;
  modifier: Modifier;
  /** `#rrggbb` */
  highlight_color: string;
  /** 0..1 */
  highlight_opacity: number;
  move_mode: MoveMode;
  edge_snapping: boolean;
  /** integer, 2..64 */
  edge_size_px: number;
  launch_at_login: boolean;
  theme: Theme;
  /** Global keyboard shortcuts that move the focused window between zones. Default true. */
  keyboard_shortcuts: boolean;
  /**
   * Default `ctrl_alt_shift`. Windows refuses most Win+Alt and Win+Ctrl chords, so
   * `shortcuts.failed` says which ones were not registered.
   */
  shortcut_modifier: ShortcutModifier;
  /** Snapped windows return to their zones when their monitor reconnects. Default true. */
  restore_on_reconnect: boolean;
  /** Dragging the edge between two snapped windows resizes both. Default true. */
  linked_dividers: boolean;
  /** Space between zones in px, integer 0..32. Default 0. May be inert in early backends. */
  zone_gap_px: number;
  /**
   * Global `<shortcut_modifier>+Space` opens the layout switcher on the monitor under
   * the cursor. Independent of `keyboard_shortcuts`. Default true.
   */
  layout_switcher_shortcut: boolean;
  // --- corners ---
  /** What each outer screen corner does when the cursor rests in it. */
  hot_corners: HotCornerSettings;
}

export interface MonitorProfile {
  id: string;
  label: string;
  hardware_id: string | null;
  device_path: string | null;
  last_rect: PxRect;
  layouts: Layout[];
  active_layout: string;
}

export interface Config {
  schema_version: number;
  settings: Settings;
  monitors: MonitorProfile[];
}

/** A currently connected monitor (app glue, not persisted). */
export interface MonitorSnapshot {
  /** Id of the matching MonitorProfile in Config.monitors. */
  profile_id: string;
  label: string;
  rect: PxRect;
  /** Work area (excludes taskbar / menu bar / dock). */
  work: PxRect;
  scale: number;
  primary: boolean;
}

/** `denied` means macOS Accessibility access is still needed. */
export type Permission = 'not_required' | 'granted' | 'denied';

/** Whether the global shortcuts are registered, and which ones could not be. */
export interface ShortcutStatus {
  active: boolean;
  /** Human-readable, e.g. "Ctrl+Alt+Shift+3 is already used by another app". */
  failed: string[];
}

/** What this build of Krova can do on this operating system (false on macOS for now). */
export interface Capabilities {
  shortcuts: boolean;
  window_memory: boolean;
}

export interface AppState {
  config: Config;
  monitors: MonitorSnapshot[];
  autostart: boolean;
  permission: Permission;
  version: string;
  /** The twelve shortcuts that move the focused window (`keyboard_shortcuts`). */
  shortcuts: ShortcutStatus;
  capabilities: Capabilities;
  /**
   * The layout switcher's own shortcut (`<shortcut_modifier>+Space`,
   * `layout_switcher_shortcut`): `active` once the OS accepted it, `failed` has at most
   * one sentence, e.g. "Ctrl+Alt+Shift+Space is already used by another app".
   */
  layout_switcher: ShortcutStatus;
}

/**
 * Divider direction. `vertical` = a vertical divider, zones end up side by side
 * (positions are x values); `horizontal` = zones stacked (positions are y).
 */
export type Axis = 'vertical' | 'horizontal';

/** Payload of the backend "navigate" event. */
export type View = 'editor' | 'settings';

/**
 * What a "navigate" event or `take_pending_view` can carry: the two views of the
 * editor window, or `switcher` (the layout switcher, opened by its global shortcut;
 * the monitor it is for comes from `switcher_monitor`). A UI that does not know
 * `switcher` yet must ignore it.
 */
export type NavigateTarget = View | 'switcher';
