// Rhythm rings: time wound into a clock of one bar (four beats) a turn,
// each bar a ring of its own, the newest outermost. The onsets are marked
// where they fall, so a rhythm that repeats lines up from ring to ring as
// spokes, a syncopation shows off the beat lines, and a fill breaks the
// pattern. The tempo comes from the onsets (`TempoTracker`); until there is
// one, the clock waits.

import { t } from "$lib/i18n";
import { TempoTracker } from "../music";
import type { Renderer, Scene, Visualization } from "../types";
import { along, approach, clearStage, rgba } from "../util";

const BEATS_PER_BAR = 4;
const SLOTS = 64;
const BARS = 14;

function create(): Renderer {
  const tempo = new TempoTracker();
  /** Onset strength per slot, by bar number. */
  const bars = new Map<number, Float32Array>();
  let trackId: number | null = null;
  let waiting = 0;
  let onset = 0;

  return {
    draw(scene: Scene) {
      const { ctx, width, height, dt, time, frame, palette } = scene;
      const track = scene.track?.trackId ?? null;
      if (track !== trackId) {
        trackId = track;
        tempo.reset();
        bars.clear();
      }
      if (scene.fresh && !frame.silent) tempo.add(time, frame.onset);
      onset = approach(onset, frame.silent ? 0 : frame.onset, dt, 0.01, 0.15);

      const position = frame.silent ? null : tempo.position(time, BEATS_PER_BAR);
      if (position !== null) {
        let bar = bars.get(position.bar);
        if (!bar) {
          bar = new Float32Array(SLOTS);
          bars.set(position.bar, bar);
          for (const number of bars.keys()) if (number <= position.bar - BARS) bars.delete(number);
        }
        const slot = Math.min(SLOTS - 1, Math.floor(position.phase * SLOTS));
        bar[slot] = Math.max(bar[slot], Math.min(1, frame.onset * 1.6));
      }

      clearStage(scene);
      const cx = width / 2;
      const cy = height / 2;
      const outer = Math.min(width, height) * 0.42;
      const inner = outer * 0.22;
      const spacing = (outer - inner) / BARS;
      ctx.save();

      // The beat lines.
      ctx.strokeStyle = rgba([255, 255, 255], 0.1);
      ctx.lineWidth = 1;
      ctx.beginPath();
      for (let beat = 0; beat < BEATS_PER_BAR; beat++) {
        const angle = -Math.PI / 2 + (beat / BEATS_PER_BAR) * 2 * Math.PI;
        ctx.moveTo(cx + Math.cos(angle) * inner * 0.8, cy + Math.sin(angle) * inner * 0.8);
        ctx.lineTo(cx + Math.cos(angle) * outer * 1.03, cy + Math.sin(angle) * outer * 1.03);
      }
      ctx.stroke();

      if (position === null) {
        // Waiting for a tempo: a ring that breathes with the onsets.
        waiting += dt;
        ctx.strokeStyle = rgba(palette.colors[0], 0.25 + 0.6 * onset);
        ctx.lineWidth = 2 + 6 * onset;
        ctx.beginPath();
        ctx.arc(cx, cy, inner + (outer - inner) * (0.5 + 0.1 * Math.sin(waiting * 1.5)), 0, 2 * Math.PI);
        ctx.stroke();
        ctx.fillStyle = rgba([255, 255, 255], 0.45);
        ctx.font = `${Math.max(12, outer * 0.06)}px system-ui, sans-serif`;
        ctx.textAlign = "center";
        ctx.textBaseline = "middle";
        ctx.fillText(t("rhythm.listening"), cx, cy);
        ctx.restore();
        return;
      }
      waiting = 0;

      // Each bar's ring, the newest outermost and the oldest fading inwards.
      const slotAngle = (2 * Math.PI) / SLOTS;
      ctx.lineCap = "butt";
      for (const [number, bar] of bars) {
        const age = position.bar - number;
        if (age < 0 || age >= BARS) continue;
        const radius = outer - age * spacing - spacing / 2;
        const fade = 1 - age / BARS;
        ctx.lineWidth = spacing * 0.78;
        for (let slot = 0; slot < SLOTS; slot++) {
          const value = bar[slot];
          if (value < 0.04) continue;
          const start = -Math.PI / 2 + slot * slotAngle;
          ctx.strokeStyle = rgba(along(palette, slot / SLOTS), Math.min(1, value) * fade);
          ctx.beginPath();
          ctx.arc(cx, cy, radius, start, start + slotAngle * 0.9);
          ctx.stroke();
        }
      }

      // The hand, on the newest ring.
      const angle = -Math.PI / 2 + position.phase * 2 * Math.PI;
      ctx.strokeStyle = rgba([255, 255, 255], 0.75);
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.moveTo(cx + Math.cos(angle) * inner, cy + Math.sin(angle) * inner);
      ctx.lineTo(cx + Math.cos(angle) * outer * 1.03, cy + Math.sin(angle) * outer * 1.03);
      ctx.stroke();

      // The tempo in the middle.
      if (tempo.period !== null) {
        ctx.fillStyle = rgba([255, 255, 255], 0.8);
        ctx.font = `600 ${Math.max(13, inner * 0.32)}px system-ui, sans-serif`;
        ctx.textAlign = "center";
        ctx.textBaseline = "middle";
        ctx.fillText(t("rhythm.bpm", { bpm: Math.round(60 / tempo.period) }), cx, cy);
      }
      ctx.restore();
    },
  };
}

export const rhythm: Visualization = {
  id: "rhythm",
  get name() {
    return t("viz.rhythm.name");
  },
  get description() {
    return t("viz.rhythm.description");
  },
  create,
};
