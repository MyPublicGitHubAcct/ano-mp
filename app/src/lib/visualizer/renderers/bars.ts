// Spectrum bars with falling peak caps and a reflection, coloured across
// the palette from bass to treble.

import { t } from "$lib/i18n";
import type { Renderer, Scene, Visualization } from "../types";
import { along, approach, approachAll, clearStage, rgba } from "../util";

const PEAK_HOLD = 0.45;
const PEAK_FALL = 1.6; // Of the bar's height per second², as gravity.

function create(): Renderer {
  let levels: Float32Array = new Float32Array(0);
  let peaks = new Float32Array(0);
  let held = new Float32Array(0);
  let speeds = new Float32Array(0);
  let flash = 0;

  return {
    draw(scene: Scene) {
      const { ctx, width, height, dt, frame, palette } = scene;
      levels = approachAll(levels, frame.bands, dt, 0.025, 0.22);
      if (peaks.length !== levels.length) {
        peaks = new Float32Array(levels.length);
        held = new Float32Array(levels.length);
        speeds = new Float32Array(levels.length);
      }
      flash = approach(flash, scene.beat ? 1 : 0, dt, 0, 0.25);

      clearStage(scene);
      const count = levels.length;
      const margin = Math.max(16, width * 0.05);
      const slot = (width - 2 * margin) / count;
      const barWidth = Math.max(1, slot * 0.72);
      const baseline = height * 0.76;
      const tallest = baseline - Math.max(24, height * 0.08);

      for (let i = 0; i < count; i++) {
        const level = levels[i];
        // Peaks hold, then fall with gravity.
        if (level >= peaks[i]) {
          peaks[i] = level;
          held[i] = PEAK_HOLD;
          speeds[i] = 0;
        } else if ((held[i] -= dt) <= 0) {
          speeds[i] += PEAK_FALL * dt;
          peaks[i] = Math.max(level, peaks[i] - speeds[i] * dt);
        }

        const x = margin + i * slot + (slot - barWidth) / 2;
        const barHeight = Math.max(2, level * tallest);
        const color = along(palette, i / (count - 1));
        const gradient = ctx.createLinearGradient(0, baseline - barHeight, 0, baseline);
        gradient.addColorStop(0, rgba(color, 0.95));
        gradient.addColorStop(1, rgba(color, 0.35 + 0.25 * flash));
        ctx.fillStyle = gradient;
        ctx.beginPath();
        ctx.roundRect(x, baseline - barHeight, barWidth, barHeight, [
          Math.min(barWidth / 2, 3),
          Math.min(barWidth / 2, 3),
          0,
          0,
        ]);
        ctx.fill();

        // The reflection, fading away below the baseline.
        const reflection = barHeight * 0.3;
        const fade = ctx.createLinearGradient(0, baseline + 2, 0, baseline + 2 + reflection);
        fade.addColorStop(0, rgba(color, 0.22));
        fade.addColorStop(1, rgba(color, 0));
        ctx.fillStyle = fade;
        ctx.fillRect(x, baseline + 2, barWidth, reflection);

        ctx.fillStyle = rgba([255, 255, 255], 0.85);
        ctx.fillRect(x, baseline - Math.max(2, peaks[i] * tallest) - 5, barWidth, 2);
      }
    },
  };
}

export const bars: Visualization = {
  id: "bars",
  get name() {
    return t("viz.bars.name");
  },
  get description() {
    return t("viz.bars.description");
  },
  create,
};
