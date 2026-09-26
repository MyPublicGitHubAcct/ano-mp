-- Metadata from online services and local files (PLAN.md Phase 4,
-- src/metadata/). The metadata code fills these tables and the scanner never
-- touches them, so a rescan doesn't overwrite a match; album and artist ids
-- survive rescans because the scanner upserts them. `source` is a source id
-- from metadata::settings, e.g. 'musicbrainz'.

-- What a source knows about an album: its id there and the details fetched
-- from it. One row per album and source, so an album can be linked to
-- several sources at once.
CREATE TABLE album_links (
    album_id    INTEGER NOT NULL REFERENCES albums (id) ON DELETE CASCADE,
    source      TEXT NOT NULL,
    -- 'matched': `external_id` is the album there. 'review': `external_id`
    -- is the best candidate, not good enough to accept without the user.
    -- 'none': the source was searched and had nothing (retried later).
    status      TEXT NOT NULL CHECK (status IN ('matched', 'review', 'none')),
    external_id TEXT,
    -- 0 to 1; 1 for an id from the tags or one the user chose.
    score       REAL NOT NULL,
    -- Automatic runs never replace a row the user chose.
    chosen_by   TEXT NOT NULL CHECK (chosen_by IN ('auto', 'user')),
    -- Normalized fields from the source, as JSON; NULL until fetched.
    details     TEXT,
    checked_at  INTEGER NOT NULL,
    PRIMARY KEY (album_id, source)
) STRICT;

-- The same for artists.
CREATE TABLE artist_links (
    artist_id   INTEGER NOT NULL REFERENCES artists (id) ON DELETE CASCADE,
    source      TEXT NOT NULL,
    status      TEXT NOT NULL CHECK (status IN ('matched', 'review', 'none')),
    external_id TEXT,
    score       REAL NOT NULL,
    chosen_by   TEXT NOT NULL CHECK (chosen_by IN ('auto', 'user')),
    details     TEXT,
    checked_at  INTEGER NOT NULL,
    PRIMARY KEY (artist_id, source)
) STRICT;

-- The picture the user chose for an album, overriding the source order.
-- Albums without a row get the first picture found in that order.
CREATE TABLE album_art (
    album_id  INTEGER PRIMARY KEY REFERENCES albums (id) ON DELETE CASCADE,
    source    TEXT NOT NULL,
    -- Which picture: for 'folder', the image's path relative to its library
    -- folder ('/'-separated, like tracks); for an online source, the image
    -- URL; NULL for 'embedded'.
    reference TEXT
) STRICT;
