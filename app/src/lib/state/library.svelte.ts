// The library's folders and sort rules, where the browser is, the search
// query, and scanning.

import { ask, open } from "@tauri-apps/plugin-dialog";
import { SvelteMap, SvelteSet } from "svelte/reactivity";
import {
  artUrl,
  library as api,
  on,
  onAll,
  type AlbumHit,
  type AlbumOrder,
  type BrowsePath,
  type DbCheck,
  type Folder,
  type Group,
  type GroupKey,
  type Level,
  type ScanProgress,
  type SortRule,
} from "$lib/api";
import { unreadableFolders } from "$lib/folders";
import { errorText, t, type MessageKey } from "$lib/i18n";
import { attempt, toasts } from "./toasts.svelte";
import { loadPreference, savePreference, ui } from "./ui.svelte";

export type Crumb = { key: GroupKey | null; name: string };

class LibraryStore {
  folders = $state.raw<Folder[]>([]);
  rules = $state.raw<SortRule[]>([]);
  ruleId = $state<string>(loadPreference("ruleId", "album-artist"));
  /** The groups browsed into, from the top of the rule. */
  crumbs = $state.raw<Crumb[]>([]);
  query = $state("");
  scanning = $state(false);
  scanProgress = $state.raw<ScanProgress | null>(null);
  /** Increases when the library's contents may have changed (after a scan). */
  version = $state(0);
  /** Per album, the number of `metadata-changed` events naming it, so its art reloads. */
  artVersions = new SvelteMap<number, number>();
  /** Per artist, the number of `metadata-changed` events naming them, so their page reloads. */
  artistVersions = new SvelteMap<number, number>();
  /** The artist the metadata worker is looking up, if any. */
  lookingUp = $state<number | null>(null);

  get rule(): SortRule | undefined {
    return this.rules.find((rule) => rule.id === this.ruleId);
  }

  get path(): BrowsePath {
    return this.crumbs.map((crumb) => crumb.key);
  }

  /** The level the current node lists groups of; null if it lists tracks. */
  get level(): Level | null {
    return this.rule?.levels[this.crumbs.length] ?? null;
  }

  /** Whether the folders have been listed yet (the welcome view waits for it). */
  loaded = $state(false);
  /** A scan the user didn't start is running (at launch, after changes on disk). */
  background = $state(false);
  /** Unavailable folders the user chose to keep: no message about them until the next launch (H22). */
  kept = new SvelteSet<number>();
  /** The launch check of the database (H10); a failure offers a repair. */
  dbCheck = $state.raw<DbCheck>({ state: "running" });

  /** Follows scans (whoever started them) and metadata changes; returns a function that stops. */
  /** The header-sized cover of a queue item: its album's, or the track's own. */
  coverUrl(item: { albumId: number | null; trackId: number }): string {
    return item.albumId !== null
      ? artUrl({ albumId: item.albumId }, this.version, this.artVersions.get(item.albumId), "header")
      : artUrl({ trackId: item.trackId }, this.version, 0, "header");
  }

  connect() {
    const listeners = [
      on("library-scan-progress", (progress) => (this.scanProgress = progress)),
      on("library-scanning", (running) => {
        if (running && !this.scanning) this.background = true;
        if (!running) {
          this.background = false;
          this.scanProgress = null;
        }
      }),
      on("library-changed", () => {
        this.version++;
        void this.refresh();
      }),
      on("metadata-changed", ({ albums, artists }) => {
        for (const id of albums) this.artVersions.set(id, (this.artVersions.get(id) ?? 0) + 1);
        for (const id of artists) this.artistVersions.set(id, (this.artistVersions.get(id) ?? 0) + 1);
      }),
      on("library-db-check", (check) => (this.dbCheck = check)),
      on("library-folders", () => void this.refresh()),
      on("metadata-progress", ({ current }) => {
        this.lookingUp = current?.kind === "artist" ? current.artistId : null;
      }),
    ];
    attempt(() => this.refresh());
    attempt(async () => {
      const check = await api.dbCheck();
      // The event may have come first.
      if (this.dbCheck.state === "running") this.dbCheck = check;
    });
    return onAll(listeners);
  }

  async refresh() {
    this.folders = await api.folders();
    this.loaded = true;
    this.rules = (await api.sortSettings()).rules;
    if (!this.rule) this.navigate(this.rules[0].id, []);
  }

  /** Shows a node of the library in the main area. */
  navigate(ruleId: string, crumbs: Crumb[]) {
    ui.showLibrary();
    this.ruleId = ruleId;
    this.crumbs = crumbs;
    savePreference("ruleId", ruleId);
  }

  /** Takes rules the settings changed, staying where it is unless its rule is gone. */
  setRules(rules: SortRule[]) {
    this.rules = rules;
    if (!this.rule) {
      this.ruleId = rules[0].id;
      this.crumbs = [];
      savePreference("ruleId", this.ruleId);
    }
  }

  /** Sets how the current rule orders albums, and saves it with the rule. */
  setAlbumOrder = (albumOrder: AlbumOrder) =>
    attempt(async () => {
      const rule = this.rule;
      if (!rule) return;
      this.rules = (await api.saveSortRule({ ...rule, albumOrder })).rules;
    });

  /** Back up to the node `depth` levels from the top. */
  goUp(depth: number) {
    this.crumbs = this.crumbs.slice(0, depth);
  }

  open(crumb: Crumb) {
    this.crumbs = [...this.crumbs, crumb];
  }

  /** The first rule whose levels start with `levels`. */
  ruleStartingWith(...levels: Level[]) {
    return this.rules.find((rule) => levels.every((level, i) => rule.levels[i] === level));
  }

  /** Scans one folder, or all when `folderId` is null. */
  scan = (folderId: number | null) =>
    attempt(async () => {
      if (this.scanning) return;
      this.scanning = true;
      try {
        const reports = await api.scan(folderId);
        const failed = reports.flatMap((report) => report.failed);
        for (const failure of failed.slice(0, 3)) toasts.show(`${failure.path}: ${errorText(failure.error)}`);
        if (failed.length > 3) toasts.show(t("folders.moreFailed", { count: failed.length - 3 }));
      } finally {
        this.scanning = false;
        this.scanProgress = null;
        this.version++;
        await this.refresh();
      }
    });

  /** Asks for a folder (starting at `defaultPath`), adds it and scans it. */
  addFolder = (defaultPath?: string) =>
    attempt(async () => {
      const path = await open({ multiple: false, directory: true, defaultPath });
      if (path === null) return;
      // A folder already in the library comes back with a fresh bookmark.
      const folder = await api.addFolder(path);
      this.folders = [...this.folders.filter((f) => f.id !== folder.id), folder];
      await this.scan(folder.id);
    });

  /** Adds the folder at `path` (dropped on the window, say) and scans it. */
  addFolderAt = (path: string) =>
    attempt(async () => {
      const folder = await api.addFolder(path);
      this.folders = [...this.folders.filter((f) => f.id !== folder.id), folder];
      await this.scan(folder.id);
    });

  /** A folder that moved, or whose drive is unplugged: the user shows where it is now (F8). */
  locateFolder = (folder: Folder) =>
    attempt(async () => {
      const path = await open({
        multiple: false,
        directory: true,
        defaultPath: folder.path,
        title: t("folders.locateTitle", { name: folderName(folder.path) }),
      });
      if (path === null) return;
      await api.locateFolder(folder.id, path);
      await this.refresh();
      await this.scan(folder.id);
    });

  /** Folders that can't be read now (H22). */
  get unavailable(): Folder[] {
    return this.folders.filter((folder) => folder.available === false);
  }

  /** Folders none of whose tracks can be opened now, with why: lists show their tracks as unavailable (H22b). */
  unreadable = $derived(unreadableFolders(this.folders));
  /** Changes only when the set of unreadable folders does: views whose suggestions leave them out reload on it. */
  unreadableKey = $derived([...this.unreadable.keys()].join(","));

  /** Rescans `folder`, removing the tracks it doesn't find, after asking (H22). */
  removeMissing = (folder: Folder) =>
    attempt(async () => {
      const confirmed = await ask(t("missing.removeTracksConfirm", { name: folderName(folder.path) }), {
        title: t("missing.removeTracksTitle"),
        kind: "warning",
        okLabel: t("missing.removeTracksOk"),
      });
      if (!confirmed) return;
      await api.removeMissing(folder.id);
      this.version++;
      await this.refresh();
    });

  removeFolder = (folder: Folder) =>
    attempt(async () => {
      await api.removeFolder(folder.id);
      if (this.rule?.levels[0] === "folder" && this.crumbs[0]?.key === folder.id) this.crumbs = [];
      this.version++;
      await this.refresh();
    });

  /** Opens an album in the browser, under the first rule that starts with
      album artist → album, else album. False if no rule fits. */
  showAlbum(album: Pick<AlbumHit, "id" | "title" | "albumArtist" | "albumArtistId">) {
    const byArtist = this.ruleStartingWith("albumArtist", "album");
    const byAlbum = this.ruleStartingWith("album");
    if (byArtist) {
      this.navigate(byArtist.id, [
        { key: album.albumArtistId, name: album.albumArtist ?? t("library.unknownArtist") },
        { key: album.id, name: album.title },
      ]);
    } else if (byAlbum) {
      this.navigate(byAlbum.id, [{ key: album.id, name: album.title }]);
    } else {
      return false;
    }
    this.query = "";
    return true;
  }

  /** Browses a library folder under the first folder rule. */
  showFolder(folder: Folder) {
    const rule = this.ruleStartingWith("folder");
    if (rule) this.navigate(rule.id, [{ key: folder.id, name: folderName(folder.path) }]);
  }
}

export const folderName = (path: string) => path.split(/[\\/]/).filter(Boolean).pop() ?? path;

const standardTrackOrder: SortRule["trackOrder"] = ["discNumber", "trackNumber", "title", "path"];

/** A rule of `levels`, for playing what search or an artist page found
    whatever the stored rules are. */
const UNKNOWN: Record<Level, MessageKey> = {
  albumArtist: "library.unknownArtist",
  artist: "library.unknownArtist",
  album: "library.unknownAlbum",
  genre: "library.unknownGenre",
  year: "library.unknownYear",
  folder: "library.unknownFolder",
  composer: "library.unknownComposer",
  work: "library.noWork",
};

/** A group's name as shown: the backend names the group of tracks
    without a value at `level` ("Unknown artist") in English. */
export const groupName = (group: Pick<Group, "key" | "name">, level: Level | null | undefined) =>
  group.key === null && level ? t(UNKNOWN[level]) : group.name;

export function adHocRule(levels: Level[], albumOrder?: AlbumOrder): SortRule {
  return {
    id: `ad-hoc-${levels.join("-")}`,
    name: t("search.placeholder"),
    levels,
    trackOrder: levels.includes("album") ? standardTrackOrder : ["album", ...standardTrackOrder],
    albumOrder: albumOrder ?? "title",
  };
}

export const library = new LibraryStore();
