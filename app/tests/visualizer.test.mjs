// The visualizer's pure logic: decoding the backend's frames and estimating
// the key. Run with `npm test`; Node imports the TypeScript modules as they
// are. Plain JavaScript (.mjs), which svelte-check leaves alone: it would
// need Node's types, which aren't installed.

import assert from "node:assert/strict";
import { test } from "node:test";
import { decodeFrame, silentFrame } from "../src/lib/visualizer/frame.ts";
import { estimateKey, keyName } from "../src/lib/visualizer/key.ts";

/** What `visualizer::encode` makes of its test frame (`ENCODED_EXAMPLE` in app/src-tauri/src/visualizer.rs). */
const ENCODED_EXAMPLE =
  "01020500030000000000f04100007a466666663fcdcc4c3f0000003fcdcccc3e0000803e" +
  "1a1a1a1a1a1a1a1a1aff1a1a0080ffff00000000ff7f0180004000c0ff7f";

/** @param {string} hex */
function bytes(hex) {
  const array = new Uint8Array(hex.length / 2);
  for (let i = 0; i < array.length; i++) array[i] = parseInt(hex.slice(2 * i, 2 * i + 2), 16);
  return array.buffer;
}

/** @type {(actual: number, expected: number, tolerance?: number) => void} */
const close = (actual, expected, tolerance = 1e-3) =>
  assert.ok(Math.abs(actual - expected) <= tolerance, `${actual} is not ${expected}`);

test("decodes the backend's frames", () => {
  const frame = silentFrame();
  assert.equal(decodeFrame(bytes(ENCODED_EXAMPLE), frame), true);

  assert.equal(frame.silent, false);
  assert.equal(frame.beat, true);
  assert.equal(frame.lowestHz, 30);
  assert.equal(frame.highestHz, 16000);
  close(frame.peak[0], 0.9);
  close(frame.peak[1], 0.8);
  close(frame.rms[0], 0.5);
  close(frame.rms[1], 0.4);
  close(frame.onset, 0.25);
  close(frame.chroma[9], 1);
  close(frame.chroma[0], 0.1, 0.01);
  assert.deepEqual(Array.from(frame.bands, (v) => Math.round(v * 255)), [0, 128, 255, 255, 0]);
  assert.deepEqual(Array.from(frame.left, (v) => Math.round(v * 100) / 100), [0, 1, -1]);
  assert.deepEqual(Array.from(frame.right, (v) => Math.round(v * 100) / 100), [0.5, -0.5, 1]);
});

test("reuses the frame's arrays when the sizes match", () => {
  const frame = silentFrame(5, 3);
  const { bands, left } = frame;
  decodeFrame(bytes(ENCODED_EXAMPLE), frame);
  assert.equal(frame.bands, bands);
  assert.equal(frame.left, left);
});

test("leaves the frame alone for a buffer it can't read", () => {
  const frame = silentFrame();
  const truncated = bytes(ENCODED_EXAMPLE).slice(0, 60);
  assert.equal(decodeFrame(truncated, frame), false);
  const otherVersion = new Uint8Array(bytes(ENCODED_EXAMPLE));
  otherVersion[0] = 2;
  assert.equal(decodeFrame(otherVersion.buffer, frame), false);
  assert.equal(frame.silent, true);
  assert.equal(frame.bands.length, 64);
});

/** Pitch-class weights for the notes given (C = 0), as a triad-heavy passage would give. */
/** @param {Record<number, number>} weights */
function chroma(weights) {
  return Array.from({ length: 12 }, (_, pc) => weights[pc] ?? 0.05);
}

test("estimates major and minor keys", () => {
  // C major scale, tonic triad strongest.
  const cMajor = estimateKey(chroma({ 0: 1, 2: 0.4, 4: 0.8, 5: 0.4, 7: 0.9, 9: 0.4, 11: 0.3 }));
  assert.ok(cMajor);
  assert.equal(keyName(cMajor), "C major");

  // A minor: the same notes, centred on A.
  const aMinor = estimateKey(chroma({ 9: 1, 11: 0.35, 0: 0.8, 2: 0.4, 4: 0.9, 5: 0.35, 7: 0.3 }));
  assert.ok(aMinor);
  assert.equal(keyName(aMinor), "A minor");

  // F♯ major, far round the circle.
  const fSharp = estimateKey(chroma({ 6: 1, 8: 0.4, 10: 0.8, 11: 0.4, 1: 0.9, 3: 0.4, 5: 0.3 }));
  assert.ok(fSharp);
  assert.equal(keyName(fSharp), "F♯ major");
  assert.ok(fSharp.strength > 0.8);
});

test("has no key for silence", () => {
  assert.equal(estimateKey(new Array(12).fill(0)), null);
  assert.equal(estimateKey(new Array(12).fill(0.5)), null);
});
