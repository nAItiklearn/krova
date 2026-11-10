// Demo-mode half of hot corners: defaults and sanitising that mirror
// krova-core's corners.rs, and a tiny event bus standing in for the backend's
// `hot-corner` event. Kept apart from mockBackend.ts so the feature touches that
// file with a few lines only.

import type {
  HotCorner,
  HotCornerAction,
  HotCornerEvent,
  HotCornerSettings,
  HotCornerStatus,
  Settings,
} from '../bindings';

export const HOT_CORNER_ACTIONS: readonly HotCornerAction[] = [
  'switcher',
  'cheat_sheet',
  'focus',
  'calm_desk',
  'pause',
  'show_zones',
  'panel',
];
export const HOT_CORNERS: readonly HotCorner[] = [
  'top_left',
  'top_right',
  'bottom_left',
  'bottom_right',
];
export const MIN_DWELL_MS = 150;
export const MAX_DWELL_MS = 1000;

export function defaultHotCorners(): HotCornerSettings {
  return {
    top_left: null,
    top_right: 'switcher',
    bottom_left: null,
    bottom_right: 'calm_desk',
    dwell_ms: 300,
    ignore_fullscreen: true,
  };
}

const isAction = (v: unknown): v is HotCornerAction =>
  typeof v === 'string' && (HOT_CORNER_ACTIONS as readonly string[]).includes(v);

/**
 * Like the Rust loader: never fails. An unknown action is "no action", a missing
 * field keeps its default, the dwell is rounded and clamped to 150..1000.
 */
export function sanitizeHotCorners(raw: unknown): HotCornerSettings {
  const d = defaultHotCorners();
  if (!raw || typeof raw !== 'object') return d;
  const r = raw as Record<string, unknown>;
  const action = (key: HotCorner): HotCornerAction | null =>
    !(key in r) ? d[key] : isAction(r[key]) ? r[key] : null;
  const dwell =
    typeof r.dwell_ms === 'number' && Number.isFinite(r.dwell_ms)
      ? Math.min(MAX_DWELL_MS, Math.max(MIN_DWELL_MS, Math.round(r.dwell_ms)))
      : d.dwell_ms;
  return {
    top_left: action('top_left'),
    top_right: action('top_right'),
    bottom_left: action('bottom_left'),
    bottom_right: action('bottom_right'),
    dwell_ms: dwell,
    ignore_fullscreen:
      typeof r.ignore_fullscreen === 'boolean' ? r.ignore_fullscreen : d.ignore_fullscreen,
  };
}

/** What the demo reports for `hotCornersStatus()`. */
export function hotCornersStatus(settings: Settings, supported: boolean): HotCornerStatus {
  const hc = settings.hot_corners ?? defaultHotCorners();
  const any = HOT_CORNERS.some((c) => hc[c] !== null);
  return { supported, active: supported && settings.enabled && any };
}

/**
 * The event a corner would send in the demo, or null when that corner is not
 * set, Krova is off or the platform cannot watch corners (like the real app).
 */
export function hotCornerEvent(
  settings: Settings,
  corner: HotCorner,
  monitorId: string | null,
  supported: boolean,
): HotCornerEvent | null {
  const action = (settings.hot_corners ?? defaultHotCorners())[corner];
  if (!supported || !settings.enabled || action === null) return null;
  return { corner, action, monitor_id: monitorId };
}
