// What the user makes of the library (PLAN.md F1–F3, F20): the playlists
// the sidebar lists, and the actions on playlists, hearts and ratings
// that every view offers, with their errors as toasts. Kept in step with
// `collection-changed` and scans (a smart playlist's tracks follow them).

import { open, save } from "@tauri-apps/plugin-dialog";
import {
  data as dataApi,
  marks,
  on,
  onAll,
  playlists as api,
  queue,
  type MarkKind,
  type Playlist,
  type SmartRules,
} from "$lib/api";
import { count, t } from "$lib/i18n";
import { attempt, toasts } from "./toasts.svelte";
import { ui } from "./ui.svelte";

class CollectionStore {
  playlists = $state.raw<Playlist[]>([]);
  /** Increases when hearts, ratings or playlists change, so views showing them reload. */
  version = $state(0);

  connect() {
    const stop = onAll([
      on("collection-changed", (what) => {
        this.version++;
        if (what === "playlists" || what === "all") void this.refresh();
      }),
      on("library-changed", () => {
        this.version++;
        void this.refresh();
      }),
      on("playlist-imported", (report) => {
        void this.refresh();
        this.#reportImport(report.playlist, report.added, report.missing.length);
      }),
    ]);
    void this.refresh();
    return stop;
  }

  async refresh() {
    const list = await api.list().catch(() => null);
    if (list) this.playlists = list;
  }

  byId(id: number | null) {
    return this.playlists.find((playlist) => playlist.id === id) ?? null;
  }

  /** A name not yet taken: "New Playlist", "New Playlist 2"… */
  #freeName(base: string) {
    const taken = new Set(this.playlists.map((playlist) => playlist.name));
    if (!taken.has(base)) return base;
    for (let n = 2; ; n++) if (!taken.has(`${base} ${n}`)) return `${base} ${n}`;
  }

  /** Makes a playlist of `trackIds` (maybe none) and shows it, its name ready to type over. */
  newPlaylist = (trackIds: number[] = []) =>
    attempt(async () => {
      const playlist = await api.create(this.#freeName(t("playlist.newName")), trackIds);
      await this.refresh();
      ui.showPlaylist(playlist.id);
      ui.renaming = playlist.id;
      return playlist;
    });

  /** "Save Queue as Playlist". */
  saveQueue = () =>
    attempt(async () => {
      const playlist = await api.createFromQueue(this.#freeName(t("playlist.queueName")));
      await this.refresh();
      toasts.show(t("playlist.saved", { name: playlist.name, count: playlist.trackCount }), "info");
      ui.showPlaylist(playlist.id);
      ui.renaming = playlist.id;
    });

  createSmart = (name: string, rules: SmartRules) =>
    attempt(async () => {
      const playlist = await api.create(name, [], rules);
      await this.refresh();
      ui.showPlaylist(playlist.id);
      return playlist;
    });

  addTo = (playlist: Playlist, trackIds: number[]) =>
    attempt(async () => {
      const added = await api.add(playlist.id, trackIds);
      toasts.show(t("playlist.added", { count: added, name: playlist.name }), "info", 3000);
      ui.announce(t("playlist.added", { count: added, name: playlist.name }));
    });

  rename = (playlist: Playlist, name: string) =>
    attempt(async () => {
      if (name.trim() && name.trim() !== playlist.name) await api.rename(playlist.id, name);
    });

  remove = (playlist: Playlist) =>
    attempt(async () => {
      await api.remove(playlist.id);
      if (ui.playlistInMain && ui.playlistId === playlist.id) ui.back();
      await this.refresh();
    });

  play = (playlist: Playlist, shuffle = false) =>
    attempt(async () => {
      const ids = await api.trackIds(playlist.id);
      if (shuffle) await queue.setShuffle(true);
      await queue.play(ids, 0);
    });

  enqueue = (playlist: Playlist, next: boolean) =>
    attempt(async () => queue.add(await api.trackIds(playlist.id), next));

  importPlaylist = () =>
    attempt(async () => {
      const path = await open({
        multiple: false,
        filters: [{ name: t("playlist.fileType"), extensions: ["m3u", "m3u8"] }],
      });
      if (path === null) return;
      const report = await api.import(path);
      await this.refresh();
      this.#reportImport(report.playlist, report.added, report.missing.length);
      ui.showPlaylist(report.playlist.id);
    });

  #reportImport(playlist: Playlist, added: number, missing: number) {
    toasts.show(
      missing > 0
        ? t("playlist.importedMissing", { name: playlist.name, count: added, missing })
        : t("playlist.imported", { name: playlist.name, count: added }),
      missing > 0 ? "error" : "info",
    );
  }

  exportPlaylist = (playlist: Playlist) =>
    attempt(async () => {
      const path = await save({
        defaultPath: `${playlist.name.replace(/[/\\:]/g, "-")}.m3u8`,
        filters: [{ name: t("playlist.fileType"), extensions: ["m3u8"] }],
      });
      if (path === null) return;
      const written = await api.export(playlist.id, path);
      toasts.show(t("playlist.exported", { name: playlist.name, count: written }), "info");
    });

  setFavourite = (kind: MarkKind, ids: number[], favourite: boolean) =>
    attempt(async () => {
      await marks.setFavourite(kind, ids, favourite);
      ui.announce(favourite ? t("heart.added", { count: ids.length }) : t("heart.removed", { count: ids.length }));
    });

  setRating = (trackIds: number[], rating: number | null) =>
    attempt(async () => {
      await marks.setRating(trackIds, rating);
      ui.announce(rating === null ? t("rating.cleared") : t("rating.value", { count: rating }));
    });

  exportData = () =>
    attempt(async () => {
      const day = new Date().toISOString().slice(0, 10);
      const path = await save({
        defaultPath: `ano-mp ${day}.json`,
        filters: [{ name: t("data.fileType"), extensions: ["json"] }],
      });
      if (path === null) return;
      await dataApi.export(path);
      toasts.show(t("data.exported"), "info");
    });

  importData = (settings: boolean) =>
    attempt(async () => {
      const path = await open({ multiple: false, filters: [{ name: t("data.fileType"), extensions: ["json"] }] });
      if (path === null) return;
      const report = await dataApi.import(path, settings);
      await this.refresh();
      toasts.show(
        t("data.imported", {
          tracks: report.tracksFound,
          of: report.tracks,
          playlists: report.playlists,
          favourites: report.favourites,
          plays: report.plays,
        }),
        report.tracksFound < report.tracks ? "error" : "info",
        12000,
      );
      return report;
    });
}

export const collection = new CollectionStore();

/** "3 tracks", for a drag's label and menus. */
export const tracksLabel = (n: number) => count("count.tracks", n);
