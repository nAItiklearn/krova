// UI-side geometry helpers: hit-testing, divider discovery and cosmetic previews.
// These only drive rendering and pointer feedback. Every actual edit goes through
// the backend (edit_split_zone / edit_remove_zone / edit_move_divider), which
// enforces the real rules.

import type { Axis, Layout, PxRect, Rect, Zone } from './bindings';

/** Smallest zone side, as a fraction of the work area (same as krova-core). */
export const MIN_ZONE = 0.05;
const EPS = 1e-6;

const approx = (a: number, b: number): boolean => Math.abs(a - b) < EPS;
export const clamp = (v: number, lo: number, hi: number): number => Math.min(Math.max(v, lo), hi);

/** Position/length of a rect along an axis (vertical divider => x/w). */
function span(r: Rect, axis: Axis): { pos: number; len: number } {
  return axis === 'vertical' ? { pos: r.x, len: r.w } : { pos: r.y, len: r.h };
}

/** Position/length of a rect along the axis perpendicular to `axis`. */
function cross(r: Rect, axis: Axis): { pos: number; len: number } {
  return axis === 'vertical' ? { pos: r.y, len: r.h } : { pos: r.x, len: r.w };
}

/** Layout::hit_test: half-open containment, smaller area wins, then lower id. */
export function hitTest(layout: Layout, nx: number, ny: number): Zone | null {
  let best: Zone | null = null;
  for (const z of layout.zones) {
    const r = z.rect;
    if (!(nx >= r.x && nx < r.x + r.w && ny >= r.y && ny < r.y + r.h)) continue;
    if (
      best === null ||
      r.w * r.h < best.rect.w * best.rect.h ||
      (r.w * r.h === best.rect.w * best.rect.h && z.id < best.id)
    ) {
      best = z;
    }
  }
  return best;
}

/** One drawn piece of a divider: a stretch of shared edge along the cross axis. */
export interface DividerSegment {
  /** Start of the stretch along the cross axis (normalized). */
  from: number;
  /** End of the stretch along the cross axis (normalized). */
  to: number;
}

/**
 * A divider is a coordinate where at least one zone ends and one begins (like
 * the backend: edges on the work-area border are not dividers). The backend
 * moves every aligned edge at that coordinate, so the whole coordinate is one
 * divider even when it is drawn as several disjoint segments.
 */
export interface Divider {
  axis: Axis;
  /** Coordinate along `axis` (x for vertical, y for horizontal). */
  pos: number;
  segments: DividerSegment[];
  /** Smallest/largest position that keeps every neighbour >= MIN_ZONE. */
  lo: number;
  hi: number;
  /** Stable key, usable in keyed each blocks and as a DOM data attribute. */
  key: string;
}

export function dividerKey(axis: Axis, pos: number): string {
  return `${axis}:${Math.round(pos * 1e6)}`;
}

export function computeDividers(layout: Layout): Divider[] {
  const out: Divider[] = [];
  for (const axis of ['vertical', 'horizontal'] as const) {
    // Cluster the candidate coordinates (zone starts and ends), skipping the border.
    const coords: number[] = [];
    for (const z of layout.zones) {
      const { pos, len } = span(z.rect, axis);
      for (const c of [pos, pos + len]) {
        if (c <= EPS || c >= 1 - EPS) continue;
        if (!coords.some((k) => approx(k, c))) coords.push(c);
      }
    }
    coords.sort((a, b) => a - b);
    for (const c of coords) {
      const before = layout.zones.filter((z) => approx(span(z.rect, axis).pos + span(z.rect, axis).len, c));
      const after = layout.zones.filter((z) => !before.includes(z) && approx(span(z.rect, axis).pos, c));
      if (before.length === 0 || after.length === 0) continue;

      // Merge the touching edges into drawable segments.
      const spans = [...before, ...after]
        .map((z) => {
          const s = cross(z.rect, axis);
          return { from: s.pos, to: s.pos + s.len };
        })
        .sort((a, b) => a.from - b.from);
      const segments: DividerSegment[] = [];
      for (const s of spans) {
        const last = segments[segments.length - 1];
        if (last && s.from <= last.to + EPS) last.to = Math.max(last.to, s.to);
        else segments.push({ ...s });
      }

      const lo = Math.max(...before.map((z) => span(z.rect, axis).pos + MIN_ZONE));
      const hi = Math.min(...after.map((z) => span(z.rect, axis).pos + span(z.rect, axis).len - MIN_ZONE));
      out.push({ axis, pos: c, segments, lo, hi, key: dividerKey(axis, c) });
    }
  }
  return out;
}

/**
 * Cosmetic preview of dragging a divider: same arithmetic as the backend but
 * lenient (no validation), so zones can be drawn live while the pointer moves.
 */
export function previewMoveDivider(layout: Layout, axis: Axis, from: number, to: number): Layout {
  return {
    ...layout,
    zones: layout.zones.map((z) => {
      const r = { ...z.rect };
      const { pos, len } = span(r, axis);
      if (approx(pos + len, from)) {
        if (axis === 'vertical') r.w = to - pos;
        else r.h = to - pos;
      } else if (approx(pos, from)) {
        const end = pos + len;
        if (axis === 'vertical') {
          r.x = to;
          r.w = end - to;
        } else {
          r.y = to;
          r.h = end - to;
        }
      }
      return { id: z.id, rect: r };
    }),
  };
}

/** Round to 0.5% steps so positions stay tidy (0.4 rather than 0.40123...). */
export function tidy(v: number): number {
  return Math.round(v * 200) / 200;
}

/** PxRect::denormalize: each edge is rounded on its own, so zones tile exactly. */
export function zonePixels(work: PxRect, r: Rect): PxRect {
  const edge = (origin: number, size: number, n: number) => origin + Math.round(n * size);
  const left = edge(work.x, work.w, r.x);
  const top = edge(work.y, work.h, r.y);
  const right = edge(work.x, work.w, r.x + r.w);
  const bottom = edge(work.y, work.h, r.y + r.h);
  return { x: left, y: top, w: right - left, h: bottom - top };
}

/** 0.4 -> "40%", 0.3333 -> "33.3%", 0.5 -> "50%". */
export function pct(v: number): string {
  const p = Math.round(v * 1000) / 10;
  return `${Number.isInteger(p) ? p.toFixed(0) : p.toFixed(1)}%`;
}

/** CSS percentage geometry for an absolutely positioned normalized rect. */
export function rectStyle(r: Rect): string {
  return `left:${r.x * 100}%;top:${r.y * 100}%;width:${r.w * 100}%;height:${r.h * 100}%`;
}
