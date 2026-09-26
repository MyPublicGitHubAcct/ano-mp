-- Full-text search over artists, albums and tracks (library/search.rs).
--
-- Contentless FTS5 tables: each row's rowid is the id of the row it indexes,
-- and the text lives only in the index. The unicode61 tokenizer folds case
-- and removes diacritics itself, so search needs none of the anomp_* SQL
-- functions, which must never appear in the schema. Triggers keep the
-- indexes in step with the tables; they fire only when an indexed value
-- changes, so a rescan that upserts unchanged rows doesn't churn them.

CREATE VIRTUAL TABLE artists_search USING fts5 (
    name,
    content = '', contentless_delete = 1, tokenize = 'unicode61 remove_diacritics 2'
);

CREATE VIRTUAL TABLE albums_search USING fts5 (
    title, artist,
    content = '', contentless_delete = 1, tokenize = 'unicode61 remove_diacritics 2'
);

-- A track without a title is found by its file name.
CREATE VIRTUAL TABLE tracks_search USING fts5 (
    title, artist, album, album_artist,
    content = '', contentless_delete = 1, tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER artists_search_insert AFTER INSERT ON artists BEGIN
    INSERT INTO artists_search (rowid, name) VALUES (new.id, new.name);
END;

CREATE TRIGGER artists_search_update AFTER UPDATE OF name ON artists
WHEN new.name IS NOT old.name BEGIN
    DELETE FROM artists_search WHERE rowid = old.id;
    INSERT INTO artists_search (rowid, name) VALUES (new.id, new.name);
END;

CREATE TRIGGER artists_search_delete AFTER DELETE ON artists BEGIN
    DELETE FROM artists_search WHERE rowid = old.id;
END;

CREATE TRIGGER albums_search_insert AFTER INSERT ON albums BEGIN
    INSERT INTO albums_search (rowid, title, artist)
    VALUES (new.id, new.title, (SELECT name FROM artists WHERE id = new.artist_id));
END;

CREATE TRIGGER albums_search_update AFTER UPDATE OF title, artist_id ON albums
WHEN new.title IS NOT old.title OR new.artist_id IS NOT old.artist_id BEGIN
    DELETE FROM albums_search WHERE rowid = old.id;
    INSERT INTO albums_search (rowid, title, artist)
    VALUES (new.id, new.title, (SELECT name FROM artists WHERE id = new.artist_id));
END;

CREATE TRIGGER albums_search_delete AFTER DELETE ON albums BEGIN
    DELETE FROM albums_search WHERE rowid = old.id;
END;

-- The file name is what follows the last '/' of the relative path: rtrim
-- strips everything but '/' from the end, leaving the folder part.
CREATE TRIGGER tracks_search_insert AFTER INSERT ON tracks BEGIN
    INSERT INTO tracks_search (rowid, title, artist, album, album_artist)
    VALUES (
        new.id,
        IFNULL(new.title, substr(new.relative_path,
            length(rtrim(new.relative_path, replace(new.relative_path, '/', ''))) + 1)),
        (SELECT name FROM artists WHERE id = new.artist_id),
        (SELECT title FROM albums WHERE id = new.album_id),
        (SELECT name FROM artists WHERE id = new.album_artist_id)
    );
END;

CREATE TRIGGER tracks_search_update
AFTER UPDATE OF title, relative_path, artist_id, album_id, album_artist_id ON tracks
WHEN new.title IS NOT old.title OR new.relative_path IS NOT old.relative_path
    OR new.artist_id IS NOT old.artist_id OR new.album_id IS NOT old.album_id
    OR new.album_artist_id IS NOT old.album_artist_id BEGIN
    DELETE FROM tracks_search WHERE rowid = old.id;
    INSERT INTO tracks_search (rowid, title, artist, album, album_artist)
    VALUES (
        new.id,
        IFNULL(new.title, substr(new.relative_path,
            length(rtrim(new.relative_path, replace(new.relative_path, '/', ''))) + 1)),
        (SELECT name FROM artists WHERE id = new.artist_id),
        (SELECT title FROM albums WHERE id = new.album_id),
        (SELECT name FROM artists WHERE id = new.album_artist_id)
    );
END;

CREATE TRIGGER tracks_search_delete AFTER DELETE ON tracks BEGIN
    DELETE FROM tracks_search WHERE rowid = old.id;
END;

-- Index what is already there.
INSERT INTO artists_search (rowid, name) SELECT id, name FROM artists;

INSERT INTO albums_search (rowid, title, artist)
SELECT al.id, al.title, ar.name FROM albums al LEFT JOIN artists ar ON ar.id = al.artist_id;

INSERT INTO tracks_search (rowid, title, artist, album, album_artist)
SELECT t.id,
       IFNULL(t.title, substr(t.relative_path,
           length(rtrim(t.relative_path, replace(t.relative_path, '/', ''))) + 1)),
       artist.name, album.title, album_artist.name
FROM tracks t
LEFT JOIN artists artist ON artist.id = t.artist_id
LEFT JOIN albums album ON album.id = t.album_id
LEFT JOIN artists album_artist ON album_artist.id = t.album_artist_id;
