// Types of the Tauri commands' payloads and events, and typed wrappers for
// them. The settings' types are generated from the Rust ones
// (`generated/settings.ts`, checked by `cargo test`); the rest are
// hand-written to match the Rust structs (serde camelCase).

import { Channel, convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AppSettings,
  CoverBasis,
  DeviceInfo,
  MetadataSettings,
  OutputStatus,
  ServiceSettings,
  SettingsPayload,
  SortRule,
  SortSettings,
  SourceId,
} from "./generated/settings";

export type * from "./generated/settings";

// ---- Library ----------------------------------------------------------------

/** Whether a library folder can be read now, and if not why (PLAN.md H22). */
export type FolderState = "available" | "missing" | "empty" | "mostlyGone" | "inTrash" | "noPermission";
export type FolderStatus = { state: FolderState; /** The system's words, in English. */ detail?: string };

export type Folder = {
  id: number;
  path: string;
  trackCount: number;
  lastScanAt: number | null;
  /** Whether it can be read now (F8); false for an unplugged drive or a folder moved out of reach. */
  available?: boolean;
  /** Why not (H22). */
  status?: FolderStatus;
};

export type Track = {
  id: number;
  path: string;
  title: string | null;
  artist: string | null;
  artistId: number | null;
  album: string | null;
  albumId: number | null;
  albumArtist: string | null;
  genre: string | null;
  year: number | null;
  discNumber: number | null;
  trackNumber: number | null;
  duration: number;
  bitrateKbps: number | null;
  /** Hz. */
  sampleRate: number;
  /** Unix seconds it came into the library (O15). */
  addedAt: number;
  /** In the listening history (O8). */
  playCount: number;
  lastPlayed: number | null;
  /** The user set playback preferences for it or its album (O7). */
  hasPrefs: boolean;
  /** Classical works (O6). */
  composer: string | null;
  work: string | null;
  movementName: string | null;
  movementNumber: number | null;
  /** Seconds into its file where it starts: > 0 for a cue sheet's track or a chapter (O5). */
  rangeStart: number;
  /** The user's heart and stars (F3). */
  favourite: boolean;
  rating: number | null;
  /** Its library folder: the track is unavailable while that can't be read (H22b, `lib/folders.ts`). */
  folderId: number;
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
  /** A hearted album or artist (F3). */
  favourite: boolean;
};

export type BrowsePage = { groups: Group[]; tracks: Track[]; total: number };

/** A stored rule's id, or a whole rule. */
export type RuleSpec = string | SortRule;

export type ScanReport = {
  folderId: number;
  added: number;
  updated: number;
  removed: number;
  /** Files moved or renamed, keeping their tracks (F10). */
  moved: number;
  unchanged: number;
  failed: { path: string; error: string }[];
  /** The folder couldn't be scanned, or the scan kept tracks it didn't find (H22): why. */
  unavailable?: FolderStatus;
};

/** What a browse (or playing a node) keeps. */
export type BrowseFilter = { favourites?: boolean };
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

export type ArtistAlbum = {
  id: number;
  title: string;
  albumArtist: string | null;
  albumArtistId: number | null;
  /** The earliest year among its tracks. */
  year: number | null;
  trackCount: number;
  /** From the album's MusicBrainz match: "Album", "EP", "Single"…, and "Compilation", "Live"… */
  releaseType: string | null;
  secondaryTypes: string[];
};

/** An artist on MusicBrainz. Dates are "1985", "1985-06" or "1985-06-01". */
export type MusicBrainzArtist = {
  id: string;
  name: string;
  sortName: string | null;
  disambiguation: string | null;
  /** "Person", "Group", "Orchestra", "Choir", "Character" or "Other". */
  type: string | null;
  area: string | null;
  beginArea: string | null;
  endArea: string | null;
  begin: string | null;
  end: string | null;
  ended: boolean;
  genres: string[];
  wikidata: string | null;
  wikipedia: string | null;
  homepage: string | null;
};

/** An artist's biography or an album's description, to be shown with its
    source credited and linked under its licence. */
export type SourcedArticle = {
  source: SourceId;
  sourceName: string;
  license: string;
  licenseUrl: string;
  /** The article, to credit and link. */
  title: string;
  url: string;
  language: string;
  paragraphs: string[];
};

export type LinkStatus = "matched" | "review" | "none";

export type ArtistInfo = {
  /** The MusicBrainz match; null if never looked up. */
  status: LinkStatus | null;
  checkedAt: number | null;
  /** The user chose the match, or "none of these". */
  chosenByUser: boolean;
  musicbrainz: MusicBrainzArtist | null;
  biography: SourcedArticle | null;
  /** Whether a lookup can be asked for now. */
  canLookUp: boolean;
};

export type ArtistPage = {
  id: number;
  name: string;
  /** Tracks by the artist or on their albums. */
  trackCount: number;
  /** Oldest first. */
  albums: ArtistAlbum[];
  appearsOn: ArtistAlbum[];
  info: ArtistInfo;
};

export type CoverAlbum = { id: number; title: string; artist: string | null; year: number | null };
export type CoverWall = {
  basis: CoverBasis;
  /** "1997", or the artist's name. */
  label: string;
  year: number | null;
  artistId: number | null;
  /** The current track's album first, if it has one. */
  albums: CoverAlbum[];
};

/** The launch check of the library database (PLAN.md H10). */
export type DbCheck =
  | { state: "running" }
  | { state: "ok" }
  | {
      state: "failed";
      /** SQLite's report, in English. */
      problems: string[];
      /** The copy a restore would use: written before the last upgrade. */
      copy: { fileName: string; writtenAt: number | null } | null;
    };

export const library = {
  dbCheck: () => invoke<DbCheck>("library_db_check"),
  /** Restarts the app with the newest copy of the database in place. */
  dbRestore: () => invoke<void>("library_db_restore"),
  /** Exports the user's data, then restarts with a new database that scans the folders and imports it.
      Fails if the export fails, unless `force`. */
  dbRebuild: (force: boolean) => invoke<void>("library_db_rebuild", { force }),
  folders: () => invoke<Folder[]>("library_folders"),
  addFolder: (path: string) => invoke<Folder>("library_add_folder", { path }),
  /** Points a folder at where the user found it; follow with a scan. */
  locateFolder: (folderId: number, path: string) => invoke<Folder>("library_locate_folder", { folderId, path }),
  removeFolder: (folderId: number) => invoke<void>("library_remove_folder", { folderId }),
  /** Rescans a folder, removing the tracks it doesn't find even when a scan would keep them (H22). */
  removeMissing: (folderId: number) => invoke<ScanReport[]>("library_remove_missing", { folderId }),
  /** One folder, or all when `folderId` is null. */
  scan: (folderId: number | null) => invoke<ScanReport[]>("library_scan", { folderId }),
  browse: (ruleId: string, path: BrowsePath, offset: number, limit: number, filter: BrowseFilter | null = null) =>
    invoke<BrowsePage>("library_browse", { ruleId, path, offset, limit, filter }),
  sortSettings: () => invoke<SortSettings>("library_sort_settings"),
  /** Adds a rule, or replaces the one with its id. */
  saveSortRule: (rule: SortRule) => invoke<SortSettings>("library_save_sort_rule", { rule }),
  /** The last rule can't be removed. */
  removeSortRule: (ruleId: string) => invoke<SortSettings>("library_remove_sort_rule", { ruleId }),
  setIgnoredArticles: (articles: string[]) => invoke<SortSettings>("library_set_ignored_articles", { articles }),
  /** Back to the built-in rules and articles. */
  resetSortSettings: () => invoke<SortSettings>("library_reset_sort_settings"),
  search: (query: string, offset: number, limit: number, kinds?: SearchKind[]) =>
    invoke<SearchResults>("library_search", { query, kinds: kinds ?? null, offset, limit }),
  /** Also asks for the artist to be looked up if needed; `metadata-changed` names them when done. */
  artist: (artistId: number) => invoke<ArtistPage>("library_artist", { artistId }),
  /** Null if the track has no year (or artist). */
  coverWall: (trackId: number, basis: CoverBasis) => invoke<CoverWall | null>("library_cover_wall", { trackId, basis }),
  /** The tracks under a node, in the order it lists them. */
  nodeTrackIds: (rule: RuleSpec, path: BrowsePath, recursive: boolean, filter: BrowseFilter | null = null) =>
    invoke<number[]>("library_node_track_ids", { rule, path, recursive, filter }),
  /** Everything a track's file says, for Get Info (F16). */
  trackDetails: (trackId: number) => invoke<TrackDetails>("library_track_details", { trackId }),
};

// ---- Get Info (F16) ------------------------------------------------------------

export type FileInfo = {
  /** Each value of each tag field, as [TagLib's key, value]. */
  fields: [string, string][];
  pictures: { kind: string; mimeType: string; description: string }[];
  /** "ID3v2.4, ID3v1"; "" if untagged. */
  tagTypes: string;
  codec: string;
  lossless: boolean;
  bitsPerSample: number | null;
  bitrateKbps: number | null;
  sampleRate: number;
  channels: number;
  duration: number;
  fileSize: number;
};

export type TrackDetails = {
  track: Track;
  folder: string;
  relativePath: string;
  rangeEnd: number | null;
  file: FileInfo;
  pictures: { kind: string; mimeType: string; description: string; size: number; dataUrl: string | null }[];
  musicbrainz: {
    recording: string | null;
    release: string | null;
    releaseGroup: string | null;
    releaseTrack: string | null;
    artists: string[];
    albumArtists: string[];
    work: string | null;
  };
  /** Why the file couldn't be read (its drive isn't there…). */
  error: string | null;
};

// ---- Playlists, hearts and ratings (F1–F3) ---------------------------------------

export type SmartCondition =
  | { field: "genre"; value: string }
  | { field: "year"; from: number | null; to: number | null }
  | { field: "format"; value: string }
  | { field: "addedWithin"; days: number }
  | { field: "favourite"; value: boolean }
  | { field: "rating"; atLeast: number }
  | { field: "playCount"; atLeast: number | null; atMost: number | null }
  | { field: "notPlayedFor"; days: number }
  | { field: "artist"; value: string };

export type SmartOrder = "random" | "dateAdded" | "mostPlayed" | "lastPlayed" | "rating" | "album" | "title";

export type SmartRules = {
  matchAll: boolean;
  conditions: SmartCondition[];
  order: SmartOrder;
  limit: number | null;
  seed: number;
};

export type Playlist = {
  id: number;
  name: string;
  /** A smart playlist's rules; null for a list of tracks. */
  rules: SmartRules | null;
  trackCount: number;
  duration: number;
  createdAt: number;
  updatedAt: number;
};

/** A track in a playlist; `itemId` tells two entries of one track apart (null in a smart playlist). */
export type PlaylistEntry = Track & { itemId: number | null };
export type PlaylistPage = { playlist: Playlist; entries: PlaylistEntry[] };
export type PlaylistImport = { playlist: Playlist; added: number; missing: string[] };

export type MarkKind = "track" | "album" | "artist";
export type Favourites = {
  tracks: Track[];
  albums: AlbumCard[];
  artists: { id: number; name: string; trackCount: number; addedAt: number }[];
};

/** What changed in the user's collection, from `collection-changed`. */
export type CollectionChanged = "playlists" | "favourites" | "ratings" | "all";

/** Longest playlist name, in characters (`MAX_NAME` in `library/playlists.rs`). */
export const PLAYLIST_NAME_MAX = 30;

export const playlists = {
  list: () => invoke<Playlist[]>("playlists_list"),
  page: (playlistId: number, offset: number, limit: number) =>
    invoke<PlaylistPage>("playlists_page", { playlistId, offset, limit }),
  /** A list of `trackIds`, or a smart playlist of `rules`. */
  create: (name: string, trackIds: number[] = [], rules: SmartRules | null = null) =>
    invoke<Playlist>("playlists_create", { name, rules, trackIds }),
  createFromQueue: (name: string) => invoke<Playlist>("playlists_create_from_queue", { name }),
  /** How many tracks `rules` match now. */
  preview: (rules: SmartRules) => invoke<number>("playlists_preview", { rules }),
  rename: (playlistId: number, name: string) => invoke<Playlist>("playlists_rename", { playlistId, name }),
  setRules: (playlistId: number, rules: SmartRules) => invoke<Playlist>("playlists_set_rules", { playlistId, rules }),
  remove: (playlistId: number) => invoke<void>("playlists_delete", { playlistId }),
  /** Before entry `at`, or at the end; resolves to how many were added. */
  add: (playlistId: number, trackIds: number[], at: number | null = null) =>
    invoke<number>("playlists_add", { playlistId, trackIds, at }),
  removeItems: (playlistId: number, itemIds: number[]) => invoke<void>("playlists_remove", { playlistId, itemIds }),
  /** `to` is the index after the move. */
  move: (playlistId: number, itemIds: number[], to: number) =>
    invoke<void>("playlists_move", { playlistId, itemIds, to }),
  trackIds: (playlistId: number) => invoke<number[]>("playlists_track_ids", { playlistId }),
  import: (path: string) => invoke<PlaylistImport>("playlists_import", { path }),
  /** Resolves to how many tracks were written. */
  export: (playlistId: number, path: string) => invoke<number>("playlists_export", { playlistId, path }),
};

export const marks = {
  setFavourite: (kind: MarkKind, ids: number[], favourite: boolean) =>
    invoke<void>("marks_set_favourite", { kind, ids, favourite }),
  favouritesAmong: (kind: MarkKind, ids: number[]) => invoke<number[]>("marks_favourites_among", { kind, ids }),
  /** 1 to 5 stars, or null to clear. */
  setRating: (trackIds: number[], rating: number | null) => invoke<void>("marks_set_rating", { trackIds, rating }),
  favourites: () => invoke<Favourites>("marks_favourites"),
};

// ---- The user's data (F20) -------------------------------------------------------

export type DataImport = {
  tracksFound: number;
  tracks: number;
  albumsFound: number;
  albums: number;
  artistsFound: number;
  artists: number;
  favourites: number;
  ratings: number;
  plays: number;
  positions: number;
  preferences: number;
  picks: number;
  playlists: number;
  missing: string[];
  settings: boolean;
  queue: boolean;
};

export const data = {
  export: (path: string) => invoke<void>("data_export", { path }),
  /** With `settings`, the file's settings replace the app's. */
  import: (path: string, settings: boolean) => invoke<DataImport>("data_import", { path, settings }),
};

// ---- The app around the page (F5–F7) ---------------------------------------------

// ---- Diagnostics (H9) -----------------------------------------------------------

export const diagnostics = {
  /** Versions, the OS, the output device, counts, the folders' states, the switches on and the log's
      last lines, as text to paste into a bug report; no paths or titles. */
  text: () => invoke<string>("diagnostics_text"),
  /** Shows the log files in the Finder. */
  showLogs: () => invoke<void>("diagnostics_show_logs"),
};

export const shell = {
  /** Paths dropped on the window: folders (to offer as library folders) and playable files. */
  sortDropped: (paths: string[]) => invoke<{ folders: string[]; files: string[] }>("shell_sort_dropped", { paths }),
  /** The menus' shortcuts: [what, keys], "CmdOrCtrl" for ⌘. */
  shortcuts: () => invoke<[string, string][]>("shell_shortcuts"),
  showMain: () => invoke<void>("shell_show_main"),
  toggleMiniPlayer: () => invoke<void>("shell_toggle_mini_player"),
};

/** The URL of an album's (or an album-less track's) art. `generation`
    changes after each scan, and an album's `version` with each
    `metadata-changed` naming it, so changed art isn't served from the
    webview's cache. The request fails (404) when there is none. */
export function artUrl(key: { albumId: number } | { trackId: number }, generation: number, version = 0) {
  const name = "albumId" in key ? `album-${key.albumId}` : `track-${key.trackId}`;
  return `${convertFileSrc(name, "anomp-art")}?g=${generation}.${version}`;
}

// ---- Metadata sources ---------------------------------------------------------

/** Albums and artists whose details or art changed; sent in batches. */
export type MetadataChanged = { albums: number[]; artists: number[] };

/** What the metadata worker is doing. */
export type MetadataProgress = {
  /** Albums and artists finished and in all since the worker was last idle; both 0 when it is. */
  done: number;
  total: number;
  current:
    | { kind: "album"; albumId: number; title: string; artist: string | null }
    | { kind: "artist"; artistId: number; name: string }
    | null;
  /** Automatic work waits for a service that couldn't be reached. */
  paused: boolean;
  /** Hosts that couldn't be reached, which are tried again later. */
  unreachable: string[];
};

/** A release (one issue of an album) at an album-details source. Dates are "2007", "2007-12" or "2007-12-26".
    Discogs releases have `styles` and `credits`, and their `releaseGroupId` is a Discogs master. */
export type Release = {
  id: string;
  title: string;
  /** The artist credit as printed. */
  artist: string;
  artistIds: string[];
  date: string | null;
  country: string | null;
  /** "Official", "Promotion", "Bootleg"… */
  status: string | null;
  barcode: string | null;
  labels: { name: string | null; catalogNumber: string | null }[];
  releaseGroupId: string | null;
  /** "Album", "Single", "EP"… */
  releaseType: string | null;
  /** "Compilation", "Live", "Soundtrack"… */
  secondaryTypes: string[];
  /** When the album first came out. */
  firstReleaseDate: string | null;
  genres: string[];
  trackCount: number;
  hasFrontArt: boolean | null;
  /** Empty in search results. */
  tracks: { disc: number; position: number; title: string; lengthMs: number | null; recordingId: string | null }[];
  /** Each medium's format ("CD", "12\" Vinyl", "Digital Media"). */
  formats: (string | null)[];
  disambiguation: string | null;
  /** Finer genres ("Art Rock"); Discogs only. */
  styles: string[];
  /** Who did what on the whole release; Discogs only. */
  credits: { role: string; name: string }[];
};

/** An album's link to a details source. */
export type AlbumLink = {
  source: SourceId;
  sourceName: string;
  status: LinkStatus;
  externalId: string | null;
  /** 0 to 1. */
  score: number;
  chosenByUser: boolean;
  /** The matched release, or the candidate awaiting review; null when not kept (see `storesDetails`). */
  release: Release | null;
  checkedAt: number;
  /** When false, `release` is null: fetch it with `metadata.releaseDetails` when shown. */
  storesDetails: boolean;
  /** The release's page at the source. */
  pageUrl: string | null;
  /** Shown next to the source's data, linked to `pageUrl`. */
  credit: string | null;
};

export type AlbumTrack = {
  id: number;
  disc: number | null;
  number: number | null;
  title: string;
  duration: number;
  /** Classical works (O6). */
  work: string | null;
  movementName: string | null;
  movementNumber: number | null;
  composer: string | null;
  conductor: string | null;
};

export type AlbumDetails = {
  id: number;
  title: string;
  albumArtist: string | null;
  albumArtistId: number | null;
  /** From the tags. */
  year: number | null;
  genres: string[];
  taggedReleaseId: string | null;
  tracks: AlbumTrack[];
  duration: number;
  /** One per album-details source shown that has a row. */
  links: AlbumLink[];
  /** From the first album-description source shown that has one. */
  description: SourcedArticle | null;
  /** Where the cover shown comes from; null if there is none. */
  cover: { source: SourceId; sourceName: string; chosen: boolean } | null;
  /** Whether a details source can be searched now. */
  canLookUp: boolean;
};

/** What one source offers in a dialog; `note` says why there's nothing (offline, turned off…). `credit` is shown
    next to the source's data as its terms require, linked to `creditUrl` (its site's search for the album). */
export type SourceCandidates<T> = {
  source: SourceId;
  sourceName: string;
  candidates: T[];
  note: string | null;
  credit: string | null;
  creditUrl: string | null;
};

export type ReleaseCandidate = {
  release: Release;
  score: number;
  /** Scored with its track lengths; otherwise from the search result alone. */
  full: boolean;
  /** The release's page at its source. */
  pageUrl: string | null;
};

export type CoverCandidate = {
  source: SourceId;
  /** What choosing it stores; null for the embedded picture. */
  reference: string | null;
  label: string;
  detail: string | null;
  /** What to preview it by (`candidateArtUrl`); an archive picture must be fetched first. */
  preview: string | null;
};

export type CoverChoices = {
  chosen: { source: SourceId; reference: string | null } | null;
  sources: SourceCandidates<CoverCandidate>[];
};

export type ArtistCandidate = { artist: MusicBrainzArtist; score: number };

/** A release group as an artist's MusicBrainz discography lists it. */
export type ReleaseGroupEntry = {
  id: string;
  title: string;
  /** The artist credit as printed. */
  artist: string;
  /** "Album", "Single", "EP", "Broadcast" or "Other", and "Compilation", "Live"… */
  releaseType: string | null;
  secondaryTypes: string[];
  /** "2007", "2007-10" or "2007-10-10". */
  firstReleaseDate: string | null;
  disambiguation: string | null;
};

/** What MusicBrainz lists for an artist that the library doesn't have. */
export type Discography = {
  musicbrainzId: string;
  musicbrainzName: string;
  /** Oldest first, undated last. */
  missing: ReleaseGroupEntry[];
  /** How many of those listed the library has. */
  inLibrary: number;
  /** Release groups MusicBrainz has, and how many were listed (fewer for very long discographies). */
  total: number;
  listed: number;
};

export const metadata = {
  settings: () => invoke<MetadataSettings>("metadata_settings"),
  /** Art may come from other sources afterwards; reload it. */
  saveSettings: (settings: ServiceSettings) => invoke<MetadataSettings>("metadata_save_settings", { settings }),
  /** Keys keep through a reset. */
  resetSettings: () => invoke<MetadataSettings>("metadata_reset_settings"),
  /** Saves a source's key in the keychain (turning the source on), or removes it with null. */
  setKey: (source: SourceId, key: string | null) => invoke<MetadataSettings>("metadata_set_key", { source, key }),
  status: () => invoke<MetadataProgress>("metadata_status"),
  /** Tries unreachable services again now. */
  retryNow: () => invoke<void>("metadata_retry_now"),
  /** Matches an album and fetches its cover now, as far as needed; fails at once offline. */
  updateAlbum: (albumId: number) => invoke<void>("metadata_update_album", { albumId }),
  /** Matches an artist and fetches their biography now, as far as needed; fails at once offline. */
  updateArtist: (artistId: number) => invoke<void>("metadata_update_artist", { artistId }),

  album: (albumId: number) => invoke<AlbumDetails>("metadata_album", { albumId }),
  /** Searches for `title` and `artist`, or the album's own; a release MBID or URL as `title` is looked up. */
  releaseCandidates: (albumId: number, title: string | null = null, artist: string | null = null) =>
    invoke<SourceCandidates<ReleaseCandidate>[]>("metadata_release_candidates", { albumId, title, artist }),
  /** The release an album is linked to at `source`, fetched now if the source doesn't keep it; fails offline for those. */
  releaseDetails: (albumId: number, source: SourceId) =>
    invoke<Release | null>("metadata_release_details", { albumId, source }),
  /** A MusicBrainz release's cover follows in `metadata-changed`. */
  chooseRelease: (albumId: number, source: SourceId, releaseId: string) =>
    invoke<void>("metadata_choose_release", { albumId, source, releaseId }),
  rejectRelease: (albumId: number, source: SourceId) => invoke<void>("metadata_reject_release", { albumId, source }),
  /** Clears the match and matches again now; fails at once offline. */
  useAutomaticRelease: (albumId: number, source: SourceId) =>
    invoke<void>("metadata_use_automatic_release", { albumId, source }),
  coverCandidates: (albumId: number) => invoke<CoverChoices>("metadata_cover_candidates", { albumId }),
  /** Downloads an archive picture for its preview; false if the archive hasn't got it. */
  fetchImage: (url: string) => invoke<boolean>("metadata_fetch_image", { url }),
  chooseCover: (albumId: number, source: SourceId, reference: string | null) =>
    invoke<void>("metadata_choose_cover", { albumId, source, reference }),
  useAutomaticCover: (albumId: number) => invoke<void>("metadata_use_automatic_cover", { albumId }),
  /** Searches for `name`, or the artist's own; an artist MBID or URL is looked up. */
  artistCandidates: (artistId: number, name: string | null = null) =>
    invoke<ArtistCandidate[]>("metadata_artist_candidates", { artistId, name }),
  /** Their biography follows in `metadata-changed`. */
  chooseArtist: (artistId: number, mbid: string) => invoke<void>("metadata_choose_artist", { artistId, mbid }),
  rejectArtist: (artistId: number) => invoke<void>("metadata_reject_artist", { artistId }),
  /** Clears the match and looks the artist up again now; fails at once offline. */
  useAutomaticArtist: (artistId: number) => invoke<void>("metadata_use_automatic_artist", { artistId }),
  /** Fetched from MusicBrainz unless cached (or `refresh`); only the cached copy while online services are off. */
  artistDiscography: (artistId: number, refresh = false) =>
    invoke<Discography>("metadata_artist_discography", { artistId, refresh }),
};

/** The URL of a picture the "Choose cover" dialog offers. */
export function candidateArtUrl(albumId: number, candidate: Pick<CoverCandidate, "source" | "preview">) {
  // Not through convertFileSrc, which would encode the slash.
  const url = `${convertFileSrc(`album-${albumId}`, "anomp-art")}/${candidate.source}`;
  return candidate.preview === null ? url : `${url}?ref=${encodeURIComponent(candidate.preview)}`;
}

// ---- Settings -------------------------------------------------------------------

export const settings = {
  get: () => invoke<SettingsPayload>("settings_get"),
  /** Applies and saves them; `settings-changed` follows. Fails, saving nothing, if a new output device won't open. */
  save: (settings: AppSettings) => invoke<AppSettings>("settings_save", { settings }),
  /** The output devices there are now, and the one playing. */
  outputStatus: () => invoke<OutputStatus>("audio_output_status"),
};

// ---- Player and queue ---------------------------------------------------------

export type PlayerState = "empty" | "stopped" | "playing" | "paused";
export type PlayerStatus = { state: PlayerState; position: number; duration: number; volume: number };
export type Repeat = "off" | "all" | "one";

export type QueueItem = {
  uid: number;
  trackId: number;
  title: string;
  artist: string | null;
  artistId: number | null;
  album: string | null;
  albumId: number | null;
  duration: number;
  /** Passed over in album and shuffle play (O7). */
  skip?: boolean;
  /** Why library radio picked it (O9). */
  reason?: string;
  /** A file outside the library, opened from the Finder (F5). */
  external?: boolean;
};

/** A sleep timer (F13). `endsAt` is Unix seconds. */
export type SleepTimer =
  { kind: "at"; endsAt: number; minutes: number } | { kind: "endOfTrack" } | { kind: "endOfAlbum" };
export type SleepRequest = { kind: "minutes"; minutes: number } | { kind: "endOfTrack" } | { kind: "endOfAlbum" };

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
  /** Library radio keeps adding tracks (O9). */
  radio: boolean;
  /** Playback stops after this item (F13). */
  stopAfter: number | null;
  sleep: SleepTimer | null;
};

/** Every step from the file to the speakers (O10). */
export type SignalPath = {
  path: {
    loaded: boolean;
    codec: string;
    lossless: boolean;
    bitsPerSample: number | null;
    bitrateKbps: number | null;
    fileSampleRate: number;
    fileChannels: number;
    /** Linear. */
    trackGain: number;
    tempo: number;
    semitones: number;
    resampling: boolean;
    crossfeed: number;
    volume: number;
    deviceSampleRate: number;
    deviceBufferSize: number;
    /** The equaliser is on (F15). */
    equaliser: boolean;
    /** Seconds the next track crossfades over (F14); 0 if none. */
    crossfade: number;
  };
  device: DeviceInfo | null;
  /** Whether the OS says the output is headphones; null if it can't tell. */
  headphones: boolean | null;
};

/** Practice mode (O12). */
export type Practice = { loop: [number, number] | null; tempo: number; semitones: number };

export const player = {
  status: () => invoke<PlayerStatus>("player_status"),
  setVolume: (volume: number) => invoke<void>("player_set_volume", { volume }),
  signalPath: () => invoke<SignalPath>("player_signal_path"),
  practice: () => invoke<Practice>("player_practice"),
  /** Loops the current track, or clears the loop with nulls. */
  setLoop: (start: number | null, end: number | null) =>
    invoke<[number, number] | null>("player_set_loop", { start, end }),
  setTempo: (rate: number, semitones: number) => invoke<void>("player_set_tempo", { rate, semitones }),
};

// ---- Optional features (PLAN.md O1–O19) -----------------------------------------

export type AnalysisProgress = { running: boolean; done: number; remaining: number; failed: number };
export type TrackAnalysis = {
  error: string | null;
  duration: number | null;
  loudness: number | null;
  /** dB to ReplayGain's reference. */
  gain: number | null;
  truePeak: number | null;
  albumLoudness: number | null;
  leadingSilence: number | null;
  trailingSilence: number | null;
  gapStart: number | null;
  gapLength: number | null;
  cutoffHz: number | null;
};

/** An album as the discovery and history views list it. */
export type AlbumCard = {
  id: number;
  title: string;
  artist: string | null;
  artistId: number | null;
  year: number | null;
  /** A time the view is about, Unix seconds. */
  at: number | null;
  note: string | null;
};

export type RecentEntry = {
  playedAt: number;
  album: AlbumCard | null;
  tracks: { trackId: number; title: string; artist: string | null; folderId: number }[];
};
export type TopKind = "tracks" | "albums" | "artists";
export type TopEntry = {
  id: number;
  title: string;
  subtitle: string | null;
  albumId: number | null;
  plays: number;
  trackIds: number[];
  /** The folders of `trackIds` (H22b). */
  folderIds: number[];
};
export type TopPlayed = { entries: TopEntry[]; plays: number; years: [number, number] | null };
export type Highlights = {
  forgotten: AlbumCard[];
  yearAgo: AlbumCard[];
  neverPlayed: AlbumCard[];
  totalPlays: number;
};
export type ListenBrainzStatus = { hasToken: boolean; pending: number; error: string | null };

export type HealthTrack = {
  trackId: number;
  path: string;
  title: string;
  artist: string | null;
  album: string | null;
  detail: string;
};
export type HealthReport = {
  undecodable: HealthTrack[];
  truncated: HealthTrack[];
  transcodes: HealthTrack[];
  albums: { albumId: number; title: string; artist: string | null; problems: string[] }[];
  duplicates: { reason: string; tracks: HealthTrack[] }[];
  analysed: number;
  tracks: number;
};

export type Lyrics = {
  lines: { time: number; text: string }[];
  text: string | null;
  source: "lrc" | "tags";
};

export type TrackPrefs = {
  skip?: boolean | null;
  gainOffset?: number | null;
  trimStart?: number | null;
  trimEnd?: number | null;
};
export type AlbumPrefs = { skip?: boolean | null; neverShuffle?: boolean | null; gainOffset?: number | null };

export type RemoteStatus = {
  running: boolean;
  url: string | null;
  error: string | null;
  code: string | null;
  devices: { id: number; name: string; pairedAt: number; lastSeen: number | null }[];
};

export const features = {
  analysisStatus: () => invoke<AnalysisProgress>("analysis_status"),
  /** (min, max) pairs, -127..127; null until analysed. */
  waveform: (trackId: number) => invoke<number[] | null>("analysis_waveform", { trackId }),
  trackAnalysis: (trackId: number) => invoke<TrackAnalysis | null>("analysis_track", { trackId }),
  recentlyPlayed: (limit: number) => invoke<RecentEntry[]>("history_recent", { limit }),
  topPlayed: (kind: TopKind, year: number, month: number | null) =>
    invoke<TopPlayed>("history_top", { kind, year, month }),
  highlights: () => invoke<Highlights>("history_highlights"),
  clearHistory: () => invoke<void>("history_clear"),
  listenBrainzStatus: () => invoke<ListenBrainzStatus>("history_listenbrainz_status"),
  /** Checks the token with ListenBrainz and resolves to its user; null removes it. */
  setListenBrainzToken: (token: string | null) => invoke<string | null>("history_set_listenbrainz_token", { token }),
  recentlyAdded: (limit: number) => invoke<AlbumCard[]>("library_recently_added", { limit }),
  onThisDay: (date: Date) =>
    invoke<AlbumCard[]>("library_on_this_day", {
      year: date.getFullYear(),
      month: date.getMonth() + 1,
      day: date.getDate(),
    }),
  moreInGenre: (albumId: number, genre: string, seed: number) =>
    invoke<AlbumCard[]>("library_more_in_genre", { albumId, genre, seed }),
  health: () => invoke<HealthReport>("library_health"),
  lyrics: (trackId: number) => invoke<Lyrics | null>("library_lyrics", { trackId }),
  prefs: (ids: { trackId?: number; albumId?: number }) =>
    invoke<{ track: TrackPrefs | null; album: AlbumPrefs | null }>("prefs_get", {
      trackId: ids.trackId ?? null,
      albumId: ids.albumId ?? null,
    }),
  setTrackPrefs: (trackId: number, prefs: TrackPrefs) => invoke<void>("prefs_set_track", { trackId, prefs }),
  setAlbumPrefs: (albumId: number, prefs: AlbumPrefs) => invoke<void>("prefs_set_album", { albumId, prefs }),
  remoteStatus: () => invoke<RemoteStatus>("remote_status"),
  remoteNewCode: () => invoke<RemoteStatus>("remote_new_code"),
  remoteForget: (deviceId: number) => invoke<RemoteStatus>("remote_forget", { deviceId }),
};

export const queue = {
  state: () => invoke<QueueState>("queue_state"),
  play: (trackIds: number[], start: number) => invoke<void>("queue_play", { trackIds, start }),
  playNode: (
    rule: RuleSpec,
    path: BrowsePath,
    recursive: boolean,
    startTrackId: number | null = null,
    filter: BrowseFilter | null = null,
  ) => invoke<void>("queue_play_node", { rule, path, recursive, startTrackId, filter }),
  add: (trackIds: number[], next: boolean) => invoke<void>("queue_add", { trackIds, next }),
  addNode: (rule: RuleSpec, path: BrowsePath, recursive: boolean, next: boolean, filter: BrowseFilter | null = null) =>
    invoke<void>("queue_add_node", { rule, path, recursive, next, filter }),
  remove: (uids: number[]) => invoke<void>("queue_remove", { uids }),
  /** `to` is the item's index after the move. */
  move: (uid: number, to: number) => invoke<void>("queue_move", { uid, to }),
  /** Moves items together, in their order; `to` is the index after the move. */
  moveItems: (uids: number[], to: number) => invoke<void>("queue_move_items", { uids, to }),
  clear: () => invoke<void>("queue_clear"),
  jump: (uid: number) => invoke<void>("queue_jump", { uid }),
  next: () => invoke<void>("queue_next"),
  previous: () => invoke<void>("queue_previous"),
  toggle: () => invoke<void>("queue_toggle"),
  seek: (seconds: number) => invoke<void>("queue_seek", { seconds }),
  setShuffle: (shuffle: boolean) => invoke<void>("queue_set_shuffle", { shuffle }),
  setRepeat: (repeat: Repeat) => invoke<void>("queue_set_repeat", { repeat }),
  /** Library radio (O9): the track, then tracks like it, and more as it plays. */
  startRadio: (trackId: number) => invoke<void>("queue_start_radio", { trackId }),
  stopRadio: () => invoke<void>("queue_stop_radio"),
  /** Stops after item `uid` (F13), or not with null. */
  setStopAfter: (uid: number | null) => invoke<void>("queue_set_stop_after", { uid }),
  setSleep: (sleep: SleepRequest | null) => invoke<void>("queue_set_sleep", { sleep }),
  /** Plays files (from the Finder, or dropped): after the current item, starting now (F5). */
  openFiles: (paths: string[]) => invoke<void>("queue_open_files", { paths }),
};

// ---- Visualizer ------------------------------------------------------------------

/** Streams the core's analysis (binary frames; `visualizer/frame.ts` decodes them) to `onFrame` until the
    returned function is called. The core analyses only while something is subscribed. */
export async function subscribeToAnalysis(onFrame: (frame: ArrayBuffer) => void): Promise<() => Promise<void>> {
  const channel = new Channel<ArrayBuffer>();
  channel.onmessage = onFrame;
  const id = await invoke<number>("visualizer_subscribe", { channel });
  return async () => {
    channel.onmessage = () => {};
    await invoke<void>("visualizer_unsubscribe", { id });
  };
}

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
