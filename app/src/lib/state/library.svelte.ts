// The library's folders and sort rules, where the browser is, the search
// query, and scanning.

import { open } from "@tauri-apps/plugin-dialog";
import { SvelteMap } from "svelte/reactivity";
import {
  library as api,
  on,
  onAll,
  type AlbumHit,
  type AlbumOrder,
  type BrowsePath,
  type Folder,
  type GroupKey,
  type Level,
  type ScanProgress,
  type SortRule,
} from "$lib/api";
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

  /** Follows scan progress and metadata changes; returns a function that stops. */
  connect() {
    const listeners = [
      on("library-scan-progress", (progress) => (this.scanProgress = progress)),
      on("metadata-changed", ({ albums, artists }) => {
        for (const id of albums) this.artVersions.set(id, (this.artVersions.get(id) ?? 0) + 1);
        for (const id of artists) this.artistVersions.set(id, (this.artistVersions.get(id) ?? 0) + 1);
      }),
      on("metadata-progress", ({ current }) => {
        this.lookingUp = current?.kind === "artist" ? current.artistId : null;
      }),
    ];
    attempt(() => this.refresh());
    return onAll(listeners);
  }

  async refresh() {
    this.folders = await api.folders();
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
        for (const failure of failed.slice(0, 3)) toasts.show(`${failure.path}: ${failure.error}`);
        if (failed.length > 3) toasts.show(`${failed.length - 3} more files could not be read`);
      } finally {
        this.scanning = false;
        this.scanProgress = null;
        this.version++;
        await this.refresh();
      }
    });

  addFolder = () =>
    attempt(async () => {
      const path = await open({ multiple: false, directory: true });
      if (path === null) return;
      // A folder already in the library comes back with a fresh bookmark.
      const folder = await api.addFolder(path);
      this.folders = [...this.folders.filter((f) => f.id !== folder.id), folder];
      await this.scan(folder.id);
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
        { key: album.albumArtistId, name: album.albumArtist ?? "Unknown artist" },
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
export function adHocRule(levels: Level[], albumOrder?: AlbumOrder): SortRule {
  return {
    id: `ad-hoc-${levels.join("-")}`,
    name: "Search",
    levels,
    trackOrder: levels.includes("album") ? standardTrackOrder : ["album", ...standardTrackOrder],
    albumOrder,
  };
}

export const library = new LibraryStore();
