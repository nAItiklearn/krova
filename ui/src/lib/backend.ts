// The single typed seam between the UI and Rust. Components and stores import
// `api` from here and nothing else talks to Tauri. In a plain browser (or with
// `vite --mode demo`) `api` is the in-memory mock from ./mock/mockBackend.
//
// Tauri v2: JS argument names are camelCase, the Rust command parameters are
// snake_case; Tauri converts between them.

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type {
  AppRule,
  AppState,
  ArrangeSettings,
  Axis,
  Layout,
  NavigateTarget,
  PresetInfo,
  PanelAnchor,
  PopupLabel,
  ProbeResult,
  Settings,
} from './bindings';
import { createMockBackend, type DemoControls, type MockBackend } from './mock/mockBackend';

export type Unlisten = () => void;

export interface Backend {
  /** `mock` = browser-only demo mode: nothing is written to a real config. */
  readonly kind: 'tauri' | 'mock';

  getState(): Promise<AppState>;
  saveSettings(settings: Settings): Promise<AppState>;
  /** Upsert by layout.id. */
  saveLayout(monitorId: string, layout: Layout): Promise<AppState>;
  deleteLayout(monitorId: string, layoutId: string): Promise<AppState>;
  setActiveLayout(monitorId: string, layoutId: string): Promise<AppState>;
  /**
   * Reorder: the layout ends up at `toIndex` (0-based, a position in the monitor's
   * current list, so `0 <= toIndex < layouts.length`). Nothing else changes, the
   * active layout stays active. Rejects for an unknown monitor or layout or an index
   * out of range; moving a layout to where it already is succeeds.
   */
  moveLayout(monitorId: string, layoutId: string, toIndex: number): Promise<AppState>;
  /**
   * Put a layout back at `index` (`0 <= index <= layouts.length`): the undo of
   * `deleteLayout` (keep the deleted Layout and its old index). With `activate` it
   * also becomes the monitor's active layout, in the same step. Unlike `saveLayout`
   * it never replaces: rejects if that monitor already has the id. Same validation
   * and 64-layouts cap as `saveLayout`.
   */
  insertLayout(
    monitorId: string,
    layout: Layout,
    index: number,
    activate: boolean,
  ): Promise<AppState>;
  listTemplates(): Promise<Layout[]>;

  /**
   * Every preset (grid, columns, rows, main + side, 1..5 windows, three screen
   * shapes) as ordinary layouts. With a monitor id, those made for its shape are
   * `recommended`. Save one with `saveLayout` to use it.
   */
  listPresets(monitorId?: string): Promise<PresetInfo[]>;
  /** Auto-arrange on/off and its options for one monitor. */
  setAutoArrange(monitorId: string, settings: ArrangeSettings): Promise<AppState>;
  /**
   * Add a rule, or replace the rule of the same program (case-insensitive). Rejects
   * with a readable sentence, e.g. for a path instead of a file name.
   */
  setAppRule(rule: AppRule): Promise<AppState>;
  /** Delete a program's rule; rejects if there is none. */
  removeAppRule(exe: string): Promise<AppState>;
  /** Arrange the windows already open on a monitor, now ("Tile now"). */
  arrangeMonitor(monitorId: string): Promise<void>;

  /** Is `chord` free for `action`? Checks Krova's own map, then the OS. */
  probeShortcut(action: string, chord: string): Promise<ProbeResult>;
  /** Rebind an action, or switch it off with null. Rejects a chord another action has. */
  setShortcut(action: string, chord: string | null): Promise<AppState>;
  /** Every shortcut back to its default. */
  resetShortcuts(): Promise<AppState>;
  /** Pause / resume auto-arrange; the new state arrives with "state-changed". */
  setPaused(paused: boolean): Promise<void>;
  /** Hide the panel or the switcher window (on Esc). They stay warm. */
  hidePopup(label: PopupLabel): Promise<void>;
  /**
   * Open a view from a popup: `switcher` / `cheat_sheet` open the switcher window,
   * anything else the editor window with that view or settings section
   * (`settings`, `editor`, `corners`, ...: lower-case letters, digits, _ and -).
   */
  openMain(view: string): Promise<void>;
  /** Backend event "panel-anchor": the panel was opened from this rect (the notch). */
  onPanelAnchor(cb: (anchor: PanelAnchor) => void): Promise<Unlisten>;
  /**
   * `MonitorProfile.id` of the monitor the layout switcher was last opened for (the
   * one under the cursor when its shortcut was pressed), or null if it was never
   * opened or no monitor is connected. Read it when a `navigate` to `switcher` arrives.
   */
  switcherMonitor(): Promise<string | null>;

  /**
   * Pretty JSON text of the envelope `{"krova_layout":1,"layout":{...}}`, for the
   * clipboard. Rejects with a string if the layout is not valid.
   */
  exportLayout(layout: Layout): Promise<string>;
  /**
   * Add the layout in `json` (an exported envelope, at most 64 KiB) to a monitor.
   * The layout is validated like `saveLayout`; its id gets a suffix if the monitor
   * already has that id. Rejects with a string that is shown to the user as is.
   */
  importLayout(monitorId: string, json: string): Promise<AppState>;

  /** The original zone keeps the left/top part; the new zone gets id max+1. */
  editSplitZone(layout: Layout, zoneId: number, axis: Axis, at: number): Promise<Layout>;
  /** Gives the space to a neighbour sharing a FULL edge. */
  editRemoveZone(layout: Layout, zoneId: number): Promise<Layout>;
  /** Moves every zone edge on coordinate `from` to `to`. */
  editMoveDivider(layout: Layout, axis: Axis, from: number, to: number): Promise<Layout>;

  /**
   * The view a tray click asked for while the window was still being created
   * (`take_pending_view`), or null. The real app destroys its window on close and
   * parks the `navigate` event; call this once after `onNavigate` is registered.
   */
  takePendingView(): Promise<string | null>;
  /** Backend event "navigate" (from the tray menu or the layout switcher shortcut). */
  onNavigate(cb: (view: NavigateTarget) => void): Promise<Unlisten>;
  /** Backend event "state-changed". */
  onStateChanged(cb: (state: AppState) => void): Promise<Unlisten>;
}

/** Commands reject with a string; be tolerant of Error objects too. */
export function errorMessage(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e instanceof Error) return e.message;
  if (e && typeof e === 'object' && 'message' in e && typeof e.message === 'string') {
    return e.message;
  }
  try {
    return JSON.stringify(e);
  } catch {
    return String(e);
  }
}

export const tauriBackend: Backend = {
  kind: 'tauri',

  getState: () => invoke<AppState>('get_state'),
  saveSettings: (settings) => invoke<AppState>('save_settings', { settings }),
  saveLayout: (monitorId, layout) => invoke<AppState>('save_layout', { monitorId, layout }),
  deleteLayout: (monitorId, layoutId) => invoke<AppState>('delete_layout', { monitorId, layoutId }),
  setActiveLayout: (monitorId, layoutId) =>
    invoke<AppState>('set_active_layout', { monitorId, layoutId }),
  moveLayout: (monitorId, layoutId, toIndex) =>
    invoke<AppState>('move_layout', { monitorId, layoutId, toIndex }),
  insertLayout: (monitorId, layout, index, activate) =>
    invoke<AppState>('insert_layout', { monitorId, layout, index, activate }),
  listTemplates: () => invoke<Layout[]>('list_templates'),
  listPresets: (monitorId) =>
    invoke<PresetInfo[]>('list_presets', { monitorId: monitorId ?? null }),
  setAutoArrange: (monitorId, settings) =>
    invoke<AppState>('set_auto_arrange', { monitorId, settings }),
  setAppRule: (rule) => invoke<AppState>('set_app_rule', { rule }),
  removeAppRule: (exe) => invoke<AppState>('remove_app_rule', { exe }),
  arrangeMonitor: (monitorId) => invoke<void>('arrange_monitor', { monitorId }),
  probeShortcut: (action, chord) => invoke<ProbeResult>('probe_shortcut', { action, chord }),
  setShortcut: (action, chord) => invoke<AppState>('set_shortcut', { action, chord }),
  resetShortcuts: () => invoke<AppState>('reset_shortcuts'),
  setPaused: (paused) => invoke<void>('set_paused', { paused }),
  hidePopup: (label) => invoke<void>('hide_popup', { label }),
  openMain: (view) => invoke<void>('open_main', { view }),
  onPanelAnchor: (cb) => listen<PanelAnchor>('panel-anchor', (e) => cb(e.payload)),
  switcherMonitor: () => invoke<string | null>('switcher_monitor'),

  exportLayout: (layout) => invoke<string>('export_layout', { layout }),
  importLayout: (monitorId, json) => invoke<AppState>('import_layout', { monitorId, json }),

  editSplitZone: (layout, zoneId, axis, at) =>
    invoke<Layout>('edit_split_zone', { layout, zoneId, axis, at }),
  editRemoveZone: (layout, zoneId) => invoke<Layout>('edit_remove_zone', { layout, zoneId }),
  editMoveDivider: (layout, axis, from, to) =>
    invoke<Layout>('edit_move_divider', { layout, axis, from, to }),

  takePendingView: () => invoke<string | null>('take_pending_view'),
  onNavigate: (cb) => listen<NavigateTarget>('navigate', (e) => cb(e.payload)),
  onStateChanged: (cb) => listen<AppState>('state-changed', (e) => cb(e.payload)),
};

/** True when running inside the Tauri webview. */
export function insideTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

// Chosen once at startup. `vite --mode demo` (npm run demo) forces the mock,
// otherwise the mock is used whenever there is no Tauri runtime.
const mock: MockBackend | null =
  import.meta.env.MODE === 'demo' || !insideTauri() ? createMockBackend() : null;

/** The one and only way the UI talks to Rust. */
export const api: Backend = mock ?? tauriBackend;

/** Test/design-review hooks; only present in demo mode. */
export const demoControls: DemoControls | null = mock ? mock.demo : null;
