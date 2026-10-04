// The stereo stage: where each sound sits between the speakers. Each band
// of the spectrum is a light placed by its balance (the core's per-band
// left/right measure), bass at the bottom and treble at the top, so a mix
// spreads out like a stage seen from above: a centred bass and voice, the
// guitars to either side, the cymbals' shimmer wide. Each band's light is a
// sprite painted once per palette, which keeps 64 glows cheap at Retina
// size.

import { t } from "$lib/i18n";
import type { Palette, Renderer, Scene, Visualization } from "../types";
import { along, approachAll, clearStage, rgba } from "../util";

/** A sprite's size in pixels; drawn scaled. */
const SPRITE = 64;

/** Frequencies marked on the stage's side. */
const MARKS: [number, string][] = [
  [100, "100"],
  [1000, "1k"],
  [10000, "10k"],
];

/** A soft glow in each band's colour, from the palette. */
function sprites(palette: Palette, count: number) {
  return Array.from({ length: count }, (_, band) => {
    const sprite = document.createElement("canvas");
    sprite.width = sprite.height = SPRITE;
    const ctx = sprite.getContext("2d");
    if (ctx) {
      const color = along(palette, band / Math.max(1, count - 1));
      const glow = ctx.createRadialGradient(SPRITE / 2, SPRITE / 2, 0, SPRITE / 2, SPRITE / 2, SPRITE / 2);
      glow.addColorStop(0, rgba([255, 255, 255], 0.95));
      glow.addColorStop(0.2, rgba(color, 0.9));
      glow.addColorStop(1, rgba(color, 0));
      ctx.fillStyle = glow;
      ctx.fillRect(0, 0, SPRITE, SPRITE);
    }
    return sprite;
  });
}

function create(): Renderer {
  let levels: Float32Array = new Float32Array(0);
  let balance: Float32Array = new Float32Array(0);
  let palette: Palette | null = null;
  let lights: HTMLCanvasElement[] = [];

  return {
    draw(scene: Scene) {
      const { ctx, width, height, dt, frame, calm } = scene;
      levels = approachAll(levels, frame.bands, dt, 0.03, 0.2);
      balance = approachAll(balance, frame.balance, dt, calm ? 0.3 : 0.08, calm ? 0.3 : 0.08);
      clearStage(scene);

      const count = levels.length;
      if (count === 0) return;
      if (scene.palette !== palette || lights.length !== count) {
        palette = scene.palette;
        lights = sprites(palette, count);
      }
      const halfWidth = Math.min(width * 0.42, height * 0.75);
      const cx = width / 2;
      const top = height * 0.1;
      const bottom = height * 0.88;
      const yOf = (band: number) => bottom - (band / Math.max(1, count - 1)) * (bottom - top);

      ctx.save();
      // The centre line and the speakers' sides.
      ctx.strokeStyle = rgba([255, 255, 255], 0.07);
      ctx.lineWidth = 1;
      ctx.beginPath();
      for (const x of [cx - halfWidth, cx, cx + halfWidth]) {
        ctx.moveTo(x, top);
        ctx.lineTo(x, bottom);
      }
      ctx.stroke();
      ctx.fillStyle = rgba([255, 255, 255], 0.3);
      ctx.font = "600 11px system-ui, sans-serif";
      ctx.textAlign = "center";
      ctx.fillText("L", cx - halfWidth, bottom + 18);
      ctx.fillText("R", cx + halfWidth, bottom + 18);
      ctx.textAlign = "right";
      ctx.textBaseline = "middle";
      const octaves = Math.log2(frame.highestHz / frame.lowestHz);
      for (const [hz, label] of MARKS) {
        const band = (Math.log2(hz / frame.lowestHz) / octaves) * count - 0.5;
        if (band < 0 || band > count - 1) continue;
        ctx.fillText(label, cx - halfWidth - 10, yOf(band));
      }

      // The bands.
      ctx.globalCompositeOperation = scene.layer ? "source-over" : "lighter";
      const step = (bottom - top) / count;
      for (let band = 0; band < count; band++) {
        const level = levels[band];
        if (level < 0.05) continue;
        const x = cx + balance[band] * halfWidth;
        const y = yOf(band);
        const radius = step * (1 + 4 * level * level);
        ctx.globalAlpha = Math.min(1, level * 1.1);
        ctx.drawImage(lights[band], x - radius * 1.6, y - radius, radius * 3.2, radius * 2);
      }
      ctx.globalAlpha = 1;
      ctx.restore();
    },
  };
}

export const stage: Visualization = {
  id: "stage",
  get name() {
    return t("viz.stage.name");
  },
  get description() {
    return t("viz.stage.description");
  },
  create,
};
