// A pair of analog VU meters. The needle moves with the level's voltage,
// as a real meter's does (so 0 VU sits at 71% of the scale), with VU
// ballistics; a lamp lights on peaks near full scale. 0 VU is -12 dBFS
// RMS rather than the studio's -18: finished masters run at -8 to -12, and
// would pin the needle.

import { t } from "$lib/i18n";
import type { Renderer, Rgb, Scene, Visualization } from "../types";
import { approach, clearStage, mix, rgba } from "../util";

const REFERENCE_DBFS = -12;
const TOP_VU = 3;
const MARKS = [-20, -10, -7, -5, -3, -2, -1, 0, 1, 2, 3];
const SWEEP = (100 * Math.PI) / 180; // The scale's arc.
const PEAK_LAMP = 0.89; // -1 dBFS.

/** Where on the scale (0..1) a level in VU sits: in proportion to its voltage. */
const deflection = (vu: number) => Math.min(1.08, 10 ** (vu / 20) / 10 ** (TOP_VU / 20));

function create(): Renderer {
  const needles = [0, 0];
  const lamps = [0, 0];

  return {
    draw(scene: Scene) {
      const { ctx, width, height, dt, frame, palette } = scene;
      clearStage(scene);

      for (let ch = 0; ch < 2; ch++) {
        const rms = frame.rms[ch];
        const vu = rms > 0 ? 20 * Math.log10(rms) - REFERENCE_DBFS : -60;
        // VU ballistics: about 300 ms to 99% either way.
        needles[ch] = approach(needles[ch], deflection(vu), dt, 0.065, 0.065);
        lamps[ch] = frame.peak[ch] >= PEAK_LAMP ? 1 : approach(lamps[ch], 0, dt, 0, 0.35);
      }

      // Side by side, or stacked in a tall window.
      const stacked = height > width * 0.9;
      const faceWidth = stacked
        ? Math.min(width * 0.86, (height * 0.44) / 0.62)
        : Math.min(width * 0.44, (height * 0.75) / 0.62);
      const faceHeight = faceWidth * 0.62;
      const gap = faceWidth * 0.08;
      for (let ch = 0; ch < 2; ch++) {
        const x = stacked ? (width - faceWidth) / 2 : width / 2 + (ch === 0 ? -gap / 2 - faceWidth : gap / 2);
        const y = stacked ? height / 2 + (ch === 0 ? -gap / 2 - faceHeight : gap / 2) : (height - faceHeight) / 2;
        meter(ctx, x, y, faceWidth, faceHeight, needles[ch], lamps[ch], ch === 0 ? "L" : "R", palette.colors[0]);
      }
    },
  };
}

function meter(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  position: number,
  lamp: number,
  label: string,
  tint: Rgb,
) {
  // A warm backlit face, tinted slightly by the cover.
  const face = mix([244, 216, 150], tint, 0.12);
  ctx.save();
  const light = ctx.createRadialGradient(x + w / 2, y + h * 0.9, h * 0.1, x + w / 2, y + h * 0.6, w * 0.75);
  light.addColorStop(0, rgba(face));
  light.addColorStop(1, rgba(mix(face, [60, 40, 10], 0.45)));
  ctx.fillStyle = light;
  ctx.strokeStyle = rgba([0, 0, 0], 0.6);
  ctx.lineWidth = Math.max(2, w * 0.012);
  ctx.beginPath();
  ctx.roundRect(x, y, w, h, w * 0.03);
  ctx.fill();
  ctx.stroke();
  ctx.clip();

  const pivotX = x + w / 2;
  const pivotY = y + h * 1.08;
  const radius = h * 0.82;
  const angle = (t: number) => -Math.PI / 2 - SWEEP / 2 + t * SWEEP;
  const ink = [40, 28, 14] as Rgb;
  const red = [196, 30, 30] as Rgb;

  // The scale: black up to 0 VU, red above.
  ctx.lineWidth = Math.max(1.5, w * 0.008);
  for (const [from, to, color] of [
    [-20, 0, ink],
    [0, TOP_VU, red],
  ] as const) {
    ctx.strokeStyle = rgba(color);
    ctx.beginPath();
    ctx.arc(pivotX, pivotY, radius, angle(deflection(from)), angle(deflection(to)));
    ctx.stroke();
  }

  ctx.font = `600 ${Math.max(9, w * 0.045)}px system-ui, sans-serif`;
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  for (const mark of MARKS) {
    const a = angle(deflection(mark));
    const major = mark % 5 === 0 || mark >= -3;
    const inner = radius + (major ? h * 0.02 : h * 0.03);
    const outer = radius + h * 0.07;
    ctx.strokeStyle = ctx.fillStyle = rgba(mark > 0 ? red : ink);
    ctx.beginPath();
    ctx.moveTo(pivotX + Math.cos(a) * inner, pivotY + Math.sin(a) * inner);
    ctx.lineTo(pivotX + Math.cos(a) * outer, pivotY + Math.sin(a) * outer);
    ctx.stroke();
    const text = radius + h * 0.14;
    ctx.fillText(mark > 0 ? `+${mark}` : `${Math.abs(mark)}`, pivotX + Math.cos(a) * text, pivotY + Math.sin(a) * text);
  }

  ctx.fillStyle = rgba(ink, 0.85);
  ctx.font = `700 ${Math.max(12, w * 0.09)}px Georgia, serif`;
  ctx.fillText("VU", pivotX, y + h * 0.66);
  ctx.font = `600 ${Math.max(9, w * 0.04)}px system-ui, sans-serif`;
  ctx.fillText(label, x + w * 0.08, y + h * 0.12);

  // The peak lamp.
  const lampX = x + w * 0.9;
  const lampY = y + h * 0.13;
  const lampRadius = Math.max(3, w * 0.022);
  ctx.fillStyle = rgba(mix([90, 20, 20], [255, 60, 40], lamp));
  ctx.shadowColor = rgba([255, 60, 40], lamp);
  ctx.shadowBlur = 16 * lamp;
  ctx.beginPath();
  ctx.arc(lampX, lampY, lampRadius, 0, 2 * Math.PI);
  ctx.fill();
  ctx.shadowBlur = 0;
  ctx.fillStyle = rgba(ink, 0.8);
  ctx.font = `600 ${Math.max(8, w * 0.03)}px system-ui, sans-serif`;
  ctx.fillText("PEAK", lampX, lampY + lampRadius + Math.max(8, w * 0.03));

  // The needle, with its shadow on the face.
  const a = angle(position);
  const tipX = pivotX + Math.cos(a) * (radius + h * 0.08);
  const tipY = pivotY + Math.sin(a) * (radius + h * 0.08);
  ctx.lineCap = "round";
  ctx.strokeStyle = rgba([0, 0, 0], 0.18);
  ctx.lineWidth = Math.max(2, w * 0.008);
  ctx.beginPath();
  ctx.moveTo(pivotX + 4, pivotY);
  ctx.lineTo(tipX + 4, tipY + 3);
  ctx.stroke();
  ctx.strokeStyle = rgba([20, 12, 6]);
  ctx.lineWidth = Math.max(1.5, w * 0.006);
  ctx.beginPath();
  ctx.moveTo(pivotX, pivotY);
  ctx.lineTo(tipX, tipY);
  ctx.stroke();
  ctx.restore();

  // Glass over the face.
  ctx.save();
  const glass = ctx.createLinearGradient(x, y, x, y + h);
  glass.addColorStop(0, "rgb(255 255 255 / 0.18)");
  glass.addColorStop(0.45, "rgb(255 255 255 / 0.02)");
  glass.addColorStop(1, "rgb(0 0 0 / 0.12)");
  ctx.fillStyle = glass;
  ctx.beginPath();
  ctx.roundRect(x, y, w, h, w * 0.03);
  ctx.fill();
  ctx.restore();
}

export const vu: Visualization = {
  id: "vu",
  get name() {
    return t("viz.vu.name");
  },
  get description() {
    return t("viz.vu.description");
  },
  create,
};
