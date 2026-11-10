// In-memory (optionally localStorage-backed) implementation of the Backend
// contract for demo mode. It never touches the network and never writes a real
// config. Edit commands go through ./edit.ts, a mirror of krova-core's edit.rs.
//
// Like the Tauri commands, every method rejects with a plain string on error.

import type { Backend, Unlisten } from "../backend";
import type {
  AppState,
  Capabilities,
  Config,
  HotCorner,
  HotCornerEvent,
  HoverPicker,
  Layout,
  MonitorProfile,
  MonitorSnapshot,
  NavigateTarget,
  NotchCorner,
  NotchNavigateTarget,
  PanelAnchor,
  Permission,
  ReservedEdges,
  Settings,
  AppRule,
  Appearance,
  ArrangeSettings,
  FrontApp,
  ShortcutStatus,
} from "../bindings";
import {
  editMoveDivider,
  editRemoveZone,
  editSplitZone,
  validateLayout,
} from "./edit";
import {
  MAX_LAYOUTS_PER_MONITOR,
  exportEnvelope,
  messages as exchangeMessages,
  parseEnvelope,
  uniqueLayoutId,
} from "./exchange";
import { templates } from "./templates";
// --- wave3 ---
import {
  createWave3Mock,
  defaultWave3Settings,
  sanitizeWave3Settings,
} from "./wave3Mock";
// --- end wave3 ---
// --- wave4 ---
import {
  DEFAULT_FOCUS_DELAY_MS,
  LANGUAGES,
  NO_RESERVE,
  applyReserved,
  clampFocusDelay,
  resolveLanguage,
  sanitizeLanguage,
  sanitizeReserved,
} from "./wave4";
// --- end wave4 ---
// --- notch ---
import { defaultNotchSettings, demoAnchor, sanitizeNotch } from "./notch";
// --- notch ---
// --- corners ---
import {
  defaultHotCorners,
  hotCornerEvent,
  hotCornersStatus,
  sanitizeHotCorners,
} from "./cornersMock";
import { allPresets, variantForSize } from "./presets";
import {
  normalizeChord,
  probe,
  rows,
  sanitizeMap,
  setChord,
} from "./shortcuts";

/** localStorage key used for the optional persistence of demo edits. */
export const STORAGE_KEY = "krova-demo";

type StorageLike = Pick<Storage, "getItem" | "setItem" | "removeItem">;

/** Fake outcome of registering the global shortcuts (`?shortcuts=failed|off`). */
export type ShortcutsMode = "ok" | "failed" | "off";
/** Fake platform support (`?capabilities=none` is the macOS case today). */
export type CapabilitiesMode = "all" | "none";

/** What `?shortcuts=failed` reports as the shortcuts that could not be registered. */
export const DEMO_FAILED_SHORTCUTS = [
  "Ctrl+Alt+Shift+3 is already used by another app",
];
/** The same, for the layout switcher's `Ctrl+Alt+Shift+Space`. */
export const DEMO_FAILED_SWITCHER_SHORTCUT =
  "Ctrl+Alt+Shift+Space is already used by another app";

export interface MockOptions {
  /** Persist config edits to localStorage (default true when it is available). */
  persist?: boolean;
  /** Storage to use instead of window.localStorage (tests). */
  storage?: StorageLike | null;
  /** Fake permission state; `denied` shows the macOS Accessibility banner. */
  permission?: Permission;
  /** Ignore and clear whatever was persisted earlier. */
  reset?: boolean;
  /** Fake shortcut registration result (default `ok`). */
  shortcuts?: ShortcutsMode;
  /** Fake platform support (default `all`). */
  capabilities?: CapabilitiesMode;
}

/** Hooks for design review / e2e tests, exposed as `window.__krovaDemo`. */
export interface DemoControls {
  /** Behaves like the tray menu (or the switcher shortcut) sending the "navigate" event. */
  navigate(view: NavigateTarget): void;
  /** Which monitor `switcherMonitor()` reports, as if the cursor were on it. */
  setSwitcherMonitor(profileId: string | null): void;
  /** Change the fake permission and emit "state-changed". */
  setPermission(permission: Permission): void;
  /** Behaves like the tray "Enable/Disable" item. */
  toggleEnabled(): void;
  /** Change the fake shortcut registration result and emit "state-changed". */
  setShortcuts(mode: ShortcutsMode): void;
  /** Change the fake platform support and emit "state-changed". */
  setCapabilities(mode: CapabilitiesMode): void;
  /** Pretend the panel was opened from this rect (the notch): emits "panel-anchor". */
  anchorPanel(anchor: PanelAnchor): void;
  /** The same as `anchorPanel` (the name the panel page and its e2e tests use). */
  panelAnchor(anchor: PanelAnchor): void;
  /** Pretend another window came to the front (for the "Not arranged" chip). */
  setFrontApp(front: FrontApp): void;
  /** Back to the initial demo state (also clears localStorage). */
  reset(): void;
  // --- wave4 ---
  /** As if the modifier were held on a window's title bar on that monitor. */
  hoverPicker(monitorId?: string): HoverPicker | null;
  /** As if the cursor had left the title bar. */
  hoverPickerEnd(): void;
  /** Where the last `placeHovered` put the fake window. */
  lastPlaced(): { monitorId: string; zoneId: number } | null;
  /** How often `openReleasesPage` was called (the demo opens nothing). */
  releasesOpened(): number;
  // --- end wave4 ---
  // --- notch ---
  /**
   * As if the notch were clicked: sends "panel-anchor" for the primary monitor at
   * `corner` (default: `settings.notch.corner`); `onNotchNavigate` listeners then
   * get `panel`, like the real panel window.
   */
  clickNotch(corner?: NotchCorner): PanelAnchor;
  /** Listen to the notch's extra navigate targets (`panel`, `cheatsheet`). */
  onNotchNavigate(cb: (view: NotchNavigateTarget) => void): () => void;
  // --- notch ---
  // --- corners ---
  /**
   * Behaves like the cursor resting in `corner` of a monitor (default the first):
   * sends `hot-corner` and runs the action like the app (switcher, panel).
   * Returns the event, or null when that corner does nothing.
   */
  hotCorner(corner: HotCorner, monitorId?: string): HotCornerEvent | null;
}

export type MockBackend = Backend & { demo: DemoControls };

export function defaultSettings(): Settings {
  return {
    enabled: true,
    modifier: "shift",
    highlight_color: "#ff5b1f",
    highlight_opacity: 0.35,
    move_mode: "animate",
    edge_snapping: false,
    edge_size_px: 8,
    launch_at_login: false,
    theme: "system",
    keyboard_shortcuts: true,
    shortcut_modifier: "ctrl_alt_shift",
    restore_on_reconnect: true,
    linked_dividers: true,
    zone_gap_px: 0,
    layout_switcher_shortcut: true,
    // --- wave3 ---
    ...defaultWave3Settings(),
    // --- end wave3 ---
    // --- wave4 ---
    focus_follows_mouse: false,
    focus_follows_mouse_delay_ms: DEFAULT_FOCUS_DELAY_MS,
    hover_picker: false,
    language: "system",
    // --- end wave4 ---
    // --- notch ---
    notch: defaultNotchSettings(),
    // --- notch ---
    // --- corners ---
    hot_corners: defaultHotCorners(),
    app_rules: [],
    shortcuts: {},
    appearance: { accent: "#ff5b1f", corner_radius_px: 10, border_width_px: 2 },
  };
}

const MONITORS: MonitorSnapshot[] = [
  {
    profile_id: "monitor-1",
    label: "Display 1 (primary)",
    rect: { x: 0, y: 0, w: 2560, h: 1440 },
    work: { x: 0, y: 0, w: 2560, h: 1400 },
    scale: 1,
    primary: true,
  },
  {
    profile_id: "monitor-2",
    label: "Display 2",
    rect: { x: 2560, y: 0, w: 1920, h: 1080 },
    work: { x: 2560, y: 0, w: 1920, h: 1080 },
    scale: 1,
    primary: false,
  },
];

function initialConfig(): Config {
  const profiles: MonitorProfile[] = MONITORS.map((m, i) => ({
    id: m.profile_id,
    label: m.label,
    hardware_id: `DEMO-${i + 1}`,
    device_path: null,
    last_rect: { ...m.rect },
    layouts: templates(),
    active_layout: i === 0 ? "halves" : "thirds",
    reserved: { ...NO_RESERVE }, // --- wave4 ---
    auto_arrange: defaultArrange(),
    preset_seeded: true,
  }));
  return {
    schema_version: 1,
    settings: defaultSettings(),
    monitors: profiles,
    // --- wave3 ---
    workspaces: [],
    // --- end wave3 ---
  };
}

const clone = <T>(v: T): T => JSON.parse(JSON.stringify(v)) as T;

/** Like `Appearance::sanitized`: bad colours fall back, numbers are clamped. */
function sanitizeAppearance(a: Partial<Appearance> | undefined): Appearance {
  const accent =
    typeof a?.accent === "string" && /^\s*#?[0-9a-fA-F]{6}\s*$/.test(a.accent)
      ? `#${a.accent.trim().replace("#", "").toLowerCase()}`
      : "#ff5b1f";
  const int = (v: unknown, def: number, lo: number, hi: number) =>
    typeof v === "number" && Number.isFinite(v)
      ? Math.min(hi, Math.max(lo, Math.round(v)))
      : def;
  return {
    accent,
    corner_radius_px: int(a?.corner_radius_px, 10, 0, 16),
    border_width_px: int(a?.border_width_px, 2, 1, 6),
  };
}
export function defaultArrange(): ArrangeSettings {
  return {
    enabled: true,
    policy: "next_free",
    fill_gaps: false,
    stack_when_full: true,
  };
}

/** Like krova-core `validate_exe_name`: a bare file name, no path, no hidden characters. */
/** Code point ranges of control, zero-width, line-separating and bidi characters. */
const HIDDEN_RANGES: [number, number][] = [
  [0x00, 0x1f],
  [0x7f, 0x9f],
  [0xad, 0xad],
  [0x200b, 0x200f],
  [0x2028, 0x202e],
  [0x2060, 0x2069],
  [0xfeff, 0xfeff],
];

function hasHidden(s: string): boolean {
  for (const ch of s) {
    const c = ch.codePointAt(0) ?? 0;
    if (HIDDEN_RANGES.some(([a, b]) => c >= a && c <= b)) return true;
  }
  return false;
}

const PATH_CHARS = ["/", ":", String.fromCharCode(92)];

export function exeNameError(exe: string): string | null {
  const name = exe.trim();
  if (!name) return "the program name is empty";
  if (name.length > 128)
    return "the program name is too long (at most 128 characters)";
  if (PATH_CHARS.some((c) => name.includes(c))) {
    return `"${name}" is a path, not a program file name`;
  }
  if (hasHidden(name))
    return "the program name has control or invisible characters";
  return null;
}

function ruleError(rule: AppRule): string | null {
  const bad = exeNameError(rule.exe);
  if (bad) return bad;
  const a = rule.action;
  if (
    a.kind === "preferred_zone" &&
    !(Number.isInteger(a.value) && a.value >= 1 && a.value <= 64)
  ) {
    return "the zone number must be between 1 and 64";
  }
  if (
    a.kind === "start_on_monitor" &&
    (typeof a.value !== "string" || !a.value)
  ) {
    return "the monitor id is empty";
  }
  return null;
}

const sameExe = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

/** Drop invalid rules and later duplicates, keep the first 256 (`AppRules::sanitized`). */
function sanitizeRules(rules: AppRule[]): AppRule[] {
  const out: AppRule[] = [];
  for (const r of rules) {
    if (out.length >= 256) break;
    const rule = { exe: String(r?.exe ?? "").trim(), action: r?.action };
    if (
      !rule.action ||
      ruleError(rule) ||
      out.some((o) => sameExe(o.exe, rule.exe))
    )
      continue;
    out.push(clone(rule));
  }
  return out;
}

const MODIFIERS = ["shift", "ctrl", "alt", "super"];
const MOVE_MODES = ["animate", "instant"];
const THEMES = ["system", "light", "dark"];
const SHORTCUT_MODIFIERS = [
  "ctrl_alt",
  "win_alt",
  "ctrl_alt_shift",
  "win_ctrl",
];

/** Settings fields added later are optional on the wire (`#[serde(default)]`). */
const flag = (v: unknown, fallback: boolean): boolean =>
  typeof v === "boolean" ? v : fallback;

/** Validate like serde would (bad enums/colors are errors) and clamp numbers. */
function sanitizeSettings(s: Settings): Settings {
  const defaults = defaultSettings();
  if (!MODIFIERS.includes(s.modifier))
    throw `invalid modifier "${String(s.modifier)}"`;
  if (!MOVE_MODES.includes(s.move_mode))
    throw `invalid move_mode "${String(s.move_mode)}"`;
  if (!THEMES.includes(s.theme)) throw `invalid theme "${String(s.theme)}"`;
  const shortcutModifier = s.shortcut_modifier ?? defaults.shortcut_modifier;
  if (!SHORTCUT_MODIFIERS.includes(shortcutModifier)) {
    throw `invalid shortcut_modifier "${String(shortcutModifier)}"`;
  }
  if (!/^#[0-9a-fA-F]{6}$/.test(s.highlight_color)) {
    throw `invalid highlight_color "${String(s.highlight_color)}"`;
  }
  const opacity = Number.isFinite(s.highlight_opacity)
    ? s.highlight_opacity
    : 0.35;
  const edge = Number.isFinite(s.edge_size_px) ? Math.round(s.edge_size_px) : 8;
  const gap = Number.isFinite(s.zone_gap_px)
    ? Math.round(s.zone_gap_px)
    : defaults.zone_gap_px;
  return {
    enabled: !!s.enabled,
    modifier: s.modifier,
    highlight_color: s.highlight_color.toLowerCase(),
    highlight_opacity: Math.min(1, Math.max(0, opacity)),
    move_mode: s.move_mode,
    edge_snapping: !!s.edge_snapping,
    edge_size_px: Math.min(64, Math.max(2, edge)),
    launch_at_login: !!s.launch_at_login,
    theme: s.theme,
    keyboard_shortcuts: flag(s.keyboard_shortcuts, defaults.keyboard_shortcuts),
    shortcut_modifier: shortcutModifier,
    restore_on_reconnect: flag(
      s.restore_on_reconnect,
      defaults.restore_on_reconnect,
    ),
    linked_dividers: flag(s.linked_dividers, defaults.linked_dividers),
    zone_gap_px: Math.min(32, Math.max(0, gap)),
    layout_switcher_shortcut: flag(
      s.layout_switcher_shortcut,
      defaults.layout_switcher_shortcut,
    ),
    // --- wave3 ---
    ...sanitizeWave3Settings(s),
    // --- end wave3 ---
    // --- wave4 ---
    focus_follows_mouse: flag(
      s.focus_follows_mouse,
      defaults.focus_follows_mouse,
    ),
    focus_follows_mouse_delay_ms: clampFocusDelay(
      s.focus_follows_mouse_delay_ms,
    ),
    hover_picker: flag(s.hover_picker, defaults.hover_picker),
    language: sanitizeLanguage(s.language),
    // --- end wave4 ---
    // --- notch ---
    notch: sanitizeNotch(s.notch),
    // --- notch ---
    // --- corners ---
    hot_corners: sanitizeHotCorners(s.hot_corners),
    app_rules: Array.isArray(s.app_rules) ? sanitizeRules(s.app_rules) : [],
    shortcuts: sanitizeMap(s.shortcuts),
    appearance: sanitizeAppearance(s.appearance),
  };
}

function optionsFromLocation(): MockOptions {
  if (typeof location === "undefined") return {};
  const q = new URLSearchParams(location.search);
  const p = q.get("permission");
  const out: MockOptions = {};
  if (p === "denied" || p === "granted" || p === "not_required")
    out.permission = p;
  if (q.has("reset")) out.reset = true;
  if (q.has("nopersist")) out.persist = false;
  const sc = q.get("shortcuts");
  if (sc === "failed" || sc === "off") out.shortcuts = sc;
  if (q.get("capabilities") === "none") out.capabilities = "none";
  return out;
}

function defaultStorage(): StorageLike | null {
  try {
    return typeof localStorage === "undefined" ? null : localStorage;
  } catch {
    return null; // storage can throw when blocked
  }
}

interface Persisted {
  config: Config;
  autostart: boolean;
}

function looksValid(p: unknown): p is Persisted {
  const c = (p as Persisted | null)?.config;
  return (
    !!c &&
    typeof c === "object" &&
    typeof c.settings === "object" &&
    Array.isArray(c.monitors) &&
    MONITORS.every((m) =>
      c.monitors.some(
        (mp) =>
          mp.id === m.profile_id &&
          Array.isArray(mp.layouts) &&
          mp.layouts.length > 0,
      ),
    )
  );
}

export function createMockBackend(options: MockOptions = {}): MockBackend {
  const opts: MockOptions = { ...optionsFromLocation(), ...options };
  const storage: StorageLike | null =
    opts.persist === false
      ? null
      : opts.storage !== undefined
        ? opts.storage
        : defaultStorage();

  let config = initialConfig();
  let autostart = false;
  let permission: Permission = opts.permission ?? "not_required";
  let shortcutsMode: ShortcutsMode = opts.shortcuts ?? "ok";
  let capabilitiesMode: CapabilitiesMode = opts.capabilities ?? "all";

  const navigateListeners = new Set<(v: NavigateTarget) => void>();
  // The demo's "cursor" is on the first monitor until a test says otherwise.
  let switcherMonitorId: string | null = MONITORS[0].profile_id;
  // A plausible front window for the "Not arranged" chip.
  let frontApp: FrontApp = { exe: "code.exe", arranged: true, reason: null };
  let paused = false;
  const stateListeners = new Set<(s: AppState) => void>();
  // --- wave3 ---
  const wave3 = createWave3Mock(() => config);
  // --- end wave3 ---
  // --- notch ---
  const anchorListeners = new Set<(a: PanelAnchor) => void>();
  const notchNavigateListeners = new Set<(v: NotchNavigateTarget) => void>();
  let lastAnchor: PanelAnchor | null = null;
  function sendAnchor(corner?: NotchCorner): PanelAnchor {
    const monitor = MONITORS.find((m) => m.primary) ?? MONITORS[0];
    const anchor = demoAnchor(monitor, corner ?? config.settings.notch.corner);
    lastAnchor = anchor;
    for (const cb of anchorListeners) cb({ ...anchor });
    for (const cb of notchNavigateListeners) cb("panel");
    return { ...anchor };
  }
  // --- notch ---
  // --- corners ---
  const hotCornerListeners = new Set<(e: HotCornerEvent) => void>();

  function persist(): void {
    if (!storage) return;
    try {
      storage.setItem(
        STORAGE_KEY,
        JSON.stringify({ config, autostart } satisfies Persisted),
      );
    } catch {
      // Quota or blocked storage: the demo keeps working in memory.
    }
  }

  function capabilities(): Capabilities {
    const on = capabilitiesMode === "all";
    return { shortcuts: on, window_memory: on, auto_arrange: on };
  }

  /**
   * The shortcuts are only registered when the platform supports them, the
   * setting is on and the demo flag does not say otherwise; conflicts are only
   * reported while registering.
   */
  function shortcutStatus(): ShortcutStatus {
    const registering =
      capabilities().shortcuts &&
      config.settings.keyboard_shortcuts &&
      shortcutsMode !== "off";
    return {
      active: registering,
      failed:
        registering && shortcutsMode === "failed"
          ? [...DEMO_FAILED_SHORTCUTS]
          : [],
    };
  }

  /** The switcher's `<mods>+Space` has its own switch and its own (single) conflict. */
  function layoutSwitcherStatus(): ShortcutStatus {
    const registering =
      capabilities().shortcuts &&
      config.settings.layout_switcher_shortcut &&
      shortcutsMode !== "off";
    return {
      active: registering && shortcutsMode !== "failed",
      failed:
        registering && shortcutsMode === "failed"
          ? [DEMO_FAILED_SWITCHER_SHORTCUT]
          : [],
    };
  }

  // --- wave4 ---
  /** The demo's monitors, with each profile's reserved edges taken off the work area. */
  function monitorSnapshots(): MonitorSnapshot[] {
    return MONITORS.map((m) => {
      const p = config.monitors.find((mp) => mp.id === m.profile_id);
      return { ...m, work: applyReserved(m.work, m.scale, p?.reserved) };
    });
  }

  function wave4State() {
    const on = capabilitiesMode === "all";
    const s = config.settings;
    const browser =
      typeof navigator === "undefined" ? undefined : navigator.language;
    return {
      wave4_capabilities: {
        focus_follows_mouse: on,
        hover_picker: on,
        open_releases_page: on,
      },
      pointer: {
        active: on && s.enabled && (s.focus_follows_mouse || s.hover_picker),
        focus_refused: 0,
      },
      language: resolveLanguage(s.language, browser),
      languages: LANGUAGES,
    };
  }

  const hoverListeners = new Set<(p: HoverPicker) => void>();
  const hoverEndListeners = new Set<() => void>();
  let hovered: HoverPicker | null = null;
  let lastPlaced: { monitorId: string; zoneId: number } | null = null;
  let releasesOpened = 0;
  // --- end wave4 ---

  function snapshot(): AppState {
    return clone({
      config,
      monitors: monitorSnapshots(),
      autostart,
      permission,
      version: "0.1.0",
      shortcuts: shortcutStatus(),
      capabilities: capabilities(),
      layout_switcher: layoutSwitcherStatus(),
      // --- wave3 ---
      wave3: wave3.status(),
      // --- end wave3 ---
      // --- wave4 ---
      ...wave4State(),
      // --- end wave4 ---
      // --- integration --- share-safe: never the program name (like Rust)
      front_app: capabilities().auto_arrange
        ? config.settings.share_safe
          ? { ...frontApp, exe: null }
          : frontApp
        : null,
      paused,
      shortcut_actions: rows(
        config.settings,
        capabilities().shortcuts,
        shortcutStatus().failed,
      ),
    });
  }

  function emitState(): void {
    const s = snapshot();
    for (const cb of stateListeners) cb(s);
  }

  function reset(clearStorage: boolean): void {
    config = initialConfig();
    autostart = false;
    shortcutsMode = opts.shortcuts ?? "ok";
    capabilitiesMode = opts.capabilities ?? "all";
    switcherMonitorId = MONITORS[0].profile_id;
    // --- wave3 ---
    wave3.reset();
    // --- end wave3 ---
    if (clearStorage && storage) {
      try {
        storage.removeItem(STORAGE_KEY);
      } catch {
        // ignore
      }
    }
  }

  if (storage) {
    if (opts.reset) {
      reset(true);
    } else {
      try {
        const raw = storage.getItem(STORAGE_KEY);
        if (raw) {
          const parsed: unknown = JSON.parse(raw);
          if (looksValid(parsed)) {
            config = { ...initialConfig(), ...parsed.config };
            config.settings = {
              ...defaultSettings(),
              ...parsed.config.settings,
            };
            for (const m of config.monitors) {
              m.auto_arrange = {
                ...defaultArrange(),
                ...(m.auto_arrange ?? {}),
              };
              m.preset_seeded = m.preset_seeded ?? true;
            }
            autostart = !!parsed.autostart;
          }
        }
      } catch {
        // Corrupt demo state: fall back to the defaults, like config.rs does.
      }
    }
  }

  function profile(monitorId: string): MonitorProfile {
    const p = config.monitors.find((m) => m.id === monitorId);
    if (!p) throw `monitor ${monitorId} not found`;
    return p;
  }

  const demo: DemoControls = {
    navigate(view) {
      for (const cb of navigateListeners) cb(view);
    },
    setSwitcherMonitor(profileId) {
      switcherMonitorId = profileId;
    },
    setPermission(p) {
      permission = p;
      emitState();
    },
    toggleEnabled() {
      config.settings = {
        ...config.settings,
        enabled: !config.settings.enabled,
      };
      persist();
      emitState();
    },
    setShortcuts(mode) {
      shortcutsMode = mode;
      emitState();
    },
    setCapabilities(mode) {
      capabilitiesMode = mode;
      emitState();
    },
    setFrontApp(front) {
      frontApp = clone(front);
      emitState();
    },
    anchorPanel(anchor) {
      lastAnchor = clone(anchor);
      for (const cb of anchorListeners) cb(clone(anchor));
    },
    panelAnchor(anchor) {
      demo.anchorPanel(anchor);
    },
    reset() {
      paused = false;
      reset(true);
      emitState();
    },
    // --- wave4 ---
    hoverPicker(monitorId = MONITORS[0].profile_id) {
      const snap = monitorSnapshots().find((m) => m.profile_id === monitorId);
      const p = config.monitors.find((m) => m.id === monitorId);
      const layout = p?.layouts.find((l) => l.id === p.active_layout);
      if (!snap || !layout) return null;
      const gap = Math.min(32, Math.max(0, config.settings.zone_gap_px));
      const w = snap.work;
      // krova-core zone.rs: margin of `lo` around the work area, then every zone
      // gives up `hi` on its left/top and the rest on its right/bottom.
      const lo = Math.floor(gap / 2);
      const hi = gap - lo;
      const inner = gap
        ? {
            x: w.x + lo,
            y: w.y + lo,
            w: Math.max(1, w.w - gap),
            h: Math.max(1, w.h - gap),
          }
        : w;
      const edge = (o: number, size: number, n: number) =>
        o + Math.round(n * size);
      const zones = layout.zones.map((z) => {
        const l = edge(inner.x, inner.w, z.rect.x);
        const t = edge(inner.y, inner.h, z.rect.y);
        const r = edge(inner.x, inner.w, z.rect.x + z.rect.w);
        const b = edge(inner.y, inner.h, z.rect.y + z.rect.h);
        const rect = gap
          ? {
              x: l + hi,
              y: t + hi,
              w: Math.max(1, r - l - gap),
              h: Math.max(1, b - t - gap),
            }
          : { x: l, y: t, w: r - l, h: b - t };
        return { id: z.id, rect };
      });
      const window_rect = { x: w.x + 200, y: w.y + 120, w: 900, h: 600 };
      hovered = {
        monitor_id: monitorId,
        window_rect,
        cursor: [window_rect.x + 300, window_rect.y + 12],
        layout_id: layout.id,
        zones,
      };
      for (const cb of hoverListeners) cb(clone(hovered));
      return clone(hovered);
    },
    hoverPickerEnd() {
      hovered = null;
      for (const cb of hoverEndListeners) cb();
    },
    lastPlaced() {
      return lastPlaced ? { ...lastPlaced } : null;
    },
    releasesOpened() {
      return releasesOpened;
    },
    // --- end wave4 ---
    // --- notch ---
    clickNotch(corner) {
      return sendAnchor(corner);
    },
    onNotchNavigate(cb) {
      notchNavigateListeners.add(cb);
      return () => notchNavigateListeners.delete(cb);
    },
    // --- notch ---
    // --- corners ---
    hotCorner(corner, monitorId = MONITORS[0].profile_id) {
      const ev = hotCornerEvent(
        config.settings,
        corner,
        monitorId,
        capabilities().shortcuts,
      );
      if (!ev) return null;
      for (const cb of hotCornerListeners) cb(ev);
      if (ev.action === "switcher") {
        switcherMonitorId = monitorId;
        demo.navigate("switcher");
      } else if (ev.action === "panel") {
        demo.navigate("settings");
      }
      return ev;
    },
  };

  const backend: MockBackend = {
    kind: "mock",
    demo,

    async getState() {
      return snapshot();
    },

    async saveSettings(settings) {
      config.settings = sanitizeSettings(settings);
      autostart = config.settings.launch_at_login;
      persist();
      return snapshot();
    },

    async saveLayout(monitorId, layout) {
      const p = profile(monitorId);
      try {
        validateLayout(layout);
      } catch (e) {
        throw e instanceof Error ? e.message : String(e);
      }
      const stored = clone(layout);
      const i = p.layouts.findIndex((l) => l.id === stored.id);
      if (i >= 0) p.layouts[i] = stored;
      else if (p.layouts.length >= MAX_LAYOUTS_PER_MONITOR)
        throw exchangeMessages.tooManyLayouts;
      else p.layouts.push(stored);
      persist();
      return snapshot();
    },

    async deleteLayout(monitorId, layoutId) {
      const p = profile(monitorId);
      const i = p.layouts.findIndex((l) => l.id === layoutId);
      if (i < 0) throw `layout ${layoutId} not found`;
      if (p.layouts.length === 1) throw "a monitor needs at least one layout";
      p.layouts.splice(i, 1);
      if (p.active_layout === layoutId) p.active_layout = p.layouts[0].id;
      persist();
      return snapshot();
    },

    async setActiveLayout(monitorId, layoutId) {
      const p = profile(monitorId);
      if (!p.layouts.some((l) => l.id === layoutId))
        throw `layout ${layoutId} not found`;
      p.active_layout = layoutId;
      persist();
      return snapshot();
    },

    async moveLayout(monitorId, layoutId, toIndex) {
      const p = profile(monitorId);
      const from = p.layouts.findIndex((l) => l.id === layoutId);
      if (from < 0) throw `layout ${layoutId} not found`;
      if (
        !Number.isInteger(toIndex) ||
        toIndex < 0 ||
        toIndex >= p.layouts.length
      ) {
        throw positionOutOfRange(toIndex, p.layouts.length);
      }
      const [moved] = p.layouts.splice(from, 1);
      p.layouts.splice(toIndex, 0, moved);
      persist();
      return snapshot();
    },

    async insertLayout(monitorId, layout, index, activate) {
      try {
        validateLayout(layout);
      } catch (e) {
        throw e instanceof Error ? e.message : String(e);
      }
      const p = profile(monitorId);
      if (p.layouts.some((l) => l.id === layout.id)) {
        throw `layout ${layout.id} already exists`;
      }
      if (p.layouts.length >= MAX_LAYOUTS_PER_MONITOR)
        throw exchangeMessages.tooManyLayouts;
      if (!Number.isInteger(index) || index < 0 || index > p.layouts.length) {
        throw positionOutOfRange(index, p.layouts.length);
      }
      p.layouts.splice(index, 0, clone(layout));
      if (activate) p.active_layout = layout.id;
      persist();
      return snapshot();
    },

    async listTemplates() {
      return templates();
    },

    async listPresets(monitorId) {
      const m = MONITORS.find((x) => x.profile_id === monitorId);
      const variant = m ? variantForSize(m.work.w, m.work.h) : null;
      return allPresets().map((p) => ({
        ...p,
        recommended: p.variant === variant,
      }));
    },

    async setAutoArrange(monitorId, settings) {
      const p = profile(monitorId);
      if (settings.policy !== "next_free" && settings.policy !== "main_first") {
        throw `invalid policy "${String(settings.policy)}"`;
      }
      p.auto_arrange = {
        enabled: !!settings.enabled,
        policy: settings.policy,
        fill_gaps: !!settings.fill_gaps,
        stack_when_full: !!settings.stack_when_full,
      };
      persist();
      return snapshot();
    },

    async setAppRule(rule) {
      const normalized: AppRule = {
        exe: String(rule.exe ?? "").trim(),
        action: rule.action,
      };
      const bad = ruleError(normalized);
      if (bad) throw bad;
      const rules = config.settings.app_rules;
      const i = rules.findIndex((r) => sameExe(r.exe, normalized.exe));
      if (i >= 0) rules[i] = clone(normalized);
      else if (rules.length >= 256)
        throw "there are too many app rules (at most 256)";
      else rules.push(clone(normalized));
      persist();
      return snapshot();
    },

    async removeAppRule(exe) {
      const rules = config.settings.app_rules;
      const i = rules.findIndex((r) => sameExe(r.exe, exe));
      if (i < 0) throw `there is no rule for ${exe}`;
      rules.splice(i, 1);
      persist();
      return snapshot();
    },

    async arrangeMonitor(monitorId) {
      if (!MONITORS.some((m) => m.profile_id === monitorId)) {
        throw `monitor ${monitorId} is not connected`;
      }
    },

    async probeShortcut(action, chord) {
      return probe(config.settings.shortcuts, action, chord);
    },

    async setShortcut(action, chord) {
      const normalized = chord === null ? null : normalizeChord(chord);
      if (normalized !== null) {
        const p = probe(config.settings.shortcuts, action, normalized);
        if (p.status === "conflict") throw p.message;
      }
      config.settings = {
        ...config.settings,
        shortcuts: setChord(config.settings.shortcuts, action, normalized),
      };
      persist();
      return snapshot();
    },

    async resetShortcuts() {
      config.settings = { ...config.settings, shortcuts: {} };
      persist();
      return snapshot();
    },

    async setPaused(value) {
      paused = !!value;
      emitState();
    },

    async hidePopup(label) {
      if (label !== "panel" && label !== "switcher" && label !== "picker")
        throw `${String(label)} is not a popup window`;
    },
    async openMain(view) {
      if (!/^[a-z][a-z0-9_-]{0,31}$/.test(view))
        throw `"${view.slice(0, 32)}" is not a view`;
      if (view !== "switcher" && view !== "cheat_sheet")
        demo.navigate(view as NavigateTarget);
    },

    async switcherMonitor() {
      return switcherMonitorId !== null &&
        config.monitors.some((m) => m.id === switcherMonitorId)
        ? switcherMonitorId
        : null;
    },

    async exportLayout(layout) {
      try {
        return exportEnvelope(layout);
      } catch (e) {
        throw e instanceof Error ? e.message : String(e);
      }
    },

    async importLayout(monitorId, json) {
      let layout: Layout;
      try {
        layout = parseEnvelope(json);
      } catch (e) {
        throw e instanceof Error ? e.message : String(e);
      }
      const p = profile(monitorId);
      if (p.layouts.length >= MAX_LAYOUTS_PER_MONITOR)
        throw exchangeMessages.tooManyLayouts;
      layout.id = uniqueLayoutId(
        p.layouts.map((l) => l.id),
        layout.id,
      );
      p.layouts.push(layout);
      persist();
      return snapshot();
    },

    async editSplitZone(layout, zoneId, axis, at) {
      return run(() => editSplitZone(layout, zoneId, axis, at));
    },

    async editRemoveZone(layout, zoneId) {
      return run(() => editRemoveZone(layout, zoneId));
    },

    async editMoveDivider(layout, axis, from, to) {
      return run(() => editMoveDivider(layout, axis, from, to));
    },

    // The browser demo has no window lifecycle: nothing is ever parked.
    async takePendingView() {
      return null;
    },

    async onNavigate(cb): Promise<Unlisten> {
      navigateListeners.add(cb);
      return () => navigateListeners.delete(cb);
    },

    async onStateChanged(cb): Promise<Unlisten> {
      stateListeners.add(cb);
      return () => stateListeners.delete(cb);
    },

    // --- wave3 ---
    async toggleFocusDim() {
      wave3.toggleFocus();
      persist();
      emitState();
      return snapshot();
    },
    async toggleCalmDesk() {
      wave3.toggleCalmDesk();
      emitState();
      return snapshot();
    },
    async captureWorkspace(name, id) {
      wave3.capture(name, id);
      persist();
      emitState();
      return snapshot();
    },
    async renameWorkspace(id, name) {
      wave3.rename(id, name);
      persist();
      emitState();
      return snapshot();
    },
    async deleteWorkspace(id) {
      wave3.remove(id);
      persist();
      emitState();
      return snapshot();
    },
    async restoreWorkspace(id, launch) {
      const report = wave3.restore(id, launch);
      persist();
      emitState();
      return report;
    },
    // --- end wave3 ---
    // --- wave4 ---
    async setReservedEdges(monitorId: string, reserved: ReservedEdges) {
      profile(monitorId).reserved = sanitizeReserved(reserved);
      persist();
      const s = snapshot();
      for (const cb of stateListeners) cb(clone(s));
      return s;
    },

    async placeHovered(zoneId) {
      if (!hovered) throw "no window is under the picker";
      if (!hovered.zones.some((z) => z.id === zoneId)) {
        throw `zone ${zoneId} is not in the active layout`;
      }
      lastPlaced = { monitorId: hovered.monitor_id, zoneId };
      demo.hoverPickerEnd();
    },

    // The demo never opens anything: it only counts, so tests can check the call.
    async openReleasesPage() {
      releasesOpened += 1;
    },

    async onHoverPicker(cb): Promise<Unlisten> {
      hoverListeners.add(cb);
      return () => hoverListeners.delete(cb);
    },

    async onHoverPickerEnd(cb): Promise<Unlisten> {
      hoverEndListeners.add(cb);
      return () => hoverEndListeners.delete(cb);
    },
    // --- end wave4 ---
    // --- notch ---
    async onPanelAnchor(cb): Promise<Unlisten> {
      anchorListeners.add(cb);
      // The demo has no real notch: pretend it was just clicked.
      if (lastAnchor) cb({ ...lastAnchor });
      else queueMicrotask(() => sendAnchor());
      return () => anchorListeners.delete(cb);
    },

    async panelAnchor() {
      return lastAnchor ? { ...lastAnchor } : null;
    },

    async hidePanel() {
      // A browser tab has no panel window to hide.
    },
    // --- notch ---
    // --- corners ---
    async hotCornersStatus() {
      return hotCornersStatus(config.settings, capabilities().shortcuts);
    },

    async onHotCorner(cb): Promise<Unlisten> {
      hotCornerListeners.add(cb);
      return () => hotCornerListeners.delete(cb);
    },
  };

  return backend;
}

/** The wording of krova-core's `ProfileError::OutOfRange`. */
function positionOutOfRange(index: number, len: number): string {
  return `position ${index} is out of range (the monitor has ${len} layouts)`;
}

/** Run an edit and turn EditError into a plain string rejection, like Tauri. */
function run(op: () => Layout): Layout {
  try {
    return op();
  } catch (e) {
    throw e instanceof Error ? e.message : String(e);
  }
}
