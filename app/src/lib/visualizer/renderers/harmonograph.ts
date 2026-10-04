// A harmonograph: the Victorian drawing machine, two damped pendulums
// moving a pen across and two more moving the paper, tuned to the interval
// between the two loudest notes in just intonation, so a fifth draws the
// 3:2 figure and a major third the 5:4. The figure precesses with the
// music's loudness and dies away as a real pendulum does; a new interval,
// held for a moment, starts a fresh drawing as the old one fades.

import { t, type MessageKey } from "$lib/i18n";
import { INTERVAL_RATIOS, strongestInterval } from "../music";
import type { Renderer, Scene, Visualization } from "../types";
import { along, approach, approachAll, clearStage, rgba } from "../util";

const INTERVALS: MessageKey[] = [
  "harmonograph.unison",
  "harmonograph.minorSecond",
  "harmonograph.majorSecond",
  "harmonograph.minorThird",
  "harmonograph.majorThird",
  "harmonograph.fourth",
  "harmonograph.tritone",
  "harmonograph.fifth",
  "harmonograph.minorSixth",
  "harmonograph.majorSixth",
  "harmonograph.minorSeventh",
  "harmonograph.majorSeventh",
];

/** Pen time between points, and the pendulums' damping per unit of it. */
const STEP = 0.03;
const DAMPING = 0.011;
/** Below this amplitude the drawing starts again. */
const SPENT = 0.12;
const CHUNK = 160;

type Drawing = {
  semitones: number;
  ratio: [number, number];
  phases: [number, number, number, number];
  penTime: number;
  /** x, y pairs, -1..1. */
  points: number[];
  /** 1 while drawing; falls to 0 once replaced. */
  opacity: number;
};

function drawing(semitones: number): Drawing {
  const phase = () => Math.random() * 2 * Math.PI;
  return {
    semitones,
    ratio: INTERVAL_RATIOS[semitones],
    phases: [phase(), phase(), phase(), phase()],
    penTime: 0,
    points: [],
    opacity: 1,
  };
}

function create(): Renderer {
  let levels: Float32Array = new Float32Array(12);
  let current: Drawing = drawing(7);
  let fading: Drawing | null = null;
  let candidate = 7;
  let heldFor = 0;
  let loudness = 0;
  let precession = 0;

  /** Moves the pen on by `penTime`, the pendulums detuned by the music's loudness. */
  function advance(d: Drawing, penTime: number) {
    const [a, b] = d.ratio;
    const [p1, p2, p3, p4] = d.phases;
    // Larger ratios turn faster, so every figure draws at a similar pace.
    const scale = 1 / Math.max(a, b);
    for (let elapsed = 0; elapsed < penTime; elapsed += STEP) {
      const time = d.penTime;
      const decay = Math.exp(-DAMPING * time);
      const x =
        decay * (0.75 * Math.sin(a * scale * time + p1) + 0.25 * Math.sin(a * scale * time * (1 + precession) + p2));
      const y =
        decay * (0.75 * Math.sin(b * scale * time + p3) + 0.25 * Math.sin(b * scale * time * (1 - precession) + p4));
      d.points.push(x, y);
      d.penTime += STEP;
    }
  }

  function stroke(scene: Scene, d: Drawing, size: number) {
    const { ctx, width, height, palette } = scene;
    const cx = width / 2;
    const cy = height / 2 - Math.min(16, height * 0.03);
    const pairs = d.points.length / 2;
    for (let start = 0; start + 1 < pairs; start += CHUNK) {
      ctx.beginPath();
      for (let i = start; i < Math.min(pairs, start + CHUNK + 1); i++) {
        const x = cx + d.points[2 * i] * size;
        const y = cy + d.points[2 * i + 1] * size;
        if (i === start) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      }
      ctx.strokeStyle = rgba(along(palette, (start / Math.max(1, pairs)) % 1), 0.55 * d.opacity);
      ctx.stroke();
    }
    return [cx, cy] as const;
  }

  return {
    draw(scene: Scene) {
      const { ctx, width, height, dt, frame, calm } = scene;
      levels = approachAll(levels, frame.chroma, dt, 0.1, 0.5);
      loudness = approach(loudness, Math.max(frame.rms[0], frame.rms[1]), dt, 0.1, 0.8);
      precession = 0.002 + 0.02 * Math.min(1, loudness * 3);

      // A new interval, once it has held long enough to be more than a passing note.
      const interval = frame.silent ? null : strongestInterval(levels);
      if (interval !== null) {
        if (interval.semitones !== candidate) {
          candidate = interval.semitones;
          heldFor = 0;
        }
        heldFor += dt;
        if (candidate !== current.semitones && heldFor > (calm ? 1.5 : 0.5)) {
          fading = current;
          current = drawing(candidate);
        }
      }
      if (!frame.silent) advance(current, dt * (calm ? 3 : 8));
      const spent = Math.exp(-DAMPING * current.penTime) < SPENT;
      if (spent) {
        fading = current;
        current = drawing(current.semitones);
      }
      if (fading) {
        fading.opacity = approach(fading.opacity, 0, dt, 0, calm ? 1.5 : 0.8);
        if (fading.opacity < 0.02) fading = null;
      }

      clearStage(scene);
      const size = Math.min(width, height) * 0.38;
      ctx.save();
      ctx.lineWidth = 1.1;
      ctx.lineJoin = "round";
      if (fading) stroke(scene, fading, size);
      const [cx, cy] = stroke(scene, current, size);

      // The pen.
      const count = current.points.length;
      if (count >= 2) {
        const x = cx + current.points[count - 2] * size;
        const y = cy + current.points[count - 1] * size;
        ctx.fillStyle = rgba([255, 255, 255], 0.9);
        ctx.shadowColor = rgba(scene.palette.colors[0]);
        ctx.shadowBlur = 12;
        ctx.beginPath();
        ctx.arc(x, y, 2.5, 0, 2 * Math.PI);
        ctx.fill();
        ctx.shadowBlur = 0;
      }

      // The interval and its ratio.
      const [a, b] = current.ratio;
      const textSize = Math.max(13, Math.min(width, height) * 0.032);
      ctx.font = `600 ${textSize}px system-ui, sans-serif`;
      ctx.textAlign = "center";
      ctx.fillStyle = rgba([255, 255, 255], 0.75);
      ctx.fillText(
        t("harmonograph.caption", {
          interval: t(INTERVALS[current.semitones]),
          ratio: `${Math.max(a, b)}:${Math.min(a, b)}`,
        }),
        width / 2,
        height - textSize * 1.2,
      );
      ctx.restore();
    },
  };
}

export const harmonograph: Visualization = {
  id: "harmonograph",
  get name() {
    return t("viz.harmonograph.name");
  },
  get description() {
    return t("viz.harmonograph.description");
  },
  create,
};
