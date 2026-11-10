// Settings that the new screens read and write, in the UI's shapes.
//
// Each key goes to the real backend when the backend has it:
// - `appearance`, `hot_corners`, `notch`, `highlight`, `focus_follows_mouse`:
//   fields of `Settings`, saved with the debounced settings form;
// - `auto_arrange`: per monitor (`MonitorProfile.auto_arrange`); the UI shows the
//   first connected monitor's and `set_auto_arrange` writes every connected one;
// - `app_rules`: `Settings.app_rules`, changed with `set_app_rule` /
//   `remove_app_rule` (the UI's `{ action, zone }` is the backend's `RuleAction`);
// - `shortcuts`: the backend's actions (`AppState.shortcut_actions`, `set_shortcut`);
//   keys the backend has no action for (`workspace:<id>`, `calm_desk`) stay local;
// - `paused`: `AppState.paused`, changed with `set_paused`.
// Otherwise (an older backend) the value lives in localStorage.

import { app } from './app.svelte';
import { api, errorMessage } from './backend';
import type { AppRule as BackendRule, AppState, Settings } from './bindings';
import { backendChords, FAMILIES, membersOf, SINGLES, uiChord } from './shortcutMap';
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
  /** `#rrggbb`, or null for the accent colour. */
  color?: string | null;
  width_px: number;
}
export type AppRuleAction = 'never_touch' | 'float' | 'preferred_zone';
export interface AppRule {
  exe: string;
  action: AppRuleAction;
  zone?: number;
}

/** Zones are numbered 1..64 in reading order (krova-core app_rules). */
export const MAX_RULE_ZONE = 64;

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

/** The backend's rule as the UI's rule; `start_on_monitor` rules are not shown. */
export function ruleToUi(r: BackendRule): AppRule | null {
  switch (r.action.kind) {
    case 'never_touch':
    case 'float':
      return { exe: r.exe, action: r.action.kind };
    case 'preferred_zone':
      return { exe: r.exe, action: 'preferred_zone', zone: r.action.value };
    default:
      return null;
  }
}

/** The UI's rule as the backend's. */
export function ruleToBackend(r: AppRule): BackendRule {
  if (r.action === 'preferred_zone') {
    const zone = Math.min(MAX_RULE_ZONE, Math.max(1, Math.round(r.zone ?? 1)));
    return { exe: r.exe, action: { kind: 'preferred_zone', value: zone } };
  }
  return { exe: r.exe, action: { kind: r.action } };
}

const sameExe = (a: string, b: string): boolean => a.toLowerCase() === b.toLowerCase();

/** Ids of the sheet rows the backend knows (families and single actions). */
const BACKED = new Set([...Object.keys(FAMILIES), ...Object.keys(SINGLES)]);

class ExtStore {
  private local = $state<Partial<Ext>>(typeof localStorage === 'undefined' ? {} : loadLocal());
  /** The pause the user asked for, until the backend's `state-changed` confirms it. */
  private pauseWish = $state<{ value: boolean; at: number } | null>(null);
  /** The last error of a command sent from here (null after a success). */
  error = $state<string | null>(null);

  constructor() {
    // Another Krova window (the main window or the quick panel) changed a value.
    if (typeof window !== 'undefined') {
      window.addEventListener('storage', (e) => {
        if (e.key === KEY) this.local = loadLocal();
      });
    }
  }

  /** The settings form (unsaved edits included), for the fields it saves. */
  private formSettings(): Record<string, unknown> | undefined {
    return (settings.draft ?? app.state?.config.settings) as Record<string, unknown> | undefined;
  }

  /** Settings as the backend has them now, for the fields saved by their own commands. */
  private savedSettings(): Settings | undefined {
    return app.state?.config.settings;
  }

  private hasRules(): boolean {
    return Array.isArray(this.savedSettings()?.app_rules);
  }

  private firstMonitorArrange(state: AppState | null): AutoArrange | undefined {
    if (!state) return undefined;
    const ids = state.monitors.map((m) => m.profile_id);
    const profile =
      state.config.monitors.find((p) => p.id === ids[0]) ?? state.config.monitors.find((p) => ids.includes(p.id));
    return profile?.auto_arrange;
  }

  private shortcutRows() {
    const rows = app.state?.shortcut_actions;
    return rows && rows.length > 0 ? rows : null;
  }

  get<K extends keyof Ext>(key: K): Ext[K] {
    const state = app.state;
    switch (key) {
      case 'paused': {
        if (state && typeof state.paused === 'boolean') {
          const wish = this.pauseWish;
          if (wish && wish.value !== state.paused && Date.now() - wish.at < 2000) return wish.value as Ext[K];
          return state.paused as Ext[K];
        }
        break;
      }
      case 'auto_arrange': {
        const a = this.firstMonitorArrange(state);
        if (a) return { ...a } as Ext[K];
        break;
      }
      case 'app_rules': {
        const saved = this.savedSettings();
        if (saved && Array.isArray(saved.app_rules)) {
          return saved.app_rules.map(ruleToUi).filter((r): r is AppRule => r !== null) as Ext[K];
        }
        break;
      }
      case 'shortcuts': {
        const rows = this.shortcutRows();
        if (rows) {
          const out: Record<string, string | null> = {};
          for (const [k, v] of Object.entries(this.local.shortcuts ?? {})) {
            if (!BACKED.has(k)) out[k] = v;
          }
          for (const id of BACKED) {
            const c = uiChord(id, rows);
            if (c !== undefined) out[id] = c;
          }
          // No backend action runs calm desk yet: show it without keys.
          out.calm_desk = null;
          return out as Ext[K];
        }
        break;
      }
      default: {
        const fromBackend = this.formSettings();
        if (fromBackend && key in fromBackend) return fromBackend[key] as Ext[K];
      }
    }
    return (this.local[key] ?? EXT_DEFAULTS[key]) as Ext[K];
  }

  set<K extends keyof Ext>(key: K, value: Ext[K]): void {
    const state = app.state;
    switch (key) {
      case 'paused':
        if (state && typeof state.paused === 'boolean') {
          this.pauseWish = { value: value as boolean, at: Date.now() };
          void this.run(api.setPaused(value as boolean));
          return;
        }
        break;
      case 'auto_arrange':
        if (this.firstMonitorArrange(state)) {
          void this.setArrange(value as AutoArrange);
          return;
        }
        break;
      case 'app_rules':
        if (this.hasRules()) {
          void this.setRules(value as AppRule[]);
          return;
        }
        break;
      case 'shortcuts':
        if (this.shortcutRows()) {
          void this.setShortcuts(value as Record<string, string | null>);
          return;
        }
        break;
      default: {
        const fromBackend = this.formSettings();
        if (fromBackend && key in fromBackend) {
          settings.update({ [key]: value } as never);
          return;
        }
      }
    }
    this.setLocal(key, value);
  }

  patch<K extends keyof Ext>(key: K, part: Partial<Ext[K]>): void {
    this.set(key, { ...(this.get(key) as object), ...(part as object) } as Ext[K]);
  }

  private setLocal<K extends keyof Ext>(key: K, value: Ext[K]): void {
    this.local = { ...this.local, [key]: value };
    try {
      localStorage.setItem(KEY, JSON.stringify(this.local));
    } catch {
      // storage may be unavailable; the value still applies for this session
    }
  }

  /** Run a command; adopt the state it returns. False (and `error` set) on failure. */
  async run(p: Promise<AppState | void>): Promise<boolean> {
    try {
      const s = await p;
      if (s) app.setState(s);
      this.error = null;
      return true;
    } catch (e) {
      this.error = errorMessage(e);
      return false;
    }
  }

  private async setArrange(value: AutoArrange): Promise<void> {
    const ids = app.state?.monitors.map((m) => m.profile_id) ?? [];
    const next = {
      enabled: value.enabled,
      policy: value.policy,
      fill_gaps: value.fill_gaps,
      stack_when_full: value.stack_when_full,
    };
    for (const id of ids) {
      if (!(await this.run(api.setAutoArrange(id, next)))) return;
    }
  }

  private async setRules(next: AppRule[]): Promise<void> {
    const before = this.savedSettings()?.app_rules ?? [];
    // Removed rules (start_on_monitor rules are not shown, so never removed here).
    for (const old of before) {
      if (old.action.kind === 'start_on_monitor') continue;
      if (!next.some((r) => sameExe(r.exe, old.exe))) {
        if (!(await this.run(api.removeAppRule(old.exe)))) return;
      }
    }
    for (const r of next) {
      const want = ruleToBackend(r);
      const old = before.find((b) => sameExe(b.exe, r.exe));
      if (old && JSON.stringify(old.action) === JSON.stringify(want.action)) continue;
      if (!(await this.run(api.setAppRule(want)))) return;
    }
  }

  private async setShortcuts(next: Record<string, string | null>): Promise<void> {
    const rows = this.shortcutRows() ?? [];
    const localPart: Record<string, string | null> = {};
    for (const [k, v] of Object.entries(next)) if (!BACKED.has(k) && k !== 'calm_desk') localPart[k] = v;
    this.setLocal('shortcuts', localPart);
    // No backend action in the map: "reset all".
    if (Object.keys(next).every((k) => !BACKED.has(k))) {
      await this.run(api.resetShortcuts());
      return;
    }
    for (const id of BACKED) {
      if (!(id in next) || membersOf(id).length === 0) continue;
      if (uiChord(id, rows) === next[id]) continue;
      for (const [action, chord] of backendChords(id, next[id])) {
        if (!(await this.run(api.setShortcut(action, chord)))) return;
      }
    }
  }
}

export const ext = new ExtStore();
