// Demo-mode mirror of krova-core's presets.rs, close enough for the template
// picker: grid, columns, rows and main + side for 1..5 windows. Ids and names
// follow the Rust ones; zones are numbered 1..N in reading order.

import type { Layout, MainSize, PresetFamily, PresetInfo, PresetVariant, Rect } from '../bindings';

const ULTRAWIDE_ASPECT = 2.1;

export function variantForSize(w: number, h: number): PresetVariant {
  if (h > w) return 'portrait';
  return h > 0 && w / h >= ULTRAWIDE_ASPECT ? 'ultrawide' : 'landscape';
}

const FRACTION: Record<MainSize, number> = {
  half: 0.5,
  sixty: 0.6,
  two_thirds: 2 / 3,
};
const ID_PART: Record<MainSize, string> = {
  half: '50',
  sixty: '60',
  two_thirds: '67',
};
const LABEL: Record<MainSize, string> = {
  half: '50%',
  sixty: '60%',
  two_thirds: '66.7%',
};

function rowsOf(counts: number[]): Rect[] {
  const h = 1 / counts.length;
  return counts.flatMap((n, row) =>
    Array.from({ length: n }, (_, i) => ({
      x: i / n,
      y: row * h,
      w: 1 / n,
      h,
    })),
  );
}

function gridRows(n: number, variant: PresetVariant): number[] {
  if (variant === 'portrait') return [[1], [1, 1], [1, 2], [2, 2], [1, 2, 2]][n - 1];
  if (variant === 'ultrawide') return [[1], [2], [3], [2, 2], [3, 2]][n - 1];
  return [[1], [2], [2, 1], [2, 2], [3, 2]][n - 1];
}

function mainSide(n: number, variant: PresetVariant, f: number): Rect[] {
  if (n === 1) return [{ x: 0, y: 0, w: 1, h: 1 }];
  if (variant === 'portrait') {
    const rest = rowsOf(n - 1 <= 2 ? [n - 1] : [2, n - 3]).map((r) => ({
      ...r,
      y: f + r.y * (1 - f),
      h: r.h * (1 - f),
    }));
    return [{ x: 0, y: 0, w: 1, h: f }, ...rest];
  }
  if (variant === 'ultrawide' && n >= 3) {
    const side = (1 - f) / 2;
    const main = (1 - side) / 2;
    const k = n - 2;
    return [
      { x: 0, y: 0, w: main, h: 1 },
      { x: main, y: 0, w: main, h: 1 },
      ...Array.from({ length: k }, (_, i) => ({
        x: 1 - side,
        y: i / k,
        w: side,
        h: 1 / k,
      })),
    ];
  }
  const side = variant === 'ultrawide' ? (1 - f) / 2 : 1 - f;
  const k = n - 1;
  return [
    { x: 0, y: 0, w: 1 - side, h: 1 },
    ...Array.from({ length: k }, (_, i) => ({
      x: 1 - side,
      y: i / k,
      w: side,
      h: 1 / k,
    })),
  ];
}

const BASE: Record<PresetFamily, [string, string]> = {
  grid: ['grid', 'Grid'],
  columns: ['columns', 'Columns'],
  rows: ['rows', 'Rows'],
  main_side: ['main-side', 'Main + side'],
};

function preset(
  family: PresetFamily,
  count: number,
  variant: PresetVariant,
  mainSize: MainSize,
): PresetInfo {
  const rects =
    family === 'grid'
      ? rowsOf(gridRows(count, variant))
      : family === 'columns'
        ? rowsOf([count])
        : family === 'rows'
          ? Array.from({ length: count }, (_, i) => ({
              x: 0,
              y: i / count,
              w: 1,
              h: 1 / count,
            }))
          : mainSide(count, variant, FRACTION[mainSize]);
  const [idBase, nameBase] = BASE[family];
  let id = `${idBase}-${count}`;
  const notes: string[] = [];
  if (variant !== 'landscape') {
    id += `-${variant}`;
    notes.push(variant);
  }
  if (family === 'main_side' && count > 1) {
    id += `-${ID_PART[mainSize]}`;
    notes.push(LABEL[mainSize]);
  }
  const name = notes.length ? `${nameBase} ${count} (${notes.join(', ')})` : `${nameBase} ${count}`;
  const layout: Layout = {
    id,
    name,
    zones: rects.map((rect, i) => ({ id: i + 1, rect })),
  };
  return {
    id,
    name,
    family,
    count,
    variant,
    main_size: mainSize,
    recommended: false,
    layout,
  };
}

/** Every distinct preset, like `presets::all()` (variants that change nothing once). */
export function allPresets(): PresetInfo[] {
  const out: PresetInfo[] = [];
  const families: PresetFamily[] = ['grid', 'columns', 'rows', 'main_side'];
  for (const family of families) {
    for (let count = 1; count <= 5; count++) {
      const variants: PresetVariant[] =
        family === 'grid' || family === 'main_side'
          ? ['landscape', 'ultrawide', 'portrait']
          : ['landscape'];
      const sizes: MainSize[] =
        family === 'main_side' && count > 1 ? ['half', 'sixty', 'two_thirds'] : ['sixty'];
      for (const variant of count === 1 ? (['landscape'] as PresetVariant[]) : variants) {
        for (const size of sizes) {
          const p = preset(family, count, variant, size);
          if (!out.some((o) => o.id === p.id)) out.push(p);
        }
      }
    }
  }
  return out;
}
