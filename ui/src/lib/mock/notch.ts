// The notch in demo mode: defaults and sanitising for `Settings.notch` (a mirror
// of krova-core's notch.rs lenient reading) and the fake `panel-anchor` the demo
// sends, as if the notch on the primary monitor had been clicked.

import type {
  MonitorSnapshot,
  NotchCorner,
  NotchMonitor,
  NotchSettings,
  PanelAnchor,
} from '../bindings';

export const NOTCH_CORNERS: readonly NotchCorner[] = [
  'top_center',
  'top_left',
  'top_right',
  'bottom_left',
  'bottom_right',
];
const NOTCH_MONITORS: readonly NotchMonitor[] = ['primary', 'cursor'];

export function defaultNotchSettings(): NotchSettings {
  return { enabled: true, corner: 'top_center', monitor: 'primary', hide_in_fullscreen: true };
}

/** Field by field: a missing or invalid value falls back to its default, like Rust. */
export function sanitizeNotch(n: unknown): NotchSettings {
  const d = defaultNotchSettings();
  if (!n || typeof n !== 'object') return d;
  const o = n as Record<string, unknown>;
  return {
    enabled: typeof o.enabled === 'boolean' ? o.enabled : d.enabled,
    corner: NOTCH_CORNERS.includes(o.corner as NotchCorner) ? (o.corner as NotchCorner) : d.corner,
    monitor: NOTCH_MONITORS.includes(o.monitor as NotchMonitor)
      ? (o.monitor as NotchMonitor)
      : d.monitor,
    hide_in_fullscreen:
      typeof o.hide_in_fullscreen === 'boolean' ? o.hide_in_fullscreen : d.hide_in_fullscreen,
  };
}

/** The expanded pill and its inset, logical px (krova-core's notch.rs). */
const EXPANDED = { w: 220, h: 28 };
const INSET = 4;

/**
 * Where the expanded notch sits at `corner` of `monitor` (physical px), the same
 * rule as `notch_rect` in Rust: the work-area edge, `INSET` in.
 */
export function demoAnchor(monitor: MonitorSnapshot, corner: NotchCorner): PanelAnchor {
  const s = monitor.scale > 0 ? monitor.scale : 1;
  const w = Math.round(EXPANDED.w * s);
  const h = Math.round(EXPANDED.h * s);
  const inset = Math.round(INSET * s);
  const r = monitor.work;
  const left = r.x + inset;
  const right = r.x + r.w - inset - w;
  const x =
    corner === 'top_center'
      ? r.x + Math.trunc((r.w - w) / 2)
      : corner === 'top_left' || corner === 'bottom_left'
        ? left
        : right;
  const y = corner.startsWith('top') ? r.y + inset : r.y + r.h - inset - h;
  return { x, y, w, h, corner, monitor_id: monitor.profile_id };
}
