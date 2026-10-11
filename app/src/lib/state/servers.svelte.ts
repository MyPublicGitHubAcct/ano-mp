// Music servers (PLAN.md Phases 7e and 7f): the signed-in sources with their
// state and sync, and what is kept offline. The Settings panel signs in and
// out; the sidebar lists the servers.

import { servers as api, on, onAll, type OfflineStatus, type ServerSummary, type Source } from "$lib/api";
import { attempt } from "./toasts.svelte";

/** A source as lists name it: the server, and a Plex source's music library ("Den · Music"). */
export function sourceName(source: Pick<Source, "serverName" | "sectionName">): string {
  return source.sectionName ? `${source.serverName} · ${source.sectionName}` : source.serverName;
}

class ServersStore {
  list = $state.raw<ServerSummary[]>([]);
  offline = $state.raw<OfflineStatus | null>(null);

  connect() {
    const listeners = [
      on("servers-changed", () => void this.refresh()),
      on("library-folders", () => void this.refresh()),
      on("library-changed", () => void this.refresh()),
    ];
    void this.refresh();
    return onAll(listeners);
  }

  async refresh() {
    try {
      this.list = await api.list();
      this.offline = await api.offlineStatus();
    } catch {
      // The library isn't there yet; the next event tries again.
    }
  }

  sync = (server: ServerSummary, full = false) =>
    attempt(async () => {
      await api.sync(server.folderId, full);
      await this.refresh();
    });

  signOut = (server: ServerSummary) =>
    attempt(async () => {
      await api.signOut(server.folderId);
      await this.refresh();
    });

  /** Whether album `albumId` or playlist `playlistId` is kept offline. */
  isOffline(target: { albumId?: number; playlistId?: number }) {
    const status = this.offline;
    if (!status) return false;
    if (target.albumId !== undefined) return status.albums.includes(target.albumId);
    return target.playlistId !== undefined && status.playlists.includes(target.playlistId);
  }

  setOffline = (target: { albumId?: number; playlistId?: number }, keep: boolean) =>
    attempt(async () => {
      this.offline = await api.setOffline(target, keep);
    });

  clearCache = () =>
    attempt(async () => {
      this.offline = await api.clearCache();
    });
}

export const servers = new ServersStore();
