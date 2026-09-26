-- Initial library schema. Times are Unix seconds unless noted.

-- Folders the user added to the library. `path` is where the folder was
-- last found; `bookmark` will hold the macOS/iOS security-scoped bookmark,
-- the durable handle, once the sandbox work lands (PLAN.md Phase 2).
CREATE TABLE folders (
    id           INTEGER PRIMARY KEY,
    path         TEXT NOT NULL UNIQUE,
    bookmark     BLOB,
    added_at     INTEGER NOT NULL,
    last_scan_at INTEGER
) STRICT;

-- Track artists and album artists alike, one row per name (compared
-- case-insensitively for ASCII). Several artists joined with "; " in a tag
-- are one name for now.
CREATE TABLE artists (
    id             INTEGER PRIMARY KEY,
    name           TEXT NOT NULL COLLATE NOCASE UNIQUE,
    musicbrainz_id TEXT
) STRICT;

-- An album is a title by an album artist (the track artist when a file has
-- no album artist).
CREATE TABLE albums (
    id                           INTEGER PRIMARY KEY,
    title                        TEXT NOT NULL COLLATE NOCASE,
    artist_id                    INTEGER REFERENCES artists (id),
    musicbrainz_release_id       TEXT,
    musicbrainz_release_group_id TEXT
) STRICT;

CREATE UNIQUE INDEX albums_identity ON albums (IFNULL(artist_id, 0), title);

-- One row per playable file. Paths are stored relative to their folder,
-- with '/' separators, so they survive the folder moving (a bookmark
-- resolving to a new path, e.g. an iOS app container after an update).
CREATE TABLE tracks (
    id                           INTEGER PRIMARY KEY,
    folder_id                    INTEGER NOT NULL REFERENCES folders (id) ON DELETE CASCADE,
    relative_path                TEXT NOT NULL,
    -- Change detection: the file is re-read when either differs.
    file_size                    INTEGER NOT NULL,
    file_mtime_ns                INTEGER NOT NULL,
    title                        TEXT,
    artist_id                    INTEGER REFERENCES artists (id),
    album_id                     INTEGER REFERENCES albums (id),
    -- The effective album artist: the tag, else the track artist.
    album_artist_id              INTEGER REFERENCES artists (id),
    genre                        TEXT,
    track_number                 INTEGER,
    track_total                  INTEGER,
    disc_number                  INTEGER,
    disc_total                   INTEGER,
    year                         INTEGER,
    -- Seconds, from the file's headers (approximate for lossy files).
    duration                     REAL NOT NULL,
    sample_rate                  INTEGER NOT NULL,
    channels                     INTEGER NOT NULL,
    bitrate_kbps                 INTEGER,
    musicbrainz_recording_id     TEXT,
    musicbrainz_release_track_id TEXT,
    scanned_at                   INTEGER NOT NULL,
    UNIQUE (folder_id, relative_path)
) STRICT;

CREATE INDEX tracks_artist ON tracks (artist_id);
CREATE INDEX tracks_album ON tracks (album_id);
CREATE INDEX tracks_album_artist ON tracks (album_artist_id);

-- App settings as JSON values; the typed schema is in Rust (PLAN.md Phase 6).
CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT;

-- MusicBrainz / Cover Art Archive responses (PLAN.md Phase 4), keyed by the
-- request path and query.
CREATE TABLE mb_cache (
    request    TEXT PRIMARY KEY,
    response   TEXT NOT NULL,
    fetched_at INTEGER NOT NULL
) STRICT;
