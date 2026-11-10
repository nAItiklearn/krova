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
  /** Per-program rules, in the order the user made them (at most 256). Default []. */
  app_rules: AppRule[];
}

export interface MonitorProfile {
  id: string;
  label: string;
  hardware_id: string | null;
  device_path: string | null;
  last_rect: PxRect;
  layouts: Layout[];
  active_layout: string;
  /** Auto-arrange on this monitor. On by default. Change with `setAutoArrange`. */
  auto_arrange: ArrangeSettings;
  /** The first-run preset was offered to this monitor (internal; never shown). */
  preset_seeded: boolean;
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
  /** Auto-arrange, stacks and tab strips (Windows only for now). */
  auto_arrange: boolean;
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
  /** The program in front and whether auto-arrange looks after it; null if unknown. */
  front_app: FrontApp | null;
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

// ---------------------------------------------------------------- Wave 1: arrange

/** Where a new window goes: the first free zone, or the first zone (others shift). */
export type ArrangePolicy = 'next_free' | 'main_first';

/** Auto-arrange for one monitor (`MonitorProfile.auto_arrange`). On by default. */
export interface ArrangeSettings {
  enabled: boolean;
  policy: ArrangePolicy;
  /** Closing a window lets the later ones move up. Default false. */
  fill_gaps: boolean;
  /** With every zone taken, a new window joins a stack (tabs). Default true. */
  stack_when_full: boolean;
}

/** What an app rule does with a program's windows. */
export type RuleAction =
  | { kind: 'never_touch' }
  | { kind: 'float' }
  /** 1-based zone number in reading order (1..64). */
  | { kind: 'preferred_zone'; value: number }
  /** A `MonitorProfile.id`. */
  | { kind: 'start_on_monitor'; value: string };

/** A rule for one program, by executable file name (`chrome.exe`), never a title. */
export interface AppRule {
  exe: string;
  action: RuleAction;
}

/** Why the front window is not arranged (`FrontApp.reason`). */
export type FrontAppReason =
  | 'dialog'
  | 'tool'
  | 'popup'
  | 'elevated'
  | 'never_touch'
  | 'float'
  | 'auto_arrange_off'
  | 'paused'
  | 'other';

/** The program in front, for the "Not arranged" chip. */
export interface FrontApp {
  exe: string | null;
  arranged: boolean;
  reason: FrontAppReason | null;
}

export type PresetFamily = 'grid' | 'columns' | 'rows' | 'main_side';
export type PresetVariant = 'landscape' | 'ultrawide' | 'portrait';
export type MainSize = 'half' | 'sixty' | 'two_thirds';

/** One entry of `list_presets`: an ordinary layout plus the choices that made it. */
export interface PresetInfo {
  /** e.g. `grid-3`, `main-side-3-ultrawide-60` (also `layout.id`). */
  id: string;
  /** e.g. `Main + side 3 (ultrawide, 60%)` (also `layout.name`). */
  name: string;
  family: PresetFamily;
  /** 1..5 windows. */
  count: number;
  variant: PresetVariant;
  main_size: MainSize;
  /** Made for the shape of the monitor that was asked about. */
  recommended: boolean;
  layout: Layout;
}
