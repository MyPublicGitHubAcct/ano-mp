// Small helpers shared by the renderers: smoothing, colour and the stage.

import type { Palette, Rgb, Scene } from "./types";

/** The stage colour behind every visualization. */
export const STAGE: Rgb = [9, 9, 12];

/** Moves `current` towards `target` with separate time constants (seconds) for rising and falling, so
    values can jump up with the music and fall back slowly. */
export function approach(current: number, target: number, dt: number, rise: number, fall: number) {
  const tau = target > current ? rise : fall;
  return tau <= 0 ? target : current + (target - current) * (1 - Math.exp(-dt / tau));
}

/** `approach` for each value of `into`, towards `target` (resized to match). */
export function approachAll(into: Float32Array, target: Float32Array, dt: number, rise: number, fall: number) {
  const values = into.length === target.length ? into : new Float32Array(target.length);
  for (let i = 0; i < target.length; i++) values[i] = approach(values[i], target[i], dt, rise, fall);
  return values;
}

export const clamp = (value: number, low = 0, high = 1) => Math.min(high, Math.max(low, value));

export const rgba = ([r, g, b]: Rgb, alpha = 1) => `rgb(${r | 0} ${g | 0} ${b | 0} / ${alpha})`;

export function mix(a: Rgb, b: Rgb, t: number): Rgb {
  return [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
}

/** A colour `t` (0..1) of the way along the palette's colours. */
export function along(palette: Palette, t: number): Rgb {
  const colors = palette.colors;
  const position = clamp(t) * (colors.length - 1);
  const index = Math.min(colors.length - 2, Math.floor(position));
  return mix(colors[index], colors[index + 1], position - index);
}

export function hsl(h: number, s: number, l: number): Rgb {
  const a = s * Math.min(l, 1 - l);
  const f = (n: number) => {
    const k = (n + h / 30) % 12;
    return 255 * (l - a * Math.max(-1, Math.min(k - 3, 9 - k, 1)));
  };
  return [f(0), f(8), f(4)];
}

export function toHsl([r, g, b]: Rgb): [number, number, number] {
  const [rr, gg, bb] = [r / 255, g / 255, b / 255];
  const max = Math.max(rr, gg, bb);
  const min = Math.min(rr, gg, bb);
  const l = (max + min) / 2;
  const d = max - min;
  if (d === 0) return [0, 0, l];
  const s = d / (1 - Math.abs(2 * l - 1));
  const h = max === rr ? ((gg - bb) / d + 6) % 6 : max === gg ? (bb - rr) / d + 2 : (rr - gg) / d + 4;
  return [h * 60, s, l];
}

/** Paints the stage, faintly tinted by the palette; `alpha` below 1 leaves a fading trace of the last
    draw (a phosphor's persistence). In a layer, clears to transparent instead. */
export function clearStage(scene: Scene, alpha = 1) {
  const { ctx, width, height, palette } = scene;
  if (scene.layer) {
    ctx.save();
    ctx.globalCompositeOperation = "destination-out";
    ctx.globalAlpha = alpha;
    ctx.fillRect(0, 0, width, height);
    ctx.restore();
    return;
  }
  ctx.save();
  ctx.globalCompositeOperation = "source-over";
  ctx.globalAlpha = alpha;
  const tint = mix(STAGE, palette.shade, 0.35);
  const glow = ctx.createRadialGradient(width / 2, height / 2, 0, width / 2, height / 2, Math.hypot(width, height) / 2);
  glow.addColorStop(0, rgba(tint));
  glow.addColorStop(1, rgba(STAGE));
  ctx.fillStyle = glow;
  ctx.fillRect(0, 0, width, height);
  ctx.restore();
}

/** Fisher–Yates, in place. */
export function shuffle<T>(items: T[]) {
  for (let i = items.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [items[i], items[j]] = [items[j], items[i]];
  }
  return items;
}
