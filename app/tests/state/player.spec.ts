// The queue and transport (`state/player.svelte.ts`) and the position
// (`state/position.svelte.ts`): the backend's events applied, the list
// kept by numbered edits (PLAN.md H16), the whole state asked for when an
// edit is missed or doesn't fit, and the commands the controls send.

import { describe, expect, test } from "vitest";
import type { PlayerStatus, QueueState } from "$lib/api";
import { errorText, t } from "$lib/i18n";
import { coded, fakeBackend, settle } from "./backend";
import { queueFixtures } from "./fixtures";

const { full, removed, added, advanced, resynced, skipped } = queueFixtures;

const status: PlayerStatus = { state: "paused", position: 12.5, duration: 60, volume: 0.5 };

const uids = (items: { uid: number }[]) => items.map((item) => item.uid);

/** The store, connected; `queue_state` answers `state` (by default, the whole list at version 1). */
async function connected(state: QueueState | Promise<QueueState> = full) {
  const backend = fakeBackend()
    .answer("playerStatus", status)
    .answer("queueState", () => state);
  const { player } = await import("$lib/state/player.svelte");
  const { playback } = await import("$lib/state/position.svelte");
  const { toasts } = await import("$lib/state/toasts.svelte");
  const stop = player.connect();
  await settle();
  return { backend, player, playback, toasts, stop };
}

describe("connecting", () => {
  test("takes the transport and the whole queue", async () => {
    const { player, playback } = await connected();
    expect(player.state).toBe(status.state);
    expect(player.volume).toBe(status.volume);
    expect(playback).toEqual({ position: status.position, duration: status.duration });
    expect(uids(player.items)).toEqual(uids(full.items ?? []));
    expect(player.current).toBe(full.current);
    expect(player.currentItem).toEqual(full.currentItem);
    expect(player.hasNext).toBe(full.hasNext);
    expect(player.loaded).toBe(full.loaded);
  });

  test("an answer the events overtook doesn't undo them", async () => {
    let answer: (state: QueueState) => void = () => {};
    const late = new Promise<QueueState>((resolve) => (answer = resolve));
    const { backend, player } = await connected(late);
    for (const state of [full, removed, added, advanced]) await backend.emit("queue-changed", state);
    expect(player.currentItem?.uid).toBe(advanced.currentItem?.uid);

    // `full` was taken before the events were sent.
    answer(full);
    await settle();
    expect(player.current).toBe(advanced.current);
    expect(player.currentItem?.uid).toBe(advanced.currentItem?.uid);
    expect(uids(player.items)).toEqual(uids(resynced.items ?? []));
  });
});

describe("queue-changed", () => {
  test("applies each version's edits to the list", async () => {
    const { backend, player } = await connected();
    await backend.emit("queue-changed", removed);
    expect(uids(player.items)).toEqual([1, 3]);
    await backend.emit("queue-changed", added);
    expect(uids(player.items)).toEqual(uids(resynced.items ?? []));
    expect(backend.argsOf("queueState")).toHaveLength(1);
  });

  test("a missed version asks for the whole state", async () => {
    const { backend, player } = await connected();
    backend.answer("queueState", resynced);
    await backend.emit("queue-changed", added);
    await settle();
    expect(backend.argsOf("queueState")).toHaveLength(2);
    expect(uids(player.items)).toEqual(uids(resynced.items ?? []));

    // Caught up: the next edit applies.
    await backend.emit("queue-changed", { ...removed, revision: 9, listVersion: 5 });
    expect(uids(player.items)).toEqual([1, 4]);
    expect(backend.argsOf("queueState")).toHaveLength(2);
  });

  test("an edit that doesn't fit the list asks for the whole state", async () => {
    const { backend, player } = await connected();
    backend.answer("queueState", resynced);
    await backend.emit("queue-changed", { ...removed, edits: [{ kind: "remove", at: 7, count: 1 }] });
    await settle();
    expect(backend.argsOf("queueState")).toHaveLength(2);
    expect(uids(player.items)).toEqual(uids(resynced.items ?? []));
  });

  test("a state already applied changes nothing", async () => {
    const { backend, player } = await connected();
    await backend.emit("queue-changed", removed);
    const items = player.items;
    await backend.emit("queue-changed", removed);
    expect(player.items).toBe(items);
  });

  test("a skipped track is shown with its error", async () => {
    const { backend, toasts } = await connected();
    await backend.emit("queue-changed", skipped);
    const [track] = skipped.skipped;
    expect(errorText(track.error)).toBe(t("error.openTimedOut", { seconds: 20 }));
    expect(toasts.list.map((toast) => toast.text)).toEqual([
      t("queue.skippedTrack", { title: track.title, error: errorText(track.error) }),
    ]);
  });

  test("stops following when stopped", async () => {
    const { backend, player, stop } = await connected();
    stop();
    await settle();
    await backend.emit("queue-changed", removed);
    await backend.emit("player-state", "playing");
    expect(uids(player.items)).toEqual(uids(full.items ?? []));
    expect(player.state).toBe(status.state);
  });
});

describe("the transport", () => {
  test("follows the player's state, volume and position", async () => {
    const { backend, player, playback } = await connected();
    await backend.emit("player-state", "playing");
    await backend.emit("player-volume", 0.25);
    await backend.emit("player-position", { position: 30, duration: 61 });
    expect(player.playing).toBe(true);
    expect(player.volume).toBe(0.25);
    expect(playback).toEqual({ position: 30, duration: 61 });
    expect([player.position, player.duration]).toEqual([30, 61]);
  });

  test("before the track is open, the position is where it will resume", async () => {
    const { backend, player } = await connected();
    await backend.emit("queue-changed", { ...advanced, revision: 9, loaded: false, resumeAt: 42 });
    expect(player.position).toBe(42);
    expect(player.duration).toBe(advanced.currentItem?.duration);
  });

  test("position events re-run only what reads the position", async () => {
    const { backend, player, playback } = await connected();
    const { watch, flush } = await import("./reactive.svelte");
    const bar = watch(() => playback.position);
    const controls = watch(() => [player.state, player.currentItem, player.items]);
    await backend.emit("player-position", { position: 31, duration: 60 });
    await backend.emit("player-position", { position: 32, duration: 60 });
    flush();
    expect([bar.runs, controls.runs]).toEqual([3, 1]);
    bar.stop();
    controls.stop();
  });
});

describe("the controls send their commands", () => {
  test("seeking is kept within the track and shown at once", async () => {
    const { backend, player, playback } = await connected();
    backend.answer("queueSeek", undefined);
    await player.seek(1000);
    await player.seek(-5);
    expect(backend.argsOf("queueSeek")).toEqual([{ seconds: full.currentItem?.duration }, { seconds: 0 }]);
    expect(playback.position).toBe(0);
  });

  test("repeat goes off, all, one", async () => {
    const { backend, player } = await connected();
    backend.answer("queueSetRepeat", undefined);
    await player.cycleRepeat();
    await backend.emit("queue-changed", { ...advanced, revision: 9, repeat: "all" });
    await player.cycleRepeat();
    await backend.emit("queue-changed", { ...advanced, revision: 10, repeat: "one" });
    await player.cycleRepeat();
    expect(backend.argsOf("queueSetRepeat")).toEqual([{ repeat: "all" }, { repeat: "one" }, { repeat: "off" }]);
  });

  test("stop after the current item, or not if it already would", async () => {
    const { backend, player } = await connected();
    backend.answer("queueSetStopAfter", undefined);
    const uid = full.currentItem?.uid ?? null;
    await player.toggleStopAfter();
    await backend.emit("queue-changed", { ...advanced, revision: 9, currentItem: full.currentItem, stopAfter: uid });
    await player.toggleStopAfter();
    expect(backend.argsOf("queueSetStopAfter")).toEqual([{ uid }, { uid: null }]);
  });

  test("the volume shows at once", async () => {
    const { backend, player } = await connected();
    backend.answer("playerSetVolume", undefined);
    const setting = player.setVolume(0.8);
    expect(player.volume).toBe(0.8);
    await setting;
    expect(backend.argsOf("playerSetVolume")).toEqual([{ volume: 0.8 }]);
  });

  test("a refused command is shown as its coded error", async () => {
    const { backend, player, toasts } = await connected();
    backend.fail("queueNext", coded("trackGone"));
    expect(await player.next()).toBeUndefined();
    expect(toasts.list.map((toast) => toast.text)).toEqual([t("error.trackGone")]);
  });
});
