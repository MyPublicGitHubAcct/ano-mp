//! Ways into the library besides browsing it (PLAN.md §4.6): the albums
//! added most recently (O15), albums released on this day in earlier years
//! (O17), and a few random albums in the same genre as one (O18). The
//! history's views (O8, O16, O19) are in `history::views`.

use rusqlite::{params, Connection};
use serde::Serialize;

use super::availability::json_ids;
use super::{rules, Error};

/// An album as the discovery views list it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AlbumCard {
    pub id: i64,
    pub title: String,
    pub artist: Option<String>,
    pub artist_id: Option<i64>,
    /// The earliest year among its tracks.
    pub year: Option<u32>,
    /// A time the view is about (added, played), Unix seconds.
    pub at: Option<i64>,
    /// What the view says about it ("30 years ago today", "12 plays").
    pub note: Option<String>,
}

/// The columns `card` reads: album `al`, its artist `ar`, and a year.
const CARD_COLUMNS: &str = "al.id, al.title, ar.name, al.artist_id,
     (SELECT min(y.year) FROM tracks y WHERE y.album_id = al.id)";

/// Albums with a track outside the folders bound as JSON at `?N`: Home's
/// suggestions leave out albums none of whose tracks can be opened now
/// (PLAN.md H22b).
pub(crate) fn playable_album(n: u8) -> String {
    format!(
        "EXISTS (SELECT 1 FROM tracks pt WHERE pt.album_id = al.id
                 AND pt.folder_id NOT IN (SELECT value FROM json_each(?{n})))"
    )
}

fn card(row: &rusqlite::Row, at: Option<i64>, note: Option<String>) -> rusqlite::Result<AlbumCard> {
    Ok(AlbumCard {
        id: row.get(0)?,
        title: row.get(1)?,
        artist: row.get(2)?,
        artist_id: row.get(3)?,
        year: row.get(4)?,
        at,
        note,
    })
}

/// Albums by when their newest track arrived, newest first (O15), counting
/// only tracks outside the folders `unreadable`. Moving or renaming a file
/// makes it look new: a track is its path.
pub fn recently_added(
    conn: &Connection,
    limit: u32,
    unreadable: &[i64],
) -> Result<Vec<AlbumCard>, Error> {
    let mut statement = conn.prepare_cached(&format!(
        "SELECT {CARD_COLUMNS}, max(t.added_at) AS added
         FROM tracks t JOIN albums al ON al.id = t.album_id
         LEFT JOIN artists ar ON ar.id = al.artist_id
         WHERE t.folder_id NOT IN (SELECT value FROM json_each(?2))
         GROUP BY al.id ORDER BY added DESC, al.id DESC LIMIT ?1"
    ))?;
    let rows = statement.query_map(params![limit, json_ids(unreadable)], |row| {
        card(row, row.get(5)?, None)
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Albums first released on `month`-`day` in an earlier year than `year`
/// (O17), oldest first, each noting how long ago. The date is the
/// MusicBrainz release group's first release date when the album is
/// matched, so a reissue counts on the original's day, else the full date
/// in its tags. Albums dated only to a year or month never match; on 28
/// February outside leap years, 29 February's albums show too. Albums
/// only in the folders `unreadable` are left out.
pub fn on_this_day(
    conn: &Connection,
    year: i32,
    month: u32,
    day: u32,
    unreadable: &[i64],
) -> Result<Vec<AlbumCard>, Error> {
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let today = format!("{month:02}-{day:02}");
    let also = if month == 2 && day == 28 && !leap {
        "02-29".to_owned()
    } else {
        today.clone()
    };
    let playable = playable_album(4);
    let mut statement = conn.prepare_cached(&format!(
        "WITH dates (album_id, date) AS (
             SELECT al.id, COALESCE(
                 (SELECT json_extract(l.details, '$.firstReleaseDate') FROM album_links l
                  WHERE l.album_id = al.id AND l.source = 'musicbrainz' AND l.status = 'matched'
                    AND length(json_extract(l.details, '$.firstReleaseDate')) = 10),
                 (SELECT min(t.release_date) FROM tracks t
                  WHERE t.album_id = al.id AND length(t.release_date) = 10))
             FROM albums al
         )
         SELECT {CARD_COLUMNS}, d.date
         FROM dates d JOIN albums al ON al.id = d.album_id
         LEFT JOIN artists ar ON ar.id = al.artist_id
         WHERE substr(d.date, 6, 5) IN (?1, ?2) AND CAST(substr(d.date, 1, 4) AS INTEGER) < ?3
           AND {playable}
         ORDER BY d.date, al.id"
    ))?;
    let rows = statement.query_map(params![today, also, year, json_ids(unreadable)], |row| {
        let date: String = row.get(5)?;
        let released: i32 = date[..4].parse().unwrap_or(year);
        let years = year - released;
        let note = if years == 1 {
            "A year ago today".to_owned()
        } else {
            format!("{years} years ago today")
        };
        card(row, None, Some(note))
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Up to `count` albums other than `album_id` with a track in `genre`,
/// drawn at random with `seed` (the same seed draws the same albums).
/// Albums played in the last 30 days are drawn a quarter as often (O18).
/// Albums only in the folders `unreadable` are left out.
pub fn more_in_genre(
    conn: &Connection,
    album_id: i64,
    genre: &str,
    seed: u64,
    count: usize,
    unreadable: &[i64],
) -> Result<Vec<AlbumCard>, Error> {
    let articles = rules::sort_settings(conn)?.ignored_articles.join("\n");
    let recent = super::unix_now() - 30 * 86400;
    let playable = playable_album(5);
    let mut statement = conn.prepare_cached(&format!(
        "SELECT {CARD_COLUMNS},
                EXISTS (SELECT 1 FROM plays p JOIN tracks pt ON pt.id = p.track_id
                        WHERE pt.album_id = al.id AND p.played_at >= ?3)
         FROM albums al LEFT JOIN artists ar ON ar.id = al.artist_id
         WHERE al.id != ?1
           AND EXISTS (SELECT 1 FROM tracks t WHERE t.album_id = al.id
                       AND anomp_has_genre(t.genre, ?2))
           AND {playable}
         ORDER BY anomp_sort_key(al.title, ?4), al.id"
    ))?;
    let candidates: Vec<(AlbumCard, bool)> = statement
        .query_map(
            params![album_id, genre, recent, articles, json_ids(unreadable)],
            |row| Ok((card(row, None, None)?, row.get(5)?)),
        )?
        .collect::<Result<_, _>>()?;
    Ok(weighted_sample(candidates, seed, count))
}

/// Draws `count` items without replacement, recently played ones with a
/// quarter of the weight (Efraimidis–Spirakis: the largest `u^(1/w)`).
fn weighted_sample(candidates: Vec<(AlbumCard, bool)>, seed: u64, count: usize) -> Vec<AlbumCard> {
    let mut rng = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
    let mut keyed: Vec<(f64, AlbumCard)> = candidates
        .into_iter()
        .map(|(album, recent)| {
            rng ^= rng >> 12;
            rng ^= rng << 25;
            rng ^= rng >> 27;
            let u = ((rng.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 11) as f64 + 1.0)
                / (1u64 << 53) as f64;
            let weight = if recent { 0.25 } else { 1.0 };
            (u.powf(1.0 / weight), album)
        })
        .collect();
    keyed.sort_by(|a, b| b.0.total_cmp(&a.0));
    keyed
        .into_iter()
        .take(count)
        .map(|(_, album)| album)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::test_library::{track, Library};

    fn titles(cards: &[AlbumCard]) -> Vec<&str> {
        cards.iter().map(|card| card.title.as_str()).collect()
    }

    #[test]
    fn lists_albums_by_when_they_were_added() {
        let library = Library::new([
            track("a/1.flac").album("Old"),
            track("b/1.flac").album("New"),
            track("c/1.flac").album("Middle"),
            track("c/2.flac").album("Middle"),
        ]);
        library
            .conn
            .execute_batch(
                "UPDATE tracks SET added_at = 100 WHERE relative_path = 'a/1.flac';
                 UPDATE tracks SET added_at = 300 WHERE relative_path = 'b/1.flac';
                 UPDATE tracks SET added_at = 150 WHERE relative_path = 'c/1.flac';
                 UPDATE tracks SET added_at = 200 WHERE relative_path = 'c/2.flac';",
            )
            .unwrap();
        let cards = recently_added(&library.conn, 10, &[]).unwrap();
        assert_eq!(titles(&cards), ["New", "Middle", "Old"]);
        assert_eq!(cards[1].at, Some(200));
        assert_eq!(recently_added(&library.conn, 1, &[]).unwrap().len(), 1);
    }

    #[test]
    fn finds_albums_released_on_this_day() {
        let library = Library::new([
            track("a.flac").album("Tagged"),
            track("b.flac").album("Matched"),
            track("c.flac").album("Year only"),
            track("d.flac").album("Leap"),
            track("e.flac").album("This year"),
        ]);
        library
            .conn
            .execute_batch(
                "UPDATE tracks SET release_date = '1997-05-21' WHERE relative_path = 'a.flac';
                 UPDATE tracks SET release_date = '2010-01-01' WHERE relative_path = 'b.flac';
                 UPDATE tracks SET release_date = '1990' WHERE relative_path = 'c.flac';
                 UPDATE tracks SET release_date = '1996-02-29' WHERE relative_path = 'd.flac';
                 UPDATE tracks SET release_date = '2027-05-21' WHERE relative_path = 'e.flac';
                 INSERT INTO album_links (album_id, source, status, external_id, score, chosen_by,
                                          details, checked_at)
                 VALUES (2, 'musicbrainz', 'matched', 'x', 1, 'auto',
                         '{\"firstReleaseDate\": \"1987-05-21\"}', 0);",
            )
            .unwrap();
        let cards = on_this_day(&library.conn, 2027, 5, 21, &[]).unwrap();
        assert_eq!(titles(&cards), ["Matched", "Tagged"]);
        assert_eq!(cards[0].note.as_deref(), Some("40 years ago today"));
        assert_eq!(cards[1].note.as_deref(), Some("30 years ago today"));
        assert!(on_this_day(&library.conn, 2027, 1, 1, &[])
            .unwrap()
            .is_empty());
        // 29 February shows on the 28th outside leap years, and on the 29th in them.
        assert_eq!(
            titles(&on_this_day(&library.conn, 2027, 2, 28, &[]).unwrap()),
            ["Leap"]
        );
        assert!(on_this_day(&library.conn, 2028, 2, 28, &[])
            .unwrap()
            .is_empty());
        assert_eq!(
            titles(&on_this_day(&library.conn, 2028, 2, 29, &[]).unwrap()),
            ["Leap"]
        );
    }

    #[test]
    fn draws_albums_in_a_genre() {
        let mut tracks = vec![track("x.flac").album("Current").genre("Jazz")];
        for i in 0..12 {
            tracks.push(
                track(&format!("{i}.flac"))
                    .album(&format!("Album {i:02}"))
                    .genre(if i % 3 == 0 { "Rock" } else { "Jazz; Soul" }),
            );
        }
        let library = Library::new(tracks);
        let current = 1;
        let five = more_in_genre(&library.conn, current, "jazz", 7, 5, &[]).unwrap();
        assert_eq!(five.len(), 5);
        assert!(five.iter().all(|card| card.id != current));
        assert!(five
            .iter()
            .all(|card| !["Album 00", "Album 03"].contains(&card.title.as_str())));
        // The same seed draws the same five; another, (almost surely) not.
        assert_eq!(
            more_in_genre(&library.conn, current, "jazz", 7, 5, &[]).unwrap(),
            five
        );
        let other = more_in_genre(&library.conn, current, "jazz", 8, 5, &[]).unwrap();
        assert_ne!(other, five);
        // Only four rock albums, and the genre is matched as a whole name.
        assert_eq!(
            more_in_genre(&library.conn, current, "Rock", 1, 5, &[])
                .unwrap()
                .len(),
            4
        );
        assert!(more_in_genre(&library.conn, current, "Jaz", 1, 5, &[])
            .unwrap()
            .is_empty());
    }

    #[test]
    fn leaves_out_albums_of_unreadable_folders() {
        let library = Library::new([
            track("here.flac").album("Here").genre("Jazz"),
            track("both/1.flac").album("Both").genre("Jazz"),
            track("current.flac").album("Current").genre("Jazz"),
        ]);
        let away = library.add_folder("/Volumes/Away");
        library.add(away, track("away.flac").album("Away").genre("Jazz"));
        library.add(away, track("both/2.flac").album("Both").genre("Jazz"));
        library
            .conn
            .execute_batch(
                "UPDATE tracks SET release_date = '1990-05-21';
                 UPDATE tracks SET added_at = 100;
                 UPDATE tracks SET added_at = 900 WHERE relative_path = 'both/2.flac';",
            )
            .unwrap();
        let mut all = titles(&recently_added(&library.conn, 10, &[]).unwrap())
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        assert_eq!(all[0], "Both", "its newest track is in the away folder");
        all.sort();
        assert_eq!(all, ["Away", "Both", "Current", "Here"]);

        // An album with a track that can be opened stays; its date is that
        // track's.
        let added = recently_added(&library.conn, 10, &[away]).unwrap();
        let mut added_titles = titles(&added);
        added_titles.sort();
        assert_eq!(added_titles, ["Both", "Current", "Here"]);
        assert!(added.iter().all(|card| card.at == Some(100)));

        let on_day = on_this_day(&library.conn, 2027, 5, 21, &[away]).unwrap();
        assert_eq!(titles(&on_day), ["Here", "Both", "Current"]);
        assert_eq!(
            on_this_day(&library.conn, 2027, 5, 21, &[]).unwrap().len(),
            4
        );

        let current = library
            .conn
            .query_row("SELECT id FROM albums WHERE title = 'Current'", [], |row| {
                row.get(0)
            })
            .unwrap();
        let mut genre = more_in_genre(&library.conn, current, "Jazz", 1, 5, &[away])
            .unwrap()
            .into_iter()
            .map(|card| card.title)
            .collect::<Vec<_>>();
        genre.sort();
        assert_eq!(genre, ["Both", "Here"]);
    }

    #[test]
    fn recently_played_albums_are_drawn_less() {
        let card = |id: i64| AlbumCard {
            id,
            title: id.to_string(),
            artist: None,
            artist_id: None,
            year: None,
            at: None,
            note: None,
        };
        let mut recent_first = 0;
        for seed in 0..400 {
            let drawn = weighted_sample(vec![(card(1), true), (card(2), false)], seed, 1);
            if drawn[0].id == 1 {
                recent_first += 1;
            }
        }
        // A weight of a quarter against one: drawn first about a fifth of the time.
        assert!((50..110).contains(&recent_first), "{recent_first}");
    }
}
