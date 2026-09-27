// The circle of fifths: the twelve pitch classes as wedges, lit by how much
// of each the music has right now, so chords show as shapes and a key as a
// bright arc of neighbours. Colours follow the circle, so related keys have
// related hues. The middle names the key the last ten seconds or so fit
// best, and the note sounding most.

import { t } from "$lib/i18n";
import type { Renderer, Scene, Visualization } from "../types";
import { estimateKey, KEY_NAMES, keyName } from "../key";
import { approach, approachAll, clearStage, hsl, rgba } from "../util";

/** Pitch classes clockwise from the top: C G D A E B F♯ D♭ A♭ E♭ B♭ F. */
const ORDER = Array.from({ length: 12 }, (_, i) => (i * 7) % 12);
const KEY_MEMORY = 10; // Seconds.

function create(): Renderer {
  let now: Float32Array = new Float32Array(12);
  let memory: Float32Array = new Float32Array(12);
  let rotation = 0;
  let ring = 0;
  let loudness = 0;

  return {
    draw(scene: Scene) {
      const { ctx, width, height, dt, frame } = scene;
      // Sharpen (squares favour the strongest) and smooth.
      const target = frame.chroma.map((value) => value * value);
      now = approachAll(now, target, dt, 0.05, 0.35);
      loudness = approach(loudness, Math.max(frame.rms[0], frame.rms[1]), dt, 0.05, 0.4);
      if (!frame.silent) {
        const weight = loudness;
        memory = approachAll(memory, target.map((v) => v * weight), dt, KEY_MEMORY, KEY_MEMORY);
      }
      const key = estimateKey(memory);
      // The circle turns so the key's tonic drifts to the top.
      if (key) {
        const wanted = -ORDER.indexOf(key.tonic) * (Math.PI / 6);
        let delta = wanted - rotation;
        delta = Math.atan2(Math.sin(delta), Math.cos(delta));
        rotation += delta * (1 - Math.exp(-dt / 3));
      }
      ring = approach(ring, scene.beat ? 1 : 0, dt, 0, 0.35);

      clearStage(scene);
      const cx = width / 2;
      const cy = height / 2;
      const outer = Math.min(width, height) * 0.36;
      const inner = outer * 0.38;
      const wedge = (2 * Math.PI) / 12;

      ctx.save();
      ctx.translate(cx, cy);
      ctx.rotate(rotation);
      for (let i = 0; i < 12; i++) {
        const pitch = ORDER[i];
        const level = Math.min(1, now[pitch]);
        const start = -Math.PI / 2 + (i - 0.5) * wedge + 0.02;
        const end = start + wedge - 0.04;
        const radius = inner + (outer - inner) * (0.35 + 0.65 * level);
        const color = hsl(i * 30, 0.75, 0.35 + 0.3 * level);

        const gradient = ctx.createRadialGradient(0, 0, inner, 0, 0, radius);
        gradient.addColorStop(0, rgba(color, 0.15 + 0.3 * level));
        gradient.addColorStop(1, rgba(color, 0.25 + 0.75 * level));
        ctx.fillStyle = gradient;
        ctx.shadowColor = rgba(color, level);
        ctx.shadowBlur = 30 * level;
        ctx.beginPath();
        ctx.arc(0, 0, radius, start, end);
        ctx.arc(0, 0, inner, end, start, true);
        ctx.closePath();
        ctx.fill();

        // The note's name, upright however the circle has turned.
        const middle = (start + end) / 2;
        const labelRadius = outer + Math.max(14, outer * 0.08);
        ctx.save();
        ctx.translate(Math.cos(middle) * labelRadius, Math.sin(middle) * labelRadius);
        ctx.rotate(-rotation);
        ctx.shadowBlur = 0;
        ctx.fillStyle = rgba([255, 255, 255], 0.35 + 0.65 * level);
        ctx.font = `600 ${Math.max(11, outer * 0.075)}px system-ui, sans-serif`;
        ctx.textAlign = "center";
        ctx.textBaseline = "middle";
        ctx.fillText(KEY_NAMES[pitch], 0, 0);
        ctx.restore();
      }

      // The beat as a ring around the circle.
      ctx.shadowBlur = 0;
      ctx.strokeStyle = rgba([255, 255, 255], 0.08 + 0.4 * ring);
      ctx.lineWidth = 1 + 3 * ring;
      ctx.beginPath();
      ctx.arc(0, 0, outer * (1.24 + 0.03 * ring), 0, 2 * Math.PI);
      ctx.stroke();
      ctx.restore();

      // The key and the loudest note.
      ctx.save();
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      if (key && key.strength > 0.5) {
        ctx.fillStyle = rgba([255, 255, 255], Math.min(1, (key.strength - 0.5) * 3));
        ctx.font = `600 ${Math.max(14, inner * 0.26)}px system-ui, sans-serif`;
        ctx.fillText(keyName(key), cx, cy - inner * 0.12);
      }
      let loudest = 0;
      for (let pitch = 1; pitch < 12; pitch++) if (now[pitch] > now[loudest]) loudest = pitch;
      if (now[loudest] > 0.2) {
        ctx.fillStyle = rgba([255, 255, 255], 0.55);
        ctx.font = `${Math.max(11, inner * 0.14)}px system-ui, sans-serif`;
        ctx.fillText(`♪ ${KEY_NAMES[loudest]}`, cx, cy + inner * 0.22);
      }
      ctx.restore();
    },
  };
}

export const fifths: Visualization = {
  id: "fifths",
  get name() {
    return t("viz.fifths.name");
  },
  get description() {
    return t("viz.fifths.description");
  },
  create,
};
