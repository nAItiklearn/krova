// In-memory (optionally localStorage-backed) implementation of the Backend
// contract for demo mode. It never touches the network and never writes a real
// config. Edit commands go through ./edit.ts, a mirror of krova-core's edit.rs.
//
// Like the Tauri commands, every method rejects with a plain string on error.

import type { Backend, Unlisten } from '../backend';
import type {
  AppState,
  Capabilities,
  Config,
  Layout,
  MonitorProfile,
  MonitorSnapshot,
  NavigateTarget,
  Permission,
  Settings,
  ShortcutStatus,
} from '../bindings';
import { editMoveDivider, editRemoveZone, editSplitZone, validateLayout } from './edit';
import {
  MAX_LAYOUTS_PER_MONITOR,
  exportEnvelope,
  messages as exchangeMessages,
  parseEnvelope,
  uniqueLayoutId,
} from './exchange';
import { templates } from './templates';

/** localStorage key used for the optional persistence of demo edits. */
export const STORAGE_KEY = 'krova-demo';

type StorageLike = Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>;

/** Fake outcome of registering the global shortcuts (`?shortcuts=failed|off`). */
export type ShortcutsMode = 'ok' | 'failed' | 'off';
/** Fake platform support (`?capabilities=none` is the macOS case today). */
export type CapabilitiesMode = 'all' | 'none';

/** What `?shortcuts=failed` reports as the shortcuts that could not be registered. */
export const DEMO_FAILED_SHORTCUTS = ['Ctrl+Alt+Shift+3 is already used by another app'];
/** The same, for the layout switcher's `Ctrl+Alt+Shift+Space`. */
export const DEMO_FAILED_SWITCHER_SHORTCUT = 'Ctrl+Alt+Shift+Space is already used by another app';

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
  /** Back to the initial demo state (also clears localStorage). */
  reset(): void;
}

export type MockBackend = Backend & { demo: DemoControls };

export function defaultSettings(): Settings {
  return {
    enabled: true,
    modifier: 'shift',
    highlight_color: '#3b82f6',
    highlight_opacity: 0.35,
    move_mode: 'animate',
    edge_snapping: false,
    edge_size_px: 8,
    launch_at_login: false,
    theme: 'system',
    keyboard_shortcuts: true,
    shortcut_modifier: 'ctrl_alt_shift',
    restore_on_reconnect: true,
    linked_dividers: true,
    zone_gap_px: 0,
    layout_switcher_shortcut: true,
  };
}

const MONITORS: MonitorSnapshot[] = [
  {
    profile_id: 'monitor-1',
    label: 'Display 1 (primary)',
    rect: { x: 0, y: 0, w: 2560, h: 1440 },
    work: { x: 0, y: 0, w: 2560, h: 1400 },
    scale: 1,
    primary: true,
  },
  {
    profile_id: 'monitor-2',
    label: 'Display 2',
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
    active_layout: i === 0 ? 'halves' : 'thirds',
  }));
  return { schema_version: 1, settings: defaultSettings(), monitors: profiles };
}

const clone = <T>(v: T): T => JSON.parse(JSON.stringify(v)) as T;

const MODIFIERS = ['shift', 'ctrl', 'alt', 'super'];
const MOVE_MODES = ['animate', 'instant'];
const THEMES = ['system', 'light', 'dark'];
const SHORTCUT_MODIFIERS = ['ctrl_alt', 'win_alt', 'ctrl_alt_shift', 'win_ctrl'];

/** Settings fields added later are optional on the wire (`#[serde(default)]`). */
const flag = (v: unknown, fallback: boolean): boolean => (typeof v === 'boolean' ? v : fallback);

/** Validate like serde would (bad enums/colors are errors) and clamp numbers. */
function sanitizeSettings(s: Settings): Settings {
  const defaults = defaultSettings();
  if (!MODIFIERS.includes(s.modifier)) throw `invalid modifier "${String(s.modifier)}"`;
  if (!MOVE_MODES.includes(s.move_mode)) throw `invalid move_mode "${String(s.move_mode)}"`;
  if (!THEMES.includes(s.theme)) throw `invalid theme "${String(s.theme)}"`;
  const shortcutModifier = s.shortcut_modifier ?? defaults.shortcut_modifier;
  if (!SHORTCUT_MODIFIERS.includes(shortcutModifier)) {
    throw `invalid shortcut_modifier "${String(shortcutModifier)}"`;
  }
  if (!/^#[0-9a-fA-F]{6}$/.test(s.highlight_color)) {
    throw `invalid highlight_color "${String(s.highlight_color)}"`;
  }
  const opacity = Number.isFinite(s.highlight_opacity) ? s.highlight_opacity : 0.35;
  const edge = Number.isFinite(s.edge_size_px) ? Math.round(s.edge_size_px) : 8;
  const gap = Number.isFinite(s.zone_gap_px) ? Math.round(s.zone_gap_px) : defaults.zone_gap_px;
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
    restore_on_reconnect: flag(s.restore_on_reconnect, defaults.restore_on_reconnect),
    linked_dividers: flag(s.linked_dividers, defaults.linked_dividers),
    zone_gap_px: Math.min(32, Math.max(0, gap)),
    layout_switcher_shortcut: flag(s.layout_switcher_shortcut, defaults.layout_switcher_shortcut),
  };
}

function optionsFromLocation(): MockOptions {
  if (typeof location === 'undefined') return {};
  const q = new URLSearchParams(location.search);
  const p = q.get('permission');
  const out: MockOptions = {};
  if (p === 'denied' || p === 'granted' || p === 'not_required') out.permission = p;
  if (q.has('reset')) out.reset = true;
  if (q.has('nopersist')) out.persist = false;
  const sc = q.get('shortcuts');
  if (sc === 'failed' || sc === 'off') out.shortcuts = sc;
  if (q.get('capabilities') === 'none') out.capabilities = 'none';
  return out;
}

function defaultStorage(): StorageLike | null {
  try {
    return typeof localStorage === 'undefined' ? null : localStorage;
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
    typeof c === 'object' &&
    typeof c.settings === 'object' &&
    Array.isArray(c.monitors) &&
    MONITORS.every((m) =>
      c.monitors.some(
        (mp) => mp.id === m.profile_id && Array.isArray(mp.layouts) && mp.layouts.length > 0,
      ),
    )
  );
}

export function createMockBackend(options: MockOptions = {}): MockBackend {
  const opts: MockOptions = { ...optionsFromLocation(), ...options };
  const storage: StorageLike | null =
    opts.persist === false ? null : opts.storage !== undefined ? opts.storage : defaultStorage();

  let config = initialConfig();
  let autostart = false;
  let permission: Permission = opts.permission ?? 'not_required';
  let shortcutsMode: ShortcutsMode = opts.shortcuts ?? 'ok';
  let capabilitiesMode: CapabilitiesMode = opts.capabilities ?? 'all';

  const navigateListeners = new Set<(v: NavigateTarget) => void>();
  // The demo's "cursor" is on the first monitor until a test says otherwise.
  let switcherMonitorId: string | null = MONITORS[0].profile_id;
  const stateListeners = new Set<(s: AppState) => void>();

  function persist(): void {
    if (!storage) return;
    try {
      storage.setItem(STORAGE_KEY, JSON.stringify({ config, autostart } satisfies Persisted));
    } catch {
      // Quota or blocked storage: the demo keeps working in memory.
    }
  }

  function capabilities(): Capabilities {
    const on = capabilitiesMode === 'all';
    return { shortcuts: on, window_memory: on };
  }

  /**
   * The shortcuts are only registered when the platform supports them, the
   * setting is on and the demo flag does not say otherwise; conflicts are only
   * reported while registering.
   */
  function shortcutStatus(): ShortcutStatus {
    const registering =
      capabilities().shortcuts && config.settings.keyboard_shortcuts && shortcutsMode !== 'off';
    return {
      active: registering,
      failed: registering && shortcutsMode === 'failed' ? [...DEMO_FAILED_SHORTCUTS] : [],
    };
  }

  /** The switcher's `<mods>+Space` has its own switch and its own (single) conflict. */
  function layoutSwitcherStatus(): ShortcutStatus {
    const registering =
      capabilities().shortcuts && config.settings.layout_switcher_shortcut && shortcutsMode !== 'off';
    return {
      active: registering && shortcutsMode !== 'failed',
      failed: registering && shortcutsMode === 'failed' ? [DEMO_FAILED_SWITCHER_SHORTCUT] : [],
    };
  }

  function snapshot(): AppState {
    return clone({
      config,
      monitors: MONITORS,
      autostart,
      permission,
      version: '0.1.0',
      shortcuts: shortcutStatus(),
      capabilities: capabilities(),
      layout_switcher: layoutSwitcherStatus(),
    });
  }

  function emitState(): void {
    const s = snapshot();
    for (const cb of stateListeners) cb(s);
  }

  function reset(clearStorage: boolean): void {
    config = initialConfig();
    autostart = false;
    shortcutsMode = opts.shortcuts ?? 'ok';
    capabilitiesMode = opts.capabilities ?? 'all';
    switcherMonitorId = MONITORS[0].profile_id;
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
            config.settings = { ...defaultSettings(), ...parsed.config.settings };
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
      config.settings = { ...config.settings, enabled: !config.settings.enabled };
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
    reset() {
      reset(true);
      emitState();
    },
  };

  const backend: MockBackend = {
    kind: 'mock',
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
      else if (p.layouts.length >= MAX_LAYOUTS_PER_MONITOR) throw exchangeMessages.tooManyLayouts;
      else p.layouts.push(stored);
      persist();
      return snapshot();
    },

    async deleteLayout(monitorId, layoutId) {
      const p = profile(monitorId);
      const i = p.layouts.findIndex((l) => l.id === layoutId);
      if (i < 0) throw `layout ${layoutId} not found`;
      if (p.layouts.length === 1) throw 'a monitor needs at least one layout';
      p.layouts.splice(i, 1);
      if (p.active_layout === layoutId) p.active_layout = p.layouts[0].id;
      persist();
      return snapshot();
    },

    async setActiveLayout(monitorId, layoutId) {
      const p = profile(monitorId);
      if (!p.layouts.some((l) => l.id === layoutId)) throw `layout ${layoutId} not found`;
      p.active_layout = layoutId;
      persist();
      return snapshot();
    },

    async moveLayout(monitorId, layoutId, toIndex) {
      const p = profile(monitorId);
      const from = p.layouts.findIndex((l) => l.id === layoutId);
      if (from < 0) throw `layout ${layoutId} not found`;
      if (!Number.isInteger(toIndex) || toIndex < 0 || toIndex >= p.layouts.length) {
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
      if (p.layouts.length >= MAX_LAYOUTS_PER_MONITOR) throw exchangeMessages.tooManyLayouts;
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

    async switcherMonitor() {
      return switcherMonitorId !== null && config.monitors.some((m) => m.id === switcherMonitorId)
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
      if (p.layouts.length >= MAX_LAYOUTS_PER_MONITOR) throw exchangeMessages.tooManyLayouts;
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
