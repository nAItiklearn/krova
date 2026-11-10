import { describe, expect, it } from 'vitest';
import type { PanelAnchor, PanelCorner } from './backend';
import { monitorFor, placePanel } from './panelPlacement';

const M1 = { profile_id: 'm1', rect: { x: 0, y: 0, w: 2560, h: 1440 }, work: { x: 0, y: 0, w: 2560, h: 1400 }, scale: 1 };
// Right of M1, HiDPI.
const M2 = { profile_id: 'm2', rect: { x: 2560, y: 0, w: 3840, h: 2160 }, work: { x: 2560, y: 0, w: 3840, h: 2100 }, scale: 1.5 };
// Left of M1, negative coordinates.
const M3 = { profile_id: 'm3', rect: { x: -1920, y: 200, w: 1920, h: 1080 }, work: { x: -1920, y: 200, w: 1920, h: 1040 }, scale: 1 };
const MONS = [M1, M2, M3];
const CARD = { w: 340, h: 300 };
const OPTS = { gap: 8, margin: 8, pad: 16 };

/** A 120x28 notch on M1 at each of the five places, 6 px from the edge. */
function notch(corner: PanelCorner, mon = M1): PanelAnchor {
  const { x, y, w, h } = mon.work;
  const nw = 120;
  const nh = 28;
  const inset = 64;
  const ax = corner === 'top_center' ? x + w / 2 - nw / 2 : corner.endsWith('left') ? x + inset : x + w - inset - nw;
  const ay = corner.startsWith('top') ? y + 6 : y + h - 6 - nh;
  return { x: ax, y: ay, w: nw, h: nh, corner, monitor_id: mon.profile_id };
}

const cardOf = (p: ReturnType<typeof placePanel>) => ({
  x: p.x + OPTS.pad * p.scale,
  y: p.y + OPTS.pad * p.scale,
  w: CARD.w * p.scale,
  h: CARD.h * p.scale,
});

describe('placePanel: the five notch positions', () => {
  it('top centre: centred under the notch', () => {
    const a = notch('top_center');
    const p = placePanel(a, MONS, CARD, OPTS);
    const c = cardOf(p);
    expect(p.side).toBe('top');
    expect(c.x + c.w / 2).toBe(a.x + a.w / 2);
    expect(c.y).toBe(a.y + a.h + 8);
    expect(p.originX).toBe(170);
    expect(p.originY).toBe(0);
    expect(p.width).toBe(372);
    expect(p.height).toBe(332);
  });

  it('top left: left edges aligned, below', () => {
    const a = notch('top_left');
    const c = cardOf(placePanel(a, MONS, CARD, OPTS));
    expect(c.x).toBe(a.x);
    expect(c.y).toBe(a.y + a.h + 8);
  });

  it('top right: right edges aligned, below', () => {
    const a = notch('top_right');
    const c = cardOf(placePanel(a, MONS, CARD, OPTS));
    expect(c.x + c.w).toBe(a.x + a.w);
    expect(c.y).toBe(a.y + a.h + 8);
  });

  it('bottom left: above the notch, origin at the card bottom', () => {
    const a = notch('bottom_left');
    const p = placePanel(a, MONS, CARD, OPTS);
    const c = cardOf(p);
    expect(p.side).toBe('bottom');
    expect(c.x).toBe(a.x);
    expect(c.y + c.h).toBe(a.y - 8);
    expect(p.originY).toBe(300);
    expect(p.originX).toBe(60);
  });

  it('bottom right: above, right edges aligned', () => {
    const a = notch('bottom_right');
    const p = placePanel(a, MONS, CARD, OPTS);
    const c = cardOf(p);
    expect(c.x + c.w).toBe(a.x + a.w);
    expect(c.y + c.h).toBe(a.y - 8);
    expect(p.originX).toBe(340 - 60);
  });
});

describe('placePanel: screen edges', () => {
  it('a notch in the very corner keeps the card inside the work area', () => {
    const a: PanelAnchor = { x: 0, y: 0, w: 40, h: 20, corner: 'top_left', monitor_id: 'm1' };
    const c = cardOf(placePanel(a, MONS, CARD, OPTS));
    expect(c.x).toBe(8);
    const b: PanelAnchor = { x: 2540, y: 1380, w: 20, h: 20, corner: 'bottom_right', monitor_id: 'm1' };
    const d = cardOf(placePanel(b, MONS, CARD, OPTS));
    expect(d.x + d.w).toBe(2560 - 8);
    expect(d.y + d.h).toBe(1380 - 8);
  });

  it('origin follows the notch even when the card is pushed sideways', () => {
    const a: PanelAnchor = { x: 0, y: 0, w: 40, h: 20, corner: 'top_center', monitor_id: 'm1' };
    const p = placePanel(a, MONS, CARD, OPTS);
    expect(p.originX).toBe(12); // notch centre 20 - card x 8
  });

  it('flips to the other side when the opening side has no room', () => {
    const short = { ...M1, work: { x: 0, y: 0, w: 2560, h: 400 } };
    const a: PanelAnchor = { x: 1000, y: 360, w: 100, h: 28, corner: 'top_center', monitor_id: 'm1' };
    const p = placePanel(a, [short], CARD, OPTS);
    expect(p.side).toBe('bottom');
    expect(cardOf(p).y + 300).toBe(360 - 8);
    const b: PanelAnchor = { x: 1000, y: 10, w: 100, h: 28, corner: 'bottom_left', monitor_id: 'm1' };
    expect(placePanel(b, [short], CARD, OPTS).side).toBe('top');
  });

  it('a card taller than the work area pins to the top margin', () => {
    const tiny = { ...M1, work: { x: 0, y: 0, w: 800, h: 200 } };
    const a: PanelAnchor = { x: 100, y: 0, w: 100, h: 20, corner: 'top_left', monitor_id: 'm1' };
    expect(cardOf(placePanel(a, [tiny], CARD, OPTS)).y).toBe(8);
  });
});

describe('placePanel: multi-monitor', () => {
  it('uses the scale of the anchor monitor (HiDPI)', () => {
    const a = notch('top_center', M2);
    const p = placePanel(a, MONS, CARD, OPTS);
    const c = cardOf(p);
    expect(p.scale).toBe(1.5);
    expect(p.monitorId).toBe('m2');
    expect(c.w).toBe(510);
    expect(c.x + c.w / 2).toBeCloseTo(a.x + a.w / 2, 0);
    expect(c.y).toBe(a.y + a.h + 12);
    expect(p.width).toBe(372); // logical size does not scale
  });

  it('clamps to the anchor monitor, not the desktop', () => {
    // Right corner of M1, near M2: the card must not spill onto M2.
    const a: PanelAnchor = { x: 2550, y: 0, w: 10, h: 10, corner: 'top_left', monitor_id: 'm1' };
    const c = cardOf(placePanel(a, MONS, CARD, OPTS));
    expect(c.x + c.w).toBe(2560 - 8);
  });

  it('works on a monitor with negative coordinates', () => {
    const a = notch('bottom_left', M3);
    const c = cardOf(placePanel(a, MONS, CARD, OPTS));
    expect(c.x).toBe(a.x);
    expect(c.y + c.h).toBe(a.y - 8);
    expect(c.x).toBeLessThan(0);
  });

  it('an unknown monitor id falls back to the monitor under the anchor', () => {
    const a = { ...notch('top_center', M2), monitor_id: 'gone' };
    expect(monitorFor(a, MONS)?.profile_id).toBe('m2');
    const off = { ...a, x: -5000, y: 500 };
    expect(monitorFor(off, MONS)?.profile_id).toBe('m3');
  });

  it('with no monitors at all it still opens under the anchor', () => {
    const a = notch('top_center');
    const p = placePanel(a, [], CARD, OPTS);
    expect(p.monitorId).toBeNull();
    expect(cardOf(p).y).toBe(a.y + a.h + 8);
  });
});
