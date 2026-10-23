// The spectral freeze's Hold (`state/effects.svelte.ts`, X2): offered only
// while effects and the freeze are on, and checked every second while it
// holds, since the core lets go by itself.

import { describe, expect, test, vi } from "vitest";
import { fakeBackend, settle } from "./backend";
import { settingsFixtures } from "./fixtures";

const { get } = settingsFixtures;

async function loaded() {
  const backend = fakeBackend()
    .answer("settingsGet", get)
    .answer("settingsSave", ({ settings }) => settings);
  const { appSettings } = await import("$lib/state/settings.svelte");
  const { effects } = await import("$lib/state/effects.svelte");
  await appSettings.load();
  return { backend, appSettings, effects };
}

test("the freeze can be held only while effects and the freeze are on", async () => {
  const { appSettings, effects } = await loaded();
  expect([get.settings.features.effects, get.settings.effects.freeze.enabled]).toEqual([false, false]);
  expect(effects.freezeOn).toBe(false);
  await appSettings.save((next) => (next.effects.freeze.enabled = true));
  expect(effects.freezeOn).toBe(false);
  await appSettings.save((next) => (next.features.effects = true));
  expect(effects.freezeOn).toBe(true);
});

describe("holding", () => {
  test("is checked every second until the core lets go", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    const { backend, effects } = await loaded();
    backend.answer("effectsFreeze", ({ hold }) => hold).answer("effectsStatus", { freezeHeld: true });
    await effects.toggleHold();
    expect(backend.argsOf("effectsFreeze")).toEqual([{ hold: true }]);
    expect(effects.held).toBe(true);

    vi.advanceTimersByTime(1000);
    await settle();
    backend.answer("effectsStatus", { freezeHeld: false });
    vi.advanceTimersByTime(1000);
    await settle();
    expect(effects.held).toBe(false);
    vi.advanceTimersByTime(5000);
    await settle();
    expect(backend.argsOf("effectsStatus")).toHaveLength(2);
  });

  test("let go by hand stops checking", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    const { backend, effects } = await loaded();
    backend.answer("effectsFreeze", ({ hold }) => hold);
    await effects.toggleHold();
    await effects.toggleHold();
    expect(backend.argsOf("effectsFreeze")).toEqual([{ hold: true }, { hold: false }]);
    vi.advanceTimersByTime(5000);
    await settle();
    expect(backend.argsOf("effectsStatus")).toEqual([]);
  });
});
