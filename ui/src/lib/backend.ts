// The single typed seam between the UI and Rust. Components and stores import
// `api` from here and nothing else talks to Tauri. In a plain browser (or with
// `vite --mode demo`) `api` is the in-memory mock from ./mock/mockBackend.
//
// Tauri v2: JS argument names are camelCase, the Rust command parameters are
// snake_case; Tauri converts between them.

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { AppState, Axis, Layout, Settings, View } from './bindings';
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
  listTemplates(): Promise<Layout[]>;

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

  /** Backend event "navigate" (from the tray menu). */
  onNavigate(cb: (view: View) => void): Promise<Unlisten>;
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
  deleteLayout: (monitorId, layoutId) =>
    invoke<AppState>('delete_layout', { monitorId, layoutId }),
  setActiveLayout: (monitorId, layoutId) =>
    invoke<AppState>('set_active_layout', { monitorId, layoutId }),
  listTemplates: () => invoke<Layout[]>('list_templates'),

  exportLayout: (layout) => invoke<string>('export_layout', { layout }),
  importLayout: (monitorId, json) => invoke<AppState>('import_layout', { monitorId, json }),

  editSplitZone: (layout, zoneId, axis, at) =>
    invoke<Layout>('edit_split_zone', { layout, zoneId, axis, at }),
  editRemoveZone: (layout, zoneId) => invoke<Layout>('edit_remove_zone', { layout, zoneId }),
  editMoveDivider: (layout, axis, from, to) =>
    invoke<Layout>('edit_move_divider', { layout, axis, from, to }),

  onNavigate: (cb) => listen<View>('navigate', (e) => cb(e.payload)),
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
