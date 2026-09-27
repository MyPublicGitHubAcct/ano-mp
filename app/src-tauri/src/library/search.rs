//! Searching the library by title, artist, album artist and album (and a
//! work's composer), with results grouped into artists, albums and tracks.
//!
//! Two kinds of index back it, both FTS5 tables kept up to date by
//! triggers, and both folding case and accents like browse sorting does, so
//! "elodie" finds "Élodie": the word indexes (migration 002), where a word
//! typed matches the start of a word in a field, and the trigram indexes
//! (009, PLAN.md F12), where a word of three or more characters matches
//! anywhere in one ("tles" finds "Beatles"). Every word typed must match;
//! matches from the start of words rank first.
//!
//! Field filters narrow a search: `artist:`, `album:`, `title:`,
//! `composer:` (matched like words, in that field alone), `genre:` (one of
//! the track's genres, or part of one) and `year:1994` or
//! `year:1990-1999`. A value with spaces is quoted: `artist:"pink floyd"`.

use rusqlite::types::Value;
use rusqlite::{Connection, Row};
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

/// A field a search term can be limited to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Artist,
    Album,
    Title,
    Composer,
}

/// What the user typed, taken apart.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Query {
    /// Words (or quoted phrases) to match in any field, or in one.
    pub terms: Vec<(Option<Field>, String)>,
    pub genres: Vec<String>,
    /// Inclusive year ranges.
    pub years: Vec<(u32, u32)>,
}

impl Query {
    fn is_empty(&self) -> bool {
        self.terms.is_empty() && self.genres.is_empty() && self.years.is_empty()
    }

    /// Whether anything limits the tracks besides artists' names.
    fn has_track_filters(&self) -> bool {
        !self.genres.is_empty()
            || !self.years.is_empty()
            || self.terms.iter().any(|(field, _)| {
                matches!(field, Some(Field::Album | Field::Title | Field::Composer))
            })
    }
}

/// Splits what the user typed into terms and field filters. Words are
/// separated by spaces; a quoted run is one term; `field:value` is a filter
/// for the fields above (any other colon is plain text). Terms without a
/// letter or digit, and unreadable years, are left out.
pub fn parse(text: &str) -> Query {
    let mut query = Query::default();
    for token in tokens(text) {
        let (field, value): (Option<Field>, &str) = match token.split_once(':') {
            Some((key, value)) if !token.starts_with('"') => {
                match key.to_ascii_lowercase().as_str() {
                    "artist" => (Some(Field::Artist), value),
                    "album" => (Some(Field::Album), value),
                    "title" => (Some(Field::Title), value),
                    "composer" => (Some(Field::Composer), value),
                    "genre" => {
                        let genre = unquote(value);
                        if genre.chars().any(char::is_alphanumeric) {
                            query.genres.push(genre);
                        }
                        continue;
                    }
                    "year" => {
                        if let Some(range) = year_range(&unquote(value)) {
                            query.years.push(range);
                        }
                        continue;
                    }
                    _ => (None, token.as_str()),
                }
            }
            _ => (None, token.as_str()),
        };
        let value = unquote(value);
        if value.chars().any(char::is_alphanumeric) {
            query.terms.push((field, value));
        }
    }
    query
}

/// Whitespace-separated tokens, keeping quoted runs (and `key:"a b"`) whole.
fn tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for c in text.chars() {
        if c == '"' {
            quoted = !quoted;
            current.push(c);
        } else if c.is_whitespace() && !quoted {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn unquote(text: &str) -> String {
    text.trim_matches('"').trim().to_owned()
}

/// "1994", "1990-1999" or "1990..1999" as an inclusive range.
fn year_range(text: &str) -> Option<(u32, u32)> {
    let year = |text: &str| -> Option<u32> {
        let text = text.trim();
        (text.len() == 4 && text.chars().all(|c| c.is_ascii_digit()))
            .then(|| text.parse().ok())
            .flatten()
    };
    let (from, to) = match text.split_once("..").or_else(|| text.split_once('-')) {
        Some((from, to)) => (year(from)?, year(to)?),
        None => {
            let only = year(text)?;
            (only, only)
        }
    };
    Some((from.min(to), from.max(to)))
}

/// The word-index query for `term` (as a prefix), in `columns` if any.
fn prefix_expression(columns: Option<&str>, term: &str) -> String {
    let phrase = format!("\"{}\"*", term.replace('"', "\"\""));
    match columns {
        Some(columns) => format!("{{{columns}}} : {phrase}"),
        None => phrase,
    }
}

/// The trigram-index query for `term` (anywhere), if it is long enough.
fn substring_expression(columns: Option<&str>, term: &str) -> Option<String> {
    (term.chars().count() >= 3).then(|| {
        let phrase = format!("\"{}\"", term.replace('"', "\"\""));
        match columns {
            Some(columns) => format!("{{{columns}}} : {phrase}"),
            None => phrase,
        }
    })
}

/// The FTS5 query for what the user typed as plain words: each becomes a
/// quoted phrase matching as a prefix, so FTS5 syntax typed in ("AND", '*',
/// '(') is plain text and a word like "AC/DC" (two tokens to FTS5) matches
/// as a phrase. `None` if nothing searchable was typed. Ranks results.
#[cfg(test)]
pub fn match_expression(query: &str) -> Option<String> {
    let phrases: Vec<String> = query
        .split_whitespace()
        .filter(|word| word.chars().any(char::is_alphanumeric))
        .map(|word| prefix_expression(None, word))
        .collect();
    (!phrases.is_empty()).then(|| phrases.join(" "))
}

/// A statement's bound values; `?1` is always the ignored articles.
struct Params(Vec<Value>);

impl Params {
    fn bind(&mut self, value: impl Into<Value>) -> String {
        self.0.push(value.into());
        format!("?{}", self.0.len())
    }
}

/// The conditions a term puts on rows `id` of an index pair (`words`, and
/// `trigrams` with the same columns): a word match, or else a substring.
fn term_condition(
    params: &mut Params,
    id: &str,
    words: &str,
    trigrams: &str,
    columns: Option<&str>,
    term: &str,
) -> String {
    let prefix = params.bind(prefix_expression(columns, term));
    let mut condition =
        format!("({id} IN (SELECT rowid FROM {words} WHERE {words} MATCH {prefix})");
    if let Some(substring) = substring_expression(columns, term) {
        let substring = params.bind(substring);
        condition.push_str(&format!(
            " OR {id} IN (SELECT rowid FROM {trigrams} WHERE {trigrams} MATCH {substring})"
        ));
    }
    condition.push(')');
    condition
}

/// Conditions on track `t` for `query`'s terms and filters; with
/// `artists_too` false, terms limited to artists are left to the caller.
fn track_conditions(params: &mut Params, query: &Query, artist_terms: bool) -> Vec<String> {
    let mut conditions = Vec::new();
    for (field, term) in &query.terms {
        let columns = match field {
            None => None,
            Some(Field::Artist) if !artist_terms => continue,
            Some(Field::Artist) => Some("artist album_artist"),
            Some(Field::Album) => Some("album"),
            Some(Field::Title) => Some("title"),
            Some(Field::Composer) => Some("composer"),
        };
        if field.is_none() && !artist_terms {
            continue;
        }
        conditions.push(term_condition(
            params,
            "t.id",
            "tracks_search",
            "tracks_trigram",
            columns,
            term,
        ));
    }
    conditions.extend(genre_and_year(params, query, "t"));
    conditions
}

/// Conditions on track `alias` for `query`'s genres and years.
fn genre_and_year(params: &mut Params, query: &Query, alias: &str) -> Vec<String> {
    let mut conditions = Vec::new();
    for genre in &query.genres {
        let exact = params.bind(genre.clone());
        let like = params.bind(format!(
            "%{}%",
            genre
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        ));
        conditions.push(format!(
            "(anomp_has_genre({alias}.genre, {exact}) OR {alias}.genre LIKE {like} ESCAPE '\\')"
        ));
    }
    for &(from, to) in &query.years {
        let (from, to) = (params.bind(i64::from(from)), params.bind(i64::from(to)));
        conditions.push(format!("IFNULL({alias}.year, 0) BETWEEN {from} AND {to}"));
    }
    conditions
}

fn and(conditions: &[String]) -> String {
    if conditions.is_empty() {
        "1".into()
    } else {
        conditions.join(" AND ")
    }
}

/// Up to `limit` (at most `MAX_PAGE_SIZE`) results of each of `kinds` from
/// `offset`, best matches first, with the total of each. A query with no
/// terms or filters finds nothing.
pub fn search(
    conn: &Connection,
    query: &str,
    kinds: &[SearchKind],
    offset: u32,
    limit: u32,
) -> Result<SearchResults, Error> {
    let mut results = SearchResults::default();
    let parsed = parse(query);
    if parsed.is_empty() {
        return Ok(results);
    }
    let limit = limit.min(MAX_PAGE_SIZE);
    let articles = rules::sort_settings(conn)?.ignored_articles.join("\n");
    // Ranks: how well the terms that aren't limited to a field match from
    // the start of words.
    let plain: Vec<&str> = parsed
        .terms
        .iter()
        .filter(|(field, _)| field.is_none())
        .map(|(_, term)| term.as_str())
        .collect();
    let rank_expression = (!plain.is_empty()).then(|| {
        plain
            .iter()
            .map(|term| prefix_expression(None, term))
            .collect::<Vec<_>>()
            .join(" ")
    });

    if kinds.contains(&SearchKind::Artists) {
        let mut params = Params(vec![Value::Text(articles.clone())]);
        let mut conditions = Vec::new();
        for (field, term) in &parsed.terms {
            if matches!(field, None | Some(Field::Artist)) {
                conditions.push(term_condition(
                    &mut params,
                    "a.id",
                    "artists_search",
                    "artists_trigram",
                    None,
                    term,
                ));
            }
        }
        if parsed.has_track_filters() {
            // Artists with a track (or an album) the other filters match.
            let tracks = and(&track_conditions(&mut params, &parsed, false));
            conditions.push(format!(
                "(a.id IN (SELECT ta.artist_id FROM track_artists ta JOIN tracks t
                           ON t.id = ta.track_id WHERE {tracks})
                  OR a.id IN (SELECT t.album_artist_id FROM tracks t WHERE {tracks}))"
            ));
        }
        let filter = and(&conditions);
        let (rank_with, rank_join, rank_order) =
            rank(&mut params, "artists_search", "a.id", &rank_expression);
        results.artist_total = count(
            conn,
            &format!("SELECT count(*) FROM artists a WHERE {filter}"),
            &params,
        )?;
        let (limit, offset) = page(&mut params, limit, offset);
        results.artists = fetch(
            conn,
            &format!(
                "{rank_with}
                 SELECT a.id, a.name,
                        (SELECT count(*) FROM tracks WHERE album_artist_id = a.id),
                        (SELECT count(*) FROM tracks WHERE album_artist_id = a.id)
                      + (SELECT count(*) FROM tracks t
                         WHERE t.id IN (SELECT track_id FROM track_artists WHERE artist_id = a.id)
                           AND t.album_artist_id IS NOT a.id)
                 FROM artists a {rank_join}
                 WHERE {filter}
                 ORDER BY {rank_order} anomp_sort_key(a.name, ?1), a.name, a.id
                 LIMIT {limit} OFFSET {offset}"
            ),
            &params,
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
        let mut params = Params(vec![Value::Text(articles.clone())]);
        let mut conditions = Vec::new();
        for (field, term) in &parsed.terms {
            let columns = match field {
                None => None,
                Some(Field::Artist) => Some("artist"),
                Some(Field::Album | Field::Title) => Some("title"),
                Some(Field::Composer) => {
                    let tracks = term_condition(
                        &mut params,
                        "t.id",
                        "tracks_search",
                        "tracks_trigram",
                        Some("composer"),
                        term,
                    );
                    conditions.push(format!(
                        "al.id IN (SELECT t.album_id FROM tracks t WHERE {tracks})"
                    ));
                    continue;
                }
            };
            conditions.push(term_condition(
                &mut params,
                "al.id",
                "albums_search",
                "albums_trigram",
                columns,
                term,
            ));
        }
        let genres = Query {
            terms: Vec::new(),
            genres: parsed.genres.clone(),
            years: Vec::new(),
        };
        for genre in genre_and_year(&mut params, &genres, "y") {
            conditions.push(format!(
                "EXISTS (SELECT 1 FROM tracks y WHERE y.album_id = al.id AND {genre})"
            ));
        }
        for &(from, to) in &parsed.years {
            let (from, to) = (params.bind(i64::from(from)), params.bind(i64::from(to)));
            conditions.push(format!(
                "IFNULL((SELECT min(y.year) FROM tracks y WHERE y.album_id = al.id), 0)
                 BETWEEN {from} AND {to}"
            ));
        }
        let filter = and(&conditions);
        let (rank_with, rank_join, rank_order) =
            rank(&mut params, "albums_search", "al.id", &rank_expression);
        results.album_total = count(
            conn,
            &format!("SELECT count(*) FROM albums al WHERE {filter}"),
            &params,
        )?;
        let (limit, offset) = page(&mut params, limit, offset);
        results.albums = fetch(
            conn,
            &format!(
                "{rank_with}
                 SELECT al.id, al.title, ar.name, al.artist_id,
                        (SELECT min(year) FROM tracks WHERE album_id = al.id),
                        (SELECT count(*) FROM tracks WHERE album_id = al.id)
                 FROM albums al LEFT JOIN artists ar ON ar.id = al.artist_id {rank_join}
                 WHERE {filter}
                 ORDER BY {rank_order} anomp_sort_key(al.title, ?1), al.title, al.id
                 LIMIT {limit} OFFSET {offset}"
            ),
            &params,
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
        let mut params = Params(vec![Value::Text(articles)]);
        let filter = and(&track_conditions(&mut params, &parsed, true));
        let (rank_with, rank_join, rank_order) =
            rank(&mut params, "tracks_search", "t.id", &rank_expression);
        results.track_total = count(
            conn,
            &format!("SELECT count(*) FROM tracks t WHERE {filter}"),
            &params,
        )?;
        let (limit, offset) = page(&mut params, limit, offset);
        results.tracks = fetch(
            conn,
            &format!(
                "{rank_with}
                 SELECT {TRACK_COLUMNS}
                 FROM tracks t
                 JOIN folders f ON f.id = t.folder_id
                 LEFT JOIN artists artist ON artist.id = t.artist_id
                 LEFT JOIN albums album ON album.id = t.album_id
                 LEFT JOIN artists album_artist ON album_artist.id = t.album_artist_id
                 {rank_join}
                 WHERE {filter}
                 ORDER BY {rank_order} anomp_sort_key(t.title, ?1), t.title, t.id
                 LIMIT {limit} OFFSET {offset}"
            ),
            &params,
            track_from_row,
        )?;
    }
    Ok(results)
}

/// A WITH clause and a join giving rows of `id` the word index's rank for
/// `expression`, and the ORDER BY terms that put word matches first, best
/// first. The ranked matches are materialized, so SQLite indexes them for
/// the join rather than running the match again for every row.
fn rank(
    params: &mut Params,
    index: &str,
    id: &str,
    expression: &Option<String>,
) -> (String, String, &'static str) {
    match expression {
        Some(expression) => {
            let expression = params.bind(expression.clone());
            (
                format!(
                    "WITH ranked(id, rank) AS MATERIALIZED
                       (SELECT rowid, rank FROM {index} WHERE {index} MATCH {expression})"
                ),
                format!("LEFT JOIN ranked ON ranked.id = {id}"),
                "ranked.rank IS NULL, ranked.rank,",
            )
        }
        None => (String::new(), String::new(), ""),
    }
}

fn page(params: &mut Params, limit: u32, offset: u32) -> (String, String) {
    (
        params.bind(i64::from(limit)),
        params.bind(i64::from(offset)),
    )
}

fn count(conn: &Connection, sql: &str, params: &Params) -> Result<u32, Error> {
    Ok(fetch(conn, sql, params, |row| row.get(0))?
        .into_iter()
        .next()
        .unwrap_or(0))
}

fn fetch<T>(
    conn: &Connection,
    sql: &str,
    params: &Params,
    mut map: impl FnMut(&Row) -> rusqlite::Result<T>,
) -> Result<Vec<T>, Error> {
    let mut statement = conn.prepare(sql)?;
    // By index: a count may not use every parameter (e.g. `?1`).
    for index in 1..=statement.parameter_count() {
        if let Some(value) = params.0.get(index - 1) {
            statement.raw_bind_parameter(index, value)?;
        }
    }
    let mut rows = statement.raw_query();
    let mut result = Vec::new();
    while let Some(row) = rows.next()? {
        result.push(map(row)?);
    }
    Ok(result)
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
    fn matches_substrings_of_three_characters_or_more() {
        let library = sample();
        let tles = find(&library, "tles");
        assert_eq!(tles.artists[0].name, "The Beatles");
        let mut found = titles(&tles);
        found.sort();
        assert_eq!(found, ["Come Together", "Something"]);
        // Shorter, only from the start of a word.
        assert!(find(&library, "le").tracks.is_empty());
        assert_eq!(titles(&find(&library, "ac/dc")), ["Thunderstruck"]);
        assert_eq!(titles(&find(&library, "dc")), ["Thunderstruck"]);
        // Across accents, inside a word.
        assert_eq!(titles(&find(&library, "lodi")), ["Guest Spot"]);
    }

    #[test]
    fn word_starts_rank_before_substrings() {
        let library = Library::new([
            track("1.flac").title("Clean").artist("X"),
            track("2.flac").title("Lean On").artist("X"),
        ]);
        assert_eq!(titles(&find(&library, "lean")), ["Lean On", "Clean"]);
    }

    #[test]
    fn field_filters_narrow_the_search() {
        let library = sample();
        assert_eq!(
            titles(&find(&library, "artist:jackson")),
            ["Beat It", "Guest Spot"],
            "the artist or the album artist"
        );
        assert_eq!(titles(&find(&library, "album:road some")), ["Something"]);
        assert!(find(&library, "title:beatles").tracks.is_empty());
        assert_eq!(
            titles(&find(&library, "title:\"come together\"")),
            ["Come Together"]
        );
        let by_year = find(&library, "year:1969");
        assert_eq!(by_year.track_total, 2);
        assert_eq!(by_year.albums[0].title, "Abbey Road");
        assert_eq!(by_year.artists[0].name, "The Beatles");
        assert_eq!(find(&library, "year:1960-1970").track_total, 2);
        assert_eq!(find(&library, "year:1970..1960").track_total, 2);
        assert_eq!(find(&library, "year:nineteen").track_total, 0);
        let artists = find(&library, "artist:\"michael jackson\"");
        assert_eq!(artists.artists.len(), 1);
        assert_eq!(artists.albums[0].title, "Thriller");
    }

    #[test]
    fn genres_match_whole_or_in_part() {
        let library = Library::new([
            track("1.flac")
                .title("One")
                .artist("X")
                .album("A")
                .genre("Hard Rock; Blues"),
            track("2.flac")
                .title("Two")
                .artist("Y")
                .album("B")
                .genre("Pop"),
        ]);
        assert_eq!(titles(&find(&library, "genre:blues")), ["One"]);
        assert_eq!(titles(&find(&library, "genre:rock")), ["One"]);
        assert_eq!(titles(&find(&library, "genre:\"hard rock\"")), ["One"]);
        let pop = find(&library, "genre:pop");
        assert_eq!(pop.albums.len(), 1);
        assert_eq!(pop.artists[0].name, "Y");
        assert_eq!(find(&library, "genre:100%").track_total, 0);
    }

    #[test]
    fn parses_terms_and_filters() {
        assert_eq!(
            parse("Artist:\"pink floyd\" wall year:1979 genre:rock foo:bar \"a b\" -"),
            Query {
                terms: vec![
                    (Some(Field::Artist), "pink floyd".into()),
                    (None, "wall".into()),
                    (None, "foo:bar".into()),
                    (None, "a b".into()),
                ],
                genres: vec!["rock".into()],
                years: vec![(1979, 1979)],
            }
        );
        assert_eq!(parse("year:1990-1999").years, [(1990, 1999)]);
        assert!(parse("year:199").years.is_empty());
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
