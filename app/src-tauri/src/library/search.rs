//! Searching the library by title, artist, album artist and album, with
//! results grouped into artists, albums and tracks.
//!
//! The indexes are FTS5 tables kept up to date by triggers
//! (`migrations/002_search.sql`). Their tokenizer folds case and removes
//! accents, like browse sorting does, so "elodie" finds "Élodie". Each word
//! typed matches the start of a word in any indexed field; all must match.

use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

use super::browse::MAX_PAGE_SIZE;
use super::{rules, track_from_row, Error, TrackSummary, TRACK_COLUMNS};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SearchKind {
    Artists,
    Albums,
    Tracks,
}

pub const ALL_KINDS: [SearchKind; 3] =
    [SearchKind::Artists, SearchKind::Albums, SearchKind::Tracks];

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistHit {
    pub id: i64,
    pub name: String,
    /// Tracks the artist is the album artist of; 0 for an artist found only
    /// on other artists' albums.
    pub album_artist_track_count: u32,
    /// Tracks by the artist or on their albums.
    pub track_count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumHit {
    pub id: i64,
    pub title: String,
    pub album_artist: Option<String>,
    pub album_artist_id: Option<i64>,
    /// The earliest year among its tracks.
    pub year: Option<u32>,
    pub track_count: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResults {
    pub artists: Vec<ArtistHit>,
    pub artist_total: u32,
    pub albums: Vec<AlbumHit>,
    pub album_total: u32,
    pub tracks: Vec<TrackSummary>,
    pub track_total: u32,
}

/// Up to `limit` (at most `MAX_PAGE_SIZE`) results of each of `kinds` from
/// `offset`, best matches first, with the total of each. A query with no
/// words finds nothing.
pub fn search(
    conn: &Connection,
    query: &str,
    kinds: &[SearchKind],
    offset: u32,
    limit: u32,
) -> Result<SearchResults, Error> {
    let mut results = SearchResults::default();
    let Some(expression) = match_expression(query) else {
        return Ok(results);
    };
    let limit = limit.min(MAX_PAGE_SIZE);
    let articles = rules::sort_settings(conn)?.ignored_articles.join("\n");
    let page = params![articles, expression, limit, offset];

    if kinds.contains(&SearchKind::Artists) {
        results.artist_total = count(conn, "artists_search", &expression)?;
        results.artists = fetch(
            conn,
            "SELECT a.id, a.name,
                    (SELECT count(*) FROM tracks WHERE album_artist_id = a.id),
                    (SELECT count(*) FROM tracks WHERE album_artist_id = a.id)
                  + (SELECT count(*) FROM tracks
                     WHERE artist_id = a.id AND album_artist_id IS NOT a.id)
             FROM artists_search JOIN artists a ON a.id = artists_search.rowid
             WHERE artists_search MATCH ?2
             ORDER BY artists_search.rank, anomp_sort_key(a.name, ?1), a.name, a.id
             LIMIT ?3 OFFSET ?4",
            page,
            |row| {
                Ok(ArtistHit {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    album_artist_track_count: row.get(2)?,
                    track_count: row.get(3)?,
                })
            },
        )?;
    }
    if kinds.contains(&SearchKind::Albums) {
        results.album_total = count(conn, "albums_search", &expression)?;
        results.albums = fetch(
            conn,
            "SELECT al.id, al.title, ar.name, al.artist_id,
                    (SELECT min(year) FROM tracks WHERE album_id = al.id),
                    (SELECT count(*) FROM tracks WHERE album_id = al.id)
             FROM albums_search JOIN albums al ON al.id = albums_search.rowid
             LEFT JOIN artists ar ON ar.id = al.artist_id
             WHERE albums_search MATCH ?2
             ORDER BY albums_search.rank, anomp_sort_key(al.title, ?1), al.title, al.id
             LIMIT ?3 OFFSET ?4",
            page,
            |row| {
                Ok(AlbumHit {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    album_artist: row.get(2)?,
                    album_artist_id: row.get(3)?,
                    year: row.get(4)?,
                    track_count: row.get(5)?,
                })
            },
        )?;
    }
    if kinds.contains(&SearchKind::Tracks) {
        results.track_total = count(conn, "tracks_search", &expression)?;
        results.tracks = fetch(
            conn,
            &format!(
                "SELECT {TRACK_COLUMNS}
                 FROM tracks_search
                 JOIN tracks t ON t.id = tracks_search.rowid
                 JOIN folders f ON f.id = t.folder_id
                 LEFT JOIN artists artist ON artist.id = t.artist_id
                 LEFT JOIN albums album ON album.id = t.album_id
                 LEFT JOIN artists album_artist ON album_artist.id = t.album_artist_id
                 WHERE tracks_search MATCH ?2
                 ORDER BY tracks_search.rank, anomp_sort_key(t.title, ?1), t.title, t.id
                 LIMIT ?3 OFFSET ?4"
            ),
            page,
            track_from_row,
        )?;
    }
    Ok(results)
}

/// The FTS5 query for what the user typed: each whitespace-separated word
/// becomes a quoted phrase matching as a prefix, so FTS5 syntax typed in
/// ("AND", '*', '(') is plain text and a word like "AC/DC" (two tokens to
/// FTS5) matches as a phrase. `None` if nothing searchable was typed.
pub fn match_expression(query: &str) -> Option<String> {
    let phrases: Vec<String> = query
        .split_whitespace()
        .filter(|word| word.chars().any(char::is_alphanumeric))
        .map(|word| format!("\"{}\"*", word.replace('"', "\"\"")))
        .collect();
    (!phrases.is_empty()).then(|| phrases.join(" "))
}

fn count(conn: &Connection, table: &str, expression: &str) -> Result<u32, Error> {
    // `table` is one of the fixed index names above, never user input.
    let sql = format!("SELECT count(*) FROM {table} WHERE {table} MATCH ?1");
    Ok(conn
        .prepare_cached(&sql)?
        .query_row([expression], |row| row.get(0))?)
}

fn fetch<T>(
    conn: &Connection,
    sql: &str,
    params: &[&dyn rusqlite::ToSql],
    map: impl FnMut(&Row) -> rusqlite::Result<T>,
) -> Result<Vec<T>, Error> {
    let mut statement = conn.prepare_cached(sql)?;
    let rows = statement.query_map(params, map)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::db;
    use crate::library::test_library::{track, Library};

    fn titles(results: &SearchResults) -> Vec<&str> {
        results
            .tracks
            .iter()
            .map(|track| track.title.as_deref().unwrap_or(&track.path))
            .collect()
    }

    fn find(library: &Library, query: &str) -> SearchResults {
        search(&library.conn, query, &ALL_KINDS, 0, 100).unwrap()
    }

    fn sample() -> Library {
        Library::new([
            track("b/1.flac")
                .title("Come Together")
                .artist("The Beatles")
                .album("Abbey Road")
                .year(1969),
            track("b/2.flac")
                .title("Something")
                .artist("The Beatles")
                .album("Abbey Road")
                .year(1969),
            track("e/1.flac")
                .title("Pour que tu m'aimes encore")
                .artist("Céline Dion")
                .album("D'eux"),
            track("c/1.flac")
                .title("Beat It")
                .artist("Michael Jackson")
                .album("Thriller"),
            track("g/1.flac")
                .title("Guest Spot")
                .artist("Élodie")
                .album_artist("Michael Jackson")
                .album("Thriller"),
            track("acdc/1.flac")
                .title("Thunderstruck")
                .artist("AC/DC")
                .album("The Razors Edge"),
            track("untitled/Demo Take 3.flac").artist("Nobody"),
        ])
    }

    #[test]
    fn groups_results_into_artists_albums_and_tracks() {
        let library = sample();
        let results = find(&library, "beat");
        let artists: Vec<_> = results.artists.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(artists, ["The Beatles"]);
        assert_eq!(results.artists[0].track_count, 2);
        assert_eq!(results.artists[0].album_artist_track_count, 2);
        let albums: Vec<_> = results.albums.iter().map(|a| a.title.as_str()).collect();
        assert_eq!(albums, ["Abbey Road"]);
        assert_eq!(
            results.albums[0].album_artist.as_deref(),
            Some("The Beatles")
        );
        assert_eq!(results.albums[0].year, Some(1969));
        assert_eq!(results.albums[0].track_count, 2);
        // "Beat It" by title, the Beatles' tracks by artist.
        let mut found = titles(&results);
        found.sort();
        assert_eq!(found, ["Beat It", "Come Together", "Something"]);
        assert_eq!(
            (
                results.artist_total,
                results.album_total,
                results.track_total
            ),
            (1, 1, 3)
        );
    }

    #[test]
    fn ignores_case_and_accents() {
        let library = sample();
        for query in ["celine", "CÉLINE", "Céline", "celi"] {
            let results = find(&library, query);
            assert_eq!(titles(&results), ["Pour que tu m'aimes encore"], "{query}");
        }
        // A guest artist is found, and counts the track they're on.
        let elodie = find(&library, "elodie");
        assert_eq!(elodie.artists[0].name, "Élodie");
        assert_eq!(elodie.artists[0].album_artist_track_count, 0);
        assert_eq!(elodie.artists[0].track_count, 1);
    }

    #[test]
    fn every_word_must_match_somewhere() {
        let library = sample();
        assert_eq!(titles(&find(&library, "beatles some")), ["Something"]);
        assert_eq!(
            titles(&find(&library, "jackson thriller guest")),
            ["Guest Spot"]
        );
        assert!(find(&library, "beatles thriller").tracks.is_empty());
    }

    #[test]
    fn matches_word_starts_only() {
        let library = sample();
        assert!(find(&library, "tles").tracks.is_empty());
        assert_eq!(titles(&find(&library, "ac/dc")), ["Thunderstruck"]);
        assert_eq!(titles(&find(&library, "dc")), ["Thunderstruck"]);
    }

    #[test]
    fn untitled_tracks_are_found_by_file_name() {
        let library = sample();
        let results = find(&library, "demo take");
        assert_eq!(results.tracks.len(), 1);
        assert!(results.tracks[0].path.ends_with("Demo Take 3.flac"));
    }

    #[test]
    fn query_syntax_is_plain_text() {
        let library = sample();
        for query in [
            "", "   ", "\"", "*", "-", "AND", "beat AND", "NEAR(", "a\"b", "(", "title:x", "^",
        ] {
            search(&library.conn, query, &ALL_KINDS, 0, 10).unwrap();
        }
        assert_eq!(find(&library, "").track_total, 0);
        assert_eq!(match_expression("  a\"b  "), Some("\"a\"\"b\"*".into()));
        assert_eq!(match_expression("- *"), None);
    }

    #[test]
    fn pages_through_each_kind() {
        let library = Library::new((0..25).map(|i| {
            track(&format!("{i}.flac"))
                .title(&format!("Song {i}"))
                .artist("Band")
                .album("Songs")
        }));
        let first = search(&library.conn, "song", &[SearchKind::Tracks], 0, 10).unwrap();
        assert_eq!((first.tracks.len(), first.track_total), (10, 25));
        assert!(first.artists.is_empty() && first.albums.is_empty());
        assert_eq!(first.album_total, 0, "kinds not asked for aren't counted");
        let mut seen: Vec<i64> = first.tracks.iter().map(|t| t.id).collect();
        for offset in [10, 20] {
            let page = search(&library.conn, "song", &[SearchKind::Tracks], offset, 10).unwrap();
            seen.extend(page.tracks.iter().map(|t| t.id));
        }
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), 25);
    }

    #[test]
    fn follows_changes_to_the_library() {
        let mut library = sample();
        let id = find(&library, "something").tracks[0].id;
        library
            .conn
            .execute(
                "UPDATE tracks SET title = 'Here Comes the Sun' WHERE id = ?1",
                [id],
            )
            .unwrap();
        assert!(find(&library, "something").tracks.is_empty());
        assert_eq!(titles(&find(&library, "sun")), ["Here Comes the Sun"]);

        // Removing the folder removes its tracks, and then its albums and
        // artists, from the results.
        crate::library::remove_folder(&mut library.conn, library.folder_id).unwrap();
        let results = find(&library, "beatles");
        assert_eq!(
            (
                results.artist_total,
                results.album_total,
                results.track_total
            ),
            (0, 0, 0)
        );
    }

    #[test]
    fn existing_rows_are_indexed_by_the_migration() {
        let conn = db::open_in_memory_at(1).unwrap();
        conn.execute(
            "INSERT INTO folders (path, added_at) VALUES ('/Music', 0)",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO artists (name) VALUES ('Björk')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO tracks (folder_id, relative_path, file_size, file_mtime_ns, title,
                                 artist_id, duration, sample_rate, channels, scanned_at)
             VALUES (1, 'a.flac', 0, 0, 'Jóga', 1, 60.0, 44100, 2, 0)",
            [],
        )
        .unwrap();
        let mut conn = conn;
        db::migrate(&mut conn).unwrap();
        let results = search(&conn, "bjork joga", &ALL_KINDS, 0, 10).unwrap();
        assert_eq!(results.tracks.len(), 1);
        assert_eq!(
            search(&conn, "bjo", &ALL_KINDS, 0, 10)
                .unwrap()
                .artist_total,
            1
        );
    }
}
