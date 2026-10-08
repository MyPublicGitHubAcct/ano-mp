# The Rust backend's types

Every struct, enum and trait in `app/src-tauri/src/`, public or not, in
alphabetical order; test-only code (`#[cfg(test)]` items, and modules
declared under it such as `bindings.rs` and `guide.rs`) is left out. Where
several modules use one name (`Entry`, `Row`, `Error`…), each has its own
entry, told apart by its file. Payloads marked "sent to the UI" are also
TypeScript types in `app/src/lib/generated/ipc.ts`, and settings marked "a
setting" in `generated/settings.ts`, with the same names (generated from
these, never edited by hand). "Helper (C layout)" types mirror the C API's
structs for the FFI, and "Helper (service JSON)" types are a web service's
answer as it is served, parsed into the app's own types. The developer
guide's [chapter 6](../developer-guide/06-rust-backend.md) explains how the
crate fits together.

Back to the [index](README.md).

### `Account`

Enum · [`app/src-tauri/src/metadata/keys.rs`](../../app/src-tauri/src/metadata/keys.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

Whose key is in the keychain: a metadata source's (the Discogs token) or
ListenBrainz's. Keys never go in the settings JSON, and each read is
registered with `logging::keep_secret`.

### `AlbumCard`

Struct, sent to the UI · [`app/src-tauri/src/library/discover.rs`](../../app/src-tauri/src/library/discover.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

An album as cards list it (Recently added, On this day, more in this
genre, Home, favourites, history): id, title, artist and year.

### `AlbumDetails`

Struct, sent to the UI · [`app/src-tauri/src/library/albums.rs`](../../app/src-tauri/src/library/albums.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

An album as its header and the "Find details" dialog show it: its tags
(year, genres, release id), its match with each metadata source, a
description, and where its cover comes from, each labelled with its
source.

### `AlbumFact`

Enum, a setting · [`app/src-tauri/src/settings.rs`](../../app/src-tauri/src/settings.rs)

A fact an album's summary line can give: date, label, country, media and
so on.

### `AlbumFacts`

Struct · [`app/src-tauri/src/metadata/matcher.rs`](../../app/src-tauri/src/metadata/matcher.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

What the library knows about an album from its tags (title, artist, year,
track count and lengths, release id), for scoring releases against it.

### `AlbumHit`

Struct, sent to the UI · [`app/src-tauri/src/library/search.rs`](../../app/src-tauri/src/library/search.rs) · [D2: A search keystroke](../developer-guide/08-flows.md#a-search-keystroke)

An album in search results: title, album artist, year and track count.

### `AlbumIssue`

Struct, sent to the UI · [`app/src-tauri/src/library/health.rs`](../../app/src-tauri/src/library/health.rs)

An album in the health report, and what is wrong with it, in words.

### `AlbumLink`

Struct, sent to the UI · [`app/src-tauri/src/metadata/albums.rs`](../../app/src-tauri/src/metadata/albums.rs) · [D2: Online metadata (the metadata worker)](../developer-guide/09-data.md#online-metadata-the-metadata-worker)

An album's `album_links` row for one source: status, external id, score,
whether the user chose it, and the release (MusicBrainz) or none (Discogs
keeps no details). Automatic matching never replaces a user's pick.

### `AlbumOrder`

Enum, a setting · [`app/src-tauri/src/library/rules.rs`](../../app/src-tauri/src/library/rules.rs)

How albums are ordered where a rule lists them: by title or by year.

### `AlbumPrefs`

Struct, sent to and from the UI · [`app/src-tauri/src/library/prefs.rs`](../../app/src-tauri/src/library/prefs.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

An album's rules for all its tracks (O7): skip, never shuffle (play it
whole), gain offset.

### `AlbumRef`

Helper · [`app/src-tauri/src/library/transfer.rs`](../../app/src-tauri/src/library/transfer.rs)

An album in a data file: title, artist and release id.

### `AlbumSources`

Helper · [`app/src-tauri/src/library/art.rs`](../../app/src-tauri/src/library/art.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

Where the art sources look for an album's or a track's picture: its files,
Cover Art Archive URLs and the folders.

### `AlbumTrack`

Struct, sent to the UI · [`app/src-tauri/src/library/albums.rs`](../../app/src-tauri/src/library/albums.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

A track as an album's list shows it: disc, number, title, length, artist
and its folder.

### `AnalysisConfig`

Struct · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs) · main thread · [D2: The visualizer](../developer-guide/07-frontend.md#the-visualizer)

How the core analyses the audio for the visualizer: bands, waveform length
and frames per second (`anomp_analysis_config`). Set by `visualizer.rs`
from the visualizer's settings.

### `AnalysisFrame`

Struct · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs) · the analysis thread · [D2: The visualizer](../developer-guide/07-frontend.md#the-visualizer)

One analysis of what plays (`anomp_analysis_frame`), borrowed from the
core for the duration of the handler: bands, chroma, levels, waveform,
onset, beat and notes. `visualizer.rs` packs it for the frontend's
channel.

### `AnalysisProgress`

Struct, sent to the UI · [`app/src-tauri/src/library/analysis.rs`](../../app/src-tauri/src/library/analysis.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

The loudness analysis's progress: whether the whole library is being
analysed, tracks done and remaining, and how many couldn't be decoded.

### `AnalysisWorker`

Struct · [`app/src-tauri/src/library/analysis.rs`](../../app/src-tauri/src/library/analysis.rs) · the analysis thread · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

The analysis thread's controls, managed by Tauri: wakes it, queues tracks,
reports progress, and holds the `quitting::Finished` that quitting waits
for. The thread decodes each track once with the core's analyser, never a
placeholder, and stores `track_analysis` and `album_analysis` in one
transaction.

### `AppearanceSettings`

Struct, a setting · [`app/src-tauri/src/theme.rs`](../../app/src-tauri/src/theme.rs) · [D2: Themes and styling](../developer-guide/07-frontend.md#themes-and-styling)

The look of the app (X1): the theme in use, the user's saved themes, and
following the system's contrast setting.

### `AppSettings`

Struct, a setting · [`app/src-tauri/src/settings.rs`](../../app/src-tauri/src/settings.rs) · [D2: Settings and features](../developer-guide/06-rust-backend.md#settings-and-features)

Every setting the app keeps, as one JSON value under `app` in `settings`:
display, playback, output, visualizer, features, equaliser, effects,
recording, library, windows and appearance. Read leniently field by field;
a new setting goes here with a default and a `validate` rule. Its
TypeScript type is generated.

### `Arming`

Helper · [`app/src-tauri/src/queue/model.rs`](../../app/src-tauri/src/queue/model.rs)

The item being opened as the engine's next track, since when, and whether
it is downloading (a placeholder gets longer).

### `Art`

Struct · [`app/src-tauri/src/library/art.rs`](../../app/src-tauri/src/library/art.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

A picture `art::lookup` found: its MIME type and bytes, its source,
whether the user chose it, and its `Origin`, by which thumbnails are
indexed.

### `ArtCache`

Struct · [`app/src-tauri/src/library/art.rs`](../../app/src-tauri/src/library/art.rs) · any thread · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

Art looked up so far, what has none included, full size and as thumbnails,
kept in memory to a size limit (least recently used goes first), managed
by Tauri. Cleared for an album when its picture changes.

### `Article`

Struct, sent to the UI · [`app/src-tauri/src/metadata/wikipedia.rs`](../../app/src-tauri/src/metadata/wikipedia.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

A Wikipedia article's lead as the app keeps it, an artist's biography or
an album's description: title, address, language and paragraphs, stored as
JSON in the link rows. CC BY-SA: always credited where shown.

### `Artist`

Struct, sent to the UI · [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

An artist as the app keeps it: the facts shown on the artist page (type,
area, life span, aliases, related artists) and the links that lead to a
biography, stored as JSON in `artist_links.details`.

### `ArtistAlbum`

Struct, sent to the UI · [`app/src-tauri/src/library/artists.rs`](../../app/src-tauri/src/library/artists.rs)

An album on the artist page: title, album artist, year, release types and
tracks.

### `ArtistCandidate`

Struct, sent to the UI · [`app/src-tauri/src/metadata/artists.rs`](../../app/src-tauri/src/metadata/artists.rs)

An artist offered in "Find artist", with MusicBrainz's score.

### `ArtistHit`

Struct, sent to the UI · [`app/src-tauri/src/library/search.rs`](../../app/src-tauri/src/library/search.rs) · [D2: A search keystroke](../developer-guide/08-flows.md#a-search-keystroke)

An artist in search results, with their track counts.

### `ArtistHit`

Struct · [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

An artist search hit, with MusicBrainz's score.

### `ArtistInfo`

Struct, sent to the UI · [`app/src-tauri/src/metadata/artists.rs`](../../app/src-tauri/src/metadata/artists.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

What the metadata sources know about an artist for the artist page: the
match's status, the `Artist` facts, a biography, and whether the user
chose it.

### `ArtistLink`

Struct · [`app/src-tauri/src/metadata/artists.rs`](../../app/src-tauri/src/metadata/artists.rs) · [D2: Online metadata (the metadata worker)](../developer-guide/09-data.md#online-metadata-the-metadata-worker)

An artist's `artist_links` row for one source; its details are the
source's JSON (an `Artist` for MusicBrainz, an `Article` for Wikipedia).

### `ArtistPage`

Struct, sent to the UI · [`app/src-tauri/src/library/artists.rs`](../../app/src-tauri/src/library/artists.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

The artist page: the artist, their track count, their albums, the albums
of others they appear on, and what the metadata sources know
(`metadata::artists::ArtistInfo`).

### `ArtistRef`

Helper · [`app/src-tauri/src/library/transfer.rs`](../../app/src-tauri/src/library/transfer.rs)

An artist in a data file, by name and id.

### `ArtKey`

Enum · [`app/src-tauri/src/library/art.rs`](../../app/src-tauri/src/library/art.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

Whose art: an album's, or a track's own for a track without an album. Part
of the cache's key and of an `anomp-art` URI.

### `Ask`

Struct · [`app/src-tauri/src/queue/opening.rs`](../../app/src-tauri/src/queue/opening.rs) · main thread · [D2: Loading off the main thread](../developer-guide/04-core.md#loading-off-the-main-thread)

What the queue asks to open: a track, as current or next (with its
crossfade), and the playback and feature settings.

### `Bookmarks`

Trait · [`app/src-tauri/src/library/access.rs`](../../app/src-tauri/src/library/access.rs) · any thread · [D2: Fakes](../developer-guide/12-testing.md#fakes)

Making and resolving bookmarks: the OS's (`System`) or a test's
(`access::testing::FakeBookmarks`). `open_folder` goes through it.

### `BrowsePage`

Struct, sent to the UI · [`app/src-tauri/src/library/browse.rs`](../../app/src-tauri/src/library/browse.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

A page of a browse node: its groups, then its tracks, and the node's
total, so the virtual list can page through a large library.

### `CacheEntries`

Helper · [`app/src-tauri/src/library/art.rs`](../../app/src-tauri/src/library/art.rs)

The `ArtCache`'s entries, their size and a clock for least recently used.

### `CallContext`

Struct · [`app/src-tauri/src/metadata/jobs.rs`](../../app/src-tauri/src/metadata/jobs.rs) · the metadata worker · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

What a `worker::call` closure works with on the worker thread: the client,
the worker's connection and the image cache, and the albums and artists
whose details or art it changed.

### `Candidate`

Struct · [`app/src-tauri/src/metadata/matcher.rs`](../../app/src-tauri/src/metadata/matcher.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

A scored release.

### `Candidate`

Helper · [`app/src-tauri/src/metadata/outside.rs`](../../app/src-tauri/src/metadata/outside.rs)

An outside artist as it is scored, with what each reason added.

### `Catalog`

Struct · [`app/src-tauri/src/library/similar.rs`](../../app/src-tauri/src/library/similar.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

Every track's `TrackFacts`, read once per recommendation, with the names
of its genres and labels and the artists' links.

### `Changes`

Struct · [`app/src-tauri/src/library/availability.rs`](../../app/src-tauri/src/library/availability.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

Folders whose state changed: those readable again (to rescan) and those
that can't be read any more.

### `Chapter`

Struct · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs) · any thread · [D2: Tags: TagReader](../developer-guide/04-core.md#tags-tagreader)

A chapter of a file (`anomp_chapter`): start, end (`None` for the end of
the file) and title. The scanner makes each one a track.

### `Checks`

Helper · [`app/src-tauri/src/shell/menu.rs`](../../app/src-tauri/src/shell/menu.rs)

What the menu's checks show, to update only what changed.

### `ChosenCover`

Struct, sent to the UI · [`app/src-tauri/src/metadata/commands.rs`](../../app/src-tauri/src/metadata/commands.rs)

The picture the user chose for an album: its source and reference.

### `ClearOnDrop`

Helper · [`app/src-tauri/src/library/commands.rs`](../../app/src-tauri/src/library/commands.rs)

Clears the "scanning" flag when a scan ends, however it ends.

### `Client`

Struct · [`app/src-tauri/src/metadata/http.rs`](../../app/src-tauri/src/metadata/http.rs) · the metadata worker and others · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

The only way the app calls a web service: rate-limits per host, retries
when a service is busy, backs off from an unreachable host, and caches
JSON (on disk with `get_json`, in memory only and briefly with
`get_json_fresh`, for Discogs). Built over a `Transport` and a `Clock`.

### `Clock`

Trait · [`app/src-tauri/src/metadata/http.rs`](../../app/src-tauri/src/metadata/http.rs) · [D2: Fakes](../developer-guide/12-testing.md#fakes)

Time as `Client` sees it (now, and sleeping), so tests run the rate
limiter and backoff without waiting.

### `CollectionChanged`

Enum, sent to the UI · [`app/src-tauri/src/collection.rs`](../../app/src-tauri/src/collection.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

What the `collection-changed` event says changed: playlists, favourites,
ratings, or everything (an import). Every view showing it reloads.

### `Command`

Enum · [`app/src-tauri/src/remote/mod.rs`](../../app/src-tauri/src/remote/mod.rs) · [D2: The LAN remote](../developer-guide/06-rust-backend.md#the-lan-remote)

A command from the remote page: toggle, next, previous, seek, volume or
jump to a queue item.

### `Condition`

Enum, sent to and from the UI · [`app/src-tauri/src/library/smart.rs`](../../app/src-tauri/src/library/smart.rs)

One smart playlist condition: genre, year, format, added within, heart,
rating, play count, not played for, artist and so on. Each picks a fixed
SQL fragment, its values bound.

### `Copy`

Struct · [`app/src-tauri/src/library/db.rs`](../../app/src-tauri/src/library/db.rs) · [D2: Migrations and copies](../developer-guide/09-data.md#migrations-and-copies)

A copy of the database written before a migration
(`library.sqlite3.pre-<n>`, H10): its path and the migration. A restore
uses the newest.

### `CopyInfo`

Struct, sent to the UI · [`app/src-tauri/src/library/recovery.rs`](../../app/src-tauri/src/library/recovery.rs)

A copy of the database as the repair dialog shows it: its file name and
when it was written.

### `CoverAlbum`

Struct, sent to the UI · [`app/src-tauri/src/library/covers.rs`](../../app/src-tauri/src/library/covers.rs)

An album on the cover wall: id, title, artist and year.

### `CoverBasis`

Enum, sent to the UI · [`app/src-tauri/src/library/covers.rs`](../../app/src-tauri/src/library/covers.rs)

What the visualizer's cover wall gathers albums by: the current track's
year or artist.

### `CoverCandidate`

Struct, sent to the UI · [`app/src-tauri/src/library/art.rs`](../../app/src-tauri/src/library/art.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

A picture the "Choose cover" dialog offers: its source, what
`album_art.reference` would store, a label and a preview.

### `CoverChoices`

Struct, sent to the UI · [`app/src-tauri/src/metadata/commands.rs`](../../app/src-tauri/src/metadata/commands.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

What "Choose cover" offers: the current pick, and each album-art source's
candidates in the configured order.

### `CoverSource`

Struct, sent to the UI · [`app/src-tauri/src/library/albums.rs`](../../app/src-tauri/src/library/albums.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

Where the cover shown comes from: its source, and whether the user chose
it.

### `CoverWall`

Struct, sent to the UI · [`app/src-tauri/src/library/covers.rs`](../../app/src-tauri/src/library/covers.rs) · [D2: The visualizer](../developer-guide/07-frontend.md#the-visualizer)

The albums the visualizer's cover wall shows around the current track,
with their basis and label. Only ids and names; the covers come from
`anomp-art`.

### `Credit`

Helper · [`app/src-tauri/src/metadata/artists.rs`](../../app/src-tauri/src/metadata/artists.rs)

A track's credited artists and their MusicBrainz ids, read from stored
JSON to match an artist.

### `Credit`

Struct, sent to the UI · [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs)

A credit on a release, such as "Producer" by a name.

### `CrossfeedLevel`

Enum, a setting · [`app/src-tauri/src/settings.rs`](../../app/src-tauri/src/settings.rs)

The crossfeed's level: off, light, medium or strong (`ANOMP_CROSSFEED_*`).

### `CueFile`

Struct · [`app/src-tauri/src/library/cue.rs`](../../app/src-tauri/src/library/cue.rs)

The tracks in one audio file a cue sheet names (as written, often the WAV
it was ripped to rather than the FLAC it became).

### `CuePart`

Struct · [`app/src-tauri/src/library/cue.rs`](../../app/src-tauri/src/library/cue.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

A track of an audio file from its cue sheet, with where it ends (`None` at
the end of the file): what the scanner turns into a row.

### `CueSheet`

Struct · [`app/src-tauri/src/library/cue.rs`](../../app/src-tauri/src/library/cue.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

A parsed cue sheet (O5), from a `.cue` file next to the audio or a
CUESHEET tag: the album's title and performer and its files' tracks. Only
what playing the tracks needs is read.

### `CueTrack`

Struct · [`app/src-tauri/src/library/cue.rs`](../../app/src-tauri/src/library/cue.rs)

A cue sheet track: number, title, performer and where it starts (INDEX
01).

### `Current`

Enum, sent to the UI · [`app/src-tauri/src/metadata/jobs.rs`](../../app/src-tauri/src/metadata/jobs.rs)

The album or artist the worker is working on, with its name.

### `DataImport`

Struct, sent to the UI · [`app/src-tauri/src/collection.rs`](../../app/src-tauri/src/collection.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

What importing a data file (F20) did: the `transfer::ImportReport`, and
whether the settings and the queue were replaced.

### `DbCheck`

Enum, sent to the UI · [`app/src-tauri/src/library/recovery.rs`](../../app/src-tauri/src/library/recovery.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

What the launch check (`PRAGMA quick_check`) found: running, ok, or failed
with SQLite's report and the copy a restore would use. The database repair
dialog shows it.

### `Decision`

Enum · [`app/src-tauri/src/metadata/artists.rs`](../../app/src-tauri/src/metadata/artists.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

What an artist search decided: matched, review (none clearly the one), or
no match.

### `Decision`

Enum · [`app/src-tauri/src/metadata/matcher.rs`](../../app/src-tauri/src/metadata/matcher.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

What scoring an album's candidates decided: matched, review, or no match.

### `Density`

Enum, a setting · [`app/src-tauri/src/theme.rs`](../../app/src-tauri/src/theme.rs)

How much room lists and controls take: compact, regular or roomy.

### `DetailedLog`

Struct · [`app/src-tauri/src/logging.rs`](../../app/src-tauri/src/logging.rs) · any thread · [D2: The logs](../developer-guide/10-troubleshooting.md#the-logs)

"Detailed logging"'s file (`ano-mp-detailed.log`): every line, debug
included and redacted, kept to a size with one old copy, until switched
off or the next launch deletes it.

### `DeviceInfo`

Struct, sent to the UI · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs) · main thread · [D2: The engine on the main thread](../developer-guide/06-rust-backend.md#the-engine-on-the-main-thread)

The open output device: name, buffer size, the sizes it offers, sample
rate and latency (`anomp_device_info`). Part of `OutputStatus` and
`SignalPathPayload`.

### `Discography`

Struct, sent to the UI · [`app/src-tauri/src/metadata/discography.rs`](../../app/src-tauri/src/metadata/discography.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

An artist's discography on MusicBrainz less what the library has: the
MusicBrainz artist, and the release groups the library doesn't hold,
oldest first. Shown on the artist's discography page.

### `DiscogsReleases`

Struct · [`app/src-tauri/src/metadata/discogs.rs`](../../app/src-tauri/src/metadata/discogs.rs) · the metadata worker · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

Discogs as a `ReleaseSource`, with the user's token. Its terms shape it:
only the match is stored, its data is fetched fresh
(`Client::get_json_fresh`) and never kept, its images are never fetched,
and its credit is shown beside its data.

### `DisplaySettings`

Struct, a setting · [`app/src-tauri/src/settings.rs`](../../app/src-tauri/src/settings.rs)

What lists and pages show: track columns, album facts, descriptions, and
Now Playing in the sidebar.

### `DockMenu`

Struct · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs) · main thread · [D2: The shell](../developer-guide/06-rust-backend.md#the-shell)

The Dock icon's menu (`anomp_dock_menu`; macOS, nothing elsewhere): its
items and a handler for the one chosen. One exists at a time. Filled by
`shell/dock.rs`.

### `DroppedPaths`

Struct, sent to the UI · [`app/src-tauri/src/shell/mod.rs`](../../app/src-tauri/src/shell/mod.rs) · [D2: The shell](../developer-guide/06-rust-backend.md#the-shell)

Paths dropped on the window, sorted into folders (offered as library
folders) and files (to play).

### `DuplicateGroup`

Struct, sent to the UI · [`app/src-tauri/src/library/health.rs`](../../app/src-tauri/src/library/health.rs)

Tracks that look like copies of one another, and why.

### `Edit`

Enum, sent to the UI · [`app/src-tauri/src/queue/model.rs`](../../app/src-tauri/src/queue/model.rs) · main thread · [D2: The queue](../developer-guide/06-rust-backend.md#the-queue)

A change to the queue's list (H16): items inserted, removed, moved, or
updated when their tags changed; indices are into the list as each edit
finds it. The UI applies the same edits by `listVersion`
(`lib/queueEdits.ts`), and `queue/store.rs` writes them as rows; a model
change must log one or a reset.

### `EffectInfo`

Struct, sent to the UI · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs) · [D2: On the Rust and UI side](../developer-guide/05-effects.md#on-the-rust-and-ui-side)

One of the core's effects (`anomp_effect_info`): its number, id (its key
in the settings), place in the chain, default mix and parameters. The
Effects settings draw their controls from it.

### `EffectParam`

Struct, sent to the UI · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs) · [D2: On the Rust and UI side](../developer-guide/05-effects.md#on-the-rust-and-ui-side)

One of an effect's parameters (`anomp_effect_param`): id, unit, range,
default and scale. Read from the core's catalogue by `effects.rs`, which
validates `EffectSettings` against it.

### `EffectSettings`

Struct, a setting · [`app/src-tauri/src/effects.rs`](../../app/src-tauri/src/effects.rs) · [D2: On the Rust and UI side](../developer-guide/05-effects.md#on-the-rust-and-ui-side)

One effect: on or off, its wet/dry mix and its parameters by id, each
within the core's range.

### `EffectsSettings`

Struct, a setting · [`app/src-tauri/src/effects.rs`](../../app/src-tauri/src/effects.rs) · [D2: On the Rust and UI side](../developer-guide/05-effects.md#on-the-rust-and-ui-side)

Each effect's `EffectSettings`, by its id in the core (reverb, chorus,
freeze, echo, flanger, phaser, tremolo, lo-fi): the `effects` part of
`AppSettings`. Validated against the core's catalogue and applied to the
engine by `effects.rs`. A new effect gets a field here.

### `EffectsStatus`

Struct, sent to the UI · [`app/src-tauri/src/effects.rs`](../../app/src-tauri/src/effects.rs) · main thread · [D2: On the Rust and UI side](../developer-guide/05-effects.md#on-the-rust-and-ui-side)

What the effects are doing now: whether the spectral freeze holds, and
which effects are on or still ringing out.

### `EffectUnit`

Enum, sent to the UI · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs) · [D2: On the Rust and UI side](../developer-guide/05-effects.md#on-the-rust-and-ui-side)

How an effect parameter's value reads: ratio, hertz, milliseconds, seconds
or bits (`ANOMP_EFFECT_UNIT_*`).

### `Engine`

Struct · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs) · main thread · [D2: The engine on the main thread](../developer-guide/06-rust-backend.md#the-engine-on-the-main-thread)

The audio engine (`anomp_engine`): devices, loading (also asynchronously),
transport, volume, loops and tempo, crossfeed, equaliser, effects,
recording and the visualizer's analysis. Created, used and dropped on the
main thread, which `!Send` and `!Sync` enforce; its event handler may call
it back. Lives in `audio.rs`'s thread-local; reached through
`with_engine`.

### `EnginePlayer`

Struct · [`app/src-tauri/src/queue/mod.rs`](../../app/src-tauri/src/queue/mod.rs) · main thread · [D2: The queue](../developer-guide/06-rust-backend.md#the-queue)

The real `Player`: the engine as the queue drives it, opening library
tracks through their folder's bookmark as `library::playback` says, off
the main thread (`opening`).

### `Entry`

Helper · [`app/src-tauri/src/metadata/images.rs`](../../app/src-tauri/src/metadata/images.rs)

A cached picture's file, its size and when it was last used.

### `Entry`

Helper · [`app/src-tauri/src/metadata/jobs.rs`](../../app/src-tauri/src/metadata/jobs.rs)

A queued job: its priority, the highest automatic priority it was queued
at, the replies waiting on it, and its place.

### `Entry`

Helper · [`app/src-tauri/src/queue/store.rs`](../../app/src-tauri/src/queue/store.rs) · [D2: The queue](../developer-guide/06-rust-backend.md#the-queue)

One queue item as the store knows it: its uid, whether it is saved (a
library track), and its sparse order keys.

### `Entry`

Helper · [`app/src-tauri/src/recording.rs`](../../app/src-tauri/src/recording.rs)

A track that became current while recording: its first mark and what the
cue sheet calls it.

### `EqualiserProfile`

Struct, a setting · [`app/src-tauri/src/settings.rs`](../../app/src-tauri/src/settings.rs)

Ten band gains and a preamp in dB, and the preset they came from.

### `EqualiserSettings`

Struct, a setting · [`app/src-tauri/src/settings.rs`](../../app/src-tauri/src/settings.rs)

The equaliser (F15): on or off, a profile for speakers and one for
headphones, and whether to follow the output.

### `Error`

Enum · [`app/src-tauri/src/library/mod.rs`](../../app/src-tauri/src/library/mod.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

The library's error: a database error, or a problem to show the user as is
(an unusable folder, say). Commands turn it into a string or a coded
error.

### `Error`

Enum · [`app/src-tauri/src/metadata/mod.rs`](../../app/src-tauri/src/metadata/mod.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

The metadata code's error: a database error, offline (the service can't be
reached, or the client is backing off), an HTTP error status, or an answer
that makes no sense. Offline leaves a job queued.

### `Event`

Enum · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs) · main thread · [D2: The engine on the main thread](../developer-guide/06-rust-backend.md#the-engine-on-the-main-thread)

What the engine reports through its callback (`anomp_event`): device
changed, state changed, position, track ended, load finished, recording
failed. `audio.rs` hands each to the queue, the history and the frontend.

### `Events`

Trait · [`app/src-tauri/src/library/analysis.rs`](../../app/src-tauri/src/library/analysis.rs) · the analysis thread · [D2: Fakes](../developer-guide/12-testing.md#fakes)

Where the analysis thread reports (tracks analysed, progress): events in
the app, kept in a list by tests.

### `ExternalFile`

Struct · [`app/src-tauri/src/library/external.rs`](../../app/src-tauri/src/library/external.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

A file opened from outside the library (Open With, the Dock, a drop),
known to the queue by a negative id for this session only: its path,
ReplayGain, sample rate and tags.

### `Facts`

Struct · [`app/src-tauri/src/diagnostics.rs`](../../app/src-tauri/src/diagnostics.rs) · [D2: "Copy diagnostics"](../developer-guide/10-troubleshooting.md#copy-diagnostics)

What "Copy diagnostics" says: versions, the OS, the output device, library
counts, folder states as counts, the switches and sources that are on,
database facts and whether detailed logging is on. No paths or titles;
adding to it is an owner decision.

### `FavouriteArtist`

Struct, sent to the UI · [`app/src-tauri/src/library/marks.rs`](../../app/src-tauri/src/library/marks.rs)

An artist the user hearted, with their track count and when.

### `Favourites`

Struct, sent to the UI · [`app/src-tauri/src/library/marks.rs`](../../app/src-tauri/src/library/marks.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

Everything with a heart, newest first: the Favourites view's tracks,
albums and artists.

### `Favourites`

Helper · [`app/src-tauri/src/library/transfer.rs`](../../app/src-tauri/src/library/transfer.rs)

A data file's hearts, by kind: each item's index in the file's lists, and
when it was hearted.

### `FeatureSettings`

Struct, a setting · [`app/src-tauri/src/settings.rs`](../../app/src-tauri/src/settings.rs) · [D2: A feature switch](../developer-guide/11-recipes.md#a-feature-switch)

The optional features' switches (O1–O19 and later): local, cheap ones on
by default; those that cost a lot, change what is heard, go online or
listen on the network off. Each feature checks its switch where it acts.

### `Fetched`

Enum · [`app/src-tauri/src/metadata/coverartarchive.rs`](../../app/src-tauri/src/metadata/coverartarchive.rs) · the metadata worker

What fetching a cover from the Cover Art Archive did: not linked, cached
already, downloaded, or not there.

### `Fetched`

Enum · [`app/src-tauri/src/metadata/wikipedia.rs`](../../app/src-tauri/src/metadata/wikipedia.rs) · the metadata worker

What fetching a biography or description did: not linked, found, or no
English article.

### `Field`

Enum · [`app/src-tauri/src/library/search.rs`](../../app/src-tauri/src/library/search.rs) · [D2: A search keystroke](../developer-guide/08-flows.md#a-search-keystroke)

A field a search term can be limited to (`artist:`, `album:`, `title:`,
`composer:`).

### `FileAnalysis`

Struct · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs) · any thread · [D2: Analysing files: FileAnalyser](../developer-guide/04-core.md#analysing-files-fileanalyser)

What one pass over a file (or part of one) measures
(`anomp_file_analysis`): loudness and its histogram, peaks, silences, end
levels, cutoff and the envelope. Made by `analyse_file`; stored by
`library/analysis.rs`.

### `FileInfo`

Struct, sent to the UI · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs) · any thread · [D2: Tags: TagReader](../developer-guide/04-core.md#tags-tagreader)

Everything a file says about itself, for Get Info (`anomp_file_info`):
every tag field, every picture, the tag types and the format as the
decoder sees it. Read by `read_file_info`; shown by `library/info.rs`'s
command.

### `FilePicture`

Struct, sent to the UI · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs) · any thread · [D2: Tags: TagReader](../developer-guide/04-core.md#tags-tagreader)

One embedded picture in `FileInfo`: its type, MIME type, description and
bytes.

### `Filter`

Struct, from the UI · [`app/src-tauri/src/library/browse.rs`](../../app/src-tauri/src/library/browse.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

What a browse, or playing a node, keeps of the library: only favourites,
for now.

### `Finished`

Struct · [`app/src-tauri/src/quitting.rs`](../../app/src-tauri/src/quitting.rs) · main thread · [D2: Quitting](../developer-guide/08-flows.md#quitting)

Says when the thread holding the matching `Running` has ended. Quitting
waits for each, up to `quitting::WAIT` for all.

### `Folder`

Struct, sent to the UI · [`app/src-tauri/src/library/mod.rs`](../../app/src-tauri/src/library/mod.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

A library folder as Settings › Library folders lists it: id, path, track
count, placeholders not read, and its state.

### `FolderAccess`

Struct · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs) · any thread · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

A resolved bookmark (`anomp_folder_access`): its folder can be read until
this is dropped. Gives the folder's path now and whether the bookmark is
stale. Made by `start_folder_access`; `library::access::System` wraps it
for the rest of the app.

### `Folders`

Helper · [`app/src-tauri/src/library/art.rs`](../../app/src-tauri/src/library/art.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

The library folders of an album's files, opened as they're needed and held
until the lookup ends; one that can't be opened is skipped.

### `FolderScan`

Helper · [`app/src-tauri/src/library/scanner.rs`](../../app/src-tauri/src/library/scanner.rs) · [D2: Adding a folder and its scan](../developer-guide/08-flows.md#adding-a-folder-and-its-scan)

A folder walked and what its scan has found so far, holding the folder
open until the scan ends.

### `FolderState`

Enum, sent to the UI · [`app/src-tauri/src/library/access.rs`](../../app/src-tauri/src/library/access.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

Whether a library folder can be read now, and if not why: available,
missing, empty, mostly gone, in the Trash, or no permission (H22).
`of_error` tells a "not now" from a broken file. Kept by
`library::availability`.

### `FolderStates`

Struct · [`app/src-tauri/src/library/availability.rs`](../../app/src-tauri/src/library/availability.rs) · any thread · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

Each library folder's `FolderStatus`, by id, managed by Tauri (a folder
not checked yet counts as there). Checked at launch, after each scan and
when a volume comes or goes; never stored.

### `FolderStatus`

Struct, sent to the UI · [`app/src-tauri/src/library/access.rs`](../../app/src-tauri/src/library/access.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

A folder's `FolderState`, with the system's words for it when there are
any.

### `Font`

Enum, a setting · [`app/src-tauri/src/theme.rs`](../../app/src-tauri/src/theme.rs)

The fonts a theme can use; the frontend maps each to a font stack.

### `Found`

Helper · [`app/src-tauri/src/library/scanner.rs`](../../app/src-tauri/src/library/scanner.rs)

An audio file found by the walk: its path, size and modification time.

### `Gone`

Enum · [`app/src-tauri/src/coded.rs`](../../app/src-tauri/src/coded.rs) · [D2: Text](../developer-guide/07-frontend.md#text)

What went missing from the library while the UI still showed it (a track,
album, artist or playlist): the `gone` coded error's `kind`, so the UI can
say which.

### `Grain`

Enum · [`app/src-tauri/src/library/similar.rs`](../../app/src-tauri/src/library/similar.rs)

What is recommended: tracks, albums or artists.

### `Group`

Struct, sent to the UI · [`app/src-tauri/src/library/browse.rs`](../../app/src-tauri/src/library/browse.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

A group in a browse node: its key (none for "Unknown artist" and the
like), name, track count, and for an album its artist and year.

### `GroupKey`

Enum, sent to the UI · [`app/src-tauri/src/library/browse.rs`](../../app/src-tauri/src/library/browse.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

Identifies a group within its level of a sort rule: a number (an artist,
album or folder id, or a year) or text (a genre, or a folder's name). A
browse path is a list of them.

### `Groups`

Helper · [`app/src-tauri/src/library/browse.rs`](../../app/src-tauri/src/library/browse.rs)

The query for a node's groups, and the name of the group with no key.

### `HealthReport`

Struct, sent to the UI · [`app/src-tauri/src/library/health.rs`](../../app/src-tauri/src/library/health.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

The library health report (O4): undecodable, truncated and suspected
transcoded tracks, albums with problems, likely duplicates, and how much
of the library is analysed.

### `HealthTrack`

Struct, sent to the UI · [`app/src-tauri/src/library/health.rs`](../../app/src-tauri/src/library/health.rs)

A track in the health report, with its absolute path so the UI can reveal
it in Finder.

### `Highlights`

Struct, sent to the UI · [`app/src-tauri/src/history/views.rs`](../../app/src-tauri/src/history/views.rs) · [D2: History and ListenBrainz](../developer-guide/06-rust-backend.md#history-and-listenbrainz)

The history page's highlights: albums played often but not for a year,
albums played a year ago today, and a few never played.

### `History`

Struct · [`app/src-tauri/src/history/mod.rs`](../../app/src-tauri/src/history/mod.rs) · the history thread · [D2: History and ListenBrainz](../developer-guide/06-rust-backend.md#history-and-listenbrainz)

Writes plays into `plays`, and sends listens to ListenBrainz, on a thread
of its own, managed by Tauri. Quitting waits for it
(`quitting::Finished`), and writes what it left if it is stuck in a
request.

### `Host`

Trait · [`app/src-tauri/src/history/mod.rs`](../../app/src-tauri/src/history/mod.rs) · the history thread · [D2: Fakes](../developer-guide/12-testing.md#fakes)

What the history thread needs from the app (to say a play was recorded,
and whether ListenBrainz is on); tests use a fake.

### `Host`

Trait · [`app/src-tauri/src/metadata/jobs.rs`](../../app/src-tauri/src/metadata/jobs.rs) · the metadata worker · [D2: Fakes](../developer-guide/12-testing.md#fakes)

Where the worker reports (art changed, metadata changed, progress): the
app's `TauriHost`, or a fake in tests.

### `HostState`

Helper · [`app/src-tauri/src/metadata/http.rs`](../../app/src-tauri/src/metadata/http.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

One host's rate-limit state: when the next request may go, and until when
it is considered unreachable.

### `Image`

Struct · [`app/src-tauri/src/metadata/coverartarchive.rs`](../../app/src-tauri/src/metadata/coverartarchive.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

A picture in a release's Cover Art Archive listing: its id, types (front,
back, booklet…), whether it is the chosen front, and a comment. "Choose
cover" offers them.

### `ImageCache`

Struct · [`app/src-tauri/src/metadata/images.rs`](../../app/src-tauri/src/metadata/images.rs) · any thread · [D2: Files on disk](../developer-guide/09-data.md#files-on-disk)

Pictures downloaded from services, and thumbnails, as files in the app's
cache folder named by SHA-256, beyond a size budget least recently used
first out. Losing it is harmless.

### `ImportReport`

Struct, sent to the UI · [`app/src-tauri/src/library/playlists.rs`](../../app/src-tauri/src/library/playlists.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

What importing an M3U or M3U8 playlist did: the playlist made, entries
added, and those the library hasn't got.

### `ImportReport`

Struct, sent to the UI · [`app/src-tauri/src/library/transfer.rs`](../../app/src-tauri/src/library/transfer.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

What importing a data file (F20) found and did: how many of its tracks,
albums and artists the library has, and what was restored.

### `InEngine`

Helper · [`app/src-tauri/src/queue/opening.rs`](../../app/src-tauri/src/queue/opening.rs)

A request the engine is opening: whether it is the current track, the rate
to switch the device to, and its folder, held open until the engine is
done.

### `Inner`

Helper · [`app/src-tauri/src/updates.rs`](../../app/src-tauri/src/updates.rs)

The checker's state: stop, the last check and its result, the newest
version announced.

### `Item`

Struct, sent to the UI · [`app/src-tauri/src/queue/model.rs`](../../app/src-tauri/src/queue/model.rs) · [D2: The queue](../developer-guide/06-rust-backend.md#the-queue)

A queue item: its uid (an item, not a track: a track can be queued twice)
and its `TrackInfo`.

### `Job`

Enum · [`app/src-tauri/src/metadata/jobs.rs`](../../app/src-tauri/src/metadata/jobs.rs) · the metadata worker · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

One piece of the metadata worker's work: match an album, fetch its cover
or description, check its Discogs match, match an artist, or fetch a
biography, each followed by what comes after it.

### `Jobs`

Struct · [`app/src-tauri/src/metadata/jobs.rs`](../../app/src-tauri/src/metadata/jobs.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

The worker's queue of jobs, by priority and order, with the albums and
artists that have jobs pending.

### `Kind`

Enum, a setting · [`app/src-tauri/src/metadata/settings.rs`](../../app/src-tauri/src/metadata/settings.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

A kind of data sources supply: album details (`Release`), album art, album
descriptions and artist biographies. TypeScript calls it `MetadataKind`.

### `Known`

Helper · [`app/src-tauri/src/library/scanner.rs`](../../app/src-tauri/src/library/scanner.rs)

A track already in the library, one part of its file: id, size and time
when last seen, and whether it was a placeholder.

### `Label`

Struct, sent to the UI · [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs)

A label and catalogue number on a release.

### `Level`

Enum, a setting · [`app/src-tauri/src/library/rules.rs`](../../app/src-tauri/src/library/rules.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

What tracks are grouped by at one level of a sort rule: album artist,
artist, album, genre, year, folder and so on.

### `LibrarySettings`

Struct, a setting · [`app/src-tauri/src/settings.rs`](../../app/src-tauri/src/settings.rs)

Keeping the library in step with the disk (F9): rescan at launch, and
watch folders.

### `LibraryState`

Struct · [`app/src-tauri/src/library/commands.rs`](../../app/src-tauri/src/library/commands.rs) · any thread · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

The library as Tauri manages it: the database's path, the shared
connection for short queries, whether a scan runs, the art cache and the
image cache. Commands borrow it; scans and long queries open connections
of their own (through `db`). Never hold its connection while waiting for
the main thread.

### `Links`

Helper · [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs)

An entity's current links to other sites, of the kinds the app follows
(Wikidata, Wikipedia, Discogs, homepage, Bandcamp).

### `LinkStatus`

Enum, sent to the UI · [`app/src-tauri/src/metadata/albums.rs`](../../app/src-tauri/src/metadata/albums.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

A link row's status: matched, to review (candidates, none clear), or none
found.

### `ListenBrainzStatus`

Struct, sent to the UI · [`app/src-tauri/src/history/listenbrainz.rs`](../../app/src-tauri/src/history/listenbrainz.rs) · [D2: History and ListenBrainz](../developer-guide/06-rust-backend.md#history-and-listenbrainz)

What Settings show about ListenBrainz: whether a token is set, how many
listens wait, and why the last attempt failed.

### `Listening`

Helper · [`app/src-tauri/src/history/mod.rs`](../../app/src-tauri/src/history/mod.rs) · main thread

The track playing as the `Tracker` follows it: which queue item and track,
when it started, its length and how much has been played.

### `Listing`

Helper · [`app/src-tauri/src/library/browse.rs`](../../app/src-tauri/src/library/browse.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

A query for a node's groups or tracks, assembled from fixed SQL fragments,
with an order that ends on a unique term so pages neither repeat nor skip
rows.

### `ListLog`

Enum · [`app/src-tauri/src/queue/model.rs`](../../app/src-tauri/src/queue/model.rs)

The list's changes since one side (the UI, the store) last took them:
none, edits, or a reset to send or write whole.

### `Loaded`

Struct · [`app/src-tauri/src/queue/store.rs`](../../app/src-tauri/src/queue/store.rs) · [D2: The queue](../developer-guide/06-rust-backend.md#the-queue)

What the store reads back at launch: (uid, track id) in play order, and
the order before shuffling.

### `Loading`

Helper · [`app/src-tauri/src/queue/model.rs`](../../app/src-tauri/src/queue/model.rs) · [D2: Loading off the main thread](../developer-guide/04-core.md#loading-off-the-main-thread)

The current item being opened: its request, whether it plays once open,
and since when (for the timeout).

### `LoadResult`

Enum · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs) · main thread · [D2: Loading off the main thread](../developer-guide/04-core.md#loading-off-the-main-thread)

How an asynchronous load ended: loaded, failed with a message, or
cancelled (`ANOMP_LOAD_*`). Reported in `Event::LoadFinished`;
`queue/opening.rs`'s `engine_finished` acts on it, ignoring a
cancellation and passing the outcome on to `Queue::load_finished`.

### `LyricLine`

Struct, sent to the UI · [`app/src-tauri/src/library/lyrics.rs`](../../app/src-tauri/src/library/lyrics.rs)

A timed line of lyrics: seconds into the track, and its text.

### `Lyrics`

Struct, sent to the UI · [`app/src-tauri/src/library/lyrics.rs`](../../app/src-tauri/src/library/lyrics.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

A track's lyrics (O13), from a `.lrc` file next to it or the tags: timed
lines, or plain text, and where they came from. Read when shown, never
stored.

### `MarkKind`

Enum, from the UI · [`app/src-tauri/src/library/marks.rs`](../../app/src-tauri/src/library/marks.rs)

What a heart is on: a track, an album or an artist.

### `Media`

Helper · [`app/src-tauri/src/media.rs`](../../app/src-tauri/src/media.rs) · main thread · [D2: The shell](../developer-guide/06-rust-backend.md#the-shell)

The media controls and their `NowPlaying`, in a main-thread thread-local
like the engine.

### `MediaCommand`

Enum · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs) · main thread · [D2: Platform code](../developer-guide/04-core.md#platform-code)

A command from the OS's media controls: play, pause, toggle, next,
previous or seek to a position (`ANOMP_MEDIA_*`). `media.rs` routes it to
the queue.

### `MediaControls`

Struct · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs) · main thread · [D2: Platform code](../developer-guide/04-core.md#platform-code)

The OS's media controls (`anomp_media_controls`): publishes the track,
playback, artwork and navigation, and passes the OS's commands to a
handler. Main thread only. `media.rs` holds it behind its `Publisher`
trait.

### `MenuItem`

Struct · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs) · main thread · [D2: The shell](../developer-guide/06-rust-backend.md#the-shell)

An item of the Dock menu (`anomp_menu_item`): id, title (empty for a
separator), enabled and checked.

### `MenuState`

Struct · [`app/src-tauri/src/shell/menu.rs`](../../app/src-tauri/src/shell/menu.rs) · main thread · [D2: The shell](../developer-guide/06-rust-backend.md#the-shell)

The menu bar items whose text or check follows the queue and player (play,
shuffle, repeat, stop after, the sleep timer, recording).

### `Message`

Enum · [`app/src-tauri/src/history/mod.rs`](../../app/src-tauri/src/history/mod.rs) · main thread to the history thread · [D2: History and ListenBrainz](../developer-guide/06-rust-backend.md#history-and-listenbrainz)

What the `Tracker` tells the history thread: a play that counted, or a
listened time to update.

### `MetadataChanged`

Struct, sent to the UI · [`app/src-tauri/src/metadata/jobs.rs`](../../app/src-tauri/src/metadata/jobs.rs) · [D2: An album's details arriving from MusicBrainz](../developer-guide/08-flows.md#an-albums-details-arriving-from-musicbrainz)

The albums and artists whose details or art changed, as the
`metadata-changed` event sends them; views showing them reload.

### `MetadataSettings`

Struct, sent to the UI · [`app/src-tauri/src/metadata/commands.rs`](../../app/src-tauri/src/metadata/commands.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

Settings › Services' data: every source this version knows (`SourceInfo`)
and the user's `ServiceSettings`.

### `MetadataWorker`

Struct · [`app/src-tauri/src/metadata/worker.rs`](../../app/src-tauri/src/metadata/worker.rs) · any thread · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

The metadata worker as Tauri manages it: the `Shared` queue other threads
post to, the thread's `quitting::Finished`, and the album last queued as
playing. The thread owns the `Client` and a connection of its own;
`worker::call` runs a command's work there.

### `MissingFile`

Helper · [`app/src-tauri/src/library/scanner.rs`](../../app/src-tauri/src/library/scanner.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

A file whose tracks weren't found where they were, as the library
remembers it, for matching against new files to keep track ids when files
move (F10).

### `MissingPool`

Helper · [`app/src-tauri/src/library/scanner.rs`](../../app/src-tauri/src/library/scanner.rs)

The files the walks didn't find, by size, for new files to match.

### `MusicBrainzIds`

Struct, sent to the UI · [`app/src-tauri/src/library/info.rs`](../../app/src-tauri/src/library/info.rs)

A track's MusicBrainz ids from its tags, as MusicBrainz's pages take them,
for Get Info's links.

### `MusicBrainzReleases`

Struct · [`app/src-tauri/src/metadata/albums.rs`](../../app/src-tauri/src/metadata/albums.rs) · the metadata worker · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

MusicBrainz as a `ReleaseSource`, whose release is kept with the link.

### `Node`

Helper · [`app/src-tauri/src/library/browse.rs`](../../app/src-tauri/src/library/browse.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

A browse node's queries: its groups and its tracks (either may be absent),
the filter matching every track under it, and for folders the path below
it.

### `NowPlaying`

Struct · [`app/src-tauri/src/media.rs`](../../app/src-tauri/src/media.rs) · main thread · [D2: The shell](../developer-guide/06-rust-backend.md#the-shell)

Keeps the OS's Now Playing in step with the queue and the player: compares
what they do with what it last published and sends only the differences,
through a `Publisher`.

### `NowSummary`

Struct · [`app/src-tauri/src/shell/mod.rs`](../../app/src-tauri/src/shell/mod.rs) · main thread · [D2: The shell](../developer-guide/06-rust-backend.md#the-shell)

What the Dock menu and the menu-bar controls show: the current track and
what the transport can do.

### `OpenFolder`

Struct · [`app/src-tauri/src/library/access.rs`](../../app/src-tauri/src/library/access.rs) · any thread · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

A library folder, readable while this lives: what `open_folder` and
`open_folder_of` return. Hold it until the file in it is open.

### `Opening`

Enum · [`app/src-tauri/src/queue/model.rs`](../../app/src-tauri/src/queue/model.rs) · main thread · [D2: Loading off the main thread](../developer-guide/04-core.md#loading-off-the-main-thread)

What `Player::load` and `set_next` did: done at once (in place, or
failed), or pending, to be reported with `Queue::load_finished`.

### `Options`

Struct · [`app/src-tauri/src/self_test.rs`](../../app/src-tauri/src/self_test.rs) · [D2: The sandboxed bundle](../developer-guide/10-troubleshooting.md#the-sandboxed-bundle)

What the bundle self-test requires: the sandbox, audio, and whether to
play through the device.

### `Origin`

Enum · [`app/src-tauri/src/library/art.rs`](../../app/src-tauri/src/library/art.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

Where a picture came from: a file in a library folder or a track's
embedded picture, or an image in the cache. A new album-art source sets
it, or its thumbnails aren't indexed.

### `Outcome`

Enum · [`app/src-tauri/src/self_test.rs`](../../app/src-tauri/src/self_test.rs)

One self-test stage's result: passed, skipped or failed, with a message.

### `OutputSettings`

Struct, a setting · [`app/src-tauri/src/settings.rs`](../../app/src-tauri/src/settings.rs)

Where the audio goes: the output device by name (the default while it's
missing) and the buffer size.

### `OutputStatus`

Struct, sent to the UI · [`app/src-tauri/src/audio.rs`](../../app/src-tauri/src/audio.rs) · main thread · [D2: The engine on the main thread](../developer-guide/06-rust-backend.md#the-engine-on-the-main-thread)

The output devices, the open one's `DeviceInfo`, and whether the device
the settings name is missing (so the default plays). Shown in Settings ›
Playback.

### `OutsideArtist`

Struct, sent to the UI · [`app/src-tauri/src/metadata/outside.rs`](../../app/src-tauri/src/metadata/outside.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

An artist the library doesn't have, suggested on Home: MusicBrainz id,
name, disambiguation, reasons and links.

### `OutsideLinks`

Struct, sent to the UI · [`app/src-tauri/src/metadata/outside.rs`](../../app/src-tauri/src/metadata/outside.rs)

Where a suggestion's links lead: MusicBrainz and ListenBrainz always, a
homepage and Bandcamp when known. Passed through `webLink` before they
become links.

### `OutsideReason`

Enum, sent to the UI · [`app/src-tauri/src/metadata/outside.rs`](../../app/src-tauri/src/metadata/outside.rs)

Why an outside artist is suggested (X5): ListenBrainz's listeners of a
library artist play them, or MusicBrainz links them.

### `OutsideStatus`

Struct, sent to the UI · [`app/src-tauri/src/metadata/outside.rs`](../../app/src-tauri/src/metadata/outside.rs)

What Settings show about outside suggestions: the seeds sent now, and how
many were dismissed.

### `Owned`

Helper · [`app/src-tauri/src/metadata/outside.rs`](../../app/src-tauri/src/metadata/outside.rs)

Every library artist's MusicBrainz id and folded name, so suggestions
leave out what the library has.

### `Page`

Helper (service JSON) · [`app/src-tauri/src/metadata/wikipedia.rs`](../../app/src-tauri/src/metadata/wikipedia.rs)

A page in a Wikipedia answer, as served.

### `PairError`

Enum · [`app/src-tauri/src/remote/mod.rs`](../../app/src-tauri/src/remote/mod.rs)

Why pairing failed: locked out, or the wrong code.

### `Pairing`

Struct · [`app/src-tauri/src/remote/mod.rs`](../../app/src-tauri/src/remote/mod.rs) · [D2: The LAN remote](../developer-guide/06-rust-backend.md#the-lan-remote)

The remote's pairing code and failed attempts by address (a lockout after
too many), apart from sockets for tests.

### `Params`

Helper · [`app/src-tauri/src/library/browse.rs`](../../app/src-tauri/src/library/browse.rs), [`app/src-tauri/src/library/search.rs`](../../app/src-tauri/src/library/search.rs), [`app/src-tauri/src/library/smart.rs`](../../app/src-tauri/src/library/smart.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

A statement's bound values as it is assembled (`?1` is the ignored
articles in browse and search): every value is bound, never formatted into
the SQL. One of each in `browse.rs`, `search.rs` and `smart.rs`.

### `Part`

Helper · [`app/src-tauri/src/library/scanner.rs`](../../app/src-tauri/src/library/scanner.rs)

One track of a file: start, end, number, title, and what its cue sheet
overrides (performer, album, album artist).

### `Pending`

Helper · [`app/src-tauri/src/library/scanner.rs`](../../app/src-tauri/src/library/scanner.rs) · [D2: Adding a folder and its scan](../developer-guide/08-flows.md#adding-a-folder-and-its-scan)

A new or changed file whose tags need reading: its parts already in the
library, the cue sheet that splits it, and whether it is a placeholder
(recorded, not read).

### `Pick`

Struct · [`app/src-tauri/src/queue/radio.rs`](../../app/src-tauri/src/queue/radio.rs) · [D2: The queue](../developer-guide/06-rust-backend.md#the-queue)

A track library radio picked (O9), and why.

### `Picture`

Struct · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs) · any thread · [D2: Tags: TagReader](../developer-guide/04-core.md#tags-tagreader)

A file's embedded picture as `Tags` carries it: its MIME type and bytes.
Album art's embedded source.

### `PictureView`

Struct, sent to the UI · [`app/src-tauri/src/library/info.rs`](../../app/src-tauri/src/library/info.rs)

An embedded picture as Get Info shows it: type, MIME type, description,
size, and a `data:` URL unless too large.

### `Placeholders`

Enum · [`app/src-tauri/src/library/scanner.rs`](../../app/src-tauri/src/library/scanner.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

How a scan tells cloud placeholders: ask the file system
(`anomp::file_is_dataless`, which downloads nothing), or, in tests, a
list.

### `PlaybackSettings`

Struct, a setting · [`app/src-tauri/src/settings.rs`](../../app/src-tauri/src/settings.rs) · [D2: Settings and features](../developer-guide/06-rust-backend.md#settings-and-features)

Playback: ReplayGain mode, preamp and untagged gain, clipping prevention,
and crossfade. `gain` turns a track's ReplayGain into the engine's linear
gain.

### `Player`

Struct · [`app/src-tauri/src/media.rs`](../../app/src-tauri/src/media.rs) · main thread

The engine as `NowPlaying` last heard of it: state, position and duration.

### `Player`

Trait · [`app/src-tauri/src/queue/model.rs`](../../app/src-tauri/src/queue/model.rs) · main thread · [D2: The queue](../developer-guide/06-rust-backend.md#the-queue)

The engine as the queue drives it: load, set the next track, play, pause,
seek, and so on. `EnginePlayer` in the app; a fake engine in tests.

### `PlayerState`

Enum, sent to the UI · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs) · main thread · [D2: The engine: AudioEngine and PlayerEngine](../developer-guide/04-core.md#the-engine-audioengine-and-playerengine)

Empty, stopped, playing or paused (`ANOMP_STATE_*`), as `Engine::state`
and `Event::StateChanged` report it.

### `PlayerStatus`

Struct, sent to the UI · [`app/src-tauri/src/audio.rs`](../../app/src-tauri/src/audio.rs) · main thread · [D2: The engine on the main thread](../developer-guide/06-rust-backend.md#the-engine-on-the-main-thread)

The player's state, position, duration and volume, as `player_status`
returns them to the UI at start-up and after a reload.

### `Playlist`

Struct, sent to the UI · [`app/src-tauri/src/library/playlists.rs`](../../app/src-tauri/src/library/playlists.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

A playlist (F1) or smart playlist (F2): id, name, rules if smart, track
count and length.

### `PlaylistData`

Helper · [`app/src-tauri/src/library/transfer.rs`](../../app/src-tauri/src/library/transfer.rs)

A playlist in a data file: name, rules, its tracks' indexes and its dates.

### `PlaylistEntry`

Struct, sent to the UI · [`app/src-tauri/src/library/playlists.rs`](../../app/src-tauri/src/library/playlists.rs)

A track in a playlist, and which entry it is (a track can be listed
twice); no entry id in a smart playlist.

### `PlaylistPage`

Struct, sent to the UI · [`app/src-tauri/src/library/playlists.rs`](../../app/src-tauri/src/library/playlists.rs)

A playlist and its entries, as its view shows it.

### `Position`

Helper · [`app/src-tauri/src/queue/mod.rs`](../../app/src-tauri/src/queue/mod.rs)

A `Player` that only knows where it is, for noting a position at quit.

### `PositionPayload`

Struct, sent to the UI · [`app/src-tauri/src/audio.rs`](../../app/src-tauri/src/audio.rs) · main thread · [D2: The engine on the main thread](../developer-guide/06-rust-backend.md#the-engine-on-the-main-thread)

The `player-position` event's payload: position and duration in seconds.

### `Practice`

Struct, sent to the UI · [`app/src-tauri/src/audio.rs`](../../app/src-tauri/src/audio.rs) · main thread · [D2: The engine on the main thread](../developer-guide/06-rust-backend.md#the-engine-on-the-main-thread)

Practice mode's state (O9): the A–B loop's points, if set, the tempo and
the transposition. Returned by the practice commands for the practice
panel.

### `Prefs`

Struct, sent to the UI · [`app/src-tauri/src/features.rs`](../../app/src-tauri/src/features.rs) · [D2: Settings and features](../developer-guide/06-rust-backend.md#settings-and-features)

A track's and its album's preferences (O7) as the preferences dialog reads
them: a `TrackPrefs` and an `AlbumPrefs`, either absent.

### `Priority`

Enum · [`app/src-tauri/src/metadata/jobs.rs`](../../app/src-tauri/src/metadata/jobs.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

A job's priority: background enrichment, the album playing, the artist
page shown, or the user's own request, which goes first.

### `Profile`

Struct · [`app/src-tauri/src/library/similar.rs`](../../app/src-tauri/src/library/similar.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

What an item (a track, album, artist, or the user's taste) is like: the
share of its genres, its artists, composers, labels, year and loudness.

### `Progress`

Struct, sent to the UI · [`app/src-tauri/src/metadata/jobs.rs`](../../app/src-tauri/src/metadata/jobs.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

The worker's progress, as `metadata-progress` sends it: done of total
since it was last idle, and what it works on now.

### `Published`

Helper · [`app/src-tauri/src/media.rs`](../../app/src-tauri/src/media.rs)

Playback as last published, and when, to tell whether the OS's
extrapolation has drifted.

### `Publisher`

Trait · [`app/src-tauri/src/media.rs`](../../app/src-tauri/src/media.rs) · main thread · [D2: Fakes](../developer-guide/12-testing.md#fakes)

Where `NowPlaying` publishes: the OS's media controls
(`anomp::MediaControls`) or a fake in tests.

### `Query`

Struct · [`app/src-tauri/src/library/search.rs`](../../app/src-tauri/src/library/search.rs) · [D2: A search keystroke](../developer-guide/08-flows.md#a-search-keystroke)

What the user typed, taken apart: terms (each in any field or one), genres
and year ranges. Matched against the FTS5 word and trigram indexes.

### `Query`

Helper (service JSON) · [`app/src-tauri/src/metadata/wikipedia.rs`](../../app/src-tauri/src/metadata/wikipedia.rs)

The `query` part of a Wikipedia answer, as served.

### `Queue`

Struct · [`app/src-tauri/src/queue/model.rs`](../../app/src-tauri/src/queue/model.rs) · main thread · [D2: The queue](../developer-guide/06-rust-backend.md#the-queue)

The play queue's logic, apart from the engine: the list in play order,
shuffle and its original order, the current item, repeat, stop-after, the
sleep timer, and what the engine should hold as current and next. Moves on
when the engine hands off, and logs every list change. Hosted in a
main-thread thread-local by `queue/mod.rs`; tested against a fake
`Player`.

### `QueueData`

Helper · [`app/src-tauri/src/library/transfer.rs`](../../app/src-tauri/src/library/transfer.rs)

The queue in a data file: its tracks' indexes, the current one, the
position and the repeat mode.

### `QueueState`

Struct, sent to the UI · [`app/src-tauri/src/queue/model.rs`](../../app/src-tauri/src/queue/model.rs) · [D2: The queue](../developer-guide/06-rust-backend.md#the-queue)

What the UI needs to show the queue and the current track: revisions, the
list (whole when replaced) or its edits, the current item, repeat,
shuffle, stop-after, the sleep timer and what was skipped.

### `Raw`

Helper (service JSON) · [`app/src-tauri/src/metadata/listenbrainz.rs`](../../app/src-tauri/src/metadata/listenbrainz.rs)

A similar artist as the ListenBrainz Labs API serves it.

### `RawAlias`

Helper (service JSON) · [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs)

An artist's alias, as served.

### `RawAnalysisConfig`

Helper (C layout) · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs)

`anomp_analysis_config`'s layout, made from `AnalysisConfig`.

### `RawAnalysisFrame`

Helper (C layout) · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs)

`anomp_analysis_frame`'s layout, borrowed as an `AnalysisFrame`.

### `RawArea`

Helper (service JSON) · [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs)

An artist's area, as served.

### `RawArtist`

Helper (service JSON) · [`app/src-tauri/src/metadata/discogs.rs`](../../app/src-tauri/src/metadata/discogs.rs), [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs)

An artist in a Discogs or MusicBrainz answer, as served.

### `RawArtistEntry`

Helper (service JSON) · [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs)

An artist search result or lookup, as served.

### `RawBookmark`

Helper (C layout) · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs)

`anomp_bookmark`'s layout, copied into a `Vec<u8>` and freed.

### `RawBrowsedGroup`

Helper (service JSON) · [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs)

A release group in that page, as served.

### `RawChapter`

Helper (C layout) · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs)

`anomp_chapter`'s layout, turned into a `Chapter`.

### `RawCoverArt`

Helper (service JSON) · [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs)

Whether a release has front cover art, as served.

### `RawCredit`

Helper (service JSON) · [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs)

An artist credit, as served.

### `RawDeviceInfo`

Helper (C layout) · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs)

`anomp_device_info`'s layout, turned into `DeviceInfo`.

### `RawDockMenu`

Helper (C layout) · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs)

The opaque `anomp_dock_menu`, held by `DockMenu`.

### `RawEffectInfo`

Helper (C layout) · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs)

`anomp_effect_info`'s layout, turned into an `EffectInfo`.

### `RawEffectParam`

Helper (C layout) · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs)

`anomp_effect_param`'s layout, turned into an `EffectParam`.

### `RawEngine`

Helper (C layout) · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs) · [D2: The C API's conventions](../developer-guide/04-core.md#the-c-apis-conventions)

The opaque `anomp_engine`, as Rust declares it for the FFI.

### `RawEvent`

Helper (C layout) · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs)

`anomp_event`'s layout, read in the event callback and turned into an
`Event`.

### `RawFileAnalysis`

Helper (C layout) · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs)

`anomp_file_analysis`'s layout, turned into `FileAnalysis`.

### `RawFileInfo`

Helper (C layout) · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs)

`anomp_file_info`'s layout, turned into `FileInfo`.

### `RawFolderAccess`

Helper (C layout) · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs)

The opaque `anomp_folder_access`, held by `FolderAccess`.

### `RawFormat`

Helper (service JSON) · [`app/src-tauri/src/metadata/discogs.rs`](../../app/src-tauri/src/metadata/discogs.rs)

A Discogs release's format (CD, vinyl…), as served.

### `RawGenre`

Helper (service JSON) · [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs)

A genre and its vote count, as served.

### `RawGroup`

Helper (service JSON) · [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs)

A release group lookup, as served.

### `RawHit`

Helper (service JSON) · [`app/src-tauri/src/metadata/discogs.rs`](../../app/src-tauri/src/metadata/discogs.rs)

A Discogs search result, as served.

### `RawIdentifier`

Helper (service JSON) · [`app/src-tauri/src/metadata/discogs.rs`](../../app/src-tauri/src/metadata/discogs.rs)

A Discogs release's identifier (barcode and the like), as served.

### `RawImage`

Helper (service JSON) · [`app/src-tauri/src/metadata/coverartarchive.rs`](../../app/src-tauri/src/metadata/coverartarchive.rs)

An image in the archive's listing, as served.

### `RawLabel`

Helper (service JSON) · [`app/src-tauri/src/metadata/discogs.rs`](../../app/src-tauri/src/metadata/discogs.rs), [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs)

A label in a Discogs or MusicBrainz release, as served.

### `RawLabelInfo`

Helper (service JSON) · [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs)

A release's label and catalogue number, as served.

### `RawLifeSpan`

Helper (service JSON) · [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs)

An artist's life span, as served.

### `RawLinks`

Helper (service JSON) · [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs)

An entity's URL relations, as served.

### `RawListing`

Helper (service JSON) · [`app/src-tauri/src/metadata/coverartarchive.rs`](../../app/src-tauri/src/metadata/coverartarchive.rs)

A release's image listing as the Cover Art Archive serves it.

### `RawMediaCommand`

Helper (C layout) · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs)

`anomp_media_command`'s layout, turned into a `MediaCommand`.

### `RawMediaControls`

Helper (C layout) · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs)

The opaque `anomp_media_controls`, held by `MediaControls`.

### `RawMediaTrack`

Helper (C layout) · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs)

`anomp_media_track`'s layout.

### `RawMedium`

Helper (service JSON) · [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs)

A medium (disc) of a release, as served.

### `RawMenuItem`

Helper (C layout) · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs)

`anomp_menu_item`'s layout, made from a `MenuItem`.

### `RawPage`

Helper (service JSON) · [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs)

A page of an artist's release groups, as served.

### `RawPicture`

Helper (C layout) · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs)

`anomp_picture`'s layout, turned into a `FilePicture`.

### `RawRecordFormat`

Helper (C layout) · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs)

`anomp_record_format`'s layout, made from `RecordingFormat`.

### `RawRecording`

Helper (C layout) · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs)

`anomp_recording`'s layout, turned into `RecordingStatus`.

### `RawRecording`

Helper (service JSON) · [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs)

A track's recording, as served.

### `RawRecordingMark`

Helper (C layout) · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs)

`anomp_recording_mark`'s layout.

### `RawRelatedArtist`

Helper (service JSON) · [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs)

The artist at the other end of a relation, as served.

### `RawRelation`

Helper (service JSON) · [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs)

A relation to a URL or an artist, as served.

### `RawRelease`

Helper (service JSON) · [`app/src-tauri/src/metadata/discogs.rs`](../../app/src-tauri/src/metadata/discogs.rs), [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs)

A release as Discogs or MusicBrainz serves it (one of each), parsed into
`Release`.

### `RawReleaseGroup`

Helper (service JSON) · [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs)

A release's release group, as served.

### `RawSearch`

Helper (service JSON) · [`app/src-tauri/src/metadata/discogs.rs`](../../app/src-tauri/src/metadata/discogs.rs), [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs)

A search answer as Discogs or MusicBrainz serves it (MusicBrainz has one
for artists and one for releases).

### `RawSignalPath`

Helper (C layout) · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs)

`anomp_signal_path`'s layout, turned into `SignalPath`.

### `RawTagField`

Helper (C layout) · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs)

`anomp_tag_field`'s layout.

### `RawTags`

Helper (C layout) · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs)

`anomp_tags`'s layout, turned into `Tags`.

### `RawTrack`

Helper (service JSON) · [`app/src-tauri/src/metadata/discogs.rs`](../../app/src-tauri/src/metadata/discogs.rs), [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs)

A track in a Discogs tracklist or a MusicBrainz medium, as served.

### `RawTrackOptions`

Helper (C layout) · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs)

`anomp_track_options`'s layout, made from `TrackOptions`.

### `RawUrl`

Helper (service JSON) · [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs)

A relation's URL, as served.

### `RawVolumeWatcher`

Helper (C layout) · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs)

The opaque `anomp_volume_watcher`, held by `VolumeWatcher`.

### `ReadFile`

Helper · [`app/src-tauri/src/library/scanner.rs`](../../app/src-tauri/src/library/scanner.rs)

What reading a file gave: its tags and the parts it plays as.

### `Reason`

Enum, sent to the UI · [`app/src-tauri/src/library/similar.rs`](../../app/src-tauri/src/library/similar.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

Why something was recommended (X4), for the UI to say: a shared genre,
era, label, a linked artist, the same artist or composer, being played
together, a like loudness.

### `RecentEntry`

Struct, sent to the UI · [`app/src-tauri/src/history/views.rs`](../../app/src-tauri/src/history/views.rs) · [D2: History and ListenBrainz](../developer-guide/06-rust-backend.md#history-and-listenbrainz)

One row of Recently played: a run of plays from one album collapsed into
that album, or a single track.

### `RecentTrack`

Struct, sent to the UI · [`app/src-tauri/src/history/views.rs`](../../app/src-tauri/src/history/views.rs)

A track in a `RecentEntry`: id, title, artist and its folder (to dim it
while unreadable).

### `RecordingFormat`

Struct · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs) · main thread · [D2: Recording: Recorder and FFmpegEncoder](../developer-guide/04-core.md#recording-recorder-and-ffmpegencoder)

A recording's format: the `RecordingKind`, and its bits or bitrate
(`anomp_record_format`). Made by `recording.rs` from the settings.

### `RecordingKind`

Enum, sent to the UI · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs) · [D2: A recording format](../developer-guide/11-recipes.md#a-recording-format)

What a recording is written as: WAV, AIFF, FLAC, ALAC, AAC or MP3, in
`anomp.h`'s order (`ANOMP_RECORD_*`). The recording settings store it.

### `RecordingSettings`

Struct, a setting · [`app/src-tauri/src/recording.rs`](../../app/src-tauri/src/recording.rs) · [D2: Settings and features](../developer-guide/06-rust-backend.md#settings-and-features)

The recording's format in `AppSettings`: the kind, bits, bitrate, and
whether to write cue sheets. The folder is stored apart, as a writable
bookmark.

### `RecordingState`

Struct, sent to the UI · [`app/src-tauri/src/recording.rs`](../../app/src-tauri/src/recording.rs)

Whether a recording runs, its length and overruns, its file's name, and
the folder recordings go to.

### `RecordingStatus`

Struct · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs) · main thread · [D2: Recording: Recorder and FFmpegEncoder](../developer-guide/04-core.md#recording-recorder-and-ffmpegencoder)

A recording's progress (`anomp_recording`): whether on, frames, seconds,
overruns and files begun.

### `RecordingStopped`

Struct, sent to the UI · [`app/src-tauri/src/recording.rs`](../../app/src-tauri/src/recording.rs)

A recording that stopped: the files written, its length, overruns, and a
coded error if it didn't stop at the user's request.

### `Recovered`

Enum · [`app/src-tauri/src/library/recovery.rs`](../../app/src-tauri/src/library/recovery.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

What `apply_scheduled` did at launch, before anything opened the database:
restored, or rebuilt (scan every folder, then import the export).

### `Recovery`

Enum · [`app/src-tauri/src/library/recovery.rs`](../../app/src-tauri/src/library/recovery.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

A recovery waiting for the next launch, stored as JSON beside the
database: restore a copy, or rebuild from the folders and then import an
export.

### `RecoveryState`

Struct · [`app/src-tauri/src/library/commands.rs`](../../app/src-tauri/src/library/commands.rs) · any thread · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

The launch database check's result (`DbCheck`) and a rebuild still to
finish (the export to import once every folder is scanned), managed by
Tauri (H10).

### `Refs`

Helper · [`app/src-tauri/src/library/transfer.rs`](../../app/src-tauri/src/library/transfer.rs)

Interns tracks, albums and artists into the data file's lists by id as it
is written.

### `RelatedArtist`

Struct, sent to the UI · [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs)

An artist linked to another on MusicBrainz, and how (member of a band,
collaboration…). Recommendations use the links.

### `Relation`

Enum, sent to the UI · [`app/src-tauri/src/library/similar.rs`](../../app/src-tauri/src/library/similar.rs)

How an artist is linked to the seed's: member, subgroup or collaborator.

### `Release`

Struct, sent to the UI · [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

A release (one issue of an album) as the app keeps it: title, artist
credit, date, country, labels, media, tracks, credits, genres and its
release group, stored as JSON in `album_links.details`. Discogs' releases
are converted to it too, so the matcher and the UI handle every source
alike.

### `ReleaseCandidate`

Struct, sent to the UI · [`app/src-tauri/src/metadata/albums.rs`](../../app/src-tauri/src/metadata/albums.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

A release offered in "Find details", scored as automatic matching scores
it, with its page.

### `ReleaseGroup`

Struct · [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs)

A release group (an album across its releases) as the app keeps it: the
Wikidata and Wikipedia links that lead to the album's description.

### `ReleaseGroupEntry`

Struct, sent to the UI · [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs)

A release group as an artist's discography lists it: title, credit, types
and first date.

### `ReleaseGroupPage`

Struct · [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs)

One page of an artist's release groups, and how many there are in all.

### `ReleaseSource`

Trait · [`app/src-tauri/src/metadata/albums.rs`](../../app/src-tauri/src/metadata/albums.rs) · the metadata worker · [D2: A metadata source](../developer-guide/11-recipes.md#a-metadata-source)

A source of album details (MusicBrainz, Discogs): search releases, look
one up, and a release known without searching. Matching, the "Find
details" candidates and the user's picks work the same through it.

### `ReleaseTrack`

Struct, sent to the UI · [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs)

A track on a release: disc, position, title, length and recording id; the
lengths help scoring.

### `RemoteDevice`

Struct, sent to the UI · [`app/src-tauri/src/remote/mod.rs`](../../app/src-tauri/src/remote/mod.rs)

A paired phone: name, when paired and last seen. Only a hash of its token
is kept.

### `RemoteServer`

Struct · [`app/src-tauri/src/remote/mod.rs`](../../app/src-tauri/src/remote/mod.rs) · any thread · [D2: The LAN remote](../developer-guide/06-rust-backend.md#the-lan-remote)

The LAN remote (O14) as Tauri manages it: the server when running,
pairing, and its last error. Answers local addresses only; any change here
needs the security review in PLAN.md §8.1.

### `RemoteStatus`

Struct, sent to the UI · [`app/src-tauri/src/remote/mod.rs`](../../app/src-tauri/src/remote/mod.rs)

What Settings show about the remote: whether it runs, its address, why
not, and the pairing code while valid.

### `Repeat`

Enum, sent to and from the UI · [`app/src-tauri/src/queue/model.rs`](../../app/src-tauri/src/queue/model.rs)

Repeat off, all, or one.

### `ReplayGain`

Struct · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs) · any thread · [D2: Tags: TagReader](../developer-guide/04-core.md#tags-tagreader)

A file's ReplayGain tags: track and album gains in dB and peaks as linear
values, Opus R128 gains converted. Part of `Tags`;
`PlaybackSettings::gain` turns it into a track's gain.

### `ReplayGainMode`

Enum, a setting · [`app/src-tauri/src/settings.rs`](../../app/src-tauri/src/settings.rs)

ReplayGain off, by track, or by album (falling back to the track gain).

### `Request`

Struct · [`app/src-tauri/src/remote/http.rs`](../../app/src-tauri/src/remote/http.rs) · the remote's threads · [D2: The LAN remote](../developer-guide/06-rust-backend.md#the-lan-remote)

A request as the remote's minimal HTTP/1.1 parser reads it: method, path,
query and headers, with bounded sizes.

### `Requests`

Helper · [`app/src-tauri/src/queue/opening.rs`](../../app/src-tauri/src/queue/opening.rs) · main thread · [D2: Loading off the main thread](../developer-guide/04-core.md#loading-off-the-main-thread)

The queue's open requests not yet reported: those being resolved, and
those given to the engine.

### `Resolved`

Struct · [`app/src-tauri/src/library/transfer.rs`](../../app/src-tauri/src/library/transfer.rs)

Library ids for a data file's tracks, albums and artists, where the
library has them.

### `Resolved`

Helper · [`app/src-tauri/src/queue/opening.rs`](../../app/src-tauri/src/queue/opening.rs)

A track resolved on a blocking thread: how to play it, its folder (open),
and whether it is a placeholder.

### `Response`

Struct · [`app/src-tauri/src/metadata/http.rs`](../../app/src-tauri/src/metadata/http.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

An HTTP response as the `Transport` returns it: status, content type,
Retry-After and body.

### `Response`

Helper (service JSON) · [`app/src-tauri/src/metadata/wikipedia.rs`](../../app/src-tauri/src/metadata/wikipedia.rs)

A Wikipedia API answer, as served.

### `Response`

Struct · [`app/src-tauri/src/remote/http.rs`](../../app/src-tauri/src/remote/http.rs) · the remote's threads · [D2: The LAN remote](../developer-guide/06-rust-backend.md#the-lan-remote)

A response for the remote's minimal HTTP server to write: status, content
type and body.

### `Row`

Helper · [`app/src-tauri/src/library/playback.rs`](../../app/src-tauri/src/library/playback.rs)

The columns `track_play` reads for a track.

### `Row`

Helper · [`app/src-tauri/src/library/scanner.rs`](../../app/src-tauri/src/library/scanner.rs)

A track as the compilation regrouping reads it.

### `Row`

Helper · [`app/src-tauri/src/library/thumbs.rs`](../../app/src-tauri/src/library/thumbs.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

An `art_thumbs` row: which picture (by `Origin` and stamp) an album shows,
and its thumbnail's hash.

### `RuleSpec`

Enum, from the UI · [`app/src-tauri/src/library/browse.rs`](../../app/src-tauri/src/library/browse.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

A sort rule by its stored id, or given in full (to play an album found by
search whatever the stored rules are).

### `Running`

Struct · [`app/src-tauri/src/quitting.rs`](../../app/src-tauri/src/quitting.rs) · any thread · [D2: Quitting](../developer-guide/08-flows.md#quitting)

Held by a thread until it ends, so quitting can wait for it.

### `Running`

Helper · [`app/src-tauri/src/remote/mod.rs`](../../app/src-tauri/src/remote/mod.rs)

The remote server running: its port and its stop flag.

### `Saved`

Struct · [`app/src-tauri/src/queue/model.rs`](../../app/src-tauri/src/queue/model.rs) · [D2: The queue](../developer-guide/06-rust-backend.md#the-queue)

The queue as saved across launches: track ids, their uids, the order
before shuffling, the current index, position, repeat and volume.

### `SavedPlayer`

Helper · [`app/src-tauri/src/queue/mod.rs`](../../app/src-tauri/src/queue/mod.rs) · [D2: The queue](../developer-guide/06-rust-backend.md#the-queue)

What the `player.queue` setting holds besides the rows: the current index,
position, repeat, shuffle and volume, written with every state.

### `ScanFailure`

Struct, sent to the UI · [`app/src-tauri/src/library/scanner.rs`](../../app/src-tauri/src/library/scanner.rs)

A file or folder a scan couldn't read, and why.

### `ScanOptions`

Struct · [`app/src-tauri/src/library/scanner.rs`](../../app/src-tauri/src/library/scanner.rs) · the scan's thread · [D2: Adding a folder and its scan](../developer-guide/08-flows.md#adding-a-folder-and-its-scan)

How a scan reads files: split them into parts (cue sheets, chapters), run
in the background at a low priority, remove missing tracks even when
`holds` would keep them, and how to tell placeholders.

### `ScanProgress`

Struct, sent to the UI · [`app/src-tauri/src/library/scanner.rs`](../../app/src-tauri/src/library/scanner.rs)

A scan's progress: files read so far of those to read, sent before the
first and after each batch.

### `ScanReport`

Struct, sent to the UI · [`app/src-tauri/src/library/scanner.rs`](../../app/src-tauri/src/library/scanner.rs) · [D2: Adding a folder and its scan](../developer-guide/08-flows.md#adding-a-folder-and-its-scan)

What a folder's scan did: tracks added, updated, removed, moved and
unchanged, failures, placeholders, and why the folder couldn't be scanned
or kept tracks it didn't find (H22).

### `Scheme`

Enum, a setting · [`app/src-tauri/src/theme.rs`](../../app/src-tauri/src/theme.rs)

Which palette shows: as the system is, light or dark.

### `Scored`

Struct · [`app/src-tauri/src/library/similar.rs`](../../app/src-tauri/src/library/similar.rs)

A candidate, its score, and the weighted reasons behind it.

### `SearchHit`

Struct · [`app/src-tauri/src/metadata/musicbrainz.rs`](../../app/src-tauri/src/metadata/musicbrainz.rs)

A release search hit, without its tracks, with MusicBrainz's score.

### `SearchKind`

Enum, from the UI · [`app/src-tauri/src/library/search.rs`](../../app/src-tauri/src/library/search.rs)

Which part of the results a "show more" asks for: artists, albums or
tracks.

### `SearchResults`

Struct, sent to the UI · [`app/src-tauri/src/library/search.rs`](../../app/src-tauri/src/library/search.rs) · [D2: A search keystroke](../developer-guide/08-flows.md#a-search-keystroke)

A search's artists, albums and tracks, each with its total.

### `Seed`

Enum · [`app/src-tauri/src/library/similar.rs`](../../app/src-tauri/src/library/similar.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

What a recommendation is like: a track, an album or an artist.

### `Seed`

Struct, sent to the UI · [`app/src-tauri/src/metadata/outside.rs`](../../app/src-tauri/src/metadata/outside.rs)

A library artist whose MusicBrainz id the suggestions send, and its
weight. Settings show them, so the user sees what goes out.

### `SeedIndex`

Struct · [`app/src-tauri/src/library/similar.rs`](../../app/src-tauri/src/library/similar.rs)

A seed's `Profile` as sets, for looking up what each candidate shares with
it.

### `Sender`

Struct · [`app/src-tauri/src/history/listenbrainz.rs`](../../app/src-tauri/src/history/listenbrainz.rs) · the history thread · [D2: History and ListenBrainz](../developer-guide/06-rust-backend.md#history-and-listenbrainz)

The history thread's ListenBrainz sending state: its `http::Client`, when
to retry, and the last error. Sends `listens_pending` in batches; a
rejected token stops it until a new one is set.

### `ServiceSettings`

Struct, a setting · [`app/src-tauri/src/metadata/settings.rs`](../../app/src-tauri/src/metadata/settings.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

Settings › Services: the online switch, background matching after scans,
each source's settings, and each kind's sources in order (the first with a
result wins). Stored under `metadata.services`.

### `Session`

Struct · [`app/src-tauri/src/recording.rs`](../../app/src-tauri/src/recording.rs) · main thread · [D2: The shell](../developer-guide/06-rust-backend.md#the-shell)

The recording running now: holds the folder open while files are written,
and gathers the tracks that became current for the cue sheet.

### `SettingsPayload`

Struct, sent to the UI · [`app/src-tauri/src/settings.rs`](../../app/src-tauri/src/settings.rs) · [D2: A setting changed in the UI](../developer-guide/08-flows.md#a-setting-changed-in-the-ui)

The settings and the defaults a section can be reset to, as `settings_get`
returns them.

### `SettingsState`

Struct · [`app/src-tauri/src/settings.rs`](../../app/src-tauri/src/settings.rs) · any thread · [D2: Settings and features](../developer-guide/06-rust-backend.md#settings-and-features)

The settings as saved, managed by Tauri, replaced whole so a panic never
leaves them half-changed.

### `Shared`

Struct · [`app/src-tauri/src/metadata/jobs.rs`](../../app/src-tauri/src/metadata/jobs.rs) · any thread · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

The worker's queue and the calls waiting for it, shared with other
threads, which only queue; never waits on the database or the network.

### `Shared`

Helper · [`app/src-tauri/src/updates.rs`](../../app/src-tauri/src/updates.rs)

The update checker's state and the condition its thread waits on.

### `SignalPath`

Struct, sent to the UI · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs) · main thread · [D2: The signal path](../developer-guide/04-core.md#the-signal-path)

Every step between the file and the speakers (`anomp_signal_path`):
format, gain, tempo, resampling, crossfeed, equaliser, effects, freeze,
recording, volume and device. Shown by the signal path panel through
`audio.rs`'s `SignalPathPayload`.

### `SignalPathPayload`

Struct, sent to the UI · [`app/src-tauri/src/audio.rs`](../../app/src-tauri/src/audio.rs) · main thread · [D2: The signal path](../developer-guide/04-core.md#the-signal-path)

What the signal path panel shows: the engine's `SignalPath`, the device,
and whether it plays through headphones.

### `SimilarAlbum`

Struct, sent to the UI · [`app/src-tauri/src/library/similar.rs`](../../app/src-tauri/src/library/similar.rs)

A recommended album and its reasons.

### `SimilarArtist`

Struct, sent to the UI · [`app/src-tauri/src/library/similar.rs`](../../app/src-tauri/src/library/similar.rs)

A recommended artist and its reasons.

### `SimilarArtist`

Struct · [`app/src-tauri/src/metadata/listenbrainz.rs`](../../app/src-tauri/src/metadata/listenbrainz.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

An artist ListenBrainz finds similar to another (X5): MusicBrainz id,
name, disambiguation, type and score.

### `SimilarTrack`

Struct, sent to the UI · [`app/src-tauri/src/library/similar.rs`](../../app/src-tauri/src/library/similar.rs)

A recommended track and its reasons.

### `Sink`

Trait · [`app/src-tauri/src/visualizer.rs`](../../app/src-tauri/src/visualizer.rs) · the analysis thread · [D2: The visualizer](../developer-guide/07-frontend.md#the-visualizer)

Where analysis frames go: a Tauri `Channel` in the app, a fake in tests.

### `Size`

Enum · [`app/src-tauri/src/library/thumbs.rs`](../../app/src-tauri/src/library/thumbs.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

The thumbnail sizes the UI asks for (H17): list rows, and the header
(grids, the visualizer, the OS's Now Playing).

### `Skipped`

Struct, sent to the UI · [`app/src-tauri/src/queue/model.rs`](../../app/src-tauri/src/queue/model.rs)

A track that couldn't be opened and was passed over, and why.

### `Sleep`

Helper · [`app/src-tauri/src/queue/model.rs`](../../app/src-tauri/src/queue/model.rs)

A sleep timer running, and the volume a fade started from.

### `SleepChoice`

Enum · [`app/src-tauri/src/shell/menu.rs`](../../app/src-tauri/src/shell/menu.rs)

A sleep timer the menu offers: some minutes, end of track or end of album.

### `SleepRequest`

Enum, from the UI · [`app/src-tauri/src/queue/mod.rs`](../../app/src-tauri/src/queue/mod.rs)

A sleep timer as the UI asks for it: in some minutes, at the end of the
track or of the album, or off.

### `SleepTimer`

Enum, sent to the UI · [`app/src-tauri/src/queue/model.rs`](../../app/src-tauri/src/queue/model.rs) · main thread

When a sleep timer stops playback: at a time (fading out before it), at
the end of the track, or at the end of the album. Made from the UI's
`SleepRequest` by `queue_set_sleep`, and shown as `QueueState`'s `sleep`.

### `SmartOrder`

Enum, sent to and from the UI · [`app/src-tauri/src/library/smart.rs`](../../app/src-tauri/src/library/smart.rs)

A smart playlist's order: a shuffle that stays put, date added, most
played, last played, rating, album or title.

### `SmartRules`

Struct, sent to and from the UI · [`app/src-tauri/src/library/smart.rs`](../../app/src-tauri/src/library/smart.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

A smart playlist's rules (F2), stored as JSON in `playlists.rules`: match
all or any of its conditions, an order and a limit. Its tracks are
whatever matches now.

### `SortRule`

Struct, a setting · [`app/src-tauri/src/library/rules.rs`](../../app/src-tauri/src/library/rules.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

A sort rule: its id and name, its grouping levels, the order of tracks in
the last, and the album order. Browsing follows it.

### `SortSettings`

Struct, sent to the UI · [`app/src-tauri/src/library/rules.rs`](../../app/src-tauri/src/library/rules.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

The sort rules (never empty) and the leading articles sorting ignores,
stored as JSON in `settings` and edited in Settings › Sort rules.

### `SourceCandidates`

Struct, sent to the UI · [`app/src-tauri/src/metadata/commands.rs`](../../app/src-tauri/src/metadata/commands.rs)

What one source offers in a picking dialog: its candidates, or a note
saying why there are none (offline, off, not matched).

### `SourcedArticle`

Struct, sent to the UI · [`app/src-tauri/src/metadata/artists.rs`](../../app/src-tauri/src/metadata/artists.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

A biography or album description as the UI shows it: the text, its source,
and the licence to credit (Wikipedia's is CC BY-SA).

### `SourcedLink`

Struct, sent to the UI · [`app/src-tauri/src/library/albums.rs`](../../app/src-tauri/src/library/albums.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

An album's match with one metadata source, as the album header shows it:
the source, whether details are kept with it (not for Discogs), the
release page and the release.

### `SourceId`

Enum, a setting · [`app/src-tauri/src/metadata/settings.rs`](../../app/src-tauri/src/metadata/settings.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

A metadata source (embedded pictures, folder images, MusicBrainz, the
Cover Art Archive, Wikipedia, Discogs). Its serialized id is what the
`source` columns store, so it must never change.

### `SourceInfo`

Struct, sent to the UI · [`app/src-tauri/src/metadata/settings.rs`](../../app/src-tauri/src/metadata/settings.rs) · [D2: A metadata source](../developer-guide/11-recipes.md#a-metadata-source)

What the UI shows about a source: name, the kinds it supplies, whether it
goes online, whether it needs a key, and whether it stores details
(Discogs doesn't).

### `SourceSettings`

Struct, a setting · [`app/src-tauri/src/metadata/settings.rs`](../../app/src-tauri/src/metadata/settings.rs)

One source's settings: on or off, and whether its key is in the keychain
(only `keys::set_key` changes that).

### `Started`

Struct · [`app/src-tauri/src/library/access.rs`](../../app/src-tauri/src/library/access.rs) · any thread · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

A folder a bookmark resolved to: its path now, whether the bookmark is
stale, and a guard that keeps it readable while it lives.

### `State`

Helper · [`app/src-tauri/src/library/analysis.rs`](../../app/src-tauri/src/library/analysis.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

The analysis worker's queue: whether it analyses the whole library, the
tracks asked for, and whether requests are served.

### `State`

Helper · [`app/src-tauri/src/metadata/jobs.rs`](../../app/src-tauri/src/metadata/jobs.rs)

Inside `Shared`: the jobs, the calls, whether the worker was woken, and
whether enrichment is on.

### `Step`

Enum · [`app/src-tauri/src/metadata/jobs.rs`](../../app/src-tauri/src/metadata/jobs.rs)

What `Worker::step` did: ran a job, found nothing queued, or paused until
an unreachable host may be tried again.

### `Store`

Struct · [`app/src-tauri/src/queue/store.rs`](../../app/src-tauri/src/queue/store.rs) · [D2: The queue](../developer-guide/06-rust-backend.md#the-queue)

The saved queue's rows (`queue_items`, H16), kept in step by applying the
same `Edit`s the UI gets, so a change writes only the rows it touches;
sparse order keys, renumbered when two meet.

### `StoredFolder`

Helper · [`app/src-tauri/src/recording.rs`](../../app/src-tauri/src/recording.rs)

The recordings' folder as stored in its own `settings` row: where it was
picked, and its bookmark.

### `Subject`

Enum · [`app/src-tauri/src/metadata/jobs.rs`](../../app/src-tauri/src/metadata/jobs.rs)

What a job is about: an album or an artist.

### `Subscribers`

Struct · [`app/src-tauri/src/visualizer.rs`](../../app/src-tauri/src/visualizer.rs) · [D2: The visualizer](../developer-guide/07-frontend.md#the-visualizer)

The subscribed sinks, by id. The core analyses only while there is one.

### `System`

Struct · [`app/src-tauri/src/library/access.rs`](../../app/src-tauri/src/library/access.rs) · any thread · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

The real `Bookmarks`: the core's security-scoped bookmarks, through
`anomp::FolderAccess`.

### `SystemClock`

Struct · [`app/src-tauri/src/metadata/http.rs`](../../app/src-tauri/src/metadata/http.rs)

The real `Clock`.

### `TagParts`

Struct · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs) · any thread · [D2: Tags: TagReader](../developer-guide/04-core.md#tags-tagreader)

What `read_tags_with` reads besides the tags: the picture, the lyrics, the
chapters (`ANOMP_TAGS_*`).

### `Tags`

Struct · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs) · any thread · [D2: Tags: TagReader](../developer-guide/04-core.md#tags-tagreader)

A file's tags and audio properties (`anomp_tags`), read by `read_tags` or
`read_tags_with`: fields the file lacks are `None`, several values are
joined with "; ". The scanner turns it into the library's rows.

### `Take`

Struct · [`app/src-tauri/src/workbench.rs`](../../app/src-tauri/src/workbench.rs) · main thread · [D2: The effects workbench's takes](../developer-guide/06-rust-backend.md#the-effects-workbenchs-takes)

A take being recorded in the effects workbench (X8): its queue item, the
item playback stopped after before it, and for a take of the A–B loop the
loop's end and the tempo, from which `due` times the stop.

### `Target`

Enum · [`app/src-tauri/src/library/art.rs`](../../app/src-tauri/src/library/art.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

What an `anomp-art` URI asks for: an album's or track's art (full size or
a thumbnail), a cover candidate, or a cached image.

### `TauriHost`

Struct · [`app/src-tauri/src/metadata/worker.rs`](../../app/src-tauri/src/metadata/worker.rs) · the metadata worker

The worker's `Host` in the app: forgets art for an album that changed and
emits `metadata-changed` and `metadata-progress`.

### `Theme`

Struct, a setting · [`app/src-tauri/src/theme.rs`](../../app/src-tauri/src/theme.rs) · [D2: Themes and styling](../developer-guide/07-frontend.md#themes-and-styling)

One theme's tokens: its name and preset, scheme, light and dark palettes,
font, text size, density, radius, and accent from the cover. Values only,
never CSS; the frontend sets them as custom properties after `validate`.

### `ThemePalette`

Struct, a setting · [`app/src-tauri/src/theme.rs`](../../app/src-tauri/src/theme.rs) · [D2: A theme colour](../developer-guide/11-recipes.md#a-theme-colour)

A theme's colours, each `#rrggbb`, named as the CSS custom properties. A
new colour token goes here, in every theme of `themes.json`, and in
`theme.ts`.

### `TopEntry`

Struct, sent to the UI · [`app/src-tauri/src/history/views.rs`](../../app/src-tauri/src/history/views.rs) · [D2: History and ListenBrainz](../developer-guide/06-rust-backend.md#history-and-listenbrainz)

One row of a most-played list: what it is, its plays, and the track ids
"Play these" queues.

### `TopKind`

Enum, sent to the UI · [`app/src-tauri/src/history/views.rs`](../../app/src-tauri/src/history/views.rs)

What a most-played list ranks: tracks, albums or artists.

### `TopPlayed`

Struct, sent to the UI · [`app/src-tauri/src/history/views.rs`](../../app/src-tauri/src/history/views.rs) · [D2: History and ListenBrainz](../developer-guide/06-rust-backend.md#history-and-listenbrainz)

A most-played list for a year or month: its entries, the plays in the
period, and the years that have any.

### `TrackAnalysis`

Struct, sent to the UI · [`app/src-tauri/src/library/analysis.rs`](../../app/src-tauri/src/library/analysis.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

A track's analysis as the UI shows it: loudness, gain, peaks, silences,
cutoff, or why it failed.

### `TrackColumn`

Enum, a setting · [`app/src-tauri/src/settings.rs`](../../app/src-tauri/src/settings.rs)

A column a track list can show besides the title.

### `TrackDetails`

Struct, sent to the UI · [`app/src-tauri/src/library/info.rs`](../../app/src-tauri/src/library/info.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

Get Info for a track (F16): the track, where its file is, the part of it
the track is, everything the file says (`FileInfo`), its pictures and its
MusicBrainz ids.

### `TrackEndedPayload`

Struct, sent to the UI · [`app/src-tauri/src/audio.rs`](../../app/src-tauri/src/audio.rs) · main thread · [D2: The engine on the main thread](../developer-guide/06-rust-backend.md#the-engine-on-the-main-thread)

The `player-track-ended` event's payload: whether the next track took
over.

### `Tracker`

Struct · [`app/src-tauri/src/history/mod.rs`](../../app/src-tauri/src/history/mod.rs) · main thread · [D2: History and ListenBrainz](../developer-guide/06-rust-backend.md#history-and-listenbrainz)

Follows what plays: fed the queue's current item and the engine's
position, it decides when a play counts (half the track or four minutes,
played time only) and tells the history thread. Free of Tauri, so tests
drive it directly.

### `TrackFacts`

Struct · [`app/src-tauri/src/library/similar.rs`](../../app/src-tauri/src/library/similar.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

A track as recommendations score it: its folder, artists, album, genres,
labels, year and loudness.

### `TrackFile`

Helper · [`app/src-tauri/src/library/art.rs`](../../app/src-tauri/src/library/art.rs)

A file of the album or track: its folder and its path in it.

### `TrackInfo`

Struct, sent to the UI · [`app/src-tauri/src/queue/model.rs`](../../app/src-tauri/src/queue/model.rs) · [D2: The queue](../developer-guide/06-rust-backend.md#the-queue)

What the queue shows and needs about a track: title, artist, album and
length, and what the queue needs: skip, the unit shuffle keeps together (a
segue, an album never shuffled), why radio picked it, where to resume, and
whether it is from outside the library.

### `TrackKey`

Enum, a setting · [`app/src-tauri/src/library/rules.rs`](../../app/src-tauri/src/library/rules.rs)

What tracks are sorted by within a group, in order of precedence; ties
fall back on the track id.

### `TrackOptions`

Struct · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs) · main thread · [D2: Pressing play on a track](../developer-guide/08-flows.md#pressing-play-on-a-track)

How the engine plays a track (`anomp_track_options`): gain, the part of
the file, a stretch to skip and the crossfade. Made by
`library::playback::track_play` from the row, the preferences and the
analysis.

### `TrackPlay`

Struct · [`app/src-tauri/src/library/playback.rs`](../../app/src-tauri/src/library/playback.rs) · [D2: Pressing play on a track](../developer-guide/08-flows.md#pressing-play-on-a-track)

What the engine needs to play a library track: its file, its part after
trims, its gain (ReplayGain from the tags or the analysis, with the user's
offset) and what to skip. Made by `track_play`; turned into
`anomp::TrackOptions`.

### `TrackPrefs`

Struct, sent to and from the UI · [`app/src-tauri/src/library/prefs.rs`](../../app/src-tauri/src/library/prefs.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

A track's own rules (O7): skip, gain offset, and trims at the start and
end. `None` leaves each to its album or the default.

### `TrackRef`

Helper · [`app/src-tauri/src/library/transfer.rs`](../../app/src-tauri/src/library/transfer.rs)

A track in a data file: its folder's index, its path in it, its start and
its recording id.

### `TrackSummary`

Struct, sent to the UI · [`app/src-tauri/src/library/mod.rs`](../../app/src-tauri/src/library/mod.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

A track as every list shows it: id, absolute path, titles, artists, album,
numbers, length, format, rating, heart and folder. The commonest payload.

### `Transport`

Trait · [`app/src-tauri/src/metadata/http.rs`](../../app/src-tauri/src/metadata/http.rs) · [D2: Fakes](../developer-guide/12-testing.md#fakes)

Fetches a URL, following redirects, with an optional `Authorization`
(a response with an error status is still a response), and posts a body
(ListenBrainz's submissions). Under every `Client`: the metadata
worker's, ListenBrainz's sender's and the update check's. Ureq in the app
(`UreqTransport`), a fake with recorded responses (`http::testing`) in
tests, which never touch the network.

### `TransportError`

Enum · [`app/src-tauri/src/metadata/http.rs`](../../app/src-tauri/src/metadata/http.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

Why a request got no response: unreachable (no network, DNS, refused,
timeout), which makes `Client` back off from the host, or another failure.

### `TrayState`

Struct · [`app/src-tauri/src/shell/tray.rs`](../../app/src-tauri/src/shell/tray.rs) · main thread · [D2: The shell](../developer-guide/06-rust-backend.md#the-shell)

The optional menu-bar controls' icon (F7), shown or removed as the
settings say.

### `Types`

Helper · [`app/src-tauri/src/library/artists.rs`](../../app/src-tauri/src/library/artists.rs)

A MusicBrainz release's primary and secondary types, read from the stored
JSON to group the artist's albums.

### `UpdateCheck`

Struct, sent to the UI · [`app/src-tauri/src/updates.rs`](../../app/src-tauri/src/updates.rs) · [D2: The shell](../developer-guide/06-rust-backend.md#the-shell)

What an update check found: this version, the latest release and its page,
and whether it is newer. The user downloads it themselves.

### `Updates`

Struct · [`app/src-tauri/src/updates.rs`](../../app/src-tauri/src/updates.rs) · its own thread · [D2: The shell](../developer-guide/06-rust-backend.md#the-shell)

The update checker, managed by Tauri: reads the latest GitHub release
through `http::Client` on a thread of its own, when the feature is on.

### `UreqTransport`

Struct · [`app/src-tauri/src/metadata/http.rs`](../../app/src-tauri/src/metadata/http.rs) · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

The real `Transport`, over ureq with the platform's certificate verifier.

### `Usable`

Helper · [`app/src-tauri/src/metadata/jobs.rs`](../../app/src-tauri/src/metadata/jobs.rs)

Which online sources the settings allow now.

### `UserData`

Struct · [`app/src-tauri/src/library/transfer.rs`](../../app/src-tauri/src/library/transfer.rs) · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

The data file F20 exports and imports: settings, sort rules, source
settings, the user's picks, playlists, favourites, ratings, history,
preferences, resume points and the queue, naming tracks by folder and path
so they survive a rebuild.

### `VisualizerSettings`

Struct, a setting · [`app/src-tauri/src/settings.rs`](../../app/src-tauri/src/settings.rs) · [D2: The visualizer](../developer-guide/07-frontend.md#the-visualizer)

The visualizer: which visualization, the cover wall's basis, the frame
rate, sensitivity, colours from the cover, cycling, and calm mode (F18).

### `VisualizerState`

Struct · [`app/src-tauri/src/visualizer.rs`](../../app/src-tauri/src/visualizer.rs) · any thread · [D2: The visualizer](../developer-guide/07-frontend.md#the-visualizer)

The visualizer's subscribers, managed by Tauri: the first starts the
core's analysis on the main thread, the last stops it; frames are sent
from the analysis thread.

### `VolumeWatcher`

Struct · [`app/src-tauri/src/anomp.rs`](../../app/src-tauri/src/anomp.rs) · main thread · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

Reports volumes mounted and unmounted (`anomp_volume_watcher`), so folders
on them are checked again; macOS only for now. Started by
`library::availability`.

### `Watching`

Helper · [`app/src-tauri/src/library/watch.rs`](../../app/src-tauri/src/library/watch.rs)

The watcher and the folders it holds open.

### `WatchState`

Struct · [`app/src-tauri/src/library/watch.rs`](../../app/src-tauri/src/library/watch.rs) · any thread · [D2: The library](../developer-guide/06-rust-backend.md#the-library)

The folders being watched for changes (F9), managed by Tauri: the `notify`
watcher and each folder held open while watched. A change rescans its
folder once things are quiet.

### `Why`

Helper · [`app/src-tauri/src/library/similar.rs`](../../app/src-tauri/src/library/similar.rs)

A reason as scored, by id, before its names are looked up.

### `WindowSettings`

Struct, a setting · [`app/src-tauri/src/settings.rs`](../../app/src-tauri/src/settings.rs)

The windows and what shows outside them: menu-bar controls, the mini
player on top, and track-change notifications.

### `WorkbenchFile`

Struct, sent to the UI · [`app/src-tauri/src/workbench.rs`](../../app/src-tauri/src/workbench.rs)

The file the effects workbench plays: its queue item and track id, title,
artist, file name, format, length, sample rate, channels and bitrate.

### `Worker`

Struct · [`app/src-tauri/src/metadata/jobs.rs`](../../app/src-tauri/src/metadata/jobs.rs) · the metadata worker · [D2: Metadata](../developer-guide/06-rust-backend.md#metadata)

Runs the metadata jobs one step at a time: the user's calls first, then
the queue by priority, through the `Client`, with its own connection and
the image cache, reporting to a `Host`. Apart from its thread and Tauri,
so tests run it step by step with the fake transport and clock.
