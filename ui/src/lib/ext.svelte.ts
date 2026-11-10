// Settings that the new screens use before every backend branch has landed.
// Each key is read from the backend's Settings when it has that field (the real
// contract name), otherwise from localStorage, so the UI works in demo mode and
// switches to the real backend automatically once the field exists.

import { app } from './app.svelte';
import { settings } from './settings.svelte';

export type HotCornerAction =
  | 'switcher'
  | 'cheat_sheet'
  | 'focus'
  | 'calm_desk'
  | 'pause'
  | 'show_zones'
  | 'panel';
export type Corner = 'top_left' | 'top_right' | 'bottom_left' | 'bottom_right';
export type NotchPlace = 'top_center' | Corner;
export type HighlightMode = 'none' | 'active' | 'all' | 'glow';

export interface Appearance {
  accent: string;
  corner_radius_px: number;
  border_width_px: number;
}
export interface HotCorners {
  top_left: HotCornerAction | null;
  top_right: HotCornerAction | null;
  bottom_left: HotCornerAction | null;
  bottom_right: HotCornerAction | null;
  dwell_ms: number;
  ignore_fullscreen: boolean;
}
export interface Notch {
  enabled: boolean;
  corner: NotchPlace;
  monitor: 'primary' | 'cursor';
  hide_in_fullscreen: boolean;
}
export interface AutoArrange {
  enabled: boolean;
  policy: 'next_free' | 'main_first';
  fill_gaps: boolean;
  stack_when_full: boolean;
}
export interface Highlight {
  mode: HighlightMode;
  width_px: number;
}
export type AppRuleAction = 'never_touch' | 'float' | 'preferred_zone';
export interface AppRule {
  exe: string;
  action: AppRuleAction;
  zone?: number;
}

export interface Ext {
  appearance: Appearance;
  hot_corners: HotCorners;
  notch: Notch;
  auto_arrange: AutoArrange;
  highlight: Highlight;
  shortcuts: Record<string, string | null>;
  app_rules: AppRule[];
  focus_follows_mouse: boolean;
  paused: boolean;
}

export const EXT_DEFAULTS: Ext = {
  appearance: { accent: '#ff5b1f', corner_radius_px: 10, border_width_px: 2 },
  hot_corners: {
    top_left: null,
    top_right: 'switcher',
    bottom_left: null,
    bottom_right: 'calm_desk',
    dwell_ms: 300,
    ignore_fullscreen: true,
  },
  notch: { enabled: true, corner: 'top_center', monitor: 'primary', hide_in_fullscreen: true },
  auto_arrange: { enabled: true, policy: 'next_free', fill_gaps: true, stack_when_full: true },
  highlight: { mode: 'active', width_px: 2 },
  shortcuts: {},
  app_rules: [
    { exe: 'Calculator.exe', action: 'float' },
    { exe: 'Spotify.exe', action: 'preferred_zone', zone: 3 },
  ],
  focus_follows_mouse: false,
  paused: false,
};

const KEY = 'krova.ext.v1';

function loadLocal(): Partial<Ext> {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? '{}') as Partial<Ext>;
  } catch {
    return {};
  }
}

class ExtStore {
  private local = $state<Partial<Ext>>(typeof localStorage === 'undefined' ? {} : loadLocal());

  constructor() {
    // Another Krova window (the main window or the quick panel) changed a value.
    if (typeof window !== 'undefined') {
      window.addEventListener('storage', (e) => {
        if (e.key === KEY) this.local = loadLocal();
      });
    }
  }

  get<K extends keyof Ext>(key: K): Ext[K] {
    const fromBackend = (settings.draft ?? app.state?.config.settings) as Record<string, unknown> | undefined;
    if (fromBackend && key in fromBackend) return fromBackend[key] as Ext[K];
    return (this.local[key] ?? EXT_DEFAULTS[key]) as Ext[K];
  }

  set<K extends keyof Ext>(key: K, value: Ext[K]): void {
    const fromBackend = (settings.draft ?? app.state?.config.settings) as Record<string, unknown> | undefined;
    if (fromBackend && key in fromBackend) {
      settings.update({ [key]: value } as never);
      return;
    }
    this.local = { ...this.local, [key]: value };
    try {
      localStorage.setItem(KEY, JSON.stringify(this.local));
    } catch {
      // storage may be unavailable; the value still applies for this session
    }
  }

  patch<K extends keyof Ext>(key: K, part: Partial<Ext[K]>): void {
    this.set(key, { ...(this.get(key) as object), ...(part as object) } as Ext[K]);
  }
}

export const ext = new ExtStore();
