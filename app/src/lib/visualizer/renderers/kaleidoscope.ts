// A kaleidoscope of the current cover: a slowly wandering slice of it,
// with light blooming over it from the spectrum, mirrored into a ring of
// wedges that turns with the music and punches in on beats.

import { t } from "$lib/i18n";
import type { Renderer, Scene, Visualization } from "../types";
import { along, approach, rgba, STAGE } from "../util";

const SOURCE = 512;
const WEDGES = 12;

function create(): Renderer {
  const source = document.createElement("canvas");
  source.width = source.height = SOURCE;
  const pattern = source.getContext("2d");
  let angle = 0;
  let zoom = 1;
  let energy = 0;
  let bass = 0;

  /** The picture the wedges mirror: part of the cover, or the palette alone, with blooms from the bands. */
  function paint(scene: Scene) {
    if (!pattern) return;
    const { time, frame, palette, cover } = scene;
    pattern.globalCompositeOperation = "source-over";
    pattern.fillStyle = rgba(palette.shade);
    pattern.fillRect(0, 0, SOURCE, SOURCE);
    if (cover) {
      // Wander over the cover, zoomed in, so the pattern keeps changing.
      const scale = 1.6 + 0.3 * Math.sin(time * 0.07);
      const x = Math.sin(time * 0.05) * SOURCE * 0.25;
      const y = Math.cos(time * 0.037) * SOURCE * 0.25;
      pattern.save();
      pattern.translate(SOURCE / 2 + x, SOURCE / 2 + y);
      pattern.rotate(time * 0.03);
      pattern.drawImage(cover, (-SOURCE * scale) / 2, (-SOURCE * scale) / 2, SOURCE * scale, SOURCE * scale);
      pattern.restore();
    }
    // Six blooms, each driven by a sixth of the spectrum.
    pattern.globalCompositeOperation = "lighter";
    const bands = frame.bands;
    for (let b = 0; b < 6; b++) {
      let level = 0;
      const from = Math.floor((b * bands.length) / 6);
      const to = Math.floor(((b + 1) * bands.length) / 6);
      for (let i = from; i < to; i++) level = Math.max(level, bands[i]);
      const x = SOURCE * (0.5 + 0.35 * Math.cos(time * (0.11 + b * 0.03) + b));
      const y = SOURCE * (0.5 + 0.35 * Math.sin(time * (0.13 + b * 0.02) + b * 2));
      const radius = SOURCE * (0.08 + 0.25 * level * level);
      const bloom = pattern.createRadialGradient(x, y, 0, x, y, radius);
      const color = along(palette, b / 5);
      bloom.addColorStop(0, rgba(color, cover ? 0.55 * level : 0.9 * level + 0.1));
      bloom.addColorStop(1, rgba(color, 0));
      pattern.fillStyle = bloom;
      pattern.fillRect(0, 0, SOURCE, SOURCE);
    }
  }

  return {
    draw(scene: Scene) {
      const { ctx, width, height, dt, frame } = scene;
      const rms = Math.max(frame.rms[0], frame.rms[1]);
      energy = approach(energy, rms, dt, 0.1, 0.8);
      bass = approach(bass, frame.bands.slice(0, 8).reduce((a, b) => Math.max(a, b), 0), dt, 0.03, 0.25);
      // Turning faster when loud; punching in on a beat and easing back.
      angle += dt * (scene.calm ? 0.02 : 0.04 + 0.35 * energy);
      zoom = scene.beat ? 1.12 : approach(zoom, 1 + (scene.calm ? 0 : 0.05 * bass), dt, 0.05, 0.6);
      paint(scene);

      ctx.save();
      ctx.fillStyle = rgba(STAGE);
      ctx.fillRect(0, 0, width, height);
      ctx.translate(width / 2, height / 2);
      ctx.rotate(angle);
      ctx.scale(zoom, zoom);
      const radius = Math.hypot(width, height) / 2 / zoom + 2;
      const slice = (2 * Math.PI) / WEDGES;
      for (let w = 0; w < WEDGES; w++) {
        ctx.save();
        ctx.rotate(w * slice);
        if (w % 2 === 1) {
          // Every other wedge mirrored, so the edges meet seamlessly.
          ctx.rotate(slice);
          ctx.scale(1, -1);
        }
        ctx.beginPath();
        ctx.moveTo(0, 0);
        // Overlapping the neighbours a little, which match at the edge, hides the seams.
        ctx.arc(0, 0, radius, -0.015, slice + 0.015);
        ctx.closePath();
        ctx.clip();
        ctx.drawImage(source, 0, -radius * 0.15, radius, radius);
        ctx.restore();
      }
      ctx.restore();

      // A soft vignette.
      const vignette = ctx.createRadialGradient(width / 2, height / 2, Math.min(width, height) * 0.3, width / 2, height / 2, Math.hypot(width, height) / 2);
      vignette.addColorStop(0, "rgb(0 0 0 / 0)");
      vignette.addColorStop(1, "rgb(0 0 0 / 0.55)");
      ctx.fillStyle = vignette;
      ctx.fillRect(0, 0, width, height);
    },
  };
}

export const kaleidoscope: Visualization = {
  id: "kaleidoscope",
  get name() {
    return t("viz.kaleidoscope.name");
  },
  get description() {
    return t("viz.kaleidoscope.description");
  },
  create,
};
