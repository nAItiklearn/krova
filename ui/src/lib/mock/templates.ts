// Mirror of crates/krova-core/src/zone.rs `templates()`: same ids, names, rects
// and zone numbering (1-based, in order).

import type { Layout, Rect } from '../bindings';

function layout(id: string, name: string, rects: Rect[]): Layout {
  return { id, name, zones: rects.map((rect, i) => ({ id: i + 1, rect })) };
}

export function templates(): Layout[] {
  const third = 1 / 3;
  return [
    layout('halves', 'Halves', [
      { x: 0, y: 0, w: 0.5, h: 1 },
      { x: 0.5, y: 0, w: 0.5, h: 1 },
    ]),
    layout('thirds', 'Thirds', [
      { x: 0, y: 0, w: third, h: 1 },
      { x: third, y: 0, w: third, h: 1 },
      { x: 2 * third, y: 0, w: third, h: 1 },
    ]),
    layout('grid', 'Grid 2x2', [
      { x: 0, y: 0, w: 0.5, h: 0.5 },
      { x: 0.5, y: 0, w: 0.5, h: 0.5 },
      { x: 0, y: 0.5, w: 0.5, h: 0.5 },
      { x: 0.5, y: 0.5, w: 0.5, h: 0.5 },
    ]),
    layout('priority', 'Priority', [
      { x: 0, y: 0, w: 0.25, h: 1 },
      { x: 0.25, y: 0, w: 0.5, h: 1 },
      { x: 0.75, y: 0, w: 0.25, h: 1 },
    ]),
  ];
}
