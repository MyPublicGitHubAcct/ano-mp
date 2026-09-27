-- What the user makes of the library (PLAN.md §4.7): playlists (F1) and
-- smart playlists (F2), favourites and ratings (F3), and where long tracks
-- were left (F17). As with the features' tables (006), the app never writes
-- the user's files: all of it is here, keyed by track, album or artist id,
-- which rescans keep, and which moves and renames keep too (F10).

-- A playlist is a list of tracks in the user's order, or, with `rules`
-- (JSON, library/smart.rs), a smart playlist whose tracks are whatever
-- matches them now; a smart playlist has no items.
CREATE TABLE playlists (
    id         INTEGER PRIMARY KEY,
    name       TEXT NOT NULL,
    rules      TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;

-- `position` orders a playlist's items; gaps are allowed. A track may be
-- listed more than once.
CREATE TABLE playlist_items (
    id          INTEGER PRIMARY KEY,
    playlist_id INTEGER NOT NULL REFERENCES playlists (id) ON DELETE CASCADE,
    position    INTEGER NOT NULL,
    track_id    INTEGER NOT NULL REFERENCES tracks (id) ON DELETE CASCADE
) STRICT;

CREATE INDEX playlist_items_order ON playlist_items (playlist_id, position);
CREATE INDEX playlist_items_track ON playlist_items (track_id);

-- Favourites: a heart on a track, an album or an artist.
CREATE TABLE track_favourites (
    track_id INTEGER PRIMARY KEY REFERENCES tracks (id) ON DELETE CASCADE,
    added_at INTEGER NOT NULL
) STRICT;

CREATE TABLE album_favourites (
    album_id INTEGER PRIMARY KEY REFERENCES albums (id) ON DELETE CASCADE,
    added_at INTEGER NOT NULL
) STRICT;

CREATE TABLE artist_favourites (
    artist_id INTEGER PRIMARY KEY REFERENCES artists (id) ON DELETE CASCADE,
    added_at  INTEGER NOT NULL
) STRICT;

-- Ratings in whole stars. A rating the tags give is copied ('tags') and
-- follows the tags when they change, until the user rates the track: the
-- user's own replaces it ('user'), and a 'user' row with no rating is one
-- the user cleared, which the tags never fill in again. Nothing is written
-- back to the files.
CREATE TABLE track_ratings (
    track_id INTEGER PRIMARY KEY REFERENCES tracks (id) ON DELETE CASCADE,
    rating   INTEGER CHECK (rating BETWEEN 1 AND 5),
    source   TEXT NOT NULL CHECK (source IN ('tags', 'user'))
) STRICT;

-- Where a long track (an audiobook, a DJ mix) was left, in seconds, to
-- carry on from there next time.
CREATE TABLE track_positions (
    track_id INTEGER PRIMARY KEY REFERENCES tracks (id) ON DELETE CASCADE,
    position REAL NOT NULL,
    saved_at INTEGER NOT NULL
) STRICT;

-- What the user chose for an album or an artist that no track refers to any
-- more (F10): its picks as JSON (the chosen links, cover, playback
-- preferences, favourite), kept for when it comes back, e.g. when its files
-- move from one library folder to another. An album comes back by its
-- release MBID, or its album artist and title; an artist by name.
CREATE TABLE kept_albums (
    id                     INTEGER PRIMARY KEY,
    title                  TEXT NOT NULL COLLATE NOCASE,
    artist                 TEXT COLLATE NOCASE,
    musicbrainz_release_id TEXT,
    picks                  TEXT NOT NULL,
    kept_at                INTEGER NOT NULL
) STRICT;

CREATE INDEX kept_albums_identity ON kept_albums (title, artist);

CREATE TABLE kept_artists (
    name    TEXT PRIMARY KEY COLLATE NOCASE,
    picks   TEXT NOT NULL,
    kept_at INTEGER NOT NULL
) STRICT;
