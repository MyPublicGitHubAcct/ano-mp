// The library store (`state/library.svelte.ts`): folders and rules read on
// connecting and on the backend's events, browsing, scans and their
// failures, unreadable folders (H22) and the launch check (H10).

import { describe, expect, test } from "vitest";
import type { DbCheck, ScanReport } from "$lib/api";
import { errorText, t } from "$lib/i18n";
import { fakeBackend, settle } from "./backend";
import { libraryFixtures, queueFixtures, serverFixtures } from "./fixtures";

const { folders, sortSettings } = libraryFixtures;
const [away, here] = folders;
const [firstRule] = sortSettings.rules;

async function connected(check: DbCheck = { state: "ok" }) {
  const backend = fakeBackend()
    .answer("libraryFolders", folders)
    .answer("librarySortSettings", sortSettings)
    .answer("libraryDbCheck", check);
  const { library } = await import("$lib/state/library.svelte");
  const { toasts } = await import("$lib/state/toasts.svelte");
  const { ui } = await import("$lib/state/ui.svelte");
  const stop = library.connect();
  await settle();
  return { backend, library, toasts, ui, stop };
}

describe("connecting", () => {
  test("lists the folders and the rules, and takes the launch check", async () => {
    const failed: DbCheck = { state: "failed", problems: ["row 7 missing from index"], copy: null };
    const { library } = await connected(failed);
    expect(library.loaded).toBe(true);
    expect(library.folders).toEqual(folders);
    expect(library.rules).toEqual(sortSettings.rules);
    expect(library.rule?.id).toBe("album-artist");
    expect(library.dbCheck).toEqual(failed);
  });

  test("a remembered rule that is gone falls back to the first", async () => {
    localStorage.setItem("anomp.ruleId", JSON.stringify("gone"));
    const { library } = await connected();
    expect(library.ruleId).toBe(firstRule.id);
    expect(localStorage.getItem("anomp.ruleId")).toBe(JSON.stringify(firstRule.id));
  });

  test("the launch check's event, if it came first, wins over the answer", async () => {
    const backend = fakeBackend()
      .answer("libraryFolders", folders)
      .answer("librarySortSettings", sortSettings)
      .answer("libraryDbCheck", () => new Promise((resolve) => setTimeout(() => resolve({ state: "running" }), 5)));
    const { library } = await import("$lib/state/library.svelte");
    library.connect();
    await settle();
    await backend.emit("library-db-check", { state: "ok" });
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(library.dbCheck).toEqual({ state: "ok" });
  });
});

describe("folders", () => {
  test("one that can't be read is unavailable, and its tracks unreadable", async () => {
    const { library } = await connected();
    expect(library.unavailable).toEqual([away]);
    expect([...library.unreadable]).toEqual([[away.id, "missing"]]);
    expect(library.unreadableKey).toBe(String(away.id));
  });

  test("music servers aren't folders on disk", async () => {
    const { backend, library } = await connected();
    const server = { ...here, id: 9, kind: "jellyfin" as const, path: "jellyfin:living-room" };
    backend.answer("libraryFolders", [...folders, server]);
    await backend.emit("library-folders", null);
    await settle();
    expect(library.folders).toHaveLength(3);
    expect(library.localFolders).toEqual(folders);
  });

  test("removing the folder being browsed goes back to the top", async () => {
    const { backend, library } = await connected();
    backend.answer("libraryRemoveFolder", undefined);
    library.showFolder(here);
    expect(library.crumbs.map((crumb) => crumb.key)).toEqual([here.id]);
    await library.removeFolder(here);
    expect(backend.argsOf("libraryRemoveFolder")).toEqual([{ folderId: here.id }]);
    expect(library.crumbs).toEqual([]);
    expect(library.version).toBe(1);
  });
});

describe("browsing", () => {
  test("goes down a rule's levels and back up", async () => {
    const { library, ui } = await connected();
    ui.showView("home");
    library.navigate("album-artist", []);
    expect(ui.mainView).toBe("library");
    expect(library.level).toBe("albumArtist");
    library.open({ key: 4, name: "Test Signal" });
    library.open({ key: 7, name: "Chirps" });
    expect([library.path, library.level]).toEqual([[4, 7], null]);
    library.goUp(1);
    expect([library.path, library.level]).toEqual([[4], "album"]);
  });

  test("an album opens under album artist, then album, and clears the search", async () => {
    const { library } = await connected();
    library.query = "chirp";
    const album = { id: 7, title: "Chirps", albumArtist: null, albumArtistId: null };
    expect(library.showAlbum(album)).toBe(true);
    expect(library.ruleId).toBe("album-artist");
    expect(library.crumbs).toEqual([
      { key: null, name: t("library.unknownArtist") },
      { key: 7, name: "Chirps" },
    ]);
    expect(library.query).toBe("");

    library.setRules(sortSettings.rules.filter((rule) => rule.levels[0] !== "albumArtist"));
    expect(library.showAlbum(album)).toBe(false);
  });
});

describe("scanning", () => {
  const report = (failed: number): ScanReport => ({
    ...serverFixtures.synced,
    failed: Array.from({ length: failed }, (_, i) => ({
      path: `/Users/me/Music/Here/${i}.flac`,
      error: queueFixtures.skipped.skipped[0].error,
    })),
  });

  test("shows the first three failures and how many more", async () => {
    const { backend, library, toasts } = await connected();
    backend.answer("libraryScan", [report(2), report(3)]);
    await library.scan(null);
    expect(backend.argsOf("libraryScan")).toEqual([{ folderId: null }]);
    const error = errorText(queueFixtures.skipped.skipped[0].error);
    expect(toasts.list.map((toast) => toast.text)).toEqual([
      `/Users/me/Music/Here/0.flac: ${error}`,
      `/Users/me/Music/Here/1.flac: ${error}`,
      `/Users/me/Music/Here/0.flac: ${error}`,
      t("folders.moreFailed", { count: 2 }),
    ]);
    expect([library.scanning, library.version]).toEqual([false, 1]);
    expect(backend.argsOf("libraryFolders")).toHaveLength(2);
  });

  test("one at a time", async () => {
    const { backend, library } = await connected();
    backend.answer("libraryScan", [report(0)]);
    await Promise.all([library.scan(here.id), library.scan(null)]);
    expect(backend.argsOf("libraryScan")).toEqual([{ folderId: here.id }]);
  });

  test("a scan someone else started shows as in the background", async () => {
    const { backend, library } = await connected();
    await backend.emit("library-scanning", true);
    await backend.emit("library-scan-progress", { folderId: here.id, read: 1, toRead: 4 });
    expect(library.background).toBe(true);
    expect(library.scanProgress?.read).toBe(1);
    await backend.emit("library-scanning", false);
    expect([library.background, library.scanProgress]).toEqual([false, null]);
  });
});

test("metadata changes count up per album and artist, so their art and pages reload", async () => {
  const { backend, library } = await connected();
  await backend.emit("metadata-changed", { albums: [7], artists: [4] });
  await backend.emit("metadata-changed", { albums: [7, 8], artists: [] });
  expect([library.artVersions.get(7), library.artVersions.get(8), library.artistVersions.get(4)]).toEqual([2, 1, 1]);
  await backend.emit("metadata-progress", {
    done: 0,
    total: 1,
    current: { kind: "artist", artistId: 4, name: "Test Signal" },
    paused: false,
    unreachable: [],
  });
  expect(library.lookingUp).toBe(4);
});
