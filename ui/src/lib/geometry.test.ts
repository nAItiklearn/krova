import { describe, expect, it } from 'vitest';
import type { Layout } from './bindings';
import { computeDividers, hitTest, pct, previewMoveDivider, tidy, zonePixels } from './geometry';
import { templates } from './mock/templates';

const tpl = (id: string): Layout => templates().find((t) => t.id === id)!;

describe('hitTest', () => {
  it('is half-open and prefers the smaller zone', () => {
    const halves = tpl('halves');
    expect(hitTest(halves, 0.4999, 0.5)?.id).toBe(1);
    expect(hitTest(halves, 0.5, 0.5)?.id).toBe(2);
    expect(hitTest(halves, 1, 0.5)).toBeNull();
    const overlap: Layout = {
      id: 'o',
      name: 'o',
      zones: [
        { id: 1, rect: { x: 0, y: 0, w: 1, h: 1 } },
        { id: 2, rect: { x: 0.25, y: 0.25, w: 0.5, h: 0.5 } },
      ],
    };
    expect(hitTest(overlap, 0.5, 0.5)?.id).toBe(2);
    expect(hitTest(overlap, 0.1, 0.1)?.id).toBe(1);
  });
});

describe('computeDividers', () => {
  it('finds no divider in a single zone and ignores the border', () => {
    const one: Layout = { id: 'a', name: 'a', zones: [{ id: 1, rect: { x: 0, y: 0, w: 1, h: 1 } }] };
    expect(computeDividers(one)).toEqual([]);
  });

  it('halves has one vertical divider spanning the full height', () => {
    const d = computeDividers(tpl('halves'));
    expect(d).toHaveLength(1);
    expect(d[0].axis).toBe('vertical');
    expect(d[0].pos).toBe(0.5);
    expect(d[0].segments).toEqual([{ from: 0, to: 1 }]);
    expect(d[0].lo).toBeCloseTo(0.05);
    expect(d[0].hi).toBeCloseTo(0.95);
  });

  it('grid has one vertical and one horizontal divider', () => {
    const d = computeDividers(tpl('grid'));
    expect(d.map((x) => x.axis).sort()).toEqual(['horizontal', 'vertical']);
  });

  it('bounds follow the neighbours (priority)', () => {
    const d = computeDividers(tpl('priority'));
    expect(d.map((x) => x.pos)).toEqual([0.25, 0.75]);
    expect(d[0].lo).toBeCloseTo(0.05);
    expect(d[0].hi).toBeCloseTo(0.7);
    expect(d[1].lo).toBeCloseTo(0.3);
    expect(d[1].hi).toBeCloseTo(0.95);
  });

  it('draws a T-junction divider as segments only where zones meet', () => {
    const l: Layout = {
      id: 't',
      name: 't',
      zones: [
        { id: 1, rect: { x: 0, y: 0, w: 0.5, h: 1 } },
        { id: 2, rect: { x: 0.5, y: 0, w: 0.5, h: 0.5 } },
        { id: 3, rect: { x: 0.5, y: 0.5, w: 0.5, h: 0.5 } },
      ],
    };
    const d = computeDividers(l);
    expect(d).toHaveLength(2);
    const v = d.find((x) => x.axis === 'vertical')!;
    expect(v.segments).toEqual([{ from: 0, to: 1 }]);
    const h = d.find((x) => x.axis === 'horizontal')!;
    expect(h.segments).toEqual([{ from: 0.5, to: 1 }]);
  });
});

describe('previewMoveDivider', () => {
  it('resizes both sides without validating', () => {
    const out = previewMoveDivider(tpl('halves'), 'vertical', 0.5, 0.3);
    expect(out.zones[0].rect.w).toBeCloseTo(0.3);
    expect(out.zones[1].rect.x).toBeCloseTo(0.3);
    expect(out.zones[1].rect.w).toBeCloseTo(0.7);
  });
});

describe('helpers', () => {
  it('tidy rounds to 0.5% steps', () => {
    expect(tidy(0.40123)).toBe(0.4);
    expect(tidy(0.4026)).toBe(0.405);
  });

  it('pct formats percentages', () => {
    expect(pct(0.4)).toBe('40%');
    expect(pct(1 / 3)).toBe('33.3%');
    expect(pct(0.5)).toBe('50%');
  });

  it('zonePixels tiles without gaps', () => {
    const work = { x: 100, y: 50, w: 1001, h: 777 };
    const [a, b, c] = tpl('thirds').zones.map((z) => zonePixels(work, z.rect));
    expect(a.x + a.w).toBe(b.x);
    expect(b.x + b.w).toBe(c.x);
    expect(c.x + c.w).toBe(work.x + work.w);
  });
});
