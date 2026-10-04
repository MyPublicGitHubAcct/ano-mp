// A phase portrait: the waveform plotted against itself a moment later and
// a moment after that (a delay embedding, as for a strange attractor),
// turning slowly in three dimensions. A pure tone draws a ring, a rich
// timbre a knot of loops, and noise a cloud, so the shape follows the
// sound's character rather than its loudness. The delay is the waveform's
// own (`embeddingDelay`), so the figure stays open at any pitch.

import { t } from "$lib/i18n";
import { embeddingDelay } from "../music";
import type { Renderer, Scene, Visualization } from "../types";
import { along, approach, clearStage, rgba } from "../util";

const RUN = 24;

function create(): Renderer {
  let mono = new Float32Array(0);
  let delay = 8;
  let gain = 1;
  let angle = 0;
  let tilt = 0.35;
  let first = true;

  return {
    draw(scene: Scene) {
      const { ctx, width, height, dt, time, frame, palette, calm } = scene;
      clearStage(scene, first ? 1 : 1 - Math.exp(-dt / (calm ? 0.3 : 0.12)));
      first = false;
      const { left, right } = frame;
      if (mono.length !== left.length) mono = new Float32Array(left.length);
      let mean = 0;
      for (let i = 0; i < left.length; i++) {
        mono[i] = (left[i] + right[i]) / 2;
        mean += mono[i] / left.length;
      }
      let loudest = 0;
      for (let i = 0; i < mono.length; i++) {
        mono[i] -= mean;
        loudest = Math.max(loudest, Math.abs(mono[i]));
      }
      delay = approach(delay, embeddingDelay(mono, 2, 64), dt, 0.3, 0.3);
      const lag = Math.max(1, Math.round(delay));
      // Scaled to the waveform's own size, so the shape fills the stage at any loudness.
      gain = approach(gain, Math.min(20, 0.9 / Math.max(loudest, 0.02)), dt, 0.3, 1.5);
      angle += dt * (calm ? 0.06 : 0.22) + (scene.beat ? 0.12 : 0);
      tilt = 0.35 + 0.15 * Math.sin(time * 0.05);

      const size = Math.min(width, height) * 0.42;
      const cx = width / 2;
      const cy = height / 2;
      const [sinA, cosA] = [Math.sin(angle), Math.cos(angle)];
      const [sinT, cosT] = [Math.sin(tilt), Math.cos(tilt)];
      const count = mono.length - 2 * lag;
      if (count < 2) return;

      ctx.save();
      ctx.globalCompositeOperation = scene.layer ? "source-over" : "lighter";
      ctx.lineWidth = 1.3;
      ctx.lineJoin = "round";
      const point = (i: number): [number, number, number] => {
        const x = mono[i] * gain;
        const y = mono[i + lag] * gain;
        const z = mono[i + 2 * lag] * gain;
        // Turn about the vertical, then tip towards the viewer.
        const rx = x * cosA + z * sinA;
        const rz = -x * sinA + z * cosA;
        const ry = y * cosT - rz * sinT;
        const depth = y * sinT + rz * cosT;
        const perspective = 1 / (1.25 - 0.3 * depth);
        return [cx + rx * size * perspective, cy - ry * size * perspective, depth];
      };
      for (let start = 0; start + 1 < count; start += RUN) {
        ctx.beginPath();
        let depth = 0;
        for (let i = start; i < Math.min(count, start + RUN + 1); i++) {
          const [x, y, z] = point(i);
          depth += z;
          if (i === start) ctx.moveTo(x, y);
          else ctx.lineTo(x, y);
        }
        depth /= RUN;
        // Nearer runs brighter; colours drift along the palette over the window.
        const hue = (start / count + time * 0.03) % 1;
        ctx.strokeStyle = rgba(along(palette, Math.abs(hue * 2 - 1)), 0.35 + 0.3 * Math.max(-1, Math.min(1, depth)));
        ctx.stroke();
      }
      ctx.restore();
    },
  };
}

export const portrait: Visualization = {
  id: "portrait",
  get name() {
    return t("viz.portrait.name");
  },
  get description() {
    return t("viz.portrait.description");
  },
  create,
};
