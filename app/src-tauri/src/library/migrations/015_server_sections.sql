-- Plex servers (PLAN.md Phase 7f). A Plex server can hold several music
-- libraries, and the user may add some of them: each is a source of its
-- own, so `server_sources` is unique on server, user and `section` (the
-- library's key on the server; '' for a Jellyfin server, whose source is
-- the whole of the user's music). A Plex server's address changes with
-- the network it is reached from, so `connections` keeps the addresses
-- plex.tv last listed for it (JSON), and `address` the one that answered
-- last. A Plex track's file is fetched from a path its id doesn't give:
-- `server_tracks.media_path` keeps it.

-- fts: no search index covers the new columns, so the triggers stay as they are.
CREATE TABLE server_sources_new (
    folder_id    INTEGER PRIMARY KEY REFERENCES folders (id) ON DELETE CASCADE,
    address      TEXT NOT NULL,
    server_id    TEXT NOT NULL,
    server_name  TEXT NOT NULL,
    version      TEXT,
    user_id      TEXT NOT NULL,
    user_name    TEXT NOT NULL,
    added_at     INTEGER NOT NULL,
    sync_cursor  INTEGER,
    section      TEXT NOT NULL DEFAULT '',
    section_name TEXT,
    connections  TEXT,
    UNIQUE (server_id, user_id, section)
) STRICT;

INSERT INTO server_sources_new (folder_id, address, server_id, server_name, version,
                                user_id, user_name, added_at, sync_cursor)
SELECT folder_id, address, server_id, server_name, version, user_id, user_name, added_at,
       sync_cursor
FROM server_sources;

DROP TABLE server_sources;
ALTER TABLE server_sources_new RENAME TO server_sources;

ALTER TABLE server_tracks ADD COLUMN media_path TEXT;
