import { describe, expect, it } from 'vitest';
import { Spring } from './motion';

describe('Spring', () => {
  it('settles on the target', () => {
    const s = new Spring(0.96);
    s.set(1);
    for (let i = 0; i < 120 && !s.settled; i++) s.step(1 / 60);
    expect(s.settled).toBe(true);
    expect(s.value).toBe(1);
    expect(s.vel).toBe(0);
  });

  it('overshoots a little when kicked (springy, not linear)', () => {
    const s = new Spring(8, { stiffness: 420, damping: 22 });
    s.set(0);
    let min = Infinity;
    for (let i = 0; i < 120; i++) {
      s.step(1 / 60);
      min = Math.min(min, s.value);
    }
    expect(min).toBeLessThan(0);
    expect(s.settled).toBe(true);
  });

  it('jump stops at once; a long frame stays stable', () => {
    const s = new Spring(0);
    s.set(100, { vel: 50 });
    s.step(5);
    expect(Number.isFinite(s.value)).toBe(true);
    s.jump(3);
    expect(s.value).toBe(3);
    expect(s.target).toBe(3);
    expect(s.settled).toBe(true);
  });
});
