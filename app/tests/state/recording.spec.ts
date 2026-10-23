// Recording (`state/recording.svelte.ts`, X6): starting and stopping,
// polling its length while it runs, and the backend's events. (Choosing
// a folder opens the system's dialog and isn't tested here.)

import { describe, expect, test, vi } from "vitest";
import type { RecordingState } from "$lib/api";
import { errorText, t } from "$lib/i18n";
import { coded, fakeBackend, settle } from "./backend";
import { serverFixtures } from "./fixtures";

const idle: RecordingState = {
  recording: false,
  seconds: 0,
  overruns: 0,
  fileName: null,
  folder: "/Users/me/Music/Recordings",
  formats: ["flac", "wav"],
};
const running: RecordingState = { ...idle, recording: true, seconds: 1, fileName: "2026-10-23 17.04.12.flac" };

async function connected(state: RecordingState = idle) {
  const backend = fakeBackend().answer("recordingStatus", state);
  const { recording } = await import("$lib/state/recording.svelte");
  const { toasts } = await import("$lib/state/toasts.svelte");
  const stop = recording.connect();
  await settle();
  return { backend, recording, toasts, stop };
}

test("knows which formats this build writes", async () => {
  const { recording } = await connected();
  expect([recording.available("flac"), recording.available("mp3")]).toEqual([true, false]);
});

describe("toggle", () => {
  test("starts, then stops and says where the files went", async () => {
    const { backend, recording, toasts } = await connected();
    backend.answer("recordingStart", running);
    await recording.toggle();
    expect(recording.recording).toBe(true);

    backend.answer("recordingStop", { files: ["a.flac", "b.flac"], seconds: 9, overruns: 2, error: null });
    await recording.toggle();
    expect(toasts.list.map((toast) => toast.text)).toEqual([
      t("recording.saved", { files: "a.flac, b.flac" }),
      t("recording.overruns", { count: 2 }),
    ]);
    expect(recording.busy).toBe(false);
  });

  test("one start or stop at a time", async () => {
    const { backend, recording } = await connected();
    backend.answer("recordingStart", running);
    await Promise.all([recording.toggle(), recording.toggle()]);
    expect(backend.argsOf("recordingStart")).toHaveLength(1);
  });

  test("a refused start shows why", async () => {
    const { backend, recording, toasts } = await connected();
    const off = coded("featureOff", { feature: "recording", name: "Recording" });
    backend.fail("recordingStart", off);
    await recording.toggle();
    expect(recording.recording).toBe(false);
    expect(toasts.list.map((toast) => toast.text)).toEqual([t("error.featureOff", { name: "Recording" })]);
  });
});

describe("while a recording runs", () => {
  test("its length is asked for every second, until it stops", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    const { backend, recording } = await connected(running);
    backend.answer("recordingStatus", { ...running, seconds: 2 });
    vi.advanceTimersByTime(1000);
    await settle();
    expect(recording.state.seconds).toBe(2);

    await backend.emit("recording", { ...idle, folder: null });
    vi.advanceTimersByTime(5000);
    await settle();
    expect(backend.argsOf("recordingStatus")).toHaveLength(2);
    // The event doesn't say the folder: it stays.
    expect(recording.state.folder).toBe(idle.folder);
  });

  test("a recording stopped by an error shows it", async () => {
    const { backend, toasts } = await connected();
    const error = serverFixtures.jellyfinOff;
    await backend.emit("recording-failed", { files: [], seconds: 3, overruns: 0, error });
    await backend.emit("recording-failed", { files: ["a.flac"], seconds: 3, overruns: 0, error: null });
    expect(toasts.list.map((toast) => toast.text)).toEqual([errorText(error)]);
  });
});
