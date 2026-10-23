// Playlists, hearts and ratings (`state/collection.svelte.ts`): the list
// kept in step with the backend's events, and each action's commands,
// view and announcement. (The import and export actions open the system's
// file dialogs and aren't tested here.)

import { describe, expect, test } from "vitest";
import type { Playlist } from "$lib/api";
import { t } from "$lib/i18n";
import { fakeBackend, settle } from "./backend";

const playlist = (id: number, name: string, trackCount = 0): Playlist => ({
  id,
  name,
  rules: null,
  trackCount,
  duration: 0,
  createdAt: 1_760_000_000,
  updatedAt: 1_760_000_000,
});

const mix = playlist(1, "Mix", 3);

async function connected(lists: Playlist[] = [mix]) {
  const backend = fakeBackend().answer("playlistsList", lists);
  const { collection } = await import("$lib/state/collection.svelte");
  const { toasts } = await import("$lib/state/toasts.svelte");
  const { ui } = await import("$lib/state/ui.svelte");
  const stop = collection.connect();
  await settle();
  return { backend, collection, toasts, ui, stop };
}

describe("the playlists", () => {
  test("are listed on connecting, and again when playlists or the library change", async () => {
    const { backend, collection } = await connected();
    expect(collection.playlists).toEqual([mix]);
    await backend.emit("collection-changed", "favourites");
    await backend.emit("collection-changed", "playlists");
    await backend.emit("library-changed", []);
    await settle();
    expect(backend.argsOf("playlistsList")).toHaveLength(3);
    expect(collection.version).toBe(3);
  });

  test("a list that can't be read leaves them as they were", async () => {
    const { backend, collection } = await connected();
    backend.fail("playlistsList", "The library is not available");
    await collection.refresh();
    expect(collection.playlists).toEqual([mix]);
    expect(collection.byId(1)).toEqual(mix);
    expect(collection.byId(2)).toBeNull();
  });
});

describe("a new playlist", () => {
  test("takes a free name, opens, and waits for its name to be typed", async () => {
    const taken = [playlist(1, t("playlist.newName")), playlist(2, `${t("playlist.newName")} 2`)];
    const { backend, collection, ui } = await connected(taken);
    const made = playlist(3, `${t("playlist.newName")} 3`);
    backend.answer("playlistsCreate", made);
    expect(await collection.newPlaylist([4, 5])).toEqual(made);
    expect(backend.argsOf("playlistsCreate")).toEqual([
      { name: `${t("playlist.newName")} 3`, rules: null, trackIds: [4, 5] },
    ]);
    expect([ui.mainView, ui.playlistId, ui.renaming]).toEqual(["playlist", 3, 3]);
  });

  test("from the queue says how many tracks it holds", async () => {
    const { backend, collection, toasts } = await connected();
    const saved = playlist(2, t("playlist.queueName"), 12);
    backend.answer("playlistsCreateFromQueue", saved);
    await collection.saveQueue();
    expect(backend.argsOf("playlistsCreateFromQueue")).toEqual([{ name: t("playlist.queueName") }]);
    expect(toasts.list.map((toast) => [toast.text, toast.kind])).toEqual([
      [t("playlist.saved", { name: saved.name, count: 12 }), "info"],
    ]);
  });
});

describe("actions", () => {
  test("removing the playlist shown goes back", async () => {
    const { backend, collection, ui } = await connected();
    backend.answer("playlistsDelete", undefined);
    ui.showView("home");
    ui.showPlaylist(mix.id);
    await collection.remove(mix);
    expect(backend.argsOf("playlistsDelete")).toEqual([{ playlistId: mix.id }]);
    expect(ui.mainView).toBe("home");
  });

  test("renaming sends only a new, non-empty name", async () => {
    const { backend, collection } = await connected();
    backend.answer("playlistsRename", ({ name }) => ({ ...mix, name }));
    await collection.rename(mix, "  ");
    await collection.rename(mix, mix.name);
    await collection.rename(mix, "Night");
    expect(backend.argsOf("playlistsRename")).toEqual([{ playlistId: mix.id, name: "Night" }]);
  });

  test("playing one shuffled turns shuffle on first", async () => {
    const { backend, collection } = await connected();
    backend.answer("playlistsTrackIds", [7, 8]).answer("queueSetShuffle", undefined).answer("queuePlay", undefined);
    await collection.play(mix, true);
    expect(backend.calls.map((call) => call.command).slice(-3)).toEqual([
      "playlistsTrackIds",
      "queueSetShuffle",
      "queuePlay",
    ]);
    expect(backend.argsOf("queuePlay")).toEqual([{ trackIds: [7, 8], start: 0 }]);
  });

  test("hearts and ratings are announced", async () => {
    const { backend, collection, ui } = await connected();
    backend.answer("marksSetFavourite", undefined).answer("marksSetRating", undefined);
    await collection.setFavourite("track", [1, 2], true);
    await Promise.resolve();
    expect(ui.announcement).toBe(t("heart.added", { count: 2 }));
    await collection.setRating([1], null);
    await Promise.resolve();
    expect(ui.announcement).toBe(t("rating.cleared"));
    expect(backend.argsOf("marksSetFavourite")).toEqual([{ kind: "track", ids: [1, 2], favourite: true }]);
    expect(backend.argsOf("marksSetRating")).toEqual([{ trackIds: [1], rating: null }]);
  });

  test("a playlist file opened from the Finder is reported, with what it missed", async () => {
    const { backend, toasts } = await connected();
    await backend.emit("playlist-imported", { playlist: mix, added: 3, missing: ["/a.flac"] });
    await settle();
    expect(toasts.list.map((toast) => [toast.text, toast.kind])).toEqual([
      [t("playlist.importedMissing", { name: mix.name, count: 3, missing: 1 }), "error"],
    ]);
  });
});
