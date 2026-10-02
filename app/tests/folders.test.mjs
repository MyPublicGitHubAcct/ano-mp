// Tracks of folders that can't be read now (src/lib/folders.ts, PLAN.md H22b).
import assert from "node:assert/strict";
import { test } from "node:test";
import { unavailableEntryState, unavailableState, unreadableFolders } from "../src/lib/folders.ts";

const folder = (id, state) => ({
  id,
  path: `/Music/${id}`,
  trackCount: 1,
  lastScanAt: null,
  available: state === "available",
  status: { state },
});

test("unreadable folders are the unavailable ones but mostlyGone", () => {
  const unreadable = unreadableFolders([
    folder(1, "available"),
    folder(2, "missing"),
    folder(3, "empty"),
    folder(4, "mostlyGone"),
    folder(5, "inTrash"),
    folder(6, "noPermission"),
  ]);
  assert.deepEqual(
    [...unreadable],
    [
      [2, "missing"],
      [3, "empty"],
      [5, "inTrash"],
      [6, "noPermission"],
    ],
  );
});

test("a folder without a status goes by available", () => {
  const unreadable = unreadableFolders([
    { id: 1, path: "/a", trackCount: 0, lastScanAt: null },
    { id: 2, path: "/b", trackCount: 0, lastScanAt: null, available: false },
  ]);
  assert.deepEqual([...unreadable], [[2, "missing"]]);
});

test("a track is unavailable while its folder can't be read", () => {
  const unreadable = unreadableFolders([folder(1, "available"), folder(2, "missing")]);
  assert.equal(unavailableState(unreadable, 1), null);
  assert.equal(unavailableState(unreadable, 2), "missing");
  assert.equal(unavailableState(unreadable, 9), null, "a folder not listed yet counts as there");
  assert.equal(unavailableState(unreadable, undefined), null);
});

test("an entry of several tracks is unavailable only when all their folders are", () => {
  const unreadable = unreadableFolders([folder(1, "available"), folder(2, "missing"), folder(3, "inTrash")]);
  assert.equal(unavailableEntryState(unreadable, [2, 3]), "missing");
  assert.equal(unavailableEntryState(unreadable, [1, 2]), null);
  assert.equal(unavailableEntryState(unreadable, []), null);
});
