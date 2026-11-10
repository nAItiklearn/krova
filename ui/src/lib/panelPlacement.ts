// Where the quick panel window goes for a given notch anchor. Pure maths, no DOM.
//
// Units: the anchor and the monitors are physical screen pixels (like the Rust side);
// the card size and the paddings are logical (CSS) pixels and get multiplied by the
// monitor's scale. The returned window position is physical, the size logical.

import type { PanelAnchor } from './backend';
import type { MonitorSnapshot, PxRect } from './bindings';

export interface PlacementOptions {
  /** Space between the notch and the card (logical px). */
  gap?: number;
  /** Minimum distance from the work-area edges (logical px). */
  margin?: number;
  /** Transparent room around the card inside the window, for its shadow (logical px). */
  pad?: number;
}

export interface Placement {
  /** Window top-left, physical px. */
  x: number;
  y: number;
  /** Window size, logical px (card + 2 * pad). */
  width: number;
  height: number;
  scale: number;
  /** The card opens downwards from a top notch, upwards from a bottom one. */
  side: 'top' | 'bottom';
  /** Transform origin of the card (logical px, relative to the card): under the notch. */
  originX: number;
  originY: number;
  /** The monitor the panel is on (`profile_id`), or null when none is known. */
  monitorId: string | null;
}

type Mon = Pick<MonitorSnapshot, 'profile_id' | 'rect' | 'work' | 'scale'>;

const contains = (r: PxRect, x: number, y: number): boolean =>
  x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h;

function distance(r: PxRect, x: number, y: number): number {
  const dx = Math.max(r.x - x, 0, x - (r.x + r.w));
  const dy = Math.max(r.y - y, 0, y - (r.y + r.h));
  return Math.hypot(dx, dy);
}

/** The monitor with the anchor's id; else the one containing (or nearest to) its centre. */
export function monitorFor(anchor: PanelAnchor, monitors: readonly Mon[]): Mon | null {
  const byId = monitors.find((m) => m.profile_id === anchor.monitor_id);
  if (byId) return byId;
  const cx = anchor.x + anchor.w / 2;
  const cy = anchor.y + anchor.h / 2;
  const inside = monitors.find((m) => contains(m.rect, cx, cy));
  if (inside) return inside;
  let best: Mon | null = null;
  let bestD = Infinity;
  for (const m of monitors) {
    const d = distance(m.rect, cx, cy);
    if (d < bestD) {
      best = m;
      bestD = d;
    }
  }
  return best;
}

const clamp = (v: number, lo: number, hi: number): number => (hi < lo ? lo : Math.min(Math.max(v, lo), hi));

export function placePanel(
  anchor: PanelAnchor,
  monitors: readonly Mon[],
  card: { w: number; h: number },
  opts: PlacementOptions = {},
): Placement {
  const gap = opts.gap ?? 8;
  const margin = opts.margin ?? 8;
  const pad = opts.pad ?? 16;

  const mon = monitorFor(anchor, monitors);
  const scale = mon?.scale && mon.scale > 0 ? mon.scale : 1;
  // Without a monitor, the anchor's own surroundings are all we know: never clamp.
  const work: PxRect = mon?.work ?? { x: -1e9, y: -1e9, w: 2e9, h: 2e9 };

  const cw = card.w * scale;
  const ch = card.h * scale;
  const g = gap * scale;
  const m = margin * scale;

  // Vertical: below a top notch, above a bottom one; flip if that side has no room.
  let side: 'top' | 'bottom' = anchor.corner.startsWith('bottom') ? 'bottom' : 'top';
  const below = anchor.y + anchor.h + g;
  const above = anchor.y - g - ch;
  const minY = work.y + m;
  const maxY = work.y + work.h - m - ch;
  if (side === 'top' && below > maxY && above >= minY) side = 'bottom';
  else if (side === 'bottom' && above < minY && below <= maxY) side = 'top';
  const cardY = clamp(side === 'top' ? below : above, minY, maxY);

  // Horizontal: centred under a centre notch, edge-aligned under a corner one.
  let cardX: number;
  if (anchor.corner.endsWith('left')) cardX = anchor.x;
  else if (anchor.corner.endsWith('right')) cardX = anchor.x + anchor.w - cw;
  else cardX = anchor.x + anchor.w / 2 - cw / 2;
  cardX = clamp(cardX, work.x + m, work.x + work.w - m - cw);

  const originX = clamp((anchor.x + anchor.w / 2 - cardX) / scale, 0, card.w);
  const originY = side === 'top' ? 0 : card.h;

  return {
    x: Math.round(cardX - pad * scale),
    y: Math.round(cardY - pad * scale),
    width: Math.round(card.w + pad * 2),
    height: Math.round(card.h + pad * 2),
    scale,
    side,
    originX: Math.round(originX),
    originY: Math.round(originY),
    monitorId: mon?.profile_id ?? null,
  };
}
