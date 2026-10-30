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

export interface AppState {
  config: Config;
  monitors: MonitorSnapshot[];
  autostart: boolean;
  permission: Permission;
  version: string;
}

/**
 * Divider direction. `vertical` = a vertical divider, zones end up side by side
 * (positions are x values); `horizontal` = zones stacked (positions are y).
 */
export type Axis = 'vertical' | 'horizontal';

/** Payload of the backend "navigate" event. */
export type View = 'editor' | 'settings';
