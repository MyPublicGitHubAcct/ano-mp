// Types of the Tauri commands' payloads and events, and typed wrappers for
// them. Hand-written to match the Rust structs (serde camelCase); PLAN.md
// Phase 6 generates them with specta/ts-rs.

import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// ---- Library ----------------------------------------------------------------

export type Folder = { id: number; path: string; trackCount: number; lastScanAt: number | null };

export type Track = {
  id: number;
  path: string;
  title: string | null;
  artist: string | null;
  album: string | null;
  albumId: number | null;
  albumArtist: string | null;
  genre: string | null;
  year: number | null;
  discNumber: number | null;
  trackNumber: number | null;
  duration: number;
};

/** An artist, album or library folder id, or a year; a genre or folder name. */
export type GroupKey = number | string;
/** One key per level browsed; null for "Unknown …" groups. */
export type BrowsePath = (GroupKey | null)[];

export type Group = {
  key: GroupKey | null;
  name: string;
  trackCount: number;
  /** Album groups only. */
  albumArtist: string | null;
  year: number | null;
};

export type BrowsePage = { groups: Group[]; tracks: Track[]; total: number };

export type Level = "albumArtist" | "artist" | "album" | "genre" | "year" | "folder";
export type TrackKey =
  | "albumArtist"
  | "artist"
  | "album"
  | "year"
  | "discNumber"
  | "trackNumber"
  | "title"
  | "path";
export type AlbumOrder = "title" | "year";
export type SortRule = {
  id: string;
  name: string;
  levels: Level[];
  trackOrder: TrackKey[];
  /** Where the rule lists albums; "title" when missing. */
  albumOrder?: AlbumOrder;
};
export type SortSettings = { rules: SortRule[]; ignoredArticles: string[] };
/** A stored rule's id, or a whole rule. */
export type RuleSpec = string | SortRule;

export type ScanReport = {
  folderId: number;
  added: number;
  updated: number;
  removed: number;
  unchanged: number;
  failed: { path: string; error: string }[];
};
export type ScanProgress = { folderId: number; read: number; toRead: number };

export type SearchKind = "artists" | "albums" | "tracks";
export type ArtistHit = { id: number; name: string; albumArtistTrackCount: number; trackCount: number };
export type AlbumHit = {
  id: number;
  title: string;
  albumArtist: string | null;
  albumArtistId: number | null;
  year: number | null;
  trackCount: number;
};
export type SearchResults = {
  artists: ArtistHit[];
  artistTotal: number;
  albums: AlbumHit[];
  albumTotal: number;
  tracks: Track[];
  trackTotal: number;
};

export const library = {
  folders: () => invoke<Folder[]>("library_folders"),
  addFolder: (path: string) => invoke<Folder>("library_add_folder", { path }),
  removeFolder: (folderId: number) => invoke<void>("library_remove_folder", { folderId }),
  /** One folder, or all when `folderId` is null. */
  scan: (folderId: number | null) => invoke<ScanReport[]>("library_scan", { folderId }),
  browse: (ruleId: string, path: BrowsePath, offset: number, limit: number) =>
    invoke<BrowsePage>("library_browse", { ruleId, path, offset, limit }),
  sortSettings: () => invoke<SortSettings>("library_sort_settings"),
  /** Adds a rule, or replaces the one with its id. */
  saveSortRule: (rule: SortRule) => invoke<SortSettings>("library_save_sort_rule", { rule }),
  search: (query: string, offset: number, limit: number, kinds?: SearchKind[]) =>
    invoke<SearchResults>("library_search", { query, kinds: kinds ?? null, offset, limit }),
};

/** The URL of an album's (or an album-less track's) art. `generation`
    changes after each scan, so changed art isn't served from the webview's
    cache. The request fails (404) when there is none. */
export function artUrl(key: { albumId: number } | { trackId: number }, generation: number) {
  const name = "albumId" in key ? `album-${key.albumId}` : `track-${key.trackId}`;
  return `${convertFileSrc(name, "anomp-art")}?g=${generation}`;
}

// ---- Player and queue ---------------------------------------------------------

export type PlayerState = "empty" | "stopped" | "playing" | "paused";
export type PlayerStatus = { state: PlayerState; position: number; duration: number; volume: number };
export type Repeat = "off" | "all" | "one";

export type QueueItem = {
  uid: number;
  trackId: number;
  title: string;
  artist: string | null;
  album: string | null;
  albumId: number | null;
  duration: number;
};

export type Skipped = { uid: number; trackId: number; title: string; error: string };

export type QueueState = {
  revision: number;
  /** Only when the list changed since the previous state. */
  items: QueueItem[] | null;
  length: number;
  current: number | null;
  currentItem: QueueItem | null;
  shuffle: boolean;
  repeat: Repeat;
  unavailable: number[];
  skipped: Skipped[];
  hasNext: boolean;
  hasPrevious: boolean;
  /** False after a relaunch until playback starts; `resumeAt` is where it will. */
  loaded: boolean;
  resumeAt: number;
};

export const player = {
  status: () => invoke<PlayerStatus>("player_status"),
  setVolume: (volume: number) => invoke<void>("player_set_volume", { volume }),
};

export const queue = {
  state: () => invoke<QueueState>("queue_state"),
  play: (trackIds: number[], start: number) => invoke<void>("queue_play", { trackIds, start }),
  playNode: (rule: RuleSpec, path: BrowsePath, recursive: boolean, startTrackId: number | null = null) =>
    invoke<void>("queue_play_node", { rule, path, recursive, startTrackId }),
  add: (trackIds: number[], next: boolean) => invoke<void>("queue_add", { trackIds, next }),
  addNode: (rule: RuleSpec, path: BrowsePath, recursive: boolean, next: boolean) =>
    invoke<void>("queue_add_node", { rule, path, recursive, next }),
  remove: (uids: number[]) => invoke<void>("queue_remove", { uids }),
  /** `to` is the item's index after the move. */
  move: (uid: number, to: number) => invoke<void>("queue_move", { uid, to }),
  clear: () => invoke<void>("queue_clear"),
  jump: (uid: number) => invoke<void>("queue_jump", { uid }),
  next: () => invoke<void>("queue_next"),
  previous: () => invoke<void>("queue_previous"),
  toggle: () => invoke<void>("queue_toggle"),
  seek: (seconds: number) => invoke<void>("queue_seek", { seconds }),
  setShuffle: (shuffle: boolean) => invoke<void>("queue_set_shuffle", { shuffle }),
  setRepeat: (repeat: Repeat) => invoke<void>("queue_set_repeat", { repeat }),
};

// ---- Dev page -------------------------------------------------------------------

export const dev = {
  coreVersion: () => invoke<string>("core_version"),
  deviceName: () => invoke<string | null>("audio_device_name"),
  playTestTone: (frequency: number) => invoke<void>("play_test_tone", { frequency }),
  stopTestTone: () => invoke<void>("stop_test_tone"),
  /** Loads a file directly; the queue stops following the engine. */
  load: (path: string) => invoke<void>("player_load", { path }),
  setNext: (path: string | null) => invoke<void>("player_set_next", { path }),
  play: () => invoke<void>("player_play"),
  pause: () => invoke<void>("player_pause"),
  stop: () => invoke<void>("player_stop"),
  seek: (seconds: number) => invoke<void>("player_seek", { seconds }),
};

// ---- Events ---------------------------------------------------------------------

type Events = {
  "audio-device-changed": null;
  "player-state": PlayerState;
  "player-position": { position: number; duration: number };
  "player-track-ended": { advanced: boolean };
  "queue-changed": QueueState;
  "library-scan-progress": ScanProgress;
};

/** Listens to a backend event; resolves to the function that stops. */
export function on<E extends keyof Events>(event: E, handler: (payload: Events[E]) => void): Promise<UnlistenFn> {
  return listen<Events[E]>(event, ({ payload }) => handler(payload));
}

/** Listens to several events; returns a function that stops them all. */
export function onAll(listeners: Promise<UnlistenFn>[]): () => void {
  return () => {
    for (const listener of listeners) listener.then((stop) => stop());
  };
}
