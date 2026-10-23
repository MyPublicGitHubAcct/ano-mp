// The visualizer (`state/visualizer.svelte.ts`): which visualization and
// cover wall, saved as settings only when they change, and the caption.
// (Full screen goes through the window's API and isn't tested here.)

import { expect, test } from "vitest";
import { VISUALIZATIONS } from "$lib/visualizer";
import { fakeBackend } from "./backend";
import { settingsFixtures } from "./fixtures";

async function loaded() {
  const backend = fakeBackend()
    .answer("settingsGet", settingsFixtures.get)
    .answer("settingsSave", ({ settings }) => settings);
  const { appSettings } = await import("$lib/state/settings.svelte");
  const { visualizer } = await import("$lib/state/visualizer.svelte");
  await appSettings.load();
  return { backend, visualizer };
}

const saved = (backend: Awaited<ReturnType<typeof loaded>>["backend"]) =>
  backend.argsOf("settingsSave").map(({ settings }) => settings.visualizer);

test("cycling goes round the visualizations, saving each choice", async () => {
  const { backend, visualizer } = await loaded();
  const index = VISUALIZATIONS.findIndex((candidate) => candidate.id === visualizer.choice);
  expect(index).toBeGreaterThanOrEqual(0);
  visualizer.cycle(-1);
  await Promise.resolve();
  const previous = VISUALIZATIONS[(index - 1 + VISUALIZATIONS.length) % VISUALIZATIONS.length].id;
  expect(saved(backend).map((settings) => settings.visualization)).toEqual([previous]);
});

test("choosing what is already chosen saves nothing", async () => {
  const { backend, visualizer } = await loaded();
  visualizer.choose(visualizer.choice);
  visualizer.setCoverBasis(visualizer.coverBasis);
  visualizer.setCoverBasis(visualizer.coverBasis === "year" ? "artist" : "year");
  await Promise.resolve();
  expect(saved(backend).map((settings) => settings.coverBasis)).toEqual([
    settingsFixtures.get.settings.visualizer.coverBasis === "year" ? "artist" : "year",
  ]);
});

test("the caption changes only when its text does", async () => {
  const { visualizer } = await loaded();
  const { watch, flush } = await import("./reactive.svelte");
  const caption = watch(() => visualizer.caption);
  for (const text of ["1994", "1994", null, null]) visualizer.setCaption(text);
  flush();
  expect(caption.runs).toBe(2);
  caption.stop();
});
