// The app's settings (`settings.rs`): read before the first page renders
// (`routes/+layout.ts`), kept in step with `settings-changed`, and saved
// whole after each change.

import { on, settings as api, type AppSettings } from "$lib/api";
import { attempt } from "./toasts.svelte";

class SettingsStore {
  #current = $state.raw<AppSettings | null>(null);
  /** What each section is reset to. */
  defaults = $state.raw<AppSettings | null>(null);
  saving = $state(false);

  /** The settings as saved. Only read after `load`. */
  get current(): AppSettings {
    if (!this.#current) throw new Error("The settings haven't been read yet");
    return this.#current;
  }

  get display() {
    return this.current.display;
  }

  get visualizer() {
    return this.current.visualizer;
  }

  async load() {
    const { settings, defaults } = await api.get();
    this.#current = settings;
    this.defaults = defaults;
    carryOverLocalPreferences();
  }

  /** Follows changes saved elsewhere; returns a function that stops. */
  connect() {
    const listener = on("settings-changed", (settings) => (this.#current = settings));
    return () => void listener.then((stop) => stop());
  }

  /** Saves the settings with `edit` applied to a copy. Resolves to whether they were saved; a failure shows
      as a toast and leaves them as they were. */
  async save(edit: (next: AppSettings) => void): Promise<boolean> {
    const next = structuredClone(this.current);
    edit(next);
    this.saving = true;
    try {
      const saved = await attempt(() => api.save(next));
      if (saved) this.#current = saved;
      return saved !== undefined;
    } finally {
      this.saving = false;
    }
  }

  /** Puts one section back to its defaults. */
  reset(section: keyof AppSettings) {
    const defaults = this.defaults;
    if (!defaults) return Promise.resolve(false);
    return this.save((next) => {
      (next as Record<keyof AppSettings, unknown>)[section] = structuredClone(defaults[section]);
    });
  }
}

export const appSettings = new SettingsStore();

/** The visualizer's choices were per viewer (`localStorage`) before they were settings: carry them over
    once, then forget them. */
function carryOverLocalPreferences() {
  let visualization: string | null = null;
  let coverBasis: string | null = null;
  try {
    visualization = JSON.parse(localStorage.getItem("anomp.visualization") ?? "null");
    coverBasis = JSON.parse(localStorage.getItem("anomp.coverBasis") ?? "null");
    localStorage.removeItem("anomp.visualization");
    localStorage.removeItem("anomp.coverBasis");
  } catch {
    return; // No storage, or nothing readable in it.
  }
  if (typeof visualization !== "string" && coverBasis !== "year" && coverBasis !== "artist") return;
  void appSettings.save((next) => {
    if (typeof visualization === "string" && visualization !== "") next.visualizer.visualization = visualization;
    if (coverBasis === "year" || coverBasis === "artist") next.visualizer.coverBasis = coverBasis;
  });
}
