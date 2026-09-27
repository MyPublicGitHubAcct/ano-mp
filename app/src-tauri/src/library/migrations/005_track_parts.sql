-- Tracks as parts of files, and the tags the optional features read
-- (PLAN.md §4.6). A track is now (folder, path, start): a cue sheet's
-- tracks or a file's chapters (O5) are parts of one file. SQLite can't
-- change a table's unique key in place, so `tracks` is rebuilt, keeping
-- every id, and its search index and triggers with it.

CREATE TABLE tracks_new (
    id                           INTEGER PRIMARY KEY,
    folder_id                    INTEGER NOT NULL REFERENCES folders (id) ON DELETE CASCADE,
    relative_path                TEXT NOT NULL,
    -- The part of the file this track is: seconds from the start of its
    -- audio, and NULL for a track that runs to the end of the file.
    range_start                  REAL NOT NULL DEFAULT 0,
    range_end                    REAL,
    file_size                    INTEGER NOT NULL,
    file_mtime_ns                INTEGER NOT NULL,
    title                        TEXT,
    artist_id                    INTEGER REFERENCES artists (id),
    album_id                     INTEGER REFERENCES albums (id),
    album_artist_id              INTEGER REFERENCES artists (id),
    genre                        TEXT,
    track_number                 INTEGER,
    track_total                  INTEGER,
    disc_number                  INTEGER,
    disc_total                   INTEGER,
    year                         INTEGER,
    duration                     REAL NOT NULL,
    sample_rate                  INTEGER NOT NULL,
    channels                     INTEGER NOT NULL,
    bitrate_kbps                 INTEGER,
    musicbrainz_recording_id     TEXT,
    musicbrainz_release_track_id TEXT,
    scanned_at                   INTEGER NOT NULL,
    replaygain_track_gain        REAL,
    replaygain_track_peak        REAL,
    replaygain_album_gain        REAL,
    replaygain_album_peak        REAL,
    -- When the file first came into the library (O15), Unix seconds. Set
    -- when the scanner inserts the row; rescans leave it alone.
    added_at                     INTEGER NOT NULL DEFAULT 0,
    -- The release date as precise as the tags give it ("2004-05-01",
    -- "2004-05" or "2004"), the original release's where tagged (O17).
    release_date                 TEXT,
    -- Classical works (O6): the work a track is a movement of, and the
    -- movement; the composer is an artist, so it can be browsed.
    work                         TEXT,
    movement_name                TEXT,
    movement_number              INTEGER,
    movement_total               INTEGER,
    composer_id                  INTEGER REFERENCES artists (id),
    conductor                    TEXT,
    UNIQUE (folder_id, relative_path, range_start)
) STRICT;

-- Tracks already there arrived about when their file was last changed:
-- their scan times are all the same first scan.
INSERT INTO tracks_new (
    id, folder_id, relative_path, file_size, file_mtime_ns, title, artist_id, album_id,
    album_artist_id, genre, track_number, track_total, disc_number, disc_total, year, duration,
    sample_rate, channels, bitrate_kbps, musicbrainz_recording_id, musicbrainz_release_track_id,
    scanned_at, replaygain_track_gain, replaygain_track_peak, replaygain_album_gain,
    replaygain_album_peak, added_at)
SELECT
    id, folder_id, relative_path, file_size, file_mtime_ns, title, artist_id, album_id,
    album_artist_id, genre, track_number, track_total, disc_number, disc_total, year, duration,
    sample_rate, channels, bitrate_kbps, musicbrainz_recording_id, musicbrainz_release_track_id,
    scanned_at, replaygain_track_gain, replaygain_track_peak, replaygain_album_gain,
    replaygain_album_peak,
    CASE WHEN file_mtime_ns > 0 THEN file_mtime_ns / 1000000000 ELSE scanned_at END
FROM tracks;

DROP TABLE tracks;
ALTER TABLE tracks_new RENAME TO tracks;

CREATE INDEX tracks_artist ON tracks (artist_id);
CREATE INDEX tracks_album ON tracks (album_id);
CREATE INDEX tracks_album_artist ON tracks (album_artist_id);
CREATE INDEX tracks_composer ON tracks (composer_id);
CREATE INDEX tracks_added ON tracks (added_at);

-- The search index gains the work and the composer. Its triggers went with
-- the old table.
DROP TABLE tracks_search;

CREATE VIRTUAL TABLE tracks_search USING fts5 (
    title, artist, album, album_artist, work, composer,
    content = '', contentless_delete = 1, tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER tracks_search_insert AFTER INSERT ON tracks BEGIN
    INSERT INTO tracks_search (rowid, title, artist, album, album_artist, work, composer)
    VALUES (
        new.id,
        IFNULL(new.title, substr(new.relative_path,
            length(rtrim(new.relative_path, replace(new.relative_path, '/', ''))) + 1)),
        (SELECT name FROM artists WHERE id = new.artist_id),
        (SELECT title FROM albums WHERE id = new.album_id),
        (SELECT name FROM artists WHERE id = new.album_artist_id),
        new.work,
        (SELECT name FROM artists WHERE id = new.composer_id)
    );
END;

CREATE TRIGGER tracks_search_update
AFTER UPDATE OF title, relative_path, artist_id, album_id, album_artist_id, work, composer_id
ON tracks
WHEN new.title IS NOT old.title OR new.relative_path IS NOT old.relative_path
    OR new.artist_id IS NOT old.artist_id OR new.album_id IS NOT old.album_id
    OR new.album_artist_id IS NOT old.album_artist_id OR new.work IS NOT old.work
    OR new.composer_id IS NOT old.composer_id BEGIN
    DELETE FROM tracks_search WHERE rowid = old.id;
    INSERT INTO tracks_search (rowid, title, artist, album, album_artist, work, composer)
    VALUES (
        new.id,
        IFNULL(new.title, substr(new.relative_path,
            length(rtrim(new.relative_path, replace(new.relative_path, '/', ''))) + 1)),
        (SELECT name FROM artists WHERE id = new.artist_id),
        (SELECT title FROM albums WHERE id = new.album_id),
        (SELECT name FROM artists WHERE id = new.album_artist_id),
        new.work,
        (SELECT name FROM artists WHERE id = new.composer_id)
    );
END;

CREATE TRIGGER tracks_search_delete AFTER DELETE ON tracks BEGIN
    DELETE FROM tracks_search WHERE rowid = old.id;
END;

INSERT INTO tracks_search (rowid, title, artist, album, album_artist, work, composer)
SELECT t.id,
       IFNULL(t.title, substr(t.relative_path,
           length(rtrim(t.relative_path, replace(t.relative_path, '/', ''))) + 1)),
       artist.name, album.title, album_artist.name, NULL, NULL
FROM tracks t
LEFT JOIN artists artist ON artist.id = t.artist_id
LEFT JOIN albums album ON album.id = t.album_id
LEFT JOIN artists album_artist ON album_artist.id = t.album_artist_id;

-- The new tags aren't read yet: the next scan re-reads every file, since no
-- size or modification time matches -1 (as migration 004 did).
UPDATE tracks SET file_mtime_ns = -1;
