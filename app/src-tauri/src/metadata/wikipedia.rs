//! Artist biographies and album descriptions from Wikipedia (PLAN.md Phase
//! 4.7): the lead section of the English article that the artist's, or the
//! album's release group's, MusicBrainz entry links to, through its
//! Wikidata item (or directly, for older entries), as plain text. Requests
//! go through `http::Client`, and responses are cached.
//!
//! The text is CC BY-SA 4.0: wherever it's shown, the article is credited
//! with a link to it (`Article::url`) and the licence (`LICENSE`).
//!
//! What was found is stored as the artist's `artist_links` row for
//! 'wikipedia', with the MusicBrainz artist it was fetched for as
//! `external_id`, and likewise as the album's `album_links` row, with its
//! MusicBrainz release group as `external_id`; so neither outlives a
//! change of match. An album matched to another release of the same
//! release group keeps its description.

use std::time::Duration;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::albums::{self, AlbumLink, LinkStatus};
use super::artists::{self, ArtistLink};
use super::http::{percent_encode, Client};
use super::musicbrainz::{self, Artist};
use super::settings::SourceId;
use super::Error;

pub const HOST: &str = "en.wikipedia.org";
pub const WIKIDATA_HOST: &str = "www.wikidata.org";

/// The Wikipedia the biographies come from, and its Wikidata site id.
const LANGUAGE: &str = "en";
const SITE: &str = "enwiki";

const ARTICLE_BASE: &str = "https://en.wikipedia.org/wiki/";

pub const LICENSE: &str = "CC BY-SA 4.0";
pub const LICENSE_URL: &str = "https://creativecommons.org/licenses/by-sa/4.0/";

/// Articles change more often than MusicBrainz entries, but a month-old
/// lead is still fine.
const MAX_AGE: Duration = Duration::from_secs(30 * 86400);

/// An article's lead as the app keeps it: an artist's biography or an
/// album's description. Stored as JSON in `artist_links.details` and
/// `album_links.details`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Article {
    /// The article's title and address, to credit and link it.
    pub title: String,
    pub url: String,
    pub language: String,
    /// The article's lead section as plain text, a paragraph each.
    pub paragraphs: Vec<String>,
}

/// The Wikidata request for item `id`'s English Wikipedia article.
pub fn sitelink_url(id: &str) -> String {
    format!(
        "https://{WIKIDATA_HOST}/w/api.php?action=wbgetentities&ids={}\
         &props=sitelinks&sitefilter={SITE}&format=json",
        percent_encode(id)
    )
}

/// The Wikipedia request for the lead section of article `title`, as plain
/// text, following redirects.
pub fn extract_url(title: &str) -> String {
    format!(
        "https://{HOST}/w/api.php?action=query&format=json&formatversion=2\
         &prop=extracts%7Cinfo&inprop=url&exintro=1&explaintext=1&redirects=1&titles={}",
        percent_encode(title)
    )
}

/// The title of Wikidata item `id`'s article in `SITE`, if it has one.
pub fn parse_sitelink(json: &str, id: &str) -> Result<Option<String>, Error> {
    let value: Value = serde_json::from_str(json)
        .map_err(|error| Error::Invalid(format!("Unexpected Wikidata response: {error}")))?;
    let entities = value
        .get("entities")
        .ok_or_else(|| Error::Invalid("Unexpected Wikidata response: no entities".into()))?;
    // An item merged into another comes back under the other's id.
    let entity = entities
        .get(id)
        .or_else(|| entities.as_object()?.values().next());
    Ok(entity
        .and_then(|entity| entity.pointer(&format!("/sitelinks/{SITE}/title")))
        .and_then(Value::as_str)
        .filter(|title| !title.trim().is_empty())
        .map(String::from))
}

/// The article in an extract response, or `None` if it doesn't exist or has
/// no lead.
pub fn parse_extract(json: &str) -> Result<Option<Article>, Error> {
    #[derive(Deserialize)]
    struct Response {
        query: Option<Query>,
    }
    #[derive(Deserialize)]
    struct Query {
        #[serde(default)]
        pages: Vec<Page>,
    }
    #[derive(Deserialize)]
    struct Page {
        title: String,
        #[serde(default)]
        missing: bool,
        extract: Option<String>,
        fullurl: Option<String>,
    }
    let response: Response = serde_json::from_str(json)
        .map_err(|error| Error::Invalid(format!("Unexpected Wikipedia response: {error}")))?;
    let Some(page) = response
        .query
        .and_then(|query| query.pages.into_iter().next())
        .filter(|page| !page.missing)
    else {
        return Ok(None);
    };
    let paragraphs: Vec<String> = page
        .extract
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(String::from)
        .collect();
    if paragraphs.is_empty() {
        return Ok(None);
    }
    let url = page
        .fullurl
        .filter(|url| url.starts_with(ARTICLE_BASE))
        .unwrap_or_else(|| article_url(&page.title));
    Ok(Some(Article {
        title: page.title,
        url,
        language: LANGUAGE.into(),
        paragraphs,
    }))
}

fn article_url(title: &str) -> String {
    format!("{ARTICLE_BASE}{}", percent_encode(&title.replace(' ', "_")))
}

/// The title in an English Wikipedia article URL, e.g. "AC/DC" from
/// ".../wiki/AC%2FDC".
fn title_from_url(url: &str) -> Option<String> {
    let encoded = url.strip_prefix(ARTICLE_BASE)?;
    let mut bytes = Vec::with_capacity(encoded.len());
    let mut rest = encoded.as_bytes();
    while let Some((&byte, tail)) = rest.split_first() {
        match (byte, tail) {
            (b'%', [high, low, tail @ ..]) => {
                let hex = std::str::from_utf8(&[*high, *low]).ok()?.to_owned();
                bytes.push(u8::from_str_radix(&hex, 16).ok()?);
                rest = tail;
            }
            (b'#' | b'?', _) => break,
            (b'_', _) => {
                bytes.push(b' ');
                rest = tail;
            }
            _ => {
                bytes.push(byte);
                rest = tail;
            }
        }
    }
    String::from_utf8(bytes)
        .ok()
        .filter(|title| !title.trim().is_empty())
}

/// What `fetch_biography` or `fetch_description` did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fetched {
    /// Nothing to fetch: the artist or album isn't matched on MusicBrainz.
    NotLinked,
    Found,
    /// No English article, or no link to one.
    NotFound,
}

/// Fetches artist `artist_id`'s biography for its MusicBrainz match and
/// records what was found. Offline, it fails with `Error::Offline` and
/// changes nothing. Whether the source is enabled is up to the caller.
pub fn fetch_biography(
    client: &Client,
    conn: &Connection,
    artist_id: i64,
) -> Result<Fetched, Error> {
    let link = artists::artist_link(conn, artist_id, SourceId::MusicBrainz)?;
    let Some((mbid, artist)) = link
        .as_ref()
        .and_then(|link| Some((link.matched_id()?.to_owned(), link.details::<Artist>()?)))
    else {
        return Ok(Fetched::NotLinked);
    };
    let biography = article(
        client,
        conn,
        artist.wikidata.as_deref(),
        artist.wikipedia.as_deref(),
    )?;
    let (status, score) = found(&biography);
    artists::store_link(
        conn,
        artist_id,
        SourceId::Wikipedia,
        status,
        Some(&mbid),
        score,
        biography.as_ref(),
    )?;
    Ok(fetched(&biography))
}

/// Fetches album `album_id`'s description for the release group of its
/// MusicBrainz match and records what was found, as `fetch_biography` does
/// for artists.
pub fn fetch_description(
    client: &Client,
    conn: &Connection,
    album_id: i64,
) -> Result<Fetched, Error> {
    let Some(group_id) = albums::matched_release_group(conn, album_id)? else {
        return Ok(Fetched::NotLinked);
    };
    let group = musicbrainz::lookup_release_group(client, conn, &group_id)?;
    let description = article(
        client,
        conn,
        group.wikidata.as_deref(),
        group.wikipedia.as_deref(),
    )?;
    let (status, score) = found(&description);
    albums::store_source_link(
        conn,
        album_id,
        SourceId::Wikipedia,
        status,
        Some(&group_id),
        score,
        description.as_ref(),
    )?;
    Ok(fetched(&description))
}

/// The lead of the article that Wikidata item `wikidata` has in English
/// Wikipedia, else of article URL `wikipedia`; `None` if there's no such
/// article.
fn article(
    client: &Client,
    conn: &Connection,
    wikidata: Option<&str>,
    wikipedia: Option<&str>,
) -> Result<Option<Article>, Error> {
    let title = match (wikidata, wikipedia) {
        (Some(id), _) => {
            let body = client.get_json(conn, &sitelink_url(id), MAX_AGE)?;
            parse_sitelink(&body, id)?
        }
        (None, Some(url)) => title_from_url(url),
        (None, None) => None,
    };
    match title {
        Some(title) => parse_extract(&client.get_json(conn, &extract_url(&title), MAX_AGE)?),
        None => Ok(None),
    }
}

fn found(article: &Option<Article>) -> (LinkStatus, f64) {
    match article {
        Some(_) => (LinkStatus::Matched, 1.0),
        None => (LinkStatus::None, 0.0),
    }
}

fn fetched(article: &Option<Article>) -> Fetched {
    match article {
        Some(_) => Fetched::Found,
        None => Fetched::NotFound,
    }
}

/// What was found when artist `artist_id`'s biography was last fetched:
/// status 'matched' or 'none', and the MusicBrainz artist asked about as
/// `external_id`.
pub fn biography_check(conn: &Connection, artist_id: i64) -> Result<Option<ArtistLink>, Error> {
    artists::artist_link(conn, artist_id, SourceId::Wikipedia)
}

/// Artist `artist_id`'s stored biography, if it was fetched for the
/// MusicBrainz artist it's matched to now.
pub fn biography(conn: &Connection, artist_id: i64) -> Result<Option<Article>, Error> {
    let Some(link) = artists::artist_link(conn, artist_id, SourceId::MusicBrainz)? else {
        return Ok(None);
    };
    let Some(mbid) = link.matched_id() else {
        return Ok(None);
    };
    Ok(biography_check(conn, artist_id)?
        .filter(|check| check.matched_id() == Some(mbid))
        .and_then(|check| check.details()))
}

/// What was found when album `album_id`'s description was last fetched:
/// status 'matched' or 'none', and the MusicBrainz release group asked about
/// as `external_id`.
pub fn description_check(conn: &Connection, album_id: i64) -> Result<Option<AlbumLink>, Error> {
    albums::album_link(conn, album_id, SourceId::Wikipedia)
}

/// Album `album_id`'s stored description, if it was fetched for the release
/// group of the release it's matched to now.
pub fn description(conn: &Connection, album_id: i64) -> Result<Option<Article>, Error> {
    let Some(group_id) = albums::matched_release_group(conn, album_id)? else {
        return Ok(None);
    };
    Ok(description_check(conn, album_id)?
        .filter(|check| check.matched_id() == Some(group_id.as_str()))
        .and_then(|check| check.details()))
}

#[cfg(test)]
pub mod fixtures {
    //! Recorded Wikidata responses (CC0) for the items of Radiohead and "In
    //! Rainbows", and extracts of their articles whose text is replaced with
    //! a stand-in, so no article text (CC BY-SA) is copied into the
    //! repository.

    pub const WIKIDATA_ID: &str = "Q44190";
    pub const SITELINK: &str = include_str!("fixtures/wikipedia/wikidata-Q44190.json");
    pub const EXTRACT: &str = include_str!("fixtures/wikipedia/extract-radiohead.json");

    pub const ALBUM_WIKIDATA_ID: &str = "Q223295";
    pub const ALBUM_SITELINK: &str = include_str!("fixtures/wikipedia/wikidata-Q223295.json");
    pub const ALBUM_EXTRACT: &str = include_str!("fixtures/wikipedia/extract-in-rainbows.json");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::test_library::{track, Library};
    use crate::metadata::http::testing::fake_client;
    use crate::metadata::musicbrainz::{self, fixtures as mb};

    /// Radiohead, matched on MusicBrainz to `artist`; returns its id.
    fn matched(library: &Library, artist: &Artist) -> i64 {
        let id = library.artist(Some("Radiohead")).unwrap();
        artists::store_link(
            &library.conn,
            id,
            SourceId::MusicBrainz,
            LinkStatus::Matched,
            Some(&artist.id),
            1.0,
            Some(artist),
        )
        .unwrap();
        id
    }

    fn library() -> Library {
        Library::new([track("Radiohead/In Rainbows/01.flac").artist("Radiohead")])
    }

    #[test]
    fn parses_responses() {
        assert_eq!(
            parse_sitelink(fixtures::SITELINK, fixtures::WIKIDATA_ID)
                .unwrap()
                .as_deref(),
            Some("Radiohead")
        );
        assert_eq!(
            parse_sitelink(r#"{"entities": {"Q1": {"sitelinks": {}}}}"#, "Q1").unwrap(),
            None
        );
        assert_eq!(
            parse_sitelink(r#"{"entities": {"Q1": {"id": "Q1", "missing": ""}}}"#, "Q1").unwrap(),
            None
        );
        let merged = r#"{"entities": {"Q2": {"sitelinks": {"enwiki": {"title": "Two"}}}}}"#;
        assert_eq!(
            parse_sitelink(merged, "Q1").unwrap().as_deref(),
            Some("Two")
        );
        assert!(parse_sitelink(r#"{"error": {}}"#, "Q1").is_err());

        let biography = parse_extract(fixtures::EXTRACT).unwrap().unwrap();
        assert_eq!(biography.title, "Radiohead");
        assert_eq!(biography.url, "https://en.wikipedia.org/wiki/Radiohead");
        assert_eq!(biography.paragraphs.len(), 2, "blank lines dropped");
        assert!(biography.paragraphs[0].starts_with("Radiohead are"));
        let missing = r#"{"query": {"pages": [{"ns": 0, "title": "X", "missing": true}]}}"#;
        assert_eq!(parse_extract(missing).unwrap(), None);
        let empty = r#"{"query": {"pages": [{"title": "X", "extract": "\n "}]}}"#;
        assert_eq!(parse_extract(empty).unwrap(), None);
        // An unexpected address is replaced with the article's own.
        let odd = r#"{"query": {"pages": [{"title": "A B", "extract": "x",
                      "fullurl": "javascript:alert(1)"}]}}"#;
        assert_eq!(
            parse_extract(odd).unwrap().unwrap().url,
            "https://en.wikipedia.org/wiki/A_B"
        );
    }

    #[test]
    fn reads_titles_from_article_urls() {
        let title = |url: &str| title_from_url(url);
        assert_eq!(
            title("https://en.wikipedia.org/wiki/AC%2FDC").as_deref(),
            Some("AC/DC")
        );
        assert_eq!(
            title("https://en.wikipedia.org/wiki/Sigur_R%C3%B3s#History").as_deref(),
            Some("Sigur Rós")
        );
        assert_eq!(title("https://fr.wikipedia.org/wiki/X"), None);
        assert_eq!(title("https://en.wikipedia.org/wiki/%E2%28"), None);
        assert_eq!(title("https://en.wikipedia.org/wiki/%zz"), None);
        assert_eq!(title("https://en.wikipedia.org/wiki/"), None);
    }

    #[test]
    fn fetches_through_wikidata_and_records_it() {
        let library = library();
        let artist = musicbrainz::parse_artist(mb::ARTIST_JSON).unwrap();
        let (client, transport, _clock) = fake_client();
        let id = library.artist(Some("Radiohead")).unwrap();
        assert_eq!(
            fetch_biography(&client, &library.conn, id).unwrap(),
            Fetched::NotLinked
        );
        let id = matched(&library, &artist);
        transport.push_status(
            &sitelink_url(fixtures::WIKIDATA_ID),
            200,
            fixtures::SITELINK,
        );
        transport.push_status(&extract_url("Radiohead"), 200, fixtures::EXTRACT);
        assert_eq!(
            fetch_biography(&client, &library.conn, id).unwrap(),
            Fetched::Found
        );
        let biography = biography(&library.conn, id).unwrap().unwrap();
        assert_eq!(biography.title, "Radiohead");
        let check = biography_check(&library.conn, id).unwrap().unwrap();
        assert_eq!(check.external_id.as_deref(), Some(mb::ARTIST));

        // Matched to another artist now: the biography no longer applies.
        let mut other = artist.clone();
        other.id = "00000000-0000-0000-0000-000000000001".into();
        matched(&library, &other);
        assert_eq!(super::biography(&library.conn, id).unwrap(), None);
    }

    #[test]
    fn records_an_artist_without_an_article() {
        let library = library();
        let mut artist = musicbrainz::parse_artist(mb::ARTIST_JSON).unwrap();
        artist.wikidata = None;
        let id = matched(&library, &artist);
        let (client, transport, _clock) = fake_client();
        assert_eq!(
            fetch_biography(&client, &library.conn, id).unwrap(),
            Fetched::NotFound
        );
        assert!(transport.urls().is_empty());
        let check = biography_check(&library.conn, id).unwrap().unwrap();
        assert_eq!(check.status, LinkStatus::None);

        // An old direct link to the article.
        artist.wikipedia = Some("https://en.wikipedia.org/wiki/Radiohead".into());
        matched(&library, &artist);
        transport.push_status(&extract_url("Radiohead"), 200, fixtures::EXTRACT);
        assert_eq!(
            fetch_biography(&client, &library.conn, id).unwrap(),
            Fetched::Found
        );
    }

    #[test]
    fn offline_changes_nothing() {
        let library = library();
        let artist = musicbrainz::parse_artist(mb::ARTIST_JSON).unwrap();
        let id = matched(&library, &artist);
        let (client, transport, _clock) = fake_client();
        transport.push(
            &sitelink_url(fixtures::WIKIDATA_ID),
            Err(crate::metadata::http::TransportError::Unreachable(
                "down".into(),
            )),
        );
        let error = fetch_biography(&client, &library.conn, id).unwrap_err();
        assert!(matches!(error, Error::Offline(_)), "{error}");
        assert_eq!(biography_check(&library.conn, id).unwrap(), None);
    }

    /// "In Rainbows" (album 1), matched on MusicBrainz to release
    /// `RELEASES[index]` with `status`.
    fn album_matched(library: &Library, index: usize, status: LinkStatus) {
        let release = musicbrainz::parse_release(mb::RELEASES[index].1).unwrap();
        albums::store_source_link(
            &library.conn,
            1,
            SourceId::MusicBrainz,
            status,
            Some(&release.id),
            1.0,
            Some(&release),
        )
        .unwrap();
    }

    fn album_library() -> Library {
        Library::new([track("Radiohead/In Rainbows/01.flac")
            .artist("Radiohead")
            .album("In Rainbows")])
    }

    fn serve_album(transport: &crate::metadata::http::testing::FakeTransport) {
        transport.push_status(
            &musicbrainz::release_group_url(mb::RELEASE_GROUP),
            200,
            mb::RELEASE_GROUP_JSON,
        );
        transport.push_status(
            &sitelink_url(fixtures::ALBUM_WIKIDATA_ID),
            200,
            fixtures::ALBUM_SITELINK,
        );
        transport.push_status(&extract_url("In Rainbows"), 200, fixtures::ALBUM_EXTRACT);
    }

    #[test]
    fn fetches_album_descriptions_by_release_group() {
        let library = album_library();
        let (client, transport, _clock) = fake_client();
        assert_eq!(
            fetch_description(&client, &library.conn, 1).unwrap(),
            Fetched::NotLinked
        );
        // A candidate awaiting review isn't enough.
        album_matched(&library, 1, LinkStatus::Review);
        assert_eq!(
            fetch_description(&client, &library.conn, 1).unwrap(),
            Fetched::NotLinked
        );
        assert!(transport.urls().is_empty());

        album_matched(&library, 1, LinkStatus::Matched);
        serve_album(&transport);
        assert_eq!(
            fetch_description(&client, &library.conn, 1).unwrap(),
            Fetched::Found
        );
        let description = description(&library.conn, 1).unwrap().unwrap();
        assert_eq!(description.title, "In Rainbows");
        assert_eq!(description.url, "https://en.wikipedia.org/wiki/In_Rainbows");
        assert!(description.paragraphs[0].starts_with("In Rainbows is"));
        let check = description_check(&library.conn, 1).unwrap().unwrap();
        assert_eq!(check.external_id.as_deref(), Some(mb::RELEASE_GROUP));
        assert_eq!(check.release, None, "only MusicBrainz links have one");

        // Another release of the same album keeps it; losing the match
        // hides it.
        album_matched(&library, 3, LinkStatus::Matched);
        assert!(super::description(&library.conn, 1).unwrap().is_some());
        album_matched(&library, 3, LinkStatus::Review);
        assert_eq!(super::description(&library.conn, 1).unwrap(), None);
    }

    #[test]
    fn records_an_album_without_an_article() {
        let library = album_library();
        album_matched(&library, 1, LinkStatus::Matched);
        let (client, transport, _clock) = fake_client();
        transport.push_status(
            &musicbrainz::release_group_url(mb::RELEASE_GROUP),
            200,
            &format!(r#"{{"id": "{}", "relations": []}}"#, mb::RELEASE_GROUP),
        );
        assert_eq!(
            fetch_description(&client, &library.conn, 1).unwrap(),
            Fetched::NotFound
        );
        assert_eq!(transport.urls().len(), 1, "only the release group");
        let check = description_check(&library.conn, 1).unwrap().unwrap();
        assert_eq!(
            (check.status, check.external_id.as_deref()),
            (LinkStatus::None, Some(mb::RELEASE_GROUP))
        );
        assert_eq!(description(&library.conn, 1).unwrap(), None);
    }

    /// Against the real services: `cargo test live_ -- --ignored`.
    #[test]
    #[ignore]
    fn live_biography() {
        use crate::metadata::http::{SystemClock, UreqTransport};
        let library = library();
        let client = Client::new(Box::new(UreqTransport::new()), Box::new(SystemClock));
        let artist = musicbrainz::lookup_artist(&client, &library.conn, mb::ARTIST).unwrap();
        let id = matched(&library, &artist);
        assert_eq!(
            fetch_biography(&client, &library.conn, id).unwrap(),
            Fetched::Found
        );
        let biography = biography(&library.conn, id).unwrap().unwrap();
        assert!(biography.paragraphs[0].contains("Radiohead"));
    }

    /// Against the real services: `cargo test live_ -- --ignored`.
    #[test]
    #[ignore]
    fn live_description() {
        use crate::metadata::http::{SystemClock, UreqTransport};
        let library = album_library();
        let client = Client::new(Box::new(UreqTransport::new()), Box::new(SystemClock));
        let (id, _) = mb::RELEASES[1];
        let release = musicbrainz::lookup_release(&client, &library.conn, id).unwrap();
        albums::store_source_link(
            &library.conn,
            1,
            SourceId::MusicBrainz,
            LinkStatus::Matched,
            Some(&release.id),
            1.0,
            Some(&release),
        )
        .unwrap();
        assert_eq!(
            fetch_description(&client, &library.conn, 1).unwrap(),
            Fetched::Found
        );
        let description = description(&library.conn, 1).unwrap().unwrap();
        assert!(description.paragraphs[0].contains("Radiohead"));
    }
}
