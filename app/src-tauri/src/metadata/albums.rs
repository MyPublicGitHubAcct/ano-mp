//! Matching library albums to MusicBrainz releases and keeping the result
//! in `album_links` (migration 003). An automatic match never replaces one
//! the user chose.

use rusqlite::{params, Connection, OptionalExtension};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::cache::unix_now;
use super::http::Client;
use super::matcher::{self, AlbumFacts, Candidate, Decision};
use super::musicbrainz::{self, Release};
use super::settings::SourceId;
use super::Error;

/// Releases looked up (for their track lists) per automatic match: the best
/// search hits by a first score without durations. Each costs a request, a
/// second each at MusicBrainz's rate.
const AUTO_LOOKUPS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LinkStatus {
    Matched,
    Review,
    None,
}

impl LinkStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            LinkStatus::Matched => "matched",
            LinkStatus::Review => "review",
            LinkStatus::None => "none",
        }
    }

    pub fn from_str(text: &str) -> Option<LinkStatus> {
        [LinkStatus::Matched, LinkStatus::Review, LinkStatus::None]
            .into_iter()
            .find(|status| status.as_str() == text)
    }
}

/// A release offered for an album in the "Find details" dialog, scored as
/// automatic matching scores it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseCandidate {
    pub release: Release,
    pub score: f64,
    /// Scored on its track lengths too; otherwise on the search result
    /// alone, which has no track list.
    pub full: bool,
}

/// An album's row in `album_links` for one source.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumLink {
    pub source: SourceId,
    pub status: LinkStatus,
    pub external_id: Option<String>,
    pub score: f64,
    pub chosen_by_user: bool,
    /// The release, for a MusicBrainz link.
    pub release: Option<Release>,
    pub checked_at: i64,
    /// The source's JSON, as stored: a `Release` for MusicBrainz, an
    /// `Article` for Wikipedia.
    #[serde(skip)]
    pub details: Option<String>,
}

impl AlbumLink {
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

/// What the library knows about album `album_id`, and the release MBID in
/// its tags if any; `None` if there is no such album.
pub fn album_facts(
    conn: &Connection,
    album_id: i64,
) -> Result<Option<(AlbumFacts, Option<String>)>, Error> {
    let album: Option<(String, Option<String>, Option<String>)> = conn
        .query_row(
            "SELECT al.title, ar.name, al.musicbrainz_release_id
             FROM albums al LEFT JOIN artists ar ON ar.id = al.artist_id
             WHERE al.id = ?1",
            [album_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let Some((title, artist, release_id)) = album else {
        return Ok(None);
    };
    let mut statement = conn.prepare_cached(
        "SELECT duration, year, IFNULL(disc_number, 1), track_total FROM tracks
         WHERE album_id = ?1 ORDER BY IFNULL(disc_number, 1), track_number NULLS LAST",
    )?;
    let rows = statement.query_map([album_id], |row| {
        Ok((
            row.get::<_, f64>(0)?,
            row.get::<_, Option<u32>>(1)?,
            row.get::<_, u32>(2)?,
            row.get::<_, Option<u32>>(3)?,
        ))
    })?;
    let mut durations = Vec::new();
    let mut year: Option<u32> = None;
    // Per disc, the largest tagged total.
    let mut totals: std::collections::BTreeMap<u32, u32> = Default::default();
    for row in rows {
        let (duration, track_year, disc, total) = row?;
        durations.push(duration);
        year = match (year, track_year) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        if let Some(total) = total {
            let entry = totals.entry(disc).or_default();
            *entry = (*entry).max(total);
        }
    }
    // Tagged totals say how long the album is even when some tracks are
    // missing, but never fewer than the tracks there are.
    let track_count = (totals.values().sum::<u32>()).max(durations.len() as u32);
    Ok(Some((
        AlbumFacts {
            title,
            artist,
            year,
            track_count,
            durations,
        },
        release_id,
    )))
}

/// MusicBrainz releases that could be `album`, best first: the search hits,
/// of which the best `lookups` by a first score are looked up for their
/// track lists and scored again with durations. Only those are returned.
pub fn candidates(
    client: &Client,
    conn: &Connection,
    album: &AlbumFacts,
    lookups: usize,
) -> Result<Vec<Candidate>, Error> {
    let hits = musicbrainz::search_releases(
        client,
        conn,
        &album.title,
        album.artist.as_deref(),
        Some(album.track_count),
    )?;
    let mut first: Vec<Candidate> = hits
        .into_iter()
        .map(|hit| Candidate {
            score: matcher::score(album, &hit.release),
            release: hit.release,
        })
        .collect();
    // Stable, so MusicBrainz's own order breaks ties.
    first.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut scored = Vec::new();
    for hit in first.into_iter().take(lookups) {
        match musicbrainz::lookup_release(client, conn, &hit.release.id) {
            Ok(release) => scored.push(Candidate {
                score: matcher::score(album, &release),
                release,
            }),
            // Gone since the search index was built.
            Err(Error::Status { status: 404, .. }) => {}
            Err(error) => return Err(error),
        }
    }
    scored.sort_by(|a, b| b.score.total_cmp(&a.score));
    Ok(scored)
}

/// Matches album `album_id` to a MusicBrainz release, unless the user chose
/// one, and stores the result. A release MBID in the tags is trusted as
/// is; otherwise releases are searched for and scored.
pub fn match_album(client: &Client, conn: &Connection, album_id: i64) -> Result<LinkStatus, Error> {
    if let Some(link) = album_link(conn, album_id, SourceId::MusicBrainz)? {
        if link.chosen_by_user {
            return Ok(link.status);
        }
    }
    let (album, tagged) = album_facts(conn, album_id)?
        .ok_or_else(|| Error::Invalid(format!("No album with id {album_id}")))?;
    if let Some(id) = tagged.filter(|id| musicbrainz::is_mbid(id)) {
        match musicbrainz::lookup_release(client, conn, &id) {
            Ok(release) => {
                store_link(
                    conn,
                    album_id,
                    LinkStatus::Matched,
                    Some(&release),
                    1.0,
                    false,
                )?;
                return Ok(LinkStatus::Matched);
            }
            // A tagged release that no longer exists (e.g. merged away):
            // search instead.
            Err(Error::Status { status: 404, .. }) => {}
            Err(error) => return Err(error),
        }
    }
    let (status, candidate) =
        match matcher::decide(&mut candidates(client, conn, &album, AUTO_LOOKUPS)?) {
            Decision::Matched(candidate) => (LinkStatus::Matched, Some(candidate)),
            Decision::Review(candidate) => (LinkStatus::Review, Some(candidate)),
            Decision::NoMatch => (LinkStatus::None, None),
        };
    store_link(
        conn,
        album_id,
        status,
        candidate.as_ref().map(|c| &c.release),
        candidate.as_ref().map_or(0.0, |c| c.score),
        false,
    )?;
    Ok(status)
}

/// The MusicBrainz releases the user can pick for album `album_id`, best
/// first: the hits of a search for `search` (title, artist; the album's
/// own when `None`, which is the automatic match's search and so usually
/// cached), of which the best `AUTO_LOOKUPS` are looked up and scored with
/// their track lengths; and the album's current match or review candidate.
/// A release MBID or MusicBrainz release URL as the title is looked up
/// instead of searched for.
pub fn release_candidates(
    client: &Client,
    conn: &Connection,
    album_id: i64,
    search: Option<(&str, Option<&str>)>,
) -> Result<Vec<ReleaseCandidate>, Error> {
    let (album, _) = album_facts(conn, album_id)?
        .ok_or_else(|| Error::Invalid(format!("No album with id {album_id}")))?;
    let full = |release: Release| ReleaseCandidate {
        score: matcher::score(&album, &release),
        full: !release.tracks.is_empty(),
        release,
    };
    let (title, artist) = search.unwrap_or((&album.title, album.artist.as_deref()));
    let mut candidates = Vec::new();
    if let Some(id) = musicbrainz::mbid_in(title, "release") {
        match musicbrainz::lookup_release(client, conn, id) {
            Ok(release) => candidates.push(full(release)),
            Err(Error::Status { status: 404, .. }) => {
                return Err(Error::Invalid(format!("MusicBrainz has no release {id}")))
            }
            Err(error) => return Err(error),
        }
    } else {
        let mut hits: Vec<ReleaseCandidate> =
            musicbrainz::search_releases(client, conn, title, artist, Some(album.track_count))?
                .into_iter()
                .map(|hit| ReleaseCandidate {
                    score: matcher::score(&album, &hit.release),
                    full: false,
                    release: hit.release,
                })
                .collect();
        // Stable, so MusicBrainz's own order breaks ties.
        hits.sort_by(|a, b| b.score.total_cmp(&a.score));
        for (index, hit) in hits.into_iter().enumerate() {
            if index >= AUTO_LOOKUPS {
                candidates.push(hit);
                continue;
            }
            match musicbrainz::lookup_release(client, conn, &hit.release.id) {
                Ok(release) => candidates.push(full(release)),
                // Gone since the search index was built.
                Err(Error::Status { status: 404, .. }) => {}
                Err(error) => return Err(error),
            }
        }
    }
    let current = album_link(conn, album_id, SourceId::MusicBrainz)?.and_then(|link| link.release);
    if let Some(release) = current {
        if !candidates.iter().any(|c| c.release.id == release.id) {
            candidates.push(full(release));
        }
    }
    candidates.sort_by(|a, b| b.score.total_cmp(&a.score));
    Ok(candidates)
}

/// Links album `album_id` to the release the user picked, which automatic
/// matching then leaves alone.
pub fn choose_release(
    client: &Client,
    conn: &Connection,
    album_id: i64,
    release_id: &str,
) -> Result<AlbumLink, Error> {
    let release = musicbrainz::lookup_release(client, conn, release_id)?;
    store_link(
        conn,
        album_id,
        LinkStatus::Matched,
        Some(&release),
        1.0,
        true,
    )?;
    Ok(album_link(conn, album_id, SourceId::MusicBrainz)?.expect("just stored"))
}

/// Records that none of MusicBrainz's releases is album `album_id` ("None
/// of these"), which automatic matching then leaves alone.
pub fn reject_releases(conn: &Connection, album_id: i64) -> Result<(), Error> {
    store_link(conn, album_id, LinkStatus::None, None, 0.0, true)
}

/// Forgets album `album_id`'s link to `source`, chosen or not, so the next
/// automatic run matches it afresh.
pub fn clear_link(conn: &Connection, album_id: i64, source: SourceId) -> Result<(), Error> {
    conn.execute(
        "DELETE FROM album_links WHERE album_id = ?1 AND source = ?2",
        params![album_id, source.as_str()],
    )?;
    Ok(())
}

fn store_link(
    conn: &Connection,
    album_id: i64,
    status: LinkStatus,
    release: Option<&Release>,
    score: f64,
    by_user: bool,
) -> Result<(), Error> {
    store(
        conn,
        album_id,
        SourceId::MusicBrainz,
        status,
        release.map(|release| release.id.as_str()),
        score,
        release,
        by_user,
    )
}

/// Stores what `source` has for album `album_id`, as an automatic result,
/// which never replaces the user's.
pub fn store_source_link(
    conn: &Connection,
    album_id: i64,
    source: SourceId,
    status: LinkStatus,
    external_id: Option<&str>,
    score: f64,
    details: Option<&impl Serialize>,
) -> Result<(), Error> {
    store(
        conn,
        album_id,
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
    album_id: i64,
    source: SourceId,
    status: LinkStatus,
    external_id: Option<&str>,
    score: f64,
    details: Option<&impl Serialize>,
    by_user: bool,
) -> Result<(), Error> {
    let details = details.map(|details| serde_json::to_string(details).expect("details serialize"));
    // An automatic result never replaces the user's.
    conn.prepare_cached(
        "INSERT INTO album_links
             (album_id, source, status, external_id, score, chosen_by, details, checked_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT (album_id, source) DO UPDATE SET
             status = excluded.status, external_id = excluded.external_id,
             score = excluded.score, chosen_by = excluded.chosen_by,
             details = excluded.details, checked_at = excluded.checked_at
         WHERE album_links.chosen_by = 'auto' OR excluded.chosen_by = 'user'",
    )?
    .execute(params![
        album_id,
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

/// The MusicBrainz release group of the release album `album_id` is matched
/// to (not one that only awaits review): the album across all its releases.
pub fn matched_release_group(conn: &Connection, album_id: i64) -> Result<Option<String>, Error> {
    Ok(album_link(conn, album_id, SourceId::MusicBrainz)?
        .filter(|link| link.matched_id().is_some())
        .and_then(|link| link.release?.release_group_id)
        .filter(|id| musicbrainz::is_mbid(id)))
}

pub fn album_link(
    conn: &Connection,
    album_id: i64,
    source: SourceId,
) -> Result<Option<AlbumLink>, Error> {
    let row: Option<(String, Option<String>, f64, String, Option<String>, i64)> = conn
        .query_row(
            "SELECT status, external_id, score, chosen_by, details, checked_at
             FROM album_links WHERE album_id = ?1 AND source = ?2",
            params![album_id, source.as_str()],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .optional()?;
    Ok(row.map(
        |(status, external_id, score, chosen_by, details, checked_at)| AlbumLink {
            source,
            status: LinkStatus::from_str(&status).unwrap_or(LinkStatus::None),
            external_id,
            score,
            chosen_by_user: chosen_by == "user",
            // Details stored by another version may not parse; they are
            // fetched again then.
            release: match source {
                SourceId::MusicBrainz => details
                    .as_deref()
                    .and_then(|json| serde_json::from_str(json).ok()),
                _ => None,
            },
            checked_at,
            details,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::test_library::{track, Library};
    use crate::metadata::http::testing::{fake_client, FakeTransport};
    use crate::metadata::musicbrainz::{fixtures, release_url, search_url};

    /// The durations of the 2016 release, plus a little encoder padding.
    const DURATIONS: [f64; 10] = [
        237.4, 242.9, 255.6, 318.5, 228.3, 129.8, 290.5, 328.9, 248.6, 279.2,
    ];

    /// A library with "In Rainbows" by Radiohead, ten tracks tagged with
    /// their durations and the given release MBID.
    fn library(release_id: Option<&str>) -> (Library, i64) {
        let library = Library::new((1..=10).map(|n| {
            track(&format!("Radiohead/In Rainbows/{n:02}.flac"))
                .artist("Radiohead")
                .album("In Rainbows")
                .year(2007)
                .number(n)
        }));
        for (n, duration) in DURATIONS.iter().enumerate() {
            library
                .conn
                .execute(
                    "UPDATE tracks SET duration = ?1, track_total = 10 WHERE track_number = ?2",
                    params![duration, n as u32 + 1],
                )
                .unwrap();
        }
        library
            .conn
            .execute(
                "UPDATE albums SET musicbrainz_release_id = ?1",
                [release_id],
            )
            .unwrap();
        (library, 1)
    }

    fn serve_search(transport: &FakeTransport) {
        transport.push_status(
            &search_url("In Rainbows", Some("Radiohead"), Some(10)),
            200,
            fixtures::SEARCH,
        );
        for (id, json) in fixtures::RELEASES {
            transport.push_status(&release_url(id), 200, json);
        }
    }

    #[test]
    fn gathers_album_facts() {
        let (library, album) = library(None);
        library
            .conn
            .execute("UPDATE tracks SET year = 2008 WHERE track_number = 3", [])
            .unwrap();
        let (facts, tagged) = album_facts(&library.conn, album).unwrap().unwrap();
        assert_eq!(facts.title, "In Rainbows");
        assert_eq!(facts.artist.as_deref(), Some("Radiohead"));
        assert_eq!(facts.year, Some(2007));
        assert_eq!(facts.track_count, 10);
        assert_eq!(facts.durations, DURATIONS);
        assert_eq!(tagged, None);
        assert!(album_facts(&library.conn, 99).unwrap().is_none());

        // Some tracks missing: the tagged total still counts.
        library
            .conn
            .execute("DELETE FROM tracks WHERE track_number > 4", [])
            .unwrap();
        let (facts, _) = album_facts(&library.conn, album).unwrap().unwrap();
        assert_eq!((facts.track_count, facts.durations.len()), (10, 4));
    }

    #[test]
    fn matches_by_search_and_track_lengths() {
        let (library, album) = library(None);
        let (client, transport, _clock) = fake_client();
        serve_search(&transport);
        assert_eq!(
            match_album(&client, &library.conn, album).unwrap(),
            LinkStatus::Matched
        );
        // One search and three lookups.
        assert_eq!(transport.urls().len(), 1 + AUTO_LOOKUPS);
        let link = album_link(&library.conn, album, SourceId::MusicBrainz)
            .unwrap()
            .unwrap();
        assert!(link.score >= matcher::ACCEPT_SCORE, "{}", link.score);
        assert!(!link.chosen_by_user);
        let release = link.release.unwrap();
        assert_eq!(Some(release.id), link.external_id);
        assert_eq!(
            release.release_group_id.as_deref(),
            Some(fixtures::RELEASE_GROUP)
        );
    }

    #[test]
    fn trusts_a_tagged_release_and_falls_back_when_it_is_gone() {
        let (id, json) = fixtures::RELEASES[3];
        let (library, album) = library(Some(id));
        let (client, transport, _clock) = fake_client();
        transport.push_status(&release_url(id), 200, json);
        assert_eq!(
            match_album(&client, &library.conn, album).unwrap(),
            LinkStatus::Matched
        );
        assert_eq!(transport.urls(), [release_url(id)]);
        let link = album_link(&library.conn, album, SourceId::MusicBrainz)
            .unwrap()
            .unwrap();
        assert_eq!((link.external_id.as_deref(), link.score), (Some(id), 1.0));

        // A tagged release MusicBrainz no longer has: a search follows. (The
        // same release is among the search's hits, and its lookup then gets
        // the fixture, queued after the 404.)
        library.conn.execute("DELETE FROM mb_cache", []).unwrap();
        let (client, transport, _clock) = fake_client();
        transport.push_status(&release_url(id), 404, "");
        serve_search(&transport);
        assert_eq!(
            match_album(&client, &library.conn, album).unwrap(),
            LinkStatus::Matched
        );
        assert_eq!(
            transport.urls()[..2],
            [
                release_url(id),
                search_url("In Rainbows", Some("Radiohead"), Some(10))
            ]
        );
    }

    #[test]
    fn a_doubtful_match_waits_for_review_and_nothing_found_is_recorded() {
        let (library, album) = library(None);
        library
            .conn
            .execute("UPDATE tracks SET duration = 60.0", [])
            .unwrap();
        let (client, transport, _clock) = fake_client();
        serve_search(&transport);
        assert_eq!(
            match_album(&client, &library.conn, album).unwrap(),
            LinkStatus::Review
        );
        let link = album_link(&library.conn, album, SourceId::MusicBrainz)
            .unwrap()
            .unwrap();
        assert!(link.external_id.is_some());

        let (client, transport, _clock) = fake_client();
        library.conn.execute("DELETE FROM mb_cache", []).unwrap();
        transport.push_status(
            &search_url("In Rainbows", Some("Radiohead"), Some(10)),
            200,
            r#"{"releases": []}"#,
        );
        assert_eq!(
            match_album(&client, &library.conn, album).unwrap(),
            LinkStatus::None
        );
        let link = album_link(&library.conn, album, SourceId::MusicBrainz)
            .unwrap()
            .unwrap();
        assert_eq!(
            (link.external_id, link.release, link.score),
            (None, None, 0.0)
        );
    }

    #[test]
    fn automatic_matching_never_replaces_the_users_choice() {
        let (library, album) = library(None);
        let (client, transport, _clock) = fake_client();
        serve_search(&transport);
        let (chosen, _) = fixtures::RELEASES[2];
        let link = choose_release(&client, &library.conn, album, chosen).unwrap();
        assert!(link.chosen_by_user);
        assert_eq!(link.external_id.as_deref(), Some(chosen));

        let requests = transport.urls().len();
        assert_eq!(
            match_album(&client, &library.conn, album).unwrap(),
            LinkStatus::Matched
        );
        assert_eq!(transport.urls().len(), requests, "nothing was looked up");
        store_link(&library.conn, album, LinkStatus::None, None, 0.0, false).unwrap();
        let link = album_link(&library.conn, album, SourceId::MusicBrainz)
            .unwrap()
            .unwrap();
        assert_eq!(link.external_id.as_deref(), Some(chosen));

        clear_link(&library.conn, album, SourceId::MusicBrainz).unwrap();
        assert_eq!(
            album_link(&library.conn, album, SourceId::MusicBrainz).unwrap(),
            None
        );
    }

    #[test]
    fn offers_the_search_hits_and_the_current_match_as_candidates() {
        let (library, album) = library(None);
        let (client, transport, _clock) = fake_client();
        serve_search(&transport);
        let candidates = release_candidates(&client, &library.conn, album, None).unwrap();
        // Every hit; the best three looked up for their track lists.
        assert_eq!(candidates.len(), 5);
        assert_eq!(candidates.iter().filter(|c| c.full).count(), AUTO_LOOKUPS);
        assert!(candidates
            .windows(2)
            .all(|pair| pair[0].score >= pair[1].score));
        assert!(candidates[0].full && candidates[0].score >= matcher::ACCEPT_SCORE);
        let searched = transport.urls().len();

        // The automatic match searches the same, so it's cached.
        match_album(&client, &library.conn, album).unwrap();
        assert_eq!(transport.urls().len(), searched);

        // Another search: the current match is offered too.
        let (other_title, other_artist) = ("Kid A", Some("Radiohead"));
        transport.push_status(
            &search_url(other_title, other_artist, Some(10)),
            200,
            r#"{"releases": []}"#,
        );
        let candidates = release_candidates(
            &client,
            &library.conn,
            album,
            Some((other_title, other_artist)),
        )
        .unwrap();
        let current = album_link(&library.conn, album, SourceId::MusicBrainz)
            .unwrap()
            .unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            Some(&candidates[0].release.id),
            current.external_id.as_ref()
        );
    }

    #[test]
    fn looks_up_a_release_by_id_or_url() {
        let (library, album) = library(None);
        let (client, transport, _clock) = fake_client();
        let (id, json) = fixtures::RELEASES[2];
        transport.push_status(&release_url(id), 200, json);
        for query in [
            id.to_owned(),
            format!(" https://musicbrainz.org/release/{id}/discids "),
        ] {
            let candidates =
                release_candidates(&client, &library.conn, album, Some((&query, None))).unwrap();
            assert_eq!(candidates.len(), 1, "{query}");
            assert_eq!(candidates[0].release.id, id);
            assert!(candidates[0].full);
        }
        assert_eq!(transport.urls(), [release_url(id)], "then cached");

        let missing = "00000000-0000-0000-0000-000000000000";
        transport.push_status(&release_url(missing), 404, "");
        let error =
            release_candidates(&client, &library.conn, album, Some((missing, None))).unwrap_err();
        assert!(error.to_string().contains("has no release"), "{error}");
    }

    #[test]
    fn none_of_these_is_kept_like_a_choice() {
        let (library, album) = library(None);
        let (client, transport, _clock) = fake_client();
        reject_releases(&library.conn, album).unwrap();
        let link = album_link(&library.conn, album, SourceId::MusicBrainz)
            .unwrap()
            .unwrap();
        assert_eq!(
            (link.status, link.chosen_by_user, link.external_id),
            (LinkStatus::None, true, None)
        );
        assert_eq!(
            match_album(&client, &library.conn, album).unwrap(),
            LinkStatus::None
        );
        assert!(transport.urls().is_empty(), "nothing was looked up");

        // Choosing a release replaces it; clearing it goes back to automatic.
        let (id, json) = fixtures::RELEASES[0];
        transport.push_status(&release_url(id), 200, json);
        choose_release(&client, &library.conn, album, id).unwrap();
        reject_releases(&library.conn, album).unwrap();
        let link = album_link(&library.conn, album, SourceId::MusicBrainz)
            .unwrap()
            .unwrap();
        assert_eq!(link.status, LinkStatus::None);
    }

    #[test]
    fn offline_leaves_the_album_as_it_was() {
        let (library, album) = library(None);
        let (client, transport, _clock) = fake_client();
        transport.push(
            &search_url("In Rainbows", Some("Radiohead"), Some(10)),
            Err(crate::metadata::http::TransportError::Unreachable(
                "down".into(),
            )),
        );
        let error = match_album(&client, &library.conn, album).unwrap_err();
        assert!(matches!(error, Error::Offline(_)), "{error}");
        assert_eq!(
            album_link(&library.conn, album, SourceId::MusicBrainz).unwrap(),
            None
        );
    }
}
