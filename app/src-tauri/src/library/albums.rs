//! An album's details as the album header and the "Find details" dialog show
//! them: what its tags say, what the metadata sources know about it (its
//! match, and a description), and where its cover comes from, each
//! labelled with its source.

use rusqlite::OptionalExtension;
use serde::Serialize;

use super::art::{self, ArtKey};
use super::commands::LibraryState;
use super::{genres, Error};
use crate::library::sort_key::fold;
use crate::metadata::albums::{self, AlbumLink};
use crate::metadata::artists::SourcedArticle;
use crate::metadata::settings::{self, Kind, SourceId};
use crate::metadata::wikipedia;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumDetails {
    pub id: i64,
    pub title: String,
    pub album_artist: Option<String>,
    pub album_artist_id: Option<i64>,
    /// From the tags: the earliest year among its tracks, its genres, and
    /// the release MBID if tagged.
    pub year: Option<u32>,
    pub genres: Vec<String>,
    pub tagged_release_id: Option<String>,
    /// In disc and track order, for comparing with a release's.
    pub tracks: Vec<AlbumTrack>,
    /// Seconds.
    pub duration: f64,
    /// Its link to each album-details source that is shown, in the
    /// configured order.
    pub links: Vec<SourcedLink>,
    /// The first description found among the album-description sources
    /// shown.
    pub description: Option<SourcedArticle>,
    pub cover: Option<CoverSource>,
    /// Whether a details source can be searched now.
    pub can_look_up: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumTrack {
    pub id: i64,
    pub disc: Option<u32>,
    pub number: Option<u32>,
    /// The title, else the file name.
    pub title: String,
    pub duration: f64,
    /// Classical works (O6): the work it's a movement of, and the movement.
    pub work: Option<String>,
    pub movement_name: Option<String>,
    pub movement_number: Option<u32>,
    pub composer: Option<String>,
    pub conductor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcedLink {
    pub source_name: &'static str,
    /// Whether the release is kept with the link (`link.release`); if not
    /// (Discogs), fetch it with `metadata_release_details` when shown.
    pub stores_details: bool,
    /// The linked release's page at the source.
    pub page_url: Option<String>,
    /// What the source's terms want shown next to its data, linked to
    /// `page_url`.
    pub credit: Option<&'static str>,
    #[serde(flatten)]
    pub link: AlbumLink,
}

/// Where the cover shown comes from.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverSource {
    pub source: SourceId,
    pub source_name: &'static str,
    /// The user chose this picture.
    pub chosen: bool,
}

/// The details of album `album_id`, or `None` if there is no such album.
/// Reads its cover (through the art cache) to say where it comes from.
pub fn album_details(library: &LibraryState, album_id: i64) -> Result<Option<AlbumDetails>, Error> {
    let details = {
        let conn = library.conn();
        let album: Option<(String, Option<String>, Option<i64>, Option<String>)> = conn
            .query_row(
                "SELECT al.title, ar.name, al.artist_id, al.musicbrainz_release_id
                 FROM albums al LEFT JOIN artists ar ON ar.id = al.artist_id
                 WHERE al.id = ?1",
                [album_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        let Some((title, album_artist, album_artist_id, tagged_release_id)) = album else {
            return Ok(None);
        };
        let mut statement = conn.prepare_cached(
            "SELECT id, disc_number, track_number, title, relative_path, duration, year, genre,
                    work, movement_name, movement_number,
                    (SELECT name FROM artists WHERE id = composer_id), conductor
             FROM tracks WHERE album_id = ?1
             ORDER BY IFNULL(disc_number, 1), track_number NULLS LAST, relative_path, range_start",
        )?;
        let rows = statement.query_map([album_id], |row| {
            let path: String = row.get(4)?;
            let title: Option<String> = row.get(3)?;
            Ok((
                AlbumTrack {
                    id: row.get(0)?,
                    disc: row.get(1)?,
                    number: row.get(2)?,
                    title: title
                        .unwrap_or_else(|| path.rsplit('/').next().unwrap_or(&path).to_owned()),
                    duration: row.get(5)?,
                    work: row.get(8)?,
                    movement_name: row.get(9)?,
                    movement_number: row.get(10)?,
                    composer: row.get(11)?,
                    conductor: row.get(12)?,
                },
                row.get::<_, Option<u32>>(6)?,
                row.get::<_, Option<String>>(7)?,
            ))
        })?;
        let mut tracks = Vec::new();
        let mut year: Option<u32> = None;
        let mut genre_list: Vec<String> = Vec::new();
        let mut folded = Vec::new();
        for row in rows {
            let (track, track_year, genre) = row?;
            tracks.push(track);
            year = match (year, track_year) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            };
            for genre in genres::split(genre.as_deref().unwrap_or("")) {
                let key = fold(genre);
                if !folded.contains(&key) {
                    folded.push(key);
                    genre_list.push(genre.to_owned());
                }
            }
        }
        let settings = settings::service_settings(&conn)?;
        let mut links = Vec::new();
        for source in settings.sources_shown(Kind::Release) {
            if let Some(link) = albums::album_link(&conn, album_id, source)? {
                let info = source.info();
                links.push(SourcedLink {
                    source_name: info.name,
                    stores_details: info.stores_details,
                    page_url: link
                        .external_id
                        .as_deref()
                        .and_then(|id| albums::release_page(source, id)),
                    credit: info.credit,
                    link,
                });
            }
        }
        let mut description = None;
        for source in settings.sources_shown(Kind::AlbumInfo) {
            description = match source {
                SourceId::Wikipedia => {
                    wikipedia::description(&conn, album_id)?.map(SourcedArticle::wikipedia)
                }
                _ => None,
            };
            if description.is_some() {
                break;
            }
        }
        AlbumDetails {
            id: album_id,
            title,
            album_artist,
            album_artist_id,
            year,
            genres: genre_list,
            tagged_release_id,
            duration: tracks.iter().map(|track| track.duration).sum(),
            tracks,
            links,
            description,
            cover: None,
            can_look_up: !settings.sources_for(Kind::Release).is_empty(),
        }
    };
    let cover = art::lookup(library, ArtKey::Album(album_id))?.map(|art| CoverSource {
        source: art.source,
        source_name: art.source.info().name,
        chosen: art.chosen,
    });
    Ok(Some(AlbumDetails { cover, ..details }))
}

/// Fails unless `source` supplies album details, for the "Find details"
/// dialog's actions.
pub fn check_details_source(source: SourceId) -> Result<(), Error> {
    if source.supplies(Kind::Release) {
        Ok(())
    } else {
        Err(Error::Invalid(format!(
            "{} doesn't supply album details",
            source.info().name
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::test_library::{track, Library};

    #[test]
    fn gathers_tags_links_and_the_cover_source() {
        let library = Library::new([
            track("A/B/02.flac")
                .title("Two")
                .artist("Artist")
                .album("Album")
                .genre("Rock; Pop")
                .year(2001)
                .number(2),
            track("A/B/01.flac")
                .artist("Artist")
                .album("Album")
                .genre("rock")
                .year(1999)
                .number(1),
        ]);
        library
            .conn
            .execute(
                "INSERT INTO album_links
                     (album_id, source, status, external_id, score, chosen_by, details,
                      checked_at)
                 VALUES (1, 'musicbrainz', 'review', 'x', 0.7, 'auto', NULL, 5)",
                [],
            )
            .unwrap();
        let state = LibraryState::for_tests(library.conn);
        let details = album_details(&state, 1).unwrap().unwrap();
        assert_eq!(details.title, "Album");
        assert_eq!(details.album_artist.as_deref(), Some("Artist"));
        assert_eq!(details.year, Some(1999));
        assert_eq!(details.genres, ["rock", "Pop"]);
        let titles: Vec<&str> = details.tracks.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, ["01.flac", "Two"]);
        assert_eq!(details.duration, 120.0);
        assert_eq!(details.links.len(), 1);
        assert_eq!(details.links[0].source_name, "MusicBrainz");
        assert_eq!(details.links[0].link.external_id.as_deref(), Some("x"));
        assert_eq!(
            details.links[0].page_url.as_deref(),
            Some("https://musicbrainz.org/release/x")
        );
        assert!(details.links[0].stores_details);
        assert_eq!(details.links[0].credit, None);
        // The files don't exist, so there is no cover.
        assert_eq!(details.cover, None);
        assert_eq!(details.description, None);
        assert!(details.can_look_up);
        assert!(album_details(&state, 9).unwrap().is_none());

        // A Discogs link, once Discogs is on: its release is fetched when
        // shown, and credited.
        state
            .conn()
            .execute(
                "INSERT INTO album_links
                     (album_id, source, status, external_id, score, chosen_by, details,
                      checked_at)
                 VALUES (1, 'discogs', 'matched', '1187003', 1.0, 'user', NULL, 5)",
                [],
            )
            .unwrap();
        assert_eq!(album_details(&state, 1).unwrap().unwrap().links.len(), 1);
        settings::set_has_key(&state.conn(), SourceId::Discogs, true).unwrap();
        let details = album_details(&state, 1).unwrap().unwrap();
        let discogs = &details.links[1];
        assert_eq!(discogs.source_name, "Discogs");
        assert!(!discogs.stores_details);
        assert_eq!(discogs.credit, Some("Data provided by Discogs"));
        assert_eq!(
            discogs.page_url.as_deref(),
            Some("https://www.discogs.com/release/1187003")
        );
        settings::set_has_key(&state.conn(), SourceId::Discogs, false).unwrap();

        // MusicBrainz turned off: its link isn't shown, and nothing can be
        // looked up.
        {
            let conn = state.conn();
            let mut settings = settings::service_settings(&conn).unwrap();
            settings.sources[2].enabled = false;
            settings::save_service_settings(&conn, settings).unwrap();
        }
        let details = album_details(&state, 1).unwrap().unwrap();
        assert!(details.links.is_empty());
        assert!(!details.can_look_up);
    }

    #[test]
    fn shows_the_description_of_the_matched_album() {
        use crate::metadata::albums::LinkStatus;
        use crate::metadata::musicbrainz::{self, fixtures::RELEASES};
        use crate::metadata::wikipedia::{fixtures, parse_extract};
        let library = Library::new([track("Radiohead/In Rainbows/01.flac")
            .artist("Radiohead")
            .album("In Rainbows")]);
        let release = musicbrainz::parse_release(RELEASES[1].1).unwrap();
        let article = parse_extract(fixtures::ALBUM_EXTRACT).unwrap().unwrap();
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
        albums::store_source_link(
            &library.conn,
            1,
            SourceId::Wikipedia,
            LinkStatus::Matched,
            release.release_group_id.as_deref(),
            1.0,
            Some(&article),
        )
        .unwrap();
        let state = LibraryState::for_tests(library.conn);
        let details = album_details(&state, 1).unwrap().unwrap();
        let description = details.description.unwrap();
        assert_eq!(description.source, SourceId::Wikipedia);
        assert_eq!(description.license, "CC BY-SA 4.0");
        assert_eq!(description.article.title, "In Rainbows");
        assert_eq!(details.links.len(), 1, "only album-details sources");

        // Wikipedia turned off: not shown.
        {
            let conn = state.conn();
            let mut settings = settings::service_settings(&conn).unwrap();
            settings.sources[4].enabled = false;
            settings::save_service_settings(&conn, settings).unwrap();
        }
        assert_eq!(album_details(&state, 1).unwrap().unwrap().description, None);
    }
}
