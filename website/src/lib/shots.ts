import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { url } from './url';

/**
 * Product screenshots live in public/screenshots/<name>-light.png and <name>-dark.png and are produced by
 * `npm run screenshots` (scripts/capture-screenshots.mjs), which drives the real UI at 2x pixel ratio.
 *
 * Their size is read from the PNG header at build time, so re-running the capture script after the desktop UI
 * changes needs no edit here: the new file is picked up with its new size. Width and height are written on
 * every <img> so the page does not jump while it loads.
 */

/** The capture script renders at this device pixel ratio. */
const DPR = 2;

export interface Shot {
  name: string;
  /** Intrinsic size in CSS pixels (file size divided by the pixel ratio). */
  width: number;
  height: number;
  light: string;
  dark: string;
}

const cache = new Map<string, Shot>();

export function shot(name: string): Shot {
  const hit = cache.get(name);
  if (hit) return hit;
  const dir = resolve(process.cwd(), 'public', 'screenshots');
  const file = resolve(dir, `${name}-light.png`);
  if (!existsSync(file) || !existsSync(resolve(dir, `${name}-dark.png`))) {
    throw new Error(
      `Screenshot "${name}" is missing in ${dir} (needs ${name}-light.png and ${name}-dark.png). ` +
        'Run "npm run screenshots" in website/ (it needs "npm ci --prefix ../ui" once).',
    );
  }
  // PNG: 8-byte signature, then the IHDR chunk: length (4), "IHDR" (4), width (4), height (4).
  const head = readFileSync(file).subarray(0, 24);
  if (head.toString('ascii', 12, 16) !== 'IHDR') throw new Error(`${file} is not a PNG file.`);
  const result: Shot = {
    name,
    width: Math.round(head.readUInt32BE(16) / DPR),
    height: Math.round(head.readUInt32BE(20) / DPR),
    light: url(`/screenshots/${name}-light.png`),
    dark: url(`/screenshots/${name}-dark.png`),
  };
  cache.set(name, result);
  return result;
}
