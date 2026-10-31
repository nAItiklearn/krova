import { describe, expect, it } from 'vitest';
import type { Layout } from '../bindings';
import { MAX_NAME_LEN, MAX_ZONES, validateLayout } from './edit';
import {
  ENVELOPE_VERSION,
  MAX_IMPORT_BYTES,
  exportEnvelope,
  messages,
  parseEnvelope,
  uniqueLayoutId,
} from './exchange';
import { templates } from './templates';

const halves = (): Layout => ({
  id: 'mine',
  name: 'Mine',
  zones: [
    { id: 1, rect: { x: 0, y: 0, w: 0.5, h: 1 } },
    { id: 2, rect: { x: 0.5, y: 0, w: 0.5, h: 1 } },
  ],
});

/** An envelope around anything, for building hostile files. */
const envelope = (layout: unknown, version: unknown = 1): string =>
  JSON.stringify({ krova_layout: version, layout });

const bytes = (s: string): number => new TextEncoder().encode(s).length;

/** The message a call rejects with (parseEnvelope throws Errors whose message is the string). */
function failure(fn: () => unknown): string {
  try {
    fn();
  } catch (e) {
    return e instanceof Error ? e.message : String(e);
  }
  throw new Error('expected the call to fail');
}

/** n zones that all fit the unit square (a grid of equal cells). */
function manyZones(n: number): Layout['zones'] {
  const side = Math.max(1, Math.ceil(Math.sqrt(n)));
  const cell = 1 / side;
  return Array.from({ length: n }, (_, i) => ({
    id: i + 1,
    rect: { x: (i % side) * cell, y: Math.floor(i / side) * cell, w: cell, h: cell },
  }));
}

describe('exportEnvelope', () => {
  it('writes the version-1 envelope, pretty printed with two spaces, envelope key first', () => {
    const text = exportEnvelope(halves());
    expect(text.startsWith('{\n  "krova_layout": 1,\n  "layout": {\n    "id": "mine",')).toBe(true);
    expect(text.endsWith('\n}')).toBe(true);
    const parsed = JSON.parse(text) as { krova_layout: number; layout: Layout };
    expect(parsed.krova_layout).toBe(ENVELOPE_VERSION);
    expect(parsed.layout).toEqual(halves());
    expect(Object.keys(parsed)).toEqual(['krova_layout', 'layout']);
    expect(Object.keys(parsed.layout)).toEqual(['id', 'name', 'zones']);
    expect(Object.keys(parsed.layout.zones[0])).toEqual(['id', 'rect']);
    expect(Object.keys(parsed.layout.zones[0].rect)).toEqual(['x', 'y', 'w', 'h']);
  });

  it('round-trips every template exactly, including 1/3 fractions', () => {
    for (const t of templates()) {
      expect(parseEnvelope(exportEnvelope(t))).toEqual(t);
    }
  });

  it('only exports what the file format knows (extra fields are dropped)', () => {
    const noisy = { ...halves(), secret: 'x' } as Layout;
    expect(exportEnvelope(noisy)).not.toContain('secret');
  });

  it('refuses invalid layouts with the validation message', () => {
    expect(failure(() => exportEnvelope({ id: 'e', name: 'e', zones: [] }))).toBe(
      'layout has no zones',
    );
    const bad = halves();
    bad.zones[0].rect.w = 2;
    expect(failure(() => exportEnvelope(bad))).toBe('zone 1 has an invalid rectangle');
  });

  it('fits well inside the import limit for the biggest allowed layout', () => {
    const big: Layout = { id: 'x'.repeat(64), name: 'y'.repeat(64), zones: manyZones(MAX_ZONES) };
    expect(bytes(exportEnvelope(big))).toBeLessThan(MAX_IMPORT_BYTES);
  });
});

describe('parseEnvelope: syntax and envelope', () => {
  it.each(['', '   ', 'not json', '{', '{"a":1,}', "{'a':1}", 'undefined', '{"krova_layout":1', '\u0000'])(
    'calls %j "not valid JSON"',
    (text) => {
      expect(failure(() => parseEnvelope(text))).toBe('that is not valid JSON');
    },
  );

  it.each([
    ['an array', '[]'],
    ['null', 'null'],
    ['a number', '42'],
    ['a string', '"hello"'],
    ['an empty object', '{}'],
    ['a bare layout', JSON.stringify(halves())],
    ['no version', JSON.stringify({ layout: halves() })],
    ['a string version', JSON.stringify({ krova_layout: '1', layout: halves() })],
    ['a null version', JSON.stringify({ krova_layout: null, layout: halves() })],
    ['a boolean version', JSON.stringify({ krova_layout: true, layout: halves() })],
    ['a version inside the layout', JSON.stringify({ layout: { ...halves(), krova_layout: 1 } })],
  ])('says "not a Krova layout file" for %s', (_name, text) => {
    expect(failure(() => parseEnvelope(text))).toBe('not a Krova layout file');
  });

  it.each([0, 2, 3, 99, -1, 1.5, 1e9])('refuses envelope version %s', (v) => {
    expect(failure(() => parseEnvelope(envelope(halves(), v)))).toBe(
      `unsupported layout file version ${v}`,
    );
  });

  it('checks the version before the layout', () => {
    expect(failure(() => parseEnvelope(envelope('garbage', 2)))).toBe(
      'unsupported layout file version 2',
    );
  });

  it('accepts version 1 written as 1.0 and surrounding whitespace', () => {
    const text = `\n  ${envelope(halves()).replace('"krova_layout":1', '"krova_layout":1.0')}\n\n`;
    expect(parseEnvelope(text)).toEqual(halves());
  });

  it('ignores unknown fields everywhere', () => {
    const text = JSON.stringify({
      krova_layout: 1,
      app: 'Krova 9',
      layout: {
        ...halves(),
        extra: [1, 2],
        zones: halves().zones.map((z) => ({ ...z, label: 'x', rect: { ...z.rect, z: 1 } })),
      },
    });
    expect(parseEnvelope(text)).toEqual(halves());
  });
});

describe('parseEnvelope: layout shape', () => {
  const zone = { id: 1, rect: { x: 0, y: 0, w: 1, h: 1 } };
  it.each([
    ['no layout', JSON.stringify({ krova_layout: 1 })],
    ['a null layout', envelope(null)],
    ['an array as layout', envelope([])],
    ['a string as layout', envelope('x')],
    ['no id', envelope({ name: 'n', zones: [zone] })],
    ['a numeric id', envelope({ id: 5, name: 'n', zones: [zone] })],
    ['no name', envelope({ id: 'a', zones: [zone] })],
    ['no zones', envelope({ id: 'a', name: 'n' })],
    ['zones that are not a list', envelope({ id: 'a', name: 'n', zones: {} })],
    ['a zone that is not an object', envelope({ id: 'a', name: 'n', zones: [7] })],
    ['a zone without a rect', envelope({ id: 'a', name: 'n', zones: [{ id: 1 }] })],
    ['a zone without an id', envelope({ id: 'a', name: 'n', zones: [{ rect: zone.rect }] })],
    ['a negative zone id', envelope({ id: 'a', name: 'n', zones: [{ ...zone, id: -1 }] })],
    ['a fractional zone id', envelope({ id: 'a', name: 'n', zones: [{ ...zone, id: 1.5 }] })],
    ['a zone id beyond u32', envelope({ id: 'a', name: 'n', zones: [{ ...zone, id: 2 ** 32 }] })],
    ['a string zone id', envelope({ id: 'a', name: 'n', zones: [{ ...zone, id: '1' }] })],
    ['a missing rect side', envelope({ id: 'a', name: 'n', zones: [{ id: 1, rect: { x: 0, y: 0, w: 1 } }] })],
    [
      'a string rect side',
      envelope({ id: 'a', name: 'n', zones: [{ id: 1, rect: { x: '0', y: 0, w: 1, h: 1 } }] }),
    ],
    ['a null rect side', envelope({ id: 'a', name: 'n', zones: [{ id: 1, rect: { x: 0, y: 0, w: 1, h: null } }] })],
  ])('refuses %s', (_name, text) => {
    expect(failure(() => parseEnvelope(text))).toBe(messages.badLayout);
  });

  it('accepts the smallest and the largest u32 zone id', () => {
    const l = parseEnvelope(
      envelope({
        id: 'a',
        name: 'n',
        zones: [
          { id: 0, rect: { x: 0, y: 0, w: 0.5, h: 1 } },
          { id: 4294967295, rect: { x: 0.5, y: 0, w: 0.5, h: 1 } },
        ],
      }),
    );
    expect(l.zones.map((z) => z.id)).toEqual([0, 4294967295]);
  });

  it('does not let __proto__ keys reach the layout or Object.prototype', () => {
    const text =
      '{"krova_layout":1,"__proto__":{"polluted":true},"layout":{"__proto__":{"polluted":true},' +
      '"id":"a","name":"n","constructor":{"prototype":{"polluted":true}},' +
      '"zones":[{"id":1,"rect":{"x":0,"y":0,"w":1,"h":1}}]}}';
    const l = parseEnvelope(text);
    expect(l).toEqual({ id: 'a', name: 'n', zones: [{ id: 1, rect: { x: 0, y: 0, w: 1, h: 1 } }] });
    expect(Object.getPrototypeOf(l)).toBe(Object.prototype);
    expect(({} as Record<string, unknown>).polluted).toBeUndefined();
  });
});

describe('parseEnvelope: layout validation (same rules and strings as save_layout)', () => {
  const withZones = (zones: unknown[], extra: Record<string, unknown> = {}): string =>
    envelope({ id: 'a', name: 'n', zones, ...extra });
  const unit = { x: 0, y: 0, w: 1, h: 1 };

  it('refuses a layout without zones', () => {
    expect(failure(() => parseEnvelope(withZones([])))).toBe('layout has no zones');
  });

  it('allows exactly 64 zones and refuses 65', () => {
    const ok = parseEnvelope(envelope({ id: 'a', name: 'n', zones: manyZones(64) }));
    expect(ok.zones).toHaveLength(64);
    expect(failure(() => parseEnvelope(envelope({ id: 'a', name: 'n', zones: manyZones(65) })))).toBe(
      'layout has too many zones (at most 64)',
    );
  });

  it('refuses a few thousand zones that still fit in 64 KiB', () => {
    const tiny = Array.from({ length: 1400 }, (_, i) => ({ id: i + 1, rect: unit }));
    const text = withZones(tiny);
    expect(bytes(text)).toBeLessThan(MAX_IMPORT_BYTES);
    expect(failure(() => parseEnvelope(text))).toBe('layout has too many zones (at most 64)');
  });

  it('refuses 10 000 zones as too large before it looks at them', () => {
    const tiny = Array.from({ length: 10_000 }, (_, i) => ({ id: i + 1, rect: unit }));
    const text = withZones(tiny);
    expect(bytes(text)).toBeGreaterThan(MAX_IMPORT_BYTES);
    expect(failure(() => parseEnvelope(text))).toBe('layout file is larger than 64 KiB');
    // The validation itself also holds, without the size limit in front.
    const l: Layout = { id: 'a', name: 'n', zones: tiny };
    expect(failure(() => validateLayout(l))).toBe('layout has too many zones (at most 64)');
  });

  it('limits id and name to 64 characters, counted in code points', () => {
    const ok = (id: string, name: string) =>
      parseEnvelope(envelope({ id, name, zones: [{ id: 1, rect: unit }] }));
    const bad = (id: string, name: string) =>
      failure(() => ok(id, name));

    expect(ok('i'.repeat(MAX_NAME_LEN), 'n'.repeat(MAX_NAME_LEN)).id).toHaveLength(MAX_NAME_LEN);
    expect(bad('i'.repeat(MAX_NAME_LEN + 1), 'n')).toBe(
      'layout id or name is too long (at most 64 characters)',
    );
    expect(bad('i', 'n'.repeat(MAX_NAME_LEN + 1))).toBe(
      'layout id or name is too long (at most 64 characters)',
    );
    // 64 emoji are 64 characters (128 UTF-16 units, 256 bytes): fine. 65 are not.
    expect(ok('i', '😀'.repeat(64)).name).toBe('😀'.repeat(64));
    expect(bad('i', '😀'.repeat(65))).toBe('layout id or name is too long (at most 64 characters)');
    expect(ok('é'.repeat(64), 'é'.repeat(64)).id).toBe('é'.repeat(64));
  });

  it('refuses duplicate zone ids, naming the id', () => {
    const text = withZones([
      { id: 3, rect: { x: 0, y: 0, w: 0.5, h: 1 } },
      { id: 4, rect: { x: 0.5, y: 0, w: 0.5, h: 1 } },
      { id: 3, rect: { x: 0, y: 0, w: 0.2, h: 0.2 } },
    ]);
    expect(failure(() => parseEnvelope(text))).toBe('duplicate zone id 3');
  });

  it('reports a duplicate id before a later bad rectangle, and the first bad zone', () => {
    const text = withZones([
      { id: 1, rect: unit },
      { id: 1, rect: { x: 5, y: 5, w: 1, h: 1 } },
    ]);
    expect(failure(() => parseEnvelope(text))).toBe('duplicate zone id 1');
    const two = withZones([
      { id: 1, rect: unit },
      { id: 7, rect: { x: 5, y: 5, w: 1, h: 1 } },
      { id: 8, rect: { x: 9, y: 9, w: 1, h: 1 } },
    ]);
    expect(failure(() => parseEnvelope(two))).toBe('zone 7 has an invalid rectangle');
  });

  it.each([
    ['zero width', { x: 0, y: 0, w: 0, h: 1 }],
    ['negative height', { x: 0, y: 0, w: 1, h: -1 }],
    ['negative x', { x: -0.1, y: 0, w: 0.5, h: 1 }],
    ['past the right edge', { x: 0.9, y: 0, w: 0.5, h: 1 }],
    ['past the bottom edge', { x: 0, y: 0.8, w: 1, h: 0.5 }],
    ['a side under 0.01 (width)', { x: 0, y: 0, w: 0.009, h: 1 }],
    ['a side under 0.01 (height)', { x: 0, y: 0, w: 1, h: 0.0099 }],
    ['larger than the screen', { x: 0, y: 0, w: 2, h: 2 }],
  ])('refuses a rectangle with %s', (_name, rect) => {
    expect(failure(() => parseEnvelope(withZones([{ id: 1, rect }])))).toBe(
      'zone 1 has an invalid rectangle',
    );
  });

  it('accepts a side of exactly 0.01 and rectangles touching the edges', () => {
    const l = parseEnvelope(withZones([{ id: 1, rect: { x: 0.99, y: 0.99, w: 0.01, h: 0.01 } }]));
    expect(l.zones[0].rect.w).toBe(0.01);
  });

  it.each(['1e999', '-1e999'])('treats %s (not a finite number) as an invalid rectangle', (n) => {
    // JSON text so the huge number is really parsed, not built in JS (where it is Infinity).
    const text =
      '{"krova_layout":1,"layout":{"id":"a","name":"n","zones":[{"id":2,"rect":' +
      `{"x":0,"y":0,"w":${n},"h":1}}]}}`;
    expect(failure(() => parseEnvelope(text))).toBe('zone 2 has an invalid rectangle');
  });

  it('treats a NaN smuggled in as a string as a malformed layout, not as a number', () => {
    const text = withZones([{ id: 1, rect: { x: 0, y: 0, w: 'NaN', h: 1 } }]);
    expect(failure(() => parseEnvelope(text))).toBe(messages.badLayout);
    // NaN is not JSON at all.
    const raw = '{"krova_layout":1,"layout":{"id":"a","name":"n","zones":[{"id":1,"rect":{"x":NaN,"y":0,"w":1,"h":1}}]}}';
    expect(failure(() => parseEnvelope(raw))).toBe('that is not valid JSON');
  });
});

describe('parseEnvelope: hostile input', () => {
  const unit = { x: 0, y: 0, w: 1, h: 1 };
  const validText = envelope({ id: 'a', name: 'n', zones: [{ id: 1, rect: unit }] });

  it('allows exactly 64 KiB (UTF-8 bytes) and refuses one byte more', () => {
    const pad = MAX_IMPORT_BYTES - bytes(validText);
    expect(pad).toBeGreaterThan(0);
    expect(bytes(validText + ' '.repeat(pad))).toBe(MAX_IMPORT_BYTES);
    expect(parseEnvelope(validText + ' '.repeat(pad)).id).toBe('a');
    expect(failure(() => parseEnvelope(validText + ' '.repeat(pad + 1)))).toBe(
      'layout file is larger than 64 KiB',
    );
  });

  it('counts bytes, not UTF-16 units', () => {
    // 40 000 UTF-16 units but 80 000 bytes.
    expect(failure(() => parseEnvelope('é'.repeat(40_000)))).toBe(
      'layout file is larger than 64 KiB',
    );
    // 20 000 emoji: 40 000 units, 80 000 bytes.
    expect(failure(() => parseEnvelope('😀'.repeat(20_000)))).toBe(
      'layout file is larger than 64 KiB',
    );
  });

  it('refuses a huge string quickly without parsing it', () => {
    const huge = 'x'.repeat(10 * 1024 * 1024);
    const started = performance.now();
    expect(failure(() => parseEnvelope(huge))).toBe('layout file is larger than 64 KiB');
    expect(performance.now() - started).toBeLessThan(500);
  });

  it('refuses a huge valid-looking JSON string value', () => {
    const text = envelope({ id: 'a', name: 'x'.repeat(100_000), zones: [{ id: 1, rect: unit }] });
    expect(failure(() => parseEnvelope(text))).toBe('layout file is larger than 64 KiB');
  });

  it('survives very deep nesting that fits in 64 KiB', () => {
    // 30 000 levels of [] are 60 000 characters: under the size limit, far over the depth limit.
    const arrays = '['.repeat(30_000) + ']'.repeat(30_000);
    expect(bytes(arrays)).toBeLessThan(MAX_IMPORT_BYTES);
    expect(failure(() => parseEnvelope(arrays))).toBe('that is not valid JSON');

    const objects = '{"a":'.repeat(10_000) + '1' + '}'.repeat(10_000);
    expect(bytes(objects)).toBeLessThan(MAX_IMPORT_BYTES);
    expect(failure(() => parseEnvelope(objects))).toBe('that is not valid JSON');

    // Deeper still and the size limit answers first.
    const huge = '{"a":'.repeat(14_000) + '1' + '}'.repeat(14_000);
    expect(failure(() => parseEnvelope(huge))).toBe('layout file is larger than 64 KiB');
  });

  it('draws the nesting line at 128 levels and ignores brackets inside strings', () => {
    expect(failure(() => parseEnvelope('['.repeat(128) + ']'.repeat(128)))).toBe(
      'not a Krova layout file',
    );
    expect(failure(() => parseEnvelope('['.repeat(129) + ']'.repeat(129)))).toBe(
      'that is not valid JSON',
    );
    // Brackets, braces and escaped quotes inside a string value do not count as nesting.
    const brackets = envelope({
      id: 'a',
      name: 'n',
      note: '[[[[{{{{ \\" ' + '['.repeat(500),
      zones: [{ id: 1, rect: unit }],
    });
    expect(parseEnvelope(brackets).id).toBe('a');
  });

  it('survives many tiny nested values and long numbers', () => {
    const wide = `[${'[],'.repeat(20_000)}[]]`;
    expect(bytes(wide)).toBeLessThan(MAX_IMPORT_BYTES);
    expect(failure(() => parseEnvelope(wide))).toBe('not a Krova layout file');
    const longNumber = `{"krova_layout":1,"layout":{"id":"a","name":"n","zones":[{"id":1,"rect":{"x":0,"y":0,"w":0.${'9'.repeat(1000)},"h":1}}]}}`;
    expect(parseEnvelope(longNumber).zones[0].rect.w).toBeCloseTo(1, 12);
  });

  it('never returns an object that shares anything with the input text', () => {
    const a = parseEnvelope(validText);
    const b = parseEnvelope(validText);
    a.zones[0].rect.w = 0.1;
    expect(b.zones[0].rect.w).toBe(1);
  });
});

describe('uniqueLayoutId', () => {
  it('keeps a free id', () => {
    expect(uniqueLayoutId(['a', 'b'], 'c')).toBe('c');
    expect(uniqueLayoutId([], 'c')).toBe('c');
  });

  it('adds -2, then -3, ... for a taken id', () => {
    expect(uniqueLayoutId(['grid'], 'grid')).toBe('grid-2');
    expect(uniqueLayoutId(['grid', 'grid-2'], 'grid')).toBe('grid-3');
    expect(uniqueLayoutId(['grid', 'grid-3'], 'grid')).toBe('grid-2');
    expect(uniqueLayoutId(['grid-2'], 'grid-2')).toBe('grid-2-2');
  });

  it('keeps a long id within 64 characters', () => {
    const id = 'x'.repeat(64);
    const first = uniqueLayoutId([id], id);
    expect(first).toBe(`${'x'.repeat(62)}-2`);
    expect([...first]).toHaveLength(64);
    const taken = [id, first];
    const second = uniqueLayoutId(taken, id);
    expect(second).toBe(`${'x'.repeat(62)}-3`);
    // Two-digit suffixes shorten the base further.
    const many = [id, ...Array.from({ length: 8 }, (_, i) => `${'x'.repeat(62)}-${i + 2}`)];
    const tenth = uniqueLayoutId(many, id);
    expect(tenth).toBe(`${'x'.repeat(61)}-10`);
    expect([...tenth]).toHaveLength(64);
  });

  it('shortens by characters, not UTF-16 units', () => {
    const id = '😀'.repeat(64);
    const next = uniqueLayoutId([id], id);
    expect([...next]).toHaveLength(64);
    expect(next.endsWith('-2')).toBe(true);
    expect(next.startsWith('😀'.repeat(62))).toBe(true);
  });
});
