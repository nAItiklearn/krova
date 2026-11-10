import { afterEach, describe, expect, it } from 'vitest';
import {
  SUBSTEP,
  Spring,
  clamp,
  motionPrefs,
  projectIndex,
  releaseVelocity,
  rubber,
  rubberClamp,
  trimHistory,
  velocityOf,
  type Sample,
} from './motion';

function run(s: Spring, seconds: number, frame = 1 / 60): void {
  for (let t = 0; t < seconds; t += frame) s.step(frame);
}

afterEach(() => {
  motionPrefs.reduced = false;
});

describe('Spring', () => {
  it('settles on its target and snaps exactly once settled', () => {
    const s = new Spring(0);
    s.set(10);
    run(s, 2);
    expect(s.settled).toBe(true);
    expect(s.value).toBe(10);
    expect(s.vel).toBe(0);
  });

  it('does not depend on the frame rate (fixed substeps)', () => {
    const a = new Spring(0);
    const b = new Spring(0);
    a.set(1);
    b.set(1);
    run(a, 0.2, 1 / 60);
    run(b, 0.2, 1 / 144);
    expect(Math.abs(a.value - b.value)).toBeLessThan(0.02);
  });

  it('a frame shorter than one substep carries over instead of being lost', () => {
    const s = new Spring(0);
    s.set(1);
    for (let i = 0; i < 12; i++) s.step(SUBSTEP / 3);
    expect(s.value).toBeGreaterThan(0);
  });

  it('underdamped springs overshoot, critically damped ones do not', () => {
    const loose = new Spring(0, { k: 400, zeta: 0.5 });
    const firm = new Spring(0, { k: 400, zeta: 1 });
    let maxLoose = 0;
    let maxFirm = 0;
    loose.set(1);
    firm.set(1);
    for (let i = 0; i < 120; i++) {
      loose.step(1 / 60);
      firm.step(1 / 60);
      maxLoose = Math.max(maxLoose, loose.value);
      maxFirm = Math.max(maxFirm, firm.value);
    }
    expect(maxLoose).toBeGreaterThan(1.05);
    expect(maxFirm).toBeLessThanOrEqual(1 + 1e-6);
  });

  it('initial velocity carries a flick past the start', () => {
    const s = new Spring(0, { k: 300, zeta: 0.9 });
    s.set(0, { vel: 20 });
    s.step(0.05);
    expect(s.value).toBeGreaterThan(0.3);
  });

  it('clamps huge frames (a stalled tab) to MAX_FRAME', () => {
    const s = new Spring(0);
    s.set(100);
    s.step(10);
    expect(s.value).toBeLessThan(100);
    expect(Number.isFinite(s.value)).toBe(true);
  });

  it('jumps to the target with reduced motion', () => {
    motionPrefs.reduced = true;
    const s = new Spring(0);
    s.set(5, { vel: 99 });
    expect(s.value).toBe(5);
    expect(s.vel).toBe(0);
    expect(s.settled).toBe(true);
  });
});

describe('rubber band', () => {
  it('is zero at the edge, follows at slope 0.55, never passes the limit', () => {
    expect(rubber(0)).toBe(0);
    expect(rubber(0.01)).toBeCloseTo(0.0055, 4);
    expect(rubber(1000)).toBeLessThan(3);
    expect(rubber(1000)).toBeGreaterThan(2.9);
    expect(rubber(2)).toBeGreaterThan(rubber(1));
  });

  it('only bends positions outside the range', () => {
    expect(rubberClamp(2, 0, 5)).toBe(2);
    expect(rubberClamp(-1, 0, 5)).toBeCloseTo(-rubber(1), 10);
    expect(rubberClamp(7, 0, 5)).toBeCloseTo(5 + rubber(2), 10);
    expect(rubberClamp(-100, 0, 5)).toBeGreaterThan(-3);
  });
});

describe('velocity', () => {
  it('measures px/ms over the recent window only', () => {
    const h: Sample[] = [
      { x: 0, y: 0, t: 0 },
      { x: 0, y: 0, t: 100 },
      { x: 50, y: 100, t: 150 },
      { x: 100, y: 200, t: 200 },
    ];
    trimHistory(h, 200);
    expect(h[0].t).toBe(150);
    expect(velocityOf(h)).toEqual({ vx: 1, vy: 2 });
  });

  it('a pause before release kills the flick', () => {
    const h: Sample[] = [
      { x: 0, y: 0, t: 0 },
      { x: 0, y: 100, t: 50 },
    ];
    expect(releaseVelocity(h.slice(), 60)).toEqual({ vx: 0, vy: 2 });
    expect(releaseVelocity(h.slice(), 200)).toEqual({ vx: 0, vy: 0 });
  });

  it('is zero without movement data', () => {
    expect(velocityOf([])).toEqual({ vx: 0, vy: 0 });
    expect(velocityOf([{ x: 3, y: 3, t: 5 }])).toEqual({ vx: 0, vy: 0 });
  });
});

describe('projectIndex', () => {
  it('snaps to the nearest slot at rest', () => {
    expect(projectIndex(1.4, 0, 5)).toBe(1);
    expect(projectIndex(1.6, 0, 5)).toBe(2);
  });
  it('a flick carries on in its direction', () => {
    expect(projectIndex(1.2, 10, 6)).toBe(3);
    expect(projectIndex(3.1, -10, 6)).toBe(1);
  });
  it('stays inside the list', () => {
    expect(projectIndex(0, -50, 4)).toBe(0);
    expect(projectIndex(3, 50, 4)).toBe(3);
    expect(projectIndex(2, 0, 0)).toBe(0);
  });
  it('clamp', () => {
    expect(clamp(5, 0, 3)).toBe(3);
    expect(clamp(-1, 0, 3)).toBe(0);
  });
});
