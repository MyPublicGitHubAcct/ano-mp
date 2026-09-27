// An oscilloscope: the waveform, held still by the core's trigger, drawn as
// a glowing phosphor trace that fades behind it, over a graticule.

import type { Renderer, Scene, Visualization } from "../types";
import { approach, clearStage, rgba } from "../util";

function create(): Renderer {
  let gain = 1;
  let first = true;

  return {
    draw(scene: Scene) {
      const { ctx, width, height, dt, frame, palette } = scene;
      // Persistence: the previous traces fade over about 60 ms.
      clearStage(scene, first ? 1 : 1 - Math.exp(-dt / 0.06));
      first = false;

      // Scale quiet passages up (at most 4×) so the wave fills the screen.
      const peak = Math.max(frame.peak[0], frame.peak[1]);
      gain = approach(gain, Math.min(4, 0.85 / Math.max(peak, 0.2)), dt, 0.3, 1.5);

      const middle = height / 2;
      const amplitude = height * 0.42 * gain;

      ctx.save();
      ctx.strokeStyle = rgba([255, 255, 255], 0.05);
      ctx.lineWidth = 1;
      ctx.beginPath();
      for (let i = 1; i < 10; i++) {
        ctx.moveTo((width * i) / 10, 0);
        ctx.lineTo((width * i) / 10, height);
      }
      for (let i = 1; i < 8; i++) {
        ctx.moveTo(0, (height * i) / 8);
        ctx.lineTo(width, (height * i) / 8);
      }
      ctx.stroke();

      const trace = (samples: Float32Array, other: Float32Array | null, color: [number, number, number], alpha: number) => {
        const count = samples.length;
        ctx.beginPath();
        for (let i = 0; i < count; i++) {
          const value = other ? (samples[i] + other[i]) / 2 : samples[i];
          const x = (i / (count - 1)) * width;
          const y = middle - value * amplitude;
          if (i === 0) ctx.moveTo(x, y);
          else ctx.lineTo(x, y);
        }
        ctx.strokeStyle = rgba(color, alpha);
        ctx.stroke();
      };

      ctx.lineJoin = "round";
      ctx.globalCompositeOperation = "lighter";
      // Each channel faintly, then their sum brightly with a glow.
      ctx.lineWidth = 1.25;
      trace(frame.left, null, palette.colors[1], 0.35);
      trace(frame.right, null, palette.colors[2], 0.35);
      ctx.shadowColor = rgba(palette.colors[0], 0.9);
      ctx.shadowBlur = 14;
      ctx.lineWidth = 2.5;
      trace(frame.left, frame.right, palette.colors[0], 0.95);
      ctx.restore();
    },
  };
}

export const scope: Visualization = {
  id: "scope",
  name: "Oscilloscope",
  description: "The waveform as a glowing trace; each channel faintly behind their mix.",
  create,
};
