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
  // --- wave3 ---
  /** Focus dim. Off by default. */
  focus: FocusSettings;
  calm_desk: CalmDeskSettings;
  /** While on, Krova's overlays show only zone shapes, never app names or icons. Default false. */
  share_safe: boolean;
  /** Window highlight. Mode `none` by default. */
  highlight: HighlightSettings;
  // --- end wave3 ---
  // --- wave4 ---
  /** Activate the window under the cursor once it rests. Default false. */
  focus_follows_mouse: boolean;
  /** How long the cursor rests first, in ms, integer 150..800. Default 350. */
  focus_follows_mouse_delay_ms: number;
  /** The activation modifier held over a title bar opens a zone picker. Default false. */
  hover_picker: boolean;
  /** Language of the native strings (tray, notifications). Default `system`. */
  language: Language;
  // --- end wave4 ---
  // --- notch ---
  /** The notch at a screen edge. Rust fills in defaults field by field. */
  notch: NotchSettings;
  // --- notch ---
}

// --- notch ---
/** Where on its monitor the notch sits. */
export type NotchCorner = 'top_center' | 'top_left' | 'top_right' | 'bottom_left' | 'bottom_right';
/** Which monitor shows the notch: the primary one, or the one under the cursor. */
export type NotchMonitor = 'primary' | 'cursor';

/** `Settings.notch`. Defaults: enabled, `top_center`, `primary`, hide in fullscreen. */
export interface NotchSettings {
  enabled: boolean;
  corner: NotchCorner;
  monitor: NotchMonitor;
  /** Hide the notch while a fullscreen app is in front of its monitor. */
  hide_in_fullscreen: boolean;
}

/**
 * Payload of the backend event `panel-anchor`, sent to the `panel` window when the
 * notch is clicked: the notch's rect in PHYSICAL screen px, its position and its
 * monitor (`MonitorProfile.id`). The panel window has already been placed next to it.
 */
export interface PanelAnchor {
  x: number;
  y: number;
  w: number;
  h: number;
  corner: NotchCorner;
  monitor_id: string;
}
// --- notch ---

export interface MonitorProfile {
  id: string;
  label: string;
  hardware_id: string | null;
  device_path: string | null;
  last_rect: PxRect;
  layouts: Layout[];
  active_layout: string;
  // --- wave4 ---
  /** Edges kept free for third-party docks and bars. Missing in old configs. */
  reserved?: ReservedEdges;
  // --- end wave4 ---
}

export interface Config {
  schema_version: number;
  settings: Settings;
  monitors: MonitorProfile[];
  // --- wave3 ---
  /** Saved workspaces, at most 32. */
  workspaces: Workspace[];
  // --- end wave3 ---
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
  // --- notch ---
  /** This build can show the notch (Windows). Missing in older backends. */
  notch?: boolean;
  // --- notch ---
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
  // --- wave3 ---
  wave3: Wave3State;
  // --- end wave3 ---
  // --- wave4 ---
  wave4_capabilities: Wave4Capabilities;
  /** The resting-cursor watcher behind focus follows mouse and the hover picker. */
  pointer: PointerStatus;
  /** The language the native strings are shown in now (never `system`). */
  language: Exclude<Language, 'system'>;
  /** The choices for `Settings.language`, `system` first. */
  languages: LanguageOption[];
  // --- end wave4 ---
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

// --- wave3 ---
// Workspaces, focus dim, calm desk, share-safe and window highlight
// (crates/krova-core: workspace.rs, focus.rs, calm.rs; src-tauri/src/wave3.rs).

export interface FocusSettings {
  enabled: boolean;
  /** How dark the rest of the screen gets, 0.2..0.8. Default 0.45. */
  opacity: number;
}

export interface CalmDeskSettings {
  /** Every monitor (default) or only the one under the cursor. */
  all_monitors: boolean;
}

/** `active` = the focused window, `all` = every window, `glow` = a soft glow on the focused one. */
export type HighlightMode = 'none' | 'active' | 'all' | 'glow';

export interface HighlightSettings {
  mode: HighlightMode;
  /** `#rrggbb`, or null for the accent colour. */
  color: string | null;
  /** 1..6 px at 100 % scale. Default 2. */
  width_px: number;
}

/** One program of a workspace: its executable path, never a title or arguments. */
export interface WorkspaceEntry {
  exe: string;
  monitor_id: string;
  zone_id: number;
}

export interface WorkspaceLayout {
  monitor_id: string;
  layout_id: string;
}

export interface Workspace {
  id: string;
  /** 1..64 characters, no control, zero-width or direction characters. */
  name: string;
  /** The layout that was active on each monitor at capture time. */
  layouts: WorkspaceLayout[];
  /** At most 64. */
  entries: WorkspaceEntry[];
}

export type EntryOutcome =
  | 'placed'
  | 'launched'
  | 'timed_out'
  | 'not_running'
  | 'refused'
  | 'no_zone'
  | 'failed';

export interface EntryResult {
  /** File name only (`code.exe`), never a folder. */
  program: string;
  monitor_id: string;
  zone_id: number;
  outcome: EntryOutcome;
  /**
   * Only for `refused`: the full path that was refused (the file's final path,
   * links resolved, when it could be opened; otherwise the stored path). Absent otherwise.
   */
  path?: string | null;
}

export interface RestoreReport {
  workspace_id: string;
  entries: EntryResult[];
  /** How many other windows were minimised (never closed). */
  minimized: number;
}

export interface Wave3Capabilities {
  workspaces: boolean;
  calm_desk: boolean;
  focus_dim: boolean;
  highlight: boolean;
  /** Windows 11 draws thin borders itself. */
  dwm_borders: boolean;
}

export interface CalmDeskState {
  active: boolean;
  count: number;
}

export interface Wave3State {
  capabilities: Wave3Capabilities;
  calm_desk: CalmDeskState;
  focus_active: boolean;
  share_safe: boolean;
  visuals_error: string | null;
  /** Id of the workspace being restored right now. */
  restoring: string | null;
}
// --- end wave3 ---
// --- wave4 ---

/** `system` follows the Windows display language (English if Krova has no table for it). */
export type Language = 'system' | 'en' | 'es' | 'de' | 'ja' | 'zh-Hans';

export interface LanguageOption {
  id: Language;
  /** The language's own name ("Deutsch", "日本語"); "System" for `system`. */
  name: string;
}

/**
 * Logical pixels (96 dpi) kept free on each side of a monitor, integers 0..400.
 * Krova multiplies them by the monitor scale and subtracts them from the OS work
 * area; `MonitorSnapshot.work` is already the reduced area. If the two sides of an
 * axis would leave less than 200 px, that axis is ignored.
 */
export interface ReservedEdges {
  top: number;
  right: number;
  bottom: number;
  left: number;
}

export interface Wave4Capabilities {
  focus_follows_mouse: boolean;
  hover_picker: boolean;
  open_releases_page: boolean;
}

export interface PointerStatus {
  /** The watcher runs: a pointer feature is on and Krova is enabled. */
  active: boolean;
  /** Times Windows refused to activate a window since Krova started. */
  focus_refused: number;
}

/** A zone of the hover picker, in physical pixels, gap applied. */
export interface HoverZone {
  id: number;
  rect: PxRect;
}

/** Payload of the backend "hover-picker" event. Never carries a window title. */
export interface HoverPicker {
  /** `MonitorProfile.id` of the monitor under the cursor. */
  monitor_id: string;
  window_rect: PxRect;
  /** `[x, y]`, virtual-screen pixels. */
  cursor: [number, number];
  layout_id: string;
  zones: HoverZone[];
}
// --- end wave4 ---
// --- notch ---
/**
 * Further "navigate" targets sent because of the notch: `cheatsheet` (to the main
 * window, from the notch menu) and `panel` (to the `panel` window, after
 * `panel-anchor`). Kept out of `NavigateTarget` so existing views ignore them.
 */
export type NotchNavigateTarget = 'cheatsheet' | 'panel';
// --- notch ---
