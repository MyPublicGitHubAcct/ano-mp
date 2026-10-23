// The theme on screen (`state/appearance.svelte.ts`, X1): the settings'
// theme or the default while themes are off, a preview over it, the
// system's request for more contrast, and `:root` kept in step with the
// system's light or dark. jsdom has no `matchMedia`; `media` stands in.

import { describe, expect, test } from "vitest";
import { BUILT_IN, DEFAULT_THEME, tokens } from "$lib/theme";
import { fakeBackend } from "./backend";
import { settingsFixtures } from "./fixtures";

const DARK = "(prefers-color-scheme: dark)";
const MORE_CONTRAST = "(prefers-contrast: more)";

/** A system whose media queries match as `matches` says, and can change. */
function media(matches: Record<string, boolean>) {
  const listeners = new Set<() => void>();
  window.matchMedia = (query: string) =>
    ({
      get matches() {
        return matches[query] ?? false;
      },
      addEventListener: (_: string, listener: () => void) => listeners.add(listener),
      removeEventListener: (_: string, listener: () => void) => listeners.delete(listener),
    }) as unknown as MediaQueryList;
  return {
    set(query: string, value: boolean) {
      matches[query] = value;
      for (const listener of listeners) listener();
    },
  };
}

const dark = BUILT_IN.find((theme) => theme.scheme === "dark")!;

async function loaded(system: Record<string, boolean> = {}) {
  const changes = media(system);
  const backend = fakeBackend()
    .answer("settingsGet", settingsFixtures.get)
    .answer("settingsSave", ({ settings }) => settings);
  const { appSettings } = await import("$lib/state/settings.svelte");
  const { appearance } = await import("$lib/state/appearance.svelte");
  await appSettings.load();
  return { backend, appSettings, appearance, changes };
}

describe("the theme", () => {
  test("is the settings', or the default while themes are off", async () => {
    const { appSettings, appearance } = await loaded();
    await appSettings.save((next) => (next.appearance.theme = dark));
    expect(appearance.theme).toEqual(dark);
    await appSettings.save((next) => (next.features.themes = false));
    expect(appearance.theme).toEqual(DEFAULT_THEME);
  });

  test("a preview shows over it until dropped", async () => {
    const { appearance } = await loaded();
    appearance.preview = dark;
    expect(appearance.current).toEqual(tokens(dark, { systemDark: false, moreContrast: false, cover: null }));
    appearance.preview = null;
    expect(appearance.theme).toEqual(appearance.chosen);
  });

  test("the system's request for more contrast counts when the user lets it, or themes are off", async () => {
    const { appSettings, appearance } = await loaded({ [MORE_CONTRAST]: true });
    expect(appearance.moreContrast).toBe(settingsFixtures.get.settings.appearance.followSystemContrast);
    await appSettings.save((next) => (next.appearance.followSystemContrast = false));
    expect(appearance.moreContrast).toBe(false);
    await appSettings.save((next) => (next.features.themes = false));
    expect(appearance.moreContrast).toBe(true);
  });
});

test("`:root` follows the system's light and dark until stopped", async () => {
  const { appearance, changes } = await loaded({ [DARK]: false });
  const { flush } = await import("./reactive.svelte");
  const style = document.documentElement.style;
  const stop = appearance.connect();
  flush();
  const light = tokens(appearance.theme, { systemDark: false, moreContrast: false, cover: null });
  expect(style.colorScheme).toBe(light.scheme);
  expect(style.getPropertyValue("--bg")).toBe(light.props.bg);

  changes.set(DARK, true);
  flush();
  const darkTokens = tokens(appearance.theme, { systemDark: true, moreContrast: false, cover: null });
  expect(appearance.theme.scheme).toBe("system");
  expect(style.colorScheme).toBe(darkTokens.scheme);
  expect(style.getPropertyValue("--bg")).toBe(darkTokens.props.bg);

  stop();
  changes.set(DARK, false);
  flush();
  expect(style.colorScheme).toBe(darkTokens.scheme);
});
