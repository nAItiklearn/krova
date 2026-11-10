// Drawing maths for the Screen edges view: where the reserved bands sit on a
// monitor, as fractions of the whole monitor. Mirrors the rule in krova-core
// edges.rs (and the demo's applyReserved): sides are logical px times the scale,
// and an axis that would keep less than 200 physical px is ignored.

import type { PxRect, ReservedEdges } from './bindings';
import { MAX_RESERVED_PX, MIN_USABLE_PX } from './mock/wave4';

export type Side = keyof ReservedEdges;
export const SIDES: Side[] = ['top', 'right', 'bottom', 'left'];

/** One side, typed in a number box: rounded and kept in 0..400; junk is 0. */
export function cleanSide(raw: unknown): number {
  const v = typeof raw === 'string' ? Number(raw.trim()) : raw;
  if (typeof v !== 'number' || !Number.isFinite(v)) return 0;
  return Math.min(MAX_RESERVED_PX, Math.max(0, Math.round(v)));
}

export interface Bands {
  /** Fractions (0..1) of the monitor's width / height. */
  top: number;
  right: number;
  bottom: number;
  left: number;
  /** The axis is ignored because too little would be left. */
  ignoredX: boolean;
  ignoredY: boolean;
}

/** The reserved bands of `r` on a monitor whose area is `rect` at `scale`. */
export function reservedBands(rect: PxRect, scale: number, r: ReservedEdges): Bands {
  const s = Number.isFinite(scale) && scale > 0 ? Math.min(8, Math.max(0.5, scale)) : 1;
  const px = (v: number) => Math.round(cleanSide(v) * s);
  const [t, rt, b, l] = [px(r.top), px(r.right), px(r.bottom), px(r.left)];
  const w = Math.max(1, rect.w);
  const h = Math.max(1, rect.h);
  const ignoredX = (l > 0 || rt > 0) && w - l - rt < MIN_USABLE_PX;
  const ignoredY = (t > 0 || b > 0) && h - t - b < MIN_USABLE_PX;
  return {
    top: ignoredY ? 0 : t / h,
    bottom: ignoredY ? 0 : b / h,
    left: ignoredX ? 0 : l / w,
    right: ignoredX ? 0 : rt / w,
    ignoredX,
    ignoredY,
  };
}

/** True if nothing is reserved. */
export const noneReserved = (r: ReservedEdges): boolean => SIDES.every((s) => cleanSide(r[s]) === 0);
