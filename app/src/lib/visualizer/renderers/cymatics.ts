// Cymatics: sand on a square plate that the music sets ringing. Each of the
// loudest notes rings one of the plate's modes (`chladniMode`, finer
// figures for higher notes) and the sand gathers where the plate is still,
// the nodal lines of the modes' sum, so a chord draws its own Chladni
// figure and a melody morphs it from note to note.

import { t } from "$lib/i18n";
import { chladniMode, peaks } from "../music";
import type { Renderer, Scene, Visualization } from "../types";
import { approach, clearStage, mix, rgba } from "../util";

/** The field's resolution; drawn smoothly scaled up. */
const GRID = 180;
const MAX_MODE = 20;
const VOICES = 3;

function create(): Renderer {
  const field = document.createElement("canvas");
  field.width = field.height = GRID;
  const fieldCtx = field.getContext("2d");
  const pixels = fieldCtx?.createImageData(GRID, GRID) ?? null;

  // cos(kπx) for each mode number k at each grid position.
  const cosines = Array.from({ length: MAX_MODE + 1 }, (_, k) =>
    Float32Array.from({ length: GRID }, (_, i) => Math.cos((k * Math.PI * i) / (GRID - 1))),
  );
  // A fixed grain, so the figure looks like sand rather than ink.
  const grain = Float32Array.from({ length: GRID * GRID }, () => 0.55 + 0.45 * Math.random());
  const values = new Float32Array(GRID * GRID);

  /** Ringing modes by "m,n", with their weights. */
  const modes = new Map<string, { m: number; n: number; weight: number }>();
  let loudness = 0;
  let pulse = 0;

  function paint(scene: Scene) {
    if (!pixels || !fieldCtx) return;
    let weights = 0;
    for (const mode of modes.values()) weights += mode.weight;
    values.fill(0);
    for (const { m, n, weight } of modes.values()) {
      if (weight < 0.01) continue;
      const share = weight / Math.max(weights, 1e-6);
      const cm = cosines[m];
      const cn = cosines[n];
      for (let y = 0; y < GRID; y++) {
        const row = y * GRID;
        const cny = cn[y] * share;
        const cmy = cm[y] * share;
        for (let x = 0; x < GRID; x++) values[row + x] += cn[x] * cmy - cm[x] * cny;
      }
    }
    // Sand where the plate is still; louder music shakes it into thinner lines.
    const width = 0.05 + 0.1 * (1 - loudness);
    const sand = mix(scene.palette.colors[0], [255, 250, 235], 0.45);
    const plate = mix([16, 16, 22], scene.palette.shade, 0.5);
    const data = pixels.data;
    const still = weights > 0.01;
    for (let i = 0; i < values.length; i++) {
      const z = values[i] / width;
      const density = still ? Math.exp(-z * z) * grain[i] : 0;
      data[i * 4] = plate[0] + (sand[0] - plate[0]) * density;
      data[i * 4 + 1] = plate[1] + (sand[1] - plate[1]) * density;
      data[i * 4 + 2] = plate[2] + (sand[2] - plate[2]) * density;
      data[i * 4 + 3] = 255;
    }
    fieldCtx.putImageData(pixels, 0, 0);
  }

  return {
    draw(scene: Scene) {
      const { ctx, width, height, dt, frame, calm } = scene;
      loudness = approach(loudness, Math.max(frame.rms[0], frame.rms[1]) * 2.5, dt, 0.1, 0.6);
      const ringing = frame.silent ? [] : peaks(frame.notes, VOICES, 0.35);
      const wanted = new Map<string, number>();
      for (const index of ringing) {
        const [m, n] = chladniMode(index);
        if (n > MAX_MODE) continue;
        const key = `${m},${n}`;
        wanted.set(key, Math.max(wanted.get(key) ?? 0, frame.notes[index]));
        if (!modes.has(key)) modes.set(key, { m, n, weight: 0 });
      }
      for (const [key, mode] of modes) {
        // Sand takes a moment to move: rising faster than it falls, slower still when calm.
        mode.weight = approach(mode.weight, wanted.get(key) ?? 0, dt, calm ? 0.6 : 0.2, calm ? 2 : 0.8);
        if (mode.weight < 0.005 && !wanted.has(key)) modes.delete(key);
      }
      pulse = approach(pulse, scene.beat ? 1 : 0, dt, 0, 0.25);
      paint(scene);

      clearStage(scene);
      const size = Math.min(width, height) * 0.78 * (1 + 0.012 * pulse);
      const x = (width - size) / 2;
      const y = (height - size) / 2;
      ctx.save();
      ctx.shadowColor = "rgb(0 0 0 / 0.6)";
      ctx.shadowBlur = 24;
      ctx.beginPath();
      ctx.roundRect(x, y, size, size, Math.max(4, size * 0.015));
      ctx.fillStyle = "rgb(0 0 0)";
      ctx.fill();
      ctx.shadowBlur = 0;
      ctx.clip();
      ctx.imageSmoothingEnabled = true;
      ctx.imageSmoothingQuality = "high";
      ctx.drawImage(field, x, y, size, size);
      ctx.restore();
      ctx.save();
      ctx.strokeStyle = rgba([255, 255, 255], 0.12);
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.roundRect(x, y, size, size, Math.max(4, size * 0.015));
      ctx.stroke();
      ctx.restore();
    },
  };
}

export const cymatics: Visualization = {
  id: "cymatics",
  get name() {
    return t("viz.cymatics.name");
  },
  get description() {
    return t("viz.cymatics.description");
  },
  create,
};
