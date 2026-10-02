// Which library folders can't be read now, for showing their tracks as
// unavailable in lists (PLAN.md H22b). Pure, so `npm test` can run it.

import type { Folder, FolderState } from "./api";
import type { MessageKey } from "./i18n";

/** A folder's state in a few words, for the sidebar and an unavailable track's tooltip. */
export const FOLDER_SHORT: Record<FolderState, MessageKey> = {
  available: "folders.unavailable",
  missing: "folderShort.missing",
  empty: "folderShort.empty",
  mostlyGone: "folderShort.mostlyGone",
  inTrash: "folderShort.inTrash",
  noPermission: "folderShort.noPermission",
};

/**
 * The folders none of whose files can be opened now, each with its state:
 * unavailable folders, except `mostlyGone` ones, whose remaining files still
 * play. The same rule as `FolderStates::unreadable` in Rust.
 */
export function unreadableFolders(folders: readonly Folder[]): Map<number, FolderState> {
  const unreadable = new Map<number, FolderState>();
  for (const folder of folders) {
    const state = folder.status?.state ?? (folder.available === false ? "missing" : "available");
    if (state !== "available" && state !== "mostlyGone") unreadable.set(folder.id, state);
  }
  return unreadable;
}

/** Why the track in folder `folderId` can't be opened now, or null if it can. */
export function unavailableState(
  unreadable: ReadonlyMap<number, FolderState>,
  folderId: number | undefined,
): FolderState | null {
  return folderId === undefined ? null : (unreadable.get(folderId) ?? null);
}

/**
 * For an entry of several tracks (an album in the history): the first
 * folder's state when none of `folderIds` (at least one) can be read now,
 * else null.
 */
export function unavailableEntryState(
  unreadable: ReadonlyMap<number, FolderState>,
  folderIds: readonly number[],
): FolderState | null {
  if (folderIds.length === 0 || !folderIds.every((id) => unreadable.has(id))) return null;
  return unreadable.get(folderIds[0]) ?? null;
}
