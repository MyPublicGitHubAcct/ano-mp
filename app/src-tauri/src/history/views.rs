//! Views on the listening history (PLAN.md O8, O16, O19): what was played
//! recently, the most played tracks, albums and artists of a year or a
//! month, and the history page's highlights (favourites not played for a
//! year, what was playing a year ago today, albums never played).

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::library::availability::json_ids;
use crate::library::discover::{playable_album, AlbumCard};
use crate::library::{unix_now, Error};

/// One row of recently played: a run of plays from one album collapsed
/// into the album, or a single track without one.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct RecentEntry {
    /// When the latest play in it started, Unix seconds.
    pub played_at: i64,
    pub album: Option<AlbumCard>,
    /// Newest first; one for a single track.
    pub tracks: Vec<RecentTrack>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct RecentTrack {
    pub track_id: i64,
    pub title: String,
    pub artist: Option<String>,
    /// So the UI can show it as unavailable while its folder is (H22b).
    pub folder_id: i64,
}

/// Plays looked at for recently played; enough for many screens of it.
const RECENT_PLAYS: u32 = 2000;

/// The latest `limit` entries of recently played, newest first (O16).
pub fn recently_played(conn: &Connection, limit: usize) -> Result<Vec<RecentEntry>, Error> {
    let mut statement = conn.prepare_cached(
        "SELECT p.played_at, t.id, IFNULL(t.title, t.relative_path),
                IFNULL(t.artist_credit, artist.name), t.album_id,
                al.title, album_artist.name, al.artist_id,
                (SELECT min(y.year) FROM tracks y WHERE y.album_id = t.album_id), t.folder_id
         FROM plays p JOIN tracks t ON t.id = p.track_id
         LEFT JOIN artists artist ON artist.id = t.artist_id
         LEFT JOIN albums al ON al.id = t.album_id
         LEFT JOIN artists album_artist ON album_artist.id = al.artist_id
         ORDER BY p.played_at DESC, p.id DESC LIMIT ?1",
    )?;
    let rows = statement.query_map([RECENT_PLAYS], |row| {
        let title: String = row.get(2)?;
        let album_id: Option<i64> = row.get(4)?;
        let album = match album_id {
            Some(id) => Some(AlbumCard {
                id,
                title: row.get(5)?,
                artist: row.get(6)?,
                artist_id: row.get(7)?,
                year: row.get(8)?,
                at: None,
                note: None,
            }),
            None => None,
        };
        Ok((
            row.get::<_, i64>(0)?,
            album,
            RecentTrack {
                track_id: row.get(1)?,
                title: title.rsplit('/').next().unwrap_or(&title).to_owned(),
                artist: row.get(3)?,
                folder_id: row.get(9)?,
            },
        ))
    })?;
    let mut entries: Vec<RecentEntry> = Vec::new();
    for row in rows {
        let (played_at, album, track) = row?;
        if let Some(last) = entries.last_mut() {
            let same_album = matches!((&last.album, &album), (Some(a), Some(b)) if a.id == b.id);
            if same_album {
                if !last.tracks.iter().any(|t| t.track_id == track.track_id) {
                    last.tracks.push(track);
                }
                continue;
            }
        }
        if entries.len() == limit {
            break;
        }
        entries.push(RecentEntry {
            played_at,
            album,
            tracks: vec![track],
        });
    }
    Ok(entries)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum TopKind {
    Tracks,
    Albums,
    Artists,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct TopEntry {
    pub id: i64,
    pub title: String,
    pub subtitle: Option<String>,
    pub album_id: Option<i64>,
    pub plays: u32,
    /// What "Play these" queues for it: the track, the album's tracks, or
    /// the artist's most played track of the period.
    pub track_ids: Vec<i64>,
    /// The folders of those tracks, so the UI can show the entry as
    /// unavailable while none of them can be read (H22b).
    pub folder_ids: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct TopPlayed {
    pub entries: Vec<TopEntry>,
    /// Plays in the period.
    pub plays: u32,
    /// The first and last years with any plays, to step between.
    pub years: Option<(i32, i32)>,
}

/// How many entries the top lists hold.
pub const TOP_COUNT: u32 = 20;

/// The most played `kind` in `year` (and `month`, 1–12, if given), in the
/// local time zone (O19).
pub fn top_played(
    conn: &Connection,
    kind: TopKind,
    year: i32,
    month: Option<u32>,
) -> Result<TopPlayed, Error> {
    let (from, to) = period(conn, year, month)?;
    let plays: u32 = conn.query_row(
        "SELECT count(*) FROM plays WHERE played_at >= ?1 AND played_at < ?2",
        [from, to],
        |row| row.get(0),
    )?;
    let years: Option<(i32, i32)> = conn.query_row(
        "SELECT CAST(strftime('%Y', min(played_at), 'unixepoch', 'localtime') AS INTEGER),
                CAST(strftime('%Y', max(played_at), 'unixepoch', 'localtime') AS INTEGER)
         FROM plays",
        [],
        |row| {
            Ok(row
                .get::<_, Option<i32>>(0)?
                .zip(row.get::<_, Option<i32>>(1)?))
        },
    )?;
    let (group, columns) = match kind {
        TopKind::Tracks => (
            "t.id",
            "t.id, IFNULL(t.title, t.relative_path), IFNULL(t.artist_credit, artist.name), t.album_id",
        ),
        TopKind::Albums => (
            "t.album_id",
            "t.album_id, al.title, album_artist.name, t.album_id",
        ),
        TopKind::Artists => ("t.artist_id", "t.artist_id, artist.name, NULL, NULL"),
    };
    let mut statement = conn.prepare(&format!(
        "SELECT {columns}, count(*) AS n
         FROM plays p JOIN tracks t ON t.id = p.track_id
         LEFT JOIN artists artist ON artist.id = t.artist_id
         LEFT JOIN albums al ON al.id = t.album_id
         LEFT JOIN artists album_artist ON album_artist.id = al.artist_id
         WHERE p.played_at >= ?1 AND p.played_at < ?2 AND {group} IS NOT NULL
         GROUP BY {group} ORDER BY n DESC, max(p.played_at) DESC LIMIT ?3"
    ))?;
    let rows: Vec<(i64, String, Option<String>, Option<i64>, u32)> = statement
        .query_map(params![from, to, TOP_COUNT], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })?
        .collect::<Result<_, _>>()?;
    let mut entries = Vec::new();
    for (id, title, subtitle, album_id, plays) in rows {
        let track_ids = match kind {
            TopKind::Tracks => vec![id],
            TopKind::Albums => album_tracks(conn, id)?,
            TopKind::Artists => most_played_track(conn, id, from, to)?.into_iter().collect(),
        };
        entries.push(TopEntry {
            id,
            title: title.rsplit('/').next().unwrap_or(&title).to_owned(),
            subtitle,
            album_id,
            plays,
            folder_ids: folders_of(conn, &track_ids)?,
            track_ids,
        });
    }
    Ok(TopPlayed {
        entries,
        plays,
        years,
    })
}

/// The Unix seconds a local year (or month) starts and ends at.
fn period(conn: &Connection, year: i32, month: Option<u32>) -> Result<(i64, i64), Error> {
    let (start, end) = match month {
        Some(month) if (1..=12).contains(&month) => {
            let start = format!("{year:04}-{month:02}-01");
            (start.clone(), (start, "+1 month"))
        }
        Some(_) => return Err(Error::Invalid("A month is 1 to 12".into())),
        None => {
            let start = format!("{year:04}-01-01");
            (start.clone(), (start, "+1 year"))
        }
    };
    let bounds = conn.query_row(
        "SELECT CAST(strftime('%s', ?1, 'utc') AS INTEGER),
                CAST(strftime('%s', ?2, ?3, 'utc') AS INTEGER)",
        params![start, end.0, end.1],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    Ok(bounds)
}

/// The folders holding `track_ids`, each once, in id order.
fn folders_of(conn: &Connection, track_ids: &[i64]) -> Result<Vec<i64>, Error> {
    let mut statement = conn.prepare_cached(
        "SELECT DISTINCT folder_id FROM tracks
         WHERE id IN (SELECT value FROM json_each(?1)) ORDER BY folder_id",
    )?;
    let ids = statement
        .query_map([json_ids(track_ids)], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(ids)
}

fn album_tracks(conn: &Connection, album_id: i64) -> Result<Vec<i64>, Error> {
    let mut statement = conn.prepare_cached(
        "SELECT id FROM tracks WHERE album_id = ?1
         ORDER BY IFNULL(disc_number, 1), track_number NULLS LAST, relative_path, range_start",
    )?;
    let ids = statement
        .query_map([album_id], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(ids)
}

fn most_played_track(
    conn: &Connection,
    artist_id: i64,
    from: i64,
    to: i64,
) -> Result<Option<i64>, Error> {
    let mut statement = conn.prepare_cached(
        "SELECT t.id FROM plays p JOIN tracks t ON t.id = p.track_id
         WHERE t.artist_id = ?1 AND p.played_at >= ?2 AND p.played_at < ?3
         GROUP BY t.id ORDER BY count(*) DESC, max(p.played_at) DESC LIMIT 1",
    )?;
    let mut rows = statement.query_map(params![artist_id, from, to], |row| row.get(0))?;
    Ok(rows.next().transpose()?)
}

/// The history page's highlights.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Highlights {
    /// Albums played often (five times or more) but not in the last year.
    pub forgotten: Vec<AlbumCard>,
    /// Albums played on this day a year ago.
    pub year_ago: Vec<AlbumCard>,
    /// A few albums never played, at random.
    pub never_played: Vec<AlbumCard>,
    /// Every play counted so far.
    pub total_plays: u32,
}

const HIGHLIGHTS: u32 = 12;

/// The highlights, leaving out albums only in the folders `unreadable`
/// (PLAN.md H22b).
pub fn highlights(conn: &Connection, unreadable: &[i64]) -> Result<Highlights, Error> {
    let now = unix_now();
    let year_ago = now - 365 * 86400;
    let folders = json_ids(unreadable);
    let card_columns = "al.id, al.title, ar.name, al.artist_id,
         (SELECT min(y.year) FROM tracks y WHERE y.album_id = al.id)";
    let cards = |sql: &str,
                 values: &[&dyn rusqlite::ToSql],
                 note: &dyn Fn(i64) -> Option<String>|
     -> Result<Vec<AlbumCard>, Error> {
        let mut statement = conn.prepare(sql)?;
        let rows = statement.query_map(values, |row| {
            let extra: i64 = row.get(5)?;
            Ok(AlbumCard {
                id: row.get(0)?,
                title: row.get(1)?,
                artist: row.get(2)?,
                artist_id: row.get(3)?,
                year: row.get(4)?,
                at: None,
                note: note(extra),
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    };
    let forgotten = cards(
        &format!(
            "SELECT {card_columns}, count(*) AS n
             FROM plays p JOIN tracks t ON t.id = p.track_id JOIN albums al ON al.id = t.album_id
             LEFT JOIN artists ar ON ar.id = al.artist_id
             WHERE {playable}
             GROUP BY al.id HAVING n >= 5 AND max(p.played_at) < ?1
             ORDER BY n DESC LIMIT ?2",
            playable = playable_album(3)
        ),
        &[&year_ago, &HIGHLIGHTS, &folders],
        &|n| Some(format!("{n} plays")),
    )?;
    let a_year_ago = cards(
        &format!(
            "SELECT {card_columns}, count(*)
             FROM plays p JOIN tracks t ON t.id = p.track_id JOIN albums al ON al.id = t.album_id
             LEFT JOIN artists ar ON ar.id = al.artist_id
             WHERE date(p.played_at, 'unixepoch', 'localtime') =
                   date(?1, 'unixepoch', 'localtime', '-1 year')
               AND {playable}
             GROUP BY al.id ORDER BY min(p.played_at) LIMIT ?2",
            playable = playable_album(3)
        ),
        &[&now, &HIGHLIGHTS, &folders],
        &|_| None,
    )?;
    let never_played = cards(
        &format!(
            "SELECT {card_columns}, 0 FROM albums al LEFT JOIN artists ar ON ar.id = al.artist_id
             WHERE NOT EXISTS (SELECT 1 FROM plays p JOIN tracks t ON t.id = p.track_id
                               WHERE t.album_id = al.id)
               AND {playable}
             ORDER BY random() LIMIT ?1",
            playable = playable_album(2)
        ),
        &[&HIGHLIGHTS, &folders],
        &|_| None,
    )?;
    let total_plays = conn.query_row("SELECT count(*) FROM plays", [], |row| row.get(0))?;
    Ok(Highlights {
        forgotten,
        year_ago: a_year_ago,
        never_played,
        total_plays,
    })
}

/// Forgets every play (the history page's "Clear history").
pub fn clear(conn: &Connection) -> Result<(), Error> {
    conn.execute("DELETE FROM plays", [])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::record;
    use crate::library::test_library::{track, Library};

    fn library() -> Library {
        Library::new([
            track("a/1.flac").title("One").artist("A").album("Alpha"),
            track("a/2.flac").title("Two").artist("A").album("Alpha"),
            track("b/1.flac").title("Three").artist("B").album("Beta"),
            track("loose.flac").title("Loose").artist("C"),
        ])
    }

    /// Unix seconds for a local date and time, as SQLite sees it.
    fn local(conn: &Connection, when: &str) -> i64 {
        conn.query_row(
            "SELECT CAST(strftime('%s', ?1, 'utc') AS INTEGER)",
            [when],
            |row| row.get(0),
        )
        .unwrap()
    }

    #[test]
    fn recently_played_collapses_runs_from_one_album() {
        let library = library();
        let conn = &library.conn;
        for (track, at) in [(1, 10), (2, 20), (4, 30), (3, 40), (1, 50), (2, 60)] {
            record(conn, track, at, 60.0).unwrap();
        }
        let entries = recently_played(conn, 10).unwrap();
        let summary: Vec<(Option<&str>, Vec<&str>)> = entries
            .iter()
            .map(|entry| {
                (
                    entry.album.as_ref().map(|album| album.title.as_str()),
                    entry.tracks.iter().map(|t| t.title.as_str()).collect(),
                )
            })
            .collect();
        assert_eq!(
            summary,
            [
                (Some("Alpha"), vec!["Two", "One"]),
                (Some("Beta"), vec!["Three"]),
                (None, vec!["Loose"]),
                (Some("Alpha"), vec!["Two", "One"]),
            ]
        );
        assert_eq!(entries[0].played_at, 60);
        assert_eq!(recently_played(conn, 2).unwrap().len(), 2);
    }

    #[test]
    fn top_played_counts_a_year_or_a_month() {
        let library = library();
        let conn = &library.conn;
        let march = local(conn, "2026-03-10 12:00:00");
        let april = local(conn, "2026-04-10 12:00:00");
        let last_year = local(conn, "2025-12-31 23:30:00");
        for (track, at) in [
            (1, march),
            (1, march + 1),
            (3, april),
            (3, april + 1),
            (3, april + 2),
            (2, last_year),
        ] {
            record(conn, track, at, 60.0).unwrap();
        }
        let tracks = top_played(conn, TopKind::Tracks, 2026, None).unwrap();
        let titles: Vec<&str> = tracks.entries.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(titles, ["Three", "One"]);
        assert_eq!(tracks.entries[0].plays, 3);
        assert_eq!(tracks.plays, 5);
        assert_eq!(tracks.years, Some((2025, 2026)));

        let march_only = top_played(conn, TopKind::Tracks, 2026, Some(3)).unwrap();
        assert_eq!(march_only.entries.len(), 1);
        assert_eq!(march_only.entries[0].title, "One");

        let albums = top_played(conn, TopKind::Albums, 2026, None).unwrap();
        assert_eq!(albums.entries[0].title, "Beta");
        assert_eq!(albums.entries[1].title, "Alpha");
        assert_eq!(albums.entries[1].track_ids.len(), 2, "the whole album");

        let artists = top_played(conn, TopKind::Artists, 2025, None).unwrap();
        assert_eq!(artists.entries[0].title, "A");
        assert_eq!(artists.entries[0].track_ids, [2]);

        assert!(top_played(conn, TopKind::Tracks, 2026, Some(13)).is_err());
    }

    #[test]
    fn highlights_find_forgotten_and_unplayed_albums() {
        let library = library();
        let conn = &library.conn;
        let long_ago = unix_now() - 400 * 86400;
        for i in 0..5 {
            record(conn, 1, long_ago + i, 60.0).unwrap();
        }
        let a_year_ago: i64 = conn
            .query_row(
                "SELECT CAST(strftime('%s', 'now', '-1 year') AS INTEGER)",
                [],
                |row| row.get(0),
            )
            .unwrap();
        record(conn, 3, a_year_ago, 60.0).unwrap();
        let highlights = highlights(conn, &[]).unwrap();
        assert_eq!(highlights.forgotten.len(), 1);
        assert_eq!(highlights.forgotten[0].title, "Alpha");
        assert_eq!(highlights.forgotten[0].note.as_deref(), Some("5 plays"));
        assert_eq!(highlights.year_ago.len(), 1);
        assert_eq!(highlights.year_ago[0].title, "Beta");
        assert!(highlights.never_played.is_empty());
        assert_eq!(highlights.total_plays, 6);

        clear(conn).unwrap();
        let empty = super::highlights(conn, &[]).unwrap();
        assert_eq!(empty.never_played.len(), 2);
        assert_eq!(empty.total_plays, 0);
    }

    #[test]
    fn highlights_leave_out_albums_of_unreadable_folders() {
        let library = library();
        let conn = &library.conn;
        let away = library.add_folder("/Volumes/Away");
        conn.execute(
            "UPDATE tracks SET folder_id = ?1 WHERE relative_path = 'b/1.flac'",
            [away],
        )
        .unwrap();
        let long_ago = unix_now() - 400 * 86400;
        for i in 0..5 {
            record(conn, 3, long_ago + i, 60.0).unwrap();
        }
        assert_eq!(highlights(conn, &[]).unwrap().forgotten[0].title, "Beta");
        let left = highlights(conn, &[away]).unwrap();
        assert!(left.forgotten.is_empty());
        let never: Vec<&str> = left.never_played.iter().map(|a| a.title.as_str()).collect();
        assert_eq!(never, ["Alpha"]);
        assert_eq!(left.total_plays, 5);

        // History itself keeps them, with their folders.
        let recent = recently_played(conn, 10).unwrap();
        assert_eq!(recent[0].tracks[0].folder_id, away);
        let year: i32 = conn
            .query_row(
                "SELECT CAST(strftime('%Y', ?1, 'unixepoch', 'localtime') AS INTEGER)",
                [long_ago],
                |row| row.get(0),
            )
            .unwrap();
        let top = top_played(conn, TopKind::Albums, year, None).unwrap();
        assert_eq!(top.entries[0].title, "Beta");
        assert_eq!(top.entries[0].folder_ids, [away]);
    }
}
