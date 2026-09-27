// Keeping the visualizer safe to look at (PLAN.md F18). WCAG 2.3.1 allows
// no more than three flashes in any second. Two guards, pure so plain
// `node --test` covers them:
//
// - beats reach the renderers at most three times a second (the core sends
//   up to four), and not at all in calm mode (reduced motion);
// - the picture's average brightness is watched: a flash is a swing of
//   `SWING` or more, a flash a pair of opposite swings, and while more than
//   `MAX_FLASHES` fall within a second the picture is dimmed, which cuts
//   the swings' contrast, easing back once it's quiet.

/** Most flashes (or beats) in any second. */
export const MAX_FLASHES = 3;
/** A change in average relative luminance (0..1) that counts as half a flash. */
export const SWING = 0.1;

export class FlashGuard {
  #beats: number[] = [];
  #swings: number[] = [];
  #level: number | null = null;
  /** The brightness a swing is measured from, and which way the last one went. */
  #anchor = 0;
  #direction = 0;
  #dim = 0;

  /** Whether a beat at `now` (seconds) may show. */
  beat(now: number, calm: boolean): boolean {
    if (calm) return false;
    this.#beats = this.#beats.filter((at) => now - at < 1);
    if (this.#beats.length >= MAX_FLASHES) return false;
    this.#beats.push(now);
    return true;
  }

  /** Records the picture's average luminance at `now`; returns how much to
      dim the next frames, 0 (not at all) to 0.7. */
  luminance(now: number, level: number, dt: number): number {
    if (this.#level === null) {
      this.#level = this.#anchor = level;
    }
    this.#level = level;
    const change = level - this.#anchor;
    if (Math.abs(change) >= SWING) {
      const direction = Math.sign(change);
      // A reversal: every two make a flash.
      if (this.#direction !== 0 && direction !== this.#direction) this.#swings.push(now);
      this.#direction = direction;
      this.#anchor = level;
    }
    this.#swings = this.#swings.filter((at) => now - at < 1);
    const target = this.#swings.length > 2 * MAX_FLASHES ? 0.7 : 0;
    // Dims at once; eases back over about two seconds.
    this.#dim = target > this.#dim ? target : Math.max(target, this.#dim - dt * 0.35);
    return this.#dim;
  }
}

/** Relative luminance (0..1) of sRGB pixels, averaged: RGBA bytes. */
export function averageLuminance(pixels: Uint8ClampedArray): number {
  const linear = (value: number) => {
    const c = value / 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  let sum = 0;
  let count = 0;
  for (let i = 0; i + 3 < pixels.length; i += 4) {
    sum += 0.2126 * linear(pixels[i]) + 0.7152 * linear(pixels[i + 1]) + 0.0722 * linear(pixels[i + 2]);
    count++;
  }
  return count === 0 ? 0 : sum / count;
}
