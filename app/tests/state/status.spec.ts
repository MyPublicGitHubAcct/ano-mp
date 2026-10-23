// Two small stores that report the backend's background work: newer
// releases (`state/updates.svelte.ts`, §8.2) and the metadata worker
// (`state/metadata.svelte.ts`).

import { describe, expect, test } from "vitest";
import type { MetadataProgress, UpdateCheck } from "$lib/api";
import { t } from "$lib/i18n";
import { fakeBackend, settle } from "./backend";

const upToDate: UpdateCheck = {
  current: "0.1.0",
  latest: "0.1.0",
  url: null,
  newer: false,
  checkedAt: 1_760_000_000,
};
const newer: UpdateCheck = { ...upToDate, latest: "0.2.0", url: "https://example.org/r", newer: true };

describe("updates", () => {
  async function connected(status: UpdateCheck | null = null) {
    const backend = fakeBackend().answer("updatesStatus", status);
    const { updates } = await import("$lib/state/updates.svelte");
    const { toasts } = await import("$lib/state/toasts.svelte");
    const stop = updates.connect();
    await settle();
    return { backend, updates, toasts, stop };
  }

  test("an automatic check that finds a release says so", async () => {
    const { backend, updates, toasts } = await connected(upToDate);
    expect(updates.last).toEqual(upToDate);
    await backend.emit("update-available", newer);
    expect(updates.last).toEqual(newer);
    expect(toasts.list.map((toast) => [toast.text, toast.kind])).toEqual([
      [t("updates.available", { version: "0.2.0" }), "info"],
    ]);
  });

  test("checking by hand keeps the last check that worked", async () => {
    const { backend, updates, toasts } = await connected();
    expect(updates.last).toBeNull();
    backend.answer("updatesCheck", upToDate);
    const checking = updates.check();
    expect(updates.checking).toBe(true);
    await checking;
    expect([updates.checking, updates.last]).toEqual([false, upToDate]);

    backend.fail("updatesCheck", "offline");
    await updates.check();
    expect(updates.last).toEqual(upToDate);
    expect(toasts.list.map((toast) => toast.text)).toEqual(["offline"]);
  });
});

describe("the metadata worker", () => {
  const idle: MetadataProgress = { done: 0, total: 0, current: null, paused: false, unreachable: [] };

  async function connected(progress: MetadataProgress = idle) {
    const backend = fakeBackend().answer("metadataStatus", progress);
    const { metadataStatus } = await import("$lib/state/metadata.svelte");
    metadataStatus.connect();
    await settle();
    return { backend, metadataStatus };
  }

  test("says what it does", async () => {
    const { backend, metadataStatus } = await connected();
    expect(metadataStatus.summary).toBe(t("metadataStatus.upToDate"));

    const album = { kind: "album" as const, albumId: 7, title: "Chirps", artist: null };
    await backend.emit("metadata-progress", { ...idle, done: 2, total: 5, current: album });
    expect(metadataStatus.summary).toBe(`Chirps: ${t("metadataStatus.step", { done: 3, total: 5 })}`);

    await backend.emit("metadata-progress", { ...idle, done: 5, total: 5 });
    expect(metadataStatus.summary).toBe(t("metadataStatus.step", { done: 5, total: 5 }));
  });

  test("names the services it can't reach, paused or not", async () => {
    const unreachable = ["MusicBrainz", "Discogs"];
    const { backend, metadataStatus } = await connected({ ...idle, unreachable });
    expect(metadataStatus.summary).toBe(t("metadataStatus.unreachable", { names: "MusicBrainz, Discogs" }));
    await backend.emit("metadata-progress", { ...idle, total: 3, paused: true, unreachable });
    expect(metadataStatus.summary).toBe(t("metadataStatus.paused", { names: "MusicBrainz, Discogs" }));
  });
});
