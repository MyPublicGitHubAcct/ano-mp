//! The MusicBrainz web service (https://musicbrainz.org/doc/MusicBrainz_API):
//! release search and lookup, parsed into `Release`. Requests go through
//! `http::Client`, which keeps to MusicBrainz's one request a second, and
//! responses are cached (`cache`).

use std::time::Duration;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use super::http::Client;
use super::Error;

const BASE: &str = "https://musicbrainz.org/ws/2";

/// What a release lookup includes.
const RELEASE_INC: &str = "recordings+artist-credits+labels+release-groups+genres";

/// How long cached responses are used before asking again. Releases change
/// rarely; searches pick up new releases sooner.
const LOOKUP_MAX_AGE: Duration = Duration::from_secs(30 * 86400);
const SEARCH_MAX_AGE: Duration = Duration::from_secs(7 * 86400);

/// Search results asked for.
const SEARCH_LIMIT: u32 = 10;

/// A release (one issue of an album) as the app keeps it: the fields shown
/// or used for matching. Stored as JSON in `album_links.details`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub id: String,
    pub title: String,
    /// The artist credit as printed, e.g. "Simon & Garfunkel".
    pub artist: String,
    pub artist_ids: Vec<String>,
    /// "2007", "2007-12" or "2007-12-26".
    pub date: Option<String>,
    pub country: Option<String>,
    /// "Official", "Promotion", "Bootleg"…
    pub status: Option<String>,
    pub barcode: Option<String>,
    pub labels: Vec<Label>,
    pub release_group_id: Option<String>,
    /// The release group's primary type: "Album", "Single", "EP"…
    pub release_type: Option<String>,
    /// "Compilation", "Live", "Soundtrack"…
    pub secondary_types: Vec<String>,
    /// The release group's first release date: the original year of a
    /// reissue.
    pub first_release_date: Option<String>,
    /// The release's genres, else its release group's, most voted first.
    pub genres: Vec<String>,
    pub track_count: u32,
    /// Whether the Cover Art Archive has a front cover; unknown in search
    /// results.
    pub has_front_art: Option<bool>,
    /// Empty in search results.
    pub tracks: Vec<ReleaseTrack>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    pub name: Option<String>,
    pub catalog_number: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseTrack {
    /// 1-based medium (disc) position.
    pub disc: u32,
    pub position: u32,
    pub title: String,
    pub length_ms: Option<u32>,
    pub recording_id: Option<String>,
}

impl Release {
    /// The year the release came out, from its date.
    pub fn year(&self) -> Option<u32> {
        year_of(self.date.as_deref())
    }

    /// The year the album first came out, from the release group.
    pub fn original_year(&self) -> Option<u32> {
        year_of(self.first_release_date.as_deref())
    }
}

fn year_of(date: Option<&str>) -> Option<u32> {
    date?.get(..4)?.parse().ok()
}

/// A search hit: a release without its tracks, and MusicBrainz's own score
/// (0 to 100) for how well it matched the query.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    pub release: Release,
    pub score: u32,
}

/// Whether `id` looks like an MBID, so a tag value can go into a URL.
pub fn is_mbid(id: &str) -> bool {
    id.len() == 36
        && id.char_indices().all(|(index, c)| match index {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        })
}

pub fn release_url(id: &str) -> String {
    format!("{BASE}/release/{id}?inc={RELEASE_INC}&fmt=json")
}

/// The search for releases called `title` by `artist` (any artist if
/// `None`), with `track_count` tracks as a hint that ranks such releases
/// higher. Fields are matched as terms, not phrases, so small differences
/// in punctuation still find the release.
pub fn search_url(title: &str, artist: Option<&str>, track_count: Option<u32>) -> String {
    let mut query = format!("release:({})", escape_lucene(title));
    if let Some(artist) = artist {
        query += &format!(" AND artist:({})", escape_lucene(artist));
    }
    if let Some(count) = track_count {
        query += &format!(" tracks:{count}");
    }
    format!(
        "{BASE}/release/?query={}&limit={SEARCH_LIMIT}&fmt=json",
        percent_encode(&query)
    )
}

/// `text` with Lucene's special characters escaped, so it's searched for as
/// words.
fn escape_lucene(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        if "+-&|!(){}[]^\"~*?:\\/".contains(c) {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

/// Percent-encodes everything but RFC 3986's unreserved characters.
fn percent_encode(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len() * 3);
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            encoded.push(byte as char);
        } else {
            encoded += &format!("%{byte:02X}");
        }
    }
    encoded
}

pub fn lookup_release(client: &Client, conn: &Connection, id: &str) -> Result<Release, Error> {
    if !is_mbid(id) {
        return Err(Error::Invalid(format!("Not a MusicBrainz id: {id}")));
    }
    let body = client.get_json(conn, &release_url(id), LOOKUP_MAX_AGE)?;
    parse_release(&body)
}

pub fn search_releases(
    client: &Client,
    conn: &Connection,
    title: &str,
    artist: Option<&str>,
    track_count: Option<u32>,
) -> Result<Vec<SearchHit>, Error> {
    let body = client.get_json(
        conn,
        &search_url(title, artist, track_count),
        SEARCH_MAX_AGE,
    )?;
    parse_search(&body)
}

pub fn parse_release(json: &str) -> Result<Release, Error> {
    let raw: RawRelease = serde_json::from_str(json)
        .map_err(|error| Error::Invalid(format!("Unexpected MusicBrainz release: {error}")))?;
    Ok(raw.into_release())
}

pub fn parse_search(json: &str) -> Result<Vec<SearchHit>, Error> {
    #[derive(Deserialize)]
    struct RawSearch {
        #[serde(default)]
        releases: Vec<RawRelease>,
    }
    let raw: RawSearch = serde_json::from_str(json).map_err(|error| {
        Error::Invalid(format!("Unexpected MusicBrainz search result: {error}"))
    })?;
    Ok(raw
        .releases
        .into_iter()
        .map(|release| SearchHit {
            score: release.score.unwrap_or(0),
            release: release.into_release(),
        })
        .collect())
}

// The subset of MusicBrainz's JSON that `Release` is made from. Everything
// is optional or defaulted: what's missing is left out, not an error.

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct RawRelease {
    id: String,
    #[serde(default)]
    title: String,
    score: Option<u32>,
    #[serde(default)]
    artist_credit: Vec<RawCredit>,
    date: Option<String>,
    country: Option<String>,
    status: Option<String>,
    barcode: Option<String>,
    #[serde(default)]
    label_info: Vec<RawLabelInfo>,
    release_group: Option<RawReleaseGroup>,
    #[serde(default)]
    genres: Vec<RawGenre>,
    track_count: Option<u32>,
    cover_art_archive: Option<RawCoverArt>,
    #[serde(default)]
    media: Vec<RawMedium>,
}

#[derive(Deserialize)]
struct RawCredit {
    name: String,
    #[serde(default)]
    joinphrase: String,
    artist: Option<RawArtist>,
}

#[derive(Deserialize)]
struct RawArtist {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct RawLabelInfo {
    catalog_number: Option<String>,
    label: Option<RawLabel>,
}

#[derive(Deserialize)]
struct RawLabel {
    name: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct RawReleaseGroup {
    id: String,
    primary_type: Option<String>,
    #[serde(default)]
    secondary_types: Vec<String>,
    first_release_date: Option<String>,
    #[serde(default)]
    genres: Vec<RawGenre>,
}

#[derive(Deserialize)]
struct RawGenre {
    name: String,
    #[serde(default)]
    count: u32,
}

#[derive(Deserialize)]
struct RawCoverArt {
    #[serde(default)]
    front: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct RawMedium {
    position: Option<u32>,
    track_count: Option<u32>,
    #[serde(default)]
    tracks: Vec<RawTrack>,
}

#[derive(Deserialize)]
struct RawTrack {
    position: Option<u32>,
    #[serde(default)]
    title: String,
    length: Option<u32>,
    recording: Option<RawRecording>,
}

#[derive(Deserialize)]
struct RawRecording {
    id: String,
    length: Option<u32>,
}

/// Genre names, most voted first (ties by name).
fn genre_names(mut genres: Vec<RawGenre>) -> Vec<String> {
    genres.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));
    genres.into_iter().map(|genre| genre.name).collect()
}

fn non_empty(text: Option<String>) -> Option<String> {
    text.filter(|text| !text.trim().is_empty())
}

impl RawRelease {
    fn into_release(self) -> Release {
        let artist = self
            .artist_credit
            .iter()
            .map(|credit| format!("{}{}", credit.name, credit.joinphrase))
            .collect::<String>();
        let artist_ids = self
            .artist_credit
            .iter()
            .filter_map(|credit| credit.artist.as_ref().map(|artist| artist.id.clone()))
            .collect();
        let mut tracks = Vec::new();
        for (index, medium) in self.media.iter().enumerate() {
            let disc = medium.position.unwrap_or(index as u32 + 1);
            for (index, track) in medium.tracks.iter().enumerate() {
                tracks.push(ReleaseTrack {
                    disc,
                    position: track.position.unwrap_or(index as u32 + 1),
                    title: track.title.clone(),
                    length_ms: track
                        .length
                        .or_else(|| track.recording.as_ref().and_then(|r| r.length)),
                    recording_id: track.recording.as_ref().map(|r| r.id.clone()),
                });
            }
        }
        let track_count = self.track_count.unwrap_or_else(|| {
            self.media
                .iter()
                .map(|medium| medium.track_count.unwrap_or(medium.tracks.len() as u32))
                .sum()
        });
        let (release_group_id, release_type, secondary_types, first_release_date, group_genres) =
            match self.release_group {
                Some(group) => (
                    Some(group.id),
                    group.primary_type,
                    group.secondary_types,
                    group.first_release_date,
                    group.genres,
                ),
                None => (None, None, Vec::new(), None, Vec::new()),
            };
        let genres = if self.genres.is_empty() {
            group_genres
        } else {
            self.genres
        };
        Release {
            id: self.id,
            title: self.title,
            artist,
            artist_ids,
            date: non_empty(self.date),
            country: non_empty(self.country),
            status: non_empty(self.status),
            barcode: non_empty(self.barcode),
            labels: self
                .label_info
                .into_iter()
                .map(|info| Label {
                    name: non_empty(info.label.and_then(|label| label.name)),
                    catalog_number: non_empty(info.catalog_number),
                })
                .filter(|label| label.name.is_some() || label.catalog_number.is_some())
                .collect(),
            release_group_id,
            release_type: non_empty(release_type),
            secondary_types,
            first_release_date: non_empty(first_release_date),
            genres: genre_names(genres),
            track_count,
            has_front_art: self.cover_art_archive.map(|art| art.front),
            tracks,
        }
    }
}

#[cfg(test)]
pub mod fixtures {
    //! Recorded MusicBrainz responses (trimmed; MusicBrainz data is CC0)
    //! for "In Rainbows" by Radiohead: a search and four of its releases.

    pub const SEARCH: &str = include_str!("fixtures/musicbrainz/search-in-rainbows.json");

    /// The releases in the search, by id.
    pub const RELEASES: [(&str, &str); 4] = [
        (
            "3b408cb5-7d51-4188-b07c-fabcf308cda3",
            include_str!("fixtures/musicbrainz/release-3b408cb5-7d51-4188-b07c-fabcf308cda3.json"),
        ),
        (
            "1a33443c-3fff-450f-8298-efbc65659d32",
            include_str!("fixtures/musicbrainz/release-1a33443c-3fff-450f-8298-efbc65659d32.json"),
        ),
        (
            "200f0a5a-41c4-49f3-b997-1ef5f975f060",
            include_str!("fixtures/musicbrainz/release-200f0a5a-41c4-49f3-b997-1ef5f975f060.json"),
        ),
        (
            "219e7d7c-806c-44b3-9972-cdb3614b3411",
            include_str!("fixtures/musicbrainz/release-219e7d7c-806c-44b3-9972-cdb3614b3411.json"),
        ),
    ];

    pub const RELEASE_GROUP: &str = "6e335887-60ba-38f0-95af-fae7774336bf";
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::db;
    use crate::metadata::http::testing::fake_client;

    #[test]
    fn recognizes_mbids() {
        assert!(is_mbid("3b408cb5-7d51-4188-b07c-fabcf308cda3"));
        assert!(is_mbid("3B408CB5-7D51-4188-B07C-FABCF308CDA3"));
        for bad in [
            "",
            "3b408cb5-7d51-4188-b07c-fabcf308cda",
            "3b408cb5-7d51-4188-b07c-fabcf308cdaz",
            "3b408cb5_7d51-4188-b07c-fabcf308cda3",
            "../../../../../../../../../../../..",
        ] {
            assert!(!is_mbid(bad), "{bad}");
        }
    }

    #[test]
    fn builds_escaped_search_urls() {
        assert_eq!(
            search_url("In Rainbows", Some("Radiohead"), Some(10)),
            format!(
                "{BASE}/release/?query=release%3A%28In%20Rainbows%29%20AND%20\
                 artist%3A%28Radiohead%29%20tracks%3A10&limit=10&fmt=json"
            )
        );
        // Lucene syntax in a title is searched for as text.
        assert_eq!(
            escape_lucene("AC/DC: (Live!) \"1991\""),
            "AC\\/DC\\: \\(Live\\!\\) \\\"1991\\\""
        );
        assert_eq!(percent_encode("é&?=#"), "%C3%A9%26%3F%3D%23");
        assert!(!search_url("x", None, None).contains("artist"));
    }

    #[test]
    fn parses_a_search() {
        let hits = parse_search(fixtures::SEARCH).unwrap();
        assert_eq!(hits.len(), 5);
        let first = &hits[0];
        assert_eq!(first.score, 100);
        assert_eq!(first.release.id, "3b408cb5-7d51-4188-b07c-fabcf308cda3");
        assert_eq!(first.release.title, "In Rainbows");
        assert_eq!(first.release.artist, "Radiohead");
        assert_eq!(first.release.track_count, 10);
        assert_eq!(first.release.year(), Some(2007));
        assert_eq!(first.release.release_type.as_deref(), Some("Album"));
        assert!(first.release.tracks.is_empty());
        assert_eq!(first.release.has_front_art, None);
        assert!(hits
            .iter()
            .all(|hit| hit.release.release_group_id.as_deref() == Some(fixtures::RELEASE_GROUP)));
    }

    #[test]
    fn parses_a_release() {
        let release = parse_release(fixtures::RELEASES[1].1).unwrap();
        assert_eq!(release.id, fixtures::RELEASES[1].0);
        assert_eq!(release.date.as_deref(), Some("2016-05-06"));
        assert_eq!(release.country.as_deref(), Some("XW"));
        assert_eq!(release.barcode.as_deref(), Some("634904032463"));
        assert_eq!(
            release.labels,
            [Label {
                name: Some("XL Recordings".into()),
                catalog_number: Some("XLDA324".into())
            }]
        );
        assert_eq!(release.original_year(), Some(2007));
        assert_eq!(release.has_front_art, Some(true));
        assert_eq!(
            release.genres.first().map(String::as_str),
            Some("alternative rock")
        );
        assert_eq!(release.tracks.len(), 10);
        let first = &release.tracks[0];
        assert_eq!((first.disc, first.position), (1, 1));
        assert_eq!(first.title, "15 Step");
        assert_eq!(first.length_ms, Some(237000));
        assert_eq!(release.artist_ids, ["a74b1b7f-71a5-4011-9441-d0b5e4122711"]);
    }

    #[test]
    fn tolerates_missing_fields_and_refuses_garbage() {
        let release = parse_release(r#"{"id": "x", "media": [{"tracks": [{}, {}]}]}"#).unwrap();
        assert_eq!(release.title, "");
        assert_eq!(release.track_count, 2);
        assert_eq!(release.tracks[1].position, 2);
        assert!(parse_release("[]").is_err());
        assert!(parse_search("not json").is_err());
    }

    /// Against the real service: `cargo test live_ -- --ignored`.
    #[test]
    #[ignore]
    fn live_search_and_lookup() {
        use crate::metadata::http::{SystemClock, UreqTransport};
        let conn = db::open_in_memory().unwrap();
        let client = Client::new(Box::new(UreqTransport::new()), Box::new(SystemClock));
        let hits =
            search_releases(&client, &conn, "In Rainbows", Some("Radiohead"), Some(10)).unwrap();
        let hit = hits
            .iter()
            .find(|hit| hit.release.release_group_id.as_deref() == Some(fixtures::RELEASE_GROUP))
            .expect("In Rainbows is found");
        let release = lookup_release(&client, &conn, &hit.release.id).unwrap();
        assert_eq!(release.title, "In Rainbows");
        assert!(!release.tracks.is_empty());
    }

    #[test]
    fn looks_up_through_the_client_and_cache() {
        let conn = db::open_in_memory().unwrap();
        let (client, transport, _clock) = fake_client();
        let (id, json) = fixtures::RELEASES[0];
        transport.push_status(&release_url(id), 200, json);
        assert_eq!(lookup_release(&client, &conn, id).unwrap().id, id);
        assert_eq!(lookup_release(&client, &conn, id).unwrap().id, id);
        assert_eq!(transport.urls().len(), 1);
        assert!(lookup_release(&client, &conn, "../admin").is_err());
        assert_eq!(transport.urls().len(), 1, "a bad id is never requested");
    }
}
