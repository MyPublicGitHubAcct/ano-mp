// The visualizer: which visualization and cover wall (from the settings),
// what it shows under the track's name, and full screen.

import { getCurrentWindow } from "@tauri-apps/api/window";
import type { CoverBasis } from "$lib/api";
import { VISUALIZATIONS } from "$lib/visualizer";
import { appSettings } from "./settings.svelte";
import { attempt } from "./toasts.svelte";

class VisualizerStore {
  get choice() {
    return appSettings.visualizer.visualization;
  }

  get coverBasis(): CoverBasis {
    return appSettings.visualizer.coverBasis;
  }

  /** A line from the visualization, e.g. which albums the cover wall shows. */
  caption = $state<string | null>(null);
  /** The window is full screen, showing the visualizer alone. */
  fullscreen = $state(false);

  choose(id: string) {
    if (id !== this.choice) void appSettings.save((next) => (next.visualizer.visualization = id));
  }

  /** The next (or with -1, previous) visualization. */
  cycle(step: 1 | -1) {
    const index = VISUALIZATIONS.findIndex((candidate) => candidate.id === this.choice);
    this.choose(VISUALIZATIONS[(index + step + VISUALIZATIONS.length) % VISUALIZATIONS.length].id);
  }

  setCoverBasis(basis: CoverBasis) {
    if (basis !== this.coverBasis) void appSettings.save((next) => (next.visualizer.coverBasis = basis));
  }

  /** Renderers call this every frame; it changes state only when the text does. */
  setCaption(caption: string | null) {
    if (caption !== this.caption) this.caption = caption;
  }

  setFullscreen(fullscreen: boolean) {
    return attempt(async () => {
      await getCurrentWindow().setFullscreen(fullscreen);
      this.fullscreen = fullscreen;
    });
  }

  /** Follows the window leaving full screen some other way (the green button); returns a function that stops. */
  connect() {
    const window = getCurrentWindow();
    const listener = window.onResized(async () => {
      const fullscreen = await window.isFullscreen();
      if (fullscreen !== this.fullscreen) this.fullscreen = fullscreen;
    });
    return () => void listener.then((stop) => stop());
  }
}

export const visualizer = new VisualizerStore();
