/**
 * Zone maths for the "zones are fractions of a monitor" diagram.
 *
 * Mirrors crates/krova-core/src/geometry.rs `PxRect::denormalize`: each edge is
 * rounded on its own (origin + round(fraction * size)), and the width is the
 * difference of two rounded edges, so neighbouring zones share an edge exactly
 * and tile with no gaps or overlaps. Rust's f64::round rounds halves away from
 * zero; Math.round rounds halves up. They agree for the non-negative values
 * used here.
 */

export interface NormZone {
  id: number;
  label: string;
  x: number;
  w: number;
}

/** The Priority template (crates/krova-core/src/zone.rs, ui/src/lib/mock/templates.ts). */
export const priorityZones: readonly NormZone[] = [
  { id: 1, label: 'Zone 1', x: 0, w: 0.25 },
  { id: 2, label: 'Zone 2', x: 0.25, w: 0.5 },
  { id: 3, label: 'Zone 3', x: 0.75, w: 0.25 },
];

export interface PxSpan {
  id: number;
  left: number;
  right: number;
  width: number;
}

export function denormalizeSpan(z: NormZone, size: number): PxSpan {
  const left = Math.round(z.x * size);
  const right = Math.round((z.x + z.w) * size);
  return { id: z.id, left, right, width: right - left };
}

export interface MonitorSpec {
  name: string;
  width: number;
  height: number;
}

export const exampleMonitors: readonly MonitorSpec[] = [
  { name: '1920 × 1080', width: 1920, height: 1080 },
  { name: '2560 × 1440', width: 2560, height: 1440 },
  { name: '1366 × 768', width: 1366, height: 768 },
];

export const fmt = (n: number): string => n.toLocaleString('en-US');
