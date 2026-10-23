// The effects workbench (`state/workbench.svelte.ts`, X8): opening a file
// once at a time, Play by where the file is in the queue, and a take that
// ends when the recording stops. (`lib/workbench.ts`'s rules have their own
// tests; choosing a file opens the system's dialog and isn't tested here.)

import { describe, expect, test } from "vitest";
import type { WorkbenchFile } from "$lib/api";
import { t } from "$lib/i18n";
import { fakeBackend, settle } from "./backend";
import { queueFixtures, settingsFixtures } from "./fixtures";

const { full, advanced } = queueFixtures;

const file = (uid: number): WorkbenchFile => ({
  uid,
  trackId: -uid,
  title: "Chirp",
  artist: null,
  fileName: "chirp.flac",
  format: "FLAC",
  duration: 1.5,
  sampleRate: 44100,
  channels: 2,
  bitrateKbps: null,
});

async function loaded() {
  const backend = fakeBackend().answer("settingsGet", settingsFixtures.get);
  const { appSettings } = await import("$lib/state/settings.svelte");
  const { player } = await import("$lib/state/player.svelte");
  const { workbench } = await import("$lib/state/workbench.svelte");
  const { toasts } = await import("$lib/state/toasts.svelte");
  await appSettings.load();
  player.apply(full);
  return { backend, player, workbench, toasts };
}

describe("the file", () => {
  test("opens once at a time, and is the queue's current item when it plays", async () => {
    const { backend, player, workbench } = await loaded();
    const opened = file(full.currentItem?.uid ?? 0);
    backend.answer("workbenchOpen", () => new Promise((resolve) => setTimeout(() => resolve(opened), 5)));
    const opening = workbench.open("/Users/me/chirp.flac");
    await workbench.open("/Users/me/other.flac");
    await opening;
    expect(backend.argsOf("workbenchOpen")).toEqual([{ path: "/Users/me/chirp.flac" }]);
    expect([workbench.file, workbench.current, workbench.opening]).toEqual([opened, true, false]);

    player.apply(advanced);
    expect(workbench.current).toBe(false);
  });

  test("Play toggles the current file, jumps to a queued one, or opens it again", async () => {
    const { backend, workbench } = await loaded();
    backend.answer("queueToggle", undefined).answer("queueJump", undefined);
    backend.answer("workbenchCurrent", file(1));
    await workbench.useCurrent();
    await workbench.play();
    expect(backend.argsOf("queueToggle")).toHaveLength(1);

    backend.answer("workbenchCurrent", file(3));
    await workbench.useCurrent();
    await workbench.play();
    expect(backend.argsOf("queueJump")).toEqual([{ uid: 3 }]);

    backend.answer("workbenchCurrent", file(99)).answer("workbenchReopen", file(100));
    await workbench.useCurrent();
    await workbench.play();
    expect(backend.argsOf("workbenchReopen")).toEqual([{ trackId: -99 }]);
    expect(workbench.file?.uid).toBe(100);
  });
});

test("a take is saved when its recording stops", async () => {
  const { backend, workbench, toasts } = await loaded();
  backend.answer("workbenchCurrent", file(1));
  await workbench.useCurrent();
  workbench.taking = true;
  workbench.recordingChanged(true, "take.flac");
  expect(workbench.saved).toBeNull();
  workbench.recordingChanged(false, "take.flac");
  await settle();
  expect([workbench.taking, workbench.saved]).toEqual([false, "take.flac"]);
  expect(toasts.list.map((toast) => toast.text)).toEqual([t("recording.saved", { files: "take.flac" })]);
});
