-- Substring search (PLAN.md F12): the word indexes of 002 match a word
-- only from its start ("beat" finds "Beatles" but "tles" finds nothing).
-- These trigram indexes find any run of three or more characters, folding
-- case and accents as the word indexes do. They are contentless like them,
-- with one column per field so a search can name the field ("artist:").
-- Triggers keep them in step, alongside the word indexes' own.

CREATE VIRTUAL TABLE artists_trigram USING fts5 (
    name,
    content = '', contentless_delete = 1, tokenize = 'trigram remove_diacritics 1'
);

CREATE VIRTUAL TABLE albums_trigram USING fts5 (
    title, artist,
    content = '', contentless_delete = 1, tokenize = 'trigram remove_diacritics 1'
);

CREATE VIRTUAL TABLE tracks_trigram USING fts5 (
    title, artist, album, album_artist, work, composer,
    content = '', contentless_delete = 1, tokenize = 'trigram remove_diacritics 1'
);

CREATE TRIGGER artists_trigram_insert AFTER INSERT ON artists BEGIN
    INSERT INTO artists_trigram (rowid, name) VALUES (new.id, new.name);
END;

CREATE TRIGGER artists_trigram_update AFTER UPDATE OF name ON artists
WHEN new.name IS NOT old.name BEGIN
    DELETE FROM artists_trigram WHERE rowid = old.id;
    INSERT INTO artists_trigram (rowid, name) VALUES (new.id, new.name);
END;

CREATE TRIGGER artists_trigram_delete AFTER DELETE ON artists BEGIN
    DELETE FROM artists_trigram WHERE rowid = old.id;
END;

CREATE TRIGGER albums_trigram_insert AFTER INSERT ON albums BEGIN
    INSERT INTO albums_trigram (rowid, title, artist)
    VALUES (new.id, new.title, (SELECT name FROM artists WHERE id = new.artist_id));
END;

CREATE TRIGGER albums_trigram_update AFTER UPDATE OF title, artist_id ON albums
WHEN new.title IS NOT old.title OR new.artist_id IS NOT old.artist_id BEGIN
    DELETE FROM albums_trigram WHERE rowid = old.id;
    INSERT INTO albums_trigram (rowid, title, artist)
    VALUES (new.id, new.title, (SELECT name FROM artists WHERE id = new.artist_id));
END;

CREATE TRIGGER albums_trigram_delete AFTER DELETE ON albums BEGIN
    DELETE FROM albums_trigram WHERE rowid = old.id;
END;

CREATE TRIGGER tracks_trigram_insert AFTER INSERT ON tracks BEGIN
    INSERT INTO tracks_trigram (rowid, title, artist, album, album_artist, work, composer)
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

CREATE TRIGGER tracks_trigram_update
AFTER UPDATE OF title, relative_path, artist_id, artist_credit, album_id, album_artist_id, work,
    composer_id
ON tracks
WHEN new.title IS NOT old.title OR new.relative_path IS NOT old.relative_path
    OR new.artist_id IS NOT old.artist_id OR new.artist_credit IS NOT old.artist_credit
    OR new.album_id IS NOT old.album_id OR new.album_artist_id IS NOT old.album_artist_id
    OR new.work IS NOT old.work OR new.composer_id IS NOT old.composer_id BEGIN
    DELETE FROM tracks_trigram WHERE rowid = old.id;
    INSERT INTO tracks_trigram (rowid, title, artist, album, album_artist, work, composer)
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

CREATE TRIGGER tracks_trigram_delete AFTER DELETE ON tracks BEGIN
    DELETE FROM tracks_trigram WHERE rowid = old.id;
END;

-- Index what is already there.
INSERT INTO artists_trigram (rowid, name) SELECT id, name FROM artists;

INSERT INTO albums_trigram (rowid, title, artist)
SELECT al.id, al.title, ar.name FROM albums al LEFT JOIN artists ar ON ar.id = al.artist_id;

INSERT INTO tracks_trigram (rowid, title, artist, album, album_artist, work, composer)
SELECT t.id,
       IFNULL(t.title, substr(t.relative_path,
           length(rtrim(t.relative_path, replace(t.relative_path, '/', ''))) + 1)),
       IFNULL(t.artist_credit, artist.name), album.title, album_artist.name, t.work, composer.name
FROM tracks t
LEFT JOIN artists artist ON artist.id = t.artist_id
LEFT JOIN albums album ON album.id = t.album_id
LEFT JOIN artists album_artist ON album_artist.id = t.album_artist_id
LEFT JOIN artists composer ON composer.id = t.composer_id;
