// The optional features (PLAN.md §4.6): which are on (from the settings),
// the loudness analysis's progress, and waveforms for the seek bar, cached
// per track and dropped when `analysis-changed` names the track.

import { features as api, on, onAll, type AnalysisProgress } from "$lib/api";
import { appSettings } from "./settings.svelte";

class FeaturesStore {
  analysis = $state.raw<AnalysisProgress>({ running: false, done: 0, remaining: 0, failed: 0 });
  /** Increases whenever tracks are analysed, so views depending on it reload. */
  analysisVersion = $state(0);
  /** Increases when a play is recorded or preferences change. */
  historyVersion = $state(0);
  prefsVersion = $state(0);
  #waveforms = new Map<number, Promise<number[] | null>>();

  /** Which features are on. */
  get on() {
    return appSettings.current.features;
  }

  connect() {
    const stop = onAll([
      on("analysis-progress", (progress) => (this.analysis = progress)),
      on("analysis-changed", (ids) => {
        for (const id of ids) this.#waveforms.delete(id);
        this.analysisVersion++;
      }),
      on("history-changed", () => this.historyVersion++),
      on("library-prefs-changed", () => this.prefsVersion++),
    ]);
    api.analysisStatus().then(
      (progress) => (this.analysis = progress),
      () => {},
    );
    return stop;
  }

  /** The waveform of a track: (min, max) pairs, -127..127; null until analysed. */
  waveform(trackId: number): Promise<number[] | null> {
    let cached = this.#waveforms.get(trackId);
    if (!cached) {
      cached = api.waveform(trackId).catch(() => null);
      this.#waveforms.set(trackId, cached);
      // Not analysed yet: ask again next time.
      cached.then((waveform) => {
        if (waveform === null) this.#waveforms.delete(trackId);
      });
    }
    return cached;
  }
}

export const features = new FeaturesStore();
