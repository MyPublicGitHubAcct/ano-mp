// A pitch spiral: seven octaves of notes wound one octave a turn, C at the
// top, so every C lies on one spoke and every G on another. A note's
// harmonics fall on few spokes (the octaves on its own, the fifth beside),
// so a timbre shows as a star and a chord as a pattern of spokes; the
// strongest notes are joined, drawing the chord's shape.

import { t } from "$lib/i18n";
import { KEY_NAMES } from "../key";
import type { Renderer, Scene, Visualization } from "../types";
import { along, approach, approachAll, clearStage, rgba } from "../util";

/** Points drawn between neighbouring semitones on the spiral's line. */
const SMOOTHNESS = 4;
const JOINED = 0.55;

function create(): Renderer {
  let levels: Float32Array = new Float32Array(0);
  let ring = 0;

  return {
    draw(scene: Scene) {
      const { ctx, width, height, dt, frame, palette, calm } = scene;
      levels = approachAll(levels, frame.notes, dt, calm ? 0.12 : 0.03, calm ? 0.6 : 0.25);
      ring = approach(ring, scene.beat ? 1 : 0, dt, 0, 0.35);

      clearStage(scene);
      const count = levels.length;
      if (count === 0) return;
      const cx = width / 2;
      const cy = height / 2;
      const outer = Math.min(width, height) * 0.42;
      const inner = outer * 0.12;
      const first = frame.lowestNote;
      /** Where note `position` (semitones from the lowest, fractions between) lies on the spiral. */
      const place = (position: number): [number, number] => {
        const angle = -Math.PI / 2 + ((first + position) / 12) * 2 * Math.PI;
        const radius = inner + (outer - inner) * (position / Math.max(1, count - 1));
        return [cx + Math.cos(angle) * radius, cy + Math.sin(angle) * radius];
      };

      ctx.save();
      // The twelve spokes, named at the rim.
      ctx.strokeStyle = rgba([255, 255, 255], 0.06);
      ctx.lineWidth = 1;
      ctx.font = `600 ${Math.max(11, outer * 0.06)}px system-ui, sans-serif`;
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      for (let pc = 0; pc < 12; pc++) {
        const angle = -Math.PI / 2 + (pc / 12) * 2 * Math.PI;
        ctx.beginPath();
        ctx.moveTo(cx + Math.cos(angle) * inner * 0.6, cy + Math.sin(angle) * inner * 0.6);
        ctx.lineTo(cx + Math.cos(angle) * outer * 1.04, cy + Math.sin(angle) * outer * 1.04);
        ctx.stroke();
        let strongest = 0;
        for (let i = (pc - (first % 12) + 12) % 12; i < count; i += 12) strongest = Math.max(strongest, levels[i]);
        ctx.fillStyle = rgba([255, 255, 255], 0.3 + 0.7 * strongest);
        ctx.fillText(KEY_NAMES[pc], cx + Math.cos(angle) * outer * 1.12, cy + Math.sin(angle) * outer * 1.12);
      }

      // The spiral's line, brighter where it sounds.
      ctx.lineCap = "round";
      for (let i = 0; i + 1 < count; i++) {
        const level = Math.max(levels[i], levels[i + 1]);
        ctx.strokeStyle = rgba(along(palette, i / count), 0.12 + 0.5 * level);
        ctx.lineWidth = 1 + 3 * level;
        ctx.beginPath();
        for (let s = 0; s <= SMOOTHNESS; s++) {
          const [x, y] = place(i + s / SMOOTHNESS);
          if (s === 0) ctx.moveTo(x, y);
          else ctx.lineTo(x, y);
        }
        ctx.stroke();
      }

      // The strongest notes, joined in pitch order.
      const loud: number[] = [];
      for (let i = 0; i < count; i++) if (levels[i] > JOINED) loud.push(i);
      if (loud.length > 1) {
        ctx.strokeStyle = rgba([255, 255, 255], 0.18);
        ctx.lineWidth = 1;
        ctx.beginPath();
        loud.forEach((index, n) => {
          const [x, y] = place(index);
          if (n === 0) ctx.moveTo(x, y);
          else ctx.lineTo(x, y);
        });
        ctx.stroke();
      }

      // The notes.
      ctx.globalCompositeOperation = scene.layer ? "source-over" : "lighter";
      for (let i = 0; i < count; i++) {
        const level = levels[i];
        if (level < 0.15) continue;
        const [x, y] = place(i);
        const strength = (level - 0.15) / 0.85;
        const radius = 1.5 + outer * 0.05 * strength * strength;
        const color = along(palette, i / count);
        const glow = ctx.createRadialGradient(x, y, 0, x, y, radius * 2.5);
        glow.addColorStop(0, rgba([255, 255, 255], 0.9 * strength));
        glow.addColorStop(0.3, rgba(color, 0.8 * strength));
        glow.addColorStop(1, rgba(color, 0));
        ctx.fillStyle = glow;
        ctx.beginPath();
        ctx.arc(x, y, radius * 2.5, 0, 2 * Math.PI);
        ctx.fill();
      }
      ctx.globalCompositeOperation = "source-over";

      // The beat as a ring at the rim.
      ctx.strokeStyle = rgba([255, 255, 255], 0.06 + 0.35 * ring);
      ctx.lineWidth = 1 + 2 * ring;
      ctx.beginPath();
      ctx.arc(cx, cy, outer * (1.2 + 0.02 * ring), 0, 2 * Math.PI);
      ctx.stroke();
      ctx.restore();
    },
  };
}

export const spiral: Visualization = {
  id: "spiral",
  get name() {
    return t("viz.spiral.name");
  },
  get description() {
    return t("viz.spiral.description");
  },
  create,
};
