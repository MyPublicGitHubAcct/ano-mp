//! Matching library albums to releases at the album-details sources
//! (MusicBrainz, Discogs) and keeping the result in `album_links` (migration
//! 003). An automatic match never replaces one the user chose.
//!
//! Each source is a `ReleaseSource`: search, lookup, and a release known
//! without searching. Matching, the "Find details" dialog's candidates and
//! the user's picks work the same for every source through it. A source
//! that `stores_details` keeps the release with its link; another (Discogs,
//! whose terms forbid keeping its data) keeps only the id, and its release
//! is looked up when shown (`linked_release`).

use rusqlite::{params, Connection, OptionalExtension};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::cache::unix_now;
use super::discogs::{self, DiscogsReleases};
use super::http::Client;
use super::matcher::{self, AlbumFacts, Candidate, Decision};
use super::musicbrainz::{self, Release};
use super::settings::{self, Kind, SourceId};
use super::{keys, Error};

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

/// A source of album details (`Kind::Release`): a catalogue of releases
/// that albums are matched against.
pub trait ReleaseSource {
    fn id(&self) -> SourceId;

    /// Releases called `title` by `artist`, in the source's own order,
    /// without their track lists (`track_count` is 0 when unknown).
    /// `track_count` is the album's, a hint for the source's ranking.
    fn search(
        &self,
        title: &str,
        artist: Option<&str>,
        track_count: u32,
    ) -> Result<Vec<Release>, Error>;

    /// Release `id` with its track list; `Error::Status` 404 if it's gone.
    fn lookup(&self, id: &str) -> Result<Release, Error>;

    /// The release `text` names, if it's one of this source's release links
    /// or ids, pasted into the dialog's search.
    fn id_in(&self, text: &str) -> Option<String>;

    /// A release known to be album `album_id` without searching, and how
    /// sure that is (0 to 1): a release MBID in the tags, or the Discogs
    /// release that the album's MusicBrainz release links to.
    fn known_release(&self, album_id: i64) -> Result<Option<(String, f64)>, Error>;
}

/// MusicBrainz, whose release is kept with the link.
pub struct MusicBrainzReleases<'a> {
    pub client: &'a Client,
    pub conn: &'a Connection,
}

impl ReleaseSource for MusicBrainzReleases<'_> {
    fn id(&self) -> SourceId {
        SourceId::MusicBrainz
    }

    fn search(
        &self,
        title: &str,
        artist: Option<&str>,
        track_count: u32,
    ) -> Result<Vec<Release>, Error> {
        let hits =
            musicbrainz::search_releases(self.client, self.conn, title, artist, Some(track_count))?;
        Ok(hits.into_iter().map(|hit| hit.release).collect())
    }

    fn lookup(&self, id: &str) -> Result<Release, Error> {
        musicbrainz::lookup_release(self.client, self.conn, id)
    }

    fn id_in(&self, text: &str) -> Option<String> {
        musicbrainz::mbid_in(text, "release").map(str::to_owned)
    }

    fn known_release(&self, album_id: i64) -> Result<Option<(String, f64)>, Error> {
        let tagged: Option<Option<String>> = self
            .conn
            .query_row(
                "SELECT musicbrainz_release_id FROM albums WHERE id = ?1",
                [album_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(tagged
            .flatten()
            .filter(|id| musicbrainz::is_mbid(id))
            .map(|id| (id, 1.0)))
    }
}

/// The album-details source `source`, over `client` and `conn`. Discogs
/// needs its token from the keychain.
pub fn release_source<'a>(
    source: SourceId,
    client: &'a Client,
    conn: &'a Connection,
) -> Result<Box<dyn ReleaseSource + 'a>, Error> {
    match source {
        SourceId::MusicBrainz => Ok(Box::new(MusicBrainzReleases { client, conn })),
        SourceId::Discogs => {
            let token = keys::get(source)?.ok_or_else(|| {
                Error::Invalid(
                    "Discogs needs a personal access token: add it in Online sources".into(),
                )
            })?;
            let musicbrainz = settings::service_settings(conn)?
                .sources_for(Kind::Release)
                .contains(&SourceId::MusicBrainz);
            Ok(Box::new(DiscogsReleases {
                client,
                conn,
                token,
                musicbrainz,
            }))
        }
        other => Err(Error::Invalid(format!(
            "{} doesn't supply album details",
            other.info().name
        ))),
    }
}

/// The page on the source's site that shows release `id`, for links and
/// credits.
pub fn release_page(source: SourceId, id: &str) -> Option<String> {
    match source {
        SourceId::MusicBrainz => Some(format!("https://musicbrainz.org/release/{id}")),
        SourceId::Discogs => Some(discogs::page_url(id)),
        _ => None,
    }
}

/// The page on the source's site listing its releases found for `query`.
pub fn search_page(source: SourceId, query: &str) -> Option<String> {
    let query = super::http::percent_encode(query);
    match source {
        SourceId::MusicBrainz => Some(format!(
            "https://musicbrainz.org/search?type=release&query={query}"
        )),
        SourceId::Discogs => Some(format!(
            "https://www.discogs.com/search/?type=release&q={query}"
        )),
        _ => None,
    }
}

/// Whether `text` is a release link or id of a source other than `source`,
/// which `source` then doesn't search for.
fn names_another_release(source: SourceId, text: &str) -> bool {
    let named = if musicbrainz::mbid_in(text, "release").is_some() {
        Some(SourceId::MusicBrainz)
    } else if discogs::release_id_in(text).is_some() {
        Some(SourceId::Discogs)
    } else {
        None
    };
    named.is_some_and(|named| named != source)
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
    /// The release's page at its source.
    pub page_url: Option<String>,
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

/// Releases at `source` that could be `album`, best first: the search hits,
/// of which the best `lookups` by a first score are looked up for their
/// track lists and scored again with durations. Only those are returned.
pub fn candidates(
    source: &dyn ReleaseSource,
    album: &AlbumFacts,
    lookups: usize,
) -> Result<Vec<Candidate>, Error> {
    let hits = source.search(&album.title, album.artist.as_deref(), album.track_count)?;
    let mut first: Vec<Candidate> = hits
        .into_iter()
        .map(|release| Candidate {
            score: matcher::score(album, &release),
            release,
        })
        .collect();
    // Stable, so the source's own order breaks ties.
    first.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut scored = Vec::new();
    for hit in first.into_iter().take(lookups) {
        match source.lookup(&hit.release.id) {
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

/// Matches album `album_id` to a MusicBrainz release; see `match_album_at`.
pub fn match_album(client: &Client, conn: &Connection, album_id: i64) -> Result<LinkStatus, Error> {
    match_album_at(&MusicBrainzReleases { client, conn }, conn, album_id)
}

/// Matches album `album_id` to a release at `source`, unless the user chose
/// one, and stores the result. A known release (a tagged MBID) is trusted
/// as is; otherwise releases are searched for and scored.
pub fn match_album_at(
    source: &dyn ReleaseSource,
    conn: &Connection,
    album_id: i64,
) -> Result<LinkStatus, Error> {
    if let Some(link) = album_link(conn, album_id, source.id())? {
        if link.chosen_by_user {
            return Ok(link.status);
        }
    }
    let (album, _) = album_facts(conn, album_id)?
        .ok_or_else(|| Error::Invalid(format!("No album with id {album_id}")))?;
    if let Some((id, score)) = source.known_release(album_id)? {
        // Only the id is kept, so there is nothing to look up.
        if !source.id().info().stores_details {
            store(
                conn,
                album_id,
                source.id(),
                LinkStatus::Matched,
                Some(&id),
                score,
                None,
                false,
            )?;
            return Ok(LinkStatus::Matched);
        }
        match source.lookup(&id) {
            Ok(release) => {
                store_release(
                    conn,
                    source.id(),
                    album_id,
                    LinkStatus::Matched,
                    Some(&release),
                    score,
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
    let (status, candidate) = match matcher::decide(&mut candidates(source, &album, AUTO_LOOKUPS)?)
    {
        Decision::Matched(candidate) => (LinkStatus::Matched, Some(candidate)),
        Decision::Review(candidate) => (LinkStatus::Review, Some(candidate)),
        Decision::NoMatch => (LinkStatus::None, None),
    };
    store_release(
        conn,
        source.id(),
        album_id,
        status,
        candidate.as_ref().map(|c| &c.release),
        candidate.as_ref().map_or(0.0, |c| c.score),
        false,
    )?;
    Ok(status)
}

/// The MusicBrainz releases the user can pick; see `release_candidates_at`.
#[cfg(test)]
pub fn release_candidates(
    client: &Client,
    conn: &Connection,
    album_id: i64,
    search: Option<(&str, Option<&str>)>,
) -> Result<Vec<ReleaseCandidate>, Error> {
    release_candidates_at(
        &MusicBrainzReleases { client, conn },
        conn,
        album_id,
        search,
    )
}

/// The releases at `source` the user can pick for album `album_id`, best
/// first: the hits of a search for `search` (title, artist; the album's
/// own when `None`, which is the automatic match's search and so usually
/// cached), of which the best `AUTO_LOOKUPS` are looked up and scored with
/// their track lengths; and the album's current match or review candidate.
/// A release link or id of the source as the title is looked up instead of
/// searched for; another source's finds nothing.
pub fn release_candidates_at(
    source: &dyn ReleaseSource,
    conn: &Connection,
    album_id: i64,
    search: Option<(&str, Option<&str>)>,
) -> Result<Vec<ReleaseCandidate>, Error> {
    let (album, _) = album_facts(conn, album_id)?
        .ok_or_else(|| Error::Invalid(format!("No album with id {album_id}")))?;
    let candidate = |release: Release, full: bool| ReleaseCandidate {
        score: matcher::score(&album, &release),
        full,
        page_url: release_page(source.id(), &release.id),
        release,
    };
    let (title, artist) = search.unwrap_or((&album.title, album.artist.as_deref()));
    let mut candidates = Vec::new();
    if let Some(id) = source.id_in(title) {
        match source.lookup(&id) {
            Ok(release) => candidates.push(candidate(release, true)),
            Err(Error::Status { status: 404, .. }) => {
                return Err(Error::Invalid(format!(
                    "{} has no release {id}",
                    source.id().info().name
                )))
            }
            Err(error) => return Err(error),
        }
    } else if !names_another_release(source.id(), title) {
        let mut hits: Vec<ReleaseCandidate> = source
            .search(title, artist, album.track_count)?
            .into_iter()
            .map(|release| candidate(release, false))
            .collect();
        // Stable, so the source's own order breaks ties.
        hits.sort_by(|a, b| b.score.total_cmp(&a.score));
        for (index, hit) in hits.into_iter().enumerate() {
            if index >= AUTO_LOOKUPS {
                candidates.push(hit);
                continue;
            }
            match source.lookup(&hit.release.id) {
                Ok(release) => candidates.push(candidate(release, true)),
                // Gone since the search index was built.
                Err(Error::Status { status: 404, .. }) => {}
                Err(error) => return Err(error),
            }
        }
    }
    if let Some(link) = album_link(conn, album_id, source.id())? {
        let offered = |id: &str| candidates.iter().any(|c| c.release.id == id);
        if link.external_id.as_deref().is_some_and(|id| !offered(id)) {
            // Gone or offline: the dialog still says what the link is.
            if let Ok(Some(release)) = linked_release(source, &link) {
                candidates.push(candidate(release, true));
            }
        }
    }
    candidates.sort_by(|a, b| b.score.total_cmp(&a.score));
    Ok(candidates)
}

/// The release album link `link` is to: stored with it, or looked up at
/// `source` if the source doesn't keep its releases.
pub fn linked_release(
    source: &dyn ReleaseSource,
    link: &AlbumLink,
) -> Result<Option<Release>, Error> {
    if link.release.is_some() || source.id().info().stores_details {
        return Ok(link.release.clone());
    }
    match &link.external_id {
        Some(id) => source.lookup(id).map(Some),
        None => Ok(None),
    }
}

/// Links album `album_id` to the MusicBrainz release the user picked; see
/// `choose_release_at`.
#[cfg(test)]
pub fn choose_release(
    client: &Client,
    conn: &Connection,
    album_id: i64,
    release_id: &str,
) -> Result<AlbumLink, Error> {
    choose_release_at(
        &MusicBrainzReleases { client, conn },
        conn,
        album_id,
        release_id,
    )
}

/// Links album `album_id` to the release at `source` the user picked,
/// which automatic matching then leaves alone.
pub fn choose_release_at(
    source: &dyn ReleaseSource,
    conn: &Connection,
    album_id: i64,
    release_id: &str,
) -> Result<AlbumLink, Error> {
    let release = source.lookup(release_id)?;
    store_release(
        conn,
        source.id(),
        album_id,
        LinkStatus::Matched,
        Some(&release),
        1.0,
        true,
    )?;
    Ok(album_link(conn, album_id, source.id())?.expect("just stored"))
}

/// Records that none of `source`'s releases is album `album_id` ("None of
/// these"), which automatic matching then leaves alone.
pub fn reject_releases(conn: &Connection, album_id: i64, source: SourceId) -> Result<(), Error> {
    store_release(conn, source, album_id, LinkStatus::None, None, 0.0, true)
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

/// Stores album `album_id`'s link to `release` at `source`: with the
/// release if the source keeps its details, else the id alone.
fn store_release(
    conn: &Connection,
    source: SourceId,
    album_id: i64,
    status: LinkStatus,
    release: Option<&Release>,
    score: f64,
    by_user: bool,
) -> Result<(), Error> {
    let details = release
        .filter(|_| source.info().stores_details)
        .map(|release| serde_json::to_string(release).expect("details serialize"));
    store(
        conn,
        album_id,
        source,
        status,
        release.map(|release| release.id.as_str()),
        score,
        details,
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
    let details = details.map(|details| serde_json::to_string(details).expect("details serialize"));
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
    details: Option<String>,
    by_user: bool,
) -> Result<(), Error> {
    let before = album_link(conn, album_id, source)?.map(|link| (link.status, link.external_id));
    // An automatic result never replaces the user's.
    let changed = conn
        .prepare_cached(
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
    // The Discogs release an automatic match took from the MusicBrainz
    // release's links belongs to that release: another match means matching
    // on Discogs again.
    let after = Some((status, external_id.map(str::to_owned)));
    if source == SourceId::MusicBrainz && changed > 0 && before != after {
        conn.execute(
            "DELETE FROM album_links WHERE album_id = ?1 AND source = ?2 AND chosen_by = 'auto'",
            params![album_id, SourceId::Discogs.as_str()],
        )?;
    }
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
        store_release(
            &library.conn,
            SourceId::MusicBrainz,
            album,
            LinkStatus::None,
            None,
            0.0,
            false,
        )
        .unwrap();
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
        reject_releases(&library.conn, album, SourceId::MusicBrainz).unwrap();
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
        reject_releases(&library.conn, album, SourceId::MusicBrainz).unwrap();
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
