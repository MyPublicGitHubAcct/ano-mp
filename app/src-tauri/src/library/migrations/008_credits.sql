-- Compilations and multiple artists (PLAN.md F11).
--
-- A track's artist tag may credit several artists ("A; B", or a
-- multi-valued ARTISTS tag). Each is an artist of its own, listed in
-- `track_artists` in the tag's order, so each is browsed, has a page, and
-- is matched on MusicBrainz alone. `tracks.artist_id` is the first of them,
-- and `artist_credit` keeps the tag's text for display when it names more
-- than that one artist.
--
-- A compilation tagged without an album artist used to split into an album
-- per track artist. The scanner now files it under "Various Artists" when
-- the file is marked as a compilation, or when one album title in one
-- folder has three or more track artists. `album_artist_tagged` records
-- whether the file named its album artist, which the latter needs.

ALTER TABLE tracks ADD COLUMN artist_credit TEXT;
ALTER TABLE tracks ADD COLUMN compilation INTEGER NOT NULL DEFAULT 0;
ALTER TABLE tracks ADD COLUMN album_artist_tagged INTEGER NOT NULL DEFAULT 0;

CREATE TABLE track_artists (
    track_id  INTEGER NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    artist_id INTEGER NOT NULL REFERENCES artists (id),
    position  INTEGER NOT NULL,
    PRIMARY KEY (track_id, artist_id)
) STRICT, WITHOUT ROWID;

CREATE INDEX track_artists_artist ON track_artists (artist_id);

INSERT INTO track_artists (track_id, artist_id, position)
SELECT id, artist_id, 0 FROM tracks WHERE artist_id IS NOT NULL;

-- The search index finds a track by its whole credit (migration 002's rule:
-- a change to what the index covers updates its triggers).
DROP TRIGGER tracks_search_insert;
DROP TRIGGER tracks_search_update;

CREATE TRIGGER tracks_search_insert AFTER INSERT ON tracks BEGIN
    INSERT INTO tracks_search (rowid, title, artist, album, album_artist, work, composer)
    VALUES (
        new.id,
        IFNULL(new.title, substr(new.relative_path,
            length(rtrim(new.relative_path, replace(new.relative_path, '/', ''))) + 1)),
        IFNULL(new.artist_credit, (SELECT name FROM artists WHERE id = new.artist_id)),
        (SELECT title FROM albums WHERE id = new.album_id),
        (SELECT name FROM artists WHERE id = new.album_artist_id),
        new.work,
        (SELECT name FROM artists WHERE id = new.composer_id)
    );
END;

CREATE TRIGGER tracks_search_update
AFTER UPDATE OF title, relative_path, artist_id, artist_credit, album_id, album_artist_id, work,
    composer_id
ON tracks
WHEN new.title IS NOT old.title OR new.relative_path IS NOT old.relative_path
    OR new.artist_id IS NOT old.artist_id OR new.artist_credit IS NOT old.artist_credit
    OR new.album_id IS NOT old.album_id OR new.album_artist_id IS NOT old.album_artist_id
    OR new.work IS NOT old.work OR new.composer_id IS NOT old.composer_id BEGIN
    DELETE FROM tracks_search WHERE rowid = old.id;
    INSERT INTO tracks_search (rowid, title, artist, album, album_artist, work, composer)
    VALUES (
        new.id,
        IFNULL(new.title, substr(new.relative_path,
            length(rtrim(new.relative_path, replace(new.relative_path, '/', ''))) + 1)),
        IFNULL(new.artist_credit, (SELECT name FROM artists WHERE id = new.artist_id)),
        (SELECT title FROM albums WHERE id = new.album_id),
        (SELECT name FROM artists WHERE id = new.album_artist_id),
        new.work,
        (SELECT name FROM artists WHERE id = new.composer_id)
    );
END;

-- The new tags (and the ratings of 007) aren't read yet: the next scan
-- re-reads every file, since no size or modification time matches -1.
UPDATE tracks SET file_mtime_ns = -1;
