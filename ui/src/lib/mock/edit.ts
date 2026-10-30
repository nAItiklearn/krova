// Faithful TypeScript mirror of crates/krova-core/src/edit.rs (split, merge-on-
// delete, divider move) plus the Layout::validate rules from zone.rs. Used only
// by the demo mock backend and its tests: in the real app the Rust commands
// are the single source of truth. Keep error strings, EPS and MIN_ZONE in sync
// with the Rust file.
//
// Like the Rust methods, the functions below mutate the layout they are given
// and guarantee that a failed edit leaves it untouched. The `edit*` wrappers at
// the bottom work on a copy, like the Tauri commands that take and return a
// Layout by value.

import type { Axis, Layout, Rect, Zone } from '../bindings';

/** Smallest zone side the editor allows, as a fraction of the work area. */
export const MIN_ZONE = 0.05;

export const EPS = 1e-6;

function approx(a: number, b: number): boolean {
  return Math.abs(a - b) < EPS;
}

/** Mirrors the `EditError` variants and their `Display` strings. */
export class EditError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'EditError';
  }
}

export const errors = {
  noSuchZone: (id: number) => new EditError(`zone ${id} not found`),
  tooSmall: () => new EditError('the change would leave a zone smaller than the minimum size'),
  noMergeTarget: (id: number) => new EditError(`zone ${id} has no neighbour it can merge into`),
  lastZone: () => new EditError('a layout needs at least one zone'),
  noDivider: () => new EditError('there is no divider at that position'),
};

/** Rect::is_valid: finite, positive size, inside the unit square (1e-9 slack). */
export function isValidRect(r: Rect): boolean {
  const e = 1e-9;
  return (
    [r.x, r.y, r.w, r.h].every((v) => Number.isFinite(v)) &&
    r.w > 0 &&
    r.h > 0 &&
    r.x >= -e &&
    r.y >= -e &&
    r.x + r.w <= 1 + e &&
    r.y + r.h <= 1 + e
  );
}

/** Layout::validate with the LayoutError display strings. */
export function validateLayout(layout: Layout): void {
  if (layout.zones.length === 0) throw new EditError('layout has no zones');
  const seen = new Set<number>();
  for (const z of layout.zones) {
    if (seen.has(z.id)) throw new EditError(`duplicate zone id ${z.id}`);
    seen.add(z.id);
    if (!isValidRect(z.rect)) throw new EditError(`zone ${z.id} has an invalid rectangle`);
  }
}

/** Union of two rects that share a full edge, if they do. */
function merged(a: Rect, b: Rect): Rect | null {
  if (
    approx(a.y, b.y) &&
    approx(a.h, b.h) &&
    (approx(a.x + a.w, b.x) || approx(b.x + b.w, a.x))
  ) {
    return { x: Math.min(a.x, b.x), y: a.y, w: a.w + b.w, h: a.h };
  }
  if (
    approx(a.x, b.x) &&
    approx(a.w, b.w) &&
    (approx(a.y + a.h, b.y) || approx(b.y + b.h, a.y))
  ) {
    return { x: a.x, y: Math.min(a.y, b.y), w: a.w, h: a.h + b.h };
  }
  return null;
}

function nextZoneId(layout: Layout): number {
  return layout.zones.reduce((m, z) => Math.max(m, z.id), 0) + 1;
}

/**
 * Split a zone at normalized position `at` along `axis`. The original zone
 * keeps the left/top part; the new zone (returned id) takes the rest.
 */
export function splitZone(layout: Layout, zoneId: number, axis: Axis, at: number): number {
  const idx = layout.zones.findIndex((z) => z.id === zoneId);
  if (idx < 0) throw errors.noSuchZone(zoneId);
  const r = layout.zones[idx].rect;
  let first: Rect;
  let second: Rect;
  if (axis === 'vertical') {
    if (at - r.x < MIN_ZONE || r.x + r.w - at < MIN_ZONE) throw errors.tooSmall();
    first = { x: r.x, y: r.y, w: at - r.x, h: r.h };
    second = { x: at, y: r.y, w: r.x + r.w - at, h: r.h };
  } else {
    if (at - r.y < MIN_ZONE || r.y + r.h - at < MIN_ZONE) throw errors.tooSmall();
    first = { x: r.x, y: r.y, w: r.w, h: at - r.y };
    second = { x: r.x, y: at, w: r.w, h: r.y + r.h - at };
  }
  const newId = nextZoneId(layout);
  layout.zones[idx].rect = first;
  layout.zones.push({ id: newId, rect: second });
  return newId;
}

/**
 * Delete a zone, giving its space to a neighbour that shares a full edge.
 * Returns the id of the zone that grew.
 */
export function removeZone(layout: Layout, zoneId: number): number {
  const idx = layout.zones.findIndex((z) => z.id === zoneId);
  if (idx < 0) throw errors.noSuchZone(zoneId);
  if (layout.zones.length === 1) throw errors.lastZone();
  const removed = layout.zones[idx].rect;
  let target = -1;
  let union: Rect | null = null;
  for (let i = 0; i < layout.zones.length; i++) {
    if (i === idx) continue;
    const u = merged(removed, layout.zones[i].rect);
    if (u) {
      target = i;
      union = u;
      break;
    }
  }
  if (target < 0 || !union) throw errors.noMergeTarget(zoneId);
  layout.zones[target].rect = union;
  const grown = layout.zones[target].id;
  layout.zones.splice(idx, 1);
  return grown;
}

/**
 * Move every zone edge lying on the divider at `from` to `to`. Edges on the
 * work-area border are not dividers and are rejected.
 */
export function moveDivider(layout: Layout, axis: Axis, from: number, to: number): void {
  const next: Zone[] = layout.zones.map((z) => ({ id: z.id, rect: { ...z.rect } }));
  let before = 0;
  let after = 0;
  for (const z of next) {
    const r = z.rect;
    // Work on the (position, length) pair for the chosen axis.
    const pos = axis === 'vertical' ? r.x : r.y;
    const len = axis === 'vertical' ? r.w : r.h;
    let newPos = pos;
    let newLen = len;
    if (approx(pos + len, from)) {
      newLen = to - pos;
      before += 1;
    } else if (approx(pos, from)) {
      const end = pos + len;
      newPos = to;
      newLen = end - to;
      after += 1;
    }
    if (axis === 'vertical') {
      r.x = newPos;
      r.w = newLen;
    } else {
      r.y = newPos;
      r.h = newLen;
    }
  }
  if (before === 0 || after === 0) throw errors.noDivider();
  const tooSmall = (z: Zone): boolean => {
    const side = axis === 'vertical' ? z.rect.w : z.rect.h;
    return side < MIN_ZONE - EPS || !isValidRect(z.rect);
  };
  if (next.some(tooSmall)) throw errors.tooSmall();
  layout.zones = next;
}

function copy(layout: Layout): Layout {
  return {
    id: layout.id,
    name: layout.name,
    zones: layout.zones.map((z) => ({ id: z.id, rect: { ...z.rect } })),
  };
}

/** edit_split_zone: returns the new layout; the argument is not modified. */
export function editSplitZone(layout: Layout, zoneId: number, axis: Axis, at: number): Layout {
  const out = copy(layout);
  splitZone(out, zoneId, axis, at);
  return out;
}

/** edit_remove_zone: returns the new layout; the argument is not modified. */
export function editRemoveZone(layout: Layout, zoneId: number): Layout {
  const out = copy(layout);
  removeZone(out, zoneId);
  return out;
}

/** edit_move_divider: returns the new layout; the argument is not modified. */
export function editMoveDivider(layout: Layout, axis: Axis, from: number, to: number): Layout {
  const out = copy(layout);
  moveDivider(out, axis, from, to);
  return out;
}
