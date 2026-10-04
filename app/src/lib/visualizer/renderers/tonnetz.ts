// The Tonnetz: Euler's lattice of the twelve pitch classes, fifths along
// each row and major thirds up each row (minor thirds up and back), so each
// triangle is a triad: pointing up major, down minor. Nodes light with the
// chroma, triangles with how much of their triad sounds, and the triad
// sounding most glows wherever it appears; a trail follows it, so a chord
// change is a step to a neighbouring triangle and a modulation a walk.

import { t } from "$lib/i18n";
import { KEY_NAMES } from "../key";
import { strongestTriad, tonnetzPitch, type Triad } from "../music";
import type { Renderer, Scene, Visualization } from "../types";
import { along, approach, approachAll, clearStage, mix, rgba } from "../util";

const TRAIL = 90;
/** Of the shorter side, between neighbouring nodes. */
const SPACING = 1 / 5.5;

/** A colour for each pitch class, by its place on the circle of fifths, so related notes look related. */
function colorOf(scene: Scene, pitch: number) {
  return along(scene.palette, ((pitch * 7) % 12) / 11);
}

function create(): Renderer {
  let levels: Float32Array = new Float32Array(12);
  /** The triad shown, and how brightly (it fades out before another takes over). */
  let shown: Triad | null = null;
  let glow = 0;
  let pulse = 0;
  /** Where the trail's head is, relative to the centre, and where it has been. */
  let head: [number, number] | null = null;
  const trail: [number, number][] = [];

  return {
    draw(scene: Scene) {
      const { ctx, width, height, dt, frame, calm } = scene;
      levels = approachAll(
        levels,
        frame.chroma.map((value) => value * value),
        dt,
        calm ? 0.2 : 0.06,
        calm ? 0.8 : 0.4,
      );
      const triad = frame.silent ? null : strongestTriad(levels);
      const same = triad !== null && shown !== null && triad.root === shown.root && triad.minor === shown.minor;
      if (same) glow = approach(glow, Math.min(1, 0.4 + triad.strength), dt, 0.1, 0.1);
      else {
        glow = approach(glow, 0, dt, 0, calm ? 0.4 : 0.15);
        if (glow < 0.05) shown = triad;
      }
      pulse = approach(pulse, scene.beat ? 1 : 0, dt, 0, 0.3);

      clearStage(scene);
      const spacing = Math.min(width, height) * SPACING;
      const rise = (spacing * Math.sqrt(3)) / 2;
      const cx = width / 2;
      const cy = height / 2;
      const rows = Math.ceil(height / 2 / rise) + 1;
      const position = (column: number, row: number): [number, number] => [
        cx + (column + row / 2) * spacing,
        cy - row * rise,
      ];
      const columnsFor = (row: number) => {
        const reach = Math.ceil(width / 2 / spacing) + 2;
        return [Math.floor(-reach - row / 2), Math.ceil(reach - row / 2)];
      };

      // Triangles: each up (major, root at its lower left) and down (minor, root at its top left).
      let target: [number, number] | null = null;
      let nearest = Infinity;
      ctx.save();
      ctx.lineJoin = "round";
      for (let row = -rows; row < rows; row++) {
        const [from, to] = columnsFor(row);
        for (let column = from; column < to; column++) {
          const a = position(column, row);
          const b = position(column + 1, row);
          const c = position(column, row + 1);
          const d = position(column + 1, row + 1);
          for (const minor of [false, true]) {
            const corners = minor ? [b, c, d] : [a, b, c];
            const pitches = minor
              ? [tonnetzPitch(column + 1, row), tonnetzPitch(column, row + 1), tonnetzPitch(column + 1, row + 1)]
              : [tonnetzPitch(column, row), tonnetzPitch(column + 1, row), tonnetzPitch(column, row + 1)];
            const root = minor ? pitches[1] : pitches[0];
            const sounding = Math.min(levels[pitches[0]], levels[pitches[1]], levels[pitches[2]]);
            const lit = shown !== null && shown.root === root && shown.minor === minor ? glow : 0;
            const alpha = 0.04 + 0.3 * sounding ** 1.5 + 0.5 * lit;
            if (alpha < 0.05) continue;
            const color = colorOf(scene, root);
            ctx.beginPath();
            ctx.moveTo(...corners[0]);
            ctx.lineTo(...corners[1]);
            ctx.lineTo(...corners[2]);
            ctx.closePath();
            ctx.fillStyle = rgba(minor ? mix(color, [20, 20, 40], 0.3) : color, Math.min(0.9, alpha));
            ctx.fill();
            if (lit > 0) {
              ctx.strokeStyle = rgba([255, 255, 255], lit * (0.5 + 0.4 * pulse));
              ctx.lineWidth = 1.5 + 2 * pulse;
              ctx.stroke();
              // The instance nearest where the trail was, to walk from.
              const centre: [number, number] = [
                (corners[0][0] + corners[1][0] + corners[2][0]) / 3 - cx,
                (corners[0][1] + corners[1][1] + corners[2][1]) / 3 - cy,
              ];
              const from = head ?? [0, 0];
              const distance = Math.hypot(centre[0] - from[0], centre[1] - from[1]);
              if (distance < nearest) {
                nearest = distance;
                target = centre;
              }
            }
          }
        }
      }

      // The lattice's edges and nodes.
      ctx.strokeStyle = rgba([255, 255, 255], 0.07);
      ctx.lineWidth = 1;
      ctx.beginPath();
      for (let row = -rows; row < rows; row++) {
        const [from, to] = columnsFor(row);
        for (let column = from; column < to; column++) {
          const a = position(column, row);
          for (const b of [position(column + 1, row), position(column, row + 1), position(column - 1, row + 1)]) {
            ctx.moveTo(...a);
            ctx.lineTo(...b);
          }
        }
      }
      ctx.stroke();
      const font = Math.max(10, spacing * 0.15);
      ctx.font = `600 ${font}px system-ui, sans-serif`;
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      for (let row = -rows; row < rows; row++) {
        const [from, to] = columnsFor(row);
        for (let column = from; column < to; column++) {
          const [x, y] = position(column, row);
          if (x < -spacing || x > width + spacing || y < -spacing || y > height + spacing) continue;
          const pitch = tonnetzPitch(column, row);
          const level = Math.min(1, levels[pitch]);
          const radius = spacing * (0.13 + 0.07 * level);
          ctx.fillStyle = rgba(mix([22, 22, 30], colorOf(scene, pitch), 0.25 + 0.75 * level));
          ctx.beginPath();
          ctx.arc(x, y, radius, 0, 2 * Math.PI);
          ctx.fill();
          ctx.fillStyle = rgba([255, 255, 255], 0.35 + 0.65 * level);
          ctx.fillText(KEY_NAMES[pitch], x, y);
        }
      }

      // The trail: the head walks to the chord's nearest triangle.
      if (target !== null && glow > 0.3) {
        head = head === null ? target : head;
        const speed = calm ? 0.5 : 0.15;
        head = [approach(head[0], target[0], dt, speed, speed), approach(head[1], target[1], dt, speed, speed)];
        trail.push([head[0], head[1]]);
      } else if (trail.length > 0) {
        trail.shift();
      }
      if (trail.length > TRAIL) trail.shift();
      if (trail.length > 1) {
        ctx.lineCap = "round";
        for (let i = 1; i < trail.length; i++) {
          const age = i / trail.length;
          ctx.strokeStyle = rgba([255, 255, 255], 0.6 * age);
          ctx.lineWidth = 1 + 3 * age;
          ctx.beginPath();
          ctx.moveTo(cx + trail[i - 1][0], cy + trail[i - 1][1]);
          ctx.lineTo(cx + trail[i][0], cy + trail[i][1]);
          ctx.stroke();
        }
      }
      ctx.restore();

      // The chord's name.
      if (shown !== null && glow > 0.1) {
        ctx.save();
        const size = Math.max(14, Math.min(width, height) * 0.045);
        ctx.font = `600 ${size}px system-ui, sans-serif`;
        ctx.textAlign = "center";
        ctx.fillStyle = rgba([255, 255, 255], Math.min(1, glow));
        ctx.shadowColor = "rgb(0 0 0 / 0.8)";
        ctx.shadowBlur = 8;
        const root = KEY_NAMES[shown.root];
        ctx.fillText(t(shown.minor ? "tonnetz.minor" : "tonnetz.major", { root }), width / 2, height - size * 1.2);
        ctx.restore();
      }
    },
  };
}

export const tonnetz: Visualization = {
  id: "tonnetz",
  get name() {
    return t("viz.tonnetz.name");
  },
  get description() {
    return t("viz.tonnetz.description");
  },
  create,
};
