// Maths for the hover zone picker: the monitor's zones (physical px) drawn as a
// small map, and where that map goes next to the cursor.

import type { HoverPicker, HoverZone, PxRect } from './bindings';

/** Union of the zones: the area the picker map stands for. */
export function zonesBounds(zones: HoverZone[]): PxRect {
  if (zones.length === 0) return { x: 0, y: 0, w: 1, h: 1 };
  let l = Infinity;
  let t = Infinity;
  let r = -Infinity;
  let b = -Infinity;
  for (const z of zones) {
    l = Math.min(l, z.rect.x);
    t = Math.min(t, z.rect.y);
    r = Math.max(r, z.rect.x + z.rect.w);
    b = Math.max(b, z.rect.y + z.rect.h);
  }
  return { x: l, y: t, w: Math.max(1, r - l), h: Math.max(1, b - t) };
}

export interface MapZone {
  id: number;
  /** Fractions (0..1) of the map. */
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface PickerMap {
  /** Map size in CSS px, fitted into maxW x maxH with the monitor's aspect ratio. */
  width: number;
  height: number;
  zones: MapZone[];
}

export function pickerMap(zones: HoverZone[], maxW: number, maxH: number): PickerMap {
  const b = zonesBounds(zones);
  const k = Math.min(maxW / b.w, maxH / b.h);
  return {
    width: Math.round(b.w * k),
    height: Math.round(b.h * k),
    zones: zones.map((z) => ({
      id: z.id,
      x: (z.rect.x - b.x) / b.w,
      y: (z.rect.y - b.y) / b.h,
      w: z.rect.w / b.w,
      h: z.rect.h / b.h,
    })),
  };
}

/** Where the cursor is, as a fraction of the zones' area (clamped to 0..1). */
export function cursorFraction(p: HoverPicker): { fx: number; fy: number } {
  const b = zonesBounds(p.zones);
  const c = (v: number) => Math.min(1, Math.max(0, v));
  return { fx: c((p.cursor[0] - b.x) / b.w), fy: c((p.cursor[1] - b.y) / b.h) };
}

/**
 * Top-left corner for a `w` x `h` card beside the point (x, y): below-right by
 * `gap`, flipped to the other side when it would leave the `vw` x `vh` viewport,
 * and kept `margin` inside it.
 */
export function placeNear(
  x: number,
  y: number,
  w: number,
  h: number,
  vw: number,
  vh: number,
  gap = 14,
  margin = 8,
): { left: number; top: number } {
  let left = x + gap;
  let top = y + gap;
  if (left + w > vw - margin) left = x - gap - w;
  if (top + h > vh - margin) top = y - gap - h;
  left = Math.min(Math.max(margin, left), Math.max(margin, vw - margin - w));
  top = Math.min(Math.max(margin, top), Math.max(margin, vh - margin - h));
  return { left: Math.round(left), top: Math.round(top) };
}

/** Keys 1..9 pick the n-th zone (in the order the backend sent them). */
export function zoneForKey(key: string, zones: HoverZone[]): number | null {
  if (!/^[1-9]$/.test(key)) return null;
  return zones[Number(key) - 1]?.id ?? null;
}
