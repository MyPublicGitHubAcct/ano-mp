//! ListenBrainz's similar artists (PLAN.md X5): for a MusicBrainz artist,
//! the artists its listeners also play, from the Labs API's
//! `similar-artists` query, the one ListenBrainz's own artist pages use.
//! The answer is a list, best first; an artist ListenBrainz doesn't know
//! gets an empty one.
//!
//! The data is MetaBrainz's (commercial use asks for a supporter plan, as
//! for MusicBrainz) and is kept in the response cache like MusicBrainz's.
//! Labs is ListenBrainz's experimental API: the algorithm named here may be
//! renamed, which shows as an error and is noted in PLAN.md's risks.

use std::time::Duration;

use rusqlite::Connection;
use serde::Deserialize;

use super::cache;
use super::http::Client;
use super::musicbrainz;
use super::Error;

const LABS: &str = "https://labs.api.listenbrainz.org";

/// The similarity ListenBrainz's artist pages show.
const ALGORITHM: &str =
    "session_based_days_7500_session_300_contribution_3_threshold_10_limit_100_filter_True_skip_30";

/// How long a fetched list is used before asking again: it is recomputed
/// from listens now and then, not daily.
pub const MAX_AGE: Duration = Duration::from_secs(30 * 86400);

/// An artist ListenBrainz finds similar to another.
#[derive(Debug, Clone, PartialEq)]
pub struct SimilarArtist {
    pub mbid: String,
    pub name: String,
    /// MusicBrainz's disambiguation, e.g. "UK rock band".
    pub comment: Option<String>,
    /// "Person", "Group"…
    pub artist_type: Option<String>,
    /// ListenBrainz's score: only its order within one list means anything.
    pub score: f64,
}

pub fn similar_artists_url(mbid: &str) -> String {
    format!("{LABS}/similar-artists/json?artist_mbids={mbid}&algorithm={ALGORITHM}")
}

/// The artist's page on ListenBrainz.
pub fn artist_page(mbid: &str) -> String {
    format!("https://listenbrainz.org/artist/{mbid}/")
}

pub fn parse_similar_artists(json: &str) -> Result<Vec<SimilarArtist>, Error> {
    #[derive(Deserialize)]
    struct Raw {
        artist_mbid: Option<String>,
        name: Option<String>,
        comment: Option<String>,
        #[serde(rename = "type")]
        artist_type: Option<String>,
        score: Option<f64>,
    }
    let raw: Vec<Raw> = serde_json::from_str(json).map_err(|error| {
        Error::Invalid(format!("Unexpected ListenBrainz similar artists: {error}"))
    })?;
    let non_empty = |text: Option<String>| text.filter(|text| !text.trim().is_empty());
    Ok(raw
        .into_iter()
        .filter_map(|raw| {
            let mbid = raw.artist_mbid.filter(|id| musicbrainz::is_mbid(id))?;
            Some(SimilarArtist {
                mbid,
                name: non_empty(raw.name)?,
                comment: non_empty(raw.comment),
                artist_type: non_empty(raw.artist_type),
                score: raw.score.unwrap_or(0.0),
            })
        })
        .collect())
}

/// The artists similar to MusicBrainz artist `mbid`, through the client and
/// the response cache.
pub fn similar_artists(
    client: &Client,
    conn: &Connection,
    mbid: &str,
) -> Result<Vec<SimilarArtist>, Error> {
    if !musicbrainz::is_mbid(mbid) {
        return Err(Error::Invalid(format!("Not a MusicBrainz id: {mbid}")));
    }
    parse_similar_artists(&client.get_json(conn, &similar_artists_url(mbid), MAX_AGE)?)
}

/// As `similar_artists`, from the response cache alone, however old: for
/// while online services are off. None for an artist never fetched.
pub fn cached_similar_artists(conn: &Connection, mbid: &str) -> Result<Vec<SimilarArtist>, Error> {
    match cache::lookup(conn, &similar_artists_url(mbid))? {
        Some((body, _)) => parse_similar_artists(&body),
        None => Ok(Vec::new()),
    }
}

#[cfg(test)]
pub mod fixtures {
    pub const RADIOHEAD: &str = "a74b1b7f-71a5-4011-9441-d0b5e4122711";
    pub const SIMILAR: &str = include_str!(
        "fixtures/listenbrainz/similar-artists-a74b1b7f-71a5-4011-9441-d0b5e4122711.json"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::db;
    use crate::metadata::http::testing::fake_client;

    #[test]
    fn parses_similar_artists_best_first() {
        let similar = parse_similar_artists(fixtures::SIMILAR).unwrap();
        assert_eq!(similar.len(), 8);
        assert_eq!(similar[0].name, "The Beatles");
        assert_eq!(similar[0].mbid, "b10bbbfc-cf9e-42e0-be17-e2c3e1d2600d");
        assert_eq!(
            similar[0].comment.as_deref(),
            Some("UK rock band, “The Fab Four”")
        );
        assert_eq!(similar[0].artist_type.as_deref(), Some("Group"));
        assert_eq!(similar[1].comment, None, "an empty comment is none");
        assert!(similar.windows(2).all(|w| w[0].score >= w[1].score));
    }

    #[test]
    fn leaves_out_entries_without_an_id_or_a_name() {
        let similar = parse_similar_artists(
            r#"[{"artist_mbid": null, "name": "A"}, {"artist_mbid": "../x", "name": "B"},
                {"artist_mbid": "b10bbbfc-cf9e-42e0-be17-e2c3e1d2600d", "name": ""},
                {"artist_mbid": "5b11f4ce-a62d-471e-81fc-a69a8278c7da", "name": "Nirvana"}]"#,
        )
        .unwrap();
        assert_eq!(similar.len(), 1);
        assert_eq!(similar[0].name, "Nirvana");
        assert!(parse_similar_artists("[]").unwrap().is_empty());
        assert!(parse_similar_artists(r#"{"error": "x"}"#).is_err());
    }

    /// Against the real service: `cargo test live_ -- --ignored`. Labs can
    /// take most of a minute to answer.
    #[test]
    #[ignore]
    fn live_similar_artists() {
        use crate::metadata::http::{SystemClock, UreqTransport};
        let conn = db::open_in_memory().unwrap();
        let client = Client::new(Box::new(UreqTransport::new()), Box::new(SystemClock));
        let similar = similar_artists(&client, &conn, fixtures::RADIOHEAD).unwrap();
        assert!(similar.len() >= 10, "the algorithm is still served");
        assert!(similar
            .iter()
            .all(|artist| artist.mbid != fixtures::RADIOHEAD));
    }

    #[test]
    fn fetches_through_the_cache() {
        let (client, transport, _clock) = fake_client();
        let conn = db::open_in_memory().unwrap();
        let url = similar_artists_url(fixtures::RADIOHEAD);
        transport.push_status(&url, 200, fixtures::SIMILAR);
        assert_eq!(
            similar_artists(&client, &conn, fixtures::RADIOHEAD)
                .unwrap()
                .len(),
            8
        );
        // The second time comes from the cache.
        similar_artists(&client, &conn, fixtures::RADIOHEAD).unwrap();
        assert_eq!(transport.urls(), [url]);
        assert!(similar_artists(&client, &conn, "../admin").is_err());
        assert_eq!(transport.urls().len(), 1, "a bad id is never requested");
        // Offline, only what was fetched.
        assert_eq!(
            cached_similar_artists(&conn, fixtures::RADIOHEAD)
                .unwrap()
                .len(),
            8
        );
        assert!(
            cached_similar_artists(&conn, "5b11f4ce-a62d-471e-81fc-a69a8278c7da")
                .unwrap()
                .is_empty()
        );
    }
}
