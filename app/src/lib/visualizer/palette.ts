// Colours for the visualizations from the current cover, so each album
// gets its own look; the defaults when there's no cover or it can't be read.

import type { Palette, Rgb } from "./types";
import { hsl, mix, toHsl } from "./util";

export const DEFAULT_PALETTE: Palette = {
  colors: [
    [127, 156, 255],
    [177, 140, 255],
    [255, 140, 198],
    [255, 184, 107],
    [107, 228, 255],
  ],
  shade: [30, 34, 60],
  fromCover: false,
};

const SIZE = 48;
const HUE_BUCKETS = 24;
const MIN_HUE_GAP = 35;

/** The cover's most prominent colours, made bright enough to draw with on the dark stage. Null if the image
    can't be read (it wasn't loaded with CORS). */
export function paletteFrom(image: CanvasImageSource): Palette | null {
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = SIZE;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) return null;
  ctx.drawImage(image, 0, 0, SIZE, SIZE);
  let pixels: Uint8ClampedArray;
  try {
    pixels = ctx.getImageData(0, 0, SIZE, SIZE).data;
  } catch {
    return null; // A tainted canvas.
  }

  // Colourful pixels by hue, weighted by how colourful; and the average of all.
  const buckets = Array.from({ length: HUE_BUCKETS }, () => ({ weight: 0, r: 0, g: 0, b: 0 }));
  const total: Rgb = [0, 0, 0];
  for (let i = 0; i < pixels.length; i += 4) {
    const rgb: Rgb = [pixels[i], pixels[i + 1], pixels[i + 2]];
    total[0] += rgb[0];
    total[1] += rgb[1];
    total[2] += rgb[2];
    const [h, s, l] = toHsl(rgb);
    if (s < 0.2 || l < 0.12 || l > 0.9) continue;
    const bucket = buckets[Math.floor(h / (360 / HUE_BUCKETS)) % HUE_BUCKETS];
    const weight = s * (1 - Math.abs(l - 0.5));
    bucket.weight += weight;
    bucket.r += rgb[0] * weight;
    bucket.g += rgb[1] * weight;
    bucket.b += rgb[2] * weight;
  }
  const count = pixels.length / 4;
  const average: Rgb = [total[0] / count, total[1] / count, total[2] / count];

  // The heaviest hues, at least MIN_HUE_GAP apart.
  const ranked = buckets.filter((bucket) => bucket.weight > 0).sort((a, b) => b.weight - a.weight);
  const minimum = ranked.reduce((sum, bucket) => sum + bucket.weight, 0) * 0.03;
  const hues: number[] = [];
  const colors: Rgb[] = [];
  for (const { weight, r, g, b } of ranked) {
    if (weight < minimum && colors.length >= 1) break;
    const [h, s, l] = toHsl([r / weight, g / weight, b / weight]);
    if (hues.some((other) => Math.min(Math.abs(other - h), 360 - Math.abs(other - h)) < MIN_HUE_GAP)) continue;
    hues.push(h);
    colors.push(vivid(h, s, l));
    if (colors.length === 5) break;
  }

  // A cover with few colours (or none: black and white) gets neighbours of its main hue.
  const [baseHue, baseSaturation] = colors.length > 0 ? toHsl(colors[0]) : [toHsl(average)[0], 0];
  const saturation = colors.length > 0 ? Math.max(0.45, baseSaturation) : 0.12;
  for (const offset of [40, -40, 180, 90]) {
    if (colors.length >= 3) break;
    colors.push(vivid(baseHue + offset, saturation, 0.65));
  }

  return { colors, shade: mix(average, [0, 0, 0], 0.55), fromCover: true };
}

/** Loads the cover at `url` with CORS so its colours can be read, then calls `done` with the image and its
    palette (null if its colours can't be read), or with nulls if it doesn't load. Returns a function that
    cancels. */
export function loadCover(
  url: string,
  done: (image: HTMLImageElement | null, palette: Palette | null) => void,
): () => void {
  let current = true;
  const load = (cors: boolean) => {
    const image = new Image();
    if (cors) image.crossOrigin = "anonymous";
    image.decoding = "async";
    image.onload = () => {
      if (current) done(image, paletteFrom(image));
    };
    image.onerror = () => {
      if (!current) return;
      // A copy cached before the art had CORS headers fails with CORS; show it without its colours.
      if (cors) load(false);
      else done(null, null);
    };
    image.src = url;
  };
  load(true);
  return () => {
    current = false;
  };
}

/** The hue at a saturation and lightness that shows on the dark stage. */
function vivid(h: number, s: number, l: number): Rgb {
  return hsl(((h % 360) + 360) % 360, Math.min(0.95, Math.max(0.5, s)), Math.min(0.75, Math.max(0.58, l)));
}
