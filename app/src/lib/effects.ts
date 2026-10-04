// The effects' presets, slider scales and value display (PLAN.md X2), pure
// so plain `node --test` covers them. What the effects are and their
// parameters' ranges come from the core (`effects_catalog`); the presets
// name parameters by id and are clamped to those ranges as they apply.

import type { EffectParam, EffectsSettings, EffectUnit } from "./api";

export type EffectId = keyof EffectsSettings;

/** What a preset sets for one effect: it is switched on with these. */
export type PresetEffect = { mix?: number; params?: Record<string, number> };

/** Every effect not named is switched off. */
export const PRESETS: Record<string, Partial<Record<EffectId, PresetEffect>>> = {
  off: {},
  smallRoom: { reverb: { mix: 0.2, params: { size: 0.35, damping: 0.5, width: 0.8, preDelay: 5 } } },
  hall: { reverb: { mix: 0.3, params: { size: 0.85, damping: 0.35, width: 1, preDelay: 35 } } },
  dreamy: {
    chorus: { mix: 0.45, params: { rate: 0.4, depth: 0.6 } },
    reverb: { mix: 0.35, params: { size: 0.9, damping: 0.6, width: 1, preDelay: 40 } },
  },
  slapback: { echo: { mix: 0.3, params: { time: 110, feedback: 0.1, tone: 0.7, spread: 0 } } },
  dub: {
    echo: { mix: 0.4, params: { time: 375, feedback: 0.6, tone: 0.3, spread: 1 } },
    reverb: { mix: 0.15, params: { size: 0.7, damping: 0.5, width: 1, preDelay: 20 } },
  },
  jet: { flanger: { mix: 0.5, params: { rate: 0.15, depth: 0.9, feedback: 0.75 } } },
  psychedelic: {
    phaser: { mix: 0.5, params: { rate: 0.3, depth: 0.9, feedback: 0.6 } },
    echo: { mix: 0.25, params: { time: 500, feedback: 0.45, tone: 0.5, spread: 1 } },
  },
  oldRadio: { lofi: { mix: 1, params: { bits: 6, sampleRate: 8000 } } },
  autoPan: { tremolo: { mix: 1, params: { rate: 0.5, depth: 0.8, stereo: 1 } } },
  frozen: {
    freeze: { mix: 0.6, params: { fade: 1 } },
    reverb: { mix: 0.3, params: { size: 0.9, damping: 0.5, width: 1, preDelay: 0 } },
  },
};

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

/** `base` with preset `id` applied: its effects on, with its mix and
    parameters, kept within `ranges` (each effect's parameters by id), and
    every other effect off. Unknown effects and parameters are left out.
    Pass the defaults as `base` for a preset's sound alone. */
export function applyPreset(
  id: string,
  base: EffectsSettings,
  ranges: Partial<Record<string, EffectParam[]>>,
): EffectsSettings {
  const next = structuredClone(base);
  for (const effect of Object.values(next)) effect.enabled = false;
  for (const [effectId, preset] of Object.entries(PRESETS[id] ?? {})) {
    if (!(effectId in next) || !preset) continue;
    const effect = next[effectId as EffectId];
    effect.enabled = true;
    if (preset.mix !== undefined) effect.mix = clamp(preset.mix, 0, 1);
    for (const [paramId, value] of Object.entries(preset.params ?? {})) {
      const param = ranges[effectId]?.find((candidate) => candidate.id === paramId);
      if (param && paramId in effect.params) effect.params[paramId] = clamp(value, param.min, param.max);
    }
  }
  return next;
}

/** A slider's positions, 0 to STEPS. */
export const STEPS = 1000;

type Range = Pick<EffectParam, "min" | "max" | "logarithmic">;

/** The slider position for `value`. */
export function toPosition(value: number, param: Range): number {
  const { min, max } = param;
  const fraction = param.logarithmic ? Math.log(value / min) / Math.log(max / min) : (value - min) / (max - min);
  return Math.round(clamp(fraction, 0, 1) * STEPS);
}

/** The value at slider position `position`, rounded as its unit reads, within the range. */
export function fromPosition(position: number, param: Range & Pick<EffectParam, "unit">): number {
  const { min, max } = param;
  const fraction = clamp(position / STEPS, 0, 1);
  const value = param.logarithmic ? min * Math.pow(max / min, fraction) : min + (max - min) * fraction;
  return clamp(tidy(value, param.unit), min, max);
}

/** `value` to the precision its unit is shown at. */
export function tidy(value: number, unit: EffectUnit): number {
  const round = (places: number) => Math.round(value * 10 ** places) / 10 ** places;
  switch (unit) {
    case "ratio":
      return round(2);
    case "bits":
      return Math.round(value);
    case "milliseconds":
      return value >= 10 ? Math.round(value) : round(1);
    case "seconds":
      return round(2);
    case "hertz":
      // Three significant figures: 0.05, 0.8, 5.25, 440, 11000.
      return value > 0 ? Number(value.toPrecision(3)) : 0;
  }
}

/** How to show a value: the message (`effects.unit.<key>`) and the number to fill in. */
export function display(
  value: number,
  unit: EffectUnit,
): { key: "percent" | "hertz" | "kilohertz" | "milliseconds" | "seconds" | "bits"; value: number } {
  switch (unit) {
    case "ratio":
      return { key: "percent", value: Math.round(value * 100) };
    case "hertz":
      return value >= 1000
        ? { key: "kilohertz", value: Math.round(value / 100) / 10 }
        : { key: "hertz", value: tidy(value, "hertz") };
    case "milliseconds":
      return { key: "milliseconds", value: tidy(value, "milliseconds") };
    case "seconds":
      return { key: "seconds", value: tidy(value, "seconds") };
    case "bits":
      return { key: "bits", value: Math.round(value) };
  }
}
