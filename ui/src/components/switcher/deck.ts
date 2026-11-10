// Deck geometry and gesture rules of the layout switcher (pure, unit-tested in
// deck.test.ts). Positions are in "slots": slot i is the i-th layout, `scroll`
// is the (fractional) slot in focus.
//
// d = scroll - slot. The card in focus (d = 0) is in front and fully visible.
// The layouts after it (d < 0) stack behind it, each showing its title strip,
// a little smaller and darker per step. Cards already flipped past (d > 0)
// slide down and fade out below.

import { clamp } from '../../lib/motion';

/** Vertical offset between the title strips of the cards stacked behind (px). */
export const STEP_BEHIND = 50;
/** Pointer travel that flips one card (px). */
export const DRAG_PX = 120;
/** Room kept free at the bottom for the key hints (px). */
export const HINTS_H = 96;
/** How many cards behind the focused one are drawn before they fade out. */
export const VISIBLE_BEHIND = 3.4;

/** A sideways swipe past this distance (px) or speed (px/ms) deletes the card. */
export const DISMISS_DISTANCE = 140;
export const DISMISS_SPEED = 0.7;
/** A press shorter than this (ms) without moving is a tap (apply). */
export const TAP_MS = 450;
/** Pointer travel (px) before a press becomes a drag. */
export const SLOP = 6;

export interface Placement {
  /** px from the focus line */
  y: number;
  /** scale */
  s: number;
  /** opacity 0..1 */
  o: number;
  /** darkening overlay 0..1 */
  shade: number;
}

/** How far a flipped-past card travels per slot (px). */
export const below = (cardH: number): number => cardH * 0.9;

export function place(d: number, cardH: number): Placement {
  if (d >= 0) {
    return { y: d * below(cardH), s: 1 - Math.min(d, 1) * 0.04, o: Math.max(0, 1 - d * 1.1), shade: 0 };
  }
  const t = -d;
  return {
    y: -t * STEP_BEHIND,
    s: Math.max(0.82, 1 - t * 0.045),
    o: Math.max(0, 1 - Math.max(0, t - VISIBLE_BEHIND) * 0.8),
    shade: Math.min(0.5, t * 0.14),
  };
}

/** Inverse of `place(d).y`: the `d` at which a card sits `y` px from the focus line. */
export function unplace(y: number, cardH: number): number {
  return y >= 0 ? y / below(cardH) : y / STEP_BEHIND;
}

/** The y (px, from the stage top) of the focused card's top edge. */
export function focusY(stageH: number, cardH: number): number {
  return Math.max(
    120 + 4 * STEP_BEHIND,
    Math.round((stageH - HINTS_H - cardH) * 0.62 + 4 * STEP_BEHIND * 0.38),
  );
}

/** Slot a lifted card is over when its top edge is at `y` (stage px). */
export function slotAt(y: number, scroll: number, stageH: number, cardH: number, count: number): number {
  return clamp(Math.round(scroll - unplace(y - focusY(stageH, cardH), cardH)), 0, Math.max(0, count - 1));
}

export type Gesture = 'pending' | 'scroll' | 'swipe' | 'reorder';

/**
 * What a press turns into once it moved `(dx, dy)`: still `pending` inside the
 * slop, a sideways `swipe` on a card (only when there is more than one layout:
 * the last one cannot be deleted), otherwise a vertical `scroll`.
 */
export function classify(dx: number, dy: number, onCard: boolean, count: number): Gesture {
  if (Math.hypot(dx, dy) <= SLOP) return 'pending';
  return onCard && count > 1 && Math.abs(dx) > Math.abs(dy) * 1.2 ? 'swipe' : 'scroll';
}

/** Does a swipe that ended at `x` (px) with velocity `vx` (px/ms) delete the card? */
export function dismisses(x: number, vx: number): boolean {
  return Math.abs(x) > DISMISS_DISTANCE || Math.abs(vx) > DISMISS_SPEED;
}

/** Red edge strength while swiping (0..1). */
export const dismissHint = (x: number): number => clamp((Math.abs(x) - 60) / 120, 0, 1);

/** Opacity factor of a card swiped `x` px sideways. */
export const swipeFade = (x: number): number => 1 - Math.min(1, Math.abs(x) / 520);

/** Tilt (deg) of a card swiped `x` px sideways. */
export const swipeTilt = (x: number): number => x / 95;

/** Copy of `list` with the item at `from` moved to `to`. */
export function moved<T>(list: readonly T[], from: number, to: number): T[] {
  const out = list.slice();
  if (from < 0 || from >= out.length) return out;
  const [item] = out.splice(from, 1);
  out.splice(clamp(to, 0, out.length), 0, item);
  return out;
}
