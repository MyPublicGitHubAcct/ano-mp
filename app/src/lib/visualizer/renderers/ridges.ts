// Ridgelines: the last few seconds of the spectrum as stacked ridges, the
// newest at the front, each hiding the ones behind it, like the cover of
// Unknown Pleasures. Bass rises in the middle and treble at the sides.

import type { Renderer, Scene, Visualization } from "../types";
import { approachAll, clearStage, mix, rgba, STAGE } from "../util";

const RIDGES = 44;
const SECONDS_PER_RIDGE = 1 / 14;
const POINTS = 96;

function create(): Renderer {
  let levels: Float32Array = new Float32Array(0);
  /** Newest last; each a profile of POINTS heights, 0..1. */
  const history: Float32Array[] = [];
  let sinceLast = Infinity;

  /** The spectrum mirrored about the middle, smoothed, and flattened towards the sides. */
  function profile(bands: Float32Array) {
    const points = new Float32Array(POINTS);
    for (let i = 0; i < POINTS; i++) {
      const fromMiddle = Math.abs(i / (POINTS - 1) - 0.5) * 2; // 0 in the middle, 1 at the sides
      const position = fromMiddle * (bands.length - 1);
      const low = Math.floor(position);
      const high = Math.min(bands.length - 1, low + 1);
      const value = bands[low] + (bands[high] - bands[low]) * (position - low);
      const envelope = 0.2 + 0.8 * Math.exp(-((fromMiddle / 0.55) ** 2));
      // A little noise keeps the flat parts alive, as in the original.
      points[i] = value * value * envelope + Math.random() * 0.008;
    }
    return points;
  }

  return {
    draw(scene: Scene) {
      const { ctx, width, height, dt, frame, palette } = scene;
      levels = approachAll(levels, frame.bands, dt, 0.02, 0.12);
      sinceLast += dt;
      if (sinceLast >= SECONDS_PER_RIDGE) {
        sinceLast = 0;
        history.push(profile(levels));
        if (history.length > RIDGES) history.shift();
      }

      clearStage(scene);
      const left = width * 0.18;
      const right = width * 0.82;
      const top = height * 0.14;
      const bottom = height * 0.9;
      const spacing = (bottom - top) / RIDGES;
      const lift = spacing * 9;
      const fill = rgba(mix(STAGE, palette.shade, 0.2));

      ctx.save();
      ctx.lineJoin = "round";
      // Oldest (top, back) first, so each ridge covers those behind it.
      const offset = RIDGES - history.length;
      for (let r = 0; r < history.length; r++) {
        const points = history[r];
        const baseline = top + (offset + r + 1) * spacing;
        const age = 1 - r / Math.max(1, history.length - 1); // 0 for the newest
        const x = (i: number) => left + ((right - left) * i) / (POINTS - 1);
        const y = (i: number) => baseline - points[i] * lift;
        ctx.beginPath();
        ctx.moveTo(left, baseline);
        ctx.lineTo(x(0), y(0));
        // Smooth: curves through the midpoints, with the points as controls.
        for (let i = 1; i < POINTS - 1; i++) {
          ctx.quadraticCurveTo(x(i), y(i), (x(i) + x(i + 1)) / 2, (y(i) + y(i + 1)) / 2);
        }
        ctx.lineTo(x(POINTS - 1), y(POINTS - 1));
        ctx.lineTo(right, baseline);
        ctx.fillStyle = fill;
        ctx.fill();
        const color = age === 0 ? palette.colors[0] : mix(palette.colors[0], [240, 240, 240], Math.min(1, age * 3));
        ctx.strokeStyle = rgba(color, 0.95 - 0.55 * age);
        ctx.lineWidth = age === 0 ? 2 : 1.4;
        ctx.stroke();
      }
      ctx.restore();
    },
  };
}

export const ridges: Visualization = {
  id: "ridges",
  name: "Ridgelines",
  description: "The last few seconds of the spectrum as stacked ridges, like the cover of Unknown Pleasures.",
  create,
};
