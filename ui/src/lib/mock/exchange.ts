// Layout export / import for the demo mock: the envelope format, the 64 KiB size
// limit, the shape check, the per-monitor id de-duplication and the cap of 64
// layouts per monitor. It mirrors the contract the Rust commands `export_layout`
// and `import_layout` implement; the real app never runs this code. Layout
// validation itself is `validateLayout` (edit.ts), the mirror of Layout::validate.
//
// Every failure is an `ExchangeError` whose message is the exact string the
// backend rejects with, so the UI can show it as is.

import type { Layout, Rect, Zone } from '../bindings';
import { MAX_NAME_LEN, validateLayout } from './edit';

/** The only envelope version there is: `{"krova_layout":1,"layout":{...}}`. */
export const ENVELOPE_VERSION = 1;
/** Largest import text, in UTF-8 bytes (64 KiB). */
export const MAX_IMPORT_BYTES = 64 * 1024;
/** config.rs MAX_LAYOUTS_PER_MONITOR. */
export const MAX_LAYOUTS_PER_MONITOR = 64;
/** serde_json refuses nesting deeper than this; so does the mock (as "not valid JSON"). */
export const MAX_JSON_DEPTH = 128;

export const messages = {
  tooLarge: 'layout file is larger than 64 KiB',
  notJson: 'that is not valid JSON',
  notKrova: 'not a Krova layout file',
  unsupported: (version: number) => `unsupported layout file version ${version}`,
  /** The envelope is fine but `layout` is not shaped like a layout (my wording; see the docs). */
  badLayout: 'that file does not contain a valid layout',
  tooManyLayouts: 'a monitor can hold at most 64 layouts',
};

export class ExchangeError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'ExchangeError';
  }
}

const MAX_U32 = 4_294_967_295;

type Json = Record<string, unknown>;

function isObject(v: unknown): v is Json {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

/** Own property only, so `__proto__` / `constructor` tricks read as missing. */
function field(o: Json, key: string): unknown {
  return Object.hasOwn(o, key) ? o[key] : undefined;
}

/** UTF-8 length of `s` without allocating for texts that are plainly too long. */
function utf8Length(s: string): number {
  // Every UTF-16 code unit takes at least one byte, so a longer string cannot fit.
  if (s.length > MAX_IMPORT_BYTES) return s.length;
  return new TextEncoder().encode(s).length;
}

/** Deepest [ / { nesting outside of strings, found without parsing. */
function nestingDepth(text: string): number {
  let depth = 0;
  let deepest = 0;
  let inString = false;
  let escaped = false;
  for (let i = 0; i < text.length; i++) {
    const c = text.charCodeAt(i);
    if (inString) {
      if (escaped) escaped = false;
      else if (c === 92) escaped = true; // backslash
      else if (c === 34) inString = false; // quote
      continue;
    }
    if (c === 34) inString = true;
    else if (c === 91 || c === 123) {
      depth += 1;
      if (depth > deepest) deepest = depth;
    } else if (c === 93 || c === 125) depth -= 1;
  }
  return deepest;
}

function readRect(v: unknown): Rect {
  if (!isObject(v)) throw new ExchangeError(messages.badLayout);
  const out: number[] = [];
  for (const key of ['x', 'y', 'w', 'h']) {
    const n = field(v, key);
    // 1e999 parses to Infinity here; validateLayout then calls the rectangle invalid.
    if (typeof n !== 'number') throw new ExchangeError(messages.badLayout);
    out.push(n);
  }
  return { x: out[0], y: out[1], w: out[2], h: out[3] };
}

function readZone(v: unknown): Zone {
  if (!isObject(v)) throw new ExchangeError(messages.badLayout);
  const id = field(v, 'id');
  // Rust zone ids are u32.
  if (typeof id !== 'number' || !Number.isInteger(id) || id < 0 || id > MAX_U32) {
    throw new ExchangeError(messages.badLayout);
  }
  return { id, rect: readRect(field(v, 'rect')) };
}

/** Copy only the known fields (extra keys are ignored, like serde does). */
function readLayout(v: unknown): Layout {
  if (!isObject(v)) throw new ExchangeError(messages.badLayout);
  const id = field(v, 'id');
  const name = field(v, 'name');
  const zones = field(v, 'zones');
  if (typeof id !== 'string' || typeof name !== 'string' || !Array.isArray(zones)) {
    throw new ExchangeError(messages.badLayout);
  }
  return { id, name, zones: zones.map(readZone) };
}

/**
 * Text of `export_layout`: the validated layout inside the version-1 envelope,
 * pretty-printed with two spaces like serde_json.
 */
export function exportEnvelope(layout: Layout): string {
  validateLayout(layout);
  const clean: Layout = {
    id: layout.id,
    name: layout.name,
    zones: layout.zones.map((z) => ({
      id: z.id,
      rect: { x: z.rect.x, y: z.rect.y, w: z.rect.w, h: z.rect.h },
    })),
  };
  return JSON.stringify({ krova_layout: ENVELOPE_VERSION, layout: clean }, null, 2);
}

/**
 * Check order of `import_layout`: size, JSON syntax and depth, envelope, layout
 * shape, then Layout::validate. Returns the layout exactly as written in the file
 * (the caller makes its id unique on the target monitor).
 */
export function parseEnvelope(json: string): Layout {
  if (utf8Length(json) > MAX_IMPORT_BYTES) throw new ExchangeError(messages.tooLarge);
  if (nestingDepth(json) > MAX_JSON_DEPTH) throw new ExchangeError(messages.notJson);
  let root: unknown;
  try {
    root = JSON.parse(json);
  } catch {
    throw new ExchangeError(messages.notJson);
  }
  if (!isObject(root)) throw new ExchangeError(messages.notKrova);
  const version = field(root, 'krova_layout');
  if (typeof version !== 'number' || !Number.isFinite(version)) {
    throw new ExchangeError(messages.notKrova);
  }
  if (version !== ENVELOPE_VERSION) throw new ExchangeError(messages.unsupported(version));
  const layout = readLayout(field(root, 'layout'));
  try {
    validateLayout(layout);
  } catch (e) {
    throw new ExchangeError(e instanceof Error ? e.message : String(e));
  }
  return layout;
}

/** First `max` Unicode scalar values of `s`. */
function firstChars(s: string, max: number): string {
  let out = '';
  let n = 0;
  for (const ch of s) {
    if (n >= max) break;
    out += ch;
    n += 1;
  }
  return out;
}

/**
 * `id` if no layout on the monitor has it, else `id-2`, `id-3`, ... (the base is
 * shortened when needed so the result still fits the 64-character limit).
 */
export function uniqueLayoutId(existing: Iterable<string>, id: string): string {
  const taken = new Set(existing);
  if (!taken.has(id)) return id;
  for (let n = 2; ; n++) {
    const suffix = `-${n}`;
    const candidate = firstChars(id, MAX_NAME_LEN - suffix.length) + suffix;
    if (!taken.has(candidate)) return candidate;
  }
}
