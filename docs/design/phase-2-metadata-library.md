# Phase 2 — Metadata and library

Moved from `PLAN.md` (H20) on 2026-10-03, unchanged but for this
heading. Section numbers (§) refer to `PLAN.md`, whose Phase 2
keeps the phase's status, decisions and open steps.

- [x] Core: add TagLib (FetchContent) with `anomp_read_tags(path) → struct` for
  title, artist, album, album artist, track/disc, year, genre, duration,
  MusicBrainz IDs (if tagged), and embedded art.

  Done 2026-09-26:
  - TagLib **2.3.2** from the release tarball (SHA-256 pinned; it bundles
    utfcpp, which is forced over any installed copy), static, no zlib
    (compressed ID3v2 frames are rare and are skipped without it). TagLib's
    target only exports include paths for installs, so `cmake/TagLib.cmake`
    adds them. `build.rs` links `libtag.a` next to the core.
  - `anomp_read_tags (path, flags, error)` returns an `anomp_tags*` freed
    with `anomp_tags_free`; strings are UTF-8 and never null. Unlike the
    engine it may be called from any thread, for the scanner. The embedded
    picture is copied only with `ANOMP_TAGS_PICTURE`, so a scan doesn't
    copy art for every track.
  - Fields come from TagLib's unified `PropertyMap`, so every format maps the
    same way: several values join with "; "; track/disc accept "3/12" or a
    separate TRACKTOTAL/DISCTOTAL; the year is the first four digits of
    DATE. MusicBrainz IDs are named after their entity
    (`musicbrainz_recording_id` is Picard's "track id").
  - The picture is the one typed "Front Cover", else the first (MP4 cover
    atoms have no type). An empty MIME type is sniffed from JPEG/PNG magic.
  - Files are opened through a read-only `FileStream`.
  - **Duration comes from TagLib's headers**, which is cheap enough for
    scanning but approximate for lossy files: MP3 lengths include encoder
    delay and padding (~40 ms). The player's `anomp_engine_duration` is the
    exact one. If TagLib gives no duration, the FFmpeg reader supplies the
    audio properties; a file neither can read is an error.
  - Known limits (of the formats): ASF/WMA holds one artist string, and
    ID3v2 keeps the recording ID in a UFID frame.
  - Tests: `TagReaderTests.cpp` reads an ID3v2.3 MP3 and a FLAC tagged by
    FFmpeg (non-ASCII text, MusicBrainz IDs, PNG cover), checks untagged
    fixtures' properties, round-trips tags and two pictures written by TagLib
    through all ten formats, and checks errors and that files are untouched.
- [x] Rust: SQLite schema (tracks, albums, artists, folders, settings, mb_cache),
  migrations, and an incremental folder scanner (mtime/size change detection).

  Done 2026-09-26 (`app/src-tauri/src/library/`):
  - `rusqlite` with its bundled SQLite (the same version on every platform,
    iOS included). The database is `library.sqlite3` in the app data folder,
    in WAL mode with foreign keys on. Migrations are numbered SQL files in
    `library/migrations/`, applied in order and counted in `PRAGMA
    user_version`. A database from a newer app version is refused, not
    touched. Tables are `STRICT`.
  - **Tracks store their path relative to their folder** ('/'-separated), so
    a folder that moves keeps its tracks. On iOS the app container path
    changes between installs, and bookmarks resolve to the new path.
    `folders.bookmark` (a BLOB, unused so far) is ready for the sandbox item
    below. Adding a folder canonicalizes its path and refuses one that is
    inside, or contains, a folder already in the library.
  - Artists have one row per name (`COLLATE NOCASE`, ASCII only). An album is
    (album artist, title), and the album artist falls back to the track
    artist. A unique index on `IFNULL(artist_id, 0)` covers albums without
    one. Tracks point at their artist, album and effective album artist.
    Release MBIDs go on albums, artist MBIDs on artists, recording and
    release-track MBIDs on tracks. Albums and artists that no track uses are
    deleted after each scan and folder removal. `settings` (key → JSON) and
    `mb_cache` (request → response) are schema only, for Phases 4 and 6.
  - **Scanner:** walks the folder with `walkdir`, following symlinks (loops
    are detected and reported) and skipping dotfiles and dot-folders (which
    also skips macOS's `._` AppleDouble files). It keeps the files whose
    extension the core decodes. A file is re-read when its size or
    modification time (ns) differs. Tags are read without pictures, on one
    thread per core, and written 256 files per transaction, with a progress
    callback after each batch. Updates are upserts, so track IDs stay stable.
    Unreadable files are reported and left out (a track that becomes
    unreadable is removed). If the folder itself is missing (e.g. an
    unmounted drive), the scan fails and changes nothing. Tracks under a
    subfolder that can't be read are kept.
  - Tauri: `library_folders`, `library_add_folder`, `library_remove_folder`,
    `library_tracks` and `library_scan` (one folder or all; one scan at a
    time; runs on a blocking thread with its own connection and emits
    `library-scan-progress`). The dev UI has a library panel; double-clicking
    a track plays it.
  - Speed (release build on the dev Mac, warm cache): 5,000 small fixture
    files take 0.46 s to scan the first time and 27 ms to rescan. Phase 7
    measures real libraries.
  - Known limits: compilations tagged without an album artist split into one
    album per track artist. Several artists in one tag ("A; B") count as one
    artist. Folder overlap checks compare paths case-sensitively.
    `library_tracks` returned every row unsorted; `library_browse` (below)
    replaced it. Scans run only when asked, with no rescan at launch and no
    file watching yet.
- [x] Logical sort/grouping rules: by album artist → album → disc/track, by folder,
  by genre, by year. Rules are configurable (feeds the admin screen).

  Done 2026-09-26 (`library/rules.rs`, `browse.rs`, `sort_key.rs`,
  `genres.rs`):
  - **Rules are data.** A rule is an id, a name, a list of grouping levels
    (album artist, artist, album, genre, year, or the folder tree on its
    own) and a track order (album artist, artist, album, year, disc, track
    number, title, path). The built-ins are album artist → album, genre →
    album artist → album (both with tracks by disc, number, title, path),
    year → album, and folder. The rules and the ignored leading articles
    (default "The" and "A") are one JSON value under `library.sort` in
    `settings`; there is no schema change. Reading it keeps what is
    usable: a rule that doesn't parse or validate (e.g. from a newer
    version) is dropped, and anything missing or invalid falls back to the
    built-ins.
  - **Browsing is one node at a time.** `library_browse(ruleId, path,
    offset, limit)` returns a page of the node's groups (key, display name,
    track count; albums also carry album artist and year) or tracks, and
    the node's total. The path holds one group key per level (an id, a
    year, a genre; null for "Unknown …"). Under the folder rule it is a
    library folder id and then folder names, and a node lists subfolders,
    then tracks, paged as one list. Paging is limit (at most 1000) + offset
    over an order that always ends in a unique key, so pages don't repeat
    or skip rows. SQL is assembled only from fixed fragments chosen by the
    rule; every value is a bound parameter.
  - **Sort order** comes from `anomp_sort_key(text, articles)`, an SQL
    function registered on every connection in `db::configure`. It returns
    a BLOB that sorts bytewise: text folded with ICU (NFKD, marks removed,
    lowercase, so "élodie" sorts with "Elodie"), digit runs by value
    ("Track 2" before "Track 10"), '/' first (a folder's files before its
    neighbours'), and an optional leading article skipped ("The Beatles"
    under B; display names are unchanged). This replaces the planned custom
    collation: a key is computed once per row and compared with memcmp,
    whereas a collation folds both strings on every comparison, and the
    articles arrive as a bound parameter rather than per-connection state.
    Ties fall back to the original text, then the id. Missing values sort
    last. A missing disc number counts as disc 1, since single-disc albums
    usually have none. Genres that differ only in case or accents are one
    group. `icu_normalizer`/`icu_properties` were already linked (via
    Tauri's `url`), so folding adds no new code.
  - **Genres:** a track is under each of its genres. `anomp_genres(tag)`
    splits a tag at ';' (the core joins values with "; ") into a JSON
    array for SQLite's `json_each`, and `anomp_has_genre(tag, genre)`
    filters. Splitting at query time avoids a `track_genres` table and a
    scanner change. A recursive CTE did the same, but at 4× the cost.
  - **Year** is the album's year (the earliest among its tracks, from an
    `album_years` CTE joined only when the rule groups by year), else the
    track's own.
  - Speed (release build, 50,000 synthetic tracks in an in-memory database,
    count + page of 200): album artists 28 ms, one album's tracks 0.2 ms,
    genres 71 ms, one genre's artists 63 ms, years 62 ms, one year's albums
    63 ms, every track by title 34 ms (85 ms at offset 40,000), a folder of
    2,000 subfolders 54 ms. Group levels scan every track, so they grow
    linearly. A keyset cursor or cached keys are the next steps if Phase 7
    finds this too slow.
  - Tauri: `library_browse`, `library_sort_settings`,
    `library_save_sort_rule` (adds, or replaces by id),
    `library_remove_sort_rule` (not the last), `library_set_ignored_articles`
    and `library_reset_sort_settings`. `TrackSummary` gained genre and year.
    The dev UI has a rule picker, breadcrumbs, a click-through list with
    "Show more", and double-click to play.
  - Known limits: compilations tagged without an album artist still split
    into one album per track artist (left for now). Folding doesn't
    decompose letters like "ø", "ł" or "ß", and CJK sorts by code point
    rather than by reading. Articles are skipped only before a space ("The
    Beatles", not "L'Amour"). Genres split only at ';', not '/' or ','
    ("Hip-Hop/Rap" is one genre). Two artists whose names differ only in
    non-ASCII case (the schema's `NOCASE`) are separate, adjacent groups.
    The genre group's display name is the spelling that sorts first
    bytewise, not the most common one. Pages come from separate queries, so
    a scan between them can shift rows.
- [x] macOS sandbox: user-selected folders plus security-scoped bookmarks. Store
  bookmarks, not raw paths, so the same model works on iOS later.

  Done 2026-09-26 (`core/src/FolderAccess*`, `library/access.rs`,
  `Entitlements.plist`):
  - **Core:** `FolderAccess` is the platform interface: create a bookmark
    for a folder, resolve one and hold access while the object lives. The
    Apple implementation (`FolderAccess_apple.mm`, ARC) makes read-only
    security-scoped bookmarks on macOS (plain ones on iOS, where the scope is
    implicit; not yet built) and resolves them without UI or mounting. Other
    platforms (`FolderAccess_unsandboxed.cpp`) store the UTF-8 path. The C
    API is `anomp_bookmark_create`/`_free` and
    `anomp_folder_access_start`/`_path`/`_is_stale`/`_stop`, callable from
    any thread. NSURL spells resolved names decomposed (NFD), so the path
    goes through `realpath` to match what `add_folder` stores.
  - **Rust:** `add_folder` saves the bookmark in `folders.bookmark` (no
    schema change). `access::open_folder` resolves it before each scan and
    before each `player_load`/`player_set_next` of a file under a library
    folder, and holds access until the scan ends or the file is open (the
    reader keeps its stream, so rewinds don't reopen). If the folder moved,
    its stored path follows (after the overlap check), and so do its tracks,
    which are stored relative to it. A stale bookmark is replaced. A folder
    saved before this change gets a bookmark the next time it can be read.
    An unresolvable bookmark reports "Folder not available" and changes
    nothing.
  - **Sandbox:** `Entitlements.plist` has the app sandbox, user-selected
    read-only files, app-scoped bookmarks and network client (for Phase 4;
    §8.3 lists it). `tauri.conf.json` signs the bundle ad hoc (`"-"`) so
    local builds carry the entitlements, with the hardened runtime off: it
    loads only libraries signed by the app's team, and ad-hoc signatures
    have none, so no separately signed dylib could load. Release signing
    (§8.3) sets a Developer ID and turns the hardened runtime back on.
    `tauri dev` is unsigned and so unsandboxed. The sandboxed app keeps its
    database in `~/Library/Containers/dev.anomp.player/`, separate from the
    unsandboxed one, so folders must be added again there.
  - **Release builds and FFmpeg:** `build.rs` now links clang's runtime
    (`libclang_rt.osx.a`, found with `xcrun`): JUCE's `@available` checks
    need `___isPlatformVersionAtLeast`, which debug builds took from Rust's
    std but release LTO dropped, so no release build had linked. The bundle
    embeds FFmpeg (`bundle.macOS.frameworks`, pulled forward from §8.3):
    Tauri copies the four dylibs under their install names into
    `Contents/Frameworks`, signs them, and adds the
    `@executable_path/../Frameworks` rpath. Only debug builds keep an rpath
    into `third_party/` (for `tauri dev` and `cargo test`); a release app
    never searches there, and a path under `~/Desktop` would make macOS ask
    for access. `cargo test --release` finds FFmpeg through
    `DYLD_LIBRARY_PATH` from `app/src-tauri/.cargo/config.toml`. The
    framework list names the libraries' major versions, so it changes with
    the FFmpeg pin (a stale name fails the bundle step).
  - Tests: `FolderAccessTests.cpp` (round trip, a moved folder, errors,
    null handles) and `cargo test` (`anomp.rs`; `access.rs`: saved on add,
    backfill, a moved folder, an unavailable folder; `scanner.rs`: a moved
    folder rescans with the same track ids, a deleted one keeps its tracks).
    These run unsandboxed. The bundle from `npm run tauri build --
    --bundles app` launches in the sandbox as built: FFmpeg loads from
    `Contents/Frameworks`, the database is in the container, and no
    denials are logged.
  - Checked by hand 2026-09-26 in the sandboxed bundle: add a folder, quit,
    relaunch, then scan it and play a track from it.
  - Known limits: symlinks leading out of a library folder aren't readable
    in the sandbox and are reported as scan failures.
- **Tests:** Catch2 tests for tag reading over fixture files (done, above);
  `cargo test` for schema, scanner (done: `library/db.rs`, `library/mod.rs`
  and `library/scanner.rs`, over temp folders of fixture copies) and sort
  rules (done: `sort_key.rs`, `genres.rs`, `rules.rs` and `browse.rs`, over
  in-memory databases of synthetic rows: disc/track order, missing values,
  accents and case, articles, natural numbers, the folder tree, multiple
  genres, album years, paging, and saving, resetting and falling back from
  bad stored settings). **Phase 2 complete.**
