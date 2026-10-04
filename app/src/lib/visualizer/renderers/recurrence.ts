// A recurrence plot: the track compared with itself as it plays
// (`../recurrence.ts`). Time runs along both axes from the bottom left, so
// the diagonal is the present meeting itself; a section heard before lights
// a line parallel to it, and a repeated chorus a grid of them. The plot
// builds up over the track, and keeps the whole of a long one by halving
// its resolution. A marker on the diagonal is now; ticks on the axis mark
// the moments the present sounds most like.

import { t } from "$lib/i18n";
import { Recurrence } from "../recurrence";
import type { Palette, Renderer, Scene, Visualization } from "../types";
import { approach, clearStage, mix, rgba, STAGE } from "../util";

const CAPACITY = 256;
const STEP = 0.5; // Seconds, until the grid fills (after about two minutes).
const ECHOES = 3;
/** The fewest steps shown across, so the plot starts large and zooms out as it fills. */
const SHOWN = 64;

function minutes(seconds: number) {
  const whole = Math.round(seconds);
  return `${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, "0")}`;
}

/** 256 RGB colours from the stage through the palette to white, for similarities above the average. */
function ramp(palette: Palette) {
  const stops = [STAGE, mix(STAGE, palette.shade, 0.8), palette.colors[1], palette.colors[0], [255, 255, 255]] as const;
  const table = new Uint8ClampedArray(256 * 3);
  for (let i = 0; i < 256; i++) {
    const position = (i / 255) * (stops.length - 1);
    const index = Math.min(stops.length - 2, Math.floor(position));
    const color = mix(
      stops[index] as [number, number, number],
      stops[index + 1] as [number, number, number],
      position - index,
    );
    table.set(color, i * 3);
  }
  return table;
}

function create(): Renderer {
  const plot = new Recurrence(CAPACITY, STEP);
  const image = document.createElement("canvas");
  image.width = image.height = CAPACITY;
  const imageCtx = image.getContext("2d");
  const pixels = imageCtx?.createImageData(CAPACITY, CAPACITY) ?? null;

  let trackId: number | null = null;
  let palette: Palette | null = null;
  let colors = new Uint8ClampedArray(0);
  let painted = 0;
  let generation = -1;
  /** The average similarity off the diagonal, which maps to the stage colour. */
  let total = 0;
  let pairs = 0;
  let pulse = 0;

  /** Rows and columns from `from` to the newest, or all of them. */
  function paint(from: number) {
    if (!pixels || !imageCtx) return;
    const count = plot.features.length;
    const floor = pairs > 0 ? total / pairs : 0.5;
    const level = (value: number) => {
      const lifted = Math.max(0, (value - floor) / Math.max(0.05, 1 - floor));
      return Math.round(255 * lifted ** 1.4);
    };
    const put = (i: number, j: number) => {
      // Row 0 at the bottom.
      const offset = ((CAPACITY - 1 - i) * CAPACITY + j) * 4;
      const index = level(plot.similarity(i, j)) * 3;
      pixels.data[offset] = colors[index];
      pixels.data[offset + 1] = colors[index + 1];
      pixels.data[offset + 2] = colors[index + 2];
      pixels.data[offset + 3] = 255;
    };
    if (from === 0) pixels.data.fill(0);
    for (let i = from; i < count; i++) {
      for (let j = 0; j <= i; j++) {
        put(i, j);
        put(j, i);
      }
    }
    imageCtx.putImageData(pixels, 0, 0);
    painted = count;
  }

  function measure() {
    total = 0;
    pairs = 0;
    for (let i = 0; i < plot.features.length; i++) {
      for (let j = 0; j < i; j++) total += plot.similarity(i, j);
      pairs += i;
    }
  }

  return {
    draw(scene: Scene) {
      const { ctx, width, height, dt, frame } = scene;
      const track = scene.track?.trackId ?? null;
      if (track !== trackId) {
        trackId = track;
        plot.reset();
        total = pairs = 0;
      }
      if (!frame.silent && plot.add(frame.chroma, frame.bands, dt)) {
        const newest = plot.features.length - 1;
        for (let j = 0; j < newest; j++) total += plot.similarity(newest, j);
        pairs += newest;
      }
      if (scene.palette !== palette) {
        palette = scene.palette;
        colors = ramp(palette);
        generation = -1;
      }
      if (plot.generation !== generation) {
        generation = plot.generation;
        measure();
        paint(0);
      } else if (plot.features.length > painted) {
        // The average moves slowly; repaint everything now and then so old cells follow it.
        paint(plot.features.length % 32 === 0 ? 0 : painted);
      }
      pulse = approach(pulse, scene.beat ? 1 : 0, dt, 0, 0.3);

      clearStage(scene);
      const size = Math.min(width * 0.86, height * 0.8);
      const left = (width - size) / 2;
      const top = (height - size) / 2 - Math.min(12, height * 0.02);
      const count = plot.features.length;
      const shown = Math.min(CAPACITY, Math.max(SHOWN, Math.ceil(count / 16) * 16));
      const cell = size / shown;
      ctx.save();
      // Row 0 is the image's bottom row; show the bottom-left `shown` cells, sharp.
      ctx.imageSmoothingEnabled = false;
      ctx.drawImage(image, 0, CAPACITY - shown, shown, shown, left, top, size, size);
      ctx.imageSmoothingEnabled = true;
      ctx.strokeStyle = rgba([255, 255, 255], 0.1);
      ctx.lineWidth = 1;
      ctx.strokeRect(left - 0.5, top - 0.5, size + 1, size + 1);

      if (count > 0) {
        // Now, on the diagonal.
        const x = left + (count - 0.5) * cell;
        const y = top + size - (count - 0.5) * cell;
        ctx.fillStyle = rgba(scene.palette.colors[0]);
        ctx.shadowColor = rgba(scene.palette.colors[0]);
        ctx.shadowBlur = 10 + 10 * pulse;
        ctx.beginPath();
        ctx.arc(x, y, Math.max(3, cell * 1.5) * (1 + 0.3 * pulse), 0, 2 * Math.PI);
        ctx.fill();
        ctx.shadowBlur = 0;

        // What now sounds most like, at least eight steps back.
        const newest = count - 1;
        const echoes = Array.from({ length: Math.max(0, newest - 8) }, (_, j) => j)
          .sort((a, b) => plot.similarity(newest, b) - plot.similarity(newest, a))
          .slice(0, ECHOES)
          .filter((j) => plot.similarity(newest, j) > 0.85);
        ctx.fillStyle = rgba(scene.palette.colors[0], 0.9);
        for (const j of echoes) {
          const ex = left + (j + 0.5) * cell;
          ctx.beginPath();
          ctx.moveTo(ex, top + size + 4);
          ctx.lineTo(ex - 4, top + size + 11);
          ctx.lineTo(ex + 4, top + size + 11);
          ctx.closePath();
          ctx.fill();
        }

        // The time the plot spans.
        ctx.fillStyle = rgba([255, 255, 255], 0.4);
        ctx.font = "11px system-ui, sans-serif";
        ctx.textBaseline = "top";
        ctx.textAlign = "left";
        ctx.fillText("0:00", left, top + size + 14);
        ctx.textAlign = "right";
        ctx.fillText(minutes(shown * plot.step), left + size, top + size + 14);
        ctx.textAlign = "center";
        ctx.fillText(t("recurrence.resolution", { seconds: plot.step }), left + size / 2, top + size + 14);
      }
      ctx.restore();
    },
  };
}

export const recurrence: Visualization = {
  id: "recurrence",
  get name() {
    return t("viz.recurrence.name");
  },
  get description() {
    return t("viz.recurrence.description");
  },
  create,
};
