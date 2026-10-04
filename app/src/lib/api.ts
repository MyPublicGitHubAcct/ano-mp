// Wrappers for the Tauri commands and events, as the components call them.
// The payloads' types and the commands themselves are generated from the
// Rust code (`generated/settings.ts`, `generated/ipc.ts` and
// `generated/commands.ts`, checked by `cargo test`; PLAN.md H15), so a
// renamed command, argument or field fails `npm run check`. Only types the
// frontend composes itself are written here.

import { Channel, convertFileSrc } from "@tauri-apps/api/core";
import { commands } from "./generated/commands";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AppSettings, CoverBasis, ServiceSettings, SortRule, SourceId, Theme } from "./generated/settings";

import type {
  AlbumPrefs,
  AnalysisProgress,
  BrowseFilter,
  CollectionChanged,
  CoverCandidate,
  DbCheck,
  GroupKey,
  MarkKind,
  MetadataChanged,
  MetadataProgress,
  PlayerState,
  PlaylistImport,
  QueueState,
  Repeat,
  RuleSpec,
  ScanProgress,
  ScanReport,
  SearchKind,
  SleepRequest,
  SmartRules,
  TopKind,
  TrackPrefs,
  UpdateCheck,
} from "./generated/ipc";

export type * from "./generated/settings";
export type * from "./generated/ipc";

// ---- Library ----------------------------------------------------------------

/** One key per level browsed; null for "Unknown …" groups. */
export type BrowsePath = (GroupKey | null)[];

export const library = {
  dbCheck: () => commands.libraryDbCheck(),
  /** Restarts the app with the newest copy of the database in place. */
  dbRestore: () => commands.libraryDbRestore(),
  /** Exports the user's data, then restarts with a new database that scans the folders and imports it.
      Fails if the export fails, unless `force`. */
  dbRebuild: (force: boolean) => commands.libraryDbRebuild({ force }),
  folders: () => commands.libraryFolders(),
  addFolder: (path: string) => commands.libraryAddFolder({ path }),
  /** Points a folder at where the user found it; follow with a scan. */
  locateFolder: (folderId: number, path: string) => commands.libraryLocateFolder({ folderId, path }),
  removeFolder: (folderId: number) => commands.libraryRemoveFolder({ folderId }),
  /** Rescans a folder, removing the tracks it doesn't find even when a scan would keep them (H22). */
  removeMissing: (folderId: number) => commands.libraryRemoveMissing({ folderId }),
  /** One folder, or all when `folderId` is null. */
  scan: (folderId: number | null) => commands.libraryScan({ folderId }),
  browse: (ruleId: string, path: BrowsePath, offset: number, limit: number, filter: BrowseFilter | null = null) =>
    commands.libraryBrowse({ ruleId, path, offset, limit, filter }),
  sortSettings: () => commands.librarySortSettings(),
  /** Adds a rule, or replaces the one with its id. */
  saveSortRule: (rule: SortRule) => commands.librarySaveSortRule({ rule }),
  /** The last rule can't be removed. */
  removeSortRule: (ruleId: string) => commands.libraryRemoveSortRule({ ruleId }),
  setIgnoredArticles: (articles: string[]) => commands.librarySetIgnoredArticles({ articles }),
  /** Back to the built-in rules and articles. */
  resetSortSettings: () => commands.libraryResetSortSettings(),
  search: (query: string, offset: number, limit: number, kinds?: SearchKind[]) =>
    commands.librarySearch({ query, kinds: kinds ?? null, offset, limit }),
  /** Also asks for the artist to be looked up if needed; `metadata-changed` names them when done. */
  artist: (artistId: number) => commands.libraryArtist({ artistId }),
  /** Null if the track has no year (or artist). */
  coverWall: (trackId: number, basis: CoverBasis) => commands.libraryCoverWall({ trackId, basis }),
  /** The tracks under a node, in the order it lists them. */
  nodeTrackIds: (rule: RuleSpec, path: BrowsePath, recursive: boolean, filter: BrowseFilter | null = null) =>
    commands.libraryNodeTrackIds({ rule, path, recursive, filter }),
  /** Everything a track's file says, for Get Info (F16). */
  trackDetails: (trackId: number) => commands.libraryTrackDetails({ trackId }),
};

// ---- Get Info (F16) ------------------------------------------------------------

// ---- Playlists, hearts and ratings (F1–F3) ---------------------------------------

/** Longest playlist name, in characters (`MAX_NAME` in `library/playlists.rs`). */
export const PLAYLIST_NAME_MAX = 30;

export const playlists = {
  list: () => commands.playlistsList(),
  page: (playlistId: number, offset: number, limit: number) => commands.playlistsPage({ playlistId, offset, limit }),
  /** A list of `trackIds`, or a smart playlist of `rules`. */
  create: (name: string, trackIds: number[] = [], rules: SmartRules | null = null) =>
    commands.playlistsCreate({ name, rules, trackIds }),
  createFromQueue: (name: string) => commands.playlistsCreateFromQueue({ name }),
  /** How many tracks `rules` match now. */
  preview: (rules: SmartRules) => commands.playlistsPreview({ rules }),
  rename: (playlistId: number, name: string) => commands.playlistsRename({ playlistId, name }),
  setRules: (playlistId: number, rules: SmartRules) => commands.playlistsSetRules({ playlistId, rules }),
  remove: (playlistId: number) => commands.playlistsDelete({ playlistId }),
  /** Before entry `at`, or at the end; resolves to how many were added. */
  add: (playlistId: number, trackIds: number[], at: number | null = null) =>
    commands.playlistsAdd({ playlistId, trackIds, at }),
  removeItems: (playlistId: number, itemIds: number[]) => commands.playlistsRemove({ playlistId, itemIds }),
  /** `to` is the index after the move. */
  move: (playlistId: number, itemIds: number[], to: number) => commands.playlistsMove({ playlistId, itemIds, to }),
  trackIds: (playlistId: number) => commands.playlistsTrackIds({ playlistId }),
  import: (path: string) => commands.playlistsImport({ path }),
  /** Resolves to how many tracks were written. */
  export: (playlistId: number, path: string) => commands.playlistsExport({ playlistId, path }),
};

export const marks = {
  setFavourite: (kind: MarkKind, ids: number[], favourite: boolean) =>
    commands.marksSetFavourite({ kind, ids, favourite }),
  favouritesAmong: (kind: MarkKind, ids: number[]) => commands.marksFavouritesAmong({ kind, ids }),
  /** 1 to 5 stars, or null to clear. */
  setRating: (trackIds: number[], rating: number | null) => commands.marksSetRating({ trackIds, rating }),
  favourites: () => commands.marksFavourites(),
};

// ---- The user's data (F20) -------------------------------------------------------

export const data = {
  export: (path: string) => commands.dataExport({ path }),
  /** With `settings`, the file's settings replace the app's. */
  import: (path: string, settings: boolean) => commands.dataImport({ path, settings }),
};

// ---- The app around the page (F5–F7) ---------------------------------------------

// ---- Diagnostics (H9) -----------------------------------------------------------

export const diagnostics = {
  /** Versions, the OS, the output device, counts, the folders' states, the switches on and the log's
      last lines, as text to paste into a bug report; no paths or titles. */
  text: () => commands.diagnosticsText(),
  /** Shows the log files in the Finder. */
  showLogs: () => commands.diagnosticsShowLogs(),
  /** The third-party notices (THIRD_PARTY_NOTICES, bundled with the app), as text. */
  notices: () => commands.diagnosticsNotices(),
  /** Discogs' non-affiliation notice, in its terms' words. */
  discogsNotice: () => commands.diagnosticsDiscogsNotice(),
  /** The main window has painted: logs the time since launch, once (H18). */
  firstPaint: () => commands.diagnosticsFirstPaint(),
};

export const updates = {
  /** The last check for a newer release that worked since launch, if any. */
  status: () => commands.updatesStatus(),
  /** Checks GitHub for a newer release now, whether or not automatic checks are on. */
  check: () => commands.updatesCheck(),
};

export const shell = {
  /** Paths dropped on the window: folders (to offer as library folders) and playable files. */
  sortDropped: (paths: string[]) => commands.shellSortDropped({ paths }),
  /** The menus' shortcuts: [what, keys], "CmdOrCtrl" for ⌘. */
  shortcuts: () => commands.shellShortcuts(),
  showMain: () => commands.shellShowMain(),
  toggleMiniPlayer: () => commands.shellToggleMiniPlayer(),
};

/** The URL of an album's (or an album-less track's) art. `generation`
    changes after each scan, and an album's `version` with each
    `metadata-changed` naming it, so changed art isn't served from the
    webview's cache. The request fails (404) when there is none. `size` asks
    for a thumbnail (PLAN.md H17): "list" (128 px) or "header" (512 px);
    "full" only where the picture is shown full size. */
export function artUrl(
  key: { albumId: number } | { trackId: number },
  generation: number,
  version = 0,
  size: ArtSize = "full",
) {
  const name = "albumId" in key ? `album-${key.albumId}` : `track-${key.trackId}`;
  const thumbnail = size === "full" ? "" : `&size=${size}`;
  return `${convertFileSrc(name, "anomp-art")}?g=${generation}.${version}${thumbnail}`;
}

/** How large a cover is drawn: a list row's, the album header's (and
    grids'), or full size. */
export type ArtSize = "list" | "header" | "full";

// ---- Metadata sources ---------------------------------------------------------

export const metadata = {
  settings: () => commands.metadataSettings(),
  /** Art may come from other sources afterwards; reload it. */
  saveSettings: (settings: ServiceSettings) => commands.metadataSaveSettings({ settings }),
  /** Keys keep through a reset. */
  resetSettings: () => commands.metadataResetSettings(),
  /** Saves a source's key in the keychain (turning the source on), or removes it with null. */
  setKey: (source: SourceId, key: string | null) => commands.metadataSetKey({ source, key }),
  status: () => commands.metadataStatus(),
  /** Tries unreachable services again now. */
  retryNow: () => commands.metadataRetryNow(),
  /** Matches an album and fetches its cover now, as far as needed; fails at once offline. */
  updateAlbum: (albumId: number) => commands.metadataUpdateAlbum({ albumId }),
  /** Matches an artist and fetches their biography now, as far as needed; fails at once offline. */
  updateArtist: (artistId: number) => commands.metadataUpdateArtist({ artistId }),

  album: (albumId: number) => commands.metadataAlbum({ albumId }),
  /** Searches for `title` and `artist`, or the album's own; a release MBID or URL as `title` is looked up. */
  releaseCandidates: (albumId: number, title: string | null = null, artist: string | null = null) =>
    commands.metadataReleaseCandidates({ albumId, title, artist }),
  /** The release an album is linked to at `source`, fetched now if the source doesn't keep it; fails offline for those. */
  releaseDetails: (albumId: number, source: SourceId) => commands.metadataReleaseDetails({ albumId, source }),
  /** A MusicBrainz release's cover follows in `metadata-changed`. */
  chooseRelease: (albumId: number, source: SourceId, releaseId: string) =>
    commands.metadataChooseRelease({ albumId, source, releaseId }),
  rejectRelease: (albumId: number, source: SourceId) => commands.metadataRejectRelease({ albumId, source }),
  /** Clears the match and matches again now; fails at once offline. */
  useAutomaticRelease: (albumId: number, source: SourceId) => commands.metadataUseAutomaticRelease({ albumId, source }),
  coverCandidates: (albumId: number) => commands.metadataCoverCandidates({ albumId }),
  /** Downloads an archive picture for its preview; false if the archive hasn't got it. */
  fetchImage: (url: string) => commands.metadataFetchImage({ url }),
  chooseCover: (albumId: number, source: SourceId, reference: string | null) =>
    commands.metadataChooseCover({ albumId, source, reference }),
  useAutomaticCover: (albumId: number) => commands.metadataUseAutomaticCover({ albumId }),
  /** Searches for `name`, or the artist's own; an artist MBID or URL is looked up. */
  artistCandidates: (artistId: number, name: string | null = null) =>
    commands.metadataArtistCandidates({ artistId, name }),
  /** Their biography follows in `metadata-changed`. */
  chooseArtist: (artistId: number, mbid: string) => commands.metadataChooseArtist({ artistId, mbid }),
  rejectArtist: (artistId: number) => commands.metadataRejectArtist({ artistId }),
  /** Clears the match and looks the artist up again now; fails at once offline. */
  useAutomaticArtist: (artistId: number) => commands.metadataUseAutomaticArtist({ artistId }),
  /** Fetched from MusicBrainz unless cached (or `refresh`); only the cached copy while online services are off. */
  artistDiscography: (artistId: number, refresh = false) => commands.metadataArtistDiscography({ artistId, refresh }),
};

/** The URL of a picture the "Choose cover" dialog offers. */
export function candidateArtUrl(albumId: number, candidate: Pick<CoverCandidate, "source" | "preview">) {
  // Not through convertFileSrc, which would encode the slash.
  const url = `${convertFileSrc(`album-${albumId}`, "anomp-art")}/${candidate.source}`;
  return candidate.preview === null ? url : `${url}?ref=${encodeURIComponent(candidate.preview)}`;
}

// ---- Settings -------------------------------------------------------------------

export const settings = {
  get: () => commands.settingsGet(),
  /** Applies and saves them; `settings-changed` follows. Fails, saving nothing, if a new output device won't open. */
  save: (settings: AppSettings) => commands.settingsSave({ settings }),
  /** The output devices there are now, and the one playing. */
  outputStatus: () => commands.audioOutputStatus(),
};

/** Theme files (PLAN.md X1). */
export const themes = {
  export: (path: string, theme: Theme) => commands.themeExport({ path, theme }),
  /** The theme in a file, read leniently; not applied. Fails with `notATheme` for any other file. */
  import: (path: string) => commands.themeImport({ path }),
};

// ---- Player and queue ---------------------------------------------------------

export const player = {
  status: () => commands.playerStatus(),
  setVolume: (volume: number) => commands.playerSetVolume({ volume }),
  signalPath: () => commands.playerSignalPath(),
  practice: () => commands.playerPractice(),
  /** Loops the current track, or clears the loop with nulls. */
  setLoop: (start: number | null, end: number | null) => commands.playerSetLoop({ start, end }),
  setTempo: (rate: number, semitones: number) => commands.playerSetTempo({ rate, semitones }),
};

// ---- Optional features (PLAN.md O1–O19) -----------------------------------------

export const features = {
  analysisStatus: () => commands.analysisStatus(),
  /** (min, max) pairs, -127..127; null until analysed. */
  waveform: (trackId: number) => commands.analysisWaveform({ trackId }),
  trackAnalysis: (trackId: number) => commands.analysisTrack({ trackId }),
  recentlyPlayed: (limit: number) => commands.historyRecent({ limit }),
  topPlayed: (kind: TopKind, year: number, month: number | null) => commands.historyTop({ kind, year, month }),
  highlights: () => commands.historyHighlights(),
  clearHistory: () => commands.historyClear(),
  listenBrainzStatus: () => commands.historyListenbrainzStatus(),
  /** Checks the token with ListenBrainz and resolves to its user; null removes it. */
  setListenBrainzToken: (token: string | null) => commands.historySetListenbrainzToken({ token }),
  recentlyAdded: (limit: number) => commands.libraryRecentlyAdded({ limit }),
  onThisDay: (date: Date) =>
    commands.libraryOnThisDay({
      year: date.getFullYear(),
      month: date.getMonth() + 1,
      day: date.getDate(),
    }),
  moreInGenre: (albumId: number, genre: string, seed: number) => commands.libraryMoreInGenre({ albumId, genre, seed }),
  health: () => commands.libraryHealth(),
  lyrics: (trackId: number) => commands.libraryLyrics({ trackId }),
  prefs: (ids: { trackId?: number; albumId?: number }) =>
    commands.prefsGet({
      trackId: ids.trackId ?? null,
      albumId: ids.albumId ?? null,
    }),
  setTrackPrefs: (trackId: number, prefs: TrackPrefs) => commands.prefsSetTrack({ trackId, prefs }),
  setAlbumPrefs: (albumId: number, prefs: AlbumPrefs) => commands.prefsSetAlbum({ albumId, prefs }),
  remoteStatus: () => commands.remoteStatus(),
  remoteNewCode: () => commands.remoteNewCode(),
  remoteForget: (deviceId: number) => commands.remoteForget({ deviceId }),
};

export const queue = {
  state: () => commands.queueState(),
  play: (trackIds: number[], start: number) => commands.queuePlay({ trackIds, start }),
  playNode: (
    rule: RuleSpec,
    path: BrowsePath,
    recursive: boolean,
    startTrackId: number | null = null,
    filter: BrowseFilter | null = null,
  ) => commands.queuePlayNode({ rule, path, recursive, startTrackId, filter }),
  add: (trackIds: number[], next: boolean) => commands.queueAdd({ trackIds, next }),
  addNode: (rule: RuleSpec, path: BrowsePath, recursive: boolean, next: boolean, filter: BrowseFilter | null = null) =>
    commands.queueAddNode({ rule, path, recursive, next, filter }),
  remove: (uids: number[]) => commands.queueRemove({ uids }),
  /** `to` is the item's index after the move. */
  move: (uid: number, to: number) => commands.queueMove({ uid, to }),
  /** Moves items together, in their order; `to` is the index after the move. */
  moveItems: (uids: number[], to: number) => commands.queueMoveItems({ uids, to }),
  clear: () => commands.queueClear(),
  jump: (uid: number) => commands.queueJump({ uid }),
  next: () => commands.queueNext(),
  previous: () => commands.queuePrevious(),
  toggle: () => commands.queueToggle(),
  seek: (seconds: number) => commands.queueSeek({ seconds }),
  setShuffle: (shuffle: boolean) => commands.queueSetShuffle({ shuffle }),
  setRepeat: (repeat: Repeat) => commands.queueSetRepeat({ repeat }),
  /** Library radio (O9): the track, then tracks like it, and more as it plays. */
  startRadio: (trackId: number) => commands.queueStartRadio({ trackId }),
  stopRadio: () => commands.queueStopRadio(),
  /** Stops after item `uid` (F13), or not with null. */
  setStopAfter: (uid: number | null) => commands.queueSetStopAfter({ uid }),
  setSleep: (sleep: SleepRequest | null) => commands.queueSetSleep({ sleep }),
  /** Plays files (from the Finder, or dropped): after the current item, starting now (F5). */
  openFiles: (paths: string[]) => commands.queueOpenFiles({ paths }),
};

// ---- Visualizer ------------------------------------------------------------------

/** Streams the core's analysis (binary frames; `visualizer/frame.ts` decodes them) to `onFrame` until the
    returned function is called. The core analyses only while something is subscribed. */
export async function subscribeToAnalysis(onFrame: (frame: ArrayBuffer) => void): Promise<() => Promise<void>> {
  const channel = new Channel<ArrayBuffer>();
  channel.onmessage = onFrame;
  const id = await commands.visualizerSubscribe({ channel });
  return async () => {
    channel.onmessage = () => {};
    await commands.visualizerUnsubscribe({ id });
  };
}

// ---- Dev page -------------------------------------------------------------------

export const dev = {
  coreVersion: () => commands.coreVersion(),
  deviceName: () => commands.audioDeviceName(),
  playTestTone: (frequency: number) => commands.playTestTone({ frequency }),
  stopTestTone: () => commands.stopTestTone(),
  /** Loads a file directly; the queue stops following the engine. */
  load: (path: string) => commands.playerLoad({ path }),
  setNext: (path: string | null) => commands.playerSetNext({ path }),
  play: () => commands.playerPlay(),
  pause: () => commands.playerPause(),
  stop: () => commands.playerStop(),
  seek: (seconds: number) => commands.playerSeek({ seconds }),
};

// ---- Events ---------------------------------------------------------------------

type Events = {
  "audio-device-changed": null;
  "player-state": PlayerState;
  "player-position": { position: number; duration: number };
  "player-track-ended": { advanced: boolean };
  "queue-changed": QueueState;
  "library-scan-progress": ScanProgress;
  "metadata-changed": MetadataChanged;
  "metadata-progress": MetadataProgress;
  "settings-changed": AppSettings;
  "analysis-progress": AnalysisProgress;
  /** Tracks just analysed. */
  "analysis-changed": number[];
  "history-changed": null;
  "library-prefs-changed": null;
  /** A scan finished, whoever started it (F9). */
  "library-changed": ScanReport[];
  /** A scan started (true) or ended (false). */
  "library-scanning": boolean;
  /** A library folder came back or went (H22). */
  "library-folders": null;
  /** The launch check of the database finished (H10). */
  "library-db-check": DbCheck;
  "collection-changed": CollectionChanged;
  /** A menu item the page handles, by id (F6). */
  menu: string;
  /** The volume changed from outside the page (the menu). */
  "player-volume": number;
  /** A playlist file opened from the Finder was imported. */
  "playlist-imported": PlaylistImport;
  /** An automatic check found a newer release, once per version. */
  "update-available": UpdateCheck;
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
