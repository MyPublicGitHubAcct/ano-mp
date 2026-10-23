// The settings store (`state/settings.svelte.ts`): loading, following
// `settings-changed`, saving a copy, a refused save, resetting a section,
// and carrying over the visualizer's old per-viewer choices.

import { describe, expect, test } from "vitest";
import { errorText } from "$lib/i18n";
import { fakeBackend, settle } from "./backend";
import { settingsFixtures } from "./fixtures";

const { get, refused } = settingsFixtures;

/** The store, loaded from `settings_get`, saving whatever it sends. */
async function loaded() {
  const backend = fakeBackend()
    .answer("settingsGet", get)
    .answer("settingsSave", ({ settings }) => settings);
  const { appSettings } = await import("$lib/state/settings.svelte");
  const { toasts } = await import("$lib/state/toasts.svelte");
  await appSettings.load();
  return { backend, appSettings, toasts };
}

describe("loading", () => {
  test("reads the settings and their defaults, and saves nothing", async () => {
    const { backend, appSettings } = await loaded();
    expect(appSettings.current).toEqual(get.settings);
    expect(appSettings.defaults).toEqual(get.defaults);
    expect(appSettings.visualizer.visualization).toBe(get.settings.visualizer.visualization);
    expect(appSettings.display).toEqual(get.settings.display);
    expect(backend.argsOf("settingsSave")).toEqual([]);
  });

  test("the settings can't be read before they are loaded", async () => {
    fakeBackend();
    const { appSettings } = await import("$lib/state/settings.svelte");
    expect(() => appSettings.current).toThrow();
  });

  test("the visualizer's old per-viewer choices are saved as settings once, then forgotten", async () => {
    localStorage.setItem("anomp.visualization", JSON.stringify("bars"));
    localStorage.setItem("anomp.coverBasis", JSON.stringify("artist"));
    const { backend, appSettings } = await loaded();
    await settle();
    const [{ settings }] = backend.argsOf("settingsSave");
    expect(settings.visualizer).toMatchObject({ visualization: "bars", coverBasis: "artist" });
    expect(appSettings.visualizer.visualization).toBe("bars");
    expect(localStorage.getItem("anomp.visualization")).toBeNull();
    expect(localStorage.getItem("anomp.coverBasis")).toBeNull();
  });
});

describe("saving", () => {
  test("sends the edited copy and takes what the backend saved", async () => {
    const { backend, appSettings } = await loaded();
    const before = appSettings.current;
    const saving = appSettings.save((next) => (next.features.themes = !before.features.themes));
    expect(appSettings.saving).toBe(true);
    expect(appSettings.current).toBe(before);
    expect(await saving).toBe(true);
    expect(appSettings.saving).toBe(false);
    const [{ settings }] = backend.argsOf("settingsSave");
    expect(settings).toEqual({ ...before, features: { ...before.features, themes: !before.features.themes } });
    expect(appSettings.current).toEqual(settings);
    expect(before.features.themes).toBe(get.settings.features.themes);
  });

  test("a refused save shows the backend's error and keeps the settings", async () => {
    const { backend, appSettings, toasts } = await loaded();
    backend.fail("settingsSave", refused);
    const before = appSettings.current;
    expect(await appSettings.save((next) => (next.display.trackColumns = ["year", "year"]))).toBe(false);
    expect(appSettings.current).toBe(before);
    expect(appSettings.saving).toBe(false);
    expect(toasts.list.map((toast) => [toast.text, toast.kind])).toEqual([[errorText(refused), "error"]]);
  });

  test("reset puts one section back to its defaults", async () => {
    const { backend, appSettings } = await loaded();
    expect(get.settings.visualizer).not.toEqual(get.defaults.visualizer);
    expect(await appSettings.reset("visualizer")).toBe(true);
    const [{ settings }] = backend.argsOf("settingsSave");
    expect(settings.visualizer).toEqual(get.defaults.visualizer);
    expect(settings.features).toEqual(get.settings.features);
  });
});

test("follows settings saved elsewhere until stopped", async () => {
  const { backend, appSettings } = await loaded();
  const stop = appSettings.connect();
  await settle();
  const elsewhere = structuredClone(get.settings);
  elsewhere.features.themes = !elsewhere.features.themes;
  await backend.emit("settings-changed", elsewhere);
  expect(appSettings.current).toEqual(elsewhere);

  stop();
  await settle();
  await backend.emit("settings-changed", get.settings);
  expect(appSettings.current).toEqual(elsewhere);
});
