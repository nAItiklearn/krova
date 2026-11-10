// A tiny critically-tunable spring, stepped by the caller (usually from
// requestAnimationFrame). Shared by the panel and other animated surfaces.
//
//   const s = new Spring(0, { stiffness: 380, damping: 30 });
//   s.set(1);
//   frame(dt) { s.step(dt); el.style.opacity = String(s.value); if (!s.settled) next(); }

export interface SpringOptions {
  /** Spring constant (1/s^2 with unit mass). Default 380. */
  stiffness?: number;
  /** Damping coefficient (1/s). Default 30. */
  damping?: number;
  /** Distance and speed under which the spring counts as at rest. Default 0.001. */
  precision?: number;
}

export class Spring {
  value: number;
  target: number;
  vel = 0;
  readonly stiffness: number;
  readonly damping: number;
  readonly precision: number;

  constructor(value = 0, opts: SpringOptions = {}) {
    this.value = value;
    this.target = value;
    this.stiffness = opts.stiffness ?? 380;
    this.damping = opts.damping ?? 30;
    this.precision = opts.precision ?? 0.001;
  }

  /** Aim at `t`; optionally kick it with an initial velocity. */
  set(t: number, opts: { vel?: number } = {}): void {
    this.target = t;
    if (opts.vel !== undefined) this.vel = opts.vel;
  }

  /** Jump to `v` and stop (no animation). */
  jump(v: number): void {
    this.value = v;
    this.target = v;
    this.vel = 0;
  }

  /** Advance by `dt` seconds (sub-stepped, so long frames stay stable). */
  step(dt: number): void {
    let left = Math.min(Math.max(dt, 0), 0.1);
    while (left > 0) {
      const h = Math.min(left, 1 / 240);
      const force = -this.stiffness * (this.value - this.target) - this.damping * this.vel;
      this.vel += force * h;
      this.value += this.vel * h;
      left -= h;
    }
    if (this.settled) {
      this.value = this.target;
      this.vel = 0;
    }
  }

  get settled(): boolean {
    return Math.abs(this.value - this.target) < this.precision && Math.abs(this.vel) < this.precision;
  }
}

/** True when the user asked the OS for less motion. */
export function prefersReducedMotion(): boolean {
  return typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion: reduce)').matches;
}
