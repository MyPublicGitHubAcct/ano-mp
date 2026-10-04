// The musical reading behind X3's visualizations (music.ts, recurrence.ts),
// on synthetic input as the core's analysis tests use synthetic signals.
// Plain JavaScript (.mjs), as visualizer.test.mjs explains.

import assert from "node:assert/strict";
import { test } from "node:test";
import {
  chladni,
  chladniMode,
  embeddingDelay,
  INTERVAL_RATIOS,
  peaks,
  strongestInterval,
  strongestTriad,
  TempoTracker,
  tonnetzPitch,
  triadNotes,
} from "../src/lib/visualizer/music.ts";
import { Recurrence, similarity } from "../src/lib/visualizer/recurrence.ts";

/** @param {Record<number, number>} weights */
function chroma(weights) {
  return Array.from({ length: 12 }, (_, pc) => weights[pc] ?? 0.05);
}

test("names the triad the pitch classes make", () => {
  assert.deepEqual(strongestTriad(chroma({ 0: 1, 4: 0.9, 7: 0.8 })), { root: 0, minor: false, strength: 0.75 });
  const aMinor = strongestTriad(chroma({ 9: 1, 0: 0.8, 4: 0.9 }));
  assert.equal(aMinor?.root, 9);
  assert.equal(aMinor?.minor, true);
  // F♯ major: F♯, A♯, C♯.
  const fSharp = strongestTriad(chroma({ 6: 1, 10: 1, 1: 1 }));
  assert.deepEqual([fSharp?.root, fSharp?.minor], [6, false]);
  assert.deepEqual(triadNotes({ root: 9, minor: true }), [9, 0, 4]);
});

test("finds no triad in silence or a chromatic smear", () => {
  assert.equal(strongestTriad(new Array(12).fill(0)), null);
  assert.equal(strongestTriad(new Array(12).fill(1)), null);
  // A lone note isn't a chord.
  assert.equal(strongestTriad(chroma({ 2: 1 })), null);
});

test("names the interval between the two loudest notes the consonant way round", () => {
  // C and G: a fifth up from C, not a fourth up from G.
  assert.deepEqual(strongestInterval(chroma({ 0: 1, 7: 0.9 })), { low: 0, semitones: 7 });
  assert.deepEqual(strongestInterval(chroma({ 7: 1, 0: 0.9 })), { low: 0, semitones: 7 });
  // E over C: a major third.
  assert.deepEqual(strongestInterval(chroma({ 4: 1, 0: 0.7 })), { low: 0, semitones: 4 });
  assert.equal(strongestInterval(chroma({ 5: 1 })), null);
  assert.deepEqual(INTERVAL_RATIOS[7], [3, 2]);
  assert.equal(INTERVAL_RATIOS.length, 12);
});

test("lays the Tonnetz out in fifths and thirds", () => {
  assert.equal(tonnetzPitch(0, 0), 0); // C
  assert.equal(tonnetzPitch(1, 0), 7); // G, a fifth along
  assert.equal(tonnetzPitch(0, 1), 4); // E, a major third up
  assert.equal(tonnetzPitch(-1, 1), 9); // A, a minor third below C: up and back
  assert.equal(tonnetzPitch(-1, 0), 5); // F
  // An upward triangle is a major triad, a downward one minor.
  assert.deepEqual([tonnetzPitch(0, 0), tonnetzPitch(0, 1), tonnetzPitch(1, 0)], triadNotes({ root: 0, minor: false }));
  assert.deepEqual(
    [tonnetzPitch(0, 1), tonnetzPitch(1, 0), tonnetzPitch(1, 1)].sort(),
    triadNotes({ root: 4, minor: true }).sort(),
  );
});

test("gives each note of seven octaves its own plate mode, finer as it rises", () => {
  const modes = Array.from({ length: 84 }, (_, i) => chladniMode(i));
  assert.equal(new Set(modes.map(([m, n]) => `${m},${n}`)).size, 84);
  const frequency = ([m, n]) => m * m + n * n;
  for (let i = 1; i < modes.length; i++) assert.ok(frequency(modes[i]) >= frequency(modes[i - 1]));
  assert.ok(modes.every(([m, n]) => m < n));
  // Antisymmetric about the diagonal, so the diagonal is a nodal line.
  for (const [x, y] of [
    [0.1, 0.7],
    [0.33, 0.9],
  ]) {
    assert.ok(Math.abs(chladni(2, 5, x, y) + chladni(2, 5, y, x)) < 1e-12);
    assert.ok(Math.abs(chladni(3, 4, x, x)) < 1e-12);
  }
});

test("picks the strongest peaks", () => {
  assert.deepEqual(peaks([0, 0.5, 0.1, 0.9, 0.2, 0.3, 0.1], 2), [3, 1]);
  assert.deepEqual(peaks([0.1, 0.1, 0.15], 3), []);
});

test("unfolds a sine with a quarter-period delay", () => {
  for (const period of [16, 40, 100]) {
    const sine = Array.from({ length: 512 }, (_, i) => Math.sin((2 * Math.PI * i) / period));
    const delay = embeddingDelay(sine, 2, 64);
    assert.ok(Math.abs(delay - Math.min(64, period / 4)) <= 1, `period ${period}: ${delay}`);
  }
  assert.equal(embeddingDelay(new Array(512).fill(0), 3), 3);
});

/** A seeded random number generator (mulberry32), so the grooves are the same every run. */
function random(seed) {
  return () => {
    seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** The onset strength a groove gives, frame by frame at 60 a second, as the core's analysis would: each hit
    a spike on the frame it lands in, up to 15 ms early or late, one in ten missed, over noise.
    `hits` lists [position in beats within a 4-beat bar, strength]. Returns [time, onset] pairs. */
function groove(bpm, seconds, hits, seed = 1) {
  const next = random(seed);
  const frames = new Float32Array(Math.round(seconds * 60));
  const beat = 60 / bpm;
  for (let bar = 0; bar * 4 * beat < seconds; bar++) {
    for (const [position, strength] of hits) {
      if (next() < 0.1) continue;
      const time = (bar * 4 + position) * beat + (next() - 0.5) * 0.03;
      const frame = Math.round(time * 60);
      if (frame >= 0 && frame < frames.length) frames[frame] = Math.max(frames[frame], strength * (0.8 + 0.4 * next()));
    }
  }
  return Array.from(frames, (onset, frame) => [frame / 60, Math.min(1, onset + 0.08 * next())]);
}

/** Kick on 1, the "and" of 2 and 3, snare on 2 and 4, hats on the eighths. */
const ROCK = [
  [0, 0.9],
  [1.5, 0.7],
  [2, 0.9],
  [1, 0.6],
  [3, 0.6],
  ...[0, 0.5, 1, 1.5, 2, 2.5, 3, 3.5].map((position) => [position, 0.25]),
];

/** Feeds a groove to a tracker; returns it. */
function track(onsets) {
  const tracker = new TempoTracker();
  for (const [time, onset] of onsets) tracker.add(time, onset);
  return tracker;
}

test("finds the tempo of a syncopated groove, and its beats", () => {
  for (const bpm of [80, 100, 120, 140]) {
    const tracker = track(groove(bpm, 20, ROCK, bpm));
    const beat = 60 / bpm;
    assert.ok(tracker.period !== null && Math.abs(tracker.period - beat) < 0.02 * beat, `${bpm}: ${tracker.period}`);
    // On a beat, the grid is on a beat (a quarter of the bar each), within 40 ms.
    const position = tracker.position(19 * beat * Math.floor(20 / beat / 19));
    const quarters = (position?.phase ?? 0.5) * 4;
    const off = Math.abs(quarters - Math.round(quarters)) * beat;
    assert.ok(off < 0.04, `${bpm}: ${off} s off the beat`);
  }
});

test("finds a tempo with sixteenth-note hats and no kick on most beats", () => {
  const hits = [
    [0, 0.9],
    [2.75, 0.7],
    [1, 0.6],
    [3, 0.6],
    ...Array.from({ length: 16 }, (_, i) => [i / 4, i % 2 ? 0.15 : 0.3]),
  ];
  const tracker = track(groove(90, 20, hits, 7));
  assert.ok(tracker.period !== null && Math.abs(tracker.period - 60 / 90) < 0.02 * (60 / 90), String(tracker.period));
});

test("finds a slow pulse rather than doubling it", () => {
  const tracker = track(
    groove(
      60,
      20,
      [
        [0, 0.9],
        [1, 0.9],
        [2, 0.9],
        [3, 0.9],
      ],
      3,
    ),
  );
  assert.ok(tracker.period !== null && Math.abs(tracker.period - 1) < 0.02, String(tracker.period));
});

test("finds no tempo without a pulse, or before a few seconds", () => {
  // A held chord: a steady onset with a little flutter.
  const next = random(11);
  const steady = Array.from({ length: 20 * 60 }, (_, frame) => [frame / 60, 0.1 + 0.0005 * next()]);
  assert.equal(track(steady).period, null);
  const early = track(groove(120, 3, ROCK));
  assert.equal(early.period, null);
  assert.equal(early.position(3), null);
  const tracker = track(groove(120, 20, ROCK));
  tracker.reset();
  assert.equal(tracker.period, null);
});

/** Bands with their energy low (bass) or high (treble). @param {"low" | "high"} where */
function bands(where) {
  return Array.from({ length: 64 }, (_, i) => (where === "low" ? 1 - i / 64 : i / 64));
}

test("a recurrence plot shows a section that comes back", () => {
  const plot = new Recurrence(64, 0.5);
  const a = { chroma: chroma({ 0: 1, 4: 0.8, 7: 0.9 }), bands: bands("low") };
  const b = { chroma: chroma({ 6: 1, 10: 0.8, 1: 0.9 }), bands: bands("high") };
  let steps = 0;
  for (const section of [a, b, a]) {
    for (let i = 0; i < 4 * 60; i++) if (plot.add(section.chroma, section.bands, 1 / 60)) steps++;
  }
  assert.equal(plot.features.length, steps);
  assert.ok(steps >= 23 && steps <= 24, String(steps));
  // The first A against the second, against B.
  assert.ok(plot.similarity(2, 18) > 0.95, String(plot.similarity(2, 18)));
  assert.ok(plot.similarity(2, 10) < 0.4, String(plot.similarity(2, 10)));
  assert.equal(plot.similarity(10, 18), plot.similarity(18, 10));
  assert.ok(Math.abs(plot.similarity(5, 5) - 1) < 1e-6);
  assert.ok(similarity(plot.features[0], plot.features[0]) > 0.999);
});

test("a full recurrence plot merges its steps to keep the whole track", () => {
  const plot = new Recurrence(8, 1);
  const generation = plot.generation;
  for (let i = 0; i < 8; i++) plot.add(chroma({ [i % 12]: 1 }), bands(i % 2 ? "low" : "high"), 1);
  assert.equal(plot.step, 2);
  assert.equal(plot.features.length, 4);
  assert.ok(plot.generation > generation);
  // Steps now take two seconds.
  assert.equal(plot.add(chroma({ 0: 1 }), bands("low"), 1), false);
  assert.equal(plot.add(chroma({ 0: 1 }), bands("low"), 1), true);
  plot.reset();
  assert.deepEqual([plot.features.length, plot.step], [0, 1]);
});
