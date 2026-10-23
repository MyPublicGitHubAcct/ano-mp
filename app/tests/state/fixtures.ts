// The payloads the Rust tests record for these tests (`ui_fixtures.rs`;
// `ANOMP_WRITE_BINDINGS=1 cargo test ui_fixtures` rewrites them), typed as
// the generated types say. Each is built the way its command or event
// builds it, so a fake answer here is one the backend could give.

import type {
  Folder,
  OfflineStatus,
  QueueState,
  ScanReport,
  ServerSummary,
  SettingsPayload,
  SortSettings,
} from "$lib/api";
import libraryJson from "./fixtures/library.json";
import queueJson from "./fixtures/queue.json";
import serversJson from "./fixtures/servers.json";
import settingsJson from "./fixtures/settings.json";

/** `library_folders` (one folder whose drive is unplugged, then one that is there) and `library_sort_settings`. */
export const libraryFixtures = libraryJson as { folders: Folder[]; sortSettings: SortSettings };

/** `settings_get` after a change was saved; `refused`, the error `settings_save` refuses invalid settings with. */
export const settingsFixtures = settingsJson as { get: SettingsPayload; refused: string };

/** `queue-changed` states in order: the whole list (version 1), an item removed (2), one added (3), the next
    track (no list), the whole list again (4, as `queue_state` answers), and, from another queue, a track that
    took too long to open (`skipped`, a coded error). */
export const queueFixtures = queueJson as Record<
  "full" | "removed" | "added" | "advanced" | "resynced" | "skipped",
  QueueState
>;

/** `servers_list` (a Plex source, then a synced Jellyfin one), the Jellyfin sync's report, `servers_offline_status`
    before and after an album was kept offline, and the error a command refuses with while Jellyfin is off. */
export const serverFixtures = serversJson as {
  list: ServerSummary[];
  synced: ScanReport;
  offline: OfflineStatus;
  albumKept: OfflineStatus;
  jellyfinOff: string;
};
