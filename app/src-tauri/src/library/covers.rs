//! The albums the visualizer's cover wall shows around the current track:
//! those from the same year (widening to nearby years when the library has
//! few from it) or by the same artist. Only ids and names: the wall loads
//! each cover from the `anomp-art` scheme and leaves out albums without one.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::{rules, Error};

/// At most this many albums on a wall.
pub const MAX_ALBUMS: usize = 200;

/// A year wall widens to nearby years until it has this many albums.
const ENOUGH_ALBUMS: usize = 16;

/// Years either side a year wall may widen to, in the order tried.
const YEAR_SPANS: [u32; 5] = [0, 1, 2, 3, 5];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum CoverBasis {
    Year,
    Artist,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverWall {
    pub basis: CoverBasis,
    /// "1997", "1995–1999", or the artist's name.
    pub label: String,
    /// A year wall's first and last year.
    pub years: Option<(u32, u32)>,
    /// An artist wall's artist.
    pub artist_id: Option<i64>,
    /// The current track's album first, if it has one; then a year wall's
    /// albums nearest the year first, or an artist's oldest first.
    pub albums: Vec<CoverAlbum>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverAlbum {
    pub id: i64,
    pub title: String,
    pub artist: Option<String>,
    /// The earliest year among its tracks.
    pub year: Option<u32>,
}

/// The wall around track `track_id`, or `None` if there is no such track, or
/// it has no year (for a year wall) or artist (for an artist wall).
pub fn cover_wall(
    conn: &Connection,
    track_id: i64,
    basis: CoverBasis,
) -> Result<Option<CoverWall>, Error> {
    // The track's album's year (its earliest track's), else its own.
    let track: Option<(Option<i64>, Option<u32>, Option<i64>, Option<i64>)> = conn
        .query_row(
            "SELECT t.album_id,
                    IFNULL((SELECT min(year) FROM tracks WHERE album_id = t.album_id), t.year),
                    t.artist_id, al.artist_id
             FROM tracks t LEFT JOIN albums al ON al.id = t.album_id
             WHERE t.id = ?1",
            [track_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    let Some((album_id, year, artist_id, album_artist_id)) = track else {
        return Ok(None);
    };
    let articles = rules::sort_settings(conn)?.ignored_articles.join("\n");

    let wall = match basis {
        CoverBasis::Year => year
            .map(|year| year_wall(conn, year, &articles))
            .transpose()?,
        // The performer, even on a compilation.
        CoverBasis::Artist => artist_id
            .or(album_artist_id)
            .map(|artist| artist_wall(conn, artist, &articles))
            .transpose()?
            .flatten(),
    };
    Ok(wall.map(|mut wall| {
        if let Some(position) = wall
            .albums
            .iter()
            .position(|album| Some(album.id) == album_id)
        {
            let current = wall.albums.remove(position);
            wall.albums.insert(0, current);
        }
        wall.albums.truncate(MAX_ALBUMS);
        wall
    }))
}

fn year_wall(conn: &Connection, year: u32, articles: &str) -> Result<CoverWall, Error> {
    let dated = albums(
        conn,
        "y.year IS NOT NULL",
        "anomp_sort_key(al.title, ?1), al.title, al.id",
        params![articles],
    )?;
    let within = |span: u32| {
        dated
            .iter()
            .filter(|album| album.year.is_some_and(|y| y.abs_diff(year) <= span))
            .count()
    };
    let span = YEAR_SPANS
        .into_iter()
        .find(|&span| within(span) >= ENOUGH_ALBUMS)
        .unwrap_or(YEAR_SPANS[YEAR_SPANS.len() - 1]);

    let mut albums: Vec<CoverAlbum> = dated
        .into_iter()
        .filter(|album| album.year.is_some_and(|y| y.abs_diff(year) <= span))
        .collect();
    // Nearest the year first, by title within a year (the stable sort
    // keeps the query's order).
    albums.sort_by_key(|album| album.year.map(|y| y.abs_diff(year)));
    let (first, last) = (year.saturating_sub(span), year + span);
    Ok(CoverWall {
        basis: CoverBasis::Year,
        label: if span == 0 {
            year.to_string()
        } else {
            format!("{first}–{last}")
        },
        years: Some((first, last)),
        artist_id: None,
        albums,
    })
}

fn artist_wall(
    conn: &Connection,
    artist_id: i64,
    articles: &str,
) -> Result<Option<CoverWall>, Error> {
    let name: Option<String> = conn
        .query_row(
            "SELECT name FROM artists WHERE id = ?1",
            [artist_id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(name) = name else {
        return Ok(None);
    };
    // Their albums and those they appear on, oldest first.
    let albums = albums(
        conn,
        "al.artist_id = ?2 OR al.id IN (SELECT album_id FROM tracks WHERE artist_id = ?2)",
        "y.year IS NULL, y.year, anomp_sort_key(al.title, ?1), al.title, al.id",
        params![articles, artist_id],
    )?;
    Ok(Some(CoverWall {
        basis: CoverBasis::Artist,
        label: name,
        years: None,
        artist_id: Some(artist_id),
        albums,
    }))
}

/// Albums matching `filter`, in `order`: fixed SQL fragments over `al` and
/// `y` (the album's earliest year), with the ignored articles as ?1.
fn albums(
    conn: &Connection,
    filter: &str,
    order: &str,
    values: impl rusqlite::Params,
) -> Result<Vec<CoverAlbum>, Error> {
    let mut statement = conn.prepare_cached(&format!(
        "SELECT al.id, al.title, ar.name, y.year
         FROM albums al
         LEFT JOIN artists ar ON ar.id = al.artist_id
         LEFT JOIN (SELECT album_id, min(year) AS year FROM tracks GROUP BY album_id) y
             ON y.album_id = al.id
         WHERE {filter}
         ORDER BY {order}"
    ))?;
    let rows = statement.query_map(values, |row| {
        Ok(CoverAlbum {
            id: row.get(0)?,
            title: row.get(1)?,
            artist: row.get(2)?,
            year: row.get(3)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::test_library::{track, Library};

    fn track_id(library: &Library, path: &str) -> i64 {
        library
            .conn
            .query_row(
                "SELECT id FROM tracks WHERE relative_path = ?1",
                [path],
                |row| row.get(0),
            )
            .unwrap()
    }

    fn titles(wall: &CoverWall) -> Vec<&str> {
        wall.albums
            .iter()
            .map(|album| album.title.as_str())
            .collect()
    }

    /// `count` albums from `year`, by one artist each.
    fn albums_of(year: u32, count: usize) -> Vec<crate::library::test_library::Track> {
        (0..count)
            .map(|i| {
                track(&format!("{year}/{i}/01.flac"))
                    .artist(&format!("Artist {year} {i}"))
                    .album(&format!("Album {year} {i:02}"))
                    .year(year)
            })
            .collect()
    }

    #[test]
    fn a_year_wall_has_that_years_albums_with_the_current_one_first() {
        let mut tracks = albums_of(1997, 20);
        tracks.extend(albums_of(1998, 3));
        tracks.push(
            track("x/01.flac")
                .artist("X")
                .album("The Current")
                .year(1997),
        );
        // Its album's year is its earliest track's.
        tracks.push(
            track("x/02.flac")
                .artist("X")
                .album("The Current")
                .year(2004),
        );
        let library = Library::new(tracks);

        let wall = cover_wall(
            &library.conn,
            track_id(&library, "x/02.flac"),
            CoverBasis::Year,
        )
        .unwrap()
        .unwrap();
        assert_eq!(wall.basis, CoverBasis::Year);
        assert_eq!(wall.label, "1997");
        assert_eq!(wall.years, Some((1997, 1997)));
        assert_eq!(wall.albums.len(), 21);
        assert_eq!(wall.albums[0].title, "The Current");
        assert_eq!(wall.albums[0].artist.as_deref(), Some("X"));
        assert_eq!(wall.albums[1].title, "Album 1997 00");
        assert!(wall.albums.iter().all(|album| album.year == Some(1997)));
    }

    #[test]
    fn a_sparse_year_widens_to_nearby_years_nearest_first() {
        let mut tracks = albums_of(1990, 2);
        tracks.extend(albums_of(1991, 6));
        tracks.extend(albums_of(1988, 10));
        tracks.extend(albums_of(1970, 30));
        let library = Library::new(tracks);

        let wall = cover_wall(
            &library.conn,
            track_id(&library, "1990/0/01.flac"),
            CoverBasis::Year,
        )
        .unwrap()
        .unwrap();
        // ±1 has 8 and ±2 has 18.
        assert_eq!(wall.label, "1988–1992");
        assert_eq!(wall.years, Some((1988, 1992)));
        let years: Vec<Option<u32>> = wall.albums.iter().map(|album| album.year).collect();
        let mut expected = vec![Some(1990); 2];
        expected.extend([Some(1991); 6]);
        expected.extend([Some(1988); 10]);
        assert_eq!(years, expected);
    }

    #[test]
    fn a_lonely_year_widens_as_far_as_it_may() {
        let mut tracks = albums_of(2000, 1);
        tracks.extend(albums_of(2006, 1));
        tracks.extend(albums_of(2005, 1));
        let library = Library::new(tracks);
        let wall = cover_wall(
            &library.conn,
            track_id(&library, "2000/0/01.flac"),
            CoverBasis::Year,
        )
        .unwrap()
        .unwrap();
        assert_eq!(wall.label, "1995–2005");
        assert_eq!(titles(&wall), ["Album 2000 00", "Album 2005 00"]);
    }

    #[test]
    fn an_artist_wall_has_their_albums_and_appearances_oldest_first() {
        let library = Library::new([
            track("a/1.flac").artist("Artist").album("Later").year(2005),
            track("a/2.flac")
                .artist("Artist")
                .album("Earlier")
                .year(1999),
            track("a/3.flac").artist("Artist").album("Undated"),
            track("b/1.flac").artist("Other").album("Split").year(2001),
            track("b/2.flac")
                .artist("Artist")
                .album_artist("Other")
                .album("Split")
                .year(2001),
            track("c/1.flac")
                .artist("Other")
                .album("Theirs Alone")
                .year(2000),
        ]);

        let wall = cover_wall(
            &library.conn,
            track_id(&library, "a/1.flac"),
            CoverBasis::Artist,
        )
        .unwrap()
        .unwrap();
        assert_eq!(wall.basis, CoverBasis::Artist);
        assert_eq!(wall.label, "Artist");
        assert_eq!(wall.artist_id, library.artist(Some("Artist")));
        assert_eq!(titles(&wall), ["Later", "Earlier", "Split", "Undated"]);

        // On someone else's album, the wall is still the performer's.
        let wall = cover_wall(
            &library.conn,
            track_id(&library, "b/2.flac"),
            CoverBasis::Artist,
        )
        .unwrap()
        .unwrap();
        assert_eq!(wall.label, "Artist");
        assert_eq!(titles(&wall), ["Split", "Earlier", "Later", "Undated"]);
    }

    #[test]
    fn no_wall_without_a_year_or_artist_or_track() {
        let library = Library::new([track("u/1.flac").title("Loose")]);
        let id = track_id(&library, "u/1.flac");
        assert_eq!(
            cover_wall(&library.conn, id, CoverBasis::Year).unwrap(),
            None
        );
        assert_eq!(
            cover_wall(&library.conn, id, CoverBasis::Artist).unwrap(),
            None
        );
        assert_eq!(
            cover_wall(&library.conn, 999, CoverBasis::Year).unwrap(),
            None
        );
    }

    #[test]
    fn an_album_less_track_gets_the_wall_of_its_own_year() {
        let mut tracks = albums_of(1980, 2);
        tracks.push(track("loose.flac").artist("Solo").year(1980));
        let library = Library::new(tracks);
        let wall = cover_wall(
            &library.conn,
            track_id(&library, "loose.flac"),
            CoverBasis::Year,
        )
        .unwrap()
        .unwrap();
        assert_eq!(wall.label, "1975–1985");
        assert_eq!(titles(&wall), ["Album 1980 00", "Album 1980 01"]);
    }
}
