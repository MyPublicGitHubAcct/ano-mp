// The spectral freeze's Hold (PLAN.md X2), shared by the now-playing bar
// and Settings › Effects. The core lets go of a held freeze by itself when
// a track takes over or another loads, so while it holds, and whenever the
// track changes, the store asks the engine again.

import { effects as api } from "$lib/api";
import { appSettings } from "./settings.svelte";
import { attempt } from "./toasts.svelte";

/** How often to check that a held freeze still holds. */
const HELD_POLL_MS = 1000;

class EffectsStore {
  /** The freeze holds the sound. */
  held = $state(false);
  #timer: ReturnType<typeof setInterval> | null = null;

  /** Whether the freeze can be held: effects on, and the freeze among them. */
  get freezeOn() {
    const settings = appSettings.current;
    return settings.features.effects && settings.effects.freeze.enabled;
  }

  /** Asks the engine whether the freeze still holds. */
  async refresh() {
    const status = await api.status().catch(() => null);
    if (status) this.#set(status.freezeHeld);
  }

  /** Holds the sound playing now, or lets it go. */
  async toggleHold() {
    const holds = await attempt(() => api.freeze(!this.held));
    if (holds !== undefined) this.#set(holds);
  }

  #set(held: boolean) {
    this.held = held;
    if (held && this.#timer === null) {
      this.#timer = setInterval(() => void this.refresh(), HELD_POLL_MS);
    } else if (!held && this.#timer !== null) {
      clearInterval(this.#timer);
      this.#timer = null;
    }
  }
}

export const effects = new EffectsStore();
