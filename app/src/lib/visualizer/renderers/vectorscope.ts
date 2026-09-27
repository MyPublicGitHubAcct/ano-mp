// A vectorscope (goniometer), as mastering engineers use: each sample pair
// plotted with the mid (L+R) upwards and the side (L−R) across, so mono
// draws a vertical line, wide stereo a cloud, and out-of-phase sound a
// horizontal smear. Traces glow and fade like a phosphor; the bar below is
// the channels' correlation.

import type { Renderer, Scene, Visualization } from "../types";
import { along, approach, clearStage, rgba } from "../util";

function create(): Renderer {
  let gain = 1;
  let correlation = 1;
  let first = true;

  return {
    draw(scene: Scene) {
      const { ctx, width, height, dt, time, frame, palette } = scene;
      clearStage(scene, first ? 1 : 1 - Math.exp(-dt / 0.12));
      first = false;

      const size = Math.min(width, height * 0.86) * 0.44;
      const cx = width / 2;
      const cy = height * 0.46;
      const peak = Math.max(frame.peak[0], frame.peak[1]);
      gain = approach(gain, Math.min(5, 0.9 / Math.max(peak, 0.18)), dt, 0.3, 1.5);

      // The graticule: a circle and the L and R axes.
      ctx.save();
      ctx.strokeStyle = rgba([255, 255, 255], 0.06);
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.arc(cx, cy, size, 0, 2 * Math.PI);
      for (const angle of [Math.PI / 4, -Math.PI / 4, 0, Math.PI / 2]) {
        ctx.moveTo(cx - Math.cos(angle) * size, cy - Math.sin(angle) * size);
        ctx.lineTo(cx + Math.cos(angle) * size, cy + Math.sin(angle) * size);
      }
      ctx.stroke();
      ctx.fillStyle = rgba([255, 255, 255], 0.25);
      ctx.font = "600 11px system-ui, sans-serif";
      ctx.textAlign = "center";
      ctx.fillText("L", cx - size * 0.74, cy - size * 0.74);
      ctx.fillText("R", cx + size * 0.74, cy - size * 0.74);

      const { left, right } = frame;
      let product = 0;
      let squaresLeft = 0;
      let squaresRight = 0;
      ctx.beginPath();
      ctx.arc(cx, cy, size + 2, 0, 2 * Math.PI);
      ctx.clip();
      ctx.globalCompositeOperation = "lighter";
      ctx.lineWidth = 1.2;
      ctx.lineJoin = "round";
      // In short runs, each coloured a step further along the palette.
      const run = 32;
      for (let start = 0; start + 1 < left.length; start += run) {
        ctx.beginPath();
        for (let i = start; i < Math.min(left.length, start + run + 1); i++) {
          const l = left[i] * gain;
          const r = right[i] * gain;
          // Mid and side as halves, so full scale stays inside the circle.
          const x = cx + ((r - l) / 2) * size;
          const y = cy - ((l + r) / 2) * size;
          if (i === start) ctx.moveTo(x, y);
          else ctx.lineTo(x, y);
        }
        const hue = (start / left.length + time * 0.05) % 1;
        ctx.strokeStyle = rgba(along(palette, Math.abs(hue * 2 - 1)), 0.5);
        ctx.stroke();
      }
      for (let i = 0; i < left.length; i++) {
        product += left[i] * right[i];
        squaresLeft += left[i] * left[i];
        squaresRight += right[i] * right[i];
      }
      ctx.restore();

      // The correlation meter: -1 (out of phase) to +1 (mono).
      const energy = squaresLeft * squaresRight;
      if (energy > 1e-9) correlation = approach(correlation, product / Math.sqrt(energy), dt, 0.15, 0.15);
      const barWidth = Math.min(width * 0.6, size * 2);
      const barX = (width - barWidth) / 2;
      const barY = Math.min(height - 28, cy + size + 36);
      ctx.save();
      ctx.fillStyle = rgba([255, 255, 255], 0.08);
      ctx.fillRect(barX, barY, barWidth, 4);
      const markerX = barX + ((correlation + 1) / 2) * barWidth;
      ctx.fillStyle = rgba(correlation < 0 ? [255, 110, 90] : palette.colors[0]);
      ctx.fillRect(Math.min(markerX, barX + barWidth / 2), barY - 1, Math.abs(markerX - (barX + barWidth / 2)), 6);
      ctx.fillStyle = rgba([255, 255, 255], 0.35);
      ctx.font = "11px system-ui, sans-serif";
      ctx.textAlign = "left";
      ctx.fillText("−1", barX, barY + 18);
      ctx.textAlign = "right";
      ctx.fillText("+1", barX + barWidth, barY + 18);
      ctx.textAlign = "center";
      ctx.fillText("correlation", barX + barWidth / 2, barY + 18);
      ctx.restore();
    },
  };
}

export const vectorscope: Visualization = {
  id: "vectorscope",
  name: "Vectorscope",
  description: "The stereo image: mono draws a vertical line, wide stereo a cloud; with the channels' correlation.",
  create,
};
