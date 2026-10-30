import { describe, expect, it } from 'vitest';
import type { Layout } from '../bindings';
import {
  EditError,
  MIN_ZONE,
  editMoveDivider,
  editRemoveZone,
  editSplitZone,
  moveDivider,
  removeZone,
  splitZone,
  validateLayout,
} from './edit';
import { templates } from './templates';

const close = (a: number, b: number) => Math.abs(a - b) < 1e-6;
const totalArea = (l: Layout) => l.zones.reduce((s, z) => s + z.rect.w * z.rect.h, 0);
const clone = <T>(v: T): T => JSON.parse(JSON.stringify(v)) as T;
const tpl = (id: string): Layout => {
  const t = templates().find((x) => x.id === id);
  if (!t) throw new Error(`no template ${id}`);
  return t;
};
const oneZone = (): Layout => ({
  id: 'one',
  name: 'One',
  zones: [{ id: 1, rect: { x: 0, y: 0, w: 1, h: 1 } }],
});

/** Zone containing the point, half-open, smaller area wins (Layout::hit_test). */
function hit(l: Layout, nx: number, ny: number): number | null {
  const hits = l.zones.filter(
    (z) => nx >= z.rect.x && nx < z.rect.x + z.rect.w && ny >= z.rect.y && ny < z.rect.y + z.rect.h,
  );
  hits.sort((a, b) => a.rect.w * a.rect.h - b.rect.w * b.rect.h || a.id - b.id);
  return hits[0]?.id ?? null;
}

describe('templates', () => {
  it('match zone.rs: ids, names, zone counts and validity', () => {
    const t = templates();
    expect(t.map((l) => l.id)).toEqual(['halves', 'thirds', 'grid', 'priority']);
    expect(t.map((l) => l.name)).toEqual(['Halves', 'Thirds', 'Grid 2x2', 'Priority']);
    expect(t.map((l) => l.zones.length)).toEqual([2, 3, 4, 3]);
    for (const l of t) {
      expect(() => validateLayout(l)).not.toThrow();
      expect(l.zones.map((z) => z.id)).toEqual(l.zones.map((_, i) => i + 1));
      expect(close(totalArea(l), 1)).toBe(true);
    }
    expect(tpl('priority').zones.map((z) => z.rect.w)).toEqual([0.25, 0.5, 0.25]);
  });
});

describe('split', () => {
  it('keeps tiling and hits both halves', () => {
    const l = oneZone();
    expect(splitZone(l, 1, 'vertical', 0.4)).toBe(2);
    expect(() => validateLayout(l)).not.toThrow();
    expect(close(totalArea(l), 1)).toBe(true);
    expect(hit(l, 0.39, 0.5)).toBe(1);
    expect(hit(l, 0.4, 0.5)).toBe(2);

    expect(splitZone(l, 2, 'horizontal', 0.5)).toBe(3);
    expect(() => validateLayout(l)).not.toThrow();
    expect(close(totalArea(l), 1)).toBe(true);
    expect(hit(l, 0.7, 0.2)).toBe(2);
    expect(hit(l, 0.7, 0.8)).toBe(3);
  });

  it('original keeps the left/top part and the new zone gets max id + 1', () => {
    const l = tpl('thirds'); // ids 1..3
    l.zones[1].id = 7; // non-contiguous ids: next id is max + 1, not len + 1
    const newId = splitZone(l, 1, 'vertical', 0.2);
    expect(newId).toBe(8);
    const first = l.zones.find((z) => z.id === 1)!;
    const second = l.zones.find((z) => z.id === 8)!;
    expect(close(first.rect.x, 0) && close(first.rect.w, 0.2)).toBe(true);
    expect(close(second.rect.x, 0.2) && close(second.rect.w, 1 / 3 - 0.2)).toBe(true);
  });

  it('rejects slivers and unknown zones, leaving the layout unchanged', () => {
    const l = tpl('halves');
    const before = clone(l);
    expect(() => splitZone(l, 1, 'vertical', 0.01)).toThrowError(
      'the change would leave a zone smaller than the minimum size',
    );
    expect(() => splitZone(l, 1, 'vertical', 0.49)).toThrowError(
      'the change would leave a zone smaller than the minimum size',
    );
    expect(() => splitZone(l, 9, 'vertical', 0.2)).toThrowError('zone 9 not found');
    expect(() => splitZone(l, 1, 'horizontal', 0.97)).toThrow(EditError);
    expect(l).toEqual(before);
    expect(l.zones).toHaveLength(2);
  });

  it('accepts a split exactly MIN_ZONE away from the edge only when not below it', () => {
    const l = oneZone();
    expect(() => splitZone(l, 1, 'vertical', MIN_ZONE - 0.001)).toThrow(EditError);
    expect(() => splitZone(l, 1, 'vertical', 0.5)).not.toThrow();
  });

  it('wrapper does not mutate its argument', () => {
    const l = tpl('halves');
    const before = clone(l);
    const out = editSplitZone(l, 2, 'horizontal', 0.5);
    expect(l).toEqual(before);
    expect(out.zones).toHaveLength(3);
  });
});

describe('remove', () => {
  it('gives space to the neighbour sharing a full edge', () => {
    const l = tpl('grid');
    expect(removeZone(l, 1)).toBe(2);
    expect(() => validateLayout(l)).not.toThrow();
    expect(l.zones).toHaveLength(3);
    expect(close(totalArea(l), 1)).toBe(true);
    expect(hit(l, 0.1, 0.1)).toBe(2);
  });

  it('refuses when no neighbour shares a full edge, leaving the layout unchanged', () => {
    const l: Layout = {
      id: 't',
      name: 't',
      zones: [
        { id: 1, rect: { x: 0, y: 0, w: 0.5, h: 1 } },
        { id: 2, rect: { x: 0.5, y: 0, w: 0.5, h: 0.5 } },
        { id: 3, rect: { x: 0.5, y: 0.5, w: 0.5, h: 0.5 } },
      ],
    };
    const before = clone(l);
    expect(() => removeZone(l, 1)).toThrowError('zone 1 has no neighbour it can merge into');
    expect(l).toEqual(before);
    expect(removeZone(l, 2)).toBe(3);
    expect(l.zones).toHaveLength(2);
  });

  it('refuses to remove the last zone and unknown zones', () => {
    const l = oneZone();
    expect(() => removeZone(l, 1)).toThrowError('a layout needs at least one zone');
    expect(() => removeZone(l, 5)).toThrowError('zone 5 not found');
    expect(l.zones).toHaveLength(1);
  });

  it('wrapper does not mutate its argument', () => {
    const l = tpl('thirds');
    const before = clone(l);
    const out = editRemoveZone(l, 3);
    expect(l).toEqual(before);
    expect(out.zones).toHaveLength(2);
    expect(close(out.zones[1].rect.w, 2 / 3)).toBe(true);
  });
});

describe('divider', () => {
  it('moves both sides', () => {
    const l = tpl('halves');
    moveDivider(l, 'vertical', 0.5, 0.3);
    expect(() => validateLayout(l)).not.toThrow();
    expect(close(l.zones[0].rect.w, 0.3)).toBe(true);
    expect(close(l.zones[1].rect.x, 0.3)).toBe(true);
    expect(close(l.zones[1].rect.w, 0.7)).toBe(true);
  });

  it('moves every aligned edge in a grid', () => {
    const l = tpl('grid');
    moveDivider(l, 'vertical', 0.5, 0.6);
    expect(() => validateLayout(l)).not.toThrow();
    expect(close(l.zones[0].rect.w, 0.6) && close(l.zones[2].rect.w, 0.6)).toBe(true);
    expect(close(l.zones[1].rect.x, 0.6) && close(l.zones[3].rect.x, 0.6)).toBe(true);
    expect(close(totalArea(l), 1)).toBe(true);
  });

  it('moves horizontal dividers along y', () => {
    const l = tpl('grid');
    moveDivider(l, 'horizontal', 0.5, 0.25);
    expect(close(l.zones[0].rect.h, 0.25) && close(l.zones[1].rect.h, 0.25)).toBe(true);
    expect(close(l.zones[2].rect.y, 0.25) && close(l.zones[3].rect.y, 0.25)).toBe(true);
    expect(close(l.zones[2].rect.h, 0.75)).toBe(true);
  });

  it('rejects the border and slivers, leaving the layout unchanged', () => {
    const l = tpl('halves');
    expect(() => moveDivider(l, 'vertical', 0.0, 0.2)).toThrowError(
      'there is no divider at that position',
    );
    expect(() => moveDivider(l, 'vertical', 1.0, 0.8)).toThrowError(
      'there is no divider at that position',
    );
    expect(() => moveDivider(l, 'vertical', 0.3, 0.2)).toThrowError(
      'there is no divider at that position',
    );
    // A horizontal divider does not exist in a two-column layout.
    expect(() => moveDivider(l, 'horizontal', 0.5, 0.2)).toThrowError(
      'there is no divider at that position',
    );
    expect(() => moveDivider(l, 'vertical', 0.5, 0.98)).toThrowError(
      'the change would leave a zone smaller than the minimum size',
    );
    expect(() => moveDivider(l, 'vertical', 0.5, 0.01)).toThrowError(
      'the change would leave a zone smaller than the minimum size',
    );
    expect(l).toEqual(tpl('halves'));
  });

  it('allows a move that leaves exactly MIN_ZONE (within epsilon)', () => {
    const l = tpl('halves');
    expect(() => moveDivider(l, 'vertical', 0.5, MIN_ZONE)).not.toThrow();
    const m = tpl('halves');
    expect(() => moveDivider(m, 'vertical', 0.5, 1 - MIN_ZONE)).not.toThrow();
  });

  it('matches edges within 1e-6 of `from`', () => {
    const l = tpl('halves');
    expect(() => moveDivider(l, 'vertical', 0.5 + 5e-7, 0.4)).not.toThrow();
    const m = tpl('halves');
    expect(() => moveDivider(m, 'vertical', 0.5 + 2e-6, 0.4)).toThrowError(
      'there is no divider at that position',
    );
  });

  it('wrapper does not mutate its argument', () => {
    const l = tpl('priority');
    const before = clone(l);
    const out = editMoveDivider(l, 'vertical', 0.25, 0.4);
    expect(l).toEqual(before);
    expect(close(out.zones[0].rect.w, 0.4)).toBe(true);
    expect(close(out.zones[1].rect.x, 0.4)).toBe(true);
    expect(close(out.zones[1].rect.w, 0.35)).toBe(true);
  });
});

describe('validateLayout', () => {
  it('mirrors LayoutError strings', () => {
    expect(() => validateLayout({ id: 'e', name: 'e', zones: [] })).toThrowError(
      'layout has no zones',
    );
    const dup = tpl('halves');
    dup.zones[1].id = 1;
    expect(() => validateLayout(dup)).toThrowError('duplicate zone id 1');
    const bad = tpl('halves');
    bad.zones[0].rect = { x: 0.9, y: 0, w: 0.5, h: 1 };
    expect(() => validateLayout(bad)).toThrowError('zone 1 has an invalid rectangle');
  });
});
