// Music servers (`state/servers.svelte.ts`): the sources listed and what is
// kept offline, refreshed on the backend's events, and the actions on
// them, whose refusals show as coded errors.

import { describe, expect, test } from "vitest";
import { errorText } from "$lib/i18n";
import { fakeBackend, settle } from "./backend";
import { serverFixtures } from "./fixtures";

const { list, synced, offline, albumKept, jellyfinOff } = serverFixtures;
const [plex, jellyfin] = list;

async function connected() {
  const backend = fakeBackend().answer("serversList", list).answer("serversOfflineStatus", offline);
  const { servers, sourceName } = await import("$lib/state/servers.svelte");
  const { toasts } = await import("$lib/state/toasts.svelte");
  const stop = servers.connect();
  await settle();
  return { backend, servers, sourceName, toasts, stop };
}

test("a source is named by its server, and a Plex one by its music library too", async () => {
  const { sourceName } = await connected();
  expect(sourceName(jellyfin)).toBe(jellyfin.serverName);
  expect(sourceName(plex)).toContain(plex.serverName);
  expect(sourceName(plex)).toContain(plex.sectionName);
});

describe("the list", () => {
  test("is read on connecting, with what is kept offline", async () => {
    const { servers } = await connected();
    expect(servers.list).toEqual(list);
    expect(servers.offline).toEqual(offline);
  });

  test("is read again when servers, folders or the library change", async () => {
    const { backend } = await connected();
    await backend.emit("servers-changed", null);
    await backend.emit("library-folders", null);
    await backend.emit("library-changed", []);
    await settle();
    expect(backend.argsOf("serversList")).toHaveLength(4);
    expect(backend.argsOf("serversOfflineStatus")).toHaveLength(4);
  });

  test("stays as it was while the library can't be read", async () => {
    const { backend, servers, toasts } = await connected();
    backend.fail("serversList", "The library is not available");
    await servers.refresh();
    expect(servers.list).toEqual(list);
    expect(toasts.list).toEqual([]);
  });

  test("is no longer followed once stopped", async () => {
    const { backend, stop } = await connected();
    stop();
    await settle();
    await backend.emit("servers-changed", null);
    await settle();
    expect(backend.argsOf("serversList")).toHaveLength(1);
  });
});

describe("actions", () => {
  test("a sync or a sign-out names the source, then reads the list again", async () => {
    const { backend, servers } = await connected();
    backend.answer("serversSync", synced).answer("serversSignOut", undefined);
    await servers.sync(jellyfin);
    await servers.sync(jellyfin, true);
    await servers.signOut(plex);
    expect(backend.argsOf("serversSync")).toEqual([
      { folderId: jellyfin.folderId, full: false },
      { folderId: jellyfin.folderId, full: true },
    ]);
    expect(backend.argsOf("serversSignOut")).toEqual([{ folderId: plex.folderId }]);
    expect(backend.argsOf("serversList")).toHaveLength(4);
  });

  test("a sync refused while the servers are off shows why", async () => {
    const { backend, servers, toasts } = await connected();
    backend.fail("serversSync", jellyfinOff);
    expect(await servers.sync(jellyfin)).toBeUndefined();
    expect(toasts.list.map((toast) => toast.text)).toEqual([errorText(jellyfinOff)]);
    expect(backend.argsOf("serversList")).toHaveLength(1);
  });

  test("keeping an album offline takes the status the backend answers", async () => {
    const { backend, servers } = await connected();
    const [album] = albumKept.albums;
    expect(servers.isOffline({ albumId: album })).toBe(false);
    backend.answer("serversSetOffline", albumKept);
    await servers.setOffline({ albumId: album }, true);
    expect(backend.argsOf("serversSetOffline")).toEqual([{ albumId: album, playlistId: null, on: true }]);
    expect(servers.isOffline({ albumId: album })).toBe(true);
    expect(servers.isOffline({ playlistId: album })).toBe(false);
    expect(servers.isOffline({})).toBe(false);
  });

  test("clearing the cache takes the status the backend answers", async () => {
    const { backend, servers } = await connected();
    backend.answer("serversClearCache", offline);
    servers.offline = albumKept;
    await servers.clearCache();
    expect(servers.offline).toEqual(offline);
  });
});
