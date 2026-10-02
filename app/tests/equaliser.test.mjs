// The equaliser's presets (src/lib/equaliser.ts).
import assert from "node:assert/strict";
import { test } from "node:test";
import { BANDS, MAX_GAIN, PRESETS, bandLabel, presetOf, safePreamp } from "../src/lib/equaliser.ts";

test("every preset has ten bands within range, and is found again", () => {
  for (const [id, gains] of Object.entries(PRESETS)) {
    assert.equal(gains.length, BANDS.length, id);
    assert.ok(
      gains.every((gain) => Math.abs(gain) <= MAX_GAIN),
      id,
    );
    assert.equal(presetOf(gains), id);
    assert.ok(safePreamp(gains) <= 0 && safePreamp(gains) >= -MAX_GAIN, id);
  }
  assert.equal(presetOf([1, 0, 0, 0, 0, 0, 0, 0, 0, 0]), "custom");
  assert.equal(safePreamp(PRESETS.bassBoost), -6);
  assert.equal(safePreamp(PRESETS.bassReduce), -0);
  assert.deepEqual(BANDS.map(bandLabel).slice(0, 2), ["31", "63"]);
  assert.equal(bandLabel(16000), "16k");
});
