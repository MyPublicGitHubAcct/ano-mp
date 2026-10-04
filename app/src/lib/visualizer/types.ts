// What a visualization draws from, and what it must provide. Each one is a
// canvas renderer outside Svelte (PLAN.md §4.2): `Visualizer.svelte` calls
// `draw` once per animation frame with the latest analysis.

import type { CoverBasis } from "$lib/api";
import type { Frame } from "./frame";

export type Rgb = [number, number, number];

export type Palette = {
  /** Bright colours to draw with, the most prominent first; at least 3. */
  colors: Rgb[];
  /** A dark tint of the picture, for backgrounds. */
  shade: Rgb;
  /** Whether the colours came from the cover rather than the defaults. */
  fromCover: boolean;
};

export type Scene = {
  ctx: CanvasRenderingContext2D;
  /** In CSS pixels; the context is scaled by `dpr`. */
  width: number;
  height: number;
  dpr: number;
  /** Seconds since the visualization started, and since its previous draw (at most 0.1). */
  time: number;
  dt: number;
  /** The latest analysis. */
  frame: Frame;
  /** Whether a new analysis arrived since the previous draw, and whether any of those was a beat
      (at most three a second, none when calm: `safety.ts`). */
  fresh: boolean;
  beat: boolean;
  /** Calm mode (reduced motion, or the setting): move slowly and never jump (PLAN.md F18). */
  calm: boolean;
  /** The current track, and its cover once loaded. */
  track: { trackId: number; albumId: number | null } | null;
  cover: CanvasImageSource | null;
  palette: Palette;
  settings: { coverBasis: CoverBasis };
  /** Drawn over another visualization into a layer of its own (`combine.ts`): `clearStage` fades the
      layer to transparent rather than painting the stage. */
  layer?: boolean;
};

export type Renderer = {
  draw(scene: Scene): void;
  dispose?(): void;
};

export type Visualization = {
  id: string;
  name: string;
  description: string;
  create(): Renderer;
};
