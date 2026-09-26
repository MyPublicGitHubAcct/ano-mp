// The library's folders and sort rules, where the browser is, the search
// query, and scanning.

import { open } from "@tauri-apps/plugin-dialog";
import {
  library as api,
  on,
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

  /** Follows scan progress; returns a function that stops. */
  connect() {
    const listener = on("library-scan-progress", (progress) => (this.scanProgress = progress));
    attempt(() => this.refresh());
    return () => listener.then((stop) => stop());
  }

  async refresh() {
    this.folders = await api.folders();
    this.rules = (await api.sortSettings()).rules;
    if (!this.rule) this.navigate(this.rules[0].id, []);
  }

  /** Shows a node of the library in the main area. */
  navigate(ruleId: string, crumbs: Crumb[]) {
    ui.mainView = "library";
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
      const folder = await api.addFolder(path);
      this.folders = [...this.folders, folder];
      await this.scan(folder.id);
    });

  removeFolder = (folder: Folder) =>
    attempt(async () => {
      await api.removeFolder(folder.id);
      if (this.rule?.levels[0] === "folder" && this.crumbs[0]?.key === folder.id) this.crumbs = [];
      this.version++;
      await this.refresh();
    });

  /** Browses a library folder under the first folder rule. */
  showFolder(folder: Folder) {
    const rule = this.ruleStartingWith("folder");
    if (rule) this.navigate(rule.id, [{ key: folder.id, name: folderName(folder.path) }]);
  }
}

export const folderName = (path: string) => path.split(/[\\/]/).filter(Boolean).pop() ?? path;

export const library = new LibraryStore();
