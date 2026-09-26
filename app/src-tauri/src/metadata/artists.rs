//! Matching library artists to MusicBrainz artists and keeping the result in
//! `artist_links` (migration 003), as `albums` does for albums. The match
//! gives the facts shown on the artist page and the links that lead to a
//! biography (`wikipedia`), whose row sits next to it. An automatic match
//! never replaces one the user chose.
//!
//! An artist is matched by the first of these that works: the artist MBID in
//! the tags; the artist credited on the releases its albums are matched to;
//! a search by name. Names are often shared ("Nirvana" is several bands), so
//! a search hit is accepted only if it's the one clear best of that name.

use rusqlite::{params, Connection, OptionalExtension};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::albums::LinkStatus;
use super::cache::unix_now;
use super::http::Client;
use super::matcher;
use super::musicbrainz::{self, Artist, ArtistHit, VARIOUS_ARTISTS};
use super::settings::{self, Kind, SourceId};
use super::wikipedia::{self, Biography};
use super::Error;

/// A search hit is accepted automatically at this MusicBrainz score (0 to
/// 100) or more, if no other artist of the same name comes within
/// `ACCEPT_LEAD` of it. MusicBrainz ranks the best-known artist of a name
/// well above the rest ("Nirvana" the US band at 100, the UK one at 75).
pub const ACCEPT_SCORE: u32 = 90;
pub const ACCEPT_LEAD: u32 = 15;

/// Score stored for a match found through the artist's matched releases.
const RELEASE_SCORE: f64 = 0.95;

/// An artist's row in `artist_links` for one source. `details` is the
/// source's JSON: an `Artist` for MusicBrainz, a `Biography` for Wikipedia.
#[derive(Debug, Clone, PartialEq)]
pub struct ArtistLink {
    pub status: LinkStatus,
    pub external_id: Option<String>,
    pub score: f64,
    pub chosen_by_user: bool,
    pub details: Option<String>,
    pub checked_at: i64,
}

impl ArtistLink {
    /// The details, if there are any and they parse (details stored by
    /// another version may not; they are fetched again then).
    pub fn details<T: DeserializeOwned>(&self) -> Option<T> {
        serde_json::from_str(self.details.as_deref()?).ok()
    }

    /// The external id if the link is an accepted match.
    pub fn matched_id(&self) -> Option<&str> {
        match self.status {
            LinkStatus::Matched => self.external_id.as_deref(),
            _ => None,
        }
    }
}

pub fn artist_link(
    conn: &Connection,
    artist_id: i64,
    source: SourceId,
) -> Result<Option<ArtistLink>, Error> {
    Ok(conn
        .prepare_cached(
            "SELECT status, external_id, score, chosen_by, details, checked_at
             FROM artist_links WHERE artist_id = ?1 AND source = ?2",
        )?
        .query_row(params![artist_id, source.as_str()], |row| {
            let status: String = row.get(0)?;
            let chosen_by: String = row.get(3)?;
            Ok(ArtistLink {
                status: LinkStatus::from_str(&status).unwrap_or(LinkStatus::None),
                external_id: row.get(1)?,
                score: row.get(2)?,
                chosen_by_user: chosen_by == "user",
                details: row.get(4)?,
                checked_at: row.get(5)?,
            })
        })
        .optional()?)
}

/// Stores what `source` has for artist `artist_id`. An automatic result
/// never replaces the user's.
pub fn store_link(
    conn: &Connection,
    artist_id: i64,
    source: SourceId,
    status: LinkStatus,
    external_id: Option<&str>,
    score: f64,
    details: Option<&impl Serialize>,
) -> Result<(), Error> {
    store(
        conn,
        artist_id,
        source,
        status,
        external_id,
        score,
        details,
        false,
    )
}

#[allow(clippy::too_many_arguments)]
fn store(
    conn: &Connection,
    artist_id: i64,
    source: SourceId,
    status: LinkStatus,
    external_id: Option<&str>,
    score: f64,
    details: Option<&impl Serialize>,
    by_user: bool,
) -> Result<(), Error> {
    let details = details.map(|details| serde_json::to_string(details).expect("details serialize"));
    conn.prepare_cached(
        "INSERT INTO artist_links
             (artist_id, source, status, external_id, score, chosen_by, details, checked_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT (artist_id, source) DO UPDATE SET
             status = excluded.status, external_id = excluded.external_id,
             score = excluded.score, chosen_by = excluded.chosen_by,
             details = excluded.details, checked_at = excluded.checked_at
         WHERE artist_links.chosen_by = 'auto' OR excluded.chosen_by = 'user'",
    )?
    .execute(params![
        artist_id,
        source.as_str(),
        status.as_str(),
        external_id,
        score,
        if by_user { "user" } else { "auto" },
        details,
        unix_now()
    ])?;
    Ok(())
}

/// An artist offered in the "Find artist" dialog, with MusicBrainz's score
/// (0 to 100) for how well it matched the search; 100 for one looked up by
/// id.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistCandidate {
    pub artist: Artist,
    pub score: u32,
}

/// The MusicBrainz artists the user can pick for artist `artist_id`: the
/// hits of a search for `name` (the artist's own when `None`, as automatic
/// matching searches), without "Various Artists", and the current match or
/// review candidate. An artist MBID or MusicBrainz artist URL is looked up
/// instead.
pub fn artist_candidates(
    client: &Client,
    conn: &Connection,
    artist_id: i64,
    name: Option<&str>,
) -> Result<Vec<ArtistCandidate>, Error> {
    let own: String = conn
        .query_row(
            "SELECT name FROM artists WHERE id = ?1",
            [artist_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| Error::Invalid(format!("No artist with id {artist_id}")))?;
    let name = name.unwrap_or(&own);
    let mut candidates: Vec<ArtistCandidate> =
        if let Some(id) = musicbrainz::mbid_in(name, "artist") {
            match musicbrainz::lookup_artist(client, conn, id) {
                Ok(artist) => vec![ArtistCandidate { artist, score: 100 }],
                Err(Error::Status { status: 404, .. }) => {
                    return Err(Error::Invalid(format!("MusicBrainz has no artist {id}")))
                }
                Err(error) => return Err(error),
            }
        } else {
            musicbrainz::search_artists(client, conn, name)?
                .into_iter()
                .filter(|hit| hit.artist.id != VARIOUS_ARTISTS)
                .map(|hit| ArtistCandidate {
                    artist: hit.artist,
                    score: hit.score,
                })
                .collect()
        };
    let current = artist_link(conn, artist_id, SourceId::MusicBrainz)?;
    if let Some(link) = current.filter(|link| link.status != LinkStatus::None) {
        if let Some(artist) = link.details::<Artist>() {
            if !candidates.iter().any(|c| c.artist.id == artist.id) {
                let score = (link.score * 100.0).round() as u32;
                candidates.push(ArtistCandidate { artist, score });
            }
        }
    }
    Ok(candidates)
}

/// Links artist `artist_id` to the MusicBrainz artist the user picked,
/// which automatic matching then leaves alone. Its biography follows the
/// new match (`jobs::needs_biography`).
pub fn choose_artist(
    client: &Client,
    conn: &Connection,
    artist_id: i64,
    mbid: &str,
) -> Result<(), Error> {
    let artist = musicbrainz::lookup_artist(client, conn, mbid)?;
    store(
        conn,
        artist_id,
        SourceId::MusicBrainz,
        LinkStatus::Matched,
        Some(&artist.id),
        1.0,
        Some(&artist),
        true,
    )
}

/// Records that none of MusicBrainz's artists is artist `artist_id` ("None
/// of these"), which automatic matching then leaves alone.
pub fn reject_artists(conn: &Connection, artist_id: i64) -> Result<(), Error> {
    store(
        conn,
        artist_id,
        SourceId::MusicBrainz,
        LinkStatus::None,
        None,
        0.0,
        None::<&Artist>,
        true,
    )
}

/// Forgets artist `artist_id`'s link to `source`, so the next automatic run
/// looks it up afresh.
pub fn clear_link(conn: &Connection, artist_id: i64, source: SourceId) -> Result<(), Error> {
    conn.execute(
        "DELETE FROM artist_links WHERE artist_id = ?1 AND source = ?2",
        params![artist_id, source.as_str()],
    )?;
    Ok(())
}

/// What `decide` made of an artist search.
#[derive(Debug, Clone, PartialEq)]
pub enum Decision<'a> {
    Matched(&'a ArtistHit),
    /// Artists of that name, none clearly the one.
    Review(&'a ArtistHit),
    NoMatch,
}

/// Picks the artist called `name` among search `hits`: those with that name
/// (or alias), of which the best is accepted at `ACCEPT_SCORE` with a lead
/// of `ACCEPT_LEAD` over the next. "Various Artists" is never an artist.
pub fn decide<'a>(name: &str, hits: &'a [ArtistHit]) -> Decision<'a> {
    let mut named: Vec<&ArtistHit> = hits
        .iter()
        .filter(|hit| {
            hit.artist.id != VARIOUS_ARTISTS
                && (matcher::same_name(&hit.artist.name, name)
                    || hit
                        .artist
                        .aliases
                        .iter()
                        .any(|a| matcher::same_name(a, name)))
        })
        .collect();
    // Stable, so MusicBrainz's order breaks ties.
    named.sort_by(|a, b| b.score.cmp(&a.score));
    let Some(best) = named.first() else {
        return Decision::NoMatch;
    };
    let runner_up = named.get(1).map_or(0, |hit| hit.score);
    if best.score >= ACCEPT_SCORE && best.score - runner_up >= ACCEPT_LEAD {
        Decision::Matched(best)
    } else {
        Decision::Review(best)
    }
}

/// The MusicBrainz artist credited alone, under the artist's name, on the
/// releases that artist `artist_id`'s albums are matched to: the one most of
/// them credit.
fn artist_from_releases(
    conn: &Connection,
    artist_id: i64,
    name: &str,
) -> Result<Option<String>, Error> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Credit {
        artist: String,
        artist_ids: Vec<String>,
    }
    let mut statement = conn.prepare_cached(
        "SELECT l.details FROM album_links l JOIN albums al ON al.id = l.album_id
         WHERE al.artist_id = ?1 AND l.source = ?2 AND l.status = 'matched'
             AND l.details IS NOT NULL
         ORDER BY al.id",
    )?;
    let rows = statement.query_map(params![artist_id, SourceId::MusicBrainz.as_str()], |row| {
        row.get::<_, String>(0)
    })?;
    let mut votes: Vec<(String, usize)> = Vec::new();
    for details in rows {
        let Ok(credit) = serde_json::from_str::<Credit>(&details?) else {
            continue;
        };
        if let [id] = credit.artist_ids.as_slice() {
            if id != VARIOUS_ARTISTS && matcher::same_name(&credit.artist, name) {
                match votes.iter_mut().find(|(voted, _)| voted == id) {
                    Some((_, count)) => *count += 1,
                    None => votes.push((id.clone(), 1)),
                }
            }
        }
    }
    // The most votes; the first album's on a tie.
    Ok(votes
        .into_iter()
        .rev()
        .max_by_key(|(_, count)| *count)
        .map(|(id, _)| id))
}

/// Matches artist `artist_id` to a MusicBrainz artist, unless the user chose
/// one, and stores the result with the artist's details.
pub fn match_artist(
    client: &Client,
    conn: &Connection,
    artist_id: i64,
) -> Result<LinkStatus, Error> {
    if let Some(link) = artist_link(conn, artist_id, SourceId::MusicBrainz)? {
        if link.chosen_by_user {
            return Ok(link.status);
        }
    }
    let (name, tagged): (String, Option<String>) = conn
        .query_row(
            "SELECT name, musicbrainz_id FROM artists WHERE id = ?1",
            [artist_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .ok_or_else(|| Error::Invalid(format!("No artist with id {artist_id}")))?;
    let store = |status, artist: Option<&Artist>, score| {
        store_link(
            conn,
            artist_id,
            SourceId::MusicBrainz,
            status,
            artist.map(|artist| artist.id.as_str()),
            score,
            artist,
        )
        .map(|()| status)
    };
    if matcher::is_various(&name) {
        return store(LinkStatus::None, None, 0.0);
    }

    let tagged = tagged.filter(|id| musicbrainz::is_mbid(id) && id != VARIOUS_ARTISTS);
    let credited = artist_from_releases(conn, artist_id, &name)?;
    for (id, score) in [(tagged, 1.0), (credited, RELEASE_SCORE)] {
        let Some(id) = id else { continue };
        match musicbrainz::lookup_artist(client, conn, &id) {
            Ok(artist) => return store(LinkStatus::Matched, Some(&artist), score),
            // Gone (e.g. merged away): try the next way.
            Err(Error::Status { status: 404, .. }) => {}
            Err(error) => return Err(error),
        }
    }

    let hits = musicbrainz::search_artists(client, conn, &name)?;
    match decide(&name, &hits) {
        Decision::Matched(hit) => {
            // The search has no links; the lookup does.
            let artist = musicbrainz::lookup_artist(client, conn, &hit.artist.id)?;
            store(LinkStatus::Matched, Some(&artist), score_of(hit))
        }
        Decision::Review(hit) => store(LinkStatus::Review, Some(&hit.artist), score_of(hit)),
        Decision::NoMatch => store(LinkStatus::None, None, 0.0),
    }
}

fn score_of(hit: &ArtistHit) -> f64 {
    f64::from(hit.score.min(100)) / 100.0
}

/// A biography as the artist page shows it: the text, the source, and the
/// licence to credit it under.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcedBiography {
    pub source: SourceId,
    pub source_name: &'static str,
    pub license: &'static str,
    pub license_url: &'static str,
    #[serde(flatten)]
    pub biography: Biography,
}

/// What the metadata sources know about an artist, as the artist page
/// shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistInfo {
    /// The MusicBrainz match's status; `None` if never looked up.
    pub status: Option<LinkStatus>,
    /// When it was last looked up (Unix seconds).
    pub checked_at: Option<i64>,
    /// Whether the user chose the match (or chose "none of these").
    pub chosen_by_user: bool,
    /// The matched artist, while MusicBrainz is shown.
    pub musicbrainz: Option<Artist>,
    /// The first biography found among the artist-info sources shown.
    pub biography: Option<SourcedBiography>,
    /// Whether the user can ask for a lookup now (MusicBrainz usable).
    pub can_look_up: bool,
}

pub fn artist_info(conn: &Connection, artist_id: i64) -> Result<ArtistInfo, Error> {
    let settings = settings::service_settings(conn)?;
    let link = artist_link(conn, artist_id, SourceId::MusicBrainz)?;
    let musicbrainz = link
        .as_ref()
        .filter(|link| link.matched_id().is_some() && settings.is_shown(SourceId::MusicBrainz))
        .and_then(|link| link.details::<Artist>());
    let mut biography = None;
    for source in settings.sources_shown(Kind::ArtistInfo) {
        let found = match source {
            SourceId::Wikipedia => {
                wikipedia::biography(conn, artist_id)?.map(|biography| SourcedBiography {
                    source,
                    source_name: source.info().name,
                    license: wikipedia::LICENSE,
                    license_url: wikipedia::LICENSE_URL,
                    biography,
                })
            }
            _ => None,
        };
        if found.is_some() {
            biography = found;
            break;
        }
    }
    Ok(ArtistInfo {
        status: link.as_ref().map(|link| link.status),
        checked_at: link.as_ref().map(|link| link.checked_at),
        chosen_by_user: link.as_ref().is_some_and(|link| link.chosen_by_user),
        musicbrainz,
        biography,
        can_look_up: settings.is_usable(SourceId::MusicBrainz),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::test_library::{track, Library};
    use crate::metadata::http::testing::fake_client;
    use crate::metadata::musicbrainz::{artist_search_url, artist_url, fixtures};

    fn hit(id: &str, name: &str, score: u32) -> ArtistHit {
        let mut artist = musicbrainz::parse_artist(&format!(r#"{{"id": "{id}"}}"#)).unwrap();
        artist.name = name.into();
        ArtistHit { artist, score }
    }

    /// Three Radiohead tracks, and "Nirvana" and "Various Artists" albums.
    fn library() -> Library {
        Library::new(
            (1..=3)
                .map(|n| {
                    track(&format!("Radiohead/In Rainbows/{n:02}.flac"))
                        .artist("Radiohead")
                        .album("In Rainbows")
                })
                .chain([
                    track("Nirvana/Nevermind/01.flac")
                        .artist("Nirvana")
                        .album("Nevermind"),
                    track("VA/Hits/01.flac")
                        .artist("Someone")
                        .album_artist("Various Artists")
                        .album("Hits"),
                ]),
        )
    }

    fn artist_id(library: &Library, name: &str) -> i64 {
        library
            .conn
            .query_row("SELECT id FROM artists WHERE name = ?1", [name], |row| {
                row.get(0)
            })
            .unwrap()
    }

    #[test]
    fn decides_among_artists_of_the_same_name() {
        let nirvana = [
            hit("us", "Nirvana", 100),
            hit("uk", "Nirvana", 75),
            hit("other", "Approaching Nirvana", 74),
        ];
        assert_eq!(decide("nirvana", &nirvana), Decision::Matched(&nirvana[0]));
        // Too close to tell.
        let close = [hit("a", "Nirvana", 100), hit("b", "Nirvana", 90)];
        assert_eq!(decide("Nirvana", &close), Decision::Review(&close[0]));
        // Not a strong enough hit.
        let weak = [hit("a", "Nírvana!", 80)];
        assert_eq!(decide("Nirvana", &weak), Decision::Review(&weak[0]));
        // Only other names.
        assert_eq!(decide("Nirvana", &nirvana[2..]), Decision::NoMatch);
        // An alias counts; "Various Artists" never does.
        let mut aliased = hit("x", "Simon & Garfunkel", 100);
        aliased.artist.aliases = vec!["Tom & Jerry".into()];
        let aliased = [aliased];
        assert_eq!(
            decide("Tom and Jerry", &aliased),
            Decision::Matched(&aliased[0])
        );
        let various = [hit(VARIOUS_ARTISTS, "Various Artists", 100)];
        assert_eq!(decide("Various Artists", &various), Decision::NoMatch);
    }

    #[test]
    fn matches_by_search_and_looks_the_artist_up() {
        let library = library();
        let radiohead = artist_id(&library, "Radiohead");
        let (client, transport, _clock) = fake_client();
        transport.push_status(
            &artist_search_url("Radiohead"),
            200,
            fixtures::ARTIST_SEARCH,
        );
        transport.push_status(&artist_url(fixtures::ARTIST), 200, fixtures::ARTIST_JSON);
        assert_eq!(
            match_artist(&client, &library.conn, radiohead).unwrap(),
            LinkStatus::Matched
        );
        let link = artist_link(&library.conn, radiohead, SourceId::MusicBrainz)
            .unwrap()
            .unwrap();
        assert_eq!(link.matched_id(), Some(fixtures::ARTIST));
        assert_eq!(link.score, 1.0);
        let artist: Artist = link.details().unwrap();
        assert_eq!(artist.wikidata.as_deref(), Some("Q44190"));

        let info = artist_info(&library.conn, radiohead).unwrap();
        assert_eq!(info.status, Some(LinkStatus::Matched));
        assert_eq!(info.musicbrainz, Some(artist));
        assert_eq!(info.biography, None);
        assert!(info.can_look_up);
    }

    #[test]
    fn trusts_the_tags_then_the_matched_releases() {
        let library = library();
        let radiohead = artist_id(&library, "Radiohead");
        let (client, transport, _clock) = fake_client();

        // The album is matched to a release credited to Radiohead alone.
        let (release_id, release_json) = fixtures::RELEASES[0];
        let release = musicbrainz::parse_release(release_json).unwrap();
        library
            .conn
            .execute(
                "INSERT INTO album_links
                     (album_id, source, status, external_id, score, chosen_by, details, checked_at)
                 VALUES (1, 'musicbrainz', 'matched', ?1, 1.0, 'auto', ?2, 0)",
                params![release_id, serde_json::to_string(&release).unwrap()],
            )
            .unwrap();
        assert_eq!(
            artist_from_releases(&library.conn, radiohead, "Radiohead")
                .unwrap()
                .as_deref(),
            Some(fixtures::ARTIST)
        );
        // Credited under another name.
        assert_eq!(
            artist_from_releases(&library.conn, radiohead, "Nirvana").unwrap(),
            None
        );
        transport.push_status(&artist_url(fixtures::ARTIST), 200, fixtures::ARTIST_JSON);
        assert_eq!(
            match_artist(&client, &library.conn, radiohead).unwrap(),
            LinkStatus::Matched
        );
        assert_eq!(
            transport.urls(),
            [artist_url(fixtures::ARTIST)],
            "no search"
        );
        let link = artist_link(&library.conn, radiohead, SourceId::MusicBrainz)
            .unwrap()
            .unwrap();
        assert_eq!(link.score, RELEASE_SCORE);

        // A tagged MBID comes first; one MusicBrainz no longer has is passed
        // over (the fake answers 404 to what isn't scripted).
        let gone = "00000000-0000-0000-0000-000000000001";
        library
            .conn
            .execute(
                "UPDATE artists SET musicbrainz_id = ?1 WHERE id = ?2",
                params![gone, radiohead],
            )
            .unwrap();
        library.conn.execute("DELETE FROM mb_cache", []).unwrap();
        let (client, transport, _clock) = fake_client();
        transport.push_status(&artist_url(fixtures::ARTIST), 200, fixtures::ARTIST_JSON);
        assert_eq!(
            match_artist(&client, &library.conn, radiohead).unwrap(),
            LinkStatus::Matched
        );
        assert_eq!(
            transport.urls(),
            [artist_url(gone), artist_url(fixtures::ARTIST)]
        );
    }

    #[test]
    fn records_doubt_and_nothing_found_but_never_various_artists() {
        let library = library();
        let nirvana = artist_id(&library, "Nirvana");
        let (client, transport, _clock) = fake_client();
        transport.push_status(
            &artist_search_url("Nirvana"),
            200,
            r#"{"artists": [
                {"id": "a", "name": "Nirvana", "score": 100},
                {"id": "b", "name": "Nirvana", "score": 95}
            ]}"#,
        );
        assert_eq!(
            match_artist(&client, &library.conn, nirvana).unwrap(),
            LinkStatus::Review
        );
        assert_eq!(transport.urls().len(), 1, "a candidate isn't looked up");
        let info = artist_info(&library.conn, nirvana).unwrap();
        assert_eq!(
            (info.status, info.musicbrainz),
            (Some(LinkStatus::Review), None)
        );

        library.conn.execute("DELETE FROM mb_cache", []).unwrap();
        let (client, transport, _clock) = fake_client();
        transport.push_status(&artist_search_url("Nirvana"), 200, r#"{"artists": []}"#);
        assert_eq!(
            match_artist(&client, &library.conn, nirvana).unwrap(),
            LinkStatus::None
        );

        let various = artist_id(&library, "Various Artists");
        let (client, transport, _clock) = fake_client();
        assert_eq!(
            match_artist(&client, &library.conn, various).unwrap(),
            LinkStatus::None
        );
        assert!(transport.urls().is_empty());
    }

    #[test]
    fn never_replaces_the_users_choice() {
        let library = library();
        let nirvana = artist_id(&library, "Nirvana");
        library
            .conn
            .execute(
                "INSERT INTO artist_links
                     (artist_id, source, status, external_id, score, chosen_by, details, checked_at)
                 VALUES (?1, 'musicbrainz', 'matched', 'chosen', 1.0, 'user', NULL, 0)",
                [nirvana],
            )
            .unwrap();
        let (client, transport, _clock) = fake_client();
        assert_eq!(
            match_artist(&client, &library.conn, nirvana).unwrap(),
            LinkStatus::Matched
        );
        assert!(transport.urls().is_empty());
        store_link(
            &library.conn,
            nirvana,
            SourceId::MusicBrainz,
            LinkStatus::None,
            None,
            0.0,
            None::<&Artist>,
        )
        .unwrap();
        let link = artist_link(&library.conn, nirvana, SourceId::MusicBrainz)
            .unwrap()
            .unwrap();
        assert_eq!(link.external_id.as_deref(), Some("chosen"));
        clear_link(&library.conn, nirvana, SourceId::MusicBrainz).unwrap();
        assert_eq!(
            artist_link(&library.conn, nirvana, SourceId::MusicBrainz).unwrap(),
            None
        );
    }

    #[test]
    fn offers_candidates_and_keeps_the_users_pick() {
        let library = library();
        let radiohead = artist_id(&library, "Radiohead");
        let (client, transport, _clock) = fake_client();
        transport.push_status(
            &artist_search_url("Radiohead"),
            200,
            fixtures::ARTIST_SEARCH,
        );
        let candidates = artist_candidates(&client, &library.conn, radiohead, None).unwrap();
        let found: Vec<(&str, u32)> = candidates
            .iter()
            .map(|c| (c.artist.name.as_str(), c.score))
            .collect();
        assert_eq!(
            found,
            [("Radiohead", 100), ("On a Friday", 60), ("radiohead 3", 51)]
        );

        // Picked by id (or URL), then kept over automatic results.
        transport.push_status(&artist_url(fixtures::ARTIST), 200, fixtures::ARTIST_JSON);
        let url = format!("https://musicbrainz.org/artist/{}", fixtures::ARTIST);
        let by_url = artist_candidates(&client, &library.conn, radiohead, Some(&url)).unwrap();
        assert_eq!(by_url.len(), 1);
        assert_eq!(by_url[0].artist.id, fixtures::ARTIST);
        choose_artist(&client, &library.conn, radiohead, fixtures::ARTIST).unwrap();
        let link = artist_link(&library.conn, radiohead, SourceId::MusicBrainz)
            .unwrap()
            .unwrap();
        assert!(link.chosen_by_user);
        assert_eq!(link.matched_id(), Some(fixtures::ARTIST));
        assert!(
            artist_info(&library.conn, radiohead)
                .unwrap()
                .chosen_by_user
        );
        let requests = transport.urls().len();
        assert_eq!(
            match_artist(&client, &library.conn, radiohead).unwrap(),
            LinkStatus::Matched
        );
        assert_eq!(transport.urls().len(), requests);

        // The current match is offered whatever is searched for.
        transport.push_status(
            &artist_search_url("Nobody at all"),
            200,
            r#"{"artists": []}"#,
        );
        let candidates =
            artist_candidates(&client, &library.conn, radiohead, Some("Nobody at all")).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].artist.id, fixtures::ARTIST);

        // "None of these" replaces it and is kept too.
        reject_artists(&library.conn, radiohead).unwrap();
        let link = artist_link(&library.conn, radiohead, SourceId::MusicBrainz)
            .unwrap()
            .unwrap();
        assert_eq!(
            (link.status, link.chosen_by_user, link.external_id),
            (LinkStatus::None, true, None)
        );
    }

    #[test]
    fn offline_leaves_the_artist_as_it_was() {
        let library = library();
        let nirvana = artist_id(&library, "Nirvana");
        let (client, transport, _clock) = fake_client();
        transport.push(
            &artist_search_url("Nirvana"),
            Err(crate::metadata::http::TransportError::Unreachable(
                "down".into(),
            )),
        );
        let error = match_artist(&client, &library.conn, nirvana).unwrap_err();
        assert!(matches!(error, Error::Offline(_)), "{error}");
        assert_eq!(
            artist_link(&library.conn, nirvana, SourceId::MusicBrainz).unwrap(),
            None
        );
    }

    #[test]
    fn hides_what_turned_off_sources_supplied() {
        let library = library();
        let radiohead = artist_id(&library, "Radiohead");
        let artist = musicbrainz::parse_artist(fixtures::ARTIST_JSON).unwrap();
        store_link(
            &library.conn,
            radiohead,
            SourceId::MusicBrainz,
            LinkStatus::Matched,
            Some(fixtures::ARTIST),
            1.0,
            Some(&artist),
        )
        .unwrap();
        let mut settings = settings::service_settings(&library.conn).unwrap();
        settings.online = false;
        settings::save_service_settings(&library.conn, settings.clone()).unwrap();
        let info = artist_info(&library.conn, radiohead).unwrap();
        assert!(info.musicbrainz.is_some(), "shown while offline");
        assert!(!info.can_look_up);
        settings.sources[2].enabled = false; // MusicBrainz.
        settings::save_service_settings(&library.conn, settings).unwrap();
        assert_eq!(
            artist_info(&library.conn, radiohead).unwrap().musicbrainz,
            None
        );
    }
}
