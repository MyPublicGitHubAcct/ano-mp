// The visualizer's flash guard (src/lib/visualizer/safety.ts).
import assert from "node:assert/strict";
import { test } from "node:test";
import { FlashGuard, MAX_FLASHES, averageLuminance } from "../src/lib/visualizer/safety.ts";

test("at most three beats a second, none when calm", () => {
  const guard = new FlashGuard();
  const shown = [0, 0.25, 0.5, 0.75, 0.9, 1.05, 1.3].map((at) => guard.beat(at, false));
  assert.deepEqual(shown, [true, true, true, false, false, true, true]);
  assert.equal(new FlashGuard().beat(0, true), false);
});

test("brightness flickering faster than three times a second is dimmed", () => {
  const guard = new FlashGuard();
  let dim = 0;
  // Black and white alternating 10 times a second.
  for (let i = 0; i < 20; i++) dim = guard.luminance(i / 10, i % 2, 0.1);
  assert.equal(dim, 0.7);
  // Quiet again: it eases back.
  for (let i = 20; i < 80; i++) dim = guard.luminance(i / 10, 0.5, 0.1);
  assert.equal(dim, 0);

  // Twice a second is allowed.
  const slow = new FlashGuard();
  for (let i = 0; i < 40; i++) dim = slow.luminance(i / 4, i % 2, 0.25);
  assert.equal(dim, 0);
  assert.equal(MAX_FLASHES, 3);
});

test("measures relative luminance", () => {
  assert.equal(averageLuminance(new Uint8ClampedArray([0, 0, 0, 255])), 0);
  assert.ok(Math.abs(averageLuminance(new Uint8ClampedArray([255, 255, 255, 255])) - 1) < 1e-9);
  const grey = averageLuminance(new Uint8ClampedArray([128, 128, 128, 255, 0, 0, 0, 255]));
  assert.ok(grey > 0.1 && grey < 0.12);
});
