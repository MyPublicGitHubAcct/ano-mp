# 9. Data

Everything ano-mp keeps: the library database table by table (what each
column means and which code writes it), the settings inside it, the
caches, and where every file the app writes lives on disk. The app never
writes the user's music files; everything it learns or is told lives
here, keyed by track, album or artist id.

## The library database

One SQLite file, `library.sqlite3`, in WAL mode (`synchronous =
normal`, foreign keys on), opened only through `library/db.rs`. Its
schema is the numbered files in `app/src-tauri/src/library/migrations/`,
applied in order; `PRAGMA user_version` counts the migrations applied.
Times are Unix seconds unless a column says otherwise. Each migration's
comments explain its tables in more detail than this summary.

Two rules shape every query:

- Paths in `tracks` are **relative to their folder**, '/'-separated, so
  a folder that moves (a bookmark resolving elsewhere) keeps its tracks;
  `library::track_path` joins them.
- `anomp_sort_key`, `anomp_genres` and `anomp_has_genre` are SQL
  functions registered on this app's connections only (`db::configure`).
  Queries may use them; the schema, indexes and migrations must not.

### The library itself

Written by the scanner (`library/scanner.rs`) unless noted.

| Table | One row per | Columns worth knowing |
|---|---|---|
| `folders` | library folder | `path` (where it was last found), `bookmark` (the security-scoped bookmark; `library/mod.rs`, `access.rs` renews it), `last_scan_at` |
| `tracks` | track: a file, or a part of one | `folder_id`, `relative_path`; `range_start` and `range_end` (seconds; NULL end is the end of the file), unique with the path; `file_size` and `file_mtime_ns` (change detection; −1 forces a re-read); the tags (`title`, `genre`, numbers, `year`, `release_date`, works and movements, `conductor`); `artist_id` (the first credited artist), `artist_credit` (the tag's text when it names several), `album_id`, `album_artist_id` (the tag, else the artist, else "Various Artists"), `album_artist_tagged`, `composer_id`, `compilation`; `duration`, `sample_rate`, `channels`, `bitrate_kbps`; ReplayGain; MusicBrainz ids; `added_at` (first seen, kept across rescans); `dataless` (a cloud placeholder when last seen) |
| `artists` | name | `name` (unique, case-insensitive), `musicbrainz_id` from the tags |
| `albums` | title by an album artist | `title`, `artist_id` (unique together), the release and release-group MBIDs from the tags |
| `track_artists` | credited artist of a track | `position` in the credit (F11) |

A track's id survives rescans and moves: the scanner updates rows in
place and gives a moved file its old row (F10). An album or artist left
with no tracks is removed (`library::remove_orphans`), and the user's
picks for it are kept in **`kept_albums`** and **`kept_artists`** (as
JSON) until it comes back.

### Search indexes

`tracks_search`, `albums_search`, `artists_search` (word indexes,
migration 002, updated by 008) and `tracks_trigram`, `albums_trigram`,
`artists_trigram` (substring indexes, 009) are contentless FTS5 tables
folding case and accents. **Triggers** on `tracks`, `albums` and
`artists` keep them in step; no Rust code writes them. A migration that
changes an indexed column must update both sets of triggers.

### Online metadata (the metadata worker)

| Table | Holds | Written by |
|---|---|---|
| `album_links` | per album and source: `status` (`matched`, `review`, `none`), `external_id`, `score`, `chosen_by` (`auto` or `user`: automatic matching never replaces `user`), `details` (the source's normalised JSON; NULL for Discogs, whose terms forbid storing it), `checked_at` | `metadata/albums.rs`, `coverartarchive.rs`, `wikipedia.rs`, `discogs.rs`, the "Find details" dialog's commands |
| `artist_links` | the same per artist | `metadata/artists.rs`, `wikipedia.rs` |
| `album_art` | the cover the user chose for an album: `source` and `reference` (a folder image's relative path, an image URL, or NULL for the embedded picture) | `library/art.rs` (`choose`), the import (`library/transfer.rs`) |
| `mb_cache` | every service's JSON responses by request URL, with `fetched_at` | `metadata/cache.rs`, pruned at launch |
| `art_thumbs` | which picture an album or track shows (`origin` JSON, `stamp`, `hash`), so thumbnails are found without reading the picture | `library/thumbs.rs` |
| `outside_dismissed` | outside recommendations the user said no to, by MusicBrainz id | `metadata/outside.rs` |

### What the optional features keep (migration 006)

| Table | Holds | Written by |
|---|---|---|
| `track_analysis` | one analysis per track, for the file as it was (`file_size`, `file_mtime_ns`): loudness, peaks, the loudness histogram, silences, start and end levels, the spectrum's cutoff, the seek bar's `envelope`; `error` when it couldn't be decoded | `library/analysis.rs` |
| `album_analysis` | an album's gated loudness and peak | `library/analysis.rs` |
| `plays` | each play that counted: `played_at`, `seconds` heard | `history/mod.rs` |
| `listens_pending` | listens waiting for ListenBrainz, as the JSON it takes | `history/listenbrainz.rs` |
| `track_prefs`, `album_prefs` | the user's playback rules (skip, never shuffle, gain offset, trims); NULL leaves a value to the album or the default | `library/prefs.rs` |
| `remote_devices` | phones paired with the LAN remote: `name`, `token_hash` (never the token) | `remote/mod.rs` |

### What the user makes (migration 007)

| Table | Holds | Written by |
|---|---|---|
| `playlists` | `name`; `rules` (JSON) for a smart playlist, which has no items | `library/playlists.rs`, `smart.rs` |
| `playlist_items` | a playlist's tracks in `position` order (gaps allowed, repeats allowed) | `library/playlists.rs` |
| `track_favourites`, `album_favourites`, `artist_favourites` | hearts, with `added_at` | `library/marks.rs` |
| `track_ratings` | whole stars; `source` `tags` (copied from the file, follows it) or `user` (the user's, never overwritten; a `user` row with no rating is one cleared) | `library/marks.rs`, the scanner for `tags` |
| `track_positions` | where a long track was left | `queue/mod.rs` |
| `queue_items` | the saved queue: `uid`, `track_id`, `ord` (play order) and `original` (the order before shuffling), sparse keys | `queue/store.rs` |

Deleting a track cascades to every table keyed by it; `queue_items` has
no foreign key, since the queue simply leaves out a track that has gone
when it next loads.

### Settings rows

The `settings` table holds JSON values by key. Each is read leniently:
fields that still parse are kept, the rest take their defaults.

| Key | Holds | Code |
|---|---|---|
| `app` | `AppSettings`: display, playback, output device, visualizer, features, equaliser, effects, recording's format, library options, window, appearance (the theme) | `settings.rs` |
| `library.sort` | the sort and grouping rules and ignored articles | `library/rules.rs` |
| `metadata.services` | which online sources are on, their order per kind of data, "match automatically" | `metadata/settings.rs` |
| `player.queue` | the queue's current item, position, repeat, shuffle and volume (the list is `queue_items`) | `queue/mod.rs` |
| `recording.folder` | the recordings' folder: its path and writable bookmark | `recording.rs` |

Keys and tokens (the Discogs token, the ListenBrainz token) are never in
the database: they are in the macOS keychain under the service "ano-mp
metadata", one entry per source (`metadata/keys.rs`); the settings record
only whether a source has one.

### Migrations and copies

Before running a migration on a file database, `db::open` copies it to
`library.sqlite3.pre-<n>` (`n` the first migration to run) and keeps
the newest two copies; if the copy can't be written, the migration
doesn't run. These copies are what the database repair dialog restores
(`library/recovery.rs`). Never edit a migration that has shipped:
append a new one ([chapter 11](11-recipes.md#a-migration)).

## Files on disk

Paths are for macOS with the bundle identifier `dev.anomp.player`. The
app under `tauri dev` is unsandboxed and uses `~/Library/...` directly;
a built bundle runs in the App Sandbox, and the same paths sit inside
its container, `~/Library/Containers/dev.anomp.player/Data/Library/...`.
The two never share a library. (A bundle check uses the real container,
which holds the owner's library: follow `docs/bundle-checks.md`.)

| What | Where (unsandboxed) | Written by |
|---|---|---|
| The library database, its WAL files, its pre-migration copies | `~/Library/Application Support/dev.anomp.player/library.sqlite3*` | `library/db.rs` |
| A damaged database, kept | the same folder, `library.sqlite3.damaged-<time>` | `library/recovery.rs` |
| A recovery chosen for the next launch | the same folder, `library.recovery.json` | `library/recovery.rs` |
| Downloaded pictures and thumbnails (500 MB budget, least recently used removed first) | `~/Library/Caches/dev.anomp.player/images/` | `metadata/images.rs`, `library/thumbs.rs` |
| The cover of a track-change notification | `~/Library/Caches/dev.anomp.player/notification-cover.jpg` (or `.png`) | `shell/notifications.rs` |
| The webview's own data (its cache, `localStorage`: folded sections and other per-window choices, `anomp.*` keys) | `~/Library/Caches/dev.anomp.player/WebKit/` and WebKit's folders | WebKit; `state/ui.svelte.ts` |
| The log, and two older ones | `~/Library/Logs/dev.anomp.player/ano-mp.log` (a new file past 2 MB) | `logging.rs` |
| Recordings | the folder the user picked in Settings › Recording | `recording.rs`, the core's `Recorder` |
| Exports (data, playlists, themes) | wherever the user saves them | `library/transfer.rs`, `playlists.rs`, `theme.rs` |
| The self-test's scratch files | the temporary folder, removed after | `self_test.rs` |

Everything in `Caches` can be deleted safely: pictures are downloaded
again and thumbnails remade. Deleting `Application Support`'s folder
starts the library from nothing (the user's folders must be added
again).
