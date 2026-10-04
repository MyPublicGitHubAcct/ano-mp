// The musical reading behind X3's visualizations (PLAN.md Phase 7b): which
// triad the pitch classes make, the interval between the two strongest,
// where a note lies on the Tonnetz, which plate mode a note rings, the delay
// that unfolds a waveform, and the tempo from the beats. Pure and without
// imports, so plain `node --test` covers it (tests/visualizations.test.mjs).

export type Triad = {
  /** Pitch class of the root, C = 0. */
  root: number;
  minor: boolean;
  /** How strongly the three notes sound together, 0..1. */
  strength: number;
};

/** The pitch classes of a triad, root first. */
export function triadNotes(triad: { root: number; minor: boolean }): [number, number, number] {
  return [triad.root, (triad.root + (triad.minor ? 3 : 4)) % 12, (triad.root + 7) % 12];
}

/** The major or minor triad whose weakest note stands out most above the other nine on average: null when
    none does (silence, a lone note, a chromatic smear). `chroma` is 12 values, C first. */
export function strongestTriad(chroma: ArrayLike<number>): Triad | null {
  let best: Triad | null = null;
  for (let root = 0; root < 12; root++) {
    for (const minor of [false, true]) {
      const notes = triadNotes({ root, minor });
      const weakest = Math.min(chroma[notes[0]], chroma[notes[1]], chroma[notes[2]]);
      let outside = 0;
      for (let pc = 0; pc < 12; pc++) if (!notes.includes(pc)) outside += chroma[pc] / 9;
      const strength = weakest - outside;
      if (best === null || strength > best.strength) best = { root, minor, strength };
    }
  }
  return best !== null && best.strength > 0.15 ? { ...best, strength: Math.min(1, best.strength) } : null;
}

/** Just-intonation ratios for each interval in semitones within an octave, smallest terms first. */
export const INTERVAL_RATIOS: [number, number][] = [
  [1, 1],
  [16, 15],
  [9, 8],
  [6, 5],
  [5, 4],
  [4, 3],
  [7, 5],
  [3, 2],
  [8, 5],
  [5, 3],
  [9, 5],
  [15, 8],
];

/** The two loudest pitch classes and the interval up from the lower-numbered one's root (0..11 semitones);
    null if fewer than two sound. */
export function strongestInterval(chroma: ArrayLike<number>): { low: number; semitones: number } | null {
  let first = -1;
  let second = -1;
  for (let pc = 0; pc < 12; pc++) {
    if (first < 0 || chroma[pc] > chroma[first]) {
      second = first;
      first = pc;
    } else if (second < 0 || chroma[pc] > chroma[second]) {
      second = pc;
    }
  }
  if (first < 0 || second < 0 || chroma[second] < 0.2) return null;
  // Name the interval as the consonant way round: a fifth rather than a fourth below.
  const up = (second - first + 12) % 12;
  const down = (first - second + 12) % 12;
  return consonance(up) >= consonance(down) ? { low: first, semitones: up } : { low: second, semitones: down };
}

/** Smaller is simpler: the sum of the ratio's terms. */
function consonance(semitones: number) {
  const [a, b] = INTERVAL_RATIOS[semitones];
  return -(a + b);
}

/** The pitch class at a node of the Tonnetz: fifths along a row, major thirds up each row (minor thirds
    up and back). */
export function tonnetzPitch(column: number, row: number) {
  return (((7 * column + 4 * row) % 12) + 12) % 12;
}

/** The mode (m, n) of a square Chladni plate that note `index` (a semitone each) rings: the modes in
    order of the plate's frequency (m² + n²), m < n, so higher notes draw finer figures. */
export function chladniMode(index: number): [number, number] {
  const modes = CHLADNI_MODES;
  return modes[((index % modes.length) + modes.length) % modes.length];
}

const CHLADNI_MODES: [number, number][] = (() => {
  const modes: [number, number][] = [];
  for (let m = 1; m <= 20; m++) for (let n = m + 1; n <= 20; n++) modes.push([m, n]);
  modes.sort((a, b) => a[0] ** 2 + a[1] ** 2 - (b[0] ** 2 + b[1] ** 2) || a[0] - b[0]);
  return modes.slice(0, 84);
})();

/** The plate's displacement for mode (m, n) at x, y in 0..1: zero along the nodal lines, where sand
    gathers. Antisymmetric about the diagonal, as a square plate's modes are. */
export function chladni(m: number, n: number, x: number, y: number) {
  return Math.cos(n * Math.PI * x) * Math.cos(m * Math.PI * y) - Math.cos(m * Math.PI * x) * Math.cos(n * Math.PI * y);
}

/** The strongest local peaks of `values`, loudest first, at most `count`, ignoring those under `floor`. */
export function peaks(values: ArrayLike<number>, count: number, floor = 0.2): number[] {
  const found: number[] = [];
  for (let i = 0; i < values.length; i++) {
    const v = values[i];
    if (v < floor) continue;
    if (i > 0 && values[i - 1] > v) continue;
    if (i + 1 < values.length && values[i + 1] >= v) continue;
    found.push(i);
  }
  return found.sort((a, b) => values[b] - values[a]).slice(0, count);
}

/** The delay (in samples, `min`..`max`) that unfolds a waveform best for a delay embedding: the first zero
    of its autocorrelation, or its first minimum if it never crosses. A sine's is a quarter of its period. */
export function embeddingDelay(samples: ArrayLike<number>, min = 2, max = 64): number {
  const n = samples.length;
  const at = (lag: number) => {
    let sum = 0;
    for (let i = 0; i + lag < n; i++) sum += samples[i] * samples[i + lag];
    return sum / Math.max(1, n - lag);
  };
  const zero = at(0);
  if (zero <= 1e-9) return min;
  let previous = zero;
  let lowest = min;
  let lowestValue = Infinity;
  for (let lag = 1; lag <= Math.min(max, n - 1); lag++) {
    const value = at(lag);
    if (lag >= min && value <= 0) {
      // Between the lags, whichever is nearer zero.
      return Math.abs(previous) < Math.abs(value) && lag - 1 >= min ? lag - 1 : lag;
    }
    if (lag >= min && value < lowestValue) {
      lowestValue = value;
      lowest = lag;
    }
    previous = value;
  }
  return lowest;
}

/** A triangle ±2 samples (±40 ms) wide, from its middle out. */
const SPREAD = [3 / 9, 2 / 9, 1 / 9];

/** Follows the tempo from the onset strength each analysis carries (how much louder the spectrum just
    got): the period is the lag, between 50 and 200 bpm, at which the last few seconds of onsets best match
    themselves (an autocorrelation of the onsets spread a little in time), leaning gently towards 120 bpm,
    which settles whether a groove is heard at its beat or twice or half as fast as listeners mostly do; and
    the beats fall where the onsets best line up on that period. Kicks, snares and hats all count, so
    irregular or missed beats don't throw it. Very slow and very fast music may read an octave off (60 bpm
    as 120, 180 as 90). */
export class TempoTracker {
  /** Onset samples a second, and seconds of them kept. */
  static readonly RATE = 50;
  static readonly WINDOW = 8;
  /** Seconds of onsets needed before a first estimate, and between estimates. */
  static readonly WARM_UP = 4;
  static readonly EVERY = 0.5;
  static readonly SHORTEST = 0.3; // 200 bpm.
  static readonly LONGEST = 1.2; // 50 bpm.
  /** How strongly the best lag must repeat (its correlation) to count as a tempo. Low, so that messy
      real grooves still find one; music without a pulse may then show a tempo all the same. */
  static readonly PERIODIC = 0.15;

  /** Seconds a beat, or null until there is a tempo. */
  period: number | null = null;

  readonly #size = TempoTracker.RATE * TempoTracker.WINDOW;
  /** A ring of onsets, one every 1/RATE s; `#written` counts every sample ever written. */
  #samples = new Float32Array(TempoTracker.RATE * TempoTracker.WINDOW);
  #written = 0;
  #start: number | null = null;
  #lastEstimate = -Infinity;
  /** A time on the beat grid. */
  #origin: number | null = null;

  /** Records the onset strength at `time` (seconds, rising). */
  add(time: number, onset: number) {
    if (this.#start === null) this.#start = time;
    const index = Math.floor((time - this.#start) * TempoTracker.RATE);
    // Every sample up to now takes this onset, at most a second's worth (after a pause).
    const from = Math.max(this.#written, index - TempoTracker.RATE);
    for (let i = from; i <= index; i++) this.#samples[i % this.#size] = onset;
    if (index + 1 > this.#written) this.#written = index + 1;
    if (this.#written >= TempoTracker.WARM_UP * TempoTracker.RATE && time - this.#lastEstimate >= TempoTracker.EVERY) {
      this.#lastEstimate = time;
      this.#estimate(time);
    }
  }

  #estimate(time: number) {
    const rate = TempoTracker.RATE;
    const n = Math.min(this.#written, this.#size);
    const spikes = new Float32Array(n);
    for (let i = 0; i < n; i++) spikes[i] = this.#samples[(this.#written - n + i) % this.#size];
    // Onsets are spikes a frame wide, landing a little early or late: spread over ±40 ms, so two a beat
    // apart still overlap at the beat's lag.
    const x = new Float32Array(n);
    let mean = 0;
    for (let i = 0; i < n; i++) {
      let sum = 0;
      for (let d = -SPREAD.length + 1; d < SPREAD.length; d++) {
        const j = i + d;
        if (j >= 0 && j < n) sum += spikes[j] * SPREAD[Math.abs(d)];
      }
      x[i] = sum;
      mean += sum / n;
    }
    let variance = 0;
    for (let i = 0; i < n; i++) {
      x[i] -= mean;
      variance += (x[i] * x[i]) / n;
    }
    if (variance < 1e-6) return;
    const correlation = (lag: number) => {
      if (lag >= n) return 0;
      let sum = 0;
      for (let i = 0; i + lag < n; i++) sum += x[i] * x[i + lag];
      return sum / (n - lag) / variance;
    };
    const shortest = Math.round(TempoTracker.SHORTEST * rate);
    const longest = Math.round(TempoTracker.LONGEST * rate);
    const raw = new Float32Array(longest + 2);
    for (let lag = 1; lag < raw.length; lag++) raw[lag] = correlation(lag);
    let best = -1;
    let bestScore = -Infinity;
    for (let lag = shortest; lag <= longest; lag++) {
      const score = raw[lag] * Math.exp(-0.5 * Math.log2(lag / rate / 0.5) ** 2);
      if (score > bestScore) {
        bestScore = score;
        best = lag;
      }
    }
    if (best < 0 || raw[best] < TempoTracker.PERIODIC) return;
    // Between samples, from the peak's neighbours.
    const [a, b, c] = [raw[best - 1], raw[best], raw[best + 1]];
    const bend = a - 2 * b + c;
    const offset = bend < 0 ? Math.max(-0.5, Math.min(0.5, (0.5 * (a - c)) / bend)) : 0;
    const period = (best + offset) / rate;
    // A small change glides; a new tempo takes over at once.
    this.period =
      this.period !== null && Math.abs(period - this.period) < 0.04 * this.period
        ? this.period + 0.3 * (period - this.period)
        : period;

    // The beats: the offset back from now at which onsets every period add up most.
    const step = this.period * rate;
    let phase = 0;
    let strongest = -Infinity;
    for (let back = 0; back < Math.ceil(step); back++) {
      let sum = 0;
      for (let k = 0; back + k * step < n; k++) sum += x[n - 1 - Math.round(back + k * step)];
      if (sum > strongest) {
        strongest = sum;
        phase = back;
      }
    }
    const latest = time - phase / rate;
    if (this.#origin === null) {
      this.#origin = latest;
    } else {
      // Pull the grid towards the beats found, by at most half the distance.
      const beats = (latest - this.#origin) / this.period;
      this.#origin += 0.5 * (beats - Math.round(beats)) * this.period;
    }
  }

  /** Where `time` falls on the grid of bars of `beatsPerBar` beats: the bar's number and 0..1 through it,
      or null without a tempo. */
  position(time: number, beatsPerBar = 4): { bar: number; phase: number } | null {
    if (this.period === null || this.#origin === null) return null;
    const bars = (time - this.#origin) / (this.period * beatsPerBar);
    return { bar: Math.floor(bars), phase: bars - Math.floor(bars) };
  }

  reset() {
    this.#samples.fill(0);
    this.#written = 0;
    this.#start = null;
    this.#lastEstimate = -Infinity;
    this.period = null;
    this.#origin = null;
  }
}
