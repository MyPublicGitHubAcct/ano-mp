// The theme on screen (PLAN.md X1): the settings' theme (the default one
// while themes are turned off), under the system's light or dark and its
// request for more contrast, with the accent from the current cover when
// the theme asks for it. The root layout keeps `:root`'s custom properties
// in step in every window; the theme editor can show a theme before it's
// saved (`preview`).

import type { Theme } from "$lib/api";
import { DEFAULT_THEME, rowHeight, tokens, type Rgb } from "$lib/theme";
import { loadCover } from "$lib/visualizer/palette";
import { library } from "./library.svelte";
import { player } from "./player.svelte";
import { appSettings } from "./settings.svelte";

const DARK = "(prefers-color-scheme: dark)";
const MORE_CONTRAST = "(prefers-contrast: more)";

const matches = (query: string) => window.matchMedia(query).matches;

class AppearanceStore {
  systemDark = $state(matches(DARK));
  systemMoreContrast = $state(matches(MORE_CONTRAST));
  /** A theme being edited, shown instead of the saved one until it's saved or dropped. */
  preview = $state.raw<Theme | null>(null);
  /** The current cover's main colour, while the theme takes its accent from it. */
  cover = $state.raw<Rgb | null>(null);

  /** The theme the user chose, or the default while themes are off. */
  get chosen(): Theme {
    const settings = appSettings.current;
    return settings.features.themes ? settings.appearance.theme : DEFAULT_THEME;
  }

  /** The theme on screen. */
  get theme(): Theme {
    return this.preview ?? this.chosen;
  }

  /** The system asks for more contrast, and the user lets that change the colours. */
  get moreContrast(): boolean {
    const settings = appSettings.current;
    return this.systemMoreContrast && (settings.appearance.followSystemContrast || !settings.features.themes);
  }

  /** A list row's height under the theme, from the height it was drawn at. */
  rowHeight(base: number): number {
    return rowHeight(base, this.theme);
  }

  /** The custom properties (names without the `--`) and colour scheme on screen. */
  current = $derived(
    tokens(this.theme, { systemDark: this.systemDark, moreContrast: this.moreContrast, cover: this.cover }),
  );

  /** Sets `:root`'s custom properties and colour scheme for the theme now. */
  apply() {
    const { scheme, props } = this.current;
    const style = document.documentElement.style;
    for (const [name, value] of Object.entries(props)) style.setProperty(`--${name}`, value);
    style.colorScheme = scheme;
  }

  /** Follows the system, the cover and the settings; returns a function that stops. */
  connect() {
    return $effect.root(() => {
      $effect(() => {
        const dark = window.matchMedia(DARK);
        const contrast = window.matchMedia(MORE_CONTRAST);
        const changed = () => {
          this.systemDark = dark.matches;
          this.systemMoreContrast = contrast.matches;
        };
        dark.addEventListener("change", changed);
        contrast.addEventListener("change", changed);
        return () => {
          dark.removeEventListener("change", changed);
          contrast.removeEventListener("change", changed);
        };
      });

      $effect(() => {
        const item = player.currentItem;
        if (!this.theme.accentFromCover || item === null) {
          this.cover = null;
          return;
        }
        return loadCover(library.coverUrl(item), (_image, palette) => {
          this.cover = palette?.fromCover ? palette.colors[0] : null;
        });
      });

      $effect(() => this.apply());
    });
  }
}

export const appearance = new AppearanceStore();
