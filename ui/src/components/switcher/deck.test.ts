import { describe, expect, it } from 'vitest';
import {
  DISMISS_DISTANCE,
  STEP_BEHIND,
  below,
  classify,
  dismissHint,
  dismisses,
  focusY,
  moved,
  place,
  slotAt,
  swipeFade,
  unplace,
} from './deck';

const H = 260;

describe('place', () => {
  it('the focused card is in front, full size and opacity', () => {
    expect(place(0, H)).toEqual({ y: 0, s: 1, o: 1, shade: 0 });
  });

  it('cards behind stack upwards, smaller and darker per step', () => {
    const a = place(-1, H);
    const b = place(-2, H);
    expect(a.y).toBe(-STEP_BEHIND);
    expect(b.y).toBe(-2 * STEP_BEHIND);
    expect(b.s).toBeLessThan(a.s);
    expect(b.shade).toBeGreaterThan(a.shade);
    expect(place(-20, H).s).toBe(0.82);
    expect(place(-20, H).shade).toBe(0.5);
  });

  it('only about three cards behind are visible', () => {
    expect(place(-3, H).o).toBe(1);
    expect(place(-5, H).o).toBe(0);
  });

  it('flipped-past cards slide down and fade out', () => {
    const p = place(1, H);
    expect(p.y).toBe(below(H));
    expect(p.o).toBe(0);
    expect(place(0.5, H).o).toBeCloseTo(0.45, 5);
  });

  it('is continuous at d = 0', () => {
    const l = place(-1e-6, H);
    const r = place(1e-6, H);
    expect(Math.abs(l.y - r.y)).toBeLessThan(1e-3);
    expect(Math.abs(l.s - r.s)).toBeLessThan(1e-3);
  });
});

describe('unplace', () => {
  it('inverts place(d).y on both sides', () => {
    for (const d of [-3, -1.5, -0.2, 0, 0.3, 0.9]) {
      expect(unplace(place(d, H).y, H)).toBeCloseTo(d, 10);
    }
  });
});

describe('focusY and slotAt', () => {
  it('leaves room above for four title strips', () => {
    expect(focusY(300, H)).toBe(120 + 4 * STEP_BEHIND);
    expect(focusY(1000, H)).toBeGreaterThan(120 + 4 * STEP_BEHIND);
  });

  it('a lifted card at the focus line is over the focused slot', () => {
    const fy = focusY(900, H);
    expect(slotAt(fy, 2, 900, H, 6)).toBe(2);
    expect(slotAt(fy - STEP_BEHIND, 2, 900, H, 6)).toBe(3);
    expect(slotAt(fy + below(H), 2, 900, H, 6)).toBe(1);
    expect(slotAt(fy - 50 * STEP_BEHIND, 2, 900, H, 6)).toBe(5);
    expect(slotAt(fy + 50 * H, 2, 900, H, 6)).toBe(0);
  });
});

describe('gestures', () => {
  it('stays a tap inside the slop', () => {
    expect(classify(3, 3, true, 4)).toBe('pending');
  });
  it('sideways on a card is a swipe, otherwise a scroll', () => {
    expect(classify(30, 5, true, 4)).toBe('swipe');
    expect(classify(30, 5, false, 4)).toBe('scroll');
    expect(classify(5, 30, true, 4)).toBe('scroll');
    expect(classify(12, 11, true, 4)).toBe('scroll');
  });
  it('the last layout cannot be swiped away', () => {
    expect(classify(30, 0, true, 1)).toBe('scroll');
  });
  it('a swipe deletes past the distance or with a flick', () => {
    expect(dismisses(DISMISS_DISTANCE + 1, 0)).toBe(true);
    expect(dismisses(-DISMISS_DISTANCE - 1, 0)).toBe(true);
    expect(dismisses(40, 1.2)).toBe(true);
    expect(dismisses(100, 0.2)).toBe(false);
  });
  it('the red edge and fade grow with the swipe', () => {
    expect(dismissHint(40)).toBe(0);
    expect(dismissHint(120)).toBeCloseTo(0.5, 10);
    expect(dismissHint(-500)).toBe(1);
    expect(swipeFade(0)).toBe(1);
    expect(swipeFade(260)).toBeCloseTo(0.5, 10);
  });
});

describe('moved', () => {
  it('moves without mutating', () => {
    const a = ['a', 'b', 'c', 'd'];
    expect(moved(a, 0, 2)).toEqual(['b', 'c', 'a', 'd']);
    expect(moved(a, 3, 0)).toEqual(['d', 'a', 'b', 'c']);
    expect(moved(a, 1, 99)).toEqual(['a', 'c', 'd', 'b']);
    expect(a).toEqual(['a', 'b', 'c', 'd']);
  });
});
