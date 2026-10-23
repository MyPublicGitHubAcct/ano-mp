// The optional features (`state/features.svelte.ts`): which are on, as the
// settings say and as what reads them sees it change; the analysis's
// progress; and the waveforms, cached until `analysis-changed` names them.

import { describe, expect, test } from "vitest";
import type { AnalysisProgress } from "$lib/api";
import { fakeBackend, settle } from "./backend";
import { settingsFixtures } from "./fixtures";

const { get } = settingsFixtures;

const idle: AnalysisProgress = { running: false, done: 0, remaining: 0, failed: 0 };

async function connected(progress: AnalysisProgress = idle) {
  const backend = fakeBackend().answer("settingsGet", get).answer("analysisStatus", progress);
  const { appSettings } = await import("$lib/state/settings.svelte");
  const { features } = await import("$lib/state/features.svelte");
  await appSettings.load();
  appSettings.connect();
  const stop = features.connect();
  await settle();
  return { backend, features, stop };
}

describe("switches", () => {
  test("are the settings'", async () => {
    const { features } = await connected();
    expect(features.on).toEqual(get.settings.features);
    expect(features.on.waveformSeekBar).toBe(true);
  });

  test("turned off elsewhere, hide what reads them", async () => {
    const { backend, features } = await connected();
    const { watch, flush } = await import("./reactive.svelte");
    let shown = false;
    const seekBar = watch(() => (shown = features.on.waveformSeekBar));
    expect(shown).toBe(true);

    const off = structuredClone(get.settings);
    off.features.waveformSeekBar = false;
    await backend.emit("settings-changed", off);
    flush();
    expect([shown, seekBar.runs]).toEqual([false, 2]);
    seekBar.stop();
  });
});

describe("the analysis", () => {
  test("shows its progress, from the start and as it goes", async () => {
    const running = { running: true, done: 3, remaining: 7, failed: 0 };
    const { backend, features } = await connected(running);
    expect(features.analysis).toEqual(running);
    await backend.emit("analysis-progress", { ...running, done: 10, remaining: 0, running: false });
    expect(features.analysis.done).toBe(10);
  });

  test("history and preference changes count up, so views reload", async () => {
    const { backend, features } = await connected();
    await backend.emit("history-changed", null);
    await backend.emit("library-prefs-changed", null);
    await backend.emit("library-prefs-changed", null);
    expect([features.historyVersion, features.prefsVersion]).toEqual([1, 2]);
  });
});

describe("waveforms", () => {
  test("are asked for once per track", async () => {
    const { backend, features } = await connected();
    backend.answer("analysisWaveform", ({ trackId }) => [trackId, -trackId]);
    expect(await features.waveform(5)).toEqual([5, -5]);
    expect(await features.waveform(5)).toEqual([5, -5]);
    expect(await features.waveform(6)).toEqual([6, -6]);
    expect(backend.argsOf("analysisWaveform")).toEqual([{ trackId: 5 }, { trackId: 6 }]);
  });

  test("not analysed yet, or failing, are asked for again next time", async () => {
    const { backend, features } = await connected();
    backend.answer("analysisWaveform", null);
    expect(await features.waveform(5)).toBeNull();
    backend.fail("analysisWaveform", "The library is not available");
    expect(await features.waveform(5)).toBeNull();
    await settle();
    backend.answer("analysisWaveform", [1, -1]);
    expect(await features.waveform(5)).toEqual([1, -1]);
    expect(backend.argsOf("analysisWaveform")).toHaveLength(3);
  });

  test("are dropped when the track is analysed again", async () => {
    const { backend, features } = await connected();
    backend.answer("analysisWaveform", [1, -1]);
    await features.waveform(5);
    await features.waveform(6);
    await backend.emit("analysis-changed", [5]);
    expect(features.analysisVersion).toBe(1);
    await features.waveform(5);
    await features.waveform(6);
    expect(backend.argsOf("analysisWaveform")).toEqual([{ trackId: 5 }, { trackId: 6 }, { trackId: 5 }]);
  });
});
