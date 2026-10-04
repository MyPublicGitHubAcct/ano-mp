// The recurrence plot's self-similarity matrix (PLAN.md Phase 7b, X3): the
// music summarised every `step` seconds as a feature (what notes sound and
// the shape of the spectrum), each compared with every other, so a chorus
// that comes back shows as a bright diagonal off the main one. The grid
// holds at most `capacity` steps: once it fills, neighbouring steps merge
// and the step doubles, so the whole track always fits. Pure and without
// imports, so plain `node --test` covers it.

const CHROMA = 12;
const TIMBRE = 8;
export const FEATURE_SIZE = CHROMA + TIMBRE;
/** How much the notes count against the spectrum's shape in a similarity. */
const CHROMA_WEIGHT = 0.6;

/** A feature from one analysis: the chroma and the bands pooled into `TIMBRE` groups, about their mean.
    Not normalised; `Recurrence` sums these over a step. */
export function feature(chroma: ArrayLike<number>, bands: ArrayLike<number>, into = new Float32Array(FEATURE_SIZE)) {
  for (let i = 0; i < CHROMA; i++) into[i] = chroma[i];
  let mean = 0;
  for (let g = 0; g < TIMBRE; g++) {
    const from = Math.floor((g * bands.length) / TIMBRE);
    const to = Math.max(from + 1, Math.floor(((g + 1) * bands.length) / TIMBRE));
    let sum = 0;
    for (let i = from; i < to && i < bands.length; i++) sum += bands[i];
    into[CHROMA + g] = sum / (to - from);
    mean += into[CHROMA + g] / TIMBRE;
  }
  for (let g = 0; g < TIMBRE; g++) into[CHROMA + g] -= mean;
  return into;
}

/** Scales each part of a summed feature to unit length (a silent part stays zero). */
function normalise(values: Float32Array) {
  for (const [from, to] of [
    [0, CHROMA],
    [CHROMA, FEATURE_SIZE],
  ]) {
    let squares = 0;
    for (let i = from; i < to; i++) squares += values[i] * values[i];
    const length = Math.sqrt(squares);
    for (let i = from; i < to; i++) values[i] = length > 1e-6 ? values[i] / length : 0;
  }
  return values;
}

/** 0 (nothing alike) to 1 (the same) for two normalised features. */
export function similarity(a: Float32Array, b: Float32Array) {
  let chroma = 0;
  let timbre = 0;
  for (let i = 0; i < CHROMA; i++) chroma += a[i] * b[i];
  for (let i = CHROMA; i < FEATURE_SIZE; i++) timbre += a[i] * b[i];
  return CHROMA_WEIGHT * Math.max(0, chroma) + (1 - CHROMA_WEIGHT) * ((timbre + 1) / 2);
}

export class Recurrence {
  readonly capacity: number;
  /** Seconds of music each step summarises. */
  step: number;
  /** One normalised feature per step, oldest first. */
  features: Float32Array[] = [];
  /** `capacity` × `capacity` similarities, row-major; only the first `features.length` of each are set. */
  readonly matrix: Float32Array;
  /** Goes up whenever every cell changed (a merge, a reset), so a renderer knows to redraw them all. */
  generation = 0;

  #sum = new Float32Array(FEATURE_SIZE);
  #one = new Float32Array(FEATURE_SIZE);
  #elapsed = 0;
  readonly #initialStep: number;

  constructor(capacity = 256, step = 0.5) {
    this.capacity = Math.max(2, capacity - (capacity % 2));
    this.step = this.#initialStep = step;
    this.matrix = new Float32Array(this.capacity * this.capacity);
  }

  /** Adds `dt` seconds of an analysis; returns true when that completed a step (a new row and column). */
  add(chroma: ArrayLike<number>, bands: ArrayLike<number>, dt: number): boolean {
    feature(chroma, bands, this.#one);
    for (let i = 0; i < FEATURE_SIZE; i++) this.#sum[i] += this.#one[i] * dt;
    this.#elapsed += dt;
    if (this.#elapsed < this.step) return false;

    const next = normalise(this.#sum.slice());
    this.#sum.fill(0);
    this.#elapsed = 0;
    const index = this.features.length;
    this.features.push(next);
    for (let j = 0; j <= index; j++) {
      const value = similarity(next, this.features[j]);
      this.matrix[index * this.capacity + j] = value;
      this.matrix[j * this.capacity + index] = value;
    }
    if (this.features.length === this.capacity) this.#merge();
    return true;
  }

  similarity(i: number, j: number) {
    return this.matrix[i * this.capacity + j];
  }

  reset() {
    this.features = [];
    this.step = this.#initialStep;
    this.#sum.fill(0);
    this.#elapsed = 0;
    this.generation++;
  }

  /** Halves the resolution: each pair of steps becomes one twice as long. */
  #merge() {
    const merged: Float32Array[] = [];
    for (let i = 0; i + 1 < this.features.length; i += 2) {
      const pair = new Float32Array(FEATURE_SIZE);
      for (let k = 0; k < FEATURE_SIZE; k++) pair[k] = this.features[i][k] + this.features[i + 1][k];
      merged.push(normalise(pair));
    }
    this.features = merged;
    this.step *= 2;
    for (let i = 0; i < merged.length; i++) {
      for (let j = 0; j <= i; j++) {
        const value = similarity(merged[i], merged[j]);
        this.matrix[i * this.capacity + j] = value;
        this.matrix[j * this.capacity + i] = value;
      }
    }
    this.generation++;
  }
}
