//! The user's marks on the library (PLAN.md F3): a heart on a track, an
//! album or an artist, and a track's rating in whole stars. They live in
//! the library DB by id (migration 007), never in the files; ratings the
//! tags give seed the tracks' (`scanner`) until the user rates them.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use super::discover::AlbumCard;
use super::{track_from_row, unix_now, Error, TrackSummary, TRACKS_FROM, TRACK_COLUMNS};

/// What a heart is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MarkKind {
    Track,
    Album,
    Artist,
}

impl MarkKind {
    fn table(self) -> (&'static str, &'static str, &'static str) {
        match self {
            MarkKind::Track => ("track_favourites", "track_id", "tracks"),
            MarkKind::Album => ("album_favourites", "album_id", "albums"),
            MarkKind::Artist => ("artist_favourites", "artist_id", "artists"),
        }
    }
}

/// Hearts or unhearts each of `ids`; ids no longer in the library are left
/// out. Returns how many changed.
pub fn set_favourite(
    conn: &Connection,
    kind: MarkKind,
    ids: &[i64],
    on: bool,
) -> Result<usize, Error> {
    let (table, column, items) = kind.table();
    let ids = serde_json::to_string(ids).expect("ids serialize");
    let changed = if on {
        conn.execute(
            &format!(
                "INSERT OR IGNORE INTO {table} ({column}, added_at)
                 SELECT id, ?2 FROM {items} WHERE id IN (SELECT value FROM json_each(?1))"
            ),
            params![ids, unix_now()],
        )?
    } else {
        conn.execute(
            &format!("DELETE FROM {table} WHERE {column} IN (SELECT value FROM json_each(?1))"),
            [ids],
        )?
    };
    Ok(changed)
}

/// Which of `ids` have a heart.
pub fn favourites_among(conn: &Connection, kind: MarkKind, ids: &[i64]) -> Result<Vec<i64>, Error> {
    let (table, column, _) = kind.table();
    let ids = serde_json::to_string(ids).expect("ids serialize");
    let mut statement = conn.prepare_cached(&format!(
        "SELECT {column} FROM {table} WHERE {column} IN (SELECT value FROM json_each(?1))"
    ))?;
    let rows = statement.query_map([ids], |row| row.get(0))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Rates tracks 1 to 5 stars, or clears their rating (`None`), as the
/// user's own: a cleared rating stays cleared whatever the tags say.
pub fn set_rating(conn: &Connection, track_ids: &[i64], rating: Option<u8>) -> Result<(), Error> {
    if let Some(stars) = rating.filter(|stars| !(1..=5).contains(stars)) {
        return Err(Error::Invalid(crate::coded::coded(
            "ratingRange",
            &[("stars", stars.into())],
            format!("A rating is 1 to 5 stars, not {stars}"),
        )));
    }
    let ids = serde_json::to_string(track_ids).expect("ids serialize");
    conn.execute(
        "INSERT INTO track_ratings (track_id, rating, source)
         SELECT id, ?2, 'user' FROM tracks WHERE id IN (SELECT value FROM json_each(?1))
         ON CONFLICT (track_id) DO UPDATE SET rating = excluded.rating, source = 'user'",
        params![ids, rating],
    )?;
    Ok(())
}

/// An artist the user hearted.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FavouriteArtist {
    pub id: i64,
    pub name: String,
    /// Tracks by the artist or on their albums.
    pub track_count: u32,
    pub added_at: i64,
}

/// Everything with a heart, newest first: the Favourites view.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Favourites {
    pub tracks: Vec<TrackSummary>,
    pub albums: Vec<AlbumCard>,
    pub artists: Vec<FavouriteArtist>,
}

pub fn favourites(conn: &Connection) -> Result<Favourites, Error> {
    let tracks = conn
        .prepare(&format!(
            "SELECT {TRACK_COLUMNS} {TRACKS_FROM}
             JOIN track_favourites fav ON fav.track_id = t.id
             ORDER BY fav.added_at DESC, t.id"
        ))?
        .query_map([], track_from_row)?
        .collect::<Result<_, _>>()?;
    let albums = conn
        .prepare(
            "SELECT al.id, al.title, ar.name, al.artist_id,
                    (SELECT min(y.year) FROM tracks y WHERE y.album_id = al.id), fav.added_at
             FROM album_favourites fav JOIN albums al ON al.id = fav.album_id
             LEFT JOIN artists ar ON ar.id = al.artist_id
             ORDER BY fav.added_at DESC, al.id",
        )?
        .query_map([], |row| {
            Ok(AlbumCard {
                id: row.get(0)?,
                title: row.get(1)?,
                artist: row.get(2)?,
                artist_id: row.get(3)?,
                year: row.get(4)?,
                at: row.get(5)?,
                note: None,
            })
        })?
        .collect::<Result<_, _>>()?;
    let artists = conn
        .prepare(
            "SELECT a.id, a.name,
                    (SELECT count(*) FROM tracks t WHERE t.album_artist_id = a.id
                        OR t.id IN (SELECT track_id FROM track_artists WHERE artist_id = a.id)),
                    fav.added_at
             FROM artist_favourites fav JOIN artists a ON a.id = fav.artist_id
             ORDER BY fav.added_at DESC, a.id",
        )?
        .query_map([], |row| {
            Ok(FavouriteArtist {
                id: row.get(0)?,
                name: row.get(1)?,
                track_count: row.get(2)?,
                added_at: row.get(3)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(Favourites {
        tracks,
        albums,
        artists,
    })
}

/// SQL true for a track `t` that is a favourite, is on a favourite album,
/// or credits (or is on an album by) a favourite artist: what the
/// browser's "Favourites only" filter keeps.
pub const FAVOURITE_FILTER: &str = "(EXISTS (SELECT 1 FROM track_favourites WHERE track_id = t.id)
      OR EXISTS (SELECT 1 FROM album_favourites WHERE album_id = t.album_id)
      OR EXISTS (SELECT 1 FROM artist_favourites WHERE artist_id = t.album_artist_id)
      OR EXISTS (SELECT 1 FROM artist_favourites af JOIN track_artists ta
                 ON ta.artist_id = af.artist_id WHERE ta.track_id = t.id))";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::test_library::{track, Library};

    fn ids(conn: &Connection, sql: &str) -> Vec<i64> {
        conn.prepare(sql)
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    #[test]
    fn hearts_tracks_albums_and_artists() {
        let library = Library::new([
            track("a/1.flac")
                .title("One")
                .artist("Band")
                .album("Record"),
            track("a/2.flac")
                .title("Two")
                .artist("Band; Guest")
                .album("Record"),
            track("b/1.flac").title("Other").artist("Else"),
        ]);
        let conn = &library.conn;
        let tracks = ids(conn, "SELECT id FROM tracks ORDER BY id");
        let album = ids(conn, "SELECT id FROM albums")[0];
        let guest = ids(conn, "SELECT id FROM artists WHERE name = 'Guest'")[0];

        assert_eq!(
            set_favourite(conn, MarkKind::Track, &[tracks[2], 999], true).unwrap(),
            1
        );
        assert_eq!(
            set_favourite(conn, MarkKind::Track, &[tracks[2]], true).unwrap(),
            0
        );
        set_favourite(conn, MarkKind::Album, &[album], true).unwrap();
        set_favourite(conn, MarkKind::Artist, &[guest], true).unwrap();
        assert_eq!(
            favourites_among(conn, MarkKind::Track, &tracks).unwrap(),
            [tracks[2]]
        );

        let all = favourites(conn).unwrap();
        assert_eq!(all.tracks.len(), 1);
        assert!(all.tracks[0].favourite);
        assert_eq!(all.albums[0].title, "Record");
        assert_eq!(
            (all.artists[0].name.as_str(), all.artists[0].track_count),
            ("Guest", 1)
        );

        // The filter: the hearted track, the album's two, the guest's one.
        let kept = ids(
            conn,
            &format!("SELECT t.id FROM tracks t WHERE {FAVOURITE_FILTER} ORDER BY t.id"),
        );
        assert_eq!(kept, tracks);
        set_favourite(conn, MarkKind::Album, &[album], false).unwrap();
        let kept = ids(
            conn,
            &format!("SELECT t.id FROM tracks t WHERE {FAVOURITE_FILTER} ORDER BY t.id"),
        );
        assert_eq!(kept, [tracks[1], tracks[2]]);
    }

    #[test]
    fn rates_in_whole_stars_as_the_users_own() {
        let library = Library::new([track("a.flac").title("A"), track("b.flac").title("B")]);
        let conn = &library.conn;
        let tracks = ids(conn, "SELECT id FROM tracks ORDER BY id");
        set_rating(conn, &tracks, Some(4)).unwrap();
        assert!(set_rating(conn, &tracks, Some(6)).is_err());
        set_rating(conn, &tracks[..1], None).unwrap();
        let rows: Vec<(Option<i64>, String)> = conn
            .prepare("SELECT rating, source FROM track_ratings ORDER BY track_id")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(rows, [(None, "user".into()), (Some(4), "user".into())]);
        let listed = crate::library::tracks(conn).unwrap();
        assert_eq!((listed[0].rating, listed[1].rating), (None, Some(4)));
    }
}
