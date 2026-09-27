// The equaliser's presets and bands (PLAN.md F15), pure so plain
// `node --test` covers them. Gains are dB at the ten bands' centres, low
// first; a preset's preamp keeps its boosts from clipping.

export const BANDS = [31.25, 62.5, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];
export const MAX_GAIN = 12;

export const PRESETS: Record<string, number[]> = {
  flat: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
  bassBoost: [6, 5, 4, 2, 0, 0, 0, 0, 0, 0],
  bassReduce: [-6, -5, -4, -2, 0, 0, 0, 0, 0, 0],
  trebleBoost: [0, 0, 0, 0, 0, 1, 2, 4, 5, 6],
  trebleReduce: [0, 0, 0, 0, 0, -1, -2, -4, -5, -6],
  vocal: [-2, -2, -1, 0, 2, 4, 4, 2, 0, -1],
  loudness: [5, 4, 2, 0, -1, 0, -1, 1, 4, 5],
  rock: [4, 3, 2, 0, -1, -1, 1, 2, 3, 4],
  classical: [3, 2, 1, 0, 0, 0, -1, -1, 1, 2],
  headphones: [2, 1, 0, 0, -1, 0, 1, 2, 1, -1],
};

/** The preamp that keeps a set of gains from clipping: minus the largest boost. */
export const safePreamp = (gains: number[]) => -Math.max(0, ...gains);

/** "31", "250", "1k", "16k". */
export const bandLabel = (hz: number) => (hz >= 1000 ? `${hz / 1000}k` : String(Math.round(hz)));

/** The preset whose gains these are, or "custom". */
export function presetOf(gains: number[]) {
  for (const [id, preset] of Object.entries(PRESETS)) {
    if (preset.length === gains.length && preset.every((gain, i) => Math.abs(gain - gains[i]) < 1e-9)) return id;
  }
  return "custom";
}
