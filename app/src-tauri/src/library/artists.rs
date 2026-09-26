//! The artist page: an artist's albums, the albums of other artists they
//! appear on, and what the metadata sources know about them
//! (`metadata::artists::artist_info`).

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::{rules, Error};
use crate::metadata::artists::{self, ArtistInfo};
use crate::metadata::settings::{self, SourceId};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistPage {
    pub id: i64,
    pub name: String,
    /// Tracks by the artist or on their albums.
    pub track_count: u32,
    /// Albums the artist is the album artist of, oldest first.
    pub albums: Vec<ArtistAlbum>,
    /// Other artists' albums with tracks by the artist, oldest first.
    pub appears_on: Vec<ArtistAlbum>,
    pub info: ArtistInfo,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistAlbum {
    pub id: i64,
    pub title: String,
    pub album_artist: Option<String>,
    pub album_artist_id: Option<i64>,
    /// The earliest year among its tracks.
    pub year: Option<u32>,
    pub track_count: u32,
    /// From the album's MusicBrainz match, while MusicBrainz is shown: the
    /// release group's type ("Album", "EP", "Single"…) and secondary types
    /// ("Compilation", "Live"…).
    pub release_type: Option<String>,
    pub secondary_types: Vec<String>,
}

/// The page for artist `artist_id`, or `None` if there is no such artist.
pub fn artist_page(conn: &Connection, artist_id: i64) -> Result<Option<ArtistPage>, Error> {
    let artist: Option<(String, u32)> = conn
        .query_row(
            "SELECT name,
                    (SELECT count(*) FROM tracks
                     WHERE album_artist_id = ?1 OR artist_id = ?1)
             FROM artists WHERE id = ?1",
            [artist_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((name, track_count)) = artist else {
        return Ok(None);
    };
    let show_types = settings::service_settings(conn)?.is_shown(SourceId::MusicBrainz);
    let own = albums(conn, artist_id, "al.artist_id = ?1", show_types)?;
    let appears_on = albums(
        conn,
        artist_id,
        "al.artist_id IS NOT ?1 AND al.id IN
             (SELECT album_id FROM tracks WHERE artist_id = ?1 AND album_id IS NOT NULL)",
        show_types,
    )?;
    Ok(Some(ArtistPage {
        id: artist_id,
        name,
        track_count,
        albums: own,
        appears_on,
        info: artists::artist_info(conn, artist_id)?,
    }))
}

/// The albums that `filter` (a fixed fragment over `al` with the artist id
/// as ?1) picks, oldest first, then by title as the library sorts them.
fn albums(
    conn: &Connection,
    artist_id: i64,
    filter: &str,
    show_types: bool,
) -> Result<Vec<ArtistAlbum>, Error> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Types {
        release_type: Option<String>,
        #[serde(default)]
        secondary_types: Vec<String>,
    }
    let articles = rules::sort_settings(conn)?.ignored_articles.join("\n");
    let mut statement = conn.prepare_cached(&format!(
        "SELECT al.id, al.title, ar.name, al.artist_id, y.year,
                (SELECT count(*) FROM tracks WHERE album_id = al.id),
                (SELECT details FROM album_links
                 WHERE album_id = al.id AND source = ?3 AND status = 'matched')
         FROM albums al
         LEFT JOIN artists ar ON ar.id = al.artist_id
         LEFT JOIN (SELECT album_id, min(year) AS year FROM tracks GROUP BY album_id) y
             ON y.album_id = al.id
         WHERE {filter}
         ORDER BY y.year IS NULL, y.year, anomp_sort_key(al.title, ?2), al.title, al.id"
    ))?;
    let rows = statement.query_map(
        params![artist_id, articles, SourceId::MusicBrainz.as_str()],
        |row| {
            let details: Option<String> = row.get(6)?;
            let types = details
                .filter(|_| show_types)
                .and_then(|json| serde_json::from_str::<Types>(&json).ok());
            let (release_type, secondary_types) = match types {
                Some(types) => (types.release_type, types.secondary_types),
                None => (None, Vec::new()),
            };
            Ok(ArtistAlbum {
                id: row.get(0)?,
                title: row.get(1)?,
                album_artist: row.get(2)?,
                album_artist_id: row.get(3)?,
                year: row.get(4)?,
                track_count: row.get(5)?,
                release_type,
                secondary_types,
            })
        },
    )?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::test_library::{track, Library};

    fn library() -> Library {
        Library::new([
            track("A/Second/01.flac")
                .artist("Artist")
                .album("Second")
                .year(2001),
            track("A/Second/02.flac")
                .artist("Artist")
                .album("Second")
                .year(2003),
            track("A/The First/01.flac")
                .artist("Artist")
                .album("The First")
                .year(1999),
            track("A/Undated/01.flac").artist("Artist").album("Undated"),
            track("A/A Later One/01.flac")
                .artist("Artist")
                .album("A Later One")
                .year(2001),
            // A feature on someone else's album.
            track("B/Theirs/01.flac")
                .artist("Other")
                .album("Theirs")
                .year(2010),
            track("B/Theirs/02.flac")
                .artist("Artist")
                .album_artist("Other")
                .album("Theirs")
                .year(2010),
        ])
    }

    #[test]
    fn lists_albums_oldest_first_and_appearances() {
        let library = library();
        let artist = library.artist(Some("Artist")).unwrap();
        let page = artist_page(&library.conn, artist).unwrap().unwrap();
        assert_eq!(page.name, "Artist");
        assert_eq!(page.track_count, 6);
        let titles: Vec<(&str, Option<u32>)> = page
            .albums
            .iter()
            .map(|album| (album.title.as_str(), album.year))
            .collect();
        // Undated last; within a year, "A Later One" sorts as "Later One".
        assert_eq!(
            titles,
            [
                ("The First", Some(1999)),
                ("A Later One", Some(2001)),
                ("Second", Some(2001)),
                ("Undated", None),
            ]
        );
        assert_eq!(page.albums[2].track_count, 2);
        assert_eq!(page.appears_on.len(), 1);
        let theirs = &page.appears_on[0];
        assert_eq!(
            (theirs.title.as_str(), theirs.album_artist.as_deref()),
            ("Theirs", Some("Other"))
        );
        assert_eq!(page.info.status, None);

        let other = library.artist(Some("Other")).unwrap();
        let page = artist_page(&library.conn, other).unwrap().unwrap();
        assert_eq!(page.albums.len(), 1);
        assert!(page.appears_on.is_empty(), "their own album isn't repeated");
        assert_eq!(artist_page(&library.conn, 999).unwrap(), None);
    }

    #[test]
    fn shows_release_types_from_matched_albums() {
        let library = library();
        let artist = library.artist(Some("Artist")).unwrap();
        let album: i64 = library
            .conn
            .query_row("SELECT id FROM albums WHERE title = 'Second'", [], |row| {
                row.get(0)
            })
            .unwrap();
        let link = |status: &str| {
            library
                .conn
                .execute(
                    "INSERT OR REPLACE INTO album_links
                         (album_id, source, status, external_id, score, chosen_by, details,
                          checked_at)
                     VALUES (?1, 'musicbrainz', ?2, 'x', 1.0, 'auto',
                             '{\"releaseType\": \"EP\", \"secondaryTypes\": [\"Live\"]}', 0)",
                    params![album, status],
                )
                .unwrap();
        };
        let second = |library: &Library| {
            artist_page(&library.conn, artist)
                .unwrap()
                .unwrap()
                .albums
                .into_iter()
                .find(|album| album.title == "Second")
                .unwrap()
        };
        link("review");
        assert_eq!(second(&library).release_type, None, "not accepted");
        link("matched");
        let found = second(&library);
        assert_eq!(found.release_type.as_deref(), Some("EP"));
        assert_eq!(found.secondary_types, ["Live"]);

        let mut services = settings::service_settings(&library.conn).unwrap();
        services.sources[2].enabled = false; // MusicBrainz.
        settings::save_service_settings(&library.conn, services).unwrap();
        assert_eq!(second(&library).release_type, None, "hidden");
    }
}
