-- Music servers as library sources (PLAN.md Phase 7e). A folder is now
-- either a folder on disk (`local`, as before) or a music server's library
-- as one user sees it (`jellyfin`; `plex` is allowed for Phase 7f). A
-- server's folder has no bookmark; its `path` is a key that names no file
-- (`jellyfin:<server id>/<user id>`), and its tracks' `relative_path` is the
-- server's item id, never a URL. Everything else about tracks, albums,
-- artists, search and playlists is as for local files.

-- fts: no search index covers the new column or tables, so the triggers stay as they are.
ALTER TABLE folders ADD COLUMN kind TEXT NOT NULL DEFAULT 'local'
    CHECK (kind IN ('local', 'jellyfin', 'plex'));

-- The server behind a server folder: where it is and who signed in. The
-- access token is in the OS keychain (`metadata::keys`), never here.
-- `sync_cursor` is when the last sync that finished started (Unix
-- seconds): the next asks only for items changed since, less a margin.
CREATE TABLE server_sources (
    folder_id   INTEGER PRIMARY KEY REFERENCES folders (id) ON DELETE CASCADE,
    address     TEXT NOT NULL,
    server_id   TEXT NOT NULL,
    server_name TEXT NOT NULL,
    version     TEXT,
    user_id     TEXT NOT NULL,
    user_name   TEXT NOT NULL,
    added_at    INTEGER NOT NULL,
    sync_cursor INTEGER,
    UNIQUE (server_id, user_id)
) STRICT;

-- What a server says about each of its tracks beyond the `tracks` columns:
-- the file's container ("flac", "mp3", "m4a"…), which decides whether it
-- is fetched as it is or transcoded, and the URL of its album's picture,
-- which the sync downloads into the image cache.
CREATE TABLE server_tracks (
    track_id  INTEGER PRIMARY KEY REFERENCES tracks (id) ON DELETE CASCADE,
    container TEXT,
    art_url   TEXT
) STRICT;

-- The server's playlists, which come in read-only: each is a playlist of
-- the library's, refilled by every sync, and deleted with its source.
CREATE TABLE server_playlists (
    playlist_id INTEGER PRIMARY KEY REFERENCES playlists (id) ON DELETE CASCADE,
    folder_id   INTEGER NOT NULL REFERENCES folders (id) ON DELETE CASCADE,
    item_id     TEXT NOT NULL,
    UNIQUE (folder_id, item_id)
) STRICT;

CREATE TRIGGER server_playlists_gone AFTER DELETE ON server_playlists
BEGIN
    DELETE FROM playlists WHERE id = old.playlist_id;
END;

-- "Keep available offline" (7e.6): albums and playlists whose server
-- tracks stay in the audio cache, downloaded ahead and never evicted.
CREATE TABLE offline_albums (
    album_id  INTEGER PRIMARY KEY REFERENCES albums (id) ON DELETE CASCADE,
    pinned_at INTEGER NOT NULL
) STRICT;

CREATE TABLE offline_playlists (
    playlist_id INTEGER PRIMARY KEY REFERENCES playlists (id) ON DELETE CASCADE,
    pinned_at   INTEGER NOT NULL
) STRICT;
