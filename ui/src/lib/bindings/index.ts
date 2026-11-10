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
export type Modifier = "shift" | "ctrl" | "alt" | "super";
export type MoveMode = "animate" | "instant";
export type Theme = "system" | "light" | "dark";

/**
 * Keys held together with Left/Right/Up/1..9 for the window-move shortcuts.
 * `win` is the Windows key (Command on macOS, once shortcuts exist there).
 */
export type ShortcutModifier =
  "ctrl_alt" | "win_alt" | "ctrl_alt_shift" | "win_ctrl";

// --- corners ---
/** What a hot corner does. Rust: `krova_core::corners::HotCornerAction`. */
export type HotCornerAction =
  | "switcher"
  | "cheat_sheet"
  | "focus"
  | "calm_desk"
  | "pause"
  | "show_zones"
  | "panel";
export type HotCorner =
  "top_left" | "top_right" | "bottom_left" | "bottom_right";

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
   * Deprecated: replaced by `shortcuts`. Kept so an older config migrates (a
   * `win_alt` / `win_ctrl` choice becomes overrides). Send back what you got.
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
  // --- corners ---
  /** What each outer screen corner does when the cursor rests in it. */
  hot_corners: HotCornerSettings;
  /** Per-program rules, in the order the user made them (at most 256). Default []. */
  app_rules: AppRule[];
  /** Changes to the default shortcut map (see `ShortcutMap`). Default {}. */
  shortcuts: ShortcutMap;
  /** Accent, corner radius and border width of the native overlays. */
  appearance: Appearance;
}

// --- notch ---
/** Where on its monitor the notch sits. */
export type NotchCorner =
  "top_center" | "top_left" | "top_right" | "bottom_left" | "bottom_right";
/** Which monitor shows the notch: the primary one, or the one under the cursor. */
export type NotchMonitor = "primary" | "cursor";

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
  /** Auto-arrange on this monitor. On by default. Change with `setAutoArrange`. */
  auto_arrange: ArrangeSettings;
  /** The first-run preset was offered to this monitor (internal; never shown). */
  preset_seeded: boolean;
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
export type Permission = "not_required" | "granted" | "denied";

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
  // --- wave3 ---
  wave3: Wave3State;
  // --- end wave3 ---
  // --- wave4 ---
  wave4_capabilities: Wave4Capabilities;
  /** The resting-cursor watcher behind focus follows mouse and the hover picker. */
  pointer: PointerStatus;
  /** The language the native strings are shown in now (never `system`). */
  language: Exclude<Language, "system">;
  /** The choices for `Settings.language`, `system` first. */
  languages: LanguageOption[];
  // --- end wave4 ---
  /** The program in front and whether auto-arrange looks after it; null if unknown. */
  front_app: FrontApp | null;
  /** Auto-arrange is paused (Ctrl+Alt+T, the tray, `setPaused`). */
  paused: boolean;
  /** Every shortcut with its chord and state, in sheet order. */
  shortcut_actions: ShortcutActionState[];
}

/**
 * Divider direction. `vertical` = a vertical divider, zones end up side by side
 * (positions are x values); `horizontal` = zones stacked (positions are y).
 */
export type Axis = "vertical" | "horizontal";

/** Payload of the backend "navigate" event. */
export type View = "editor" | "settings";

/**
 * What a "navigate" event or `take_pending_view` can carry: the two views of the
 * editor window, or `switcher` (the layout switcher, opened by its global shortcut;
 * the monitor it is for comes from `switcher_monitor`). A UI that does not know
 * `switcher` yet must ignore it.
 */
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
export type HighlightMode = "none" | "active" | "all" | "glow";

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
  | "placed"
  | "launched"
  | "timed_out"
  | "not_running"
  | "refused"
  | "no_zone"
  | "failed";

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
export type Language = "system" | "en" | "es" | "de" | "ja" | "zh-Hans";

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
export type NotchNavigateTarget = "cheatsheet" | "panel";
// --- notch ---
export type NavigateTarget = View | "switcher" | "cheat_sheet" | "panel";

// ---------------------------------------------------------------- Wave 1: arrange

/** Where a new window goes: the first free zone, or the first zone (others shift). */
export type ArrangePolicy = "next_free" | "main_first";

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
  | { kind: "never_touch" }
  | { kind: "float" }
  /** 1-based zone number in reading order (1..64). */
  | { kind: "preferred_zone"; value: number }
  /** A `MonitorProfile.id`. */
  | { kind: "start_on_monitor"; value: string };

/** A rule for one program, by executable file name (`chrome.exe`), never a title. */
export interface AppRule {
  exe: string;
  action: RuleAction;
}

/** Why the front window is not arranged (`FrontApp.reason`). */
export type FrontAppReason =
  | "dialog"
  | "tool"
  | "popup"
  | "elevated"
  | "never_touch"
  | "float"
  | "auto_arrange_off"
  | "paused"
  | "other";

/** The program in front, for the "Not arranged" chip. */
export interface FrontApp {
  exe: string | null;
  arranged: boolean;
  reason: FrontAppReason | null;
}

export type PresetFamily = "grid" | "columns" | "rows" | "main_side";
export type PresetVariant = "landscape" | "ultrawide" | "portrait";
export type MainSize = "half" | "sixty" | "two_thirds";

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

// ---------------------------------------------------------------- Wave 2: shortcuts

/**
 * The user's changes to the default shortcut map: action id -> chord text
 * (`"Ctrl+Alt+Left"`) or `null` (switched off). Missing ids keep their default.
 */
export type ShortcutMap = Record<string, string | null>;

export type ShortcutGroup = "Windows" | "Layouts" | "Krova";

/**
 * `active` registered; `off` no chord or its feature is off; `in_use` Windows refused
 * it (another app owns it); `error` another refusal; `unsupported` no global
 * shortcuts on this OS.
 */
export type ShortcutStatusCode =
  "active" | "off" | "in_use" | "error" | "unsupported";

/** One row of the shortcut sheet (`AppState.shortcut_actions`, in sheet order). */
export interface ShortcutActionState {
  /** `move_left`, `swap_up`, `zone_3`, `layout_2`, `maximize_in_zone`, `stack_prev`,
   * `stack_next`, `switcher`, `pause`, `cheat_sheet`, `focus`. */
  id: string;
  /** One to four words, e.g. "Move left". */
  label: string;
  group: ShortcutGroup;
  /** Display text of the chord now; null when switched off. */
  chord: string | null;
  default_chord: string | null;
  status: ShortcutStatusCode;
  /** Why, for `in_use` / `error`. */
  message: string | null;
}

/** Answer of `probe_shortcut`. */
export interface ProbeResult {
  /** `free`, `current` (already this action's), `conflict` (another Krova action),
   * `in_use` (another program), `invalid` (not a chord), `unknown` (cannot check). */
  status: "free" | "current" | "conflict" | "in_use" | "invalid" | "unknown";
  /** Normalised chord text, when it could be read. */
  chord: string | null;
  /** The Krova action id that has it, for `conflict`. */
  conflict_with: string | null;
  /** One sentence for the UI. */
  message: string;
  /** Up to three free chords with the same key. */
  suggestions: string[];
}

/** Window labels of the two warm popups. */
export type PopupLabel = "panel" | "switcher" | "picker";

/** Payload of the `panel-anchor` event: where the panel was opened from (the notch). */
// ---------------------------------------------------------------- appearance

/**
 * How Krova's native overlays (zones, stack tabs, the notch) look. `highlight_opacity`
 * stays the opacity. `highlight_color` is the old name of the accent (kept; a colour
 * picked there before `appearance` existed was migrated to `accent`).
 */
export interface Appearance {
  /** `#rrggbb`. Default `#ff5b1f`. */
  accent: string;
  /** Integer 0..16, logical px. Default 10. */
  corner_radius_px: number;
  /** Integer 1..6, logical px; the highlighted zone uses twice this. Default 2. */
  border_width_px: number;
}
