// Physics for the UI's direct-manipulation surfaces (the layout switcher deck,
// its toast, anything that follows a finger). One spring model, stepped at a
// fixed 240 Hz so the feel does not depend on the display's frame rate.
//
// Pure TypeScript: no DOM access except `prefersReducedMotion()` and `Loop`,
// so the maths is unit-tested in node (motion.test.ts).

/** Fixed integration step (seconds). */
export const SUBSTEP = 1 / 240;
/** Longest frame we integrate; a stalled tab must not fling everything away. */
export const MAX_FRAME = 0.05;

/**
 * Shared motion preferences. `reduced` mirrors `prefers-reduced-motion: reduce`;
 * while it is true every spring jumps straight to its target.
 */
export const motionPrefs = { reduced: false };

/** Read `prefers-reduced-motion` and keep `motionPrefs.reduced` in sync. */
export function watchReducedMotion(): () => void {
  if (typeof matchMedia === 'undefined') return () => {};
  const mq = matchMedia('(prefers-reduced-motion: reduce)');
  const sync = (): void => {
    motionPrefs.reduced = mq.matches;
  };
  sync();
  mq.addEventListener('change', sync);
  return () => mq.removeEventListener('change', sync);
}

export interface SpringOptions {
  /** Stiffness (1/s^2). Higher = faster. */
  k?: number;
  /** Damping ratio: 1 = critically damped, < 1 overshoots a little. */
  zeta?: number;
}

/** Damped harmonic oscillator. `vel` is in units per second. */
export class Spring {
  value: number;
  target: number;
  vel = 0;
  readonly k: number;
  readonly c: number;
  /** Carry-over of time shorter than one substep, so 60 and 144 Hz agree. */
  private acc = 0;

  constructor(value: number, { k = 420, zeta = 0.86 }: SpringOptions = {}) {
    this.value = value;
    this.target = value;
    this.k = k;
    this.c = 2 * Math.sqrt(k) * zeta;
  }

  /** Move towards `target`, optionally starting with velocity `vel`. */
  set(target: number, opts: { vel?: number } = {}): void {
    this.target = target;
    if (opts.vel !== undefined) this.vel = opts.vel;
    if (motionPrefs.reduced) this.jump(target);
  }

  /** Teleport: value = target, no velocity. */
  jump(value: number): void {
    this.value = this.target = value;
    this.vel = 0;
    this.acc = 0;
  }

  /** Advance by `dt` seconds (semi-implicit Euler, fixed substeps). */
  step(dt: number): void {
    this.acc += Math.min(Math.max(dt, 0), MAX_FRAME);
    while (this.acc >= SUBSTEP) {
      this.acc -= SUBSTEP;
      const a = -this.k * (this.value - this.target) - this.c * this.vel;
      this.vel += a * SUBSTEP;
      this.value += this.vel * SUBSTEP;
    }
    if (this.settled) {
      this.value = this.target;
      this.vel = 0;
    }
  }

  get settled(): boolean {
    return Math.abs(this.value - this.target) < 1e-3 && Math.abs(this.vel) < 1e-2;
  }
}

/**
 * Rubber band: how far content follows a drag of `over` past its end.
 * Starts at `slope` (0.55) and approaches `limit` asymptotically.
 */
export function rubber(over: number, limit = 3, slope = 0.55): number {
  if (over <= 0) return over;
  return (over * slope * limit) / (over * slope + limit);
}

/** Apply the rubber band to `pos` outside `[min, max]`. */
export function rubberClamp(pos: number, min: number, max: number, limit = 3): number {
  if (pos < min) return min - rubber(min - pos, limit);
  if (pos > max) return max + rubber(pos - max, limit);
  return pos;
}

export interface Sample {
  x: number;
  y: number;
  /** milliseconds */
  t: number;
}

/** How much pointer history the velocity estimate looks at (ms). */
export const VELOCITY_WINDOW_MS = 80;

/** Keep only the samples of the last `VELOCITY_WINDOW_MS` (at least two). */
export function trimHistory(hist: Sample[], now: number): void {
  while (hist.length > 2 && now - hist[0].t > VELOCITY_WINDOW_MS) hist.shift();
}

/** Pointer velocity in px per millisecond, from the oldest to the newest sample. */
export function velocityOf(hist: readonly Sample[]): { vx: number; vy: number } {
  if (hist.length < 2) return { vx: 0, vy: 0 };
  const a = hist[0];
  const b = hist[hist.length - 1];
  const dt = b.t - a.t || 1;
  return { vx: (b.x - a.x) / dt, vy: (b.y - a.y) / dt };
}

/**
 * Velocity at release (px/ms): zero when the pointer rested for longer than the
 * velocity window before it was released (a pause kills the flick).
 */
export function releaseVelocity(hist: Sample[], now: number): { vx: number; vy: number } {
  const last = hist[hist.length - 1];
  if (!last || now - last.t > VELOCITY_WINDOW_MS) return { vx: 0, vy: 0 };
  trimHistory(hist, now);
  return velocityOf(hist);
}

/**
 * Where a flick ends: the position the content would coast to in `lookahead`
 * seconds at velocity `vel` (units per second), rounded to a whole slot and
 * clamped to `[0, count - 1]`.
 */
export function projectIndex(pos: number, vel: number, count: number, lookahead = 0.18): number {
  if (count <= 0) return 0;
  return clamp(Math.round(pos + vel * lookahead), 0, count - 1);
}

export function clamp(v: number, lo: number, hi: number): number {
  return Math.max(lo, Math.min(hi, v));
}

/**
 * A requestAnimationFrame loop that only runs while `frame` says something is
 * still moving. `kick()` (re)starts it; it is cheap to call often.
 */
export class Loop {
  private raf = 0;
  private last = 0;
  constructor(private readonly frame: (dt: number) => boolean) {}

  kick(): void {
    if (this.raf) return;
    this.last = performance.now();
    this.raf = requestAnimationFrame(this.tick);
  }

  stop(): void {
    if (this.raf) cancelAnimationFrame(this.raf);
    this.raf = 0;
  }

  private tick = (now: number): void => {
    const dt = Math.min(MAX_FRAME, (now - this.last) / 1000);
    this.last = now;
    this.raf = this.frame(dt) ? requestAnimationFrame(this.tick) : 0;
  };
}
