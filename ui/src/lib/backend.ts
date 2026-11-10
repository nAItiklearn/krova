// The single typed seam between the UI and Rust. Components and stores import
// `api` from here and nothing else talks to Tauri. In a plain browser (or with
// `vite --mode demo`) `api` is the in-memory mock from ./mock/mockBackend.
//
// Tauri v2: JS argument names are camelCase, the Rust command parameters are
// snake_case; Tauri converts between them.

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { AppState, Axis, Layout, NavigateTarget, PanelAnchor, Settings } from './bindings';
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

  // --- notch ---
  /**
   * Backend event "panel-anchor": the notch was clicked and the `panel` window was
   * placed next to it. In demo mode a fake anchor (the notch on the primary monitor)
   * is sent right after subscribing, and `demoControls.clickNotch()` sends more.
   */
  onPanelAnchor(cb: (anchor: PanelAnchor) => void): Promise<Unlisten>;
  /** The last anchor, for a panel page that missed the event (null if none). */
  panelAnchor(): Promise<PanelAnchor | null>;
  /** Hide the panel window (Esc). It stays warm for the next click. */
  hidePanel(): Promise<void>;
  // --- notch ---
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
  deleteLayout: (monitorId, layoutId) =>
    invoke<AppState>('delete_layout', { monitorId, layoutId }),
  setActiveLayout: (monitorId, layoutId) =>
    invoke<AppState>('set_active_layout', { monitorId, layoutId }),
  moveLayout: (monitorId, layoutId, toIndex) =>
    invoke<AppState>('move_layout', { monitorId, layoutId, toIndex }),
  insertLayout: (monitorId, layout, index, activate) =>
    invoke<AppState>('insert_layout', { monitorId, layout, index, activate }),
  listTemplates: () => invoke<Layout[]>('list_templates'),
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

  // --- notch ---
  onPanelAnchor: (cb) => listen<PanelAnchor>('panel-anchor', (e) => cb(e.payload)),
  panelAnchor: () => invoke<PanelAnchor | null>('panel_anchor'),
  hidePanel: () => invoke<void>('hide_panel'),
  // --- notch ---
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
