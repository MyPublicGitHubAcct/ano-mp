// The effects' presets, slider scales and value display (src/lib/effects.ts).
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { PRESETS, STEPS, applyPreset, display, fromPosition, tidy, toPosition } from "../src/lib/effects.ts";

const en = JSON.parse(readFileSync(new URL("../src/lib/i18n/en.json", import.meta.url), "utf8"));

const param = (id, min, max, defaultValue, unit = "ratio", logarithmic = false) => ({
  id,
  unit,
  min,
  max,
  defaultValue,
  logarithmic,
});

// As the core describes them (the parts these tests use).
const RANGES = {
  reverb: [
    param("size", 0, 1, 0.6),
    param("damping", 0, 1, 0.4),
    param("width", 0, 1, 1),
    param("preDelay", 0, 250, 20, "milliseconds"),
  ],
  echo: [
    param("time", 20, 2000, 375, "milliseconds", true),
    param("feedback", 0, 0.95, 0.35),
    param("tone", 0, 1, 0.6),
    param("spread", 0, 1, 0.5),
  ],
};
const off = (ranges) => ({
  enabled: false,
  mix: 0.5,
  params: Object.fromEntries((ranges ?? []).map((p) => [p.id, p.defaultValue])),
});
const DEFAULTS = Object.fromEntries(
  ["reverb", "chorus", "freeze", "echo", "flanger", "phaser", "tremolo", "lofi"].map((id) => [id, off(RANGES[id])]),
);

test("every preset names known effects and parameters, with a name to show", () => {
  for (const [id, effects] of Object.entries(PRESETS)) {
    assert.ok(`effects.preset.${id}` in en, `en.json lacks effects.preset.${id}`);
    for (const [effect, settings] of Object.entries(effects)) {
      assert.ok(effect in DEFAULTS, `${id}: ${effect}`);
      assert.ok(settings.mix === undefined || (settings.mix >= 0 && settings.mix <= 1), `${id}: ${effect}'s mix`);
      for (const name of Object.keys(settings.params ?? {})) {
        assert.ok(`effects.param.${name}` in en, `${id}: ${effect}.${name}`);
      }
    }
  }
});

test("a preset switches its effects on, the rest off, within range", () => {
  const before = structuredClone(DEFAULTS);
  before.chorus.enabled = true;
  const dub = applyPreset("dub", before, RANGES);
  assert.equal(dub.echo.enabled, true);
  assert.equal(dub.echo.mix, 0.4);
  assert.equal(dub.echo.params.time, 375);
  assert.equal(dub.echo.params.feedback, 0.6);
  assert.equal(dub.reverb.enabled, true);
  assert.equal(dub.chorus.enabled, false);
  assert.equal(before.echo.enabled, false, "what it started from is left alone");

  // Out of range is clamped; unknown parameters and effects are left out.
  const clamped = applyPreset("dub", DEFAULTS, { echo: [param("time", 20, 300, 100, "milliseconds", true)] });
  assert.equal(clamped.echo.params.time, 300);
  assert.equal(clamped.echo.params.feedback, 0.35, "no range known: unchanged");
  assert.deepEqual(applyPreset("off", before, RANGES), DEFAULTS);
  assert.deepEqual(applyPreset("nonsense", DEFAULTS, RANGES), DEFAULTS);
});

test("sliders move logarithmically for rates and times, and land on tidy values", () => {
  const [time] = RANGES.echo;
  assert.equal(toPosition(20, time), 0);
  assert.equal(toPosition(2000, time), STEPS);
  assert.equal(toPosition(200, time), STEPS / 2);
  assert.equal(fromPosition(STEPS / 2, time), 200);
  assert.equal(fromPosition(0, time), 20);
  assert.equal(fromPosition(STEPS, time), 2000);
  for (let position = 0; position <= STEPS; position += 37) {
    const value = fromPosition(position, time);
    assert.ok(value >= 20 && value <= 2000 && Number.isInteger(value), `${position}: ${value}`);
    // A value the slider made puts the slider where it gives that value again.
    assert.equal(fromPosition(toPosition(value, time), time), value, `${position} round trip`);
  }

  const feedback = param("feedback", -0.95, 0.95, 0.5);
  assert.equal(toPosition(0, feedback), STEPS / 2);
  assert.equal(fromPosition(STEPS / 2, feedback), 0);
  assert.equal(fromPosition(-50, feedback), -0.95);
  const rate = param("rate", 0.05, 5, 0.8, "hertz", true);
  assert.equal(fromPosition(0, rate), 0.05);
  assert.equal(tidy(0.123456, "hertz"), 0.123);
  assert.equal(tidy(8.4, "bits"), 8);
});

test("values show in their units", () => {
  assert.deepEqual(display(0.35, "ratio"), { key: "percent", value: 35 });
  assert.deepEqual(display(0.8, "hertz"), { key: "hertz", value: 0.8 });
  assert.deepEqual(display(11025, "hertz"), { key: "kilohertz", value: 11 });
  assert.deepEqual(display(375.4, "milliseconds"), { key: "milliseconds", value: 375 });
  assert.deepEqual(display(0.3, "seconds"), { key: "seconds", value: 0.3 });
  assert.deepEqual(display(7.6, "bits"), { key: "bits", value: 8 });
  for (const key of ["percent", "hertz", "kilohertz", "milliseconds", "seconds", "bits"]) {
    assert.ok(`effects.unit.${key}` in en, `en.json lacks effects.unit.${key}`);
  }
});
