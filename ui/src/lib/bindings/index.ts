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
  /** Default `win_alt`. */
  shortcut_modifier: ShortcutModifier;
  /** Snapped windows return to their zones when their monitor reconnects. Default true. */
  restore_on_reconnect: boolean;
  /** Dragging the edge between two snapped windows resizes both. Default true. */
  linked_dividers: boolean;
  /** Space between zones in px, integer 0..32. Default 0. May be inert in early backends. */
  zone_gap_px: number;
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
  /** Human-readable, e.g. "Win+Alt+3 is already used by another app". */
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
  shortcuts: ShortcutStatus;
  capabilities: Capabilities;
}

/**
 * Divider direction. `vertical` = a vertical divider, zones end up side by side
 * (positions are x values); `horizontal` = zones stacked (positions are y).
 */
export type Axis = 'vertical' | 'horizontal';

/** Payload of the backend "navigate" event. */
export type View = 'editor' | 'settings';
